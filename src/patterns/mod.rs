//! Pattern recognition and signal generation
//!
//! This module provides a comprehensive pattern recognition system for detecting
//! candlestick patterns, calculating technical indicators, and generating trading signals.
//! The system is built using functional programming principles with pure functions,
//! immutable data structures, and composable pattern detection algorithms.
//!
//! # Architecture
//!
//! The pattern recognition system consists of several key components:
//!
//! - **Pattern Detectors**: Implement the `PatternDetector` trait for specific patterns
//! - **Pattern Registry**: Centralized management and execution of pattern detectors
//! - **Signal Generation**: Combines multiple patterns and indicators into trading signals
//! - **Performance Monitoring**: Tracks pattern effectiveness and detection performance
//!
//! # Design Principles
//!
//! - **Functional Programming**: Pure functions with no side effects
//! - **Type Safety**: Strong typing with newtype pattern for domain concepts
//! - **Performance**: Optimized for real-time processing with < 1ms detection times
//! - **Composability**: Patterns can be combined and filtered dynamically
//! - **Extensibility**: Easy to add new patterns and indicators
//!
//! # Examples
//!
//! ```
//! use daytrader::patterns::{PatternRegistry, PatternParams};
//! use daytrader::events::types::Candle;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let registry = PatternRegistry::new();
//!
//! // Register pattern detectors (would be done during initialization)
//! // registry.register("hammer", Box::new(HammerDetector::new()))?;
//!
//! // Detect patterns in candle data
//! let candles: Vec<Candle> = vec![]; // Your candle data
//! let results = registry.detect_all(&candles, None).await;
//!
//! for result in results {
//!     println!("Detected {} with confidence {:.2}",
//!              result.pattern_name.as_str(),
//!              result.confidence.value());
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Modules
//!
//! - [`types`] - Core types and data structures for pattern detection
//! - [`traits`] - Traits and interfaces for pattern detectors
//! - [`registry`] - Pattern registry for managing and executing detectors
//! - [`candlestick`] - Candlestick pattern detection algorithms (Phase 3A)
//! - [`indicators`] - Technical indicator calculations (Phase 3B)
//! - [`signals`] - Signal generation and composition (Phase 3C)

pub mod types;
pub mod traits;
pub mod utils;
pub mod registry;

// Re-export commonly used types and traits
pub use registry::{PatternRegistry, PatternRegistryBuilder, RegistryConfig};
pub use traits::{PatternDetector, PatternDetectorFactory, PatternCategory};
pub use types::{
    PatternResult, PatternContext, PatternParams, PatternStats, PatternError,
    VolumeConfirmation, TrendContext,
};

// Pattern detection modules
pub mod candlestick;  // Phase 3A-2: Basic candlestick patterns (Hammer, Shooting Star, Doji)
pub mod engulfing;    // Phase 3A-2: Engulfing patterns (Bullish/Bearish Engulfing)
pub mod multi_candle; // Phase 3A-3: Multi-candle patterns (Morning/Evening Star, Three White Soldiers/Black Crows)
pub mod indicators;   // Phase 3B: Technical indicators (SMA, EMA, RSI, MACD, Bollinger Bands, ATR)
pub mod signals;      // Phase 3C: Signal generation engine with market context analysis
pub mod streaming;   // Real-time signal generation streaming pipeline

// Re-export pattern detectors for easy access
pub use candlestick::{HammerDetector, ShootingStarDetector, DojiDetector};
pub use engulfing::{BullishEngulfingDetector, BearishEngulfingDetector};
pub use multi_candle::{MorningStarDetector, EveningStarDetector, ThreeWhiteSoldiersDetector, ThreeBlackCrowsDetector};

// Re-export signal generation components
pub use signals::{
    SignalEngine, SignalConfig, TradingSignal, MarketContext, SignalComponent,
    SignalComponentType, MarketContextAnalyzer, SignalComposer,
};
pub use streaming::{SignalStreamingPipeline, StreamingConfig, PipelineStats, StreamingError};







