//! Configuration management for the trading platform
//!
//! This module handles application configuration including:
//! - Broker settings (API keys, endpoints)
//! - Risk management parameters
//! - Pattern detection settings
//! - Database configuration
//! - Logging configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Broker configuration
    pub broker: BrokerConfig,
    
    /// Database configuration
    pub database: DatabaseConfig,
    
    /// Logging configuration
    pub logging: LoggingConfig,
    
    /// Risk management configuration
    pub risk: RiskConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerConfig {
    pub provider: String,
    pub is_paper: bool,
    pub api_key: String,
    pub api_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: PathBuf,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    // Daily loss limits
    pub max_daily_loss_dollars: f64,
    pub max_daily_loss_percent: f64,
    pub halt_trading_on_limit: bool,

    // Position sizing
    pub max_position_size_dollars: f64,
    pub max_position_size_percent: f64,
    pub max_concurrent_positions: u32,

    // PDT protection
    pub enforce_pdt_rule: bool,
    pub pdt_min_equity: f64,
    pub max_day_trades_under_pdt: u32,

    // Pattern-specific rules
    pub max_trades_per_pattern_per_day: u32,
    pub stop_pattern_after_consecutive_losses: u32,
    pub required_win_rate_for_auto: f64,
    pub min_pattern_sample_size: u32,

    // Account safeguards
    pub min_account_balance: f64,
    pub emergency_liquidate_threshold: f64,
}

impl AppConfig {
    /// Load configuration from file
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self, config::ConfigError> {
        let settings = config::Config::builder()
            .add_source(config::File::from(path.as_ref()))
            .build()?;
        
        settings.try_deserialize()
    }
    
    /// Load configuration from default location
    pub fn load_default() -> Result<Self, config::ConfigError> {
        Self::from_file("config/default.toml")
    }
}