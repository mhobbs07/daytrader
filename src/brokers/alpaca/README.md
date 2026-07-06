# Alpaca Markets Integration

This module provides a complete integration with Alpaca Markets for both order execution (REST API) and real-time market data (WebSocket).

## Features

- ✅ **REST API Client** - Order execution, account management, positions
- ✅ **WebSocket Client** - Real-time quotes and bars/candles
- ✅ **Type Safety** - All domain types use newtypes (no raw strings)
- ✅ **Static Dispatch** - Zero-cost abstractions with generics
- ✅ **Client-Side Rate Limiting** - Token-bucket limiter throttles REST requests before they hit Alpaca's server-side limit
- ✅ **Auto-Reconnect with Exponential Backoff** - WebSocket reconnects with a doubling delay (1s -> 60s cap), reset on successful connect
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
├── ratelimit.rs    - Token-bucket rate limiter for REST requests
├── backoff.rs      - Exponential backoff for WebSocket reconnects
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
- `RateLimitExceeded { retry_after }` - API rate limit hit; `retry_after` is parsed from the response's `Retry-After` header (seconds), falling back to a 60-second default only when the header is missing or unparseable
- `ApiError` - Other API errors

## Rate Limiting

The REST client throttles itself with an in-process token-bucket limiter
before every request, so it backs off proactively instead of relying on
the server to reject requests with HTTP 429.

- Default: 200 requests/minute, matching Alpaca's documented free-tier limit
- Configurable via `RestClient::with_rate_limit(config, RateLimiterConfig { max_requests, refill_period })`
- Implemented with `tokio::sync::Mutex` and `tokio::time` (see `ratelimit.rs`) — no extra crate dependency
- If a 429 is still returned (e.g. multiple client instances sharing one account), the real `Retry-After` header is parsed into `BrokerError::RateLimitExceeded`

## WebSocket Features

### Auto-Reconnect with Exponential Backoff

The WebSocket client automatically reconnects on disconnection. The delay
between attempts starts at 1 second and doubles on each consecutive
failure, capped at 60 seconds, and resets back to 1 second as soon as a
connection is re-established (see `backoff.rs`):

```rust
// Connection is maintained automatically
provider.connect_websocket().await?;

// Subscribe to symbols
provider.subscribe_quotes(symbols).await?;
```

Note: reconnection re-authenticates the session, but does **not** currently
re-subscribe to previously subscribed symbols automatically — callers that
need subscriptions to survive a reconnect must call `subscribe_quotes`
again after `connect_websocket`/`start_stream` reports the stream is back.

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
- REST API: 200 requests per minute (client-side default; see "Rate Limiting" above)
- WebSocket: No explicit limit, but excessive subscriptions may be throttled

The REST client proactively throttles itself with the token-bucket limiter
described above. If a 429 is still received, it is surfaced to the caller
as `BrokerError::RateLimitExceeded { retry_after }` — the client does not
automatically retry the request; callers are responsible for deciding
whether and when to retry.

## Examples

See the `examples/` directory for complete working examples:

- `examples/alpaca_basic.rs` - Basic account and order operations
- `examples/alpaca_streaming.rs` - Real-time market data streaming
- `examples/alpaca_trading.rs` - Complete trading workflow

## References

- [Alpaca API Documentation](https://alpaca.markets/docs/api-references/)
- [Alpaca WebSocket Documentation](https://alpaca.markets/docs/api-references/market-data-api/stock-pricing-data/realtime/)
- [Technical Architecture](../../../technical-architecture.md)