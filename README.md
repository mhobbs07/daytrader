# DayTrader - Pattern-Based Day Trading Platform

A comprehensive day trading platform focused on candlestick pattern recognition with automated execution capabilities.

## Features

- Real-time market data ingestion (1-minute candles)
- Candlestick pattern detection
- Technical indicator analysis
- Comprehensive risk management with PDT compliance
- Manual and automated order execution
- Paper trading support
- Desktop application (Rust backend, future Tauri frontend)

## Prerequisites

- Rust 1.85+ (Edition 2024)
- SQLite 3
- Python 3.11+ (for future analytics)

## Quick Start

### 1. Clone and Setup

```bash
git clone <repository-url>
cd daytrader
./scripts/setup_db.sh
```

### 2. Configure API Keys

```bash
# Set Alpaca API credentials (paper trading)
export ALPACA_API_KEY="your_api_key"
export ALPACA_API_SECRET="your_api_secret"
```

### 3. Run Application

```bash
cargo run
```

### 4. Run Tests

```bash
cargo test
cargo clippy
cargo fmt --check
```

## Project Structure

```
daytrader/
├── src/               # Rust source code
│   ├── brokers/       # Broker abstraction layer
│   ├── config/        # Configuration management
│   ├── data/          # Market data pipeline
│   ├── db/            # Database layer
│   ├── events/        # Event bus (Phase 1)
│   ├── execution/     # Order execution
│   ├── patterns/      # Pattern detection
│   ├── risk/          # Risk management
│   └── utils/         # Utilities and errors
├── config/            # Configuration files
├── data/              # Local data (gitignored)
├── scripts/           # Utility scripts
└── docs/              # Documentation
```

## Development Roadmap

- [x] Phase 0: Setup (Week 0) ✅
- [ ] Phase 1: Core Infrastructure (Weeks 1-4)
- [ ] Phase 2: Risk Management (Weeks 5-8)
- [ ] Phase 3: Pattern Recognition (Weeks 9-12)
- [ ] Phase 4: Order Execution (Weeks 13-16)
- [ ] Phase 5: User Interface (Weeks 17-20)
- [ ] Phase 6: Testing & Validation (Weeks 21-24)

## Documentation

- [Project Knowledge Base](project-knowledge-base.md)
- [Technical Architecture](technical-architecture.md)
- [Architecture Analysis](architecture-analysis.md)
- [Phase 0 Setup Plan](phase-0-setup-plan.md)

## License

MIT

## Disclaimer

This software is for educational purposes only. Trading involves substantial risk of loss. Use at your own risk.