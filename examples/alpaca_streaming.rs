//! Alpaca real-time market data streaming example
//!
//! This example demonstrates:
//! - Connecting to Alpaca WebSocket
//! - Subscribing to real-time quotes
//! - Receiving and displaying market data updates
//!
//! To run this example:
//! ```bash
//! export ALPACA_API_KEY="your_paper_api_key"
//! export ALPACA_API_SECRET="your_paper_api_secret"
//! cargo run --example alpaca_streaming
//! ```

use daytrader::brokers::alpaca::{AlpacaConfig, AlpacaProvider};
use daytrader::brokers::{MarketDataProvider, Symbol};
use std::env;
use tokio::time::{Duration, sleep};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Get credentials from environment
    let api_key = env::var("ALPACA_API_KEY").expect("ALPACA_API_KEY environment variable not set");
    let api_secret =
        env::var("ALPACA_API_SECRET").expect("ALPACA_API_SECRET environment variable not set");

    println!("🚀 Initializing Alpaca provider with WebSocket...");

    // Create configuration for paper trading
    let config = AlpacaConfig::paper().with_credentials(api_key, api_secret);

    // Create provider
    let mut provider = AlpacaProvider::new(config);

    // Connect WebSocket
    println!("🔌 Connecting to WebSocket...");
    provider.connect_websocket().await?;
    println!("✅ WebSocket connected!");

    // Define symbols to watch
    let symbols = vec![
        Symbol::new("AAPL")?,
        Symbol::new("MSFT")?,
        Symbol::new("GOOGL")?,
        Symbol::new("TSLA")?,
        Symbol::new("AMZN")?,
    ];

    println!("\n📊 Subscribing to symbols: {:?}", symbols);
    provider.subscribe_quotes(symbols.clone()).await?;
    println!("✅ Subscribed successfully!");

    println!("\n📈 Streaming real-time quotes (press Ctrl+C to stop)...\n");

    // Poll for quotes every 2 seconds
    for i in 0..30 {
        // Wait a bit for data to arrive
        sleep(Duration::from_secs(2)).await;

        println!("--- Update {} ---", i + 1);

        for symbol in &symbols {
            match provider.get_quote(symbol).await {
                Ok(quote) => {
                    let spread = quote.ask.value() - quote.bid.value();
                    let spread_pct =
                        (spread / quote.bid.value()) * rust_decimal::Decimal::new(100, 0);

                    println!(
                        "{:6} | Bid: ${:8.2} x {:4} | Ask: ${:8.2} x {:4} | Last: ${:8.2} | Spread: ${:.4} ({:.3}%)",
                        quote.symbol,
                        quote.bid,
                        quote.bid_size,
                        quote.ask,
                        quote.ask_size,
                        quote.last,
                        spread,
                        spread_pct
                    );
                }
                Err(e) => {
                    println!("{:6} | No data yet: {}", symbol, e);
                }
            }
        }
        println!();
    }

    // Unsubscribe
    println!("🔌 Unsubscribing from symbols...");
    provider.unsubscribe_quotes(symbols).await?;

    // Stop stream
    println!("🛑 Stopping stream...");
    provider.stop_stream().await?;

    println!("\n✅ Example completed successfully!");
    Ok(())
}
