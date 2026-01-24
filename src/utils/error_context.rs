//! Enhanced error context system with correlation IDs, tracing, and context accumulation

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// Severity levels for error logging
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorSeverity::Debug => write!(f, "DEBUG"),
            ErrorSeverity::Info => write!(f, "INFO"),
            ErrorSeverity::Warning => write!(f, "WARNING"),
            ErrorSeverity::Error => write!(f, "ERROR"),
            ErrorSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Correlation ID for tracking requests across the system
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(Uuid);

impl CorrelationId {
    /// Generate a new correlation ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from existing UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Error context containing metadata and tracing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// Unique correlation ID for tracking
    pub correlation_id: CorrelationId,
    /// Timestamp when the error occurred
    pub timestamp: DateTime<Utc>,
    /// Severity level
    pub severity: ErrorSeverity,
    /// Component where the error originated
    pub component: String,
    /// Operation being performed
    pub operation: String,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    /// Call stack context
    pub call_stack: Vec<String>,
}

impl ErrorContext {
    /// Create a new error context
    pub fn new(component: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            correlation_id: CorrelationId::new(),
            timestamp: Utc::now(),
            severity: ErrorSeverity::Error,
            component: component.into(),
            operation: operation.into(),
            metadata: HashMap::new(),
            call_stack: Vec::new(),
        }
    }

    /// Create with specific correlation ID
    pub fn with_correlation_id(
        correlation_id: CorrelationId,
        component: impl Into<String>,
        operation: impl Into<String>
    ) -> Self {
        Self {
            correlation_id,
            timestamp: Utc::now(),
            severity: ErrorSeverity::Error,
            component: component.into(),
            operation: operation.into(),
            metadata: HashMap::new(),
            call_stack: Vec::new(),
        }
    }

    /// Set severity level
    pub fn with_severity(mut self, severity: ErrorSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Add call stack context
    pub fn with_call_context(mut self, context: impl Into<String>) -> Self {
        self.call_stack.push(context.into());
        self
    }

    /// Chain context from another error context
    pub fn chain_from(mut self, other: &ErrorContext) -> Self {
        self.call_stack.extend(other.call_stack.clone());
        self.call_stack.push(format!("{}::{}", other.component, other.operation));
        self.metadata.extend(other.metadata.clone());
        self
    }
}

impl Default for ErrorContext {
    fn default() -> Self {
        Self::new("unknown", "unknown")
    }
}

/// Enhanced error type with context
#[derive(Debug)]
pub struct ContextualError {
    /// The underlying error
    pub error: anyhow::Error,
    /// Error context
    pub context: ErrorContext,
}

impl ContextualError {
    /// Create a new contextual error
    pub fn new(error: impl Into<anyhow::Error>, context: ErrorContext) -> Self {
        Self {
            error: error.into(),
            context,
        }
    }

    /// Create from existing error with additional context
    pub fn with_context(error: impl Into<anyhow::Error>, component: impl Into<String>, operation: impl Into<String>) -> Self {
        Self::new(error, ErrorContext::new(component, operation))
    }
}

impl fmt::Display for ContextualError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}::{} - {}", self.context.correlation_id, self.context.component, self.context.operation, self.error)
    }
}

impl std::error::Error for ContextualError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.error.source()
    }
}

/// Global error context registry for tracking and metrics
#[derive(Debug)]
pub struct ErrorContextRegistry {
    errors: RwLock<HashMap<CorrelationId, Vec<ErrorContext>>>,
}

impl ErrorContextRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            errors: RwLock::new(HashMap::new()),
        }
    }

    /// Register an error context
    pub async fn register_error(&self, context: ErrorContext) {
        let mut errors = self.errors.write().await;
        errors.entry(context.correlation_id.clone())
            .or_insert_with(Vec::new)
            .push(context);
    }

    /// Get all errors for a correlation ID
    pub async fn get_errors(&self, correlation_id: &CorrelationId) -> Vec<ErrorContext> {
        let errors = self.errors.read().await;
        errors.get(correlation_id).cloned().unwrap_or_default()
    }

    /// Get error metrics
    pub async fn get_metrics(&self) -> ErrorMetrics {
        let errors = self.errors.read().await;
        let mut metrics = ErrorMetrics::default();

        for contexts in errors.values() {
            for context in contexts {
                metrics.total_errors += 1;
                match context.severity {
                    ErrorSeverity::Debug => metrics.debug_count += 1,
                    ErrorSeverity::Info => metrics.info_count += 1,
                    ErrorSeverity::Warning => metrics.warning_count += 1,
                    ErrorSeverity::Error => metrics.error_count += 1,
                    ErrorSeverity::Critical => metrics.critical_count += 1,
                }

                // Count by component
                *metrics.errors_by_component.entry(context.component.clone()).or_insert(0) += 1;
            }
        }

        metrics
    }

    /// Clear old errors (older than specified duration)
    pub async fn cleanup_old_errors(&self, max_age: chrono::Duration) {
        let cutoff = Utc::now() - max_age;
        let mut errors = self.errors.write().await;

        errors.retain(|_, contexts| {
            contexts.retain(|ctx| ctx.timestamp > cutoff);
            !contexts.is_empty()
        });
    }
}

impl Default for ErrorContextRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Error metrics for monitoring
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ErrorMetrics {
    pub total_errors: u64,
    pub debug_count: u64,
    pub info_count: u64,
    pub warning_count: u64,
    pub error_count: u64,
    pub critical_count: u64,
    pub errors_by_component: HashMap<String, u64>,
}

/// Global registry instance
lazy_static::lazy_static! {
    pub static ref ERROR_REGISTRY: ErrorContextRegistry = ErrorContextRegistry::new();
}

/// Helper trait for adding context to results
pub trait WithContext<T> {
    fn with_context<F>(self, f: F) -> Result<T, ContextualError>
    where
        F: FnOnce() -> ErrorContext;
}

impl<T, E> WithContext<T> for Result<T, E>
where
    E: Into<anyhow::Error>,
{
    fn with_context<F>(self, f: F) -> Result<T, ContextualError>
    where
        F: FnOnce() -> ErrorContext,
    {
        self.map_err(|e| ContextualError::new(e, f()))
    }
}

/// Helper macro for creating contextual errors
#[macro_export]
macro_rules! contextual_error {
    ($error:expr, $component:expr, $operation:expr) => {
        $crate::utils::error_context::ContextualError::with_context($error, $component, $operation)
    };
    ($error:expr, $component:expr, $operation:expr, $($key:expr => $value:expr),*) => {
        {
            let mut ctx = $crate::utils::error_context::ErrorContext::new($component, $operation);
            $(
                ctx = ctx.with_metadata($key, $value);
            )*
            $crate::utils::error_context::ContextualError::new($error, ctx)
        }
    };
}