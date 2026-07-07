//! US equity market session boundaries (America/New_York), DST-correct
//!
//! Regular NYSE/NASDAQ trading hours are 9:30 AM to 4:00 PM in the
//! `America/New_York` time zone, year-round. Because that zone observes
//! daylight saving time (UTC-5 in winter, UTC-4 in summer), a fixed UTC
//! offset is wrong for roughly half the year. This module is the single
//! canonical place that converts between UTC and the Eastern-time trading
//! session using the IANA tz database (via `chrono-tz`), so callers never
//! need to hardcode an offset.
//!
//! This module intentionally does not know about market holidays — pair it
//! with [`crate::utils::holidays::MarketHolidayCalendar`] when a caller also
//! needs to skip non-trading days.
//!
//! # Examples
//!
//! ```
//! use daytrader::utils::market_hours::{is_market_hours, market_open_utc, market_close_utc};
//! use chrono::{NaiveDate, TimeZone, Utc};
//!
//! // January 15, 2024 is EST (UTC-5): market opens at 14:30 UTC.
//! let winter_date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
//! let open = market_open_utc(winter_date).unwrap();
//! assert_eq!(open, Utc.with_ymd_and_hms(2024, 1, 15, 14, 30, 0).unwrap());
//!
//! // July 15, 2024 is EDT (UTC-4): market opens at 13:30 UTC.
//! let summer_date = NaiveDate::from_ymd_opt(2024, 7, 15).unwrap();
//! let open = market_open_utc(summer_date).unwrap();
//! assert_eq!(open, Utc.with_ymd_and_hms(2024, 7, 15, 13, 30, 0).unwrap());
//! ```

use chrono::{DateTime, LocalResult, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::America::New_York;
use thiserror::Error;

/// Regular market open time in Eastern time: 9:30 AM.
const MARKET_OPEN_ET: (u32, u32, u32) = (9, 30, 0);

/// Regular market close time in Eastern time: 4:00 PM.
const MARKET_CLOSE_ET: (u32, u32, u32) = (16, 0, 0);

/// Errors that can occur while computing market session boundaries.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum MarketHoursError {
    /// The supplied hour/minute/second do not form a valid `NaiveTime`.
    #[error("invalid session time {0}:{1}:{2}")]
    InvalidTime(u32, u32, u32),

    /// The local Eastern-time wall-clock instant does not map to a valid
    /// UTC instant (or maps ambiguously) — e.g. the nonexistent 2:30 AM
    /// during the spring-forward transition.
    #[error(
        "local time {0} on {1} is not a valid unambiguous America/New_York instant (DST transition)"
    )]
    AmbiguousOrInvalidLocalTime(NaiveTime, NaiveDate),
}

/// Returns the UTC instant corresponding to 9:30 AM America/New_York on `date`.
///
/// DST-correct: automatically UTC-5 in winter (EST) and UTC-4 in summer (EDT).
///
/// # Errors
///
/// Returns [`MarketHoursError`] if the composed local time is invalid, or if
/// it falls in the nonexistent hour during the spring-forward transition
/// (this cannot happen for 9:30 AM under current US DST rules, but the
/// check is exhaustive rather than assumed).
pub fn market_open_utc(date: NaiveDate) -> Result<DateTime<Utc>, MarketHoursError> {
    local_et_to_utc(date, MARKET_OPEN_ET, AmbiguousResolution::Earliest)
}

/// Returns the UTC instant corresponding to 4:00 PM America/New_York on `date`.
///
/// DST-correct: automatically UTC-5 in winter (EST) and UTC-4 in summer (EDT).
///
/// # Errors
///
/// Returns [`MarketHoursError`] if the composed local time is invalid or
/// falls in a DST transition gap.
pub fn market_close_utc(date: NaiveDate) -> Result<DateTime<Utc>, MarketHoursError> {
    local_et_to_utc(date, MARKET_CLOSE_ET, AmbiguousResolution::Latest)
}

/// Returns `true` if `instant` falls within regular trading hours
/// (9:30 AM–4:00 PM America/New_York) on its Eastern-time calendar date.
///
/// This does not account for market holidays or weekends — combine with
/// [`crate::utils::holidays::MarketHolidayCalendar::is_business_day`] for a
/// complete "is the market open right now" check.
///
/// # Errors
///
/// Returns [`MarketHoursError`] if the session boundaries for the
/// Eastern-time calendar date of `instant` cannot be computed.
pub fn is_market_hours(instant: DateTime<Utc>) -> Result<bool, MarketHoursError> {
    let et_date = instant.with_timezone(&New_York).date_naive();
    let open = market_open_utc(et_date)?;
    let close = market_close_utc(et_date)?;
    Ok(instant >= open && instant < close)
}

/// How to resolve an ambiguous (fall-back, repeated-hour) local time.
///
/// The conservative direction depends on which session boundary is being
/// computed: the *earliest* instant for an open keeps the session from
/// starting late, while the *latest* instant for a close keeps it from
/// ending early. Neither branch is reachable for the current 9:30/16:00
/// constants (the US fall-back window is 1:00–2:00 AM ET), but the helper
/// must stay correct if it is ever reused for other wall-clock times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AmbiguousResolution {
    /// Resolve to the first (pre-transition) occurrence.
    Earliest,
    /// Resolve to the second (post-transition) occurrence.
    Latest,
}

/// Converts a `(hour, minute, second)` wall-clock time on `date` in
/// America/New_York to a UTC instant, resolving DST via the IANA tz database.
fn local_et_to_utc(
    date: NaiveDate,
    (hour, minute, second): (u32, u32, u32),
    ambiguous: AmbiguousResolution,
) -> Result<DateTime<Utc>, MarketHoursError> {
    let naive_time = NaiveTime::from_hms_opt(hour, minute, second)
        .ok_or(MarketHoursError::InvalidTime(hour, minute, second))?;
    let naive_dt = date.and_time(naive_time);

    match New_York.from_local_datetime(&naive_dt) {
        LocalResult::Single(et_dt) => Ok(et_dt.with_timezone(&Utc)),
        LocalResult::Ambiguous(earliest, latest) => Ok(match ambiguous {
            AmbiguousResolution::Earliest => earliest.with_timezone(&Utc),
            AmbiguousResolution::Latest => latest.with_timezone(&Utc),
        }),
        LocalResult::None => Err(MarketHoursError::AmbiguousOrInvalidLocalTime(
            naive_time, date,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_local_et_to_utc_ambiguous_fall_back_resolves_by_direction() {
        // 2026-11-01 is the US fall-back date: 1:30 AM ET occurs twice
        // (once at UTC-4, once at UTC-5, one hour apart).
        let date = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let earliest = local_et_to_utc(date, (1, 30, 0), AmbiguousResolution::Earliest).unwrap();
        let latest = local_et_to_utc(date, (1, 30, 0), AmbiguousResolution::Latest).unwrap();
        assert_eq!(latest - earliest, chrono::Duration::hours(1));
        assert!(earliest < latest);
    }

    #[test]
    fn test_local_et_to_utc_spring_forward_gap_is_error() {
        // 2026-03-08 is the US spring-forward date: 2:30 AM ET does not exist.
        let date = NaiveDate::from_ymd_opt(2026, 3, 8).unwrap();
        let result = local_et_to_utc(date, (2, 30, 0), AmbiguousResolution::Earliest);
        assert!(matches!(
            result,
            Err(MarketHoursError::AmbiguousOrInvalidLocalTime(_, _))
        ));
    }

    #[test]
    fn test_market_open_utc_winter_est_is_utc_minus_5() {
        // January 15, 2024 (Monday) — EST, UTC-5. 9:30 ET = 14:30 UTC.
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let open = market_open_utc(date).unwrap();
        assert_eq!(open, Utc.with_ymd_and_hms(2024, 1, 15, 14, 30, 0).unwrap());
    }

    #[test]
    fn test_market_close_utc_winter_est_is_utc_minus_5() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let close = market_close_utc(date).unwrap();
        assert_eq!(close, Utc.with_ymd_and_hms(2024, 1, 15, 21, 0, 0).unwrap());
    }

    #[test]
    fn test_market_open_utc_summer_edt_is_utc_minus_4() {
        // July 15, 2024 (Monday) — EDT, UTC-4. 9:30 ET = 13:30 UTC.
        let date = NaiveDate::from_ymd_opt(2024, 7, 15).unwrap();
        let open = market_open_utc(date).unwrap();
        assert_eq!(open, Utc.with_ymd_and_hms(2024, 7, 15, 13, 30, 0).unwrap());
    }

    #[test]
    fn test_market_close_utc_summer_edt_is_utc_minus_4() {
        let date = NaiveDate::from_ymd_opt(2024, 7, 15).unwrap();
        let close = market_close_utc(date).unwrap();
        assert_eq!(close, Utc.with_ymd_and_hms(2024, 7, 15, 20, 0, 0).unwrap());
    }

    #[test]
    fn test_spring_forward_sunday_boundaries_still_resolve() {
        // 2024 spring-forward was Sunday March 10 (2:00 AM -> 3:00 AM local).
        // 9:30 AM and 4:00 PM ET are both outside the nonexistent 2-3 AM
        // gap, so both must resolve without error, using EDT (UTC-4) since
        // the transition happens at 2 AM, before market open.
        let date = NaiveDate::from_ymd_opt(2024, 3, 10).unwrap();
        let open = market_open_utc(date).unwrap();
        let close = market_close_utc(date).unwrap();
        assert_eq!(open, Utc.with_ymd_and_hms(2024, 3, 10, 13, 30, 0).unwrap());
        assert_eq!(close, Utc.with_ymd_and_hms(2024, 3, 10, 20, 0, 0).unwrap());
    }

    #[test]
    fn test_fall_back_sunday_boundaries_still_resolve() {
        // 2024 fall-back was Sunday November 3 (2:00 AM -> 1:00 AM local,
        // repeating the 1-2 AM hour). 9:30 AM and 4:00 PM ET are unaffected
        // by the ambiguous hour and use EST (UTC-5) after the transition.
        let date = NaiveDate::from_ymd_opt(2024, 11, 3).unwrap();
        let open = market_open_utc(date).unwrap();
        let close = market_close_utc(date).unwrap();
        assert_eq!(open, Utc.with_ymd_and_hms(2024, 11, 3, 14, 30, 0).unwrap());
        assert_eq!(close, Utc.with_ymd_and_hms(2024, 11, 3, 21, 0, 0).unwrap());
    }

    #[test]
    fn test_is_market_hours_boundary_minutes_open_winter() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 16).unwrap();
        // 9:29 ET (14:29 UTC) is before open.
        let before_open = Utc.with_ymd_and_hms(2024, 1, 16, 14, 29, 0).unwrap();
        assert!(!is_market_hours(before_open).unwrap());
        // 9:30 ET (14:30 UTC) is exactly open — inclusive.
        let at_open = Utc.with_ymd_and_hms(2024, 1, 16, 14, 30, 0).unwrap();
        assert!(is_market_hours(at_open).unwrap());
    }

    #[test]
    fn test_is_market_hours_boundary_minutes_close_winter() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 16).unwrap();
        let _ = date;
        // 15:59 ET (20:59 UTC) is still open.
        let before_close = Utc.with_ymd_and_hms(2024, 1, 16, 20, 59, 0).unwrap();
        assert!(is_market_hours(before_close).unwrap());
        // 16:00 ET (21:00 UTC) is exactly close — exclusive.
        let at_close = Utc.with_ymd_and_hms(2024, 1, 16, 21, 0, 0).unwrap();
        assert!(!is_market_hours(at_close).unwrap());
    }

    #[test]
    fn test_is_market_hours_boundary_minutes_summer() {
        // July 16, 2024 — EDT, UTC-4. Open 13:30 UTC, close 20:00 UTC.
        let before_open = Utc.with_ymd_and_hms(2024, 7, 16, 13, 29, 0).unwrap();
        assert!(!is_market_hours(before_open).unwrap());
        let at_open = Utc.with_ymd_and_hms(2024, 7, 16, 13, 30, 0).unwrap();
        assert!(is_market_hours(at_open).unwrap());
        let before_close = Utc.with_ymd_and_hms(2024, 7, 16, 19, 59, 0).unwrap();
        assert!(is_market_hours(before_close).unwrap());
        let at_close = Utc.with_ymd_and_hms(2024, 7, 16, 20, 0, 0).unwrap();
        assert!(!is_market_hours(at_close).unwrap());
    }

    #[test]
    fn test_is_market_hours_outside_session_both_seasons() {
        // Midnight UTC is always outside trading hours regardless of season.
        let winter_midnight = Utc.with_ymd_and_hms(2024, 1, 16, 0, 0, 0).unwrap();
        assert!(!is_market_hours(winter_midnight).unwrap());
        let summer_midnight = Utc.with_ymd_and_hms(2024, 7, 16, 0, 0, 0).unwrap();
        assert!(!is_market_hours(summer_midnight).unwrap());
    }

    #[test]
    fn test_market_open_before_close_same_day() {
        let date = NaiveDate::from_ymd_opt(2024, 5, 1).unwrap();
        let open = market_open_utc(date).unwrap();
        let close = market_close_utc(date).unwrap();
        assert!(open < close);
        assert_eq!(open.with_timezone(&New_York).date_naive(), date);
        assert_eq!(close.with_timezone(&New_York).date_naive(), date);
    }

    #[test]
    fn test_session_length_is_six_and_half_hours() {
        let date = NaiveDate::from_ymd_opt(2024, 6, 10).unwrap();
        let open = market_open_utc(date).unwrap();
        let close = market_close_utc(date).unwrap();
        let duration = close - open;
        assert_eq!(duration.num_minutes(), 6 * 60 + 30);
    }

    #[test]
    fn test_market_hours_error_display_is_descriptive() {
        let err = MarketHoursError::InvalidTime(25, 61, 0);
        let msg = err.to_string();
        assert!(msg.contains("25"));
        assert!(msg.contains("61"));
    }

    #[test]
    fn test_open_date_matches_requested_date_across_year_boundary() {
        // Sanity check around a year boundary to guard against off-by-one
        // date shifts when converting local ET to UTC.
        let date = NaiveDate::from_ymd_opt(2023, 12, 29).unwrap();
        let open = market_open_utc(date).unwrap();
        assert_eq!(open.year(), 2023);
        assert_eq!(open.with_timezone(&New_York).date_naive(), date);
    }
}
