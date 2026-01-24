# Day Trading Platform Examples Guide

This guide provides comprehensive examples and usage patterns for the Day Trading Platform, covering common workflows and integration scenarios.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Pattern Recognition Examples](#pattern-recognition-examples)
3. [Risk Management Examples](#risk-management-examples)
4. [Broker Integration Examples](#broker-integration-examples)
5. [Event-Driven Architecture](#event-driven-architecture)
6. [Database Operations](#database-operations)
7. [Performance Optimization](#performance-optimization)
8. [Testing Patterns](#testing-patterns)

## Getting Started

### Basic Setup

```rust
use daytrader::{AppConfig, AppError};
use std::env;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Load configuration
    let config = AppConfig::from_file("config/default.toml")
        .unwrap_or_else(|_| AppConfig::default());

    // Override with environment variables
    if let Ok(api_key) = env::var("ALPACA_API_KEY") {
        config.broker.api_key = api_key;
    }

    // Validate configuration
    config.validate()?;

    println!("✅ Configuration loaded successfully");
    Ok(())
}
```

### Environment Setup

```bash
# Set required environment variables
export ALPACA_API_KEY="your_paper_api_key"
export ALPACA_API_SECRET="your_paper_api_secret"
export RUST_LOG="daytrader=info"

# Run with configuration
cargo run -- --config config/default.toml
```

## Pattern Recognition Examples

### Single Pattern Detection

```rust
use daytrader::patterns::candlestick::HammerDetector;
use daytrader::patterns::types::{PatternParams, PatternResult};
use daytrader::events::types::Candle;
use daytrader::brokers::types::{Symbol, Price};
use rust_decimal::Decimal;
use chrono::Utc;

async fn detect_hammer_pattern() -> Result<(), Box<dyn std::error::Error>> {
    let detector = HammerDetector::new();

    // Create sample candle data
    let candles = vec![
        Candle {
            symbol: Symbol::new("AAPL")?,
            timestamp: Utc::now(),
            open: Price::new(Decimal::new(10000, 2))?,  // $100.00
            high: Price::new(Decimal::new(10200, 2))?,  // $102.00
            low: Price::new(Decimal::new(9500, 2))?,    // $95.00 (long lower wick)
            close: Price::new(Decimal::new(10100, 2))?, // $101.00
            volume: 1000000,
        }
    ];

    // Configure detection parameters
    let params = PatternParams::new()
        .with_min_confidence(0.7)
        .require_volume_confirmation(true);

    // Detect pattern
    match detector.detect(&candles, &params) {
        Ok(Some(result)) => {
            println!("Hammer pattern detected!");
            println!("Confidence: {:.1}%", result.confidence.as_percentage());
            println!("Signal: {:?}", result.signal_type);
        }
        Ok(None) => println!("No hammer pattern detected"),
        Err(e) => println!("Detection error: {}", e),
    }

    Ok(())
}
```

### Multi-Pattern Analysis

```rust
use daytrader::patterns::{PatternRegistry, PatternRegistryBuilder};
use daytrader::patterns::traits::PatternCategory;

async fn analyze_multiple_patterns() -> Result<(), Box<dyn std::error::Error>> {
    // Create pattern registry
    let registry = PatternRegistryBuilder::new()
        .max_parallel_patterns(10)
        .global_min_confidence(0.6)
        .enabled_categories(vec![PatternCategory::Candlestick])
        .build()
        .await?;

    // Register patterns
    registry.register("hammer", Box::new(HammerDetector::new()))?;
    registry.register("doji", Box::new(DojiDetector::new()))?;
    registry.register("engulfing", Box::new(BullishEngulfingDetector::new()))?;

    // Get market data
    let candles = get_market_data().await?;

    // Detect all patterns
    let results = registry.detect_all(&candles, None).await;

    // Group by symbol and sort by confidence
    let mut by_symbol = std::collections::HashMap::new();
    for result in results {
        by_symbol.entry(result.symbol.clone())
            .or_insert_with(Vec::new)
            .push(result);
    }

    // Display results
    for (symbol, patterns) in by_symbol {
        println!("📊 {}: {} patterns detected", symbol.as_str(), patterns.len());

        // Sort by confidence (highest first)
        let mut sorted = patterns;
        sorted.sort_by(|a, b| b.confidence.value().partial_cmp(&a.confidence.value()).unwrap());

        for pattern in sorted.iter().take(3) { // Top 3 patterns
            println!("  - {} ({:.1}%)",
                    pattern.pattern_name.as_str(),
                    pattern.confidence.as_percentage());
        }
    }

    Ok(())
}
```

### Real-Time Pattern Monitoring

```rust
use daytrader::events::bus::EventBus;
use daytrader::events::types::MarketEvent;
use tokio::sync::mpsc;

async fn monitor_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let event_bus = Arc::new(EventBus::new(1000));
    let mut event_receiver = event_bus.subscribe();

    // Pattern detection task
    let event_bus_clone = event_bus.clone();
    tokio::spawn(async move {
        let registry = PatternRegistry::new().await;
        let mut market_data_stream = get_market_data_stream().await;

        while let Some(candles) = market_data_stream.recv().await {
            let patterns = registry.detect_all(&candles, None).await;

            for pattern in patterns {
                if pattern.confidence.value() >= 0.8 {
                    event_bus_clone.publish(MarketEvent::PatternDetected {
                        signal: pattern.into()
                    });
                }
            }
        }
    });

    // Event processing
    while let Ok(event) = event_receiver.recv().await {
        match event {
            MarketEvent::PatternDetected { signal } => {
                println!("🚨 High-confidence pattern: {} on {} ({:.1}%)",
                        signal.pattern_name.as_str(),
                        signal.symbol.as_str(),
                        signal.confidence.as_percentage());

                // Trigger trading logic
                handle_trading_signal(signal).await?;
            }
            _ => {}
        }
    }

    Ok(())
}
```

## Risk Management Examples

### Basic Risk Configuration

```rust
use daytrader::risk::types::{RiskConfig, DollarAmount, Percentage, Count};
use rust_decimal::Decimal;

fn create_conservative_risk_config() -> RiskConfig {
    RiskConfig {
        max_daily_loss_dollars: DollarAmount::new(Decimal::new(25000, 2)).unwrap(), // $250
        max_daily_loss_percent: Percentage::new(Decimal::new(100, 2)).unwrap(),     // 1%
        max_position_size_dollars: DollarAmount::new(Decimal::new(250000, 2)).unwrap(), // $2,500
        max_position_size_percent: Percentage::new(Decimal::new(500, 2)).unwrap(),  // 5%
        max_concurrent_positions: Count::new(3).unwrap(),
        enforce_pdt_rule: true,
        pdt_min_equity: DollarAmount::new(Decimal::new(2500000, 2)).unwrap(), // $25,000
        max_day_trades_under_pdt: Count::new(3).unwrap(),
    }
}

fn create_aggressive_risk_config() -> RiskConfig {
    RiskConfig {
        max_daily_loss_dollars: DollarAmount::new(Decimal::new(100000, 2)).unwrap(), // $1,000
        max_daily_loss_percent: Percentage::new(Decimal::new(200, 2)).unwrap(),      // 2%
        max_position_size_dollars: DollarAmount::new(Decimal::new(1000000, 2)).unwrap(), // $10,000
        max_position_size_percent: Percentage::new(Decimal::new(1000, 2)).unwrap(),  // 10%
        max_concurrent_positions: Count::new(10).unwrap(),
        enforce_pdt_rule: true,
        pdt_min_equity: DollarAmount::new(Decimal::new(2500000, 2)).unwrap(),
        max_day_trades_under_pdt: Count::new(3).unwrap(),
    }
}
```

### Order Validation with Risk Management

```rust
use daytrader::risk::RiskEngine;
use daytrader::brokers::types::{Order, OrderSide, Symbol, Quantity, Price};

async fn validate_and_place_order(
    risk_engine: &RiskEngine,
    broker: &dyn Broker,
    symbol: Symbol,
    side: OrderSide,
    quantity: Quantity,
    price: Price,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create order
    let order = Order::limit(symbol, side, quantity, price);

    // Validate against risk rules
    risk_engine.validate_order(&order).await
        .map_err(|e| anyhow::anyhow!("Risk validation failed: {}", e))?;

    // Check account balance
    let account = broker.get_account_balance().await?;
    let order_value = price.value() * Decimal::from(quantity.value());

    if account.cash < order_value && matches!(side, OrderSide::Buy) {
        return Err(anyhow::anyhow!("Insufficient cash: have ${:.2}, need ${:.2}",
                                  account.cash, order_value));
    }

    // Place order
    let order_id = broker.place_order(order).await?;
    println!("✅ Order placed: {}", order_id);

    Ok(())
}
```

### PDT Rule Monitoring

```rust
use daytrader::risk::pdt::PDTMonitor;
use daytrader::risk::types::PDTConfig;

async fn monitor_pdt_compliance() -> Result<(), Box<dyn std::error::Error>> {
    let pdt_config = PDTConfig {
        min_equity: DollarAmount::new(Decimal::new(2500000, 2))?, // $25,000
        max_day_trades: 3,
        reset_time: NaiveTime::from_hms(16, 0, 0), // 4:00 PM ET
    };

    let mut pdt_monitor = PDTMonitor::new(pdt_config);

    // Simulate trading day
    for trade_num in 1..=5 {
        if pdt_monitor.can_place_order()? {
            println!("Trade {}: Allowed", trade_num);
            pdt_monitor.record_day_trade()?;

            // Check if we've hit PDT limits
            if pdt_monitor.is_pdt_violation() {
                println!("⚠️  PDT violation detected! Trading restricted.");
                break;
            }
        } else {
            println!("Trade {}: Blocked by PDT rules", trade_num);
            break;
        }
    }

    println!("Day trades today: {}", pdt_monitor.day_trade_count());
    println!("Equity: ${}", pdt_monitor.current_equity());

    Ok(())
}
```

## Broker Integration Examples

### Alpaca Paper Trading

```rust
use daytrader::brokers::alpaca::{AlpacaBroker, AlpacaConfig};

async fn setup_paper_trading() -> Result<(), Box<dyn std::error::Error>> {
    // Configuration for paper trading
    let config = AlpacaConfig::paper()
        .with_credentials(
            env::var("ALPACA_API_KEY")?,
            env::var("ALPACA_API_SECRET")?
        )
        .with_timeout(Duration::from_secs(30));

    let broker = AlpacaBroker::new(config)?;

    // Get account info
    let account = broker.get_account().await?;
    println!("Paper Account: {}", account.account_number);
    println!("Buying Power: ${}", account.buying_power);

    // Place a small test order
    let symbol = Symbol::new("SPY")?;
    let quantity = Quantity::new(1)?;
    let order = Order::market(symbol, OrderSide::Buy, quantity);

    match broker.place_order(order).await {
        Ok(order_id) => {
            println!("Test order placed: {}", order_id);

            // Monitor order status
            for _ in 0..10 {
                let status = broker.get_order_status(&order_id).await?;
                println!("Order status: {:?}", status);

                if matches!(status, OrderStatus::Filled | OrderStatus::Rejected) {
                    break;
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            }

            // Cancel if still open
            if let Ok(OrderStatus::Accepted) = broker.get_order_status(&order_id).await {
                broker.cancel_order(&order_id).await?;
                println!("Order cancelled");
            }
        }
        Err(e) => println!("Order failed: {}", e),
    }

    Ok(())
}
```

### WebSocket Streaming

```rust
use daytrader::brokers::alpaca::websocket::{AlpacaWebSocket, StreamConfig};
use futures_util::StreamExt;

async fn stream_market_data() -> Result<(), Box<dyn std::error::Error>> {
    let config = StreamConfig::new()
        .with_credentials(
            env::var("ALPACA_API_KEY")?,
            env::var("ALPACA_API_SECRET")?
        )
        .subscribe_to(vec!["AAPL", "GOOGL", "MSFT"]);

    let mut stream = AlpacaWebSocket::connect(config).await?;

    println!("📡 Connected to Alpaca WebSocket stream");

    // Process streaming data
    while let Some(message) = stream.next().await {
        match message? {
            StreamMessage::Trade(trade) => {
                println!("Trade: {} @ ${:.2} x{}",
                        trade.symbol,
                        trade.price,
                        trade.size);
            }
            StreamMessage::Quote(quote) => {
                println!("Quote: {} | Bid: ${:.2} | Ask: ${:.2}",
                        quote.symbol,
                        quote.bid_price,
                        quote.ask_price);
            }
            StreamMessage::Bar(bar) => {
                println!("Bar: {} | O:${:.2} H:${:.2} L:${:.2} C:${:.2} V:{}",
                        bar.symbol,
                        bar.open,
                        bar.high,
                        bar.low,
                        bar.close,
                        bar.volume);
            }
            StreamMessage::Error(error) => {
                eprintln!("Stream error: {}", error);
                break;
            }
        }
    }

    Ok(())
}
```

## Event-Driven Architecture

### Event Bus Setup

```rust
use daytrader::events::bus::EventBus;
use daytrader::events::types::MarketEvent;
use tokio::sync::mpsc;

async fn setup_event_driven_system() -> Result<(), Box<dyn std::error::Error>> {
    let event_bus = Arc::new(EventBus::new(1000));

    // Start multiple event processors
    let processors = vec![
        tokio::spawn(process_pattern_events(event_bus.clone())),
        tokio::spawn(process_risk_events(event_bus.clone())),
        tokio::spawn(process_order_events(event_bus.clone())),
    ];

    // Start market data feed
    let market_data_task = tokio::spawn(feed_market_data(event_bus.clone()));

    // Wait for all tasks
    for processor in processors {
        processor.await??;
    }
    market_data_task.await??;

    Ok(())
}

async fn process_pattern_events(event_bus: Arc<EventBus>) -> Result<(), Box<dyn std::error::Error>> {
    let mut receiver = event_bus.subscribe();

    while let Ok(event) = receiver.recv().await {
        if let MarketEvent::PatternDetected { signal } = event {
            if signal.confidence.value() >= 0.8 {
                println!("🎯 High-confidence signal: {}", signal.pattern_name.as_str());

                // Generate trading decision
                let decision = analyze_signal(&signal).await?;
                if let Some(order) = decision {
                    event_bus.publish(MarketEvent::OrderPlaced { order });
                }
            }
        }
    }

    Ok(())
}
```

## Database Operations

### Pattern Storage and Retrieval

```rust
use daytrader::db::Database;
use daytrader::patterns::types::PatternResult;

async fn store_and_retrieve_patterns(
    db: &dyn Database,
    patterns: Vec<PatternResult>
) -> Result<(), Box<dyn std::error::Error>> {
    // Store patterns
    for pattern in &patterns {
        db.store_pattern(pattern).await?;
    }

    println!("✅ Stored {} patterns", patterns.len());

    // Retrieve patterns for analysis
    let symbol = Symbol::new("AAPL")?;
    let recent_patterns = db.get_patterns(&symbol, 50).await?;

    // Analyze pattern effectiveness
    let mut successful_patterns = 0;
    for pattern in recent_patterns {
        if let Some(outcome) = analyze_pattern_outcome(&pattern).await? {
            if outcome.profit > 0 {
                successful_patterns += 1;
            }
        }
    }

    let success_rate = successful_patterns as f64 / recent_patterns.len() as f64;
    println!("Pattern success rate: {:.1}%", success_rate * 100.0);

    Ok(())
}
```

## Performance Optimization

### Parallel Pattern Detection

```rust
use std::sync::Arc;
use tokio::sync::Semaphore;
use futures::future::join_all;

async fn parallel_pattern_analysis(
    registry: &PatternRegistry,
    symbols: Vec<String>,
    max_concurrent: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let semaphore = Arc::new(Semaphore::new(max_concurrent));
    let registry = Arc::new(registry.clone());

    let tasks: Vec<_> = symbols.into_iter().map(|symbol| {
        let semaphore = semaphore.clone();
        let registry = registry.clone();

        tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            let symbol = Symbol::new(&symbol)?;
            let candles = get_candles_for_symbol(&symbol, 100).await?;

            let start = std::time::Instant::now();
            let patterns = registry.detect_all(&candles, None).await;
            let duration = start.elapsed();

            println!("Analyzed {}: {} patterns in {:?}", symbol.as_str(), patterns.len(), duration);

            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((symbol, patterns, duration))
        })
    }).collect();

    let results = join_all(tasks).await;

    for result in results {
        match result?? {
            (symbol, patterns, duration) => {
                // Process results
                if !patterns.is_empty() {
                    println!("📊 {}: {} patterns (avg: {:.2}ms per pattern)",
                            symbol.as_str(),
                            patterns.len(),
                            duration.as_millis() as f64 / patterns.len() as f64);
                }
            }
        }
    }

    Ok(())
}
```

### Memory-Efficient Processing

```rust
use std::collections::VecDeque;

// Circular buffer for recent candles
struct CandleBuffer {
    buffer: VecDeque<Candle>,
    max_size: usize,
}

impl CandleBuffer {
    fn new(max_size: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    fn push(&mut self, candle: Candle) {
        if self.buffer.len() == self.max_size {
            self.buffer.pop_front();
        }
        self.buffer.push_back(candle);
    }

    fn candles(&self) -> &[Candle] {
        self.buffer.as_slices().0
    }
}

async fn process_streaming_candles() -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = CandleBuffer::new(50); // Keep last 50 candles
    let mut stream = get_candle_stream().await?;

    while let Some(candle) = stream.recv().await {
        buffer.push(candle);

        // Only process if we have enough data
        if buffer.candles().len() >= 5 {
            let patterns = detect_patterns(buffer.candles()).await?;

            for pattern in patterns {
                if pattern.confidence.value() >= 0.7 {
                    handle_pattern_signal(pattern).await?;
                }
            }
        }
    }

    Ok(())
}
```

## Testing Patterns

### Unit Testing Pattern Detectors

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn create_test_candles() -> Vec<Candle> {
        vec![
            Candle {
                symbol: Symbol::new("TEST").unwrap(),
                timestamp: Utc::now(),
                open: Price::new(Decimal::new(10000, 2)).unwrap(),
                high: Price::new(Decimal::new(10500, 2)).unwrap(),
                low: Price::new(Decimal::new(9500, 2)).unwrap(),
                close: Price::new(Decimal::new(10200, 2)).unwrap(),
                volume: 1000000,
            }
        ]
    }

    #[test]
    fn test_hammer_detection_success() {
        let detector = HammerDetector::new();
        let candles = create_hammer_candles();
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
        let candles = create_test_candles().into_iter().take(candle_count).collect::<Vec<_>>();
        let params = PatternParams::default();

        let result = detector.detect(&candles, &params);

        assert!(matches!(
            result,
            Err(PatternError::InsufficientData { available, .. }) if available == expected_available
        ));
    }

    #[tokio::test]
    async fn test_registry_integration() {
        let registry = PatternRegistry::new().await;
        registry.register("hammer", Box::new(HammerDetector::new())).unwrap();

        let candles = create_test_candles();
        let results = registry.detect_all(&candles, None).await;

        assert!(!results.is_empty());
        assert!(results.iter().all(|r| r.confidence.value() >= 0.0));
    }
}
```

### Integration Testing

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use daytrader::brokers::MockBroker;

    #[tokio::test]
    async fn test_complete_trading_workflow() {
        // Setup mock broker
        let broker = MockBroker::new();
        broker.set_balance(DollarAmount::new(Decimal::new(5000000, 2)).unwrap());

        // Setup risk engine
        let risk_config = RiskConfig::default();
        let event_bus = Arc::new(EventBus::new(100));
        let risk_engine = RiskEngine::new(risk_config, event_bus, AccountId::new("test"), Decimal::new(5000000, 2));

        // Setup pattern registry
        let registry = PatternRegistry::new().await;

        // Simulate market data
        let candles = generate_test_candles();

        // Detect patterns
        let patterns = registry.detect_all(&candles, None).await;

        // Process signals
        for pattern in patterns {
            if pattern.confidence.value() >= 0.8 {
                if let Ok(order) = create_order_from_pattern(&pattern) {
                    if risk_engine.validate_order(&order).await.is_ok() {
                        let _order_id = broker.place_order(order).await.unwrap();
                    }
                }
            }
        }

        // Verify results
        let positions = broker.get_positions().await.unwrap();
        assert!(!positions.is_empty()); // Should have opened positions
    }
}
```

This examples guide provides comprehensive coverage of common usage patterns and integration scenarios for the Day Trading Platform. Each example includes error handling, best practices, and practical implementation details.</content>
</xai:function_call">The examples guide has been created successfully. It provides comprehensive examples covering:

- Getting started and basic setup
- Pattern recognition with single and multi-pattern detection
- Real-time pattern monitoring
- Risk management configuration and validation
- Broker integration with Alpaca
- WebSocket streaming
- Event-driven architecture
- Database operations
- Performance optimization techniques
- Testing patterns and best practices

This serves as a practical guide for developers to understand and implement various features of the platform.