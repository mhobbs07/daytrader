//! Configuration management for the trading platform
//!
//! This module handles application configuration including:
//! - Broker settings (API keys, endpoints)
//! - Risk management parameters
//! - Pattern detection settings
//! - Database configuration
//! - Logging configuration

use crate::risk::types::{ConfidenceMultiplier, PatternPerformanceThresholds, PatternRiskProfile};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Broker configuration
    pub broker: BrokerConfig,

    /// Database configuration
    pub database: DatabaseConfig,

    /// Logging configuration
    pub logging: LoggingConfig,

    /// Risk management configuration
    pub risk: RiskConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerConfig {
    pub provider: String,
    pub is_paper: bool,
    /// Credentials come from the environment (`ALPACA_API_KEY`), never from
    /// the config file, so these fields default to empty when absent.
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub api_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: PathBuf,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file_path: PathBuf,
}

/// Validated risk configuration.
///
/// There is deliberately no `Default`: an all-zero risk config is a *disabled*
/// risk config. Production code obtains one by deserializing (which routes
/// through [`RawRiskConfig`] and validation) or via [`RiskConfig::try_from`].
/// Tests may use [`RiskConfig::test_fixture`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "RawRiskConfig")]
pub struct RiskConfig {
    // Daily loss limits
    pub max_daily_loss_dollars: f64,
    pub max_daily_loss_percent: f64,
    pub halt_trading_on_limit: bool,

    // Position sizing
    pub max_position_size_dollars: f64,
    pub max_position_size_percent: f64,
    pub max_concurrent_positions: u32,

    // PDT protection
    pub enforce_pdt_rule: bool,
    pub pdt_min_equity: f64,
    pub max_day_trades_under_pdt: u32,

    // Pattern-specific rules
    pub max_trades_per_pattern_per_day: u32,
    pub stop_pattern_after_consecutive_losses: u32,
    pub required_win_rate_for_auto: f64,
    pub min_pattern_sample_size: u32,

    // Account safeguards
    pub min_account_balance: f64,
    pub emergency_liquidate_threshold: f64,

    // Pattern-specific risk management
    pub pattern_profiles: HashMap<String, PatternRiskProfile>,
    pub confidence_multipliers: Vec<ConfidenceMultiplier>,
    pub pattern_performance_thresholds: PatternPerformanceThresholds,
}

/// Risk configuration exactly as parsed from TOML, before validation.
///
/// No `Default`, and unknown keys are rejected: a typo'd limit is a startup
/// error rather than a silently-zero limit. Every limit field is required.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRiskConfig {
    pub max_daily_loss_dollars: f64,
    pub max_daily_loss_percent: f64,
    pub halt_trading_on_limit: bool,
    pub max_position_size_dollars: f64,
    pub max_position_size_percent: f64,
    pub max_concurrent_positions: u32,
    pub enforce_pdt_rule: bool,
    pub pdt_min_equity: f64,
    pub max_day_trades_under_pdt: u32,
    pub max_trades_per_pattern_per_day: u32,
    pub stop_pattern_after_consecutive_losses: u32,
    pub required_win_rate_for_auto: f64,
    pub min_pattern_sample_size: u32,
    pub min_account_balance: f64,
    pub emergency_liquidate_threshold: f64,
    #[serde(default)]
    pub pattern_profiles: HashMap<String, PatternRiskProfile>,
    #[serde(default)]
    pub confidence_multipliers: Vec<ConfidenceMultiplier>,
    #[serde(default)]
    pub pattern_performance_thresholds: PatternPerformanceThresholds,
}

/// Why a [`RawRiskConfig`] failed validation.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RiskConfigError {
    /// A numeric field was NaN or infinite.
    #[error("risk config field `{field}` must be finite, got {value}")]
    NotFinite { field: &'static str, value: f64 },

    /// A numeric field is finite but too large for `Decimal`, which every
    /// limit is converted to downstream.
    #[error("risk config field `{field}` is out of range, got {value}")]
    NotRepresentable {
        /// Name of the offending field.
        field: &'static str,
        /// The value that could not be converted.
        value: f64,
    },

    /// A dollar amount that must be strictly positive was zero or negative.
    #[error("risk config field `{field}` must be > 0, got {value}")]
    NotPositive { field: &'static str, value: f64 },

    /// A dollar amount that must be non-negative was negative.
    #[error("risk config field `{field}` must be >= 0, got {value}")]
    Negative { field: &'static str, value: f64 },

    /// A percentage was outside `(0, 100]`.
    #[error("risk config field `{field}` must be in (0, 100], got {value}")]
    PercentOutOfRange { field: &'static str, value: f64 },

    /// A count that must be at least one was zero.
    #[error("risk config field `{field}` must be >= 1")]
    ZeroCount { field: &'static str },

    /// The emergency liquidation threshold exceeds the minimum account balance.
    #[error(
        "emergency_liquidate_threshold ({emergency}) must not exceed \
         min_account_balance ({min_balance})"
    )]
    EmergencyAboveMinBalance { emergency: f64, min_balance: f64 },

    /// A pattern profile field was invalid.
    #[error("pattern profile `{pattern}`: {source}")]
    PatternProfile {
        pattern: String,
        #[source]
        source: Box<RiskConfigError>,
    },

    /// A confidence multiplier band was invalid.
    #[error("confidence_multipliers[{index}]: {source}")]
    ConfidenceBand {
        index: usize,
        #[source]
        source: Box<RiskConfigError>,
    },

    /// A multiplier was NaN, non-positive or above [`MAX_MULTIPLIER`].
    #[error("risk config field `{field}` must be in (0, 10], got {value}")]
    MultiplierOutOfRange { field: &'static str, value: f64 },

    /// A confidence score was outside `[0, 1]`.
    #[error("risk config field `{field}` must be in [0, 1], got {value}")]
    ConfidenceOutOfRange { field: &'static str, value: f64 },

    /// A confidence band had `min_confidence >= max_confidence`.
    #[error("confidence band is empty or inverted: min {min} >= max {max}")]
    InvertedConfidenceBand { min: f64, max: f64 },

    /// Two confidence bands overlap.
    #[error("confidence bands overlap: [{a_min}, {a_max}) and [{b_min}, {b_max})")]
    OverlappingConfidenceBands {
        a_min: f64,
        a_max: f64,
        b_min: f64,
        b_max: f64,
    },

    /// A performance-threshold percentage was outside `[0, 100]`.
    #[error("pattern_performance_thresholds.{field} must be in [0, 100], got {value}")]
    ThresholdPercentOutOfRange { field: &'static str, value: String },
}

/// Upper bound for any risk/size/limit multiplier. Guards against a typo'd
/// value silently lifting a limit by orders of magnitude.
pub const MAX_MULTIPLIER: f64 = 10.0;

fn multiplier(field: &'static str, value: f64) -> Result<f64, RiskConfigError> {
    if value.is_finite() && value > 0.0 && value <= MAX_MULTIPLIER {
        Ok(value)
    } else {
        Err(RiskConfigError::MultiplierOutOfRange { field, value })
    }
}

fn confidence(field: &'static str, value: f64) -> Result<f64, RiskConfigError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(RiskConfigError::ConfidenceOutOfRange { field, value })
    }
}

fn validate_profile(p: &PatternRiskProfile) -> Result<(), RiskConfigError> {
    at_least_one("max_trades_per_day", p.max_trades_per_day.value())?;
    at_least_one(
        "stop_after_consecutive_losses",
        p.stop_after_consecutive_losses.value(),
    )?;
    multiplier(
        "max_position_size_multiplier",
        p.max_position_size_multiplier,
    )?;
    multiplier("risk_multiplier", p.risk_multiplier)?;
    confidence("min_confidence_threshold", p.min_confidence_threshold)?;
    Ok(())
}

fn validate_band(b: &ConfidenceMultiplier) -> Result<(), RiskConfigError> {
    let min = confidence("min_confidence", b.min_confidence)?;
    let max = confidence("max_confidence", b.max_confidence)?;
    if min >= max {
        return Err(RiskConfigError::InvertedConfidenceBand { min, max });
    }
    multiplier("position_size_multiplier", b.position_size_multiplier)?;
    multiplier("trade_limit_multiplier", b.trade_limit_multiplier)?;
    multiplier("risk_limit_multiplier", b.risk_limit_multiplier)?;
    Ok(())
}

fn validate_bands(bands: &[ConfidenceMultiplier]) -> Result<(), RiskConfigError> {
    for (index, b) in bands.iter().enumerate() {
        validate_band(b).map_err(|e| RiskConfigError::ConfidenceBand {
            index,
            source: Box::new(e),
        })?;
    }
    let mut sorted: Vec<&ConfidenceMultiplier> = bands.iter().collect();
    sorted.sort_by(|a, b| a.min_confidence.total_cmp(&b.min_confidence));
    for pair in sorted.windows(2) {
        if pair[0].max_confidence > pair[1].min_confidence {
            return Err(RiskConfigError::OverlappingConfidenceBands {
                a_min: pair[0].min_confidence,
                a_max: pair[0].max_confidence,
                b_min: pair[1].min_confidence,
                b_max: pair[1].max_confidence,
            });
        }
    }
    Ok(())
}

fn validate_thresholds(t: &PatternPerformanceThresholds) -> Result<(), RiskConfigError> {
    let lo = Decimal::ZERO;
    let hi = Decimal::new(1000, 1);
    for (field, p) in [
        ("min_win_rate_short", t.min_win_rate_short),
        ("min_win_rate_medium", t.min_win_rate_medium),
        ("min_win_rate_long", t.min_win_rate_long),
        ("auto_disable_threshold", t.auto_disable_threshold),
    ] {
        let v = p.value();
        if v < lo || v > hi {
            return Err(RiskConfigError::ThresholdPercentOutOfRange {
                field,
                value: v.to_string(),
            });
        }
    }
    at_least_one("max_consecutive_losses", t.max_consecutive_losses.value())?;
    at_least_one("min_sample_size", t.min_sample_size.value())?;
    Ok(())
}

fn finite(field: &'static str, value: f64) -> Result<f64, RiskConfigError> {
    if !value.is_finite() {
        return Err(RiskConfigError::NotFinite { field, value });
    }
    // Limits are converted with `Decimal::try_from(..)` downstream; reject
    // anything that conversion would fail on here, as a config error.
    if rust_decimal::Decimal::try_from(value).is_err() {
        return Err(RiskConfigError::NotRepresentable { field, value });
    }
    Ok(value)
}

fn positive(field: &'static str, value: f64) -> Result<f64, RiskConfigError> {
    let v = finite(field, value)?;
    if v > 0.0 {
        Ok(v)
    } else {
        Err(RiskConfigError::NotPositive { field, value })
    }
}

fn non_negative(field: &'static str, value: f64) -> Result<f64, RiskConfigError> {
    let v = finite(field, value)?;
    if v >= 0.0 {
        Ok(v)
    } else {
        Err(RiskConfigError::Negative { field, value })
    }
}

/// Percent in the half-open range `(0, 100]`.
fn percent(field: &'static str, value: f64) -> Result<f64, RiskConfigError> {
    let v = finite(field, value)?;
    if v > 0.0 && v <= 100.0 {
        Ok(v)
    } else {
        Err(RiskConfigError::PercentOutOfRange { field, value })
    }
}

fn at_least_one(field: &'static str, value: u32) -> Result<u32, RiskConfigError> {
    if value >= 1 {
        Ok(value)
    } else {
        Err(RiskConfigError::ZeroCount { field })
    }
}

impl TryFrom<RawRiskConfig> for RiskConfig {
    type Error = RiskConfigError;

    fn try_from(raw: RawRiskConfig) -> Result<Self, Self::Error> {
        let min_account_balance = non_negative("min_account_balance", raw.min_account_balance)?;
        let emergency = non_negative(
            "emergency_liquidate_threshold",
            raw.emergency_liquidate_threshold,
        )?;
        if emergency > min_account_balance {
            return Err(RiskConfigError::EmergencyAboveMinBalance {
                emergency,
                min_balance: min_account_balance,
            });
        }
        let pdt_min_equity = if raw.enforce_pdt_rule {
            positive("pdt_min_equity", raw.pdt_min_equity)?
        } else {
            non_negative("pdt_min_equity", raw.pdt_min_equity)?
        };

        for (name, profile) in &raw.pattern_profiles {
            validate_profile(profile).map_err(|e| RiskConfigError::PatternProfile {
                pattern: name.clone(),
                source: Box::new(e),
            })?;
        }
        validate_bands(&raw.confidence_multipliers)?;
        validate_thresholds(&raw.pattern_performance_thresholds)?;

        Ok(Self {
            max_daily_loss_dollars: positive("max_daily_loss_dollars", raw.max_daily_loss_dollars)?,
            max_daily_loss_percent: percent("max_daily_loss_percent", raw.max_daily_loss_percent)?,
            halt_trading_on_limit: raw.halt_trading_on_limit,
            max_position_size_dollars: positive(
                "max_position_size_dollars",
                raw.max_position_size_dollars,
            )?,
            max_position_size_percent: percent(
                "max_position_size_percent",
                raw.max_position_size_percent,
            )?,
            max_concurrent_positions: at_least_one(
                "max_concurrent_positions",
                raw.max_concurrent_positions,
            )?,
            enforce_pdt_rule: raw.enforce_pdt_rule,
            pdt_min_equity,
            max_day_trades_under_pdt: raw.max_day_trades_under_pdt,
            max_trades_per_pattern_per_day: at_least_one(
                "max_trades_per_pattern_per_day",
                raw.max_trades_per_pattern_per_day,
            )?,
            stop_pattern_after_consecutive_losses: at_least_one(
                "stop_pattern_after_consecutive_losses",
                raw.stop_pattern_after_consecutive_losses,
            )?,
            required_win_rate_for_auto: percent(
                "required_win_rate_for_auto",
                raw.required_win_rate_for_auto,
            )?,
            min_pattern_sample_size: at_least_one(
                "min_pattern_sample_size",
                raw.min_pattern_sample_size,
            )?,
            min_account_balance,
            emergency_liquidate_threshold: emergency,
            pattern_profiles: raw.pattern_profiles,
            confidence_multipliers: raw.confidence_multipliers,
            pattern_performance_thresholds: raw.pattern_performance_thresholds,
        })
    }
}

impl RiskConfig {
    /// A validated, enabled config for tests, doctests and fixtures.
    ///
    /// Mirrors the limits in `config/default.toml`. Not for production use:
    /// production must load and validate the config file.
    #[doc(hidden)]
    #[must_use]
    pub fn test_fixture() -> Self {
        Self {
            max_daily_loss_dollars: 500.0,
            max_daily_loss_percent: 2.0,
            halt_trading_on_limit: true,
            max_position_size_dollars: 5000.0,
            max_position_size_percent: 10.0,
            max_concurrent_positions: 5,
            enforce_pdt_rule: true,
            pdt_min_equity: 25000.0,
            max_day_trades_under_pdt: 3,
            max_trades_per_pattern_per_day: 5,
            stop_pattern_after_consecutive_losses: 3,
            required_win_rate_for_auto: 60.0,
            min_pattern_sample_size: 20,
            min_account_balance: 1000.0,
            emergency_liquidate_threshold: 500.0,
            pattern_profiles: HashMap::new(),
            confidence_multipliers: Vec::new(),
            pattern_performance_thresholds: PatternPerformanceThresholds::default(),
        }
    }
}

impl AppConfig {
    /// Load configuration from file
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self, config::ConfigError> {
        let settings = config::Config::builder()
            .add_source(config::File::from(path.as_ref()))
            .build()?;

        settings.try_deserialize()
    }

    /// Parse and validate the `config/default.toml` embedded at build time.
    ///
    /// # Errors
    /// Returns an error if the embedded file fails to parse or validate.
    pub fn embedded_default() -> Result<Self, config::ConfigError> {
        config::Config::builder()
            .add_source(config::File::from_str(
                include_str!("../../config/default.toml"),
                config::FileFormat::Toml,
            ))
            .build()?
            .try_deserialize()
    }

    /// Load configuration from default location
    pub fn load_default() -> Result<Self, config::ConfigError> {
        Self::from_file("config/default.toml")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn raw() -> RawRiskConfig {
        let c = RiskConfig::test_fixture();
        RawRiskConfig {
            max_daily_loss_dollars: c.max_daily_loss_dollars,
            max_daily_loss_percent: c.max_daily_loss_percent,
            halt_trading_on_limit: c.halt_trading_on_limit,
            max_position_size_dollars: c.max_position_size_dollars,
            max_position_size_percent: c.max_position_size_percent,
            max_concurrent_positions: c.max_concurrent_positions,
            enforce_pdt_rule: c.enforce_pdt_rule,
            pdt_min_equity: c.pdt_min_equity,
            max_day_trades_under_pdt: c.max_day_trades_under_pdt,
            max_trades_per_pattern_per_day: c.max_trades_per_pattern_per_day,
            stop_pattern_after_consecutive_losses: c.stop_pattern_after_consecutive_losses,
            required_win_rate_for_auto: c.required_win_rate_for_auto,
            min_pattern_sample_size: c.min_pattern_sample_size,
            min_account_balance: c.min_account_balance,
            emergency_liquidate_threshold: c.emergency_liquidate_threshold,
            pattern_profiles: HashMap::new(),
            confidence_multipliers: Vec::new(),
            pattern_performance_thresholds: PatternPerformanceThresholds::default(),
        }
    }

    #[test]
    fn test_embedded_default_toml_parses_and_is_enabled() {
        let config = AppConfig::embedded_default().unwrap();
        assert_eq!(config.risk.max_daily_loss_dollars, 500.0);
        assert!(config.risk.enforce_pdt_rule);
        assert_eq!(config.risk.max_concurrent_positions, 5);
    }

    fn profile() -> PatternRiskProfile {
        PatternRiskProfile::moderate()
    }

    fn band(min: f64, max: f64) -> ConfidenceMultiplier {
        ConfidenceMultiplier {
            min_confidence: min,
            max_confidence: max,
            position_size_multiplier: 1.0,
            trade_limit_multiplier: 1.0,
            risk_limit_multiplier: 1.0,
        }
    }

    fn with_profile(p: PatternRiskProfile) -> Result<RiskConfig, RiskConfigError> {
        let mut r = raw();
        r.pattern_profiles.insert("hammer".into(), p);
        RiskConfig::try_from(r)
    }

    fn with_bands(b: Vec<ConfidenceMultiplier>) -> Result<RiskConfig, RiskConfigError> {
        let mut r = raw();
        r.confidence_multipliers = b;
        RiskConfig::try_from(r)
    }

    fn profile_err(e: RiskConfigError) -> RiskConfigError {
        match e {
            RiskConfigError::PatternProfile { source, .. } => *source,
            other => other,
        }
    }

    fn band_err(e: RiskConfigError) -> RiskConfigError {
        match e {
            RiskConfigError::ConfidenceBand { source, .. } => *source,
            other => other,
        }
    }

    #[test]
    fn test_valid_profile_and_bands_accepted() {
        assert!(with_profile(profile()).is_ok());
        assert!(with_bands(vec![band(0.0, 0.5), band(0.5, 1.0)]).is_ok());
    }

    #[test]
    fn test_profile_nan_risk_multiplier_rejected() {
        let mut p = profile();
        p.risk_multiplier = f64::NAN;
        assert!(matches!(
            profile_err(with_profile(p).unwrap_err()),
            RiskConfigError::MultiplierOutOfRange {
                field: "risk_multiplier",
                ..
            }
        ));
    }

    #[test]
    fn test_profile_negative_risk_multiplier_rejected() {
        let mut p = profile();
        p.risk_multiplier = -0.5;
        assert!(matches!(
            profile_err(with_profile(p).unwrap_err()),
            RiskConfigError::MultiplierOutOfRange {
                field: "risk_multiplier",
                ..
            }
        ));
    }

    #[test]
    fn test_profile_huge_position_multiplier_rejected() {
        let mut p = profile();
        p.max_position_size_multiplier = 1.0e9;
        assert!(matches!(
            profile_err(with_profile(p).unwrap_err()),
            RiskConfigError::MultiplierOutOfRange {
                field: "max_position_size_multiplier",
                ..
            }
        ));
    }

    #[test]
    fn test_profile_confidence_threshold_out_of_range_rejected() {
        let mut p = profile();
        p.min_confidence_threshold = 1.5;
        assert!(matches!(
            profile_err(with_profile(p).unwrap_err()),
            RiskConfigError::ConfidenceOutOfRange {
                field: "min_confidence_threshold",
                ..
            }
        ));
    }

    #[test]
    fn test_profile_zero_max_trades_rejected() {
        let mut p = profile();
        p.max_trades_per_day = crate::risk::types::Count::new(0).unwrap();
        assert_eq!(
            profile_err(with_profile(p).unwrap_err()),
            RiskConfigError::ZeroCount {
                field: "max_trades_per_day"
            }
        );
    }

    #[test]
    fn test_profile_zero_stop_losses_rejected() {
        let mut p = profile();
        p.stop_after_consecutive_losses = crate::risk::types::Count::new(0).unwrap();
        assert_eq!(
            profile_err(with_profile(p).unwrap_err()),
            RiskConfigError::ZeroCount {
                field: "stop_after_consecutive_losses"
            }
        );
    }

    #[test]
    fn test_band_inverted_rejected() {
        assert!(matches!(
            band_err(with_bands(vec![band(0.6, 0.4)]).unwrap_err()),
            RiskConfigError::InvertedConfidenceBand { .. }
        ));
    }

    #[test]
    fn test_band_confidence_out_of_range_rejected() {
        assert!(matches!(
            band_err(with_bands(vec![band(0.0, 1.5)]).unwrap_err()),
            RiskConfigError::ConfidenceOutOfRange {
                field: "max_confidence",
                ..
            }
        ));
        assert!(matches!(
            band_err(with_bands(vec![band(-0.1, 0.5)]).unwrap_err()),
            RiskConfigError::ConfidenceOutOfRange {
                field: "min_confidence",
                ..
            }
        ));
    }

    #[test]
    fn test_band_bad_multipliers_rejected() {
        for (i, field) in [
            "position_size_multiplier",
            "trade_limit_multiplier",
            "risk_limit_multiplier",
        ]
        .into_iter()
        .enumerate()
        {
            let mut b = band(0.0, 1.0);
            match i {
                0 => b.position_size_multiplier = f64::NAN,
                1 => b.trade_limit_multiplier = 0.0,
                _ => b.risk_limit_multiplier = 100.0,
            }
            assert!(matches!(
                band_err(with_bands(vec![b]).unwrap_err()),
                RiskConfigError::MultiplierOutOfRange { field: f, .. } if f == field
            ));
        }
    }

    #[test]
    fn test_overlapping_bands_rejected() {
        assert!(matches!(
            with_bands(vec![band(0.5, 1.0), band(0.0, 0.6)]).unwrap_err(),
            RiskConfigError::OverlappingConfidenceBands { .. }
        ));
    }

    fn with_thresholds(
        f: impl FnOnce(&mut PatternPerformanceThresholds),
    ) -> Result<RiskConfig, RiskConfigError> {
        let mut r = raw();
        f(&mut r.pattern_performance_thresholds);
        RiskConfig::try_from(r)
    }

    #[test]
    fn test_thresholds_out_of_range_percentage_rejected() {
        // Percentage's derived Deserialize bypasses its range check.
        let bad = serde_json::from_str::<crate::risk::types::Percentage>("\"150.0\"").unwrap();
        for field in [
            "min_win_rate_short",
            "min_win_rate_medium",
            "min_win_rate_long",
            "auto_disable_threshold",
        ] {
            let err = with_thresholds(|t| match field {
                "min_win_rate_short" => t.min_win_rate_short = bad,
                "min_win_rate_medium" => t.min_win_rate_medium = bad,
                "min_win_rate_long" => t.min_win_rate_long = bad,
                _ => t.auto_disable_threshold = bad,
            })
            .unwrap_err();
            assert!(matches!(
                err,
                RiskConfigError::ThresholdPercentOutOfRange { field: f, .. } if f == field
            ));
        }
    }

    #[test]
    fn test_thresholds_zero_counts_rejected() {
        let zero = crate::risk::types::Count::new(0).unwrap();
        assert_eq!(
            with_thresholds(|t| t.max_consecutive_losses = zero).unwrap_err(),
            RiskConfigError::ZeroCount {
                field: "max_consecutive_losses"
            }
        );
        assert_eq!(
            with_thresholds(|t| t.min_sample_size = zero).unwrap_err(),
            RiskConfigError::ZeroCount {
                field: "min_sample_size"
            }
        );
    }

    #[test]
    fn test_fixture_passes_validation() {
        assert!(RiskConfig::try_from(raw()).is_ok());
    }

    #[test]
    fn test_zero_daily_loss_dollars_rejected() {
        let mut r = raw();
        r.max_daily_loss_dollars = 0.0;
        assert_eq!(
            RiskConfig::try_from(r).unwrap_err(),
            RiskConfigError::NotPositive {
                field: "max_daily_loss_dollars",
                value: 0.0
            }
        );
    }

    #[test]
    fn test_zero_position_size_dollars_rejected() {
        let mut r = raw();
        r.max_position_size_dollars = -1.0;
        assert!(matches!(
            RiskConfig::try_from(r),
            Err(RiskConfigError::NotPositive {
                field: "max_position_size_dollars",
                ..
            })
        ));
    }

    #[test]
    fn test_position_size_dollars_beyond_decimal_range_rejected() {
        // Finite and positive, but `Decimal::try_from` fails on it; before the
        // check this config was accepted and later panicked in
        // `PositionSizeLimits::from_config`.
        let mut r = raw();
        r.max_position_size_dollars = 1e100;
        assert!(matches!(
            RiskConfig::try_from(r),
            Err(RiskConfigError::NotRepresentable {
                field: "max_position_size_dollars",
                ..
            })
        ));
    }

    #[test]
    fn test_percent_over_100_rejected() {
        let mut r = raw();
        r.max_position_size_percent = 100.5;
        assert!(matches!(
            RiskConfig::try_from(r),
            Err(RiskConfigError::PercentOutOfRange {
                field: "max_position_size_percent",
                ..
            })
        ));
        let mut r = raw();
        r.max_daily_loss_percent = 0.0;
        assert!(matches!(
            RiskConfig::try_from(r),
            Err(RiskConfigError::PercentOutOfRange {
                field: "max_daily_loss_percent",
                ..
            })
        ));
        let mut r = raw();
        r.required_win_rate_for_auto = 150.0;
        assert!(matches!(
            RiskConfig::try_from(r),
            Err(RiskConfigError::PercentOutOfRange {
                field: "required_win_rate_for_auto",
                ..
            })
        ));
    }

    #[test]
    fn test_nan_rejected() {
        let mut r = raw();
        r.min_account_balance = f64::NAN;
        assert!(matches!(
            RiskConfig::try_from(r),
            Err(RiskConfigError::NotFinite {
                field: "min_account_balance",
                ..
            })
        ));
    }

    #[test]
    fn test_zero_counts_rejected() {
        let mut r = raw();
        r.max_concurrent_positions = 0;
        assert_eq!(
            RiskConfig::try_from(r).unwrap_err(),
            RiskConfigError::ZeroCount {
                field: "max_concurrent_positions"
            }
        );
        let mut r = raw();
        r.max_trades_per_pattern_per_day = 0;
        assert!(matches!(
            RiskConfig::try_from(r),
            Err(RiskConfigError::ZeroCount { .. })
        ));
        let mut r = raw();
        r.stop_pattern_after_consecutive_losses = 0;
        assert!(matches!(
            RiskConfig::try_from(r),
            Err(RiskConfigError::ZeroCount { .. })
        ));
        let mut r = raw();
        r.min_pattern_sample_size = 0;
        assert!(matches!(
            RiskConfig::try_from(r),
            Err(RiskConfigError::ZeroCount { .. })
        ));
    }

    #[test]
    fn test_pdt_equity_must_be_positive_when_enforced() {
        let mut r = raw();
        r.pdt_min_equity = 0.0;
        assert!(matches!(
            RiskConfig::try_from(r),
            Err(RiskConfigError::NotPositive {
                field: "pdt_min_equity",
                ..
            })
        ));
        let mut r = raw();
        r.enforce_pdt_rule = false;
        r.pdt_min_equity = 0.0;
        assert!(RiskConfig::try_from(r).is_ok());
    }

    #[test]
    fn test_emergency_above_min_balance_rejected() {
        let mut r = raw();
        r.emergency_liquidate_threshold = 2000.0;
        assert!(matches!(
            RiskConfig::try_from(r),
            Err(RiskConfigError::EmergencyAboveMinBalance { .. })
        ));
    }

    #[test]
    fn test_unknown_field_rejected_by_deserializer() {
        let toml_str = r#"
max_daily_loss_dollar = 500.0
"#;
        let result: Result<RawRiskConfig, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_limit_rejected_by_deserializer() {
        let result: Result<RiskConfig, _> = toml::from_str("max_daily_loss_dollars = 500.0");
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_risk_config_validates() {
        let result: Result<RiskConfig, _> = toml::from_str(
            r#"
max_daily_loss_dollars = 0.0
max_daily_loss_percent = 2.0
halt_trading_on_limit = true
max_position_size_dollars = 5000.0
max_position_size_percent = 10.0
max_concurrent_positions = 5
enforce_pdt_rule = true
pdt_min_equity = 25000.0
max_day_trades_under_pdt = 3
max_trades_per_pattern_per_day = 5
stop_pattern_after_consecutive_losses = 3
required_win_rate_for_auto = 60.0
min_pattern_sample_size = 20
min_account_balance = 1000.0
emergency_liquidate_threshold = 500.0
"#,
        );
        let err = result.unwrap_err().to_string();
        assert!(err.contains("max_daily_loss_dollars"), "{err}");
    }

    proptest! {
        #[test]
        fn prop_percent_accepted_iff_in_range(p in -50.0f64..200.0) {
            let mut r = raw();
            r.max_position_size_percent = p;
            let ok = RiskConfig::try_from(r).is_ok();
            prop_assert_eq!(ok, p > 0.0 && p <= 100.0);
        }

        #[test]
        fn prop_loss_dollars_accepted_iff_positive(d in -1.0e6f64..1.0e6) {
            let mut r = raw();
            r.max_daily_loss_dollars = d;
            prop_assert_eq!(RiskConfig::try_from(r).is_ok(), d > 0.0);
        }

        #[test]
        fn prop_emergency_threshold_bounded_by_min_balance(
            bal in 0.0f64..1.0e6, em in 0.0f64..1.0e6
        ) {
            let mut r = raw();
            r.min_account_balance = bal;
            r.emergency_liquidate_threshold = em;
            prop_assert_eq!(RiskConfig::try_from(r).is_ok(), em <= bal);
        }
    }
}
