# Alpaca Markets Integration

This module provides a complete integration with Alpaca Markets for both order execution (REST API) and real-time market data (WebSocket).

## Features

- ✅ **REST API Client** - Order execution, account management, positions
- ✅ **WebSocket Client** - Real-time quotes and bars/candles
- ✅ **Type Safety** - All domain types use newtypes (no raw strings)
- ✅ **Static Dispatch** - Zero-cost abstractions with generics
- ✅ **Auto-Reconnect** - WebSocket automatically reconnects on disconnect
- ✅ **Paper Trading** - Easy configuration for paper vs live trading
- ✅ **Comprehensive Tests** - Unit tests for all components

## Quick Start

### Basic Setup

```rust
use daytrader::brokers::alpaca::{AlpacaConfig, AlpacaProvider};
use daytrader::brokers::{OrderExecutionProvider, MarketDataProvider};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure for paper trading
    let config = AlpacaConfig::paper()
        .with_credentials(
            std::env::var("ALPACA_API_KEY")?,
            std::env::var("ALPACA_API_SECRET")?,
        );
    
    // Create provider
    let mut provider = AlpacaProvider::new(config);
    
    // Connect WebSocket for real-time data
    provider.connect_websocket().await?;
    
    Ok(())
}
```

### Account Information

```rust
use daytrader::brokers::OrderExecutionProvider;

// Get account details
let account = provider.get_account().await?;
println!("Buying Power: ${}", account.buying_power);
println!("Cash: ${}", account.cash);
println!("Portfolio Value: ${}", account.portfolio_value);
```

### Placing Orders

```rust
use daytrader::brokers::{Order, OrderSide, Symbol, Quantity, Price};
use rust_decimal::Decimal;

// Market order
let symbol = Symbol::new("AAPL")?;
let qty = Quantity::new(100)?;
let order = Order::market(symbol.clone(), OrderSide::Buy, qty);
let order_id = provider.place_order(order).await?;
println!("Order placed: {}", order_id);

// Limit order
let price = Price::new(Decimal::new(15000, 2))?; // $150.00
let order = Order::limit(symbol, OrderSide::Sell, qty, price);
let order_id = provider.place_order(order).await?;
```

### Managing Orders

```rust
// Get order status
let order = provider.get_order(&order_id).await?;
println!("Order status: {:?}", order.status);

// Get all open orders
let open_orders = provider.get_open_orders().await?;
println!("Open orders: {}", open_orders.len());

// Cancel an order
provider.cancel_order(&order_id).await?;
```

### Position Management

```rust
// Get all positions
let positions = provider.get_positions().await?;
for position in positions {
    println!("{}: {} shares @ ${}", 
        position.symbol, 
        position.quantity, 
        position.avg_entry_price
    );
}

// Get specific position
let symbol = Symbol::new("AAPL")?;
if let Some(position) = provider.get_position(&symbol).await? {
    println!("Unrealized P/L: ${}", position.unrealized_pl);
}
```

### Real-Time Market Data

```rust
use daytrader::brokers::MarketDataProvider;

// Subscribe to quotes
let symbols = vec![
    Symbol::new("AAPL")?,
    Symbol::new("MSFT")?,
    Symbol::new("GOOGL")?,
];
provider.subscribe_quotes(symbols.clone()).await?;

// Get latest quote
let quote = provider.get_quote(&symbols[0]).await?;
println!("Bid: ${}, Ask: ${}", quote.bid, quote.ask);

// Unsubscribe when done
provider.unsubscribe_quotes(symbols).await?;
```

## Configuration

### Paper Trading (Default)

```rust
let config = AlpacaConfig::paper()
    .with_credentials(api_key, api_secret);
```

### Live Trading

```rust
let config = AlpacaConfig::live()
    .with_credentials(api_key, api_secret);
```

### Environment Variables

It's recommended to store credentials in environment variables:

```bash
export ALPACA_API_KEY="your_api_key"
export ALPACA_API_SECRET="your_api_secret"
```

## Architecture

### Module Structure

```
alpaca/
├── config.rs       - Configuration for paper/live trading
├── types.rs        - Alpaca-specific JSON response types
├── rest.rs         - REST API client (OrderExecutionProvider)
├── websocket.rs    - WebSocket client (MarketDataProvider)
└── mod.rs          - Main AlpacaProvider combining both
```

### Type Mapping

Alpaca JSON responses are converted to our domain types:

| Alpaca Type | Domain Type | Description |
|-------------|-------------|-------------|
| `AlpacaAccount` | `Account` | Account information |
| `AlpacaOrder` | `Order` | Order details |
| `AlpacaPosition` | `Position` | Position information |
| `AlpacaQuote` | `Quote` | Real-time quote |
| `AlpacaBar` | `Candle` | OHLCV bar/candle |

### Error Handling

All errors are mapped to `BrokerError`:

- `ConnectionFailed` - Network or WebSocket connection issues
- `AuthenticationFailed` - Invalid API credentials
- `OrderRejected` - Order validation failed
- `InsufficientFunds` - Not enough buying power
- `SymbolNotFound` - Invalid symbol
- `RateLimitExceeded` - API rate limit hit
- `ApiError` - Other API errors

## WebSocket Features

### Auto-Reconnect

The WebSocket client automatically reconnects on disconnection with exponential backoff:

```rust
// Connection is maintained automatically
provider.connect_websocket().await?;

// Subscribe to symbols - connection persists across reconnects
provider.subscribe_quotes(symbols).await?;
```

### State Recovery

After reconnection, the client automatically:
1. Re-authenticates
2. Re-subscribes to all previously subscribed symbols
3. Continues receiving updates

### Heartbeat Monitoring

The client sends periodic heartbeats (every 30 seconds) to maintain the connection.

## Testing

### Unit Tests

Each module includes unit tests:

```bash
cargo test --lib alpaca
```

### Integration Tests

Integration tests with paper trading:

```bash
# Set credentials
export ALPACA_API_KEY="paper_key"
export ALPACA_API_SECRET="paper_secret"

# Run integration tests
cargo test --test alpaca_integration
```

## Performance Considerations

### Static Dispatch

The provider uses static dispatch with generics for zero-cost abstractions:

```rust
async fn trade<P: OrderExecutionProvider>(provider: &P) {
    // No virtual function call overhead
    provider.place_order(order).await?;
}
```

### Connection Pooling

The REST client uses connection pooling via `reqwest::Client` for efficient HTTP requests.

### Non-Blocking WebSocket

WebSocket processing is fully asynchronous and non-blocking, using `tokio::select!` for concurrent message handling.

## Limitations

### Historical Data

The current implementation does not support historical candle retrieval via REST API. This will be added in a future update.

To work around this, you can:
1. Use the WebSocket to collect real-time bars
2. Store them in the database
3. Query historical data from your database

### Rate Limits

Alpaca enforces rate limits:
- REST API: 200 requests per minute
- WebSocket: No explicit limit, but excessive subscriptions may be throttled

The client automatically handles rate limit errors with retry logic.

## Examples

See the `examples/` directory for complete working examples:

- `examples/alpaca_basic.rs` - Basic account and order operations
- `examples/alpaca_streaming.rs` - Real-time market data streaming
- `examples/alpaca_trading.rs` - Complete trading workflow

## References

- [Alpaca API Documentation](https://alpaca.markets/docs/api-references/)
- [Alpaca WebSocket Documentation](https://alpaca.markets/docs/api-references/market-data-api/stock-pricing-data/realtime/)
- [Technical Architecture](../../../technical-architecture.md)