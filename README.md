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

- Rust 1.75+ (Edition 2021)
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
│   ├── brokers/       # Broker abstraction layer (Alpaca, mock)
│   ├── config/        # Configuration management
│   ├── db/            # Database layer (SQLite)
│   ├── events/        # Event bus system
│   ├── execution/     # Order execution engine
│   ├── patterns/      # Candlestick pattern detection
│   ├── risk/          # Risk management & PDT compliance
│   └── utils/         # Utilities, logging, metrics, errors
├── config/            # Configuration files (TOML)
├── scripts/           # Setup and utility scripts
├── docs/              # Documentation
├── examples/          # Usage examples
├── benches/           # Performance benchmarks
└── tests/             # Integration tests
```

## Development Roadmap

- [x] Phase 0: Setup (Week 0) ✅
- [x] Phase 1: Core Infrastructure (Weeks 1-4) ✅
- [x] Phase 2: Risk Management (Weeks 5-8) ✅
- [x] Phase 3A: Pattern Recognition - Basic Patterns (Weeks 9-10) ✅
- [ ] Phase 3B: Pattern Recognition - Technical Indicators (Weeks 11-12)
- [ ] Phase 3C: Pattern Recognition - Signal Generation (Week 13)
- [ ] Phase 3D: Pattern Recognition - Integration & Optimization (Week 14)
- [ ] Phase 4: Order Execution (Weeks 15-18)
- [ ] Phase 5: User Interface (Weeks 19-22)
- [ ] Phase 6: Testing & Validation (Weeks 23-26)

## Documentation

### Core Documentation
- [Project Knowledge Base](project-knowledge-base.md) - Comprehensive project overview
- [Technical Architecture](technical-architecture.md) - System design and patterns
- [Architecture Analysis](architecture-analysis.md) - Current architecture review
- [Security Guidelines](SECURITY.md) - Security policies and procedures

### Development Guides
- [Phase 0 Setup Plan](phase-0-setup-plan.md) - Initial project setup
- [Phase 3 Pattern Recognition Plan](phase-3-pattern-recognition-plan.md) - Current development phase
- [Phase 3A-2 Completion Report](phase-3a-2-completion-report.md) - Latest implementation status
- [Performance Analysis Report](performance-analysis-report.md) - Performance benchmarks

### API & Examples
- [API Usage Examples](api-usage-examples.md) - Code examples and patterns
- [Alpaca Broker Examples](examples/alpaca_basic.rs) - Broker integration examples
- [Mobile API Documentation](mobile-kilocode-api.md) - Mobile client API

### Development Workflow
- [Git Workflow Rules](git-workflow-rules.md) - Development workflow guidelines
- [CI/CD Setup Guide](ci-cd-setup-guide.md) - Build and deployment automation

## License

MIT

## Disclaimer

This software is for educational purposes only. Trading involves substantial risk of loss. Use at your own risk.