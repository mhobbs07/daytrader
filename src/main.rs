//! Day Trading Platform - Main Entry Point

use daytrader::{
    Result,
    utils::{init_error_logging, init_metrics_collection},
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize structured error logging
    init_error_logging()?;

    // Initialize metrics collection
    init_metrics_collection().await;

    tracing::info!("Day Trading Platform starting...");
    tracing::info!("Phase 0: Foundation established");
    tracing::info!("Phase 3: Error Context Enhancements implemented");

    // TODO: Initialize application in Phase 1

    Ok(())
}
