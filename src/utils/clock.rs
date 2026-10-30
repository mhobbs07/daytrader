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

/// Clock advanced explicitly by its owner, for historical replay
///
/// The backtester sets this to each candle's timestamp so every time-aware
/// component (market-hours checks, order timestamps) observes replay time
/// rather than wall-clock time. Backed by an atomic so `now()` stays a
/// cheap sync call shareable across tasks.
#[derive(Debug, Clone, Default)]
pub struct SteppedClock {
    millis: std::sync::Arc<std::sync::atomic::AtomicI64>,
}

impl SteppedClock {
    /// Creates a clock starting at `instant`.
    pub fn new(instant: DateTime<Utc>) -> Self {
        let clock = Self::default();
        clock.set(instant);
        clock
    }

    /// Moves the clock to `instant`. Clones observe the change immediately.
    pub fn set(&self, instant: DateTime<Utc>) {
        self.millis.store(
            instant.timestamp_millis(),
            std::sync::atomic::Ordering::SeqCst,
        );
    }
}

impl Clock for SteppedClock {
    fn now(&self) -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp_millis(
            self.millis.load(std::sync::atomic::Ordering::SeqCst),
        )
        .unwrap_or_else(Utc::now)
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

    #[test]
    fn test_stepped_clock_set_is_visible_to_clones() {
        let t1 = Utc.with_ymd_and_hms(2024, 6, 12, 15, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2024, 6, 12, 15, 1, 0).unwrap();

        let clock = SteppedClock::new(t1);
        let observer = clock.clone();
        assert_eq!(observer.now(), t1);

        clock.set(t2);
        assert_eq!(observer.now(), t2, "clones share the same instant");
    }
}
