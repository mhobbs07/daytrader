-- Database schema for day trading platform
-- SQLite version

-- Accounts
CREATE TABLE IF NOT EXISTS accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    broker TEXT NOT NULL,
    account_number TEXT NOT NULL UNIQUE,
    account_type TEXT NOT NULL CHECK(account_type IN ('paper', 'live')),
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Market Data (1-min candles)
CREATE TABLE IF NOT EXISTS candles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    open REAL NOT NULL,
    high REAL NOT NULL,
    low REAL NOT NULL,
    close REAL NOT NULL,
    volume INTEGER NOT NULL,
    broker TEXT NOT NULL,
    -- Corporate-action treatment. Bars under different adjustments are
    -- different series and must not collide; see BarAdjustment.
    adjustment TEXT NOT NULL DEFAULT 'raw'
        CHECK(adjustment IN ('raw', 'split', 'dividend', 'all')),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(symbol, timestamp, broker, adjustment)
);

CREATE INDEX IF NOT EXISTS idx_candles_symbol_time ON candles(symbol, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_candles_adjustment ON candles(symbol, adjustment, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_candles_timestamp ON candles(timestamp DESC);

-- Positions (Active Positions Only)
CREATE TABLE IF NOT EXISTS positions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    side TEXT NOT NULL CHECK(side IN ('long', 'short')) DEFAULT 'long',
    avg_entry_price REAL NOT NULL,
    current_price REAL NOT NULL,
    market_value REAL NOT NULL,
    cost_basis REAL NOT NULL,
    unrealized_pl REAL NOT NULL DEFAULT 0.0,
    unrealized_pl_percent REAL NOT NULL DEFAULT 0.0,
    realized_pl REAL NOT NULL DEFAULT 0.0,
    opened_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    -- Pattern tracking
    pattern_name TEXT,
    signal_confidence REAL,
    -- Order tracking stored as JSON arrays
    entry_orders TEXT,  -- JSON array of order IDs
    exit_orders TEXT,   -- JSON array of order IDs
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    UNIQUE(account_id, symbol)
);

CREATE INDEX IF NOT EXISTS idx_positions_account ON positions(account_id);
CREATE INDEX IF NOT EXISTS idx_positions_symbol ON positions(symbol);
CREATE INDEX IF NOT EXISTS idx_positions_pattern ON positions(pattern_name) WHERE pattern_name IS NOT NULL;

-- Position History (Closed Positions)
CREATE TABLE IF NOT EXISTS position_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    side TEXT NOT NULL CHECK(side IN ('long', 'short')),
    avg_entry_price REAL NOT NULL,
    exit_price REAL NOT NULL,
    cost_basis REAL NOT NULL,
    unrealized_pl REAL,
    realized_pl REAL NOT NULL,
    total_pl REAL NOT NULL,
    opened_at TIMESTAMP NOT NULL,
    closed_at TIMESTAMP NOT NULL,
    holding_period_minutes INTEGER,
    close_reason TEXT CHECK(close_reason IN ('manual', 'stop_loss', 'take_profit', 'risk_violation', 'pattern_exit', 'end_of_day')),
    -- Pattern tracking
    pattern_name TEXT,
    signal_confidence REAL,
    -- Order tracking stored as JSON arrays
    entry_orders TEXT,
    exit_orders TEXT,
    notes TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);

CREATE INDEX IF NOT EXISTS idx_position_history_account_closed ON position_history(account_id, closed_at DESC);
CREATE INDEX IF NOT EXISTS idx_position_history_symbol ON position_history(symbol);
CREATE INDEX IF NOT EXISTS idx_position_history_pattern ON position_history(pattern_name) WHERE pattern_name IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_position_history_pl ON position_history(realized_pl);

-- Orders
CREATE TABLE IF NOT EXISTS orders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    broker_order_id TEXT UNIQUE,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL CHECK(side IN ('buy', 'sell')),
    order_type TEXT NOT NULL CHECK(order_type IN ('market', 'limit', 'stop', 'stop_limit', 'trailing_stop')),
    quantity INTEGER NOT NULL,
    filled_quantity INTEGER DEFAULT 0,
    limit_price REAL,
    stop_price REAL,
    trailing_percent REAL,
    status TEXT NOT NULL CHECK(status IN ('pending', 'filled', 'partial', 'cancelled', 'rejected')),
    time_in_force TEXT DEFAULT 'day' CHECK(time_in_force IN ('day', 'gtc', 'ioc', 'fok')),
    submitted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    filled_at TIMESTAMP,
    filled_avg_price REAL,
    commission REAL DEFAULT 0.0,
    error_message TEXT,
    is_pattern_trade BOOLEAN DEFAULT FALSE,
    pattern_name TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);

CREATE INDEX IF NOT EXISTS idx_orders_account_time ON orders(account_id, submitted_at DESC);
CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(status);

-- Trades (Filled Orders)
CREATE TABLE IF NOT EXISTS trades (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    order_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL CHECK(side IN ('buy', 'sell')),
    quantity INTEGER NOT NULL,
    price REAL NOT NULL,
    commission REAL DEFAULT 0.0,
    realized_pl REAL,
    executed_at TIMESTAMP NOT NULL,
    is_day_trade BOOLEAN DEFAULT FALSE,
    pattern_name TEXT,
    notes TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    FOREIGN KEY (order_id) REFERENCES orders(id)
);

CREATE INDEX IF NOT EXISTS idx_trades_account_time ON trades(account_id, executed_at DESC);
CREATE INDEX IF NOT EXISTS idx_trades_symbol ON trades(symbol);

-- Pattern Signals
CREATE TABLE IF NOT EXISTS pattern_signals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    pattern_name TEXT NOT NULL,
    signal_type TEXT NOT NULL CHECK(signal_type IN ('buy', 'sell')),
    confidence REAL CHECK(confidence >= 0 AND confidence <= 1),
    detected_at TIMESTAMP NOT NULL,
    candle_timestamp TIMESTAMP NOT NULL,
    price_at_detection REAL NOT NULL,
    acted_upon BOOLEAN DEFAULT FALSE,
    order_id INTEGER,
    outcome TEXT CHECK(outcome IN ('win', 'loss', 'pending', 'ignored')),
    FOREIGN KEY (order_id) REFERENCES orders(id)
);

CREATE INDEX IF NOT EXISTS idx_signals_symbol_time ON pattern_signals(symbol, detected_at DESC);
CREATE INDEX IF NOT EXISTS idx_signals_pattern ON pattern_signals(pattern_name);

-- Pattern Performance Tracking
CREATE TABLE IF NOT EXISTS pattern_stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern_name TEXT NOT NULL UNIQUE,
    total_signals INTEGER DEFAULT 0,
    total_trades INTEGER DEFAULT 0,
    winning_trades INTEGER DEFAULT 0,
    losing_trades INTEGER DEFAULT 0,
    total_profit REAL DEFAULT 0.0,
    win_rate REAL DEFAULT 0.0,
    avg_profit_per_trade REAL DEFAULT 0.0,
    consecutive_losses INTEGER DEFAULT 0,
    is_enabled BOOLEAN DEFAULT TRUE,
    max_trades_per_day INTEGER DEFAULT 5,
    stop_after_consecutive_losses INTEGER DEFAULT 3,
    required_win_rate REAL DEFAULT 60.0,
    min_sample_size INTEGER DEFAULT 20,
    avg_confidence REAL DEFAULT 0.0,
    confidence_weighted_win_rate REAL DEFAULT 0.0,
    last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Rolling Window Pattern Performance
CREATE TABLE IF NOT EXISTS pattern_rolling_stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern_name TEXT NOT NULL,
    window_days INTEGER NOT NULL,
    total_trades INTEGER DEFAULT 0,
    winning_trades INTEGER DEFAULT 0,
    losing_trades INTEGER DEFAULT 0,
    total_profit REAL DEFAULT 0.0,
    win_rate REAL DEFAULT 0.0,
    avg_profit_per_trade REAL DEFAULT 0.0,
    consecutive_losses INTEGER DEFAULT 0,
    performance_score REAL DEFAULT 0.0,
    risk_multiplier REAL DEFAULT 1.0,
    window_start TIMESTAMP NOT NULL,
    window_end TIMESTAMP NOT NULL,
    last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (pattern_name) REFERENCES pattern_stats(pattern_name) ON DELETE CASCADE,
    UNIQUE(pattern_name, window_days, window_end)
);

CREATE INDEX IF NOT EXISTS idx_rolling_stats_pattern_window ON pattern_rolling_stats(pattern_name, window_days);
CREATE INDEX IF NOT EXISTS idx_rolling_stats_updated ON pattern_rolling_stats(last_updated DESC);

-- Pattern Trade History for Rolling Windows
CREATE TABLE IF NOT EXISTS pattern_trade_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern_name TEXT NOT NULL,
    trade_timestamp TIMESTAMP NOT NULL,
    confidence REAL NOT NULL,
    profit REAL NOT NULL,
    was_profitable BOOLEAN NOT NULL,
    order_id TEXT,
    symbol TEXT,
    quantity INTEGER,
    price REAL,
    commission REAL DEFAULT 0.0,
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (pattern_name) REFERENCES pattern_stats(pattern_name) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_trade_history_pattern_time ON pattern_trade_history(pattern_name, trade_timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_trade_history_timestamp ON pattern_trade_history(trade_timestamp DESC);

-- Risk Adjustment Audit Log
CREATE TABLE IF NOT EXISTS risk_adjustment_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern_name TEXT NOT NULL,
    adjustment_type TEXT NOT NULL CHECK(adjustment_type IN ('escalation', 'deescalation', 'enable', 'disable')),
    old_risk_multiplier REAL,
    new_risk_multiplier REAL NOT NULL,
    performance_score REAL,
    trigger_reason TEXT NOT NULL,
    window_days INTEGER,
    trade_count INTEGER,
    win_rate REAL,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (pattern_name) REFERENCES pattern_stats(pattern_name) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_risk_audit_pattern_time ON risk_adjustment_audit(pattern_name, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_risk_audit_type ON risk_adjustment_audit(adjustment_type);

-- Daily Risk Tracking
CREATE TABLE IF NOT EXISTS daily_risk_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    trading_date DATE NOT NULL,
    starting_balance REAL NOT NULL,
    current_balance REAL NOT NULL,
    realized_pl REAL DEFAULT 0.0,
    unrealized_pl REAL DEFAULT 0.0,
    total_pl REAL DEFAULT 0.0,
    day_trades_count INTEGER DEFAULT 0,
    trades_count INTEGER DEFAULT 0,
    largest_loss REAL DEFAULT 0.0,
    is_trading_halted BOOLEAN DEFAULT FALSE,
    halt_reason TEXT,
    max_loss_limit_dollars REAL,
    max_loss_limit_percent REAL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    UNIQUE(account_id, trading_date)
);

CREATE INDEX IF NOT EXISTS idx_risk_metrics_date ON daily_risk_metrics(trading_date DESC);

-- Position Risk Tracking
CREATE TABLE IF NOT EXISTS position_risk_tracking (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    position_size REAL NOT NULL,
    market_value REAL NOT NULL,
    portfolio_allocation_percent REAL NOT NULL,
    max_position_limit_dollars REAL,
    max_portfolio_limit_percent REAL,
    is_violation BOOLEAN DEFAULT FALSE,
    violation_type TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    UNIQUE(account_id, symbol)
);

CREATE INDEX IF NOT EXISTS idx_position_risk_account_symbol ON position_risk_tracking(account_id, symbol);
CREATE INDEX IF NOT EXISTS idx_position_risk_updated ON position_risk_tracking(updated_at DESC);

-- PDT Tracking
CREATE TABLE IF NOT EXISTS pdt_tracking (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL,
    trading_date DATE NOT NULL,
    day_trades_count INTEGER DEFAULT 0,
    rolling_window_start DATE NOT NULL,
    rolling_window_end DATE NOT NULL,
    total_day_trades_window INTEGER DEFAULT 0,
    equity_value REAL NOT NULL,
    min_equity_threshold REAL,
    max_day_trades_allowed INTEGER,
    is_restricted BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id),
    UNIQUE(account_id, trading_date)
);

CREATE INDEX IF NOT EXISTS idx_pdt_tracking_date ON pdt_tracking(trading_date DESC);
CREATE INDEX IF NOT EXISTS idx_pdt_tracking_account ON pdt_tracking(account_id);

-- Watchlists
CREATE TABLE IF NOT EXISTS watchlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS watchlist_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    watchlist_id INTEGER NOT NULL,
    symbol TEXT NOT NULL,
    added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    notes TEXT,
    FOREIGN KEY (watchlist_id) REFERENCES watchlists(id) ON DELETE CASCADE,
    UNIQUE(watchlist_id, symbol)
);

-- Price Alerts
CREATE TABLE IF NOT EXISTS alerts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    alert_type TEXT NOT NULL CHECK(alert_type IN ('price_above', 'price_below', 'pattern_detected')),
    threshold_value REAL,
    pattern_name TEXT,
    is_active BOOLEAN DEFAULT TRUE,
    triggered_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Audit Log
CREATE TABLE IF NOT EXISTS audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL CHECK(severity IN ('info', 'warning', 'error', 'critical')),
    user_id TEXT,
    account_id INTEGER,
    description TEXT NOT NULL,
    metadata TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);

CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_severity ON audit_log(severity);

-- Risk Events Storage
-- Stores all risk-related events for audit, compliance, and performance analysis
CREATE TABLE IF NOT EXISTS risk_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL CHECK(event_type IN (
        'RiskViolation', 'RiskWarning', 'RiskMultiplierAdjusted',
        'PatternEnabled', 'PatternDisabled', 'PatternPerformanceUpdate'
    )),
    timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    account_id INTEGER,
    pattern_name TEXT,
    violation_type TEXT,  -- For RiskViolation events
    severity TEXT CHECK(severity IN ('low', 'moderate', 'high', 'critical')),
    message TEXT NOT NULL,
    details TEXT,  -- JSON or serialized data (e.g., old/new multipliers, win_rate, etc.)
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_risk_events_timestamp ON risk_events(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_risk_events_pattern ON risk_events(pattern_name);
CREATE INDEX IF NOT EXISTS idx_risk_events_type ON risk_events(event_type);
CREATE INDEX IF NOT EXISTS idx_risk_events_account ON risk_events(account_id);
CREATE INDEX IF NOT EXISTS idx_risk_events_severity ON risk_events(severity);

-- Retention policy: Keep events for 90 days (handled in application code via cron job or trigger)
-- Example cleanup query: DELETE FROM risk_events WHERE created_at < datetime('now', '-90 days');

-- Application Settings
CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Insert default settings
INSERT OR IGNORE INTO app_settings (key, value) VALUES
    ('risk.max_daily_loss_dollars', '500.00'),
    ('risk.max_daily_loss_percent', '2.00'),
    ('risk.max_position_size_dollars', '5000.00'),
    ('risk.max_position_size_percent', '10.00'),
    ('risk.max_concurrent_positions', '5'),
    ('risk.enforce_pdt_rule', 'true'),
    ('risk.pdt_min_equity', '25000.00'),
    ('risk.max_day_trades_under_pdt', '3'),
    ('risk.max_trades_per_pattern_per_day', '5'),
    ('risk.stop_pattern_after_consecutive_losses', '3'),
    ('risk.required_win_rate_for_auto', '60.00'),
    ('risk.min_pattern_sample_size', '20'),
    ('risk.min_account_balance', '1000.00'),
    ('risk.emergency_liquidate_threshold', '500.00'),
    ('events.retention_days', '90'),  -- Default retention for risk events
    ('events.persistence_enabled', 'true'),
    ('events.metrics_collection', 'true');

-- Trading Signals Table (Phase 3D-2)
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

-- Indicator Values Table (Phase 3D-2)
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

-- Corporate Actions (Phase R0.1 step 2 — B-086)
-- Ledger of splits and cash dividends used to back-adjust raw candles at
-- query time. Bars are stored raw (see candles.adjustment='raw') and never
-- rewritten in place; crate::data::corporate_actions computes adjustment
-- factors from these rows on read. See migrations/004_corporate_actions.sql
-- for the full rationale.
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