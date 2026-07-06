//! Exponential backoff helper for WebSocket reconnect attempts.
//!
//! Hand-rolled for the same reason as `ratelimit.rs`: `brokers/alpaca` may
//! only depend on `brokers::types`, `brokers::traits`, and `utils::errors`,
//! so the `tokio-retry`-based helpers in `utils::retry` are out of bounds
//! here. The algorithm is a few lines: start at `initial_delay`, double on
//! each failure up to `max_delay`, and reset back to `initial_delay` after
//! a successful connection.

use tokio::time::Duration;

/// Tracks reconnect backoff state across repeated connection attempts.
#[derive(Debug, Clone)]
pub struct ReconnectBackoff {
    initial_delay: Duration,
    max_delay: Duration,
    current_delay: Duration,
}

impl ReconnectBackoff {
    /// Creates a new backoff tracker starting at `initial_delay` and
    /// capping at `max_delay`.
    pub fn new(initial_delay: Duration, max_delay: Duration) -> Self {
        Self {
            initial_delay,
            max_delay,
            current_delay: initial_delay,
        }
    }

    /// The default policy used for Alpaca WebSocket reconnects: 1s initial
    /// delay, doubling up to a 60s cap.
    pub fn default_policy() -> Self {
        Self::new(Duration::from_secs(1), Duration::from_secs(60))
    }

    /// Returns the delay to use for the next reconnect attempt, then
    /// doubles the internal delay (capped at `max_delay`) for next time.
    pub fn next_delay(&mut self) -> Duration {
        let delay = self.current_delay;
        self.current_delay = (self.current_delay * 2).min(self.max_delay);
        delay
    }

    /// Resets the backoff to its initial delay. Call this after a
    /// successful connection so the next disconnect starts the ramp over.
    pub fn reset(&mut self) {
        self.current_delay = self.initial_delay;
    }
}

impl Default for ReconnectBackoff {
    fn default() -> Self {
        Self::default_policy()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_doubles_each_attempt() {
        let mut backoff = ReconnectBackoff::new(Duration::from_secs(1), Duration::from_secs(60));

        assert_eq!(backoff.next_delay(), Duration::from_secs(1));
        assert_eq!(backoff.next_delay(), Duration::from_secs(2));
        assert_eq!(backoff.next_delay(), Duration::from_secs(4));
        assert_eq!(backoff.next_delay(), Duration::from_secs(8));
    }

    #[test]
    fn test_backoff_caps_at_max_delay() {
        let mut backoff = ReconnectBackoff::new(Duration::from_secs(1), Duration::from_secs(10));

        let delays: Vec<Duration> = (0..10).map(|_| backoff.next_delay()).collect();
        assert_eq!(delays.last(), Some(&Duration::from_secs(10)));
        assert!(delays.iter().all(|d| *d <= Duration::from_secs(10)));
    }

    #[test]
    fn test_backoff_resets_after_success() {
        let mut backoff = ReconnectBackoff::new(Duration::from_secs(1), Duration::from_secs(60));

        backoff.next_delay();
        backoff.next_delay();
        backoff.next_delay();
        assert_eq!(backoff.next_delay(), Duration::from_secs(8));

        backoff.reset();
        assert_eq!(backoff.next_delay(), Duration::from_secs(1));
    }

    #[test]
    fn test_default_policy_matches_documented_values() {
        let mut backoff = ReconnectBackoff::default_policy();
        assert_eq!(backoff.next_delay(), Duration::from_secs(1));
        assert_eq!(backoff.next_delay(), Duration::from_secs(2));
    }
}
