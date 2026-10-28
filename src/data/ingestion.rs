//! Data ingestion pipeline coordinator
//!
//! This module orchestrates the flow of market data from providers through
//! validation, caching, storage, and event publishing using functional patterns.

use crate::brokers::traits::BarAdjustment;
use crate::brokers::types::Symbol;
use crate::data::{CandleStorage, MarketDataCache, validation};
use crate::events::types::{Candle, Quote};
use crate::events::{EventBus, types::MarketEvent};
use crate::utils::errors::Result;
use std::sync::Arc;
use tracing::{debug, error, warn};

/// Corporate-action label recorded for bars arriving on the live feed.
///
/// Streaming bars carry as-traded prices, so `Raw` is their truthful
/// provenance and the only honest label to store them under.
///
/// # Closing the reconciliation gap
///
/// A detection window that splices these live raw bars onto adjusted
/// historical bars straddles two price bases, and if a split occurred during
/// the stored history the join is discontinuous. Storing everything raw
/// (as this constant already does) and adjusting at query time removes the
/// discontinuity: `crate::data::corporate_actions::adjust_candles` takes a
/// raw series spanning both the live and historical windows plus the
/// corporate-action factor table and returns one continuously-adjusted
/// series — B-086 step 2. Callers reading a range that may include both
/// live and historical bars should fetch raw (`BarAdjustment::Raw`) and run
/// it through `adjust_candles` rather than mixing `get_recent_candles`
/// results fetched under different adjustments.
const LIVE_FEED_ADJUSTMENT: BarAdjustment = BarAdjustment::Raw;

/// Data ingestion pipeline that processes market data
///
/// Coordinates the flow: Validation → Cache → Storage → Events
///
/// # Examples
///
/// ```no_run
/// use daytrader::data::{DataIngestionPipeline, MarketDataCache, CandleStorage};
/// use daytrader::events::EventBus;
/// use sqlx::SqlitePool;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let pool = SqlitePool::connect("sqlite::memory:").await?;
/// let cache = Arc::new(MarketDataCache::new(100));
/// let storage = Arc::new(CandleStorage::new(pool));
/// let event_bus = EventBus::new(1000);
///
/// let pipeline = DataIngestionPipeline::new(
///     cache,
///     storage,
///     event_bus,
/// );
/// # Ok(())
/// # }
/// ```
pub struct DataIngestionPipeline {
    cache: Arc<MarketDataCache>,
    storage: Arc<CandleStorage>,
    event_bus: EventBus,
}

impl DataIngestionPipeline {
    /// Create a new data ingestion pipeline
    ///
    /// # Arguments
    ///
    /// * `cache` - In-memory cache for market data
    /// * `storage` - Database storage for persistence
    /// * `event_bus` - Event bus for publishing events
    pub fn new(
        cache: Arc<MarketDataCache>,
        storage: Arc<CandleStorage>,
        event_bus: EventBus,
    ) -> Self {
        Self {
            cache,
            storage,
            event_bus,
        }
    }

    /// Process an incoming candle through the pipeline
    ///
    /// Pipeline stages:
    /// 1. Validate candle data (OHLC relationships)
    /// 2. Detect anomalies (price jumps, zero volume)
    /// 3. Store in cache (bounded, concurrent)
    /// 4. Store in database (async, non-blocking)
    /// 5. Publish event to event bus
    ///
    /// # Arguments
    ///
    /// * `candle` - Candle data to process
    ///
    /// # Errors
    ///
    /// Returns error if validation fails. Storage errors are logged but don't
    /// fail the pipeline (data is still cached and events are published).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use daytrader::data::DataIngestionPipeline;
    /// use daytrader::events::types::Candle;
    /// use daytrader::brokers::types::{Symbol, Price};
    /// use rust_decimal::Decimal;
    /// use chrono::Utc;
    ///
    /// # async fn example(pipeline: DataIngestionPipeline) -> Result<(), Box<dyn std::error::Error>> {
    /// let candle = Candle {
    ///     symbol: Symbol::new("AAPL"),
    ///     timestamp: Utc::now(),
    ///     open: Price::new(Decimal::new(150, 0)).unwrap(),
    ///     high: Price::new(Decimal::new(155, 0)).unwrap(),
    ///     low: Price::new(Decimal::new(148, 0)).unwrap(),
    ///     close: Price::new(Decimal::new(152, 0)).unwrap(),
    ///     volume: 1_000_000,
    /// };
    ///
    /// pipeline.process_candle(candle).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn process_candle(&self, candle: Candle) -> Result<()> {
        debug!(
            symbol = %candle.symbol,
            timestamp = %candle.timestamp,
            close = %candle.close,
            "Processing candle"
        );

        // Stage 1: Validate candle
        validation::validate_candle(&candle)?;

        // Stage 2: Detect anomalies
        let prev_candles = self.cache.get_candles(&candle.symbol, 1);
        let prev_candle = prev_candles.first().map(|c| c.as_ref());

        let anomalies = validation::detect_anomalies(&candle, prev_candle);

        if !anomalies.is_empty() {
            warn!(
                symbol = %candle.symbol,
                anomalies = ?anomalies,
                "Anomalies detected in candle"
            );
            // Continue processing but log the anomalies
        }

        // Stage 3: Store in cache
        let candle_arc = Arc::new(candle.clone());
        self.cache
            .add_candle(candle.symbol.clone(), candle_arc.clone());

        debug!(
            symbol = %candle.symbol,
            "Candle added to cache"
        );

        // Stage 4: Store in database (async, non-blocking)
        // Spawn a task so database operations don't block the pipeline
        let storage = self.storage.clone();
        let candle_for_db = candle.clone();
        tokio::spawn(async move {
            if let Err(e) = storage
                .store_candle(&candle_for_db, LIVE_FEED_ADJUSTMENT)
                .await
            {
                error!(
                    symbol = %candle_for_db.symbol,
                    error = %e,
                    "Failed to store candle in database"
                );
            } else {
                debug!(
                    symbol = %candle_for_db.symbol,
                    "Candle stored in database"
                );
            }
        });

        // Stage 5: Publish event
        self.event_bus.publish(MarketEvent::NewCandle {
            symbol: candle.symbol.clone(),
            candle: candle_arc,
        });

        debug!(
            symbol = %candle.symbol,
            "Candle event published"
        );

        Ok(())
    }

    /// Process an incoming quote through the pipeline
    ///
    /// Pipeline stages:
    /// 1. Validate quote data (bid < ask)
    /// 2. Detect anomalies (wide spreads)
    /// 3. Update cache
    /// 4. Publish event to event bus
    ///
    /// # Arguments
    ///
    /// * `symbol` - Symbol for the quote
    /// * `quote` - Quote data to process
    ///
    /// # Errors
    ///
    /// Returns error if validation fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use daytrader::data::DataIngestionPipeline;
    /// use daytrader::events::types::Quote;
    /// use daytrader::brokers::types::{Symbol, Price, Quantity};
    /// use rust_decimal::Decimal;
    /// use chrono::Utc;
    ///
    /// # async fn example(pipeline: DataIngestionPipeline) -> Result<(), Box<dyn std::error::Error>> {
    /// let symbol = Symbol::new("AAPL");
    /// let quote = Quote {
    ///     timestamp: Utc::now(),
    ///     bid: Price::new(Decimal::new(15000, 2)).unwrap(),
    ///     ask: Price::new(Decimal::new(15010, 2)).unwrap(),
    ///     bid_size: Quantity::new(100),
    ///     ask_size: Quantity::new(200),
    /// };
    ///
    /// pipeline.process_quote(symbol, quote).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn process_quote(&self, symbol: Symbol, quote: Quote) -> Result<()> {
        debug!(
            symbol = %symbol,
            bid = %quote.bid,
            ask = %quote.ask,
            "Processing quote"
        );

        // Stage 1: Validate quote
        validation::validate_quote(&quote)?;

        // Stage 2: Detect anomalies
        let anomalies = validation::detect_quote_anomalies(&quote);

        if !anomalies.is_empty() {
            warn!(
                symbol = %symbol,
                anomalies = ?anomalies,
                "Anomalies detected in quote"
            );
            // Continue processing but log the anomalies
        }

        // Stage 3: Update cache
        self.cache.update_quote(symbol.clone(), quote.clone());

        debug!(
            symbol = %symbol,
            "Quote updated in cache"
        );

        // Stage 4: Publish event
        self.event_bus.publish(MarketEvent::QuoteUpdate {
            symbol: symbol.clone(),
            quote,
        });

        debug!(
            symbol = %symbol,
            "Quote event published"
        );

        Ok(())
    }

    /// Process a batch of candles efficiently
    ///
    /// Validates all candles first, then processes them in parallel.
    /// Database storage is done in a single batch operation for efficiency.
    ///
    /// # Arguments
    ///
    /// * `candles` - Vector of candles to process
    ///
    /// # Errors
    ///
    /// Returns error if any validation fails. Stops processing at first error.
    pub async fn process_candle_batch(&self, candles: Vec<Candle>) -> Result<()> {
        if candles.is_empty() {
            return Ok(());
        }

        debug!(count = candles.len(), "Processing candle batch");

        // Validate all candles first
        for candle in &candles {
            validation::validate_candle(candle)?;
        }

        // Process each candle (cache + events)
        for candle in &candles {
            let prev_candles = self.cache.get_candles(&candle.symbol, 1);
            let prev_candle = prev_candles.first().map(|c| c.as_ref());

            let anomalies = validation::detect_anomalies(candle, prev_candle);
            if !anomalies.is_empty() {
                warn!(
                    symbol = %candle.symbol,
                    anomalies = ?anomalies,
                    "Anomalies detected in batch candle"
                );
            }

            let candle_arc = Arc::new(candle.clone());
            self.cache
                .add_candle(candle.symbol.clone(), candle_arc.clone());

            self.event_bus.publish(MarketEvent::NewCandle {
                symbol: candle.symbol.clone(),
                candle: candle_arc,
            });
        }

        // Store all in database as a batch (async, non-blocking)
        let storage = self.storage.clone();
        let candles_for_db = candles.clone();
        tokio::spawn(async move {
            if let Err(e) = storage
                .store_candles(&candles_for_db, LIVE_FEED_ADJUSTMENT)
                .await
            {
                error!(
                    count = candles_for_db.len(),
                    error = %e,
                    "Failed to store candle batch in database"
                );
            } else {
                debug!(
                    count = candles_for_db.len(),
                    "Candle batch stored in database"
                );
            }
        });

        debug!(count = candles.len(), "Candle batch processed");

        Ok(())
    }

    /// Get the cache reference
    pub fn cache(&self) -> &Arc<MarketDataCache> {
        &self.cache
    }

    /// Get the storage reference
    pub fn storage(&self) -> &Arc<CandleStorage> {
        &self.storage
    }

    /// Get the event bus reference
    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::types::{Price, Quantity};
    use chrono::{Duration, Utc};
    use rust_decimal::Decimal;
    use sqlx::SqlitePool;
    use tokio::time::{Duration as TokioDuration, sleep};

    async fn create_test_pipeline() -> DataIngestionPipeline {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test pool");

        // Create candles table
        sqlx::query(
            r"
            CREATE TABLE IF NOT EXISTS candles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                timestamp DATETIME NOT NULL,
                open REAL NOT NULL,
                high REAL NOT NULL,
                low REAL NOT NULL,
                close REAL NOT NULL,
                volume INTEGER NOT NULL,
                broker TEXT NOT NULL,
                adjustment TEXT NOT NULL DEFAULT 'raw'
                    CHECK(adjustment IN ('raw', 'split', 'dividend', 'all')),
                UNIQUE(symbol, timestamp, broker, adjustment)
            )
            ",
        )
        .execute(&pool)
        .await
        .expect("Failed to create table");

        let cache = Arc::new(MarketDataCache::new(100));
        let storage = Arc::new(CandleStorage::new(pool));
        let event_bus = EventBus::new(1000);

        DataIngestionPipeline::new(cache, storage, event_bus)
    }

    fn create_test_candle(symbol: Symbol, close: i64) -> Candle {
        Candle {
            symbol,
            timestamp: Utc::now(),
            open: Price::new(Decimal::new(close - 1, 0)).unwrap(),
            high: Price::new(Decimal::new(close + 1, 0)).unwrap(),
            low: Price::new(Decimal::new(close - 2, 0)).unwrap(),
            close: Price::new(Decimal::new(close, 0)).unwrap(),
            volume: 1000,
        }
    }

    #[tokio::test]
    async fn test_process_candle() {
        let pipeline = create_test_pipeline().await;
        let symbol = Symbol::new("AAPL").unwrap();
        let candle = create_test_candle(symbol.clone(), 150);

        let result = pipeline.process_candle(candle.clone()).await;
        assert!(result.is_ok());

        // Check cache
        let cached = pipeline.cache().get_candles(&symbol, 1);
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].close, candle.close);

        // Give database time to write
        sleep(TokioDuration::from_millis(100)).await;

        // Check database
        let stored = pipeline
            .storage()
            .get_recent_candles(&symbol, 1, LIVE_FEED_ADJUSTMENT)
            .await
            .unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].close, candle.close);
    }

    #[tokio::test]
    async fn test_process_invalid_candle() {
        let pipeline = create_test_pipeline().await;
        let symbol = Symbol::new("AAPL").unwrap();

        // Create invalid candle (high < low)
        let invalid_candle = Candle {
            symbol,
            timestamp: Utc::now(),
            open: Price::new(Decimal::new(100, 0)).unwrap(),
            high: Price::new(Decimal::new(95, 0)).unwrap(), // Invalid: high < low
            low: Price::new(Decimal::new(98, 0)).unwrap(),
            close: Price::new(Decimal::new(99, 0)).unwrap(),
            volume: 1000,
        };

        let result = pipeline.process_candle(invalid_candle).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_process_quote() {
        let pipeline = create_test_pipeline().await;
        let symbol = Symbol::new("AAPL").unwrap();

        let quote = Quote {
            timestamp: Utc::now(),
            bid: Price::new(Decimal::new(15000, 2)).unwrap(),
            ask: Price::new(Decimal::new(15010, 2)).unwrap(),
            bid_size: Quantity::new(100).unwrap(),
            ask_size: Quantity::new(200).unwrap(),
        };

        let result = pipeline.process_quote(symbol.clone(), quote.clone()).await;
        assert!(result.is_ok());

        // Check cache
        let cached = pipeline.cache().get_quote(&symbol);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().bid, quote.bid);
    }

    #[tokio::test]
    async fn test_process_candle_batch() {
        let pipeline = create_test_pipeline().await;
        let symbol = Symbol::new("AAPL").unwrap();
        let base_time = Utc::now();

        // Use distinct timestamps (1 minute apart) to avoid ON CONFLICT deduplication
        let candles: Vec<Candle> = (0..5)
            .map(|i| {
                let mut candle = create_test_candle(symbol.clone(), 150 + i);
                candle.timestamp = base_time - Duration::minutes(5 - i);
                candle
            })
            .collect();

        let result = pipeline.process_candle_batch(candles.clone()).await;
        assert!(result.is_ok());

        // Check cache
        let cached = pipeline.cache().get_candles(&symbol, 10);
        assert_eq!(cached.len(), 5);

        // Give database time to write
        sleep(TokioDuration::from_millis(100)).await;

        // Check database
        let count = pipeline
            .storage()
            .get_candle_count(&symbol, LIVE_FEED_ADJUSTMENT)
            .await
            .unwrap();
        assert_eq!(count, 5);
    }

    #[tokio::test]
    async fn test_anomaly_detection() {
        let pipeline = create_test_pipeline().await;
        let symbol = Symbol::new("AAPL").unwrap();

        // First candle
        let candle1 = create_test_candle(symbol.clone(), 100);
        pipeline.process_candle(candle1).await.unwrap();

        // Second candle with huge price jump (should trigger anomaly)
        let candle2 = create_test_candle(symbol.clone(), 200);
        let result = pipeline.process_candle(candle2).await;

        // Should still succeed (anomalies are warnings, not errors)
        assert!(result.is_ok());

        // Both candles should be cached
        let cached = pipeline.cache().get_candles(&symbol, 10);
        assert_eq!(cached.len(), 2);
    }
}
