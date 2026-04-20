//! Alpaca broker configuration

use serde::{Deserialize, Serialize};

/// Alpaca broker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlpacaConfig {
    /// API key (read from environment or keychain)
    pub api_key: String,
    /// API secret (read from environment or keychain)
    pub api_secret: String,
    /// Whether to use paper trading
    pub is_paper: bool,
    /// Base URL for REST API
    pub base_url: String,
    /// WebSocket URL for market data
    pub ws_url: String,
}

impl AlpacaConfig {
    /// Creates configuration for paper trading
    pub fn paper() -> Self {
        Self {
            api_key: String::new(),
            api_secret: String::new(),
            is_paper: true,
            base_url: "https://paper-api.alpaca.markets".to_string(),
            ws_url: "wss://stream.data.alpaca.markets/v2/iex".to_string(),
        }
    }

    /// Creates configuration for live trading
    pub fn live() -> Self {
        Self {
            api_key: String::new(),
            api_secret: String::new(),
            is_paper: false,
            base_url: "https://api.alpaca.markets".to_string(),
            ws_url: "wss://stream.data.alpaca.markets/v2/iex".to_string(),
        }
    }

    /// Sets credentials for the configuration
    pub fn with_credentials(mut self, api_key: String, api_secret: String) -> Self {
        self.api_key = api_key;
        self.api_secret = api_secret;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paper_config() {
        let config = AlpacaConfig::paper();
        assert!(config.is_paper);
        assert!(config.base_url.contains("paper-api"));
    }

    #[test]
    fn test_live_config() {
        let config = AlpacaConfig::live();
        assert!(!config.is_paper);
        assert!(!config.base_url.contains("paper"));
    }

    #[test]
    fn test_with_credentials() {
        let config = AlpacaConfig::paper()
            .with_credentials("test_key".to_string(), "test_secret".to_string());
        assert_eq!(config.api_key, "test_key");
        assert_eq!(config.api_secret, "test_secret");
    }
}
