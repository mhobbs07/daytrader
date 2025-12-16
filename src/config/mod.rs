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
    pub max_daily_loss_dollars: f64,
    pub max_position_size_dollars: f64,
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