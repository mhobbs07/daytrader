//! Basic Alpaca provider usage example
//!
//! This example demonstrates:
//! - Creating an Alpaca provider with paper trading
//! - Getting account information
//! - Placing and managing orders
//! - Checking positions
//!
//! To run this example:
//! ```bash
//! export ALPACA_API_KEY="your_paper_api_key"
//! export ALPACA_API_SECRET="your_paper_api_secret"
//! cargo run --example alpaca_basic
//! ```

use daytrader::brokers::alpaca::{AlpacaConfig, AlpacaProvider};
use daytrader::brokers::{Order, OrderExecutionProvider, OrderSide, Quantity, Symbol};
use rust_decimal::Decimal;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Get credentials from environment
    let api_key = env::var("ALPACA_API_KEY").expect("ALPACA_API_KEY environment variable not set");
    let api_secret =
        env::var("ALPACA_API_SECRET").expect("ALPACA_API_SECRET environment variable not set");

    println!("🚀 Initializing Alpaca provider (Paper Trading)...");

    // Create configuration for paper trading
    let config = AlpacaConfig::paper().with_credentials(api_key, api_secret);

    // Create provider
    let provider = AlpacaProvider::new(config);

    // Get account information
    println!("\n📊 Fetching account information...");
    let account = provider.get_account().await?;
    println!("  Account Number: {}", account.account_number);
    println!("  Buying Power: ${}", account.buying_power);
    println!("  Cash: ${}", account.cash);
    println!("  Portfolio Value: ${}", account.portfolio_value);
    println!("  Day Trade Count: {}", account.day_trade_count);
    println!(
        "  Pattern Day Trader: {}",
        if account.is_pattern_day_trader {
            "Yes"
        } else {
            "No"
        }
    );

    // Get current positions
    println!("\n📈 Current Positions:");
    let positions = provider.get_positions().await?;
    if positions.is_empty() {
        println!("  No open positions");
    } else {
        for position in positions {
            println!(
                "  {} - {} shares @ ${:.2} | Current: ${:.2} | P/L: ${:.2} ({:.2}%)",
                position.symbol,
                position.quantity,
                position.avg_entry_price,
                position.current_price,
                position.unrealized_pl,
                position.unrealized_pl_percent
            );
        }
    }

    // Get open orders
    println!("\n📋 Open Orders:");
    let open_orders = provider.get_open_orders().await?;
    if open_orders.is_empty() {
        println!("  No open orders");
    } else {
        for order in open_orders {
            println!(
                "  {} - {:?} {} {} @ ${:?} | Status: {:?}",
                order.id, order.side, order.quantity, order.symbol, order.limit_price, order.status
            );
        }
    }

    // Example: Place a market order (commented out for safety)
    // Uncomment to actually place an order
    /*
    println!("\n💰 Placing market order...");
    let symbol = Symbol::new("AAPL")?;
    let qty = Quantity::new(1)?;
    let order = Order::market(symbol.clone(), OrderSide::Buy, qty);

    match provider.place_order(order).await {
        Ok(order_id) => {
            println!("  ✅ Order placed successfully: {}", order_id);

            // Wait a moment for order to process
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            // Get order status
            let order_status = provider.get_order(&order_id).await?;
            println!("  Order status: {:?}", order_status.status);

            // Cancel the order if it's still open
            if matches!(order_status.status, daytrader::brokers::OrderStatus::Accepted) {
                println!("  Cancelling order...");
                provider.cancel_order(&order_id).await?;
                println!("  ✅ Order cancelled");
            }
        }
        Err(e) => {
            println!("  ❌ Failed to place order: {}", e);
        }
    }
    */

    println!("\n✅ Example completed successfully!");
    Ok(())
}
