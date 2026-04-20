//! Structured error logging with metadata and severity levels

use crate::utils::error_context::{ContextualError, ERROR_REGISTRY, ErrorContext, ErrorSeverity};
use std::collections::HashMap;
use tracing::{Level, debug, error, info, warn};

/// Initialize structured logging with error context integration
pub fn init_error_logging() -> anyhow::Result<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let subscriber = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "daytrader=info".into()),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_timer(tracing_subscriber::fmt::time::UtcTime::rfc_3339()),
        );

    subscriber.init();
    Ok(())
}

/// Log a contextual error with structured metadata
pub async fn log_contextual_error(error: &ContextualError) {
    // Register the error in the global registry
    ERROR_REGISTRY.register_error(error.context.clone()).await;

    // Create structured log fields
    let mut fields = HashMap::new();
    fields.insert(
        "correlation_id".to_string(),
        error.context.correlation_id.to_string(),
    );
    fields.insert("component".to_string(), error.context.component.clone());
    fields.insert("operation".to_string(), error.context.operation.clone());
    fields.insert("severity".to_string(), error.context.severity.to_string());
    fields.insert(
        "timestamp".to_string(),
        error.context.timestamp.to_rfc3339(),
    );

    // Add metadata
    for (key, value) in &error.context.metadata {
        fields.insert(format!("meta_{}", key), value.clone());
    }

    // Add call stack
    if !error.context.call_stack.is_empty() {
        fields.insert(
            "call_stack".to_string(),
            error.context.call_stack.join(" -> "),
        );
    }

    // Log based on severity
    match error.context.severity {
        ErrorSeverity::Critical => {
            error!(
                correlation_id = %error.context.correlation_id,
                component = %error.context.component,
                operation = %error.context.operation,
                error = %error.error,
                "Critical error occurred"
            );
        }
        ErrorSeverity::Error => {
            error!(
                correlation_id = %error.context.correlation_id,
                component = %error.context.component,
                operation = %error.context.operation,
                error = %error.error,
                "Error occurred"
            );
        }
        ErrorSeverity::Warning => {
            warn!(
                correlation_id = %error.context.correlation_id,
                component = %error.context.component,
                operation = %error.context.operation,
                error = %error.error,
                "Warning occurred"
            );
        }
        ErrorSeverity::Info => {
            info!(
                correlation_id = %error.context.correlation_id,
                component = %error.context.component,
                operation = %error.context.operation,
                error = %error.error,
                "Info event"
            );
        }
        ErrorSeverity::Debug => {
            debug!(
                correlation_id = %error.context.correlation_id,
                component = %error.context.component,
                operation = %error.context.operation,
                error = %error.error,
                "Debug event"
            );
        }
    }
}

/// Log an error context directly
pub async fn log_error_context(context: &ErrorContext, message: &str) {
    // Register the error in the global registry
    ERROR_REGISTRY.register_error(context.clone()).await;

    // Log based on severity
    match context.severity {
        ErrorSeverity::Critical => {
            error!(
                correlation_id = %context.correlation_id,
                component = %context.component,
                operation = %context.operation,
                "{}", message
            );
        }
        ErrorSeverity::Error => {
            error!(
                correlation_id = %context.correlation_id,
                component = %context.component,
                operation = %context.operation,
                "{}", message
            );
        }
        ErrorSeverity::Warning => {
            warn!(
                correlation_id = %context.correlation_id,
                component = %context.component,
                operation = %context.operation,
                "{}", message
            );
        }
        ErrorSeverity::Info => {
            info!(
                correlation_id = %context.correlation_id,
                component = %context.component,
                operation = %context.operation,
                "{}", message
            );
        }
        ErrorSeverity::Debug => {
            debug!(
                correlation_id = %context.correlation_id,
                component = %context.component,
                operation = %context.operation,
                "{}", message
            );
        }
    }
}

/// Helper macro for logging contextual errors
#[macro_export]
macro_rules! log_error {
    ($error:expr) => {
        tokio::spawn(async move {
            $crate::utils::logging::log_contextual_error(&$error).await;
        });
    };
}

/// Helper macro for logging error contexts
#[macro_export]
macro_rules! log_context {
    ($context:expr, $message:expr) => {
        tokio::spawn(async move {
            $crate::utils::logging::log_error_context($context, $message).await;
        });
    };
}

/// Structured logger for different components
pub struct ComponentLogger {
    component: String,
}

impl ComponentLogger {
    /// Create a new component logger
    pub fn new(component: impl Into<String>) -> Self {
        Self {
            component: component.into(),
        }
    }

    /// Log an error with context
    pub async fn log_error(
        &self,
        operation: &str,
        error: impl Into<anyhow::Error>,
        severity: ErrorSeverity,
    ) {
        let context = ErrorContext::new(&self.component, operation).with_severity(severity);

        let contextual_error = ContextualError::new(error.into(), context);
        log_contextual_error(&contextual_error).await;
    }

    /// Log a message with context
    pub async fn log_message(&self, operation: &str, message: &str, severity: ErrorSeverity) {
        let context = ErrorContext::new(&self.component, operation).with_severity(severity);

        log_error_context(&context, message).await;
    }

    /// Log with additional metadata
    pub async fn log_with_metadata(
        &self,
        operation: &str,
        message: &str,
        severity: ErrorSeverity,
        metadata: HashMap<String, String>,
    ) {
        let mut context = ErrorContext::new(&self.component, operation).with_severity(severity);

        for (key, value) in metadata {
            context = context.with_metadata(key, value);
        }

        log_error_context(&context, message).await;
    }
}

/// Global component loggers
lazy_static::lazy_static! {
    pub static ref BROKER_LOGGER: ComponentLogger = ComponentLogger::new("broker");
    pub static ref RISK_LOGGER: ComponentLogger = ComponentLogger::new("risk");
    pub static ref PATTERN_LOGGER: ComponentLogger = ComponentLogger::new("pattern");
    pub static ref EXECUTION_LOGGER: ComponentLogger = ComponentLogger::new("execution");
    pub static ref DB_LOGGER: ComponentLogger = ComponentLogger::new("database");
}

/// Performance logging utilities
pub mod performance {
    use std::time::Instant;
    use tracing::{info, warn};

    /// Log operation duration
    pub async fn log_duration(operation: &str, start: Instant, threshold_ms: u64) {
        let duration = start.elapsed();
        let duration_ms = duration.as_millis() as u64;

        if duration_ms > threshold_ms {
            warn!(
                operation = operation,
                duration_ms = duration_ms,
                threshold_ms = threshold_ms,
                "Operation exceeded performance threshold"
            );
        } else {
            info!(
                operation = operation,
                duration_ms = duration_ms,
                "Operation completed within threshold"
            );
        }
    }

    /// Helper macro for timing operations
    #[macro_export]
    macro_rules! time_operation {
        ($operation:expr, $threshold_ms:expr, $code:block) => {{
            let start = std::time::Instant::now();
            let result = $code;
            tokio::spawn(async move {
                $crate::utils::logging::performance::log_duration($operation, start, $threshold_ms)
                    .await;
            });
            result
        }};
    }
}
