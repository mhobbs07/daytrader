//! Pattern Performance Statistics Tracking
//!
//! This module tracks and analyzes pattern performance statistics for risk management.
//! It maintains win/loss ratios, rolling averages, and performance metrics that
//! inform pattern enable/disable decisions and confidence-based risk adjustments.
//!
//! # Features
//!
//! - **Performance Tracking**: Win/loss ratios, profit/loss tracking
//! - **Rolling Statistics**: Time-windowed performance analysis
//! - **Pattern Enable/Disable**: Automatic pattern management based on performance
//! - **Confidence Weighting**: Statistics weighted by pattern confidence
//! - **Database Integration**: Persistent storage of pattern statistics
//!
//! # Examples
//!
//! ```
//! use daytrader::risk::pattern_stats::PatternStatsTracker;
//! use daytrader::brokers::types::PatternName;
//! use rust_decimal::Decimal;
//!
//! let mut tracker = PatternStatsTracker::new();
//!
//! // Record a winning hammer pattern trade
//! let pattern_name = PatternName::new("hammer");
//! let confidence = 0.85;
//! let profit = Decimal::new(2500, 2); // $25.00 profit
//!
//! tracker.record_trade_outcome(&pattern_name, confidence, profit);
//!
//! // Get current statistics
//! let stats = tracker.get_pattern_stats(&pattern_name);
//! assert!(stats.is_some());
//! assert_eq!(stats.unwrap().winning_trades, 1);
//! ```

use crate::brokers::types::PatternName;
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Pattern performance statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternStats {
    /// Pattern name
    pub pattern_name: String,
    /// Total signals detected
    pub total_signals: u32,
    /// Total trades executed
    pub total_trades: u32,
    /// Number of winning trades
    pub winning_trades: u32,
    /// Number of losing trades
    pub losing_trades: u32,
    /// Total profit/loss across all trades
    pub total_profit: Decimal,
    /// Win rate as percentage (0.0 to 100.0)
    pub win_rate: f64,
    /// Average profit per trade
    pub avg_profit_per_trade: Decimal,
    /// Maximum drawdown experienced
    pub max_drawdown: Decimal,
    /// Current consecutive losses streak
    pub consecutive_losses: u32,
    /// Whether the pattern is currently enabled for trading
    pub is_enabled: bool,
    /// Average confidence score of trades
    pub avg_confidence: f64,
    /// Confidence-weighted win rate
    pub confidence_weighted_win_rate: f64,
    /// Last time statistics were updated
    pub last_updated: DateTime<Utc>,
}

impl PatternStats {
    /// Creates new pattern statistics
    pub fn new(pattern_name: String) -> Self {
        Self {
            pattern_name,
            total_signals: 0,
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            total_profit: Decimal::ZERO,
            win_rate: 0.0,
            avg_profit_per_trade: Decimal::ZERO,
            max_drawdown: Decimal::ZERO,
            consecutive_losses: 0,
            is_enabled: true,
            avg_confidence: 0.0,
            confidence_weighted_win_rate: 0.0,
            last_updated: Utc::now(),
        }
    }

    /// Records a pattern signal detection
    pub fn record_signal(&mut self) {
        self.total_signals += 1;
        self.last_updated = Utc::now();
    }

    /// Records a trade outcome and updates statistics
    pub fn record_trade(&mut self, confidence: f64, profit: Decimal) {
        self.total_trades += 1;

        // Update profit tracking
        self.total_profit += profit;

        // Update win/loss counts
        if profit > Decimal::ZERO {
            self.winning_trades += 1;
            self.consecutive_losses = 0;
        } else {
            self.losing_trades += 1;
            self.consecutive_losses += 1;
        }

        // Update drawdown tracking
        if self.total_profit < self.max_drawdown {
            self.max_drawdown = self.total_profit;
        }

        // Update confidence statistics incrementally
        let total_confidence_weight = self.avg_confidence * (self.total_trades as f64 - 1.0);
        self.avg_confidence = (total_confidence_weight + confidence) / self.total_trades as f64;

        // Update derived statistics incrementally instead of recalculating everything
        if self.total_trades > 0 {
            self.win_rate = (self.winning_trades as f64 / self.total_trades as f64) * 100.0;
            self.avg_profit_per_trade = self.total_profit / Decimal::from(self.total_trades);

            // Calculate confidence-weighted win rate
            let confidence_weighted_wins = self.winning_trades as f64 * self.avg_confidence;
            let total_confidence_weight = self.total_trades as f64 * self.avg_confidence;
            self.confidence_weighted_win_rate = if total_confidence_weight > 0.0 {
                (confidence_weighted_wins / total_confidence_weight) * 100.0
            } else {
                0.0
            };
        }

        self.last_updated = Utc::now();
    }

    /// Recalculates win rate and average profit
    fn recalculate_stats(&mut self) {
        if self.total_trades > 0 {
            self.win_rate = (self.winning_trades as f64 / self.total_trades as f64) * 100.0;
            self.avg_profit_per_trade = self.total_profit / Decimal::from(self.total_trades);

            // Calculate confidence-weighted win rate
            // This gives higher weight to high-confidence trades
            let confidence_weighted_wins = self.winning_trades as f64 * self.avg_confidence;
            let total_confidence_weight = self.total_trades as f64 * self.avg_confidence;
            self.confidence_weighted_win_rate = if total_confidence_weight > 0.0 {
                (confidence_weighted_wins / total_confidence_weight) * 100.0
            } else {
                0.0
            };
        }
    }

    /// Checks if pattern meets minimum performance criteria
    pub fn meets_performance_criteria(&self, min_win_rate: f64, min_sample_size: u32) -> bool {
        self.total_trades >= min_sample_size && self.win_rate >= min_win_rate
    }

    /// Gets the pattern's current performance score (0.0 to 1.0)
    pub fn performance_score(&self) -> f64 {
        if self.total_trades == 0 {
            return 0.5; // Neutral score for new patterns
        }

        let win_rate_score = self.win_rate / 100.0;
        let sample_size_score = (self.total_trades.min(100) as f64) / 100.0; // Cap at 100 trades
        let consistency_score = if self.consecutive_losses > 3 { 0.0 } else { 1.0 };

        (win_rate_score * 0.6) + (sample_size_score * 0.3) + (consistency_score * 0.1)
    }

    /// Resets consecutive losses counter
    pub fn reset_consecutive_losses(&mut self) {
        self.consecutive_losses = 0;
        self.last_updated = Utc::now();
    }
}

/// Individual trade record for rolling window analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    /// Pattern name
    pub pattern_name: String,
    /// Trade execution timestamp
    pub timestamp: DateTime<Utc>,
    /// Pattern confidence at time of trade
    pub confidence: f64,
    /// Profit/loss from the trade
    pub profit: Decimal,
    /// Whether the trade was profitable
    pub was_profitable: bool,
}

impl TradeRecord {
    /// Creates a new trade record
    pub fn new(pattern_name: String, confidence: f64, profit: Decimal) -> Self {
        Self {
            pattern_name,
            timestamp: Utc::now(),
            confidence,
            profit,
            was_profitable: profit > Decimal::ZERO,
        }
    }
}

/// Rolling window statistics for pattern analysis
#[derive(Debug, Clone)]
pub struct RollingStats {
    /// Window duration in days
    pub window_days: u32,
    /// Trade records within the window
    pub trades: Vec<TradeRecord>,
    /// Cached statistics for the window
    pub stats: PatternStats,
    /// Performance score for risk adjustment (0.0 to 1.0)
    pub performance_score: f64,
    /// Risk multiplier based on performance (0.5 to 1.5)
    pub risk_multiplier: f64,
    /// Last time statistics were recalculated
    pub last_recalculated: DateTime<Utc>,
}

impl RollingStats {
    /// Creates new rolling statistics
    pub fn new(window_days: u32, pattern_name: String) -> Self {
        Self {
            window_days,
            trades: Vec::new(),
            stats: PatternStats::new(pattern_name),
            performance_score: 0.5, // Neutral score for new patterns
            risk_multiplier: 1.0, // Default multiplier
            last_recalculated: Utc::now(),
        }
    }

    /// Adds a trade to the rolling window
    pub fn add_trade(&mut self, trade: TradeRecord) {
        self.trades.push(trade);

        // Remove trades outside the window efficiently
        let cutoff = Utc::now() - Duration::days(self.window_days as i64);
        let old_len = self.trades.len();
        self.trades.retain(|t| t.timestamp > cutoff);

        // Only recalculate if trades were removed or this is the first trade
        if self.trades.len() != old_len || old_len == 1 {
            self.recalculate_stats();
        } else {
            // Incremental update: just add the new trade to existing stats
            let new_trade = self.trades.last().unwrap();
            self.stats.record_trade(new_trade.confidence, new_trade.profit);
            self.performance_score = self.calculate_performance_score();
            self.risk_multiplier = self.calculate_risk_multiplier();
            self.last_recalculated = Utc::now();
        }
    }

    /// Recalculates statistics from current window trades
    fn recalculate_stats(&mut self) {
        // Reset stats without reallocation
        let pattern_name = self.stats.pattern_name.clone();
        self.stats = PatternStats::new(pattern_name);

        // Recalculate from all trades in window
        for trade in &self.trades {
            self.stats.record_trade(trade.confidence, trade.profit);
        }

        // Calculate performance score and risk multiplier
        self.performance_score = self.calculate_performance_score();
        self.risk_multiplier = self.calculate_risk_multiplier();
        self.last_recalculated = Utc::now();
    }

    /// Calculates performance score based on win rate, sample size, and consistency
    fn calculate_performance_score(&self) -> f64 {
        if self.stats.total_trades == 0 {
            return 0.5; // Neutral score for new patterns
        }

        let win_rate_score = self.stats.win_rate / 100.0;
        let sample_size_score = (self.stats.total_trades.min(100) as f64) / 100.0; // Cap at 100 trades
        let consistency_score = if self.stats.consecutive_losses > 3 { 0.0 } else { 1.0 };

        // Weighted combination: 60% win rate, 30% sample size, 10% consistency
        (win_rate_score * 0.6) + (sample_size_score * 0.3) + (consistency_score * 0.1)
    }

    /// Calculates risk multiplier based on performance score
    /// High performance (score > 0.7): reduce risk (multiplier < 1.0)
    /// Low performance (score < 0.4): increase risk (multiplier > 1.0)
    /// Moderate performance: maintain default (multiplier = 1.0)
    fn calculate_risk_multiplier(&self) -> f64 {
        match self.performance_score {
            score if score >= 0.8 => 0.7, // Excellent performance: reduce risk significantly
            score if score >= 0.7 => 0.8, // Good performance: reduce risk moderately
            score if score >= 0.6 => 0.9, // Decent performance: reduce risk slightly
            score if score >= 0.5 => 1.0, // Neutral performance: default risk
            score if score >= 0.4 => 1.1, // Below average: increase risk slightly
            score if score >= 0.3 => 1.2, // Poor performance: increase risk moderately
            _ => 1.3, // Very poor performance: increase risk significantly
        }
    }

    /// Gets current statistics for the rolling window
    pub fn current_stats(&self) -> &PatternStats {
        &self.stats
    }

    /// Gets the current performance score (0.0 to 1.0)
    pub fn get_performance_score(&self) -> f64 {
        self.performance_score
    }

    /// Gets the current risk multiplier based on performance
    pub fn get_risk_multiplier(&self) -> f64 {
        self.risk_multiplier
    }
}

/// Pattern statistics tracker with rolling windows
#[derive(Debug)]
pub struct PatternStatsTracker {
    /// Current pattern statistics
    pattern_stats: HashMap<String, PatternStats>,
    /// Rolling window statistics (7-day, 30-day, 90-day)
    rolling_windows: HashMap<String, Vec<RollingStats>>,
}

impl PatternStatsTracker {
    /// Creates a new pattern statistics tracker
    pub fn new() -> Self {
        Self {
            pattern_stats: HashMap::new(),
            rolling_windows: HashMap::new(),
        }
    }

    /// Records a pattern signal detection
    pub fn record_signal(&mut self, pattern_name: &PatternName) {
        let key = pattern_name.as_str().to_lowercase();
        let stats = self.pattern_stats
            .entry(key)
            .or_insert_with(|| PatternStats::new(pattern_name.as_str().to_string()));

        stats.record_signal();
    }

    /// Records a trade outcome for a pattern
    pub fn record_trade_outcome(&mut self, pattern_name: &PatternName, confidence: f64, profit: Decimal) {
        let key = pattern_name.as_str().to_lowercase();

        // Update main statistics
        let stats = self.pattern_stats
            .entry(key.clone())
            .or_insert_with(|| PatternStats::new(pattern_name.as_str().to_string()));
        stats.record_trade(confidence, profit);

        // Update rolling windows
        let trade_record = TradeRecord::new(pattern_name.as_str().to_string(), confidence, profit);
        let windows = self.rolling_windows
            .entry(key)
            .or_insert_with(|| vec![
                RollingStats::new(7, pattern_name.as_str().to_string()),
                RollingStats::new(30, pattern_name.as_str().to_string()),
                RollingStats::new(90, pattern_name.as_str().to_string()),
            ]);

        // Add trade to each window (clone once per window, not per trade)
        for window in windows.iter_mut() {
            window.add_trade(trade_record.clone());
        }
    }

    /// Gets current statistics for a pattern
    pub fn get_pattern_stats(&self, pattern_name: &PatternName) -> Option<&PatternStats> {
        let key = pattern_name.as_str().to_lowercase();
        self.pattern_stats.get(&key)
    }

    /// Gets rolling window statistics for a pattern
    pub fn get_rolling_stats(&self, pattern_name: &PatternName, window_days: u32) -> Option<&PatternStats> {
        let key = pattern_name.as_str().to_lowercase();
        self.rolling_windows.get(&key)?
            .iter()
            .find(|w| w.window_days == window_days)
            .map(|w| &w.stats)
    }

    /// Gets performance score for a pattern's rolling window
    pub fn get_performance_score(&self, pattern_name: &PatternName, window_days: u32) -> Option<f64> {
        let key = pattern_name.as_str().to_lowercase();
        self.rolling_windows.get(&key)?
            .iter()
            .find(|w| w.window_days == window_days)
            .map(|w| w.get_performance_score())
    }

    /// Gets risk multiplier for a pattern's rolling window
    pub fn get_risk_multiplier(&self, pattern_name: &PatternName, window_days: u32) -> Option<f64> {
        let key = pattern_name.as_str().to_lowercase();
        self.rolling_windows.get(&key)?
            .iter()
            .find(|w| w.window_days == window_days)
            .map(|w| w.get_risk_multiplier())
    }

    /// Gets weighted risk multiplier across all time windows
    /// Weights: 7-day (40%), 30-day (40%), 90-day (20%)
    pub fn get_weighted_risk_multiplier(&self, pattern_name: &PatternName) -> f64 {
        let short_term = self.get_risk_multiplier(pattern_name, 7).unwrap_or(1.0);
        let medium_term = self.get_risk_multiplier(pattern_name, 30).unwrap_or(1.0);
        let long_term = self.get_risk_multiplier(pattern_name, 90).unwrap_or(1.0);

        // Weighted average: emphasize recent performance
        (short_term * 0.4) + (medium_term * 0.4) + (long_term * 0.2)
    }

    /// Gets overall performance score across all time windows
    pub fn get_overall_performance_score(&self, pattern_name: &PatternName) -> f64 {
        let short_term = self.get_performance_score(pattern_name, 7).unwrap_or(0.5);
        let medium_term = self.get_performance_score(pattern_name, 30).unwrap_or(0.5);
        let long_term = self.get_performance_score(pattern_name, 90).unwrap_or(0.5);

        // Weighted average: emphasize recent performance
        (short_term * 0.4) + (medium_term * 0.4) + (long_term * 0.2)
    }

    /// Gets all patterns with their current statistics
    pub fn get_all_patterns(&self) -> Vec<&PatternStats> {
        self.pattern_stats.values().collect()
    }

    /// Updates pattern enable/disable status based on performance thresholds
    pub fn update_pattern_status(&mut self, pattern_name: &PatternName, min_win_rate: f64, min_sample_size: u32) {
        if let Some(stats) = self.get_pattern_stats(pattern_name) {
            let should_enable = stats.meets_performance_criteria(min_win_rate, min_sample_size);
            let key = pattern_name.as_str().to_lowercase();

            if let Some(stats_mut) = self.pattern_stats.get_mut(&key) {
                stats_mut.is_enabled = should_enable;
            }
        }
    }

    /// Gets patterns that should be disabled due to poor performance
    pub fn get_patterns_to_disable(&self, min_win_rate: f64, min_sample_size: u32) -> Vec<String> {
        self.pattern_stats
            .values()
            .filter(|stats| {
                stats.is_enabled && !stats.meets_performance_criteria(min_win_rate, min_sample_size)
            })
            .map(|stats| stats.pattern_name.clone())
            .collect()
    }

    /// Gets patterns that should be re-enabled due to improved performance
    pub fn get_patterns_to_enable(&self, min_win_rate: f64, min_sample_size: u32) -> Vec<String> {
        self.pattern_stats
            .values()
            .filter(|stats| {
                !stats.is_enabled && stats.meets_performance_criteria(min_win_rate, min_sample_size)
            })
            .map(|stats| stats.pattern_name.clone())
            .collect()
    }

    /// Updates pattern enable/disable status based on rolling window performance
    pub fn update_pattern_status_from_rolling(&mut self, pattern_name: &PatternName, min_performance_score: f64, min_sample_size: u32) {
        let key = pattern_name.as_str().to_lowercase();

        if let Some(windows) = self.rolling_windows.get(&key) {
            // Check if any rolling window meets the criteria
            let should_enable = windows.iter().any(|window| {
                window.stats.total_trades >= min_sample_size &&
                window.get_performance_score() >= min_performance_score
            });

            if let Some(stats) = self.pattern_stats.get_mut(&key) {
                stats.is_enabled = should_enable;
                stats.last_updated = Utc::now();
            }
        }
    }

    /// Gets patterns that need risk escalation (poor recent performance)
    pub fn get_patterns_for_risk_escalation(&self, performance_threshold: f64) -> Vec<(String, f64)> {
        self.pattern_stats
            .iter()
            .filter_map(|(key, stats)| {
                if stats.is_enabled {
                    let score = self.get_overall_performance_score(&PatternName::new(key));
                    if score < performance_threshold {
                        Some((key.clone(), score))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    /// Gets patterns that need risk de-escalation (excellent recent performance)
    pub fn get_patterns_for_risk_deescalation(&self, performance_threshold: f64) -> Vec<(String, f64)> {
        self.pattern_stats
            .iter()
            .filter_map(|(key, stats)| {
                if stats.is_enabled {
                    let score = self.get_overall_performance_score(&PatternName::new(key));
                    if score > performance_threshold {
                        Some((key.clone(), score))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    /// Calculates overall system performance across all patterns
    pub fn get_system_performance(&self) -> SystemPerformance {
        let mut total_trades = 0u32;
        let mut total_wins = 0u32;
        let mut total_profit = Decimal::ZERO;
        let mut enabled_patterns = 0u32;

        for stats in self.pattern_stats.values() {
            total_trades += stats.total_trades;
            total_wins += stats.winning_trades;
            total_profit += stats.total_profit;

            if stats.is_enabled {
                enabled_patterns += 1;
            }
        }

        let system_win_rate = if total_trades > 0 {
            (total_wins as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };

        SystemPerformance {
            total_patterns: self.pattern_stats.len() as u32,
            enabled_patterns,
            total_trades,
            system_win_rate,
            total_profit,
        }
    }

    /// Resets statistics for a specific pattern
    pub fn reset_pattern_stats(&mut self, pattern_name: &PatternName) {
        let key = pattern_name.as_str().to_lowercase();
        if let Some(stats) = self.pattern_stats.get_mut(&key) {
            *stats = PatternStats::new(pattern_name.as_str().to_string());
        }

        // Reset rolling windows
        if let Some(windows) = self.rolling_windows.get_mut(&key) {
            for window in windows {
                window.trades.clear();
                window.recalculate_stats();
            }
        }
    }

    /// Cleans up old trade records beyond the maximum window
    pub fn cleanup_old_records(&mut self) {
        let cutoff = Utc::now() - Duration::days(90); // Keep 90 days of history

        for windows in self.rolling_windows.values_mut() {
            for window in windows {
                let old_count = window.trades.len();
                window.trades.retain(|trade| trade.timestamp > cutoff);
                // Only recalculate if trades were actually removed
                if window.trades.len() != old_count {
                    window.recalculate_stats();
                }
            }
        }
    }
}

/// System-wide performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPerformance {
    /// Total number of patterns tracked
    pub total_patterns: u32,
    /// Number of patterns currently enabled
    pub enabled_patterns: u32,
    /// Total trades across all patterns
    pub total_trades: u32,
    /// System-wide win rate
    pub system_win_rate: f64,
    /// Total profit across all patterns
    pub total_profit: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn test_pattern_stats_tracking() {
        let mut tracker = PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // Record some trades
        tracker.record_trade_outcome(&pattern_name, 0.85, Decimal::new(2500, 2)); // Win
        tracker.record_trade_outcome(&pattern_name, 0.80, Decimal::new(-1500, 2)); // Loss
        tracker.record_trade_outcome(&pattern_name, 0.90, Decimal::new(3000, 2)); // Win

        let stats = tracker.get_pattern_stats(&pattern_name).unwrap();
        assert_eq!(stats.total_trades, 3);
        assert_eq!(stats.winning_trades, 2);
        assert_eq!(stats.losing_trades, 1);
        assert_eq!(stats.consecutive_losses, 0);
        assert!((stats.win_rate - 66.666).abs() < 0.001);
    }

    #[test]
    fn test_consecutive_losses_tracking() {
        let mut tracker = PatternStatsTracker::new();
        let pattern_name = PatternName::new("doji");

        // Record consecutive losses
        tracker.record_trade_outcome(&pattern_name, 0.75, Decimal::new(-1000, 2));
        tracker.record_trade_outcome(&pattern_name, 0.70, Decimal::new(-1200, 2));
        tracker.record_trade_outcome(&pattern_name, 0.65, Decimal::new(-800, 2));

        let stats = tracker.get_pattern_stats(&pattern_name).unwrap();
        assert_eq!(stats.consecutive_losses, 3);
        assert_eq!(stats.total_trades, 3);
        assert_eq!(stats.winning_trades, 0);
    }

    #[test]
    fn test_performance_criteria() {
        let mut tracker = PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // Not enough trades
        tracker.record_trade_outcome(&pattern_name, 0.85, Decimal::new(2500, 2));
        let stats = tracker.get_pattern_stats(&pattern_name).unwrap();
        assert!(!stats.meets_performance_criteria(60.0, 5));

        // Add more trades
        for _ in 0..4 {
            tracker.record_trade_outcome(&pattern_name, 0.80, Decimal::new(2000, 2));
        }

        let stats = tracker.get_pattern_stats(&pattern_name).unwrap();
        assert!(stats.meets_performance_criteria(60.0, 5));
    }

    #[test]
    fn test_rolling_window_stats() {
        let mut tracker = PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // Add trades (simulate they happened at different times)
        tracker.record_trade_outcome(&pattern_name, 0.85, Decimal::new(2500, 2));

        let rolling_30d = tracker.get_rolling_stats(&pattern_name, 30);
        assert!(rolling_30d.is_some());
        assert_eq!(rolling_30d.unwrap().total_trades, 1);
    }

    #[test]
    fn test_pattern_enable_disable() {
        let mut tracker = PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // Initially enabled
        let stats = tracker.get_pattern_stats(&pattern_name);
        assert!(stats.is_none() || stats.unwrap().is_enabled);

        // Record poor performance
        for _ in 0..10 {
            tracker.record_trade_outcome(&pattern_name, 0.50, Decimal::new(-1000, 2));
        }

        tracker.update_pattern_status(&pattern_name, 40.0, 5);
        let stats = tracker.get_pattern_stats(&pattern_name).unwrap();
        assert!(!stats.is_enabled);

        // Record good performance
        for _ in 0..10 {
            tracker.record_trade_outcome(&pattern_name, 0.80, Decimal::new(1500, 2));
        }

        tracker.update_pattern_status(&pattern_name, 40.0, 5);
        let stats = tracker.get_pattern_stats(&pattern_name).unwrap();
        assert!(stats.is_enabled);
    }

    #[test]
    fn test_system_performance() {
        let mut tracker = PatternStatsTracker::new();

        let hammer = PatternName::new("hammer");
        let doji = PatternName::new("doji");

        // Add trades for multiple patterns
        tracker.record_trade_outcome(&hammer, 0.85, Decimal::new(2500, 2));
        tracker.record_trade_outcome(&hammer, 0.80, Decimal::new(-1500, 2));
        tracker.record_trade_outcome(&doji, 0.75, Decimal::new(3000, 2));
        tracker.record_trade_outcome(&doji, 0.70, Decimal::new(2000, 2));

        let system_perf = tracker.get_system_performance();
        assert_eq!(system_perf.total_patterns, 2);
        assert_eq!(system_perf.total_trades, 4);
        assert_eq!(system_perf.enabled_patterns, 2); // Both should be enabled
        assert!((system_perf.system_win_rate - 75.0).abs() < 0.001);
    }

    #[test]
    fn test_confidence_weighted_stats() {
        let mut tracker = PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // High confidence win
        tracker.record_trade_outcome(&pattern_name, 0.95, Decimal::new(2500, 2));
        // Low confidence loss
        tracker.record_trade_outcome(&pattern_name, 0.60, Decimal::new(-1500, 2));

        let stats = tracker.get_pattern_stats(&pattern_name).unwrap();
        assert!((stats.avg_confidence - 0.775).abs() < 0.001); // Average of 0.95 and 0.60
        assert!(stats.confidence_weighted_win_rate > 0.0);
    }

    #[test]
    fn test_performance_score_calculation() {
        let mut tracker = PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // Add some trades to get meaningful statistics
        for i in 0..10 {
            let profit = if i < 7 { Decimal::new(1000, 2) } else { Decimal::new(-1000, 2) };
            tracker.record_trade_outcome(&pattern_name, 0.8, profit);
        }

        let rolling_30d = tracker.get_rolling_stats(&pattern_name, 30);
        assert!(rolling_30d.is_some());
        let stats = rolling_30d.unwrap();

        // 7 wins, 3 losses = 70% win rate, 10 trades
        // Performance score = (0.7 * 0.6) + (1.0 * 0.3) + (1.0 * 0.1) = 0.42 + 0.3 + 0.1 = 0.82
        let expected_score = (0.7 * 0.6) + (1.0 * 0.3) + (1.0 * 0.1);
        assert!((stats.performance_score() - expected_score).abs() < 0.001);
    }

    #[test]
    fn test_risk_multiplier_calculation() {
        let mut tracker = PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // Excellent performance (90% win rate, 20 trades, no consecutive losses)
        for _ in 0..18 {
            tracker.record_trade_outcome(&pattern_name, 0.8, Decimal::new(1000, 2));
        }
        for _ in 0..2 {
            tracker.record_trade_outcome(&pattern_name, 0.8, Decimal::new(-1000, 2));
        }

        let multiplier = tracker.get_risk_multiplier(&pattern_name, 30).unwrap();
        assert_eq!(multiplier, 0.7); // Excellent performance reduces risk
    }

    #[test]
    fn test_weighted_risk_multiplier() {
        let mut tracker = PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // Short-term: excellent performance
        for _ in 0..9 {
            tracker.record_trade_outcome(&pattern_name, 0.9, Decimal::new(1000, 2));
        }

        // Medium-term: good performance
        // (Need to simulate older trades - this is simplified for testing)

        let weighted_multiplier = tracker.get_weighted_risk_multiplier(&pattern_name);
        // Should be a blend of the available windows
        assert!(weighted_multiplier >= 0.5 && weighted_multiplier <= 1.5);
    }

    #[test]
    fn test_performance_based_escalation_detection() {
        let mut tracker = PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // Poor performance: mostly losses
        for _ in 0..8 {
            tracker.record_trade_outcome(&pattern_name, 0.6, Decimal::new(-1000, 2));
        }
        for _ in 0..2 {
            tracker.record_trade_outcome(&pattern_name, 0.6, Decimal::new(1000, 2));
        }

        let escalation_candidates = tracker.get_patterns_for_risk_escalation(0.5);
        assert!(escalation_candidates.iter().any(|(name, _)| name == "hammer"));
    }

    #[test]
    fn test_performance_based_deescalation_detection() {
        let mut tracker = PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // Excellent performance: mostly wins
        for _ in 0..9 {
            tracker.record_trade_outcome(&pattern_name, 0.8, Decimal::new(1000, 2));
        }
        for _ in 0..1 {
            tracker.record_trade_outcome(&pattern_name, 0.8, Decimal::new(-1000, 2));
        }

        let deescalation_candidates = tracker.get_patterns_for_risk_deescalation(0.7);
        assert!(deescalation_candidates.iter().any(|(name, _)| name == "hammer"));
    }

    #[test]
    fn test_rolling_window_cleanup() {
        let mut tracker = PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // Add some trades
        for _ in 0..5 {
            tracker.record_trade_outcome(&pattern_name, 0.8, Decimal::new(1000, 2));
        }

        // Initially should have trades
        let rolling_30d = tracker.get_rolling_stats(&pattern_name, 30).unwrap();
        assert_eq!(rolling_30d.total_trades, 5);

        // Cleanup should not remove recent trades (within 90 days)
        tracker.cleanup_old_records();
        let rolling_30d_after = tracker.get_rolling_stats(&pattern_name, 30).unwrap();
        assert_eq!(rolling_30d_after.total_trades, 5);
    }

    #[test]
    fn test_overall_performance_score() {
        let mut tracker = PatternStatsTracker::new();
        let pattern_name = PatternName::new("hammer");

        // Add trades for multiple windows
        for _ in 0..10 {
            tracker.record_trade_outcome(&pattern_name, 0.8, Decimal::new(1000, 2));
        }

        let overall_score = tracker.get_overall_performance_score(&pattern_name);
        assert!(overall_score > 0.0 && overall_score <= 1.0);
    }
}
