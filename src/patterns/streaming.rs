//! Real-time signal generation streaming pipeline
//!
//! This module provides streaming signal generation with event-driven processing,
//! caching, and integration with the event bus for real-time trading signals.
//!
//! # Architecture
//!
//! The streaming pipeline processes market data and pattern results in real-time:
//!
//! ```text
//! ┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
//! │  Market Data    │ -> │  Pattern Engine  │ -> │  Signal Engine  │
//! │   Stream        │    │   Detection      │    │   Generation    │
//! └─────────────────┘    └──────────────────┘    └─────────────────┘
//!        │                       │                       │
//!        ▼                       ▼                       ▼
//! ┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
//! │  Event Bus      │    │  Event Bus       │    │  Event Bus      │
//! │  (Candles)      │    │  (Patterns)      │    │  (Signals)      │
//! └─────────────────┘    └──────────────────┘    └─────────────────┘
//! ```
//!
//! # Features
//!
//! - **Real-time Processing**: Process market data as it arrives
//! - **Event Integration**: Seamless integration with the event bus
//! - **Signal Caching**: Prevent duplicate signals and improve performance
//! - **Streaming Composition**: Combine multiple signal sources
//! - **Backpressure Handling**: Graceful handling of high-throughput scenarios

use crate::events::{EventBus, MarketEvent, filters};
use crate::patterns::signals::{SignalEngine, TradingSignal, SignalConfig};
use crate::patterns::types::PatternResult;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

/// Real-time signal generation pipeline
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
        let mut pattern_stream = filters::filter_patterns(filters::to_stream(event_bus.subscribe()));

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
                    None
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
        candle_buffer: &[(crate::brokers::types::Symbol, Arc<crate::events::types::Candle>)],
        pattern_buffer: &[crate::events::types::PatternSignal],
    ) {
        // Convert candle buffer to slice of candles
        let candles: Vec<_> = candle_buffer.iter().map(|(_, candle)| (**candle).clone()).collect();

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
    use crate::events::EventBus;
    use crate::brokers::types::{Symbol, Confidence, Price};
    use crate::events::types::{Candle, SignalType};
    use rust_decimal::Decimal;
    use std::time::Duration;

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
        assert!(matches!(pipeline.start().await, Err(StreamingError::AlreadyRunning)));
    }

    #[tokio::test]
    async fn test_pipeline_double_stop() {
        let event_bus = EventBus::new(100);
        let config = StreamingConfig::default();
        let pipeline = SignalStreamingPipeline::new(event_bus, config);

        assert!(matches!(pipeline.stop().await, Err(StreamingError::NotRunning)));
    }
}