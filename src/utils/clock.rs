//! Injectable time source
//!
//! Production code that needs "now" should take a [`Clock`] instead of
//! calling [`Utc::now`] directly, so tests can pin time deterministically
//! (e.g. market-hours checks that would otherwise depend on when the test
//! suite runs).

use chrono::{DateTime, Utc};

/// A source of the current UTC time.
pub trait Clock: Send + Sync + std::fmt::Debug {
    /// Returns the current UTC time according to this clock.
    fn now(&self) -> DateTime<Utc>;
}

/// Production clock backed by the system time.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Clock that always returns the same instant. Intended for tests.
#[derive(Debug, Clone, Copy)]
pub struct FixedClock {
    instant: DateTime<Utc>,
}

impl FixedClock {
    /// Creates a clock frozen at `instant`.
    pub fn new(instant: DateTime<Utc>) -> Self {
        Self { instant }
    }
}

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.instant
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_system_clock_now_tracks_real_time() {
        let before = Utc::now();
        let observed = SystemClock.now();
        let after = Utc::now();
        assert!(observed >= before);
        assert!(observed <= after);
    }

    #[test]
    fn test_fixed_clock_now_returns_pinned_instant() {
        let instant = Utc.with_ymd_and_hms(2024, 6, 12, 15, 0, 0).unwrap();
        let clock = FixedClock::new(instant);
        assert_eq!(clock.now(), instant);
        // Stays pinned across calls
        assert_eq!(clock.now(), instant);
    }
}
