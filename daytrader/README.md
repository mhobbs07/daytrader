# DayTrader Desktop App

Tauri 2 desktop shell for the DayTrader platform. React 19 + TypeScript + Vite frontend,
with a Rust shell crate (`src-tauri/`) that depends on the root workspace library for all
domain logic (patterns, risk, events, brokers).

## Run

From the repository root, once (creates the SQLite db and prints the `DATABASE_URL` export
needed by sqlx macros):

```bash
./scripts/setup_db.sh
```

Then:

```bash
cd daytrader
npm install
npm run tauri dev      # full desktop app (Vite dev server on :1421 + Tauri window)
```

Other scripts:

```bash
npm run dev            # Vite dev server only (browser, no Tauri APIs)
npm run build          # typecheck + production frontend build
npm run tauri build    # production desktop bundle
```

No broker API keys are needed: the app currently streams synthetic data from
`src-tauri/src/mock_driver.rs` (random-walk quotes/candles at 1s intervals).

## What the app does today

Single-screen live dashboard:

- Watchlist (add/remove symbols, persisted to localStorage)
- Per-symbol quote tiles and live candlestick charts (`lightweight-charts`)
- Live pattern detections with symbol/type/confidence filters
- Risk status: daily P&L, halt state, day-trade count, PDT status, violations
- System status: broker connection, event bus metrics

It does not place orders yet — order entry, positions, and automation views are pending
(see `BACKLOG.md` at the repo root).

## Structure

```
daytrader/
├── src/                  # React frontend
│   ├── components/       # CandleChart, Watchlist, PatternList, RiskStatus, ...
│   ├── hooks/            # useMarketData, usePatterns, useTauriEvents
│   ├── store/            # Zustand stores (market, patterns, risk, system)
│   └── types/            # Shared TS types mirroring backend DTOs
└── src-tauri/            # Rust shell crate
    └── src/
        ├── main.rs       # wiring: mock driver, pattern task, risk engine, event bridge
        ├── state.rs      # AppState
        ├── events.rs     # EventBridge (backend events -> Tauri events)
        ├── mock_driver.rs
        └── commands/     # #[tauri::command] handlers (market_data, patterns, risk, system)
```

Frontend/backed communication is hybrid: Tauri events for pushes
(`market_data:quote_update`, `patterns:detected`, `risk:violation`, ...) plus polled
commands as fallback.
