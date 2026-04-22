//! Error metrics and monitoring dashboard

use crate::utils::error_context::{
    ERROR_REGISTRY, ErrorMetrics as ContextErrorMetrics, ErrorSeverity,
};
use metrics::{counter, gauge, histogram};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, interval};

/// Individual performance metric measurement
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerformanceMetric {
    /// Name/identifier of the measurement
    pub name: String,
    /// Execution time in nanoseconds
    pub execution_time_ns: u64,
    /// Timestamp of measurement
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Additional performance data
    pub additional_data: HashMap<String, f64>,
}

/// Global metrics registry for error monitoring
#[derive(Debug)]
pub struct ErrorMetricsCollector {
    /// Error counts by severity
    error_counts: RwLock<HashMap<String, u64>>,
    /// Error rates (errors per second)
    error_rates: RwLock<HashMap<String, f64>>,
    /// Circuit breaker states
    circuit_breaker_states: RwLock<HashMap<String, String>>,
    /// Recovery attempt counts
    recovery_attempts: RwLock<HashMap<String, u64>>,
    /// Recovery success rates
    recovery_success_rates: RwLock<HashMap<String, f64>>,
}

impl ErrorMetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            error_counts: RwLock::new(HashMap::new()),
            error_rates: RwLock::new(HashMap::new()),
            circuit_breaker_states: RwLock::new(HashMap::new()),
            recovery_attempts: RwLock::new(HashMap::new()),
            recovery_success_rates: RwLock::new(HashMap::new()),
        }
    }

    /// Record an error occurrence
    pub async fn record_error(&self, component: &str, severity: ErrorSeverity) {
        let key = format!("{}_{}", component, severity.to_string().to_lowercase());

        // Update error count
        {
            let mut counts = self.error_counts.write().await;
            let count = counts.entry(key.clone()).or_insert(0);
            *count += 1;

            // Record to metrics crate
            counter!("daytrader_errors_total", "component" => component.to_string(), "severity" => severity.to_string()).increment(1);
        }

        // Update error rate (simplified - in real implementation would use time windows)
        {
            let mut rates = self.error_rates.write().await;
            rates.entry(key).or_insert(0.0);
            // Rate calculation would be more sophisticated in production
        }
    }

    /// Record circuit breaker state change
    pub async fn record_circuit_breaker_state(&self, component: &str, state: &str) {
        let mut states = self.circuit_breaker_states.write().await;
        states.insert(component.to_string(), state.to_string());

        // Record to metrics crate
        gauge!("daytrader_circuit_breaker_state", "component" => component.to_string()).set(
            match state {
                "closed" => 0.0,
                "open" => 1.0,
                "half_open" => 0.5,
                _ => -1.0,
            },
        );
    }

    /// Record recovery attempt
    pub async fn record_recovery_attempt(&self, component: &str, success: bool) {
        let key = component.to_string();

        // Update attempt count
        {
            let mut attempts = self.recovery_attempts.write().await;
            let count = attempts.entry(key.clone()).or_insert(0);
            *count += 1;
        }

        // Update success rate
        {
            let mut rates = self.recovery_success_rates.write().await;
            let current_rate = rates.entry(key.clone()).or_insert(0.0);
            // Simplified success rate calculation
            if success {
                *current_rate = (*current_rate + 1.0) / 2.0; // Moving average
            } else {
                *current_rate = *current_rate / 2.0;
            }

            histogram!("daytrader_recovery_success_rate", "component" => component.to_string())
                .record(*current_rate);
        }

        // Record to metrics crate
        counter!("daytrader_recovery_attempts_total", "component" => component.to_string(), "success" => success.to_string()).increment(1);
    }

    /// Record a performance metric
    pub async fn record_performance_metric(
        &self,
        metric: PerformanceMetric,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Record the metric to the histogram
        histogram!("daytrader_performance_metric", "name" => metric.name.clone())
            .record(metric.execution_time_ns as f64);
        Ok(())
    }

    /// Get recent performance metrics
    pub async fn get_recent_metrics(
        &self,
        _limit: usize,
    ) -> Result<Vec<PerformanceMetric>, Box<dyn std::error::Error + Send + Sync>> {
        // Simplified implementation - in production this would query from storage
        Ok(Vec::new())
    }

    /// Get current metrics snapshot
    pub async fn get_metrics_snapshot(&self) -> MetricsSnapshot {
        let context_metrics = ERROR_REGISTRY.get_metrics().await;

        MetricsSnapshot {
            error_counts: self.error_counts.read().await.clone(),
            error_rates: self.error_rates.read().await.clone(),
            circuit_breaker_states: self.circuit_breaker_states.read().await.clone(),
            recovery_attempts: self.recovery_attempts.read().await.clone(),
            recovery_success_rates: self.recovery_success_rates.read().await.clone(),
            context_metrics,
        }
    }

    /// Start metrics collection task
    pub async fn start_collection_task(self: Arc<Self>) {
        let mut interval = interval(Duration::from_secs(60)); // Collect every minute

        tokio::spawn(async move {
            loop {
                interval.tick().await;

                // Collect and update metrics
                let snapshot = self.get_metrics_snapshot().await;

                // Update global gauges
                gauge!("daytrader_total_errors").set(snapshot.context_metrics.total_errors as f64);
                gauge!("daytrader_critical_errors")
                    .set(snapshot.context_metrics.critical_count as f64);
                gauge!("daytrader_error_errors").set(snapshot.context_metrics.error_count as f64);
                gauge!("daytrader_warning_errors")
                    .set(snapshot.context_metrics.warning_count as f64);

                // Log summary if needed
                if snapshot.context_metrics.critical_count > 0 {
                    tracing::warn!(
                        "Critical errors detected: {}",
                        snapshot.context_metrics.critical_count
                    );
                }
            }
        });
    }
}

impl Default for ErrorMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Metrics snapshot for monitoring dashboard
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricsSnapshot {
    pub error_counts: HashMap<String, u64>,
    pub error_rates: HashMap<String, f64>,
    pub circuit_breaker_states: HashMap<String, String>,
    pub recovery_attempts: HashMap<String, u64>,
    pub recovery_success_rates: HashMap<String, f64>,
    pub context_metrics: ContextErrorMetrics,
}

/// Monitoring dashboard interface
pub struct ErrorMonitoringDashboard {
    collector: Arc<ErrorMetricsCollector>,
}

impl ErrorMonitoringDashboard {
    /// Create a new dashboard
    pub fn new(collector: Arc<ErrorMetricsCollector>) -> Self {
        Self { collector }
    }

    /// Get current dashboard data
    pub async fn get_dashboard_data(&self) -> DashboardData {
        let snapshot = self.collector.get_metrics_snapshot().await;

        // Calculate health scores
        let health_score = self.calculate_health_score(&snapshot);

        // Identify problematic components
        let problematic_components = self.identify_problematic_components(&snapshot);

        // Generate recommendations
        let recommendations = self.generate_recommendations(&snapshot);

        DashboardData {
            snapshot,
            health_score,
            problematic_components,
            recommendations,
            last_updated: chrono::Utc::now(),
        }
    }

    fn calculate_health_score(&self, snapshot: &MetricsSnapshot) -> f64 {
        let total_errors = snapshot.context_metrics.total_errors as f64;
        let critical_errors = snapshot.context_metrics.critical_count as f64;

        if total_errors == 0.0 {
            return 100.0; // Perfect health
        }

        // Health score decreases with more errors, especially critical ones
        let base_score = 100.0 - (total_errors * 2.0).min(50.0);
        let critical_penalty = critical_errors * 10.0;

        (base_score - critical_penalty).max(0.0)
    }

    fn identify_problematic_components(&self, snapshot: &MetricsSnapshot) -> Vec<String> {
        let mut problematic = Vec::new();

        // Components with high error rates
        for (component_severity, count) in &snapshot.error_counts {
            if *count > 10 {
                // Threshold for problematic
                if let Some(component) = component_severity.split('_').next() {
                    if !problematic.contains(&component.to_string()) {
                        problematic.push(component.to_string());
                    }
                }
            }
        }

        // Components with open circuit breakers
        for (component, state) in &snapshot.circuit_breaker_states {
            if state == "open" {
                problematic.push(component.clone());
            }
        }

        problematic
    }

    fn generate_recommendations(&self, snapshot: &MetricsSnapshot) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Check for critical errors
        if snapshot.context_metrics.critical_count > 0 {
            recommendations
                .push("Immediate attention required: Critical errors detected".to_string());
        }

        // Check circuit breaker states
        let open_circuits: Vec<_> = snapshot
            .circuit_breaker_states
            .iter()
            .filter(|(_, state)| *state == "open")
            .map(|(comp, _)| comp.clone())
            .collect();

        if !open_circuits.is_empty() {
            recommendations.push(format!(
                "Circuit breakers open for components: {}",
                open_circuits.join(", ")
            ));
        }

        // Check recovery success rates
        for (component, rate) in &snapshot.recovery_success_rates {
            if *rate < 0.5 {
                // Less than 50% success rate
                recommendations.push(format!(
                    "Low recovery success rate for {}: {:.1}%",
                    component,
                    rate * 100.0
                ));
            }
        }

        if recommendations.is_empty() {
            recommendations.push("System operating normally".to_string());
        }

        recommendations
    }
}

/// Dashboard data for display
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DashboardData {
    pub snapshot: MetricsSnapshot,
    pub health_score: f64,
    pub problematic_components: Vec<String>,
    pub recommendations: Vec<String>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Global metrics collector instance
lazy_static::lazy_static! {
    pub static ref METRICS_COLLECTOR: Arc<ErrorMetricsCollector> = Arc::new(ErrorMetricsCollector::new());
}

/// Initialize metrics collection
pub async fn init_metrics_collection() {
    // Start the collection task
    let collector = Arc::clone(&METRICS_COLLECTOR);
    collector.start_collection_task().await;

    // Register metrics with the metrics crate
    metrics::describe_counter!(
        "daytrader_errors_total",
        "Total number of errors by component and severity"
    );
    metrics::describe_gauge!(
        "daytrader_circuit_breaker_state",
        "Circuit breaker state (0=closed, 1=open, 0.5=half-open)"
    );
    metrics::describe_counter!(
        "daytrader_recovery_attempts_total",
        "Total recovery attempts by component and success"
    );
    metrics::describe_histogram!(
        "daytrader_recovery_success_rate",
        "Recovery success rate by component"
    );
    metrics::describe_gauge!("daytrader_total_errors", "Total error count");
    metrics::describe_gauge!("daytrader_critical_errors", "Critical error count");
    metrics::describe_gauge!("daytrader_error_errors", "Error severity count");
    metrics::describe_gauge!("daytrader_warning_errors", "Warning severity count");
}

/// Helper macros for metrics recording
#[macro_export]
macro_rules! record_error {
    ($component:expr, $severity:expr) => {
        tokio::spawn(async move {
            $crate::utils::metrics::METRICS_COLLECTOR
                .record_error($component, $severity)
                .await;
        });
    };
}

#[macro_export]
macro_rules! record_circuit_breaker_state {
    ($component:expr, $state:expr) => {
        tokio::spawn(async move {
            $crate::utils::metrics::METRICS_COLLECTOR
                .record_circuit_breaker_state($component, $state)
                .await;
        });
    };
}

#[macro_export]
macro_rules! record_recovery_attempt {
    ($component:expr, $success:expr) => {
        tokio::spawn(async move {
            $crate::utils::metrics::METRICS_COLLECTOR
                .record_recovery_attempt($component, $success)
                .await;
        });
    };
}
