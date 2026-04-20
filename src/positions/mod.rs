// Position tracking and P&L management
//
// This module provides comprehensive position tracking with real-time P&L calculations,
// position lifecycle management, and portfolio-level analytics.

pub mod pnl;
pub mod tracker;

// Re-export key types for convenience
pub use pnl::{PerformanceError, PnLCalculator, PnLConfig, PortfolioPnL, TaxLotMethod};
pub use tracker::{CloseReason, Position, PositionError, PositionSide, PositionTracker};
