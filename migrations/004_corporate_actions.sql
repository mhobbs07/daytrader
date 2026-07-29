-- Migration 004: Corporate-action factor table
--
-- Step 1 (migration 003) made the corporate-action *basis* of a stored bar
-- explicit via the `adjustment` column, but adjusted bars returned by a
-- vendor are only correct as of the moment they were fetched: a later split
-- changes what the same historical request returns, so an adjusted series
-- pulled today is not reproducible against one pulled after a subsequent
-- split. It also cannot be spliced with live "raw" bars without a
-- discontinuity across the split.
--
-- The fix is the standard one: store bars raw, record corporate actions
-- separately in a ledger, and adjust at query time. Never adjust stored
-- prices in place — an in-place backfill silently rewrites history and
-- destroys reproducibility the same way an unpinned vendor request does.
-- This migration adds that ledger; `crate::data::corporate_actions` computes
-- the adjustment factors from its rows.
--
-- A row's `action_type` determines which type-specific columns are
-- populated: `split_from`/`split_to` for `'split'`, `cash_amount` for
-- `'cash_dividend'`. The CHECK constraint enforces that exactly the right
-- columns are set for each type, so a half-populated row cannot exist.
--
-- `cash_amount` is REAL, matching the pre-existing (already imprecise)
-- convention for money elsewhere in this schema — see the `candles` price
-- columns. The planned `Money<6>` integer-micros migration (B-087, not yet
-- landed) should convert this column alongside the rest when it does.
--
-- Applied programmatically by MigrationManager::migrate_to_v4; this file is
-- the reviewable reference form.

CREATE TABLE IF NOT EXISTS corporate_actions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    ex_date DATE NOT NULL,
    action_type TEXT NOT NULL CHECK(action_type IN ('split', 'cash_dividend')),
    split_from INTEGER,
    split_to INTEGER,
    -- REAL pending the B-087 Money<6> integer-micros migration; convert this
    -- column alongside the rest when it lands.
    cash_amount REAL,
    source TEXT NOT NULL,
    ingested_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    CHECK (
        (action_type = 'split'
            AND split_from IS NOT NULL AND split_from > 0
            AND split_to IS NOT NULL AND split_to > 0
            AND cash_amount IS NULL)
        OR
        (action_type = 'cash_dividend'
            AND cash_amount IS NOT NULL AND cash_amount > 0
            AND split_from IS NULL AND split_to IS NULL)
    ),
    UNIQUE(symbol, ex_date, action_type)
);

CREATE INDEX IF NOT EXISTS idx_corporate_actions_symbol_date ON corporate_actions(symbol, ex_date);

INSERT INTO schema_migrations (version, description)
VALUES (4, 'Add corporate_actions factor table for query-time bar adjustment');
