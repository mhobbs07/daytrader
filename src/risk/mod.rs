pub mod engine;
pub mod limits;
pub mod pattern_limits;
pub mod pattern_stats;
pub mod pdt;
pub mod position_sizing;
pub mod position_tracker;
pub mod rules;
pub mod types;

// Re-export main types for convenience
pub use engine::RiskEngine;
pub use types::*;

// Re-export config types that are defined elsewhere
pub use crate::config::RiskConfig;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::bus::EventBus;
    use rust_decimal::Decimal;
    use std::sync::Arc;

    #[test]
    fn test_risk_engine_construction() {
        // Verify RiskEngine can be constructed and exports are accessible
        let config = RiskConfig::test_fixture();
        let event_bus = Arc::new(EventBus::new(100));
        let account_id = AccountId::new("TEST_ACCOUNT");
        let initial_equity = Decimal::from(100_000);

        let engine = RiskEngine::new(config, event_bus, account_id, initial_equity);

        // Verify the engine was successfully created by checking it exists
        let _ = &engine;
    }
}
