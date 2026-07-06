//! Market data management
//!
//! This module handles real-time and historical market data including
//! validation, caching, storage, and ingestion pipelines.
//!
//! # Architecture
//!
//! The data module follows a functional pipeline architecture:
//!
//! ```text
//! WebSocket → Validation → Cache → Storage → Events
//!                ↓           ↓        ↓         ↓
//!            Pure Fns    DashMap   SQLite   EventBus
//! ```
//!
//! # Components
//!
//! - **Validation**: Pure functions for data validation and anomaly detection
//! - **Cache**: Thread-safe in-memory cache using DashMap
//! - **Storage**: Async database operations using SQLx
//! - **Ingestion**: Pipeline coordinator that ties everything together
//!
//! # Examples
//!
//! ```no_run
//! use daytrader::data::{DataIngestionPipeline, MarketDataCache, CandleStorage};
//! use daytrader::events::EventBus;
//! use sqlx::SqlitePool;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Setup components
//! let pool = SqlitePool::connect("sqlite:data.db").await?;
//! let cache = Arc::new(MarketDataCache::new(100));
//! let storage = Arc::new(CandleStorage::new(pool));
//! let event_bus = EventBus::new(1000);
//!
//! // Create pipeline
//! let pipeline = DataIngestionPipeline::new(
//!     cache,
//!     storage,
//!     event_bus,
//! );
//!
//! // Process data
//! // pipeline.process_candle(candle).await?;
//! # Ok(())
//! # }
//! ```

pub mod cache;
pub mod ingestion;
pub mod storage;
pub mod validation;

// Re-export main types for convenience
pub use cache::MarketDataCache;
pub use ingestion::DataIngestionPipeline;
pub use storage::CandleStorage;
pub use validation::{
    Anomaly, detect_anomalies, detect_quote_anomalies, validate_candle, validate_quote,
};
