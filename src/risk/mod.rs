pub mod engine;
pub mod limits;
pub mod pattern_limits;
pub mod pattern_stats;
pub mod pdt;
pub mod position_sizing;
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

    #[test]
    fn test_module_compilation() {
        // Basic compilation test - will be expanded as implementation progresses
        assert!(true);
    }
}
