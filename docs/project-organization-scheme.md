# Project Organization Scheme

## Overview

This document outlines the organization scheme for the DayTrader project, a comprehensive algorithmic trading platform built with Rust and Tauri. The project follows a monorepo structure that separates concerns while maintaining clear boundaries between different components and layers.

## Rationale

The chosen organization scheme is designed to:

- **Maintain Separation of Concerns**: Each directory serves a specific purpose, making it easy to locate and modify code related to particular functionality.
- **Support Multi-Language Development**: The project combines Rust (backend/core logic), TypeScript/React (frontend), and various configuration/scripting languages.
- **Facilitate Scalability**: As the project grows, new features can be added to appropriate directories without disrupting existing code.
- **Enable Efficient Development**: Clear structure reduces cognitive load and makes onboarding new developers easier.
- **Support Testing and Documentation**: Dedicated directories for tests, benchmarks, examples, and documentation ensure these critical aspects are properly organized.

## Directory Structure and Purposes

### Root Level (`/`)
Contains project-wide configuration, metadata, and tooling files.

- **Purpose**: Project initialization, build configuration, version control, and CI/CD setup.
- **Key Files**:
  - `Cargo.toml`: Main Rust project configuration and dependencies.
  - `README.md`: Project overview and setup instructions.
  - `SECURITY.md`: Security guidelines and vulnerability reporting.
  - `.gitignore`, `.dockerignore`: Version control exclusions.
  - `Dockerfile`, `docker-compose.yml`: Containerization setup.
  - `.gitlab-ci.yml`: CI/CD pipeline configuration.
  - `.husky/`: Git hooks for code quality.
  - `.github/`: GitHub-specific configurations.

### `src/` - Core Rust Library
The heart of the trading platform's backend logic.

- **Purpose**: Contains the main Rust library code for trading algorithms, risk management, data processing, and broker integrations.
- **Subdirectories**:
  - `brokers/`: Broker API integrations (Alpaca, Interactive Brokers, mock implementations).
  - `config/`: Configuration loading and validation.
  - `db/`: Database connection, models, repositories, and migrations.
  - `events/`: Event-driven architecture components (event bus, filters, types).
  - `execution/`: Order execution and trade management.
  - `patterns/`: Technical analysis patterns and indicators.
  - `risk/`: Risk management engine, position sizing, PDT rules, limits.
  - `utils/`: Shared utilities (error handling, logging, metrics, performance monitoring).

### `daytrader/` - Tauri Desktop Application
The user-facing desktop application built with Tauri.

- **Purpose**: Provides the graphical user interface and bridges between the Rust backend and user interactions.
- **Subdirectories**:
  - `src/`: React/TypeScript frontend code.
    - `components/`: Reusable UI components (MarketData, RiskStatus, etc.).
    - `store/`: State management stores (marketStore, riskStore, etc.).
    - `assets/`: Static assets (images, icons).
  - `src-tauri/`: Rust backend for the Tauri application.
    - `src/commands/`: Tauri command handlers for IPC between frontend and backend.
    - `src/`: Core Tauri application logic (main.rs, state.rs, events.rs).
    - `icons/`: Application icons for different platforms.
    - `capabilities/`: Tauri capability configurations.

### `docs/` - Documentation
Comprehensive project documentation.

- **Purpose**: Maintains all documentation for development, usage, deployment, and maintenance.
- **Contents**: API references, usage examples, setup guides, troubleshooting, performance guides, etc.

### `tests/` - Integration Tests
End-to-end and integration test suites.

- **Purpose**: Validates the interaction between different components and ensures system-level functionality.
- **Examples**: Database integration tests, risk management edge cases, market scenario simulations.

### `benches/` - Performance Benchmarks
Performance testing and benchmarking code.

- **Purpose**: Measures and optimizes the performance of critical trading algorithms and data processing pipelines.
- **Examples**: Pattern recognition benchmarks, risk calculation performance tests.

### `examples/` - Usage Examples
Sample code demonstrating platform usage.

- **Purpose**: Provides practical examples for users and developers to understand how to use the platform's APIs.
- **Examples**: Basic Alpaca integration, streaming data processing.

### `plans/` - Planning and Design Documents
Strategic planning and architectural decisions.

- **Purpose**: Documents the evolution of the project, design decisions, and future plans.
- **Contents**: Architecture reviews, phase plans, coding standards, workflow diagrams.

### `reports/` - Progress Reports
Development progress and milestone reports.

- **Purpose**: Tracks project advancement, completion reviews, and performance analyses.
- **Contents**: Phase completion reports, performance analysis reports.

### `scripts/` - Utility Scripts
Automation and utility scripts.

- **Purpose**: Automates common development tasks, setup, and maintenance operations.
- **Examples**: Documentation generation, database setup, build scripts.

### `config/` - Configuration Files
Application configuration templates.

- **Purpose**: Provides default and example configurations for different environments.
- **Examples**: Default TOML configuration files.

## File Placement Rules

### New Rust Backend Code
- **Core algorithms**: Place in appropriate `src/` subdirectories (e.g., new indicators in `src/patterns/indicators.rs`).
- **New broker integrations**: Create new modules in `src/brokers/`.
- **Database changes**: Add migrations to `src/db/migrations.rs`, models to `src/db/models.rs`.
- **Risk management features**: Extend `src/risk/` modules.
- **Utilities**: Add to `src/utils/` or create new submodules as needed.

### New Frontend Code
- **UI components**: Add to `daytrader/src/components/`.
- **State management**: Create new stores in `daytrader/src/store/`.
- **Assets**: Place in `daytrader/src/assets/`.
- **Tauri commands**: Add to `daytrader/src-tauri/src/commands/`.

### New Tests
- **Unit tests**: Place alongside the code they test (e.g., `src/patterns/indicators.rs` has corresponding tests in the same file or `tests/`).
- **Integration tests**: Add to `tests/` directory.
- **Benchmarks**: Add to `benches/` directory.

### New Documentation
- **API documentation**: Update `docs/api-reference.md`.
- **Usage examples**: Add to `docs/examples-guide.md` or create new files in `docs/`.
- **Setup/deployment guides**: Place in appropriate `docs/` files.

### New Scripts and Configurations
- **Build/utility scripts**: Add to `scripts/`.
- **Configuration templates**: Place in `config/`.
- **CI/CD changes**: Modify `.gitlab-ci.yml` or `.github/` files.

## Cleanup Directives for Future Development

### Regular Maintenance
- **Remove unused code**: Periodically audit and remove dead code, unused imports, and obsolete files.
- **Update dependencies**: Keep Cargo.toml and package.json dependencies current, removing unused crates/packages.
- **Archive old documents**: Move outdated plans and reports to an `archive/` subdirectory or separate repository.

### Code Organization
- **Refactor when necessary**: As modules grow, consider splitting large files into smaller, focused modules.
- **Maintain naming consistency**: Follow Rust naming conventions (snake_case for files/modules) and TypeScript conventions.
- **Keep imports clean**: Use `pub use` for clean module interfaces and avoid wildcard imports.

### Documentation Updates
- **Keep docs current**: Update documentation whenever APIs or functionality changes.
- **Review examples**: Ensure example code remains functional and reflects current best practices.
- **Consolidate similar docs**: Merge redundant documentation files to avoid duplication.

### Testing and Quality
- **Expand test coverage**: Add tests for new features and regularly run the full test suite.
- **Performance monitoring**: Use benchmarks to identify and optimize performance bottlenecks.
- **Security audits**: Regularly review code for security vulnerabilities and update dependencies.

## Examples of File Placement

### Adding a New Technical Indicator
- **Code**: `src/patterns/indicators.rs` (add new indicator implementation)
- **Tests**: `src/patterns/indicators.rs` (add unit tests) or `tests/pattern_tests.rs`
- **Benchmarks**: `benches/pattern_benchmarks.rs` (add performance tests)
- **Documentation**: `docs/api-reference.md` (document the new indicator)

### Implementing a New Broker Integration
- **Code**: `src/brokers/new_broker/` (create new subdirectory with mod.rs, config.rs, rest.rs, websocket.rs)
- **Types**: `src/brokers/new_broker/types.rs`
- **Tests**: `tests/new_broker_integration.rs`
- **Examples**: `examples/new_broker_basic.rs`
- **Documentation**: `docs/api-usage-examples.md` (add integration example)

### Adding a New UI Component
- **Component**: `daytrader/src/components/NewComponent.tsx`
- **Store**: `daytrader/src/store/newStore.ts` (if new state management needed)
- **Tauri Command**: `daytrader/src-tauri/src/commands/new_command.rs` (if backend interaction required)
- **Tests**: Add component tests in a dedicated test file or alongside the component

### Creating a New Risk Rule
- **Code**: `src/risk/rules.rs` (add new rule implementation)
- **Types**: `src/risk/types.rs` (add any new types)
- **Tests**: `tests/risk_integration.rs` (add integration tests)
- **Documentation**: `docs/api-reference.md` (document the rule)

This organization scheme provides a solid foundation for maintaining a clean, scalable, and maintainable codebase. Regular adherence to these guidelines will ensure the project remains organized as it evolves.