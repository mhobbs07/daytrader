//! Error types for the application

use thiserror::Error;
use rust_decimal::Decimal;

/// Application result type
pub type Result<T> = std::result::Result<T, AppError>;

/// Top-level application error
#[derive(Debug, Error)]
pub enum AppError {
    /// Broker error
    #[error("Broker error: {0}")]
    Broker(#[from] BrokerError),
    
    /// Risk violation
    #[error("Risk violation: {0}")]
    Risk(#[from] RiskError),
    
    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
    
    /// Pattern detection error
    #[error("Pattern detection error: {0}")]
    Pattern(#[from] PatternError),
    
    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),
    
    /// Network error
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    /// WebSocket error
    #[error("WebSocket error: {0}")]
    WebSocket(String),
    
    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    /// Other error
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

/// Broker-specific errors
#[derive(Debug, Error)]
pub enum BrokerError {
    /// Connection failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    /// Order rejected
    #[error("Order rejected: {reason}")]
    OrderRejected {
        /// Rejection reason
        reason: String
    },
    
    /// Insufficient funds
    #[error("Insufficient funds: required {required}, available {available}")]
    InsufficientFunds {
        /// Required amount
        required: Decimal,
        /// Available amount
        available: Decimal
    },
    
    /// Symbol not found
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),
    
    /// Rate limit exceeded
    #[error("Rate limit exceeded, retry after {retry_after:?}")]
    RateLimitExceeded {
        /// Duration to wait before retrying
        retry_after: std::time::Duration
    },
    
    /// API error
    #[error("API error: {0}")]
    ApiError(String),
}

/// Risk management errors
#[derive(Debug, Error)]
pub enum RiskError {
    /// Daily loss limit exceeded
    #[error("Daily loss limit exceeded: current loss ${current_loss}, limit ${limit}")]
    DailyLossLimitExceeded {
        /// Current loss amount
        current_loss: Decimal,
        /// Loss limit
        limit: Decimal
    },
    
    /// Position size too large
    #[error("Position size too large: ${order_size} exceeds limit of ${limit}")]
    PositionSizeTooLarge {
        /// Order size
        order_size: Decimal,
        /// Size limit
        limit: Decimal
    },
    
    /// PDT limit reached
    #[error("PDT limit reached: {current_count} day trades, account value ${account_value} (need ${required_value})")]
    PdtLimitReached {
        /// Current day trade count
        current_count: u32,
        /// Current account value
        account_value: Decimal,
        /// Required account value
        required_value: Decimal,
    },
    
    /// Pattern disabled
    #[error("Pattern disabled: {pattern}")]
    PatternDisabled {
        /// Pattern name
        pattern: String
    },
    
    /// Account below minimum balance
    #[error("Account below minimum balance: ${current} (minimum: ${minimum})")]
    BelowMinimumBalance {
        /// Current balance
        current: Decimal,
        /// Minimum balance
        minimum: Decimal
    },
}

/// Database errors
#[derive(Debug, Error)]
pub enum DatabaseError {
    /// Connection failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    /// Query failed
    #[error("Query failed: {0}")]
    QueryFailed(String),
    
    /// Transaction failed
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),
    
    /// Constraint violation
    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),
    
    /// SQLite error
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

/// Pattern detection errors
#[derive(Debug, Error)]
pub enum PatternError {
    /// Insufficient data
    #[error("Insufficient data: need {required} candles, have {available}")]
    InsufficientData {
        /// Required candles
        required: usize,
        /// Available candles
        available: usize
    },
    
    /// Invalid candle data
    #[error("Invalid candle data: {0}")]
    InvalidCandleData(String),
    
    /// Calculation error
    #[error("Calculation error: {0}")]
    CalculationError(String),
}