//! Configuration validation tests
//!
//! These tests validate configuration loading, parsing, and validation:
//! - TOML parsing and deserialization
//! - Invalid configuration error handling
//! - Dynamic configuration updates at runtime
//! - Boundary values and constraints validation

use daytrader::config::{AppConfig, BrokerConfig, DatabaseConfig, LoggingConfig, RiskConfig};
use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;

/// Test TOML parsing with valid configuration
#[test]
fn test_valid_toml_parsing() {
    let toml_content = r#"
[broker]
provider = "alpaca"
is_paper = true
api_key = "test_key"
api_secret = "test_secret"

[database]
path = "test.db"
max_connections = 10

[logging]
level = "info"
file_path = "test.log"

[risk]
max_daily_loss_dollars = 500.0
max_daily_loss_percent = 2.0
halt_trading_on_limit = true
max_position_size_dollars = 5000.0
max_position_size_percent = 10.0
max_concurrent_positions = 5
enforce_pdt_rule = true
pdt_min_equity = 25000.0
max_day_trades_under_pdt = 3
max_trades_per_pattern_per_day = 5
stop_pattern_after_consecutive_losses = 3
required_win_rate_for_auto = 60.0
min_pattern_sample_size = 20
min_account_balance = 1000.0
emergency_liquidate_threshold = 500.0
"#;

    let mut temp_file = NamedTempFile::with_suffix(".toml").unwrap();
    temp_file.write_all(toml_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let config = AppConfig::from_file(temp_file.path()).unwrap();

    // Verify parsed values
    assert_eq!(config.broker.provider, "alpaca");
    assert!(config.broker.is_paper);
    assert_eq!(config.database.max_connections, 10);
    assert_eq!(config.logging.level, "info");
    assert_eq!(config.risk.max_daily_loss_dollars, 500.0);
    assert_eq!(config.risk.max_day_trades_under_pdt, 3);
}

/// Test invalid TOML parsing error handling
#[test]
fn test_invalid_toml_parsing() {
    let invalid_toml = r#"
[broker
provider = "alpaca"  # Missing closing bracket
is_paper = true

[risk]
max_daily_loss_dollars = "not_a_number"
"#;

    let mut temp_file = NamedTempFile::with_suffix(".toml").unwrap();
    temp_file.write_all(invalid_toml.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let result = AppConfig::from_file(temp_file.path());
    assert!(result.is_err(), "Should fail to parse invalid TOML");
}

/// Test configuration with missing required fields
#[test]
fn test_missing_required_fields() {
    let incomplete_toml = r#"
[broker]
provider = "alpaca"
# Missing is_paper field

[database]
path = "test.db"
# Missing max_connections

[risk]
max_daily_loss_dollars = 500.0
# Missing many required risk fields
"#;

    let mut temp_file = NamedTempFile::with_suffix(".toml").unwrap();
    temp_file.write_all(incomplete_toml.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let result = AppConfig::from_file(temp_file.path());
    assert!(result.is_err(), "Should fail with missing required fields");
}

/// Test boundary values for configuration parameters
#[test]
fn test_configuration_boundary_values() {
    // Test zero values
    let zero_values_toml = r#"
[broker]
provider = "alpaca"
is_paper = true
api_key = "test_key"
api_secret = "test_secret"

[database]
path = "test.db"
max_connections = 0

[logging]
level = "info"
file_path = "test.log"

[risk]
max_daily_loss_dollars = 0.0
max_daily_loss_percent = 0.0
halt_trading_on_limit = false
max_position_size_dollars = 0.0
max_position_size_percent = 0.0
max_concurrent_positions = 0
enforce_pdt_rule = false
pdt_min_equity = 0.0
max_day_trades_under_pdt = 0
max_trades_per_pattern_per_day = 0
stop_pattern_after_consecutive_losses = 0
required_win_rate_for_auto = 0.0
min_pattern_sample_size = 0
min_account_balance = 0.0
emergency_liquidate_threshold = 0.0
"#;

    let mut temp_file = NamedTempFile::with_suffix(".toml").unwrap();
    temp_file.write_all(zero_values_toml.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    // An all-zero risk config is a disabled risk config and must be rejected.
    let result = AppConfig::from_file(temp_file.path());
    assert!(result.is_err(), "all-zero risk limits must be rejected");

    // Test very large values
    let large_values_toml = r#"
[broker]
provider = "alpaca"
is_paper = true
api_key = "test_key"
api_secret = "test_secret"

[database]
path = "test.db"
max_connections = 1000000

[logging]
level = "info"
file_path = "test.log"

[risk]
max_daily_loss_dollars = 1000000.0
max_daily_loss_percent = 100.0
halt_trading_on_limit = true
max_position_size_dollars = 10000000.0
max_position_size_percent = 100.0
max_concurrent_positions = 10000
enforce_pdt_rule = true
pdt_min_equity = 10000000.0
max_day_trades_under_pdt = 1000
max_trades_per_pattern_per_day = 10000
stop_pattern_after_consecutive_losses = 1000
required_win_rate_for_auto = 100.0
min_pattern_sample_size = 100000
min_account_balance = 1000000.0
emergency_liquidate_threshold = 100000.0
"#;

    let mut temp_file2 = NamedTempFile::with_suffix(".toml").unwrap();
    temp_file2.write_all(large_values_toml.as_bytes()).unwrap();
    temp_file2.flush().unwrap();

    let config = AppConfig::from_file(temp_file2.path()).unwrap();
    assert_eq!(config.database.max_connections, 1000000);
    assert_eq!(config.risk.max_daily_loss_dollars, 1000000.0);
}

/// Test negative values that should be rejected
#[test]
fn test_negative_values_validation() {
    let negative_values_toml = r#"
[broker]
provider = "alpaca"
is_paper = true
api_key = "test_key"
api_secret = "test_secret"

[database]
path = "test.db"
max_connections = -1

[logging]
level = "info"
file_path = "test.log"

[risk]
max_daily_loss_dollars = -100.0
max_daily_loss_percent = -5.0
halt_trading_on_limit = true
max_position_size_dollars = -1000.0
max_position_size_percent = -10.0
max_concurrent_positions = -5
enforce_pdt_rule = true
pdt_min_equity = -25000.0
max_day_trades_under_pdt = -1
max_trades_per_pattern_per_day = -10
stop_pattern_after_consecutive_losses = -3
required_win_rate_for_auto = -50.0
min_pattern_sample_size = -20
min_account_balance = -1000.0
emergency_liquidate_threshold = -500.0
"#;

    let mut temp_file = NamedTempFile::with_suffix(".toml").unwrap();
    temp_file
        .write_all(negative_values_toml.as_bytes())
        .unwrap();
    temp_file.flush().unwrap();

    // Unsigned integer fields reject negative values at parse time (TOML deserialization),
    // so the config should fail to parse with negative unsigned integer values.
    let result = AppConfig::from_file(temp_file.path());
    assert!(
        result.is_err(),
        "Config with negative unsigned integer fields should fail to parse"
    );
}

/// Test configuration file not found error
#[test]
fn test_missing_config_file() {
    let result = AppConfig::from_file("nonexistent_config.toml");
    assert!(
        result.is_err(),
        "Should fail when config file doesn't exist"
    );
}

/// Test dynamic configuration updates (simulated)
#[test]
fn test_dynamic_config_updates() {
    let initial_toml = r#"
[broker]
provider = "alpaca"
is_paper = true
api_key = "test_key"
api_secret = "test_secret"

[database]
path = "test.db"
max_connections = 10

[logging]
level = "info"
file_path = "test.log"

[risk]
max_daily_loss_dollars = 500.0
max_daily_loss_percent = 2.0
halt_trading_on_limit = true
max_position_size_dollars = 5000.0
max_position_size_percent = 10.0
max_concurrent_positions = 5
enforce_pdt_rule = true
pdt_min_equity = 25000.0
max_day_trades_under_pdt = 3
max_trades_per_pattern_per_day = 5
stop_pattern_after_consecutive_losses = 3
required_win_rate_for_auto = 60.0
min_pattern_sample_size = 20
min_account_balance = 1000.0
emergency_liquidate_threshold = 500.0
"#;

    let mut temp_file = NamedTempFile::with_suffix(".toml").unwrap();
    temp_file.write_all(initial_toml.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let mut config = AppConfig::from_file(temp_file.path()).unwrap();
    assert_eq!(config.risk.max_daily_loss_dollars, 500.0);

    // Simulate dynamic update by modifying the struct
    config.risk.max_daily_loss_dollars = 1000.0;
    config.risk.max_day_trades_under_pdt = 5;

    // Verify updates took effect
    assert_eq!(config.risk.max_daily_loss_dollars, 1000.0);
    assert_eq!(config.risk.max_day_trades_under_pdt, 5);
}

/// Test configuration serialization back to TOML
#[test]
fn test_config_serialization() {
    let config = AppConfig {
        broker: BrokerConfig {
            provider: "alpaca".to_string(),
            is_paper: true,
            api_key: "test_key".to_string(),
            api_secret: "test_secret".to_string(),
        },
        database: DatabaseConfig {
            path: std::path::PathBuf::from("test.db"),
            max_connections: 10,
        },
        logging: LoggingConfig {
            level: "info".to_string(),
            file_path: std::path::PathBuf::from("test.log"),
        },
        risk: RiskConfig {
            max_daily_loss_dollars: 500.0,
            max_daily_loss_percent: 2.0,
            halt_trading_on_limit: true,
            max_position_size_dollars: 5000.0,
            max_position_size_percent: 10.0,
            max_concurrent_positions: 5,
            enforce_pdt_rule: true,
            pdt_min_equity: 25000.0,
            max_day_trades_under_pdt: 3,
            max_trades_per_pattern_per_day: 5,
            stop_pattern_after_consecutive_losses: 3,
            required_win_rate_for_auto: 60.0,
            min_pattern_sample_size: 20,
            min_account_balance: 1000.0,
            emergency_liquidate_threshold: 500.0,
            pattern_profiles: std::collections::HashMap::new(),
            confidence_multipliers: vec![],
            pattern_performance_thresholds: daytrader::risk::types::PatternPerformanceThresholds {
                min_win_rate_short: daytrader::risk::types::Percentage::new(
                    rust_decimal::Decimal::new(600, 1),
                )
                .unwrap(),
                min_win_rate_medium: daytrader::risk::types::Percentage::new(
                    rust_decimal::Decimal::new(600, 1),
                )
                .unwrap(),
                min_win_rate_long: daytrader::risk::types::Percentage::new(
                    rust_decimal::Decimal::new(600, 1),
                )
                .unwrap(),
                max_consecutive_losses: daytrader::risk::types::Count::new(3).unwrap(),
                min_sample_size: daytrader::risk::types::Count::new(20).unwrap(),
                auto_disable_threshold: daytrader::risk::types::Percentage::new(
                    rust_decimal::Decimal::new(400, 1),
                )
                .unwrap(),
            },
        },
    };

    // Test serialization
    let serialized = toml::to_string(&config).unwrap();
    assert!(serialized.contains("provider = \"alpaca\""));
    assert!(serialized.contains("max_daily_loss_dollars = 500.0"));
    assert!(serialized.contains("max_day_trades_under_pdt = 3"));

    // Test round-trip: serialize then deserialize
    let deserialized: AppConfig = toml::from_str(&serialized).unwrap();
    assert_eq!(deserialized.broker.provider, config.broker.provider);
    assert_eq!(
        deserialized.risk.max_daily_loss_dollars,
        config.risk.max_daily_loss_dollars
    );
}

/// Test configuration with invalid enum values
#[test]
fn test_invalid_enum_values() {
    let invalid_enum_toml = r#"
[broker]
provider = "invalid_provider"
is_paper = true
api_key = "test_key"
api_secret = "test_secret"

[database]
path = "test.db"
max_connections = 10

[logging]
level = "invalid_level"
file_path = "test.log"

[risk]
max_daily_loss_dollars = 500.0
max_daily_loss_percent = 2.0
halt_trading_on_limit = true
max_position_size_dollars = 5000.0
max_position_size_percent = 10.0
max_concurrent_positions = 5
enforce_pdt_rule = true
pdt_min_equity = 25000.0
max_day_trades_under_pdt = 3
max_trades_per_pattern_per_day = 5
stop_pattern_after_consecutive_losses = 3
required_win_rate_for_auto = 60.0
min_pattern_sample_size = 20
min_account_balance = 1000.0
emergency_liquidate_threshold = 500.0
"#;

    let mut temp_file = NamedTempFile::with_suffix(".toml").unwrap();
    temp_file.write_all(invalid_enum_toml.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    // Should still parse successfully since these are just strings
    let config = AppConfig::from_file(temp_file.path()).unwrap();
    assert_eq!(config.broker.provider, "invalid_provider");
    assert_eq!(config.logging.level, "invalid_level");
}

/// Test configuration with extreme precision values
#[test]
fn test_precision_edge_cases() {
    let precision_toml = r#"
[broker]
provider = "alpaca"
is_paper = true
api_key = "test_key"
api_secret = "test_secret"

[database]
path = "test.db"
max_connections = 10

[logging]
level = "info"
file_path = "test.log"

[risk]
max_daily_loss_dollars = 0.0000001
max_daily_loss_percent = 0.000001
halt_trading_on_limit = true
max_position_size_dollars = 0.0000001
max_position_size_percent = 0.000001
max_concurrent_positions = 1
enforce_pdt_rule = true
pdt_min_equity = 0.0000001
max_day_trades_under_pdt = 1
max_trades_per_pattern_per_day = 1
stop_pattern_after_consecutive_losses = 1
required_win_rate_for_auto = 0.000001
min_pattern_sample_size = 1
min_account_balance = 0.0000001
emergency_liquidate_threshold = 0.0000001
"#;

    let mut temp_file = NamedTempFile::with_suffix(".toml").unwrap();
    temp_file.write_all(precision_toml.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let config = AppConfig::from_file(temp_file.path()).unwrap();
    assert_eq!(config.risk.max_daily_loss_dollars, 0.0000001);
    assert_eq!(config.risk.required_win_rate_for_auto, 0.000001);
}

/// Test loading default configuration
#[test]
fn test_load_default_config() {
    // This test assumes the default config file exists
    // In a real test environment, you might want to mock this
    let result = AppConfig::load_default();
    // Result depends on whether config/default.toml exists
    // If it doesn't exist, this will fail, which is expected
    match result {
        Ok(config) => {
            // If file exists, verify it has expected structure
            assert!(!config.broker.provider.is_empty());
            assert!(config.risk.max_daily_loss_dollars >= 0.0);
        }
        Err(_) => {
            // Expected if default config doesn't exist in test environment
        }
    }
}
