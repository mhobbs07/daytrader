//! Client-side rate limiting for the Alpaca REST API
//!
//! Alpaca's free tier allows a limited number of REST requests per minute
//! (200 by default). This module implements a token-bucket limiter so the
//! client throttles itself instead of hammering the API and tripping
//! server-side rate limits (HTTP 429).
//!
//! The bucket is intentionally hand-rolled with `tokio::sync::Mutex` and
//! `tokio::time` rather than pulling in a crate: the algorithm is small,
//! and `brokers/alpaca` is only permitted to depend on `brokers::types`,
//! `brokers::traits`, and `utils::errors` (see `src/brokers/CLAUDE.md`),
//! which rules out reusing `utils::retry`'s `tokio-retry`-based helpers.

use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};

/// Configuration for the token-bucket rate limiter.
#[derive(Debug, Clone, Copy)]
pub struct RateLimiterConfig {
    /// Maximum number of requests allowed per `refill_period`.
    pub max_requests: u32,
    /// The period over which `max_requests` tokens are replenished.
    pub refill_period: Duration,
}

impl RateLimiterConfig {
    /// Alpaca's documented free-tier limit: 200 requests per minute.
    pub const ALPACA_FREE_TIER: Self = Self {
        max_requests: 200,
        refill_period: Duration::from_secs(60),
    };
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self::ALPACA_FREE_TIER
    }
}

/// Internal, mutex-guarded bucket state.
#[derive(Debug)]
struct BucketState {
    /// Tokens currently available.
    tokens: f64,
    /// Last time the bucket was refilled.
    last_refill: Instant,
}

/// A token-bucket rate limiter for throttling outbound REST requests.
///
/// Tokens refill continuously at `max_requests / refill_period`. Callers
/// invoke [`RateLimiter::acquire`] before issuing a request; it waits (via
/// `tokio::time::sleep`) until a token is available, then consumes one.
#[derive(Debug)]
pub struct RateLimiter {
    config: RateLimiterConfig,
    state: Mutex<BucketState>,
}

impl RateLimiter {
    /// Creates a new rate limiter with the given configuration, starting
    /// with a full bucket so the first burst of requests is not delayed.
    pub fn new(config: RateLimiterConfig) -> Self {
        Self {
            config,
            state: Mutex::new(BucketState {
                tokens: config.max_requests as f64,
                last_refill: Instant::now(),
            }),
        }
    }

    /// Tokens replenished per second, given the configured rate.
    fn refill_rate_per_sec(&self) -> f64 {
        self.config.max_requests as f64 / self.config.refill_period.as_secs_f64()
    }

    /// Waits until a token is available, then consumes it.
    ///
    /// This must be called before every outbound REST request. It never
    /// panics or errors — worst case it waits for the bucket to refill.
    pub async fn acquire(&self) {
        loop {
            let wait = {
                let mut state = self.state.lock().await;
                let now = Instant::now();

                // Refill based on elapsed time since the last refill.
                let elapsed = now.saturating_duration_since(state.last_refill);
                let refilled = elapsed.as_secs_f64() * self.refill_rate_per_sec();
                if refilled > 0.0 {
                    state.tokens = (state.tokens + refilled).min(self.config.max_requests as f64);
                    state.last_refill = now;
                }

                if state.tokens >= 1.0 {
                    state.tokens -= 1.0;
                    None
                } else {
                    // Not enough tokens: compute how long until one token
                    // becomes available and sleep outside the lock.
                    let deficit = 1.0 - state.tokens;
                    let secs_needed = deficit / self.refill_rate_per_sec();
                    Some(Duration::from_secs_f64(secs_needed.max(0.0)))
                }
            };

            match wait {
                None => return,
                Some(duration) => {
                    // Sleep without holding the lock so other callers can
                    // still check/refill the bucket concurrently.
                    tokio::time::sleep(duration).await;
                }
            }
        }
    }

    /// Returns the number of whole tokens currently available, refilling
    /// first. Intended for tests and diagnostics.
    #[cfg(test)]
    pub async fn available_tokens(&self) -> f64 {
        let mut state = self.state.lock().await;
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(state.last_refill);
        let refilled = elapsed.as_secs_f64() * self.refill_rate_per_sec();
        if refilled > 0.0 {
            state.tokens = (state.tokens + refilled).min(self.config.max_requests as f64);
            state.last_refill = now;
        }
        state.tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fast_config() -> RateLimiterConfig {
        // 10 requests per 1 second, easy to reason about in tests.
        RateLimiterConfig {
            max_requests: 10,
            refill_period: Duration::from_secs(1),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn test_bucket_starts_full_and_allows_burst_up_to_capacity() {
        let limiter = RateLimiter::new(fast_config());

        for _ in 0..10 {
            limiter.acquire().await;
        }

        let tokens = limiter.available_tokens().await;
        assert!(tokens < 1.0, "expected bucket to be drained, got {tokens}");
    }

    #[tokio::test(start_paused = true)]
    async fn test_bucket_refills_over_time() {
        let limiter = RateLimiter::new(fast_config());

        // Drain the bucket completely.
        for _ in 0..10 {
            limiter.acquire().await;
        }
        assert!(limiter.available_tokens().await < 1.0);

        // Advance virtual time by half the refill period: expect ~5 tokens.
        tokio::time::advance(Duration::from_millis(500)).await;
        let tokens = limiter.available_tokens().await;
        assert!(
            (4.0..=6.0).contains(&tokens),
            "expected ~5 tokens after half refill period, got {tokens}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn test_bucket_never_exceeds_capacity() {
        let limiter = RateLimiter::new(fast_config());

        // Advance well beyond the refill period without consuming tokens.
        tokio::time::advance(Duration::from_secs(10)).await;
        let tokens = limiter.available_tokens().await;
        assert!(
            tokens <= 10.0 + f64::EPSILON,
            "expected tokens capped at max_requests, got {tokens}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn test_acquire_waits_when_bucket_empty() {
        let limiter = RateLimiter::new(fast_config());

        // Drain the bucket.
        for _ in 0..10 {
            limiter.acquire().await;
        }

        // The 11th acquire must wait for a refill; with time paused this
        // completes only once `advance` moves the clock forward, which
        // `tokio::time::sleep` will do automatically under auto-advance.
        // We assert it completes without panicking and within a bounded
        // number of virtual-time steps by racing it against a timeout.
        let acquire_fut = limiter.acquire();
        tokio::time::timeout(Duration::from_secs(2), acquire_fut)
            .await
            .expect("acquire should complete once bucket refills");
    }

    #[tokio::test]
    async fn test_default_config_matches_alpaca_free_tier() {
        let config = RateLimiterConfig::default();
        assert_eq!(config.max_requests, 200);
        assert_eq!(config.refill_period, Duration::from_secs(60));
    }
}
