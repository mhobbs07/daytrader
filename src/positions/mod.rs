// Position tracking and P&L management
//
// This module provides comprehensive position tracking with real-time P&L calculations,
// position lifecycle management, and portfolio-level analytics.

pub mod tracker;
pub mod pnl;

// Re-export key types for convenience
pub use tracker::{Position, PositionSide, PositionTracker, CloseReason, PositionError};
pub use pnl::{PnLCalculator, PnLConfig, TaxLotMethod, PortfolioPnL, PerformanceError};
