//! Corporate-action factor table and query-time bar adjustment
//!
//! # Why this exists
//!
//! Vendor-adjusted bars ("give me split/dividend-adjusted history") are
//! adjusted *as of the moment of the request*. Re-fetching the exact same
//! historical window after a later split returns different numbers, so a
//! backtest built on adjusted bars pulled today is not reproducible against
//! one pulled next month. Worse, live bars are necessarily stored raw
//! (there is no vendor to ask "adjusted for a split that hasn't happened
//! yet"), so splicing them onto adjusted history produces a discontinuity
//! at any split that occurred during the stored window.
//!
//! The fix is the standard one: **store bars raw, store corporate actions
//! separately as a factor table, and adjust at query time.** This module is
//! that factor table plus the adjustment function. It never rewrites stored
//! prices in place — an in-place backfill would silently rewrite history
//! and destroy reproducibility the same way an unpinned vendor request
//! does.
//!
//! # Direction convention
//!
//! [`AdjustmentFactors`] computes, for a bar dated `D`, the product of every
//! action whose `ex_date` is *strictly after* `D`. An action on or before
//! `D` has already happened as of that bar, so it needs no adjustment — the
//! bar's raw price already reflects it. An action after `D` has not
//! happened yet as of that bar, so the bar's raw price is still on the old
//! basis and must be restated onto the current one. This is why the factor
//! for the most recent bar in a series is always the identity (nothing
//! after it to adjust for), while older bars accumulate one factor per
//! action that has landed since.

use crate::brokers::traits::{BarAdjustment, Candle, CorporateActionAnnouncement};
use crate::brokers::types::{Price, Symbol};
use crate::utils::errors::{AppError, CorporateActionError, DatabaseError, Result};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use sqlx::{Row, SqlitePool};

/// The kind of corporate action and its economically-relevant parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionType {
    /// A stock split expressed as a ratio: `from` old shares become `to`
    /// new shares. A forward 4:1 split is `{ from: 1, to: 4 }`; a 1:10
    /// reverse split is `{ from: 10, to: 1 }` — modeling the ratio directly,
    /// rather than a single multiplier, is what makes reverse splits
    /// representable without a separate direction flag.
    Split {
        /// Old shares in the ratio
        from: u32,
        /// New shares in the ratio
        to: u32,
    },
    /// A cash dividend of `amount` per share.
    CashDividend {
        /// Cash amount paid per share
        amount: Decimal,
    },
}

impl ActionType {
    /// The database discriminator for this action's type.
    #[inline]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Split { .. } => "split",
            Self::CashDividend { .. } => "cash_dividend",
        }
    }

    /// The multiplicative price factor a split contributes to
    /// back-adjustment: multiplying a raw pre-split price by `from/to`
    /// restates it in current-share terms.
    ///
    /// # Errors
    ///
    /// Returns [`CorporateActionError::InvalidSplitRatio`] if either side of
    /// the ratio is zero.
    fn split_price_factor(from: u32, to: u32) -> Result<Decimal> {
        if from == 0 || to == 0 {
            return Err(CorporateActionError::InvalidSplitRatio { from, to }.into());
        }
        Ok(Decimal::from(from) / Decimal::from(to))
    }
}

/// A single corporate action affecting a symbol's historical price series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorporateAction {
    /// The affected symbol
    pub symbol: Symbol,
    /// The date the action takes effect
    pub ex_date: NaiveDate,
    /// The kind of action and its parameters
    pub action_type: ActionType,
    /// Where this record came from (e.g. `"alpaca"`), for provenance/audit
    pub source: String,
}

impl CorporateAction {
    /// Builds a domain corporate action from a broker's wire-format
    /// announcement, validating the economically-meaningful invariants a
    /// broker response is not trusted to have already enforced.
    ///
    /// # Errors
    ///
    /// Returns [`CorporateActionError::InvalidSplitRatio`] for a split with
    /// a zero share count on either side, or
    /// [`CorporateActionError::InvalidDividendAmount`] for a non-positive
    /// dividend.
    pub fn from_announcement(
        symbol: Symbol,
        announcement: CorporateActionAnnouncement,
        source: impl Into<String>,
    ) -> Result<Self> {
        let (ex_date, action_type) = match announcement {
            CorporateActionAnnouncement::Split(split) => {
                if split.from == 0 || split.to == 0 {
                    return Err(CorporateActionError::InvalidSplitRatio {
                        from: split.from,
                        to: split.to,
                    }
                    .into());
                }
                (
                    split.ex_date,
                    ActionType::Split {
                        from: split.from,
                        to: split.to,
                    },
                )
            }
            CorporateActionAnnouncement::CashDividend(dividend) => {
                if dividend.amount_per_share <= Decimal::ZERO {
                    return Err(CorporateActionError::InvalidDividendAmount {
                        amount: dividend.amount_per_share,
                    }
                    .into());
                }
                (
                    dividend.ex_date,
                    ActionType::CashDividend {
                        amount: dividend.amount_per_share,
                    },
                )
            }
        };

        Ok(Self {
            symbol,
            ex_date,
            action_type,
            source: source.into(),
        })
    }
}

/// Computes cumulative back-adjustment factors from a symbol's corporate
/// actions. See the module docs for the direction convention.
pub struct AdjustmentFactors;

impl AdjustmentFactors {
    /// Cumulative split-only factor for `as_of_date`: the product of
    /// `from/to` for every split with `ex_date > as_of_date`.
    ///
    /// # Errors
    ///
    /// Returns [`CorporateActionError::InvalidSplitRatio`] if any
    /// contributing split has a zero share count (should not occur for
    /// actions that passed [`CorporateAction::from_announcement`] or the
    /// `corporate_actions` table's CHECK constraint, but is not assumed).
    pub fn split_factor(actions: &[CorporateAction], as_of_date: NaiveDate) -> Result<Decimal> {
        actions
            .iter()
            .filter(|action| action.ex_date > as_of_date)
            .try_fold(Decimal::ONE, |acc, action| match action.action_type {
                ActionType::Split { from, to } => {
                    Ok(acc * ActionType::split_price_factor(from, to)?)
                }
                ActionType::CashDividend { .. } => Ok(acc),
            })
    }

    /// Cumulative dividend-only factor for `as_of_date`.
    ///
    /// Each dividend contributes `(prior_close - amount) / prior_close`,
    /// the standard ex-dividend adjustment, where `prior_close` is the raw
    /// close on the trading session immediately before that dividend's
    /// `ex_date`.
    ///
    /// `prior_close` resolves that reference price. A dividend whose
    /// reference price cannot be resolved (e.g. the raw series supplied by
    /// the caller does not extend far enough back) is skipped rather than
    /// erroring — a documented limitation, not silent data loss: callers
    /// own the raw series and can widen its range if this matters.
    ///
    /// # Errors
    ///
    /// Returns [`CorporateActionError::InvalidDividendAmount`] if any
    /// contributing dividend has a non-positive amount.
    pub fn dividend_factor(
        actions: &[CorporateAction],
        as_of_date: NaiveDate,
        prior_close: impl Fn(NaiveDate) -> Option<Decimal>,
    ) -> Result<Decimal> {
        actions
            .iter()
            .filter(|action| action.ex_date > as_of_date)
            .try_fold(Decimal::ONE, |acc, action| match action.action_type {
                ActionType::CashDividend { amount } => {
                    if amount <= Decimal::ZERO {
                        return Err(CorporateActionError::InvalidDividendAmount { amount }.into());
                    }
                    match prior_close(action.ex_date) {
                        Some(close) if close > Decimal::ZERO => Ok(acc * (close - amount) / close),
                        _ => Ok(acc),
                    }
                }
                ActionType::Split { .. } => Ok(acc),
            })
    }

    /// The combined factor for `as_of_date` under the given
    /// [`BarAdjustment`] mode: [`BarAdjustment::Raw`] is always the
    /// identity, [`BarAdjustment::Split`]/[`BarAdjustment::Dividend`] apply
    /// only their namesake, and [`BarAdjustment::All`] (and any future
    /// `#[non_exhaustive]` variant, treated the same way as the safest
    /// default) applies both.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`Self::split_factor`] and
    /// [`Self::dividend_factor`].
    pub fn factor_for(
        actions: &[CorporateAction],
        as_of_date: NaiveDate,
        adjustment: BarAdjustment,
        prior_close: impl Fn(NaiveDate) -> Option<Decimal>,
    ) -> Result<Decimal> {
        match adjustment {
            BarAdjustment::Raw => Ok(Decimal::ONE),
            BarAdjustment::Split => Self::split_factor(actions, as_of_date),
            BarAdjustment::Dividend => Self::dividend_factor(actions, as_of_date, prior_close),
            // `BarAdjustment` is `#[non_exhaustive]`: any future variant we
            // don't yet know about falls in here too, and is treated the
            // same as `All` — the safest default, matching
            // `BarAdjustment::default()`.
            BarAdjustment::All => {
                let split = Self::split_factor(actions, as_of_date)?;
                let dividend = Self::dividend_factor(actions, as_of_date, prior_close)?;
                Ok(split * dividend)
            }
        }
    }
}

/// Applies corporate-action adjustment to a raw candle series.
///
/// This is the query-time half of the raw-plus-factors design: bars are
/// stored once, as fetched (`BarAdjustment::Raw`), and this function derives
/// any adjusted view from them on demand rather than the storage layer ever
/// holding a second, mutated copy. Feeding the same raw bars and factor
/// table through this function always yields the same result, which is what
/// makes a backtest built on it reproducible.
///
/// `adjustment = BarAdjustment::Raw` and an empty `actions` slice both
/// return the input unchanged (cloned).
///
/// # Errors
///
/// Propagates [`AdjustmentFactors`] factor-computation failures, and
/// [`Price`] construction failures if a computed adjustment would drive a
/// price to zero or negative (which would indicate corrupt input, not a
/// real market condition).
pub fn adjust_candles(
    raw: &[Candle],
    actions: &[CorporateAction],
    adjustment: BarAdjustment,
) -> Result<Vec<Candle>> {
    if adjustment == BarAdjustment::Raw || actions.is_empty() {
        return Ok(raw.to_vec());
    }

    // A sorted date -> raw close index, built once, so dividend reference-
    // price lookups don't rescan the series per action per bar.
    let mut closes_by_date: Vec<(NaiveDate, Decimal)> = raw
        .iter()
        .map(|candle| (candle.timestamp.date_naive(), candle.close.value()))
        .collect();
    closes_by_date.sort_by_key(|(date, _)| *date);

    let prior_close = |ex_date: NaiveDate| -> Option<Decimal> {
        closes_by_date
            .iter()
            .rev()
            .find(|(date, _)| *date < ex_date)
            .map(|(_, close)| *close)
    };

    raw.iter()
        .map(|candle| {
            let as_of_date = candle.timestamp.date_naive();
            let factor =
                AdjustmentFactors::factor_for(actions, as_of_date, adjustment, prior_close)?;
            scale_candle(candle, factor)
        })
        .collect()
}

/// Scales a candle's OHLC by `factor`, leaving volume untouched.
///
/// Volume is intentionally not rescaled: doing so correctly requires
/// tracking split-adjusted share counts, a separate concern this pass does
/// not attempt. Downstream consumers that need adjusted volume should treat
/// this as a known simplification.
fn scale_candle(candle: &Candle, factor: Decimal) -> Result<Candle> {
    if factor == Decimal::ONE {
        return Ok(candle.clone());
    }

    let scale = |price: Price| -> Result<Price> {
        Price::new(price.value() * factor).map_err(|e| AppError::Other(anyhow::anyhow!(e)))
    };

    Ok(Candle {
        symbol: candle.symbol.clone(),
        timestamp: candle.timestamp,
        open: scale(candle.open)?,
        high: scale(candle.high)?,
        low: scale(candle.low)?,
        close: scale(candle.close)?,
        volume: candle.volume,
    })
}

/// Persistent repository for corporate actions.
///
/// # Examples
///
/// ```no_run
/// use daytrader::data::corporate_actions::CorporateActionRepository;
/// use sqlx::SqlitePool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let pool = SqlitePool::connect("sqlite::memory:").await?;
/// let repo = CorporateActionRepository::new(pool);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct CorporateActionRepository {
    pool: SqlitePool,
}

impl CorporateActionRepository {
    /// Creates a new corporate-action repository.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Inserts a corporate action.
    ///
    /// Idempotent: a duplicate `(symbol, ex_date, action_type)` is ignored
    /// rather than erroring, so a re-run of an ingestion job is safe.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::QueryFailed`] if the insert fails for a
    /// reason other than the uniqueness conflict it deliberately ignores.
    pub async fn insert(&self, action: &CorporateAction) -> Result<()> {
        match action.action_type {
            ActionType::Split { from, to } => {
                sqlx::query(
                    r"
                    INSERT INTO corporate_actions
                        (symbol, ex_date, action_type, split_from, split_to, source)
                    VALUES (?, ?, 'split', ?, ?, ?)
                    ON CONFLICT(symbol, ex_date, action_type) DO NOTHING
                    ",
                )
                .bind(action.symbol.as_str())
                .bind(action.ex_date)
                .bind(i64::from(from))
                .bind(i64::from(to))
                .bind(&action.source)
                .execute(&self.pool)
                .await
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
            }
            ActionType::CashDividend { amount } => {
                // Bound as f64, matching the column's REAL affinity (see the
                // migration's B-087 note): SQLite would coerce a numeric
                // TEXT bind to REAL storage anyway, so binding the REAL
                // value directly keeps the write and the read (below) using
                // the same representation instead of round-tripping through
                // an intermediate string.
                let amount_f64 = amount.to_f64().ok_or_else(|| {
                    AppError::Other(anyhow::anyhow!(
                        "cash dividend amount {amount} has no finite f64 representation"
                    ))
                })?;
                sqlx::query(
                    r"
                    INSERT INTO corporate_actions
                        (symbol, ex_date, action_type, cash_amount, source)
                    VALUES (?, ?, 'cash_dividend', ?, ?)
                    ON CONFLICT(symbol, ex_date, action_type) DO NOTHING
                    ",
                )
                .bind(action.symbol.as_str())
                .bind(action.ex_date)
                .bind(amount_f64)
                .bind(&action.source)
                .execute(&self.pool)
                .await
                .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Fetches every action for `symbol` with `ex_date` in `[start, end]`
    /// (inclusive), ordered oldest-first.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::QueryFailed`] if the query fails.
    pub async fn get_actions(
        &self,
        symbol: &Symbol,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<CorporateAction>> {
        let rows = sqlx::query(
            r"
            SELECT symbol, ex_date, action_type, split_from, split_to, cash_amount, source
            FROM corporate_actions
            WHERE symbol = ? AND ex_date >= ? AND ex_date <= ?
            ORDER BY ex_date ASC
            ",
        )
        .bind(symbol.as_str())
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        rows.iter().map(row_to_action).collect()
    }

    /// Fetches every action on record for `symbol`, ordered oldest-first.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::QueryFailed`] if the query fails.
    pub async fn get_all_actions(&self, symbol: &Symbol) -> Result<Vec<CorporateAction>> {
        let rows = sqlx::query(
            r"
            SELECT symbol, ex_date, action_type, split_from, split_to, cash_amount, source
            FROM corporate_actions
            WHERE symbol = ?
            ORDER BY ex_date ASC
            ",
        )
        .bind(symbol.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

        rows.iter().map(row_to_action).collect()
    }
}

/// Converts a database row into a [`CorporateAction`].
fn row_to_action(row: &sqlx::sqlite::SqliteRow) -> Result<CorporateAction> {
    let db_err = |e: sqlx::Error| AppError::Database(DatabaseError::QueryFailed(e.to_string()));

    let symbol_str: String = row.try_get("symbol").map_err(db_err)?;
    let symbol = Symbol::new(symbol_str).map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;

    let ex_date: NaiveDate = row.try_get("ex_date").map_err(db_err)?;
    let action_type_str: String = row.try_get("action_type").map_err(db_err)?;
    let source: String = row.try_get("source").map_err(db_err)?;

    let action_type = match action_type_str.as_str() {
        "split" => {
            let from: i64 = row.try_get("split_from").map_err(db_err)?;
            let to: i64 = row.try_get("split_to").map_err(db_err)?;
            let from = u32::try_from(from)
                .map_err(|e| AppError::Other(anyhow::anyhow!("invalid split_from: {e}")))?;
            let to = u32::try_from(to)
                .map_err(|e| AppError::Other(anyhow::anyhow!("invalid split_to: {e}")))?;
            ActionType::Split { from, to }
        }
        "cash_dividend" => {
            // The column has REAL affinity (see the migration's B-087
            // note), so it must be decoded as f64, not String — SQLite
            // has already coerced whatever was written into REAL storage.
            let amount_f64: f64 = row.try_get("cash_amount").map_err(db_err)?;
            let amount = Decimal::from_f64_retain(amount_f64).ok_or_else(|| {
                AppError::Database(DatabaseError::QueryFailed(format!(
                    "non-finite cash_amount in database: {amount_f64}"
                )))
            })?;
            ActionType::CashDividend { amount }
        }
        other => {
            return Err(AppError::Database(DatabaseError::QueryFailed(format!(
                "unknown corporate action type in database: {other}"
            ))));
        }
    };

    Ok(CorporateAction {
        symbol,
        ex_date,
        action_type,
        source,
    })
}

/// Converts a raw `f64` price ratio into `Decimal`, used only in tests where
/// fixture values are easiest to express as float literals.
#[cfg(test)]
fn decimal(value: f64) -> Decimal {
    Decimal::from_f64_retain(value).expect("finite test literal")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::traits::{CashDividendAnnouncement, SplitAnnouncement};
    use chrono::TimeZone;
    use chrono::Utc;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn symbol(s: &str) -> Symbol {
        Symbol::new(s).unwrap()
    }

    fn forward_split(symbol: Symbol, ex_date: NaiveDate, from: u32, to: u32) -> CorporateAction {
        CorporateAction {
            symbol,
            ex_date,
            action_type: ActionType::Split { from, to },
            source: "test".to_string(),
        }
    }

    fn dividend(symbol: Symbol, ex_date: NaiveDate, amount: f64) -> CorporateAction {
        CorporateAction {
            symbol,
            ex_date,
            action_type: ActionType::CashDividend {
                amount: decimal(amount),
            },
            source: "test".to_string(),
        }
    }

    fn candle(
        symbol: Symbol,
        ts: chrono::DateTime<Utc>,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
    ) -> Candle {
        Candle {
            symbol,
            timestamp: ts,
            open: Price::new(decimal(open)).unwrap(),
            high: Price::new(decimal(high)).unwrap(),
            low: Price::new(decimal(low)).unwrap(),
            close: Price::new(decimal(close)).unwrap(),
            volume: 1000,
        }
    }

    // ------------------------------------------------------------------
    // Split factor: direction, cumulative, reverse
    // ------------------------------------------------------------------

    #[test]
    fn test_split_factor_forward_split_divides_prior_price() {
        // A 4:1 forward split (1 share -> 4 shares): a bar dated before the
        // ex-date must be divided by 4 to sit on the post-split basis.
        let sym = symbol("AAPL");
        let actions = vec![forward_split(sym.clone(), date(2020, 8, 31), 1, 4)];

        let factor = AdjustmentFactors::split_factor(&actions, date(2020, 8, 30)).unwrap();
        assert_eq!(factor, Decimal::new(25, 2), "1/4 = 0.25");
    }

    #[test]
    fn test_split_factor_reverse_split_multiplies_prior_price() {
        // A 1:10 reverse split (10 shares -> 1 share): a bar before the
        // ex-date must be multiplied by 10.
        let sym = symbol("XYZ");
        let actions = vec![forward_split(sym.clone(), date(2024, 1, 15), 10, 1)];

        let factor = AdjustmentFactors::split_factor(&actions, date(2024, 1, 14)).unwrap();
        assert_eq!(factor, Decimal::from(10), "10/1 = 10");
    }

    #[test]
    fn test_split_factor_ignores_actions_on_or_before_as_of_date() {
        // The direction convention is ex_date > D, strictly. A bar dated
        // exactly on (or after) the ex-date already reflects the split.
        let sym = symbol("AAPL");
        let actions = vec![forward_split(sym.clone(), date(2020, 8, 31), 1, 4)];

        let on_ex_date = AdjustmentFactors::split_factor(&actions, date(2020, 8, 31)).unwrap();
        let after_ex_date = AdjustmentFactors::split_factor(&actions, date(2020, 9, 1)).unwrap();

        assert_eq!(
            on_ex_date,
            Decimal::ONE,
            "bar on the ex-date needs no adjustment"
        );
        assert_eq!(
            after_ex_date,
            Decimal::ONE,
            "bar after the ex-date needs no adjustment"
        );
    }

    #[test]
    fn test_split_factor_cumulative_across_multiple_sequential_splits() {
        // Two splits after `as_of_date` must both contribute: a 2:1 split
        // followed later by another 2:1 split is a cumulative 4x.
        let sym = symbol("TSLA");
        let actions = vec![
            forward_split(sym.clone(), date(2020, 8, 31), 1, 2),
            forward_split(sym.clone(), date(2022, 8, 25), 1, 2),
        ];

        let factor = AdjustmentFactors::split_factor(&actions, date(2020, 1, 1)).unwrap();
        assert_eq!(factor, Decimal::new(25, 2), "0.5 * 0.5 = 0.25");

        // A bar dated between the two splits only picks up the second one.
        let between = AdjustmentFactors::split_factor(&actions, date(2021, 1, 1)).unwrap();
        assert_eq!(between, Decimal::new(5, 1), "only the second split applies");
    }

    #[test]
    fn test_split_factor_rejects_zero_denominator() {
        let sym = symbol("AAPL");
        let actions = vec![forward_split(sym, date(2020, 8, 31), 1, 0)];

        let result = AdjustmentFactors::split_factor(&actions, date(2020, 1, 1));
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // Dividend factor
    // ------------------------------------------------------------------

    #[test]
    fn test_dividend_factor_applies_ex_dividend_ratio() {
        let sym = symbol("KO");
        let actions = vec![dividend(sym.clone(), date(2024, 3, 15), 0.485)];

        // Prior close of $60.00 the day before the ex-date.
        let factor = AdjustmentFactors::dividend_factor(&actions, date(2024, 1, 1), |ex_date| {
            assert_eq!(ex_date, date(2024, 3, 15));
            Some(decimal(60.0))
        })
        .unwrap();

        // (60.00 - 0.485) / 60.00
        let expected = (decimal(60.0) - decimal(0.485)) / decimal(60.0);
        assert_eq!(factor, expected);
        assert!(
            factor < Decimal::ONE,
            "a dividend factor discounts the price"
        );
    }

    #[test]
    fn test_dividend_factor_skips_when_prior_close_unresolvable() {
        let sym = symbol("KO");
        let actions = vec![dividend(sym, date(2024, 3, 15), 0.485)];

        let factor =
            AdjustmentFactors::dividend_factor(&actions, date(2024, 1, 1), |_| None).unwrap();
        assert_eq!(
            factor,
            Decimal::ONE,
            "an unresolvable reference price must not silently corrupt the series"
        );
    }

    #[test]
    fn test_dividend_factor_ignores_actions_on_or_before_as_of_date() {
        let sym = symbol("KO");
        let actions = vec![dividend(sym, date(2024, 3, 15), 0.485)];

        let factor = AdjustmentFactors::dividend_factor(&actions, date(2024, 3, 15), |_| {
            Some(decimal(60.0))
        })
        .unwrap();
        assert_eq!(factor, Decimal::ONE);
    }

    #[test]
    fn test_dividend_factor_rejects_non_positive_amount() {
        let sym = symbol("KO");
        let actions = vec![dividend(sym, date(2024, 3, 15), -1.0)];

        let result =
            AdjustmentFactors::dividend_factor(&actions, date(2024, 1, 1), |_| Some(decimal(60.0)));
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // Combined factor_for
    // ------------------------------------------------------------------

    #[test]
    fn test_factor_for_raw_is_always_identity() {
        let sym = symbol("AAPL");
        let actions = vec![forward_split(sym, date(2020, 8, 31), 1, 4)];

        let factor =
            AdjustmentFactors::factor_for(&actions, date(2020, 1, 1), BarAdjustment::Raw, |_| None)
                .unwrap();
        assert_eq!(factor, Decimal::ONE);
    }

    #[test]
    fn test_factor_for_all_combines_split_and_dividend() {
        let sym = symbol("AAPL");
        let actions = vec![
            forward_split(sym.clone(), date(2020, 8, 31), 1, 4),
            dividend(sym, date(2020, 5, 8), 0.82),
        ];

        let factor =
            AdjustmentFactors::factor_for(&actions, date(2020, 1, 1), BarAdjustment::All, |_| {
                Some(decimal(300.0))
            })
            .unwrap();

        let expected_split = Decimal::new(25, 2);
        let expected_dividend = (decimal(300.0) - decimal(0.82)) / decimal(300.0);
        assert_eq!(factor, expected_split * expected_dividend);
    }

    // ------------------------------------------------------------------
    // adjust_candles
    // ------------------------------------------------------------------

    #[test]
    fn test_adjust_candles_raw_mode_returns_input_unchanged() {
        let sym = symbol("AAPL");
        let raw = vec![candle(
            sym.clone(),
            Utc.with_ymd_and_hms(2020, 8, 28, 20, 0, 0).unwrap(),
            396.0,
            402.0,
            394.0,
            400.0,
        )];
        let actions = vec![forward_split(sym, date(2020, 8, 31), 1, 4)];

        let adjusted = adjust_candles(&raw, &actions, BarAdjustment::Raw).unwrap();
        assert_eq!(adjusted[0].close, raw[0].close);
    }

    #[test]
    fn test_adjust_candles_scales_only_bars_before_the_split() {
        let sym = symbol("AAPL");
        let before_split = candle(
            sym.clone(),
            Utc.with_ymd_and_hms(2020, 8, 28, 20, 0, 0).unwrap(),
            396.0,
            402.0,
            394.0,
            400.0,
        );
        let after_split = candle(
            sym.clone(),
            Utc.with_ymd_and_hms(2020, 9, 1, 13, 30, 0).unwrap(),
            100.5,
            101.0,
            99.5,
            100.0,
        );
        let raw = vec![before_split.clone(), after_split.clone()];
        let actions = vec![forward_split(sym, date(2020, 8, 31), 1, 4)];

        let adjusted = adjust_candles(&raw, &actions, BarAdjustment::Split).unwrap();

        assert_eq!(
            adjusted[0].close.value(),
            before_split.close.value() / Decimal::from(4),
            "pre-split bar must be divided by the split ratio"
        );
        assert_eq!(
            adjusted[1].close, after_split.close,
            "post-split bar is already on the current basis and must be untouched"
        );
    }

    #[test]
    fn test_adjust_candles_no_discontinuity_across_the_split() {
        // The whole point: raw bars straddling a split have a huge gap
        // (roughly 4x here); adjusted bars must not.
        let sym = symbol("AAPL");
        let before_split = candle(
            sym.clone(),
            Utc.with_ymd_and_hms(2020, 8, 28, 20, 0, 0).unwrap(),
            396.0,
            402.0,
            394.0,
            400.0,
        );
        let after_split = candle(
            sym.clone(),
            Utc.with_ymd_and_hms(2020, 9, 1, 13, 30, 0).unwrap(),
            100.5,
            101.0,
            99.5,
            100.0,
        );
        let raw = vec![before_split, after_split];
        let actions = vec![forward_split(sym, date(2020, 8, 31), 1, 4)];

        let raw_gap = (raw[0].close.value() - raw[1].close.value()).abs();
        assert!(
            raw_gap > decimal(250.0),
            "raw series has the split artefact"
        );

        let adjusted = adjust_candles(&raw, &actions, BarAdjustment::All).unwrap();
        let adjusted_gap = (adjusted[0].close.value() - adjusted[1].close.value()).abs();
        assert!(
            adjusted_gap < decimal(5.0),
            "adjusted series must not show the split as a price move, got gap {adjusted_gap}"
        );
    }

    // ------------------------------------------------------------------
    // CorporateAction::from_announcement
    // ------------------------------------------------------------------

    #[test]
    fn test_from_announcement_builds_split() {
        let action = CorporateAction::from_announcement(
            symbol("AAPL"),
            CorporateActionAnnouncement::Split(SplitAnnouncement {
                ex_date: date(2020, 8, 31),
                from: 1,
                to: 4,
            }),
            "alpaca",
        )
        .unwrap();

        assert_eq!(action.action_type, ActionType::Split { from: 1, to: 4 });
        assert_eq!(action.source, "alpaca");
    }

    #[test]
    fn test_from_announcement_rejects_zero_share_split() {
        let result = CorporateAction::from_announcement(
            symbol("AAPL"),
            CorporateActionAnnouncement::Split(SplitAnnouncement {
                ex_date: date(2020, 8, 31),
                from: 0,
                to: 4,
            }),
            "alpaca",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_from_announcement_rejects_non_positive_dividend() {
        let result = CorporateAction::from_announcement(
            symbol("KO"),
            CorporateActionAnnouncement::CashDividend(CashDividendAnnouncement {
                ex_date: date(2024, 3, 15),
                amount_per_share: Decimal::ZERO,
            }),
            "alpaca",
        );
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // Repository (round-trip against the real schema)
    // ------------------------------------------------------------------

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let schema = include_str!("../db/schema.sql");
        sqlx::raw_sql(schema).execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_repository_insert_and_query_round_trip() {
        let pool = test_pool().await;
        let repo = CorporateActionRepository::new(pool);
        let sym = symbol("AAPL");

        repo.insert(&forward_split(sym.clone(), date(2020, 8, 31), 1, 4))
            .await
            .unwrap();
        repo.insert(&dividend(sym.clone(), date(2020, 5, 8), 0.82))
            .await
            .unwrap();

        let actions = repo.get_all_actions(&sym).await.unwrap();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].ex_date, date(2020, 5, 8), "oldest-first order");
        assert_eq!(actions[1].action_type, ActionType::Split { from: 1, to: 4 });
    }

    #[tokio::test]
    async fn test_repository_get_actions_scopes_to_date_range() {
        let pool = test_pool().await;
        let repo = CorporateActionRepository::new(pool);
        let sym = symbol("AAPL");

        repo.insert(&forward_split(sym.clone(), date(2005, 2, 28), 1, 2))
            .await
            .unwrap();
        repo.insert(&forward_split(sym.clone(), date(2020, 8, 31), 1, 4))
            .await
            .unwrap();

        let recent = repo
            .get_actions(&sym, date(2015, 1, 1), date(2025, 1, 1))
            .await
            .unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].ex_date, date(2020, 8, 31));
    }

    #[tokio::test]
    async fn test_repository_insert_is_idempotent() {
        // Re-running an ingestion job must not error or duplicate rows.
        let pool = test_pool().await;
        let repo = CorporateActionRepository::new(pool);
        let sym = symbol("AAPL");
        let action = forward_split(sym.clone(), date(2020, 8, 31), 1, 4);

        repo.insert(&action).await.unwrap();
        repo.insert(&action).await.unwrap();

        let actions = repo.get_all_actions(&sym).await.unwrap();
        assert_eq!(actions.len(), 1);
    }
}
