//! Error recovery mechanisms with automatic retry logic and circuit breakers

use std::future::Future;
use std::time::Duration;
use tokio_retry::{strategy::ExponentialBackoff, Retry};
use tokio::sync::RwLock;
use tokio::time::sleep;
use crate::utils::error_context::{ErrorContext, ErrorSeverity, ContextualError};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,   // Normal operation
    Open,     // Failing, requests rejected
    HalfOpen, // Testing if service recovered
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Failure threshold before opening circuit
    pub failure_threshold: u32,
    /// Success threshold to close circuit from half-open
    pub success_threshold: u32,
    /// Timeout before attempting half-open
    pub timeout: Duration,
    /// Time window for tracking failures
    pub monitoring_window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(60),
            monitoring_window: Duration::from_secs(60),
        }
    }
}

/// Circuit breaker for protecting against cascading failures
#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: RwLock<CircuitState>,
    failures: RwLock<Vec<std::time::Instant>>,
    successes: RwLock<u32>,
    last_failure_time: RwLock<Option<std::time::Instant>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: RwLock::new(CircuitState::Closed),
            failures: RwLock::new(Vec::new()),
            successes: RwLock::new(0),
            last_failure_time: RwLock::new(None),
        }
    }

    /// Execute a function with circuit breaker protection
    pub async fn execute<F, Fut, T, E>(&self, mut f: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::fmt::Debug,
    {
        let state = *self.state.read().await;

        match state {
            CircuitState::Open => {
                // Check if we should transition to half-open
                if let Some(last_failure) = *self.last_failure_time.read().await {
                    if last_failure.elapsed() >= self.config.timeout {
                        *self.state.write().await = CircuitState::HalfOpen;
                        *self.successes.write().await = 0;
                    } else {
                        return Err(CircuitBreakerError::CircuitOpen);
                    }
                }
            }
            CircuitState::HalfOpen => {
                // Allow request but monitor closely
            }
            CircuitState::Closed => {
                // Normal operation
            }
        }

        match f().await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(error) => {
                self.on_failure().await;
                Err(CircuitBreakerError::OperationFailed(error))
            }
        }
    }

    async fn on_success(&self) {
        let mut state = self.state.write().await;
        match *state {
            CircuitState::HalfOpen => {
                let mut successes = self.successes.write().await;
                *successes += 1;
                if *successes >= self.config.success_threshold {
                    *state = CircuitState::Closed;
                    *self.failures.write().await = Vec::new();
                    *self.last_failure_time.write().await = None;
                }
            }
            CircuitState::Closed => {
                // Clean up old failures
                let mut failures = self.failures.write().await;
                let cutoff = std::time::Instant::now() - self.config.monitoring_window;
                failures.retain(|&time| time > cutoff);
            }
            CircuitState::Open => {
                // Should not happen, but reset if it does
                *state = CircuitState::Closed;
            }
        }
    }

    async fn on_failure(&self) {
        let mut state = self.state.write().await;
        let now = std::time::Instant::now();

        match *state {
            CircuitState::HalfOpen => {
                *state = CircuitState::Open;
                *self.last_failure_time.write().await = Some(now);
            }
            CircuitState::Closed => {
                let mut failures = self.failures.write().await;
                failures.push(now);

                // Clean up old failures
                let cutoff = now - self.config.monitoring_window;
                failures.retain(|&time| time > cutoff);

                if failures.len() >= self.config.failure_threshold as usize {
                    *state = CircuitState::Open;
                    *self.last_failure_time.write().await = Some(now);
                }
            }
            CircuitState::Open => {
                *self.last_failure_time.write().await = Some(now);
            }
        }
    }

    /// Get current circuit state
    pub async fn state(&self) -> CircuitState {
        *self.state.read().await
    }
}

/// Circuit breaker error types
#[derive(Debug, thiserror::Error)]
pub enum CircuitBreakerError<E> {
    #[error("Circuit breaker is open")]
    CircuitOpen,
    #[error("Operation failed: {0:?}")]
    OperationFailed(E),
}

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Backoff multiplier
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
        }
    }
}

/// Retry executor with exponential backoff
pub struct RetryExecutor {
    config: RetryConfig,
}

impl RetryExecutor {
    /// Create a new retry executor
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Execute a function with retry logic
    pub async fn execute<F, Fut, T, E>(&self, f: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::fmt::Debug,
    {
        let strategy = ExponentialBackoff::from_millis(self.config.initial_delay.as_millis() as u64)
            .factor(self.config.multiplier as u64)
            .max_delay(self.config.max_delay)
            .take(self.config.max_attempts as usize);

        Retry::spawn(strategy, f).await
    }
}

/// Contextual retry executor that integrates with error context
pub struct ContextualRetryExecutor {
    retry_executor: RetryExecutor,
    circuit_breaker: Option<CircuitBreaker>,
}

impl ContextualRetryExecutor {
    /// Create a new contextual retry executor
    pub fn new(config: RetryConfig) -> Self {
        Self {
            retry_executor: RetryExecutor::new(config),
            circuit_breaker: None,
        }
    }

    /// Create with circuit breaker
    pub fn with_circuit_breaker(config: RetryConfig, circuit_config: CircuitBreakerConfig) -> Self {
        Self {
            retry_executor: RetryExecutor::new(config),
            circuit_breaker: Some(CircuitBreaker::new(circuit_config)),
        }
    }

    /// Execute with full error context and recovery
    pub async fn execute_with_context<F, Fut, T>(
        &self,
        context: &ErrorContext,
        operation: F,
    ) -> Result<T, ContextualError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, ContextualError>>,
    {
        let operation_with_context = || async {
            match operation().await {
                Ok(result) => Ok(result),
                Err(mut error) => {
                    // Chain the context
                    error.context = error.context.chain_from(context);
                    Err(error)
                }
            }
        };

        if let Some(circuit_breaker) = &self.circuit_breaker {
            circuit_breaker
                .execute(operation_with_context)
                .await
                .map_err(|e| match e {
                    CircuitBreakerError::CircuitOpen => {
                        ContextualError::new(
                            anyhow::anyhow!("Circuit breaker is open"),
                            ErrorContext::new(&context.component, "circuit_breaker")
                                .with_severity(ErrorSeverity::Warning)
                                .with_metadata("correlation_id", &context.correlation_id.to_string())
                        )
                    }
                    CircuitBreakerError::OperationFailed(error) => error,
                })
        } else {
            self.retry_executor.execute(operation_with_context).await
        }
    }
}

/// Helper trait for adding retry capabilities to results
pub trait Retryable<T, E> {
    fn with_retry(self, config: RetryConfig) -> impl Future<Output = Result<T, E>>;
}

impl<T, E, F, Fut> Retryable<T, E> for F
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    async fn with_retry(self, config: RetryConfig) -> Result<T, E> {
        RetryExecutor::new(config).execute(self).await
    }
}