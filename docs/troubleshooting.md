# Troubleshooting Guide

This guide helps you diagnose and resolve common issues when working with the Day Trading Platform.

## Table of Contents

1. [Build and Compilation Issues](#build-and-compilation-issues)
2. [Runtime Errors](#runtime-errors)
3. [Broker API Issues](#broker-api-issues)
4. [Pattern Detection Problems](#pattern-detection-problems)
5. [Risk Management Issues](#risk-management-issues)
6. [Database Problems](#database-problems)
7. [Performance Issues](#performance-issues)
8. [Testing Problems](#testing-problems)

## Build and Compilation Issues

### Compilation Fails with "unsafe code" Error

**Error:** `error: unsafe code is not allowed in this crate`

**Cause:** The crate forbids unsafe code usage.

**Solution:**
- Remove any `unsafe` blocks or functions
- Use safe alternatives from the standard library
- If unsafe code is absolutely necessary, consider moving it to a separate crate

### Missing Dependencies

**Error:** `error: linking with 'cc' failed` or similar linker errors

**Solution:**
```bash
# Install system dependencies
# Ubuntu/Debian
sudo apt-get install build-essential pkg-config libssl-dev sqlite3

# macOS
brew install sqlite3 openssl

# Verify installation
cargo check
```

### Version Conflicts

**Error:** `error: multiple packages with the same name`

**Solution:**
- Update Cargo.toml to use compatible versions
- Run `cargo update` to update dependencies
- Check for breaking changes in dependency updates

### Feature Flag Issues

**Error:** `error: feature 'xyz' is not enabled`

**Solution:**
```toml
# In Cargo.toml, enable required features
[dependencies]
daytrader = { version = "0.1", features = ["alpaca-streaming", "prometheus-metrics"] }
```

## Runtime Errors

### Configuration Loading Fails

**Error:** `Error loading configuration: No such file or directory`

**Solutions:**
1. **Check file path:**
   ```bash
   ls -la config/default.toml
   ```

2. **Create default configuration:**
   ```bash
   mkdir -p config
   cat > config/default.toml << EOF
   [broker]
   provider = "Alpaca"
   api_key = "your_key"
   api_secret = "your_secret"
   is_paper = true

   [risk]
   max_daily_loss_percent = 0.02
   max_position_size_percent = 0.10
   enforce_pdt_rule = true
   EOF
   ```

3. **Check permissions:**
   ```bash
   chmod 600 config/default.toml
   ```

### Environment Variables Not Set

**Error:** `ALPACA_API_KEY environment variable not set`

**Solutions:**
```bash
# Set environment variables
export ALPACA_API_KEY="your_paper_api_key"
export ALPACA_API_SECRET="your_paper_api_secret"
export RUST_LOG="daytrader=debug"

# Or create a .env file
echo "ALPACA_API_KEY=your_key" > .env
echo "ALPACA_API_SECRET=your_secret" >> .env

# Load environment variables
source .env
```

### Database Connection Issues

**Error:** `Database error: unable to open database file`

**Solutions:**
1. **Check database path:**
   ```bash
   ls -la data/
   mkdir -p data
   ```

2. **Set correct permissions:**
   ```bash
   chmod 755 data
   ```

3. **Check SQLite installation:**
   ```bash
   sqlite3 --version
   which sqlite3
   ```

4. **Initialize database:**
   ```bash
   ./scripts/setup_db.sh
   ```

## Broker API Issues

### Authentication Failed

**Error:** `BrokerError::AuthenticationFailed`

**Solutions:**
1. **Verify API credentials:**
   - Check that API key and secret are correct
   - Ensure you're using paper trading credentials for testing
   - Verify credentials haven't expired

2. **Check account permissions:**
   - Ensure your Alpaca account has API access enabled
   - Verify you're using the correct account type (paper/live)

3. **Network connectivity:**
   ```bash
   curl -I https://api.alpaca.markets
   ```

### Rate Limiting

**Error:** `BrokerError::RateLimitExceeded`

**Solutions:**
1. **Implement backoff strategy:**
   ```rust
   use tokio::time::{sleep, Duration};

   async fn place_order_with_retry(order: Order, broker: &dyn Broker) -> Result<OrderId> {
       let mut attempts = 0;
       loop {
           match broker.place_order(order.clone()).await {
               Ok(order_id) => return Ok(order_id),
               Err(BrokerError::RateLimitExceeded) if attempts < 3 => {
                   attempts += 1;
                   sleep(Duration::from_secs(2u64.pow(attempts))).await;
                   continue;
               }
               Err(e) => return Err(e),
           }
       }
   }
   ```

2. **Reduce request frequency:**
   - Batch operations where possible
   - Implement request throttling
   - Use WebSocket streaming instead of REST polling

### Order Rejection

**Error:** `BrokerError::OrderRejected`

**Common causes:**
- Insufficient buying power
- Invalid symbol
- Market closed
- Position size limits exceeded

**Debugging:**
```rust
// Check account balance
let account = broker.get_account_balance().await?;
println!("Buying Power: ${}", account.buying_power);
println!("Cash: ${}", account.cash);

// Validate order parameters
println!("Order: {:?}", order);
assert!(order.quantity.value() > 0);
assert!(order.price.value() > Decimal::ZERO);
```

## Pattern Detection Problems

### No Patterns Detected

**Issue:** Pattern detection returns empty results

**Solutions:**
1. **Check input data:**
   ```rust
   println!("Candles provided: {}", candles.len());
   assert!(candles.len() >= 5); // Minimum required for most patterns

   for candle in &candles {
       println!("Candle: O:{:.2} H:{:.2} L:{:.2} C:{:.2} V:{}",
               candle.open.value(),
               candle.high.value(),
               candle.low.value(),
               candle.close.value(),
               candle.volume);
   }
   ```

2. **Verify pattern parameters:**
   ```rust
   let params = PatternParams::new()
       .with_min_confidence(0.1) // Lower threshold for testing
       .require_volume_confirmation(false); // Disable volume check
   ```

3. **Check pattern registration:**
   ```rust
   println!("Registered patterns: {:?}", registry.list_patterns());
   ```

### Low Confidence Scores

**Issue:** Patterns detected but with very low confidence

**Solutions:**
1. **Adjust confidence thresholds:**
   ```rust
   let params = PatternParams::new()
       .with_min_confidence(0.3); // Lower minimum confidence
   ```

2. **Improve data quality:**
   - Ensure candles have sufficient price movement
   - Check for realistic volume data
   - Verify timestamp ordering

3. **Context analysis:**
   ```rust
   // Add trend context
   let params = params.with_trend_context(TrendDirection::Bullish);
   ```

### Incorrect Pattern Classification

**Issue:** Patterns detected but wrong type or signal direction

**Solutions:**
1. **Validate candle data:**
   ```rust
   // Check for data anomalies
   for candle in &candles {
       assert!(candle.high >= candle.low);
       assert!(candle.open > Decimal::ZERO);
       assert!(candle.close > Decimal::ZERO);
   }
   ```

2. **Review pattern algorithms:**
   - Check mathematical formulas in pattern detectors
   - Verify threshold values
   - Test with known pattern examples

## Risk Management Issues

### Orders Rejected by Risk Engine

**Error:** `RiskError::PositionSizeLimitExceeded`

**Solutions:**
1. **Check risk configuration:**
   ```rust
   println!("Max position size: ${}", risk_config.max_position_size_dollars);
   println!("Max position percent: {}%", risk_config.max_position_size_percent);
   ```

2. **Calculate position size:**
   ```rust
   let position_value = order.quantity.value() * order.price.value();
   let account_balance = broker.get_account_balance().await?;
   let position_percent = position_value / account_balance.portfolio_value;

   println!("Position value: ${:.2}", position_value);
   println!("Portfolio value: ${:.2}", account_balance.portfolio_value);
   println!("Position percent: {:.2}%", position_percent * 100.0);
   ```

3. **Adjust risk settings:**
   ```rust
   let conservative_config = RiskConfig {
       max_position_size_percent: Percentage::new(Decimal::new(500, 2))?, // 5%
       ..Default::default()
   };
   ```

### PDT Rule Violations

**Error:** `RiskError::PDTViolation`

**Solutions:**
1. **Check PDT status:**
   ```rust
   let risk_state = risk_engine.get_risk_state().await;
   println!("Day trades today: {}", risk_state.day_trade_count);
   println!("Is PDT restricted: {}", risk_state.is_pdt_restricted);
   ```

2. **Reset day trade count:**
   - Wait until next trading day (after 4:00 PM ET)
   - Monitor day trade count carefully
   - Consider upgrading to cash account for more trades

3. **Alternative strategies:**
   - Use limit orders instead of market orders
   - Implement position sizing to stay under PDT limits
   - Focus on swing trading instead of day trading

## Database Problems

### Migration Failures

**Error:** `Database migration failed`

**Solutions:**
1. **Check database schema:**
   ```sql
   .schema
   ```

2. **Manual migration:**
   ```bash
   sqlite3 data/daytrader.db < src/db/schema.sql
   ```

3. **Backup and restore:**
   ```bash
   cp data/daytrader.db data/daytrader.db.backup
   rm data/daytrader.db
   ./scripts/setup_db.sh
   ```

### Connection Pool Exhaustion

**Error:** `Database connection pool exhausted`

**Solutions:**
1. **Increase pool size:**
   ```rust
   let pool = sqlx::sqlite::SqlitePoolOptions::new()
       .max_connections(10) // Increase from default
       .connect(&database_url)
       .await?;
   ```

2. **Implement connection pooling:**
   ```rust
   // Reuse database connections
   let db = Arc::new(database);
   // Share across tasks
   ```

3. **Add timeouts:**
   ```rust
   .acquire_timeout(Duration::from_secs(30))
   .idle_timeout(Duration::from_secs(300))
   ```

### Data Corruption

**Error:** `Database disk image is malformed`

**Solutions:**
1. **Immediate backup:**
   ```bash
   cp data/daytrader.db data/daytrader.db.corrupt
   ```

2. **Repair database:**
   ```bash
   sqlite3 data/daytrader.db "PRAGMA integrity_check;"
   sqlite3 data/daytrader.db ".dump" | sqlite3 data/daytrader_repaired.db
   ```

3. **Restore from backup:**
   ```bash
   cp data/daytrader_repaired.db data/daytrader.db
   ```

## Performance Issues

### Slow Pattern Detection

**Issue:** Pattern detection taking > 1 second

**Solutions:**
1. **Profile performance:**
   ```rust
   use std::time::Instant;

   let start = Instant::now();
   let results = registry.detect_all(&candles, None).await;
   let duration = start.elapsed();

   println!("Detection took: {:?}", duration);
   println!("Candles processed: {}", candles.len());
   ```

2. **Optimize data structures:**
   ```rust
   // Use circular buffers for recent data
   let mut buffer = CircularBuffer::new(100);
   ```

3. **Parallel processing:**
   ```rust
   // Process multiple symbols concurrently
   let tasks: Vec<_> = symbols.into_iter()
       .map(|symbol| tokio::spawn(process_symbol(symbol)))
       .collect();

   let results = join_all(tasks).await;
   ```

### High Memory Usage

**Issue:** Application using excessive memory

**Solutions:**
1. **Monitor memory usage:**
   ```rust
   // Add memory profiling
   #[cfg(feature = "performance-monitoring")]
   {
       println!("Memory usage: {} MB", get_memory_usage());
   }
   ```

2. **Reduce data retention:**
   ```rust
   // Limit stored candle history
   const MAX_CANDLES: usize = 1000;
   if candles.len() > MAX_CANDLES {
       candles = candles[candles.len() - MAX_CANDLES..].to_vec();
   }
   ```

3. **Use streaming processing:**
   ```rust
   // Process data in chunks
   for chunk in candles.chunks(100) {
       process_candle_chunk(chunk).await?;
   }
   ```

### Event Bus Overload

**Issue:** Event bus buffer filling up

**Solutions:**
1. **Increase buffer size:**
   ```rust
   let event_bus = EventBus::new(10000); // Larger buffer
   ```

2. **Implement event filtering:**
   ```rust
   // Only publish high-confidence signals
   if pattern.confidence.value() >= 0.8 {
       event_bus.publish(event);
   }
   ```

3. **Async event processing:**
   ```rust
   // Process events asynchronously
   tokio::spawn(async move {
       while let Ok(event) = receiver.recv().await {
           process_event_async(event).await;
       }
   });
   ```

## Testing Problems

### Flaky Tests

**Issue:** Tests pass sometimes, fail others

**Solutions:**
1. **Add test isolation:**
   ```rust
   #[tokio::test]
   async fn test_pattern_detection() {
       // Use unique database for each test
       let db_path = format!("/tmp/test_{}.db", uuid::Uuid::new_v4());
       let db = SQLiteDatabase::new(&db_path).await.unwrap();

       // Test logic here

       // Cleanup
       std::fs::remove_file(db_path).ok();
   }
   ```

2. **Fix timing issues:**
   ```rust
   // Use proper async timing
   tokio::time::sleep(Duration::from_millis(100)).await;
   ```

3. **Mock external dependencies:**
   ```rust
   let broker = MockBroker::new();
   broker.set_balance(DollarAmount::new(Decimal::new(5000000, 2)).unwrap());
   ```

### Integration Test Failures

**Issue:** Integration tests failing in CI but passing locally

**Solutions:**
1. **Check environment differences:**
   ```bash
   # Compare environments
   env | grep -E "(API|DATABASE|LOG)"
   ```

2. **Use test-specific configuration:**
   ```rust
   let config = TestConfig {
       database_url: "sqlite::memory:".to_string(),
       broker: MockBroker::new(),
       ..Default::default()
   };
   ```

3. **Add retry logic for flaky services:**
   ```rust
   #[tokio::test]
   async fn test_broker_integration() {
       let mut attempts = 0;
       loop {
           match run_broker_test().await {
               Ok(_) => break,
               Err(_) if attempts < 3 => {
                   attempts += 1;
                   tokio::time::sleep(Duration::from_secs(1)).await;
                   continue;
               }
               Err(e) => panic!("Test failed after {} attempts: {}", attempts, e),
           }
       }
   }
   ```

## Getting Help

If you can't resolve an issue:

1. **Check existing issues:** Search the GitHub issue tracker
2. **Enable debug logging:** `RUST_LOG=debug cargo run`
3. **Collect diagnostics:**
   ```bash
   # System information
   uname -a
   rustc --version
   cargo --version

   # Dependency information
   cargo tree

   # Test with minimal example
   cargo run --example minimal_test
   ```
4. **Create a minimal reproduction case**
5. **File a detailed bug report with:**
   - Steps to reproduce
   - Expected vs actual behavior
   - Full error messages
   - System information
   - Code snippets

## Common Error Codes

| Error Code | Description | Solution |
|------------|-------------|----------|
| `E001` | Invalid configuration | Check config file syntax |
| `E002` | Database connection failed | Verify database path and permissions |
| `E003` | Broker authentication failed | Check API credentials |
| `E004` | Insufficient funds | Add more cash to account |
| `E005` | Rate limit exceeded | Implement backoff strategy |
| `E006` | Pattern detection failed | Check input data quality |
| `E007` | Risk limit exceeded | Adjust risk parameters |
| `E008` | PDT violation | Wait for next trading day |

## Prevention Best Practices

1. **Use proper error handling:**
   ```rust
   match operation().await {
       Ok(result) => println!("Success: {:?}", result),
       Err(e) => {
           eprintln!("Error: {}", e);
           // Log additional context
           // Attempt recovery
       }
   }
   ```

2. **Implement health checks:**
   ```rust
   async fn health_check() -> Result<()> {
       // Check database connectivity
       db.ping().await?;

       // Check broker connectivity
       broker.get_account().await?;

       // Check system resources
       check_memory_usage()?;
       check_disk_space()?;

       Ok(())
   }
   ```

3. **Add monitoring and alerting:**
   ```rust
   // Log important events
   tracing::info!("Order placed: {}", order_id);

   // Track metrics
   metrics::counter!("orders_placed_total").increment(1);

   // Alert on errors
   if let Err(e) = critical_operation().await {
       alert_system.send_alert(&format!("Critical error: {}", e)).await;
   }
   ```

This troubleshooting guide covers the most common issues. For additional support, check the documentation or file an issue on GitHub.</content>
</xai:function_call">The troubleshooting guide has been created successfully. It provides comprehensive solutions for:

- Build and compilation issues
- Runtime errors and configuration problems
- Broker API authentication and rate limiting
- Pattern detection accuracy and performance
- Risk management rule violations
- Database connection and migration issues
- Performance optimization
- Testing problems and flaky tests

The guide includes practical code examples, debugging steps, and prevention best practices.