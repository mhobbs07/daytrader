-- Migration 003: Candle corporate-action adjustment discriminator
--
-- Bars fetched under different corporate-action adjustments are different
-- series for the same instrument and instant. The original uniqueness key
-- (symbol, timestamp, broker) could not distinguish them, so writing an
-- adjusted bar over a previously-stored raw bar silently overwrote it via the
-- ON CONFLICT ... DO UPDATE path, leaving the table with mixed and unlabelled
-- provenance.
--
-- Why this matters: unadjusted ("raw") bars leave split discontinuities in the
-- series. A 4:1 split appears as a ~75% single-bar drop that pattern detectors
-- cannot distinguish from a real move, so they generate signals on an artefact.
-- Every fetch before this migration hardcoded adjustment=raw, so existing rows
-- are backfilled as 'raw' — historically accurate, and correctly marked as not
-- analysis-safe.
--
-- SQLite cannot alter a UNIQUE constraint in place, so the table is rebuilt.
-- Applied programmatically by MigrationManager::migrate_to_v3; this file is the
-- reviewable reference form.

CREATE TABLE IF NOT EXISTS candles_v3 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    open REAL NOT NULL,
    high REAL NOT NULL,
    low REAL NOT NULL,
    close REAL NOT NULL,
    volume INTEGER NOT NULL,
    broker TEXT NOT NULL,
    adjustment TEXT NOT NULL DEFAULT 'raw'
        CHECK(adjustment IN ('raw', 'split', 'dividend', 'all')),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(symbol, timestamp, broker, adjustment)
);

INSERT OR IGNORE INTO candles_v3
    (id, symbol, timestamp, open, high, low, close, volume, broker, adjustment, created_at)
SELECT id, symbol, timestamp, open, high, low, close, volume, broker, 'raw', created_at
FROM candles;

DROP TABLE candles;

ALTER TABLE candles_v3 RENAME TO candles;

CREATE INDEX IF NOT EXISTS idx_candles_symbol_time ON candles(symbol, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_candles_adjustment ON candles(symbol, adjustment, timestamp DESC);

INSERT INTO schema_migrations (version, description)
VALUES (3, 'Add candle corporate-action adjustment discriminator');
