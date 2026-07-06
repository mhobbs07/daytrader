-- Add trading signals and indicator values tables (Phase 3D-2)

-- Trading Signals Table
-- Stores composite trading signals from SignalEngine
CREATE TABLE IF NOT EXISTS trading_signals (
    id TEXT PRIMARY KEY,
    symbol TEXT NOT NULL,
    signal_type TEXT NOT NULL CHECK(signal_type IN ('buy', 'sell')),
    confidence REAL NOT NULL CHECK(confidence >= 0 AND confidence <= 1),
    price_level REAL NOT NULL,
    generated_at TIMESTAMP NOT NULL,

    -- Market context fields
    trend_type TEXT,
    trend_strength REAL,
    volatility REAL,
    market_strength REAL,

    -- Components and metadata as JSON
    components TEXT NOT NULL,
    metadata TEXT,

    -- Outcome tracking
    acted_upon BOOLEAN DEFAULT FALSE,
    outcome TEXT CHECK(outcome IN ('win', 'loss', 'pending', 'ignored')),
    order_id INTEGER,
    FOREIGN KEY (order_id) REFERENCES orders(id)
);

CREATE INDEX IF NOT EXISTS idx_trading_signals_symbol_time ON trading_signals(symbol, generated_at DESC);
CREATE INDEX IF NOT EXISTS idx_trading_signals_confidence ON trading_signals(confidence DESC);
CREATE INDEX IF NOT EXISTS idx_trading_signals_outcome ON trading_signals(outcome);

-- Indicator Values Table
-- Stores technical indicator values for analytics
CREATE TABLE IF NOT EXISTS indicator_values (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    indicator_name TEXT NOT NULL,
    value REAL NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    metadata TEXT,
    UNIQUE(symbol, indicator_name, timestamp)
);

CREATE INDEX IF NOT EXISTS idx_indicator_values_symbol_time ON indicator_values(symbol, timestamp DESC);
