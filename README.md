# DayTrader - Pattern-Based Day Trading Platform

A day trading platform focused on candlestick pattern recognition with risk-managed,
PDT-compliant order execution. Rust backend library + Tauri 2 desktop app.

## Current Status (honest snapshot)

- **What runs today:** the Tauri desktop app — a live dashboard showing streaming quotes,
  candlestick charts, detected patterns, and risk/PDT status over a **synthetic mock data
  feed**. It does not place orders yet.
- **What exists as library code:** mature risk/PDT engine, 10 candlestick pattern
  detectors + 8 technical indicators, signal engine, risk-gated order engine, execution
  automation engine, Alpaca broker integration (streaming + order REST). The
  signal → execution → order pipeline is not yet wired into the running app.
- **What does not exist yet:** backtesting, historical data fetch, live/paper trading in
  the app. See `BACKLOG.md` and `plans/backtesting-requirements.md`.

## Prerequisites

- Rust 1.92+ (Edition 2024)
- Node.js 20+ and npm (for the desktop app)
- SQLite 3

## Quick Start

### Desktop app (the primary way to run DayTrader)

```bash
git clone <repository-url>
cd <repo>
cp .env.example .env           # SQLX_OFFLINE=true: the build uses the committed .sqlx/ cache, no database needed
./scripts/setup_db.sh          # optional: dev database, only needed to regenerate the .sqlx/ cache
cd daytrader
npm install
npm run tauri dev              # Vite dev server on :1421 + Tauri desktop shell
```

No broker API keys are required — the app currently runs entirely on the built-in
synthetic mock data driver (`daytrader/src-tauri/src/mock_driver.rs`).

### Backend binary

```bash
cargo run
```

`src/main.rs` is currently a placeholder entry point (initializes logging/metrics and
exits). The backend functionality lives in the library crate and is consumed by the
Tauri app and the examples in `examples/`.

### Broker examples (Alpaca paper trading)

```bash
export ALPACA_API_KEY="your_api_key"
export ALPACA_API_SECRET="your_api_secret"
cargo run --example alpaca_basic
cargo run --example alpaca_streaming
```

### Tests and verification

```bash
cargo test                        # full test suite
./scripts/verify_agent_work.sh    # mandatory pre-commit pipeline (fmt, clippy, tests,
                                  # docs, architecture boundaries, test quality, backlog)
```

## Project Structure

```
.
├── src/                   # Rust library crate
│   ├── brokers/           # Broker abstraction (Alpaca, mock)
│   ├── config/            # Configuration management
│   ├── data/              # Market data ingestion/cache/validation
│   ├── db/                # SQLite persistence (sqlx)
│   ├── events/            # Event bus (tokio broadcast)
│   ├── execution/         # Automated execution engine
│   ├── orders/            # Risk-gated order lifecycle
│   ├── patterns/          # Pattern detection, indicators, signal engine
│   ├── positions/         # Position tracking and P&L
│   ├── risk/              # Risk engine, PDT compliance, position sizing
│   └── utils/             # Errors, retry, market hours, metrics, logging
├── daytrader/             # Tauri 2 desktop app (React 19 + TypeScript + Vite)
│   └── src-tauri/         # Tauri shell crate (commands, event bridge, mock driver)
├── config/                # TOML configuration (config/default.toml)
├── scripts/               # setup_db.sh, verify_agent_work.sh, etc.
├── docs/                  # Guides (onboarding, PDT, risk, deployment, FAQ)
├── plans/                 # Design docs, phase plans, architecture docs
├── examples/              # Runnable broker/library examples
├── benches/               # Performance benchmarks
└── tests/                 # Integration tests
```

## Development Roadmap

- [x] Phase 0: Setup
- [x] Phase 1: Core infrastructure (events, db, brokers)
- [x] Phase 2: Risk management & PDT compliance
- [x] Phase 3: Pattern recognition (patterns, indicators, signals)
- [x] Phase 4: Order execution & position tracking (library complete; not yet wired
      into the running app)
- [~] Phase 5: User interface (live dashboard done; order entry/positions views pending)
- [ ] Phase 6: Backtesting & validation framework (required before any live trading —
      see `plans/backtesting-requirements.md`)

Authoritative task tracking lives in `BACKLOG.md`.

## Documentation

### Core
- [Project Knowledge Base](plans/project-knowledge-base.md) — comprehensive project overview
- [Technical Architecture](plans/technical-architecture.md) — system design and patterns
- [Developer Onboarding](docs/developer-onboarding.md) — full setup walkthrough
- [Security Guidelines](SECURITY.md)

### Guides
- [Risk Management Guide](docs/risk-management-guide.md)
- [PDT Implementation Guide](docs/pdt-implementation-guide.md)
- [API Usage Examples](docs/api-usage-examples.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Deployment](docs/deployment.md)

### Workflow
- [Git Branching Rules](.claude/rules/branching.md)
- [CI/CD Setup Guide](docs/ci-cd-setup-guide.md)

## License

MIT

## Disclaimer

This software is for educational purposes only. Trading involves substantial risk of loss.
Use at your own risk.
