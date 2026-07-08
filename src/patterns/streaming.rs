//! Real-time pattern detection and signal generation streaming pipeline
//!
//! This module provides a complete event-driven streaming pipeline that:
//! 1. Subscribes to NewCandle events from the event bus
//! 2. Maintains a candle cache for pattern detection
//! 3. Detects patterns using the pattern registry
//! 4. Generates signals using the signal engine
//! 5. Publishes patterns and signals back to the event bus
//! 6. Persists signals to the database
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
//! │  Event Bus      │ -> │  Candle Cache    │ -> │  Pattern        │
//! │  (NewCandle)    │    │  (100 candles)   │    │  Detection      │
//! └─────────────────┘    └──────────────────┘    └─────────────────┘
//!        │                                                │
//!        ▼                                                ▼
//! ┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
//! │  Event Bus      │ <- │  Signal Engine   │ <- │  Event Bus      │
//! │  (Signals)      │    │  Generation      │    │  (Patterns)     │
//! └─────────────────┘    └──────────────────┘    └─────────────────┘
//!        │
//!        ▼
//! ┌─────────────────┐
//! │  Database       │
//! │  (Persistence)  │
//! └─────────────────┘
//! ```

use crate::brokers::types::Symbol;
use crate::db::repositories::SignalRepository;
use crate::events::types::{Candle, PatternSignal, SignalType};
use crate::events::{EventBus, MarketEvent, filters};
use crate::patterns::registry::PatternRegistry;
use crate::patterns::signals::SignalEngine;
use crate::patterns::types::PatternResult;
use futures::StreamExt;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Cache for recent candles per symbol
///
/// Maintains a fixed-size cache of recent candles for each symbol.
/// This enables pattern detection algorithms to analyze historical data.
#[derive(Debug, Clone)]
pub struct CandleCache {
    /// Candles stored by symbol
    candles: HashMap<String, VecDeque<Candle>>,
    /// Maximum number of candles to store per symbol
    max_candles_per_symbol: usize,
}

impl CandleCache {
    /// Creates a new candle cache
    ///
    /// # Arguments
    ///
    /// * `max_candles_per_symbol` - Maximum number of candles to cache per symbol (typically 100)
    pub fn new(max_candles_per_symbol: usize) -> Self {
        Self {
            candles: HashMap::new(),
            max_candles_per_symbol,
        }
    }

    /// Adds a candle to the cache
    ///
    /// If the cache exceeds the maximum size, the oldest candle is removed.
    pub fn add_candle(&mut self, symbol: &str, candle: Candle) {
        let deque = self
            .candles
            .entry(symbol.to_string())
            .or_insert_with(VecDeque::new);

        deque.push_back(candle);

        // Remove oldest candle if we exceed the limit
        if deque.len() > self.max_candles_per_symbol {
            deque.pop_front();
        }
    }

    /// Gets the most recent N candles for a symbol
    ///
    /// Returns an empty vector if the symbol is not in the cache.
    pub fn get_recent_candles(&self, symbol: &str, count: usize) -> Vec<Candle> {
        self.candles
            .get(symbol)
            .map(|deque| deque.iter().rev().take(count).rev().cloned().collect())
            .unwrap_or_default()
    }

    /// Gets all cached candles for a symbol
    pub fn get_all_candles(&self, symbol: &str) -> Vec<Candle> {
        self.candles
            .get(symbol)
            .map(|deque| deque.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Returns the number of symbols being tracked
    pub fn symbol_count(&self) -> usize {
        self.candles.len()
    }

    /// Returns the total number of candles cached
    pub fn total_candles(&self) -> usize {
        self.candles.values().map(|deque| deque.len()).sum()
    }
}

/// Metrics for the pattern streaming pipeline
#[derive(Debug)]
pub struct PatternPipelineMetrics {
    /// Total candles processed
    pub candles_processed: AtomicU64,
    /// Total patterns detected
    pub patterns_detected: AtomicU64,
    /// Total signals generated
    pub signals_generated: AtomicU64,
    /// Database writes completed
    pub database_writes: AtomicU64,
    /// Processing errors encountered
    pub processing_errors: AtomicU64,
    /// Cumulative processing time in nanoseconds
    pub total_processing_time_ns: AtomicU64,
}

impl PatternPipelineMetrics {
    /// Creates new metrics
    pub fn new() -> Self {
        Self {
            candles_processed: AtomicU64::new(0),
            patterns_detected: AtomicU64::new(0),
            signals_generated: AtomicU64::new(0),
            database_writes: AtomicU64::new(0),
            processing_errors: AtomicU64::new(0),
            total_processing_time_ns: AtomicU64::new(0),
        }
    }

    /// Records a candle being processed
    pub fn record_candle_processed(&self, duration: Duration) {
        self.candles_processed.fetch_add(1, Ordering::Relaxed);
        self.total_processing_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Records a pattern detection
    pub fn record_pattern_detected(&self) {
        self.patterns_detected.fetch_add(1, Ordering::Relaxed);
    }

    /// Records a signal generation
    pub fn record_signal_generated(&self) {
        self.signals_generated.fetch_add(1, Ordering::Relaxed);
    }

    /// Records a database write
    pub fn record_database_write(&self) {
        self.database_writes.fetch_add(1, Ordering::Relaxed);
    }

    /// Records an error
    pub fn record_error(&self) {
        self.processing_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Gets current statistics
    pub fn get_statistics(&self) -> PipelineStatistics {
        let candles = self.candles_processed.load(Ordering::Relaxed);
        let total_time_ns = self.total_processing_time_ns.load(Ordering::Relaxed);

        PipelineStatistics {
            candles_processed: candles,
            patterns_detected: self.patterns_detected.load(Ordering::Relaxed),
            signals_generated: self.signals_generated.load(Ordering::Relaxed),
            database_writes: self.database_writes.load(Ordering::Relaxed),
            processing_errors: self.processing_errors.load(Ordering::Relaxed),
            avg_processing_time_ns: if candles > 0 {
                total_time_ns / candles
            } else {
                0
            },
        }
    }
}

impl Default for PatternPipelineMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics snapshot from the pipeline
#[derive(Debug, Clone)]
pub struct PipelineStatistics {
    pub candles_processed: u64,
    pub patterns_detected: u64,
    pub signals_generated: u64,
    pub database_writes: u64,
    pub processing_errors: u64,
    pub avg_processing_time_ns: u64,
}

/// Pattern streaming pipeline that integrates candle events -> patterns -> signals -> database
///
/// This is the main streaming pipeline for Phase 3D-1 that:
/// - Subscribes to NewCandle events
/// - Maintains candle cache for pattern detection
/// - Detects patterns using PatternRegistry
/// - Generates signals using SignalEngine
/// - Publishes pattern and signal events
/// - Persists signals to database
pub struct PatternStreamingPipeline {
    event_bus: EventBus,
    pattern_registry: Arc<PatternRegistry>,
    signal_engine: Arc<RwLock<SignalEngine>>,
    signal_repository: Arc<SignalRepository>,
    candle_cache: Arc<RwLock<CandleCache>>,
    metrics: Arc<PatternPipelineMetrics>,
    task_handle: Option<JoinHandle<()>>,
}

impl PatternStreamingPipeline {
    /// Creates a new pattern streaming pipeline
    pub fn new(
        event_bus: EventBus,
        pattern_registry: Arc<PatternRegistry>,
        signal_engine: Arc<RwLock<SignalEngine>>,
        signal_repository: Arc<SignalRepository>,
    ) -> Self {
        Self {
            event_bus,
            pattern_registry,
            signal_engine,
            signal_repository,
            candle_cache: Arc::new(RwLock::new(CandleCache::new(100))),
            metrics: Arc::new(PatternPipelineMetrics::new()),
            task_handle: None,
        }
    }

    /// Starts the streaming pipeline
    ///
    /// Spawns a background task that processes candles through the full pipeline.
    pub fn start(&mut self) {
        if self.task_handle.is_some() {
            warn!("Pattern streaming pipeline already started");
            return;
        }

        info!("Starting pattern streaming pipeline");

        // Clone references for the spawned task
        let event_bus = self.event_bus.clone();
        let pattern_registry = Arc::clone(&self.pattern_registry);
        let signal_engine = Arc::clone(&self.signal_engine);
        let signal_repository = Arc::clone(&self.signal_repository);
        let candle_cache = Arc::clone(&self.candle_cache);
        let metrics = Arc::clone(&self.metrics);

        let handle = tokio::spawn(async move {
            Self::run_pipeline(
                event_bus,
                pattern_registry,
                signal_engine,
                signal_repository,
                candle_cache,
                metrics,
            )
            .await;
        });

        self.task_handle = Some(handle);
    }

    /// Stops the streaming pipeline
    pub async fn stop(&mut self) {
        if let Some(handle) = self.task_handle.take() {
            info!("Stopping pattern streaming pipeline");
            handle.abort();
            let _ = handle.await;
        }
    }

    /// Gets current pipeline statistics
    pub fn get_statistics(&self) -> PipelineStatistics {
        self.metrics.get_statistics()
    }

    /// Main pipeline execution loop
    async fn run_pipeline(
        event_bus: EventBus,
        pattern_registry: Arc<PatternRegistry>,
        signal_engine: Arc<RwLock<SignalEngine>>,
        signal_repository: Arc<SignalRepository>,
        candle_cache: Arc<RwLock<CandleCache>>,
        metrics: Arc<PatternPipelineMetrics>,
    ) {
        // Subscribe to NewCandle events
        let mut candle_stream = filters::filter_candles(filters::to_stream(event_bus.subscribe()));

        info!("Pattern streaming pipeline started, listening for candles");

        while let Some((symbol, candle)) = candle_stream.next().await {
            let start_time = Instant::now();

            match Self::process_new_candle(
                &symbol,
                &candle,
                &pattern_registry,
                &signal_engine,
                &signal_repository,
                &event_bus,
                &candle_cache,
            )
            .await
            {
                Ok(_) => {
                    metrics.record_candle_processed(start_time.elapsed());
                    debug!(
                        "Processed candle for {} in {:?}",
                        symbol.as_str(),
                        start_time.elapsed()
                    );
                }
                Err(e) => {
                    metrics.record_error();
                    error!("Error processing candle for {}: {:?}", symbol.as_str(), e);
                }
            }
        }

        info!("Pattern streaming pipeline stopped");
    }

    /// Processes a single new candle through the full pipeline
    async fn process_new_candle(
        symbol: &Symbol,
        candle: &Arc<Candle>,
        pattern_registry: &Arc<PatternRegistry>,
        signal_engine: &Arc<RwLock<SignalEngine>>,
        signal_repository: &Arc<SignalRepository>,
        event_bus: &EventBus,
        candle_cache: &Arc<RwLock<CandleCache>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Step 1: Add candle to cache
        {
            let mut cache = candle_cache.write().await;
            cache.add_candle(symbol.as_str(), (**candle).clone());
        }

        // Step 2: Get cached candles for pattern detection
        let cached_candles = {
            let cache = candle_cache.read().await;
            cache.get_all_candles(symbol.as_str())
        };

        // Need at least a few candles for pattern detection
        if cached_candles.len() < 3 {
            debug!(
                "Not enough candles for pattern detection: {} (need 3+)",
                cached_candles.len()
            );
            return Ok(());
        }

        // Step 3: Detect patterns. Detectors index candles newest-first
        // (candles[0] is the bar under evaluation), while the cache — and the
        // signal engine's context analysis below — are chronological, so the
        // detection window is a reversed copy.
        let detection_window: Vec<Candle> = cached_candles.iter().rev().cloned().collect();
        let pattern_results = pattern_registry.detect_all(&detection_window, None).await;

        // Step 4: Publish PatternDetected events
        for pattern_result in &pattern_results {
            let pattern_signal = PatternSignal {
                symbol: symbol.clone(),
                pattern_name: pattern_result.pattern_name.clone(),
                signal_type: pattern_result.signal_type.clone(),
                confidence: pattern_result.confidence,
                detected_at: pattern_result.detected_at,
                price: pattern_result.price_level,
            };

            event_bus.publish(MarketEvent::PatternDetected {
                signal: pattern_signal,
            });

            debug!(
                "Published PatternDetected: {} for {} with confidence {:.2}",
                pattern_result.pattern_name.as_str(),
                symbol.as_str(),
                pattern_result.confidence.value()
            );
        }

        // Step 5: Generate signals from patterns
        if !pattern_results.is_empty() {
            let mut engine = signal_engine.write().await;

            match engine
                .generate_signals(&pattern_results, &cached_candles)
                .await
            {
                Ok(signals) => {
                    for signal in signals {
                        // Step 6: Publish SignalGenerated events
                        event_bus.publish(MarketEvent::SignalGenerated {
                            signal: signal.clone(),
                        });

                        info!(
                            "Generated signal: {} {} with confidence {:.2}",
                            signal.symbol.as_str(),
                            signal.signal_type.as_str(),
                            signal.confidence.value()
                        );

                        // Step 7: Persist signal to database (async, non-blocking)
                        let repo = Arc::clone(signal_repository);
                        let signal_clone = signal.clone();
                        tokio::spawn(async move {
                            match repo.store_signal(&signal_clone).await {
                                Ok(_) => {
                                    debug!(
                                        "Persisted signal for {} to database",
                                        signal_clone.symbol.as_str()
                                    );
                                }
                                Err(e) => {
                                    error!("Failed to persist signal to database: {:?}", e);
                                }
                            }
                        });
                    }
                }
                Err(e) => {
                    error!("Error generating signals: {:?}", e);
                    return Err(Box::new(e));
                }
            }
        }

        Ok(())
    }
}

// Keep the existing SignalStreamingPipeline for backward compatibility
// (The code below this line is the original implementation)

use crate::patterns::signals::{SignalConfig, TradingSignal};

/// Real-time signal generation pipeline (original implementation)
pub struct SignalStreamingPipeline {
    /// Signal engine for generating signals
    signal_engine: Arc<RwLock<SignalEngine>>,
    /// Event bus for publishing signals
    event_bus: EventBus,
    /// Pipeline configuration
    config: StreamingConfig,
    /// Running state
    running: Arc<RwLock<bool>>,
}

impl SignalStreamingPipeline {
    /// Creates a new streaming pipeline
    pub fn new(event_bus: EventBus, config: StreamingConfig) -> Self {
        let signal_engine = Arc::new(RwLock::new(SignalEngine::new(config.signal_config.clone())));

        Self {
            signal_engine,
            event_bus,
            config,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Starts the streaming pipeline
    pub async fn start(&self) -> Result<(), StreamingError> {
        let mut running = self.running.write().await;
        if *running {
            return Err(StreamingError::AlreadyRunning);
        }
        *running = true;
        drop(running);

        info!("Starting signal streaming pipeline");

        // Clone references for the spawned task
        let event_bus = self.event_bus.clone();
        let signal_engine = Arc::clone(&self.signal_engine);
        let config = self.config.clone();
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            Self::run_pipeline(event_bus, signal_engine, config, running).await;
        });

        Ok(())
    }

    /// Stops the streaming pipeline
    pub async fn stop(&self) -> Result<(), StreamingError> {
        let mut running = self.running.write().await;
        if !*running {
            return Err(StreamingError::NotRunning);
        }
        *running = false;
        info!("Stopping signal streaming pipeline");
        Ok(())
    }

    /// Main pipeline execution loop
    async fn run_pipeline(
        event_bus: EventBus,
        signal_engine: Arc<RwLock<SignalEngine>>,
        config: StreamingConfig,
        running: Arc<RwLock<bool>>,
    ) {
        // Subscribe to market data and pattern events
        let mut market_stream = filters::filter_candles(filters::to_stream(event_bus.subscribe()));
        let mut pattern_stream =
            filters::filter_patterns(filters::to_stream(event_bus.subscribe()));

        // Buffer for accumulating data
        let mut candle_buffer = Vec::new();
        let mut pattern_buffer = Vec::new();

        let mut interval = tokio::time::interval(config.processing_interval);

        loop {
            tokio::select! {
                // Check if we should stop
                _ = async {
                    let running = running.read().await;
                    if !*running {
                        return Some(());
                    }
                    None::<()>
                } => {
                    info!("Signal streaming pipeline stopped");
                    break;
                }

                // Process market data
                candle_event = market_stream.next() => {
                    if let Some((symbol, candle)) = candle_event {
                        candle_buffer.push((symbol, Arc::clone(&candle)));

                        // Keep buffer size manageable
                        if candle_buffer.len() > config.max_buffer_size {
                            candle_buffer.remove(0);
                        }
                    }
                }

                // Process pattern results
                pattern_event = pattern_stream.next() => {
                    if let Some(pattern_result) = pattern_event {
                        pattern_buffer.push(pattern_result);

                        // Keep buffer size manageable
                        if pattern_buffer.len() > config.max_buffer_size {
                            pattern_buffer.remove(0);
                        }
                    }
                }

                // Periodic processing
                _ = interval.tick() => {
                    if !candle_buffer.is_empty() && !pattern_buffer.is_empty() {
                        Self::process_batch(
                            &signal_engine,
                            &event_bus,
                            &candle_buffer,
                            &pattern_buffer,
                        ).await;
                    }
                }
            }
        }
    }

    /// Processes a batch of market data and patterns
    async fn process_batch(
        signal_engine: &Arc<RwLock<SignalEngine>>,
        event_bus: &EventBus,
        candle_buffer: &[(
            crate::brokers::types::Symbol,
            Arc<crate::events::types::Candle>,
        )],
        pattern_buffer: &[crate::events::types::PatternSignal],
    ) {
        // Convert candle buffer to slice of candles
        let candles: Vec<_> = candle_buffer
            .iter()
            .map(|(_, candle)| (**candle).clone())
            .collect();

        // Convert PatternSignals to PatternResults for signal generation
        let pattern_results: Vec<crate::patterns::types::PatternResult> = pattern_buffer
            .iter()
            .map(|signal| {
                crate::patterns::types::PatternResult::new(
                    crate::brokers::types::PatternName::new(signal.pattern_name.as_str()),
                    signal.signal_type.clone(),
                    signal.confidence,
                    signal.price,
                )
            })
            .collect();

        // Generate signals
        let mut engine = signal_engine.write().await;
        match engine.generate_signals(&pattern_results, &candles).await {
            Ok(signals) => {
                for signal in signals {
                    // Publish signal to event bus
                    let event = MarketEvent::SignalGenerated {
                        signal: signal.clone(),
                    };

                    event_bus.publish(event);

                    info!(
                        "Generated signal: {} {} with confidence {:.2}",
                        signal.symbol.as_str(),
                        signal.signal_type.as_str(),
                        signal.confidence.value()
                    );
                }
            }
            Err(e) => {
                error!("Error generating signals: {:?}", e);
            }
        }
    }

    /// Returns pipeline statistics
    pub async fn stats(&self) -> PipelineStats {
        let engine = self.signal_engine.read().await;
        let (cache_size, max_cache) = engine.cache_stats().await;
        let running = *self.running.read().await;

        PipelineStats {
            running,
            cache_size,
            max_cache_size: max_cache,
            processing_interval: self.config.processing_interval,
        }
    }
}

/// Configuration for the streaming pipeline
#[derive(Clone, Debug)]
pub struct StreamingConfig {
    /// Signal engine configuration
    pub signal_config: SignalConfig,
    /// Processing interval for batch processing
    pub processing_interval: std::time::Duration,
    /// Maximum buffer size for candles and patterns
    pub max_buffer_size: usize,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            signal_config: SignalConfig::default(),
            processing_interval: std::time::Duration::from_millis(100), // 10Hz processing
            max_buffer_size: 1000,
        }
    }
}

/// Pipeline statistics
#[derive(Clone, Debug)]
pub struct PipelineStats {
    /// Whether the pipeline is running
    pub running: bool,
    /// Current signal cache size
    pub cache_size: usize,
    /// Maximum cache size
    pub max_cache_size: usize,
    /// Processing interval
    pub processing_interval: std::time::Duration,
}

/// Streaming pipeline errors
#[derive(Clone, Debug, thiserror::Error)]
pub enum StreamingError {
    #[error("Pipeline is already running")]
    AlreadyRunning,
    #[error("Pipeline is not running")]
    NotRunning,
    #[error("Event bus error: {message}")]
    EventBusError { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::types::{Confidence, Price, Symbol};
    use crate::events::EventBus;
    use crate::events::types::Candle;
    use rust_decimal::Decimal;
    use std::time::Duration;

    #[test]
    fn test_candle_cache_creation() {
        let cache = CandleCache::new(100);
        assert_eq!(cache.symbol_count(), 0);
        assert_eq!(cache.total_candles(), 0);
    }

    #[test]
    fn test_candle_cache_add_and_retrieve() {
        let mut cache = CandleCache::new(100);

        let candle = Candle {
            symbol: Symbol::new("AAPL").unwrap(),
            timestamp: chrono::Utc::now(),
            open: Price::new(Decimal::new(100, 0)).unwrap(),
            high: Price::new(Decimal::new(110, 0)).unwrap(),
            low: Price::new(Decimal::new(95, 0)).unwrap(),
            close: Price::new(Decimal::new(105, 0)).unwrap(),
            volume: 1000,
        };

        cache.add_candle("AAPL", candle);

        assert_eq!(cache.symbol_count(), 1);
        assert_eq!(cache.total_candles(), 1);

        let retrieved = cache.get_all_candles("AAPL");
        assert_eq!(retrieved.len(), 1);
    }

    #[test]
    fn test_candle_cache_max_size() {
        let mut cache = CandleCache::new(5);

        // Add more candles than max
        for i in 0..10 {
            let candle = Candle {
                symbol: Symbol::new("AAPL").unwrap(),
                timestamp: chrono::Utc::now(),
                open: Price::new(Decimal::new(100 + i, 0)).unwrap(),
                high: Price::new(Decimal::new(110 + i, 0)).unwrap(),
                low: Price::new(Decimal::new(95 + i, 0)).unwrap(),
                close: Price::new(Decimal::new(105 + i, 0)).unwrap(),
                volume: 1000,
            };
            cache.add_candle("AAPL", candle);
        }

        // Should only keep the last 5
        let candles = cache.get_all_candles("AAPL");
        assert_eq!(candles.len(), 5);

        // First candle should have open price of 105 (candle 5)
        assert_eq!(candles[0].open.value(), Decimal::new(105, 0));
    }

    #[test]
    fn test_candle_cache_get_recent() {
        let mut cache = CandleCache::new(100);

        // Add 10 candles
        for i in 0..10 {
            let candle = Candle {
                symbol: Symbol::new("AAPL").unwrap(),
                timestamp: chrono::Utc::now(),
                open: Price::new(Decimal::new(100 + i, 0)).unwrap(),
                high: Price::new(Decimal::new(110 + i, 0)).unwrap(),
                low: Price::new(Decimal::new(95 + i, 0)).unwrap(),
                close: Price::new(Decimal::new(105 + i, 0)).unwrap(),
                volume: 1000,
            };
            cache.add_candle("AAPL", candle);
        }

        // Get last 3 candles
        let recent = cache.get_recent_candles("AAPL", 3);
        assert_eq!(recent.len(), 3);

        // Should be candles 7, 8, 9
        assert_eq!(recent[0].open.value(), Decimal::new(107, 0));
        assert_eq!(recent[2].open.value(), Decimal::new(109, 0));
    }

    #[test]
    fn test_pipeline_metrics() {
        let metrics = PatternPipelineMetrics::new();

        metrics.record_candle_processed(Duration::from_millis(10));
        metrics.record_pattern_detected();
        metrics.record_signal_generated();
        metrics.record_error();

        let stats = metrics.get_statistics();

        assert_eq!(stats.candles_processed, 1);
        assert_eq!(stats.patterns_detected, 1);
        assert_eq!(stats.signals_generated, 1);
        assert_eq!(stats.processing_errors, 1);
        assert!(stats.avg_processing_time_ns > 0);
    }

    #[tokio::test]
    async fn test_pipeline_creation() {
        let event_bus = EventBus::new(100);
        let config = StreamingConfig::default();
        let pipeline = SignalStreamingPipeline::new(event_bus, config);

        let stats = pipeline.stats().await;
        assert!(!stats.running);
        assert_eq!(stats.max_cache_size, 1000);
    }

    #[tokio::test]
    async fn test_pipeline_start_stop() {
        let event_bus = EventBus::new(100);
        let config = StreamingConfig::default();
        let pipeline = SignalStreamingPipeline::new(event_bus, config);

        // Start pipeline
        assert!(pipeline.start().await.is_ok());

        // Check it's running
        let stats = pipeline.stats().await;
        assert!(stats.running);

        // Stop pipeline
        assert!(pipeline.stop().await.is_ok());

        // Give it a moment to stop
        tokio::time::sleep(Duration::from_millis(10)).await;

        let stats = pipeline.stats().await;
        assert!(!stats.running);
    }

    #[tokio::test]
    async fn test_pipeline_double_start() {
        let event_bus = EventBus::new(100);
        let config = StreamingConfig::default();
        let pipeline = SignalStreamingPipeline::new(event_bus, config);

        assert!(pipeline.start().await.is_ok());
        assert!(matches!(
            pipeline.start().await,
            Err(StreamingError::AlreadyRunning)
        ));
    }

    #[tokio::test]
    async fn test_pipeline_double_stop() {
        let event_bus = EventBus::new(100);
        let config = StreamingConfig::default();
        let pipeline = SignalStreamingPipeline::new(event_bus, config);

        assert!(matches!(
            pipeline.stop().await,
            Err(StreamingError::NotRunning)
        ));
    }
}

// =============================================================================
// Parallel Pattern Processing
// =============================================================================

/// Parallel pattern processor for high-throughput pattern detection
///
/// This processor uses a worker pool to detect patterns across multiple symbols
/// concurrently, enabling sub-second processing of 50+ symbols.
///
/// # Performance Characteristics
///
/// - Concurrent processing via buffer_unordered
/// - Worker pool size configurable (default: num_cpus)
/// - Zero-copy design with Arc for shared pattern registry
/// - Target: < 100μs per symbol pattern detection
///
/// # Example
///
/// ```rust,no_run
/// use daytrader::patterns::streaming::ParallelPatternProcessor;
/// use daytrader::patterns::registry::PatternRegistry;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let registry = Arc::new(PatternRegistry::new());
/// let processor = ParallelPatternProcessor::new(registry, 8);
///
/// // Process multiple symbols in parallel
/// let symbol_candles = vec![]; // Vec<(Symbol, Vec<Candle>)>
/// let results = processor.process_symbols_parallel(symbol_candles).await;
/// # Ok(())
/// # }
/// ```
pub struct ParallelPatternProcessor {
    /// Shared pattern registry for detection
    pattern_registry: Arc<PatternRegistry>,
    /// Number of concurrent workers
    worker_count: usize,
}

impl ParallelPatternProcessor {
    /// Creates a new parallel pattern processor
    ///
    /// # Arguments
    ///
    /// * `pattern_registry` - Shared pattern registry for detection
    /// * `worker_count` - Number of concurrent workers (recommend: num_cpus)
    pub fn new(pattern_registry: Arc<PatternRegistry>, worker_count: usize) -> Self {
        Self {
            pattern_registry,
            worker_count: worker_count.max(1), // At least 1 worker
        }
    }

    /// Creates a processor with default worker count (num_cpus)
    pub fn with_default_workers(pattern_registry: Arc<PatternRegistry>) -> Self {
        let worker_count = num_cpus::get();
        Self::new(pattern_registry, worker_count)
    }

    /// Process multiple symbols in parallel using worker pool
    ///
    /// This method processes pattern detection for multiple symbols concurrently,
    /// significantly improving throughput for multi-symbol scenarios.
    ///
    /// # Arguments
    ///
    /// * `symbol_candles` - Vector of (Symbol, Candles) tuples to process
    ///
    /// # Returns
    ///
    /// Vector of (Symbol, PatternResults) tuples with detected patterns
    ///
    /// # Performance
    ///
    /// - Processes up to `worker_count` symbols concurrently
    /// - Target latency: < 100μs per symbol
    /// - Scales linearly up to CPU core count
    pub async fn process_symbols_parallel(
        &self,
        symbol_candles: Vec<(Symbol, Vec<Candle>)>,
    ) -> Vec<(Symbol, Vec<PatternResult>)> {
        use futures::stream::{self, StreamExt};

        stream::iter(symbol_candles)
            .map(|(symbol, candles)| {
                let registry = self.pattern_registry.clone();
                async move {
                    let results = registry.detect_all(&candles, None).await;
                    (symbol, results)
                }
            })
            .buffer_unordered(self.worker_count)
            .collect()
            .await
    }

    /// Process symbols with custom pattern parameters
    ///
    /// # Arguments
    ///
    /// * `symbol_candles` - Vector of (Symbol, Candles) tuples to process
    /// * `params` - Pattern detection parameters
    ///
    /// # Returns
    ///
    /// Vector of (Symbol, PatternResults) tuples with detected patterns
    pub async fn process_symbols_with_params(
        &self,
        symbol_candles: Vec<(Symbol, Vec<Candle>)>,
        params: crate::patterns::types::PatternParams,
    ) -> Vec<(Symbol, Vec<PatternResult>)> {
        use futures::stream::{self, StreamExt};

        stream::iter(symbol_candles)
            .map(|(symbol, candles)| {
                let registry = self.pattern_registry.clone();
                let params = params.clone();
                async move {
                    let results = registry.detect_all(&candles, Some(&params)).await;
                    (symbol, results)
                }
            })
            .buffer_unordered(self.worker_count)
            .collect()
            .await
    }

    /// Returns the number of parallel workers
    pub fn worker_count(&self) -> usize {
        self.worker_count
    }
}

#[cfg(test)]
mod parallel_tests {
    use super::*;
    use crate::brokers::types::Price;
    use crate::patterns::types::PatternParams;
    use rust_decimal::Decimal;

    fn create_test_candle(symbol: Symbol) -> Candle {
        Candle {
            symbol,
            timestamp: chrono::Utc::now(),
            open: Price::new(Decimal::new(100, 0)).unwrap(),
            high: Price::new(Decimal::new(110, 0)).unwrap(),
            low: Price::new(Decimal::new(95, 0)).unwrap(),
            close: Price::new(Decimal::new(105, 0)).unwrap(),
            volume: 1000,
        }
    }

    #[tokio::test]
    async fn test_parallel_processor_creation() {
        let registry = Arc::new(PatternRegistry::new());
        let processor = ParallelPatternProcessor::new(registry, 4);

        assert_eq!(processor.worker_count(), 4);
    }

    #[tokio::test]
    async fn test_parallel_processing_multiple_symbols() {
        let registry = Arc::new(PatternRegistry::new());
        let processor = ParallelPatternProcessor::new(registry, 4);

        // Create test data for multiple symbols
        let mut symbol_candles = Vec::new();
        for i in 0..10 {
            let symbol = Symbol::new(&format!("TEST{}", i)).unwrap();
            let candles = vec![create_test_candle(symbol.clone())];
            symbol_candles.push((symbol, candles));
        }

        let results = processor.process_symbols_parallel(symbol_candles).await;

        assert_eq!(results.len(), 10);
    }

    #[tokio::test]
    async fn test_parallel_processing_with_params() {
        let registry = Arc::new(PatternRegistry::new());
        let processor = ParallelPatternProcessor::new(registry, 4);

        let symbol = Symbol::new("AAPL").unwrap();
        let candles = vec![create_test_candle(symbol.clone())];
        let symbol_candles = vec![(symbol, candles)];

        let params = PatternParams::new().with_min_confidence(0.7);
        let results = processor
            .process_symbols_with_params(symbol_candles, params)
            .await;

        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_default_worker_count() {
        let registry = Arc::new(PatternRegistry::new());
        let processor = ParallelPatternProcessor::with_default_workers(registry);

        // Should match number of CPUs
        assert!(processor.worker_count() > 0);
    }
}
