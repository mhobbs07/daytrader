//! Regression test for B-086 step 2: a stock split must not manufacture a
//! spurious pattern signal.
//!
//! This lives in `tests/` rather than as a unit test inside
//! `src/data/corporate_actions.rs` because it exercises the pattern
//! detectors (`daytrader::patterns`), which the `data` module is not
//! permitted to import (architecture boundary: `data` is infrastructure,
//! `patterns` is business logic — see `src/data/CLAUDE.md`-equivalent rules
//! in `.claude/rules/architecture.md`).
//!
//! # Scenario
//!
//! AAPL executed a real 4:1 forward split with ex-date 2020-08-31. A raw
//! (unadjusted) bar series spanning that date shows the pre-split day
//! trading around $400 and the post-split days trading around $100 — an
//! apparent ~75% single-step drop that is entirely a share-count artefact,
//! not a real price move. Fed to a pattern detector, this artefact is
//! indistinguishable from a genuine bearish reversal.
//!
//! This test builds exactly that raw series, confirms it fires a spurious
//! `evening_star` (bearish reversal) signal, then adjusts the same series
//! through `corporate_actions::adjust_candles` using the recorded split and
//! confirms the signal disappears — proving the adjustment removes the
//! artefact rather than just changing the numbers.

use chrono::{TimeZone, Utc};
use daytrader::brokers::traits::BarAdjustment;
use daytrader::brokers::types::{Price, Symbol};
use daytrader::data::corporate_actions::{ActionType, CorporateAction, adjust_candles};
use daytrader::events::types::Candle;
use daytrader::patterns::EveningStarDetector;
use daytrader::patterns::traits::PatternDetector;
use daytrader::patterns::types::PatternParams;
use rust_decimal::Decimal;

fn price(value: &str) -> Price {
    Price::new(value.parse::<Decimal>().unwrap()).unwrap()
}

/// Builds the raw, chronologically-ordered (oldest-first) three-bar window
/// straddling the split: a strong pre-split up day, a small post-split
/// doji on the ex-date, and a mild post-split pullback.
fn raw_candles_around_split(symbol: &Symbol) -> Vec<Candle> {
    vec![
        // Pre-split, 2020-08-28: raw prices still on the old (~$400) basis.
        Candle {
            symbol: symbol.clone(),
            timestamp: Utc.with_ymd_and_hms(2020, 8, 28, 20, 0, 0).unwrap(),
            open: price("396.00"),
            high: price("402.00"),
            low: price("394.00"),
            close: price("400.00"),
            volume: 1_000_000,
        },
        // Ex-date, 2020-08-31: split already executed overnight, so even
        // the raw feed reports post-split (~$100) prices from here on.
        Candle {
            symbol: symbol.clone(),
            timestamp: Utc.with_ymd_and_hms(2020, 8, 31, 20, 0, 0).unwrap(),
            open: price("100.60"),
            high: price("100.80"),
            low: price("100.50"),
            close: price("100.65"),
            volume: 4_000_000,
        },
        // Post-split, 2020-09-01: a mild, unremarkable pullback.
        Candle {
            symbol: symbol.clone(),
            timestamp: Utc.with_ymd_and_hms(2020, 9, 1, 20, 0, 0).unwrap(),
            open: price("100.40"),
            high: price("100.50"),
            low: price("99.00"),
            close: price("99.60"),
            volume: 3_500_000,
        },
    ]
}

#[test]
fn test_four_to_one_split_produces_no_evening_star_signal_after_adjustment() {
    let symbol = Symbol::new("AAPL").unwrap();
    let raw = raw_candles_around_split(&symbol);

    let split = CorporateAction {
        symbol: symbol.clone(),
        ex_date: chrono::NaiveDate::from_ymd_opt(2020, 8, 31).unwrap(),
        action_type: ActionType::Split { from: 1, to: 4 },
        source: "test".to_string(),
    };

    // The detector consumes candles newest-first (documented contract of
    // `PatternDetector::detect` and `PatternRegistry::detect_all`).
    let raw_newest_first: Vec<Candle> = raw.iter().rev().cloned().collect();

    let detector = EveningStarDetector::new();
    let params = PatternParams::default();

    // --- Raw series: the split artefact must read as a signal ---
    let raw_result = detector
        .detect(&raw_newest_first, &params)
        .expect("detection must not error on valid candle data");
    let raw_signal = raw_result.expect(
        "the raw, unadjusted series is expected to manufacture a spurious \
         evening_star signal from the split's apparent 75% drop — if this \
         fails, the fixture no longer reproduces the artefact this test \
         exists to catch",
    );
    assert_eq!(raw_signal.pattern_name.as_str(), "evening_star");
    assert!(
        raw_signal.confidence.value() >= params.min_confidence,
        "spurious signal must clear the detection threshold to be a real regression case"
    );

    // --- Adjusted series: the same detector, same window, must be silent ---
    let adjusted = adjust_candles(&raw, &[split], BarAdjustment::All)
        .expect("adjustment must succeed for a well-formed split");
    let adjusted_newest_first: Vec<Candle> = adjusted.iter().rev().cloned().collect();

    let adjusted_result = detector
        .detect(&adjusted_newest_first, &params)
        .expect("detection must not error on valid candle data");
    assert!(
        adjusted_result.is_none(),
        "adjusting for the split must remove the spurious evening_star signal, \
         got: {adjusted_result:?}"
    );
}

#[test]
fn test_adjustment_removes_the_price_discontinuity_the_signal_was_reading() {
    // Directly demonstrates the mechanism behind the previous test: the raw
    // series has a ~75% one-bar drop; the adjusted series does not.
    let symbol = Symbol::new("AAPL").unwrap();
    let raw = raw_candles_around_split(&symbol);
    let split = CorporateAction {
        symbol,
        ex_date: chrono::NaiveDate::from_ymd_opt(2020, 8, 31).unwrap(),
        action_type: ActionType::Split { from: 1, to: 4 },
        source: "test".to_string(),
    };

    let raw_drop = (raw[0].close.value() - raw[1].close.value()) / raw[0].close.value();
    assert!(
        raw_drop > Decimal::new(70, 2),
        "fixture must reproduce a ~75% raw drop, got {raw_drop}"
    );

    let adjusted = adjust_candles(&raw, &[split], BarAdjustment::All).unwrap();
    let adjusted_drop =
        (adjusted[0].close.value() - adjusted[1].close.value()).abs() / adjusted[0].close.value();
    assert!(
        adjusted_drop < Decimal::new(5, 2),
        "adjusted series must not show the split as a price move, got {adjusted_drop}"
    );
}
