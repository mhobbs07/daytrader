//! Performance monitoring and regression testing for pattern detection
//!
//! This module provides comprehensive performance monitoring capabilities including:
//! - Baseline performance tracking
//! - Regression detection
//! - Performance metrics collection
//! - Automated alerting for performance issues

use crate::utils::metrics::{ErrorMetricsCollector, PerformanceMetric};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Performance baseline data for regression testing
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerformanceBaseline {
    /// Pattern name or benchmark identifier
    pub identifier: String,
    /// Mean execution time in nanoseconds
    pub mean_time_ns: f64,
    /// Standard deviation of execution time
    pub std_dev_ns: f64,
    /// Minimum execution time observed
    pub min_time_ns: f64,
    /// Maximum execution time observed
    pub max_time_ns: f64,
    /// Sample size used for baseline
    pub sample_size: usize,
    /// Timestamp when baseline was established
    pub established_at: chrono::DateTime<chrono::Utc>,
    /// Performance threshold for regression detection (percentage)
    pub regression_threshold_percent: f64,
}

/// Performance regression detection result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegressionResult {
    /// Whether a regression was detected
    pub is_regression: bool,
    /// Current performance metrics
    pub current_metrics: PerformanceMetric,
    /// Baseline metrics for comparison
    pub baseline_metrics: PerformanceBaseline,
    /// Regression percentage (positive = slower, negative = faster)
    pub regression_percent: f64,
    /// Confidence level of regression detection
    pub confidence: f64,
    /// Timestamp of measurement
    pub measured_at: chrono::DateTime<chrono::Utc>,
}

/// Performance monitor for tracking and detecting regressions
pub struct PerformanceMonitor {
    /// Current baselines
    baselines: Arc<RwLock<HashMap<String, PerformanceBaseline>>>,
    /// Metrics collector
    metrics_collector: Arc<ErrorMetricsCollector>,
    /// Baseline storage path
    baseline_path: std::path::PathBuf,
    /// Default regression threshold (percentage)
    default_regression_threshold: f64,
}

impl PerformanceMonitor {
    /// Create a new performance monitor
    pub fn new(
        metrics_collector: Arc<ErrorMetricsCollector>,
        baseline_path: impl AsRef<Path>,
    ) -> Self {
        Self {
            baselines: Arc::new(RwLock::new(HashMap::new())),
            metrics_collector,
            baseline_path: baseline_path.as_ref().to_path_buf(),
            default_regression_threshold: 10.0, // 10% regression threshold
        }
    }

    /// Load baselines from disk
    pub async fn load_baselines(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.baseline_path.exists() {
            info!("No baseline file found, starting with empty baselines");
            return Ok(());
        }

        let data = fs::read_to_string(&self.baseline_path)?;
        let baselines: HashMap<String, PerformanceBaseline> = serde_json::from_str(&data)?;

        let mut current_baselines = self.baselines.write().await;
        *current_baselines = baselines;

        info!("Loaded {} performance baselines", current_baselines.len());
        Ok(())
    }

    /// Save baselines to disk
    pub async fn save_baselines(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let baselines = self.baselines.read().await;
        let data = serde_json::to_string_pretty(&*baselines)?;

        // Ensure directory exists
        if let Some(parent) = self.baseline_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&self.baseline_path, data)?;
        info!("Saved {} performance baselines", baselines.len());
        Ok(())
    }

    /// Establish or update a performance baseline
    pub async fn establish_baseline(
        &self,
        identifier: &str,
        metrics: &[PerformanceMetric],
        regression_threshold_percent: Option<f64>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if metrics.is_empty() {
            return Err("Cannot establish baseline with empty metrics".into());
        }

        // Calculate statistics
        let times: Vec<f64> = metrics.iter().map(|m| m.execution_time_ns as f64).collect();
        let mean = statistical::mean(&times);
        let std_dev = statistical::standard_deviation(&times, Some(mean));
        let min = times.iter().fold(f64::INFINITY, |a: f64, &b| a.min(b));
        let max = times.iter().fold(f64::NEG_INFINITY, |a: f64, &b| a.max(b));

        let baseline = PerformanceBaseline {
            identifier: identifier.to_string(),
            mean_time_ns: mean,
            std_dev_ns: std_dev,
            min_time_ns: min,
            max_time_ns: max,
            sample_size: metrics.len(),
            established_at: chrono::Utc::now(),
            regression_threshold_percent: regression_threshold_percent
                .unwrap_or(self.default_regression_threshold),
        };

        {
            let mut baselines = self.baselines.write().await;
            baselines.insert(identifier.to_string(), baseline.clone());
        }

        info!(
            "Established baseline for {}: mean={:.2}ns, std_dev={:.2}ns, samples={}",
            identifier,
            mean,
            std_dev,
            metrics.len()
        );

        // Save to disk (needs read lock, so write lock must be dropped first)
        self.save_baselines().await?;

        Ok(())
    }

    /// Check for performance regression
    pub async fn check_regression(
        &self,
        identifier: &str,
        current_metric: PerformanceMetric,
    ) -> Result<RegressionResult, Box<dyn std::error::Error + Send + Sync>> {
        let baselines = self.baselines.read().await;
        let baseline = baselines
            .get(identifier)
            .ok_or_else(|| format!("No baseline found for identifier: {}", identifier))?;

        let current_time = current_metric.execution_time_ns as f64;
        let baseline_mean = baseline.mean_time_ns;

        // Calculate regression percentage
        // Positive = slower (regression), Negative = faster (improvement)
        let regression_percent = ((current_time - baseline_mean) / baseline_mean) * 100.0;

        // Check if this is a regression
        let is_regression = regression_percent > baseline.regression_threshold_percent;

        // Calculate confidence based on standard deviation
        let confidence = if baseline.std_dev_ns > 0.0 {
            let z_score = (current_time - baseline_mean).abs() / baseline.std_dev_ns;
            // Convert z-score to confidence (simplified)
            (1.0 - (-z_score / 2.0).exp()).min(0.999)
        } else {
            0.5 // No confidence if no variance
        };

        let result = RegressionResult {
            is_regression,
            current_metrics: current_metric,
            baseline_metrics: baseline.clone(),
            regression_percent,
            confidence,
            measured_at: chrono::Utc::now(),
        };

        if is_regression {
            warn!(
                "Performance regression detected for {}: {:.2}% slower (confidence: {:.2}%)",
                identifier,
                regression_percent,
                confidence * 100.0
            );
        } else {
            info!(
                "Performance check passed for {}: {:.2}% change from baseline",
                identifier, regression_percent
            );
        }

        Ok(result)
    }

    /// Record a performance measurement
    pub async fn record_measurement(
        &self,
        identifier: &str,
        execution_time_ns: u64,
        additional_data: Option<HashMap<String, f64>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let metric = PerformanceMetric {
            name: identifier.to_string(),
            execution_time_ns,
            timestamp: chrono::Utc::now(),
            additional_data: additional_data.unwrap_or_default(),
        };

        self.metrics_collector
            .record_performance_metric(metric.clone())
            .await?;

        // Check for regression if baseline exists
        if let Ok(regression_result) = self.check_regression(identifier, metric).await {
            if regression_result.is_regression {
                // Could trigger alerts here
                error!("Performance regression alert for {}", identifier);
            }
        }

        Ok(())
    }

    /// Get current baseline for an identifier
    pub async fn get_baseline(&self, identifier: &str) -> Option<PerformanceBaseline> {
        let baselines = self.baselines.read().await;
        baselines.get(identifier).cloned()
    }

    /// List all current baselines
    pub async fn list_baselines(&self) -> Vec<PerformanceBaseline> {
        let baselines = self.baselines.read().await;
        baselines.values().cloned().collect()
    }

    /// Remove a baseline
    pub async fn remove_baseline(
        &self,
        identifier: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut baselines = self.baselines.write().await;
        baselines.remove(identifier);
        self.save_baselines().await?;
        Ok(())
    }

    /// Update regression threshold for a baseline
    pub async fn update_regression_threshold(
        &self,
        identifier: &str,
        new_threshold: f64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut baselines = self.baselines.write().await;
        if let Some(baseline) = baselines.get_mut(identifier) {
            baseline.regression_threshold_percent = new_threshold;
            self.save_baselines().await?;
        }
        Ok(())
    }
}

/// Performance dashboard for real-time metrics visualization
pub struct PerformanceDashboard {
    monitor: Arc<PerformanceMonitor>,
}

impl PerformanceDashboard {
    /// Create a new performance dashboard
    pub fn new(monitor: Arc<PerformanceMonitor>) -> Self {
        Self { monitor }
    }

    /// Generate a performance report
    pub async fn generate_report(
        &self,
    ) -> Result<PerformanceReport, Box<dyn std::error::Error + Send + Sync>> {
        let baselines = self.monitor.list_baselines().await;
        let recent_metrics = self
            .monitor
            .metrics_collector
            .get_recent_metrics(100)
            .await?;
        let metrics_count = recent_metrics.len();

        let mut report = PerformanceReport {
            generated_at: chrono::Utc::now(),
            baselines,
            recent_metrics,
            summary: PerformanceSummary::default(),
        };

        // Calculate summary statistics
        if !report.baselines.is_empty() {
            let total_baselines = report.baselines.len();
            let avg_regression_threshold: f64 = report
                .baselines
                .iter()
                .map(|b| b.regression_threshold_percent)
                .sum::<f64>()
                / total_baselines as f64;

            report.summary = PerformanceSummary {
                total_baselines,
                average_regression_threshold: avg_regression_threshold,
                total_measurements: metrics_count,
            };
        }

        Ok(report)
    }
}

/// Performance report for dashboard display
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerformanceReport {
    /// When the report was generated
    pub generated_at: chrono::DateTime<chrono::Utc>,
    /// Current baselines
    pub baselines: Vec<PerformanceBaseline>,
    /// Recent performance metrics
    pub recent_metrics: Vec<PerformanceMetric>,
    /// Summary statistics
    pub summary: PerformanceSummary,
}

/// Summary statistics for performance report
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PerformanceSummary {
    /// Total number of baselines
    pub total_baselines: usize,
    /// Average regression threshold across all baselines
    pub average_regression_threshold: f64,
    /// Total number of recent measurements
    pub total_measurements: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_baseline_establishment() {
        let temp_dir = tempdir().unwrap();
        let baseline_path = temp_dir.path().join("baselines.json");
        let metrics_collector = Arc::new(ErrorMetricsCollector::new());

        let monitor = PerformanceMonitor::new(metrics_collector, &baseline_path);

        let metrics = vec![
            PerformanceMetric {
                name: "test_pattern".to_string(),
                execution_time_ns: 1000,
                timestamp: chrono::Utc::now(),
                additional_data: HashMap::new(),
            },
            PerformanceMetric {
                name: "test_pattern".to_string(),
                execution_time_ns: 1100,
                timestamp: chrono::Utc::now(),
                additional_data: HashMap::new(),
            },
        ];

        monitor
            .establish_baseline("test_pattern", &metrics, Some(5.0))
            .await
            .unwrap();

        let baseline = monitor.get_baseline("test_pattern").await.unwrap();
        assert_eq!(baseline.identifier, "test_pattern");
        assert_eq!(baseline.sample_size, 2);
        assert!((baseline.mean_time_ns - 1050.0).abs() < 1.0);
    }

    #[tokio::test]
    async fn test_regression_detection() {
        let temp_dir = tempdir().unwrap();
        let baseline_path = temp_dir.path().join("baselines.json");
        let metrics_collector = Arc::new(ErrorMetricsCollector::new());

        let monitor = PerformanceMonitor::new(metrics_collector, &baseline_path);

        // Establish baseline
        let baseline_metrics = vec![PerformanceMetric {
            name: "test_pattern".to_string(),
            execution_time_ns: 1000,
            timestamp: chrono::Utc::now(),
            additional_data: HashMap::new(),
        }];

        monitor
            .establish_baseline("test_pattern", &baseline_metrics, Some(10.0))
            .await
            .unwrap();

        // Test with slower performance (20% regression)
        let current_metric = PerformanceMetric {
            name: "test_pattern".to_string(),
            execution_time_ns: 1200, // 20% slower
            timestamp: chrono::Utc::now(),
            additional_data: HashMap::new(),
        };

        let regression_result = monitor
            .check_regression("test_pattern", current_metric)
            .await
            .unwrap();

        assert!(regression_result.is_regression);
        assert!((regression_result.regression_percent - 20.0).abs() < 1.0);
    }
}
