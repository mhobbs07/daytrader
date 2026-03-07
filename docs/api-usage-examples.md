# Day Trading Platform API Usage Examples

## Overview

This document provides comprehensive examples of how to use the Day Trading Platform APIs. All examples follow the established coding standards and demonstrate real-world usage patterns.

## Core Type Usage

### Domain Types (Newtypes)

```rust
use daytrader::brokers::types::{Symbol, Price, Quantity, Confidence};
use rust_decimal::Decimal;

// Symbol validation and normalization
let symbol = Symbol::new("AAPL").unwrap();
assert_eq!(symbol.as_str(), "AAPL");

// Invalid symbols are rejected
assert!(Symbol::new("").is_err());
assert!(Symbol::new("TOOLONGSYMBOLNAME").is_err());

// Price validation (must be positive)
let price = Price::new(Decimal::new(15050, 2)).unwrap(); // $150.50
assert_eq!(price.value(), Decimal::new(15050, 2));

// Zero or negative prices are rejected
assert!(Price::new(Decimal::ZERO).is_err());
assert!(Price::new(Decimal::new(-100, 0)).is_err());

// Quantity validation (must be positive)
let quantity = Quantity::new(100).unwrap();
assert_eq!(quantity.value(), 100);

// Zero quantity is rejected
assert!(Quantity::new(0).is_err());

// Confidence score validation (0.0 to 1.0)
let confidence = Confidence::new(0.85).unwrap();
assert_eq!(confidence.value(), 0.85);
assert_eq!(confidence.as_percentage(), 85.0);

// Out of range confidence is rejected
assert!(Confidence::new(-0.1).is_err());
assert!(Confidence::new(1.5).is_err());
```

### Pattern Detection

```rust
use daytrader::patterns::traits::PatternDetector;
use daytrader::patterns::candlestick::HammerDetector;
use daytrader::patterns::types::{PatternParams, PatternResult};
use daytrader::events::types::Candle;
use daytrader::brokers::types::{Symbol, Price};
use rust_decimal::Decimal;
use chrono::Utc;

// Create a hammer pattern detector
let detector = HammerDetector::new();

// Create sample candle data showing a hammer pattern
let candles = vec![
    Candle {
        symbol: Symbol::new("AAPL").unwrap(),
        timestamp: Utc::now(),
        open: Price::new(Decimal::new(10000, 2)).unwrap(),
        high: Price::new(Decimal::new(10200, 2)).unwrap(),
        low: Price::new(Decimal::new(9500, 2)).unwrap(),  // Long lower wick
        close: Price::new(Decimal::new(10100, 2)).unwrap(),
        volume: 1000000,
    }
];

// Configure detection parameters
let params = PatternParams::new()
    .with_min_confidence(0.5)
    .require_volume_confirmation(true);

// Detect patterns
match detector.detect(&candles, &params) {
    Ok(Some(result)) => {
        println!("Pattern detected: {}", result.pattern_name.as_str());
        println!("Confidence: {:.2}", result.confidence.value());
        println!("Signal: {:?}", result.signal_type);
    }
    Ok(None) => println!("No pattern detected"),
    Err(e) => println!("Detection error: {}", e),
}
```

### Pattern Registry

```rust
use daytrader::patterns::{PatternRegistry, PatternRegistryBuilder};
use daytrader::patterns::traits::PatternCategory;
use daytrader::patterns::types::PatternParams;

// Create registry with custom configuration
let registry = PatternRegistryBuilder::new()
    .max_parallel_patterns(10)
    .global_min_confidence(0.6)
    .enabled_categories(vec![PatternCategory::Candlestick])
    .build()
    .await
    .unwrap();

// Register pattern detectors
// registry.register("hammer", Box::new(HammerDetector::new())).unwrap();
// registry.register("doji", Box::new(DojiDetector::new())).unwrap();

// Get market data
let candles: Vec<Candle> = get_market_data();
let params = PatternParams::default();

// Detect all patterns
let results = registry.detect_all(&candles, Some(&params)).await;

println!("Detected {} patterns:", results.len());
for result in results {
    println!("  - {} (confidence: {:.2})",
             result.pattern_name.as_str(),
             result.confidence.value());
}

// Detect patterns by category
let candlestick_patterns = registry
    .detect_by_category(&candles, PatternCategory::Candlestick, Some(&params))
    .await;

println!("Candlestick patterns: {}", candlestick_patterns.len());
```

### Risk Management

```rust
use daytrader::risk::types::{AccountId, DollarAmount, Percentage, Count};
use daytrader::config::RiskConfig;
use rust_decimal::Decimal;

// Create risk configuration
let risk_config = RiskConfig {
    max_daily_loss_dollars: DollarAmount::new(Decimal::new(50000, 2)).unwrap(), // $500
    max_daily_loss_percent: Percentage::new(Decimal::new(200, 2)).unwrap(),     // 2%
    max_position_size_dollars: DollarAmount::new(Decimal::new(500000, 2)).unwrap(), // $5,000
    max_position_size_percent: Percentage::new(Decimal::new(1000, 2)).unwrap(), // 10%
    max_concurrent_positions: Count::new(5).unwrap(),
    enforce_pdt_rule: true,
    pdt_min_equity: DollarAmount::new(Decimal::new(2500000, 2)).unwrap(), // $25,000
    max_day_trades_under_pdt: Count::new(3).unwrap(),
    // ... other configuration
};

// Create risk engine
let account_id = AccountId::new("account-123");
let initial_equity = DollarAmount::new(Decimal::new(5000000, 2)).unwrap(); // $50,000

let risk_engine = RiskEngine::new(risk_config, event_bus, account_id, initial_equity.value());
risk_engine.start().await.unwrap();

// Monitor risk state
let risk_state = risk_engine.get_risk_state().await;
println!("Daily P&L: ${}", risk_state.daily_pl);
println!("Day trades: {}", risk_state.day_trade_count);
println!("Trading halted: {}", risk_state.is_trading_halted);
```

### Event-Driven Architecture

```rust
use daytrader::events::bus::EventBus;
use daytrader::events::types::{MarketEvent, PatternSignal, Order, Fill};
use daytrader::brokers::types::{Symbol, SignalType, Price, Quantity};
use tokio::sync::mpsc;

// Create event bus
let event_bus = Arc::new(EventBus::new(1000));

// Subscribe to events
let mut event_receiver = event_bus.subscribe();

// Spawn event processing task
tokio::spawn(async move {
    while let Ok(event) = event_receiver.recv().await {
        match event {
            MarketEvent::PatternDetected { signal } => {
                println!("Pattern detected: {} ({})",
                        signal.pattern_name.as_str(),
                        signal.confidence.as_percentage());
            }
            MarketEvent::OrderFilled { order_id, fill } => {
                println!("Order {} filled: {} @ ${}",
                        order_id.as_str(),
                        fill.filled_quantity.value(),
                        fill.filled_price.value());
            }
            MarketEvent::RiskViolation { violation } => {
                println!("Risk violation: {}", violation.message);
            }
            _ => {}
        }
    }
});

// Publish pattern detection event
let pattern_signal = PatternSignal {
    symbol: Symbol::new("AAPL").unwrap(),
    pattern_name: PatternName::new("hammer"),
    signal_type: SignalType::Buy,
    confidence: Confidence::new(0.85).unwrap(),
    detected_at: Utc::now(),
    price: Price::new(Decimal::new(15000, 2)).unwrap(),
};

event_bus.publish(MarketEvent::PatternDetected {
    signal: pattern_signal
});
```

## Advanced Usage Patterns

### Custom Pattern Implementation

```rust
use daytrader::patterns::traits::PatternDetector;
use daytrader::patterns::types::{PatternResult, PatternParams, PatternError};

pub struct CustomPatternDetector {
    name: &'static str,
    min_candles: usize,
}

impl CustomPatternDetector {
    pub fn new() -> Self {
        Self {
            name: "custom_pattern",
            min_candles: 5, // Requires 5 candles for analysis
        }
    }
}

impl PatternDetector for CustomPatternDetector {
    fn name(&self) -> &'static str {
        self.name
    }

    fn min_candles_required(&self) -> usize {
        self.min_candles
    }

    fn detect(
        &self,
        candles: &[Candle],
        params: &PatternParams,
    ) -> Result<Option<PatternResult>, PatternError> {
        if candles.len() < self.min_candles {
            return Err(PatternError::InsufficientData {
                required: self.min_candles,
                available: candles.len(),
            });
        }

        // Implement custom pattern logic
        let is_pattern = self.analyze_custom_pattern(candles)?;

        if !is_pattern {
            return Ok(None);
        }

        let confidence = self.calculate_confidence(candles)?;
        if confidence.value() < params.min_confidence {
            return Ok(None);
        }

        let result = PatternResult::new(
            PatternName::new(self.name),
            SignalType::Buy, // or Sell based on analysis
            confidence,
            candles[0].close, // Use most recent close
        );

        Ok(Some(result))
    }
}

// Register custom pattern
let registry = PatternRegistry::new();
registry.register("custom", Box::new(CustomPatternDetector::new()))?;
```

### Configuration Management

```rust
use daytrader::config::AppConfig;
use std::env;

// Load configuration from file
let config = AppConfig::from_file("config/default.toml")?;

// Override with environment variables
if let Ok(api_key) = env::var("ALPACA_API_KEY") {
    config.broker.api_key = api_key;
}

if let Ok(api_secret) = env::var("ALPACA_API_SECRET") {
    config.broker.api_secret = api_secret;
}

// Validate configuration
config.validate()?;

// Use configuration
let broker = create_broker(&config.broker)?;
let risk_engine = create_risk_engine(&config.risk)?;
```

### Error Handling Patterns

```rust
use daytrader::patterns::types::PatternError;
use daytrader::brokers::types::BrokerError;

// Comprehensive error handling
async fn process_trading_signal(signal: PatternSignal) -> Result<(), anyhow::Error> {
    // Validate signal
    if signal.confidence.value() < 0.7 {
        return Err(anyhow::anyhow!("Signal confidence too low: {:.2}",
                                  signal.confidence.value()));
    }

    // Create order
    let order = create_order_from_signal(&signal)
        .map_err(|e| anyhow::anyhow!("Failed to create order: {}", e))?;

    // Validate with risk engine
    risk_engine.validate_order(&order).await
        .map_err(|e| anyhow::anyhow!("Risk validation failed: {}", e))?;

    // Submit order
    let order_id = broker.place_order(order).await
        .map_err(|e| match e {
            BrokerError::InsufficientFunds => anyhow::anyhow!("Insufficient funds for order"),
            BrokerError::RateLimitExceeded => anyhow::anyhow!("Rate limit exceeded, retry later"),
            _ => anyhow::anyhow!("Order placement failed: {}", e),
        })?;

    println!("Order placed successfully: {}", order_id);
    Ok(())
}
```

### Performance Optimization Examples

```rust
use daytrader::patterns::registry::PatternRegistry;
use std::time::Instant;

// Parallel pattern detection for multiple symbols
async fn analyze_multiple_symbols(
    registry: &PatternRegistry,
    symbol_data: &HashMap<String, Vec<Candle>>,
) -> Vec<PatternResult> {
    let mut handles = vec![];

    for (symbol, candles) in symbol_data {
        let registry = registry.clone();
        let candles = candles.clone();
        let symbol = symbol.clone();

        let handle = tokio::spawn(async move {
            let start = Instant::now();
            let results = registry.detect_all(&candles, None).await;
            let duration = start.elapsed();

            println!("Analyzed {}: {} patterns in {:?}", symbol, results.len(), duration);
            results
        });

        handles.push(handle);
    }

    // Collect results
    let mut all_results = vec![];
    for handle in handles {
        all_results.extend(handle.await.unwrap());
    }

    // Sort by confidence
    all_results.sort_by(|a, b| b.confidence.value().partial_cmp(&a.confidence.value()).unwrap());
    all_results.truncate(50); // Keep top 50 signals

    all_results
}
```

### Testing Examples

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn test_pattern_detection_success() {
        let detector = HammerDetector::new();
        let candles = create_test_hammer_candles();
        let params = PatternParams::default();

        let result = detector.detect(&candles, &params);

        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[rstest]
    #[case(0, 1)]  // Empty candles
    #[case(1, 1)]  // Insufficient candles
    fn test_insufficient_data(#[case] candle_count: usize, #[case] expected_available: usize) {
        let detector = HammerDetector::new();
        let candles = create_test_candles(candle_count);
        let params = PatternParams::default();

        let result = detector.detect(&candles, &params);

        assert!(matches!(
            result,
            Err(PatternError::InsufficientData { available, .. }) if available == expected_available
        ));
    }

    #[tokio::test]
    async fn test_registry_integration() {
        let registry = PatternRegistry::new();
        registry.register("hammer", Box::new(HammerDetector::new())).unwrap();

        let candles = create_test_candles(10);
        let results = registry.detect_all(&candles, None).await;

        assert!(!results.is_empty());
        assert!(results.iter().all(|r| r.confidence.value() >= 0.0));
    }
}
```

## Integration Examples

### Complete Trading Workflow

```rust
use daytrader::{
    brokers::{AlpacaBroker, BrokerConfig},
    patterns::{PatternRegistry, PatternRegistryBuilder},
    risk::{RiskEngine, RiskConfig},
    events::bus::EventBus,
};

async fn run_trading_system() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize components
    let event_bus = Arc::new(EventBus::new(1000));

    // Set up broker
    let broker_config = BrokerConfig {
        provider: BrokerProvider::Alpaca,
        api_key: env::var("ALPACA_API_KEY")?,
        api_secret: env::var("ALPACA_API_SECRET")?,
        is_paper: true,
    };
    let broker = AlpacaBroker::new(broker_config)?;

    // Set up pattern recognition
    let pattern_registry = PatternRegistryBuilder::new()
        .max_parallel_patterns(20)
        .global_min_confidence(0.7)
        .build()
        .await?;

    // Register patterns
    pattern_registry.register("hammer", Box::new(HammerDetector::new()))?;
    pattern_registry.register("doji", Box::new(DojiDetector::new()))?;
    pattern_registry.register("engulfing", Box::new(BullishEngulfingDetector::new()))?;

    // Set up risk management
    let risk_config = RiskConfig::default();
    let account_id = AccountId::new("paper-account");
    let initial_equity = broker.get_account_balance().await?;
    let risk_engine = RiskEngine::new(risk_config, event_bus.clone(), account_id, initial_equity);

    // Start components
    risk_engine.start().await?;
    broker.start_market_data_stream().await?;

    // Main trading loop
    let mut market_data = broker.subscribe_market_data().await?;

    while let Some(candles) = market_data.recv().await {
        // Detect patterns
        let patterns = pattern_registry.detect_all(&candles, None).await;

        // Process signals
        for pattern in patterns.into_iter().filter(|p| p.confidence.value() >= 0.8) {
            if let Ok(order) = create_order_from_pattern(&pattern) {
                if risk_engine.validate_order(&order).await.is_ok() {
                    let _order_id = broker.place_order(order).await?;
                }
            }
        }
    }

    Ok(())
}
```

This comprehensive set of examples demonstrates the proper usage of all major APIs in the Day Trading Platform, following established patterns and best practices.