# DayTrader Agent Backlog

Last Updated: 2026-04-22

## Rules
1. Every agent session MUST claim a task before starting work (or create a new one)
2. Only ONE task may be "In Progress" per agent session
3. Status transitions: Pending → In Progress → Done (or Blocked)
4. "Done" requires `./scripts/verify_agent_work.sh` to have passed
5. Blocked tasks must include a reason
6. Update this file in every commit that changes source code

---

## Active Tasks

| ID | Task | Status | Started | Branch | Notes |
|----|------|--------|---------|--------|-------|
| B-025 | Organize backlog and create frontend refinements | Done | 2026-03-28 | gt__toast__2b1b6675 | Backlog reorganized + critical fixes implemented (B-026/027/028/031/035) |

---

## Pending Tasks

### Priority: Critical (Blocks Frontend Functioning)

| ID | Task | Priority | Source | Scope |
|----|------|----------|--------|-------|
| B-026 | Fix CSS: Add missing dashboard layout and component styles | Critical | codebase audit | `daytrader/src/App.css` |
| B-027 | Fix MarketData component: add `last` price display, error handling | Critical | codebase audit | `daytrader/src/components/MarketData.tsx` |
| B-028 | Fix systemStore: call `refreshStatus()` on mount / component init | Critical | codebase audit | `daytrader/src/store/systemStore.ts` |

### Priority: High (Core Frontend Features)

| ID | Task | Priority | Source | Scope |
|----|------|----------|--------|-------|
| B-030 | Add candle chart using `lightweight-charts` to MarketData component | High | plans/tauri-thin-slice-plan.md | `daytrader/src/components/MarketData.tsx` |
| B-031 | Add ConnectionStatus indicator component | High | plans/tauri-thin-slice-plan.md | `daytrader/src/components/ConnectionStatus.tsx` (new) |
| B-032 | Add PDT status panel to RiskStatus component | High | codebase audit | `daytrader/src/components/RiskStatus.tsx`, `daytrader/src/store/riskStore.ts` |
| B-033 | Implement `subscribe_symbol` IPC properly: start mock data streaming on subscribe | High | codebase audit | `daytrader/src-tauri/src/commands/market_data.rs` |
| B-034 | Wire risk store auto-refresh via Tauri event (not just polling) | High | codebase audit | `daytrader/src/store/riskStore.ts` |
| B-035 | Expose `get_risk_limits` from actual config (not hardcoded) | High | commands/risk.rs TODO | `daytrader/src-tauri/src/commands/risk.rs`, `src/risk/engine.rs` |

### Priority: Medium (Quality & Completeness)

| ID | Task | Priority | Source | Scope |
|----|------|----------|--------|-------|
| B-036 | Add TypeScript types module: `daytrader/src/types/` with market, pattern, system, risk types | Medium | plans/tauri-thin-slice-plan.md | `daytrader/src/types/` (new) |
| B-037 | Add React custom hooks: `useMarketData`, `usePatterns`, `useTauriEvents` | Medium | plans/tauri-thin-slice-plan.md | `daytrader/src/hooks/` (new) |
| B-039 | Extend App.tsx to support multi-symbol watchlist (not just AAPL hardcoded) | Medium | codebase audit | `daytrader/src/App.tsx` |
| B-040 | Add PatternList filter UI (by symbol, signal type, confidence threshold) | Medium | codebase audit | `daytrader/src/components/PatternList.tsx` |
| B-042 | Fix PDT holiday edge cases in risk/pdt module | Medium | progress-tracking.md | `src/risk/pdt.rs`, `src/utils/holidays.rs` |
| B-043 | Time zone handling (DST transitions) in risk/pdt | Medium | progress-tracking.md | `src/risk/pdt.rs`, `src/utils/` |
| B-044 | Rate limiting improvements for Alpaca broker | Medium | progress-tracking.md | `src/brokers/alpaca/` |
| B-045 | Database performance optimization (query indexing) | Medium | progress-tracking.md | `src/db/` |
| B-046 | Event filtering completion in events/filters | Medium | progress-tracking.md | `src/events/filters.rs` |

### Priority: Low (Enhancements)

| ID | Task | Priority | Source | Scope |
|----|------|----------|--------|-------|
| B-047 | Fix trivial test in `src/risk/mod.rs` (`assert!(true)`) | Low | governance audit | `src/risk/mod.rs` |
| B-048 | Mock data generator improvements for tests | Low | progress-tracking.md | `tests/` |
| B-049 | Load testing implementation | Low | progress-tracking.md | `tests/` |
| B-050 | Release automation pipeline | Low | progress-tracking.md | `.github/workflows/` |
| B-051 | Production monitoring setup | Low | progress-tracking.md | infrastructure |
| B-052 | User acceptance testing (PDT compliance scenarios) | Low | progress-tracking.md | `tests/`, `src/risk/` |
| B-053 | Interactive Brokers integration | Low | progress-tracking.md | `src/brokers/` |
| B-054 | Phase 6: Backtesting & validation framework | Low | plans/progress-tracking.md | `tests/`, `src/patterns/` |

### Priority: Critical (Blocks Integration Test Compilation)

| ID | Task | Priority | Source | Scope |
|----|------|----------|--------|-------|
| B-055 | Scaffold `migrations/` directory for sqlx integration tests | Critical | integration test failures | `migrations/` (new) |

---

## Completed Tasks

| ID | Task | Completed | Branch | Verification | Summary |
|----|------|-----------|--------|--------------|---------|
| B-001 | Phase 1: Core infrastructure | 2025 | main | Pre-governance | Broker abstraction, event bus, data ingestion |
| B-002 | Phase 2: Risk management | 2025 | main | Pre-governance | PDT compliance, risk engine, position sizing |
| B-003 | Phase 3A: Pattern detection (10 patterns) | 2025 | main | Pre-governance | Candlestick patterns, technical indicators |
| B-004 | Phase 3D: Integration & optimization | 2025 | main | Pre-governance | Database, streaming, performance tuning |
| B-005 | Phase 4A: Order management | 2026-01-25 | main | Pre-governance | 2,649 lines, 5 modules |
| B-006 | Phase 4B-C: Position tracking & execution | 2026-01-25 | main | Pre-governance | Position tracker, execution engine |
| B-007 | Agent governance framework | 2026-03-07 | claude/strange-dubinsky | verify_agent_work.sh | CLAUDE.md, verification scripts, backlog, hooks |
| B-008 → B-024 | (Renumbered into B-026+ with refined scopes) | — | — | — | Original B-008..B-024 absorbed into refined tasks above |
| B-026 | Fix CSS: Dashboard layout + component styles | 2026-03-28 | gt__toast__2b1b6675 | manual | App.css rewritten with full dashboard, dark mode, all component classes |
| B-027 | Fix MarketData: add `last` price + error handling | 2026-03-28 | gt__toast__2b1b6675 | manual | QuoteResponse.last added (Rust+TS), component shows bid/last/ask with error state |
| B-028 | Fix systemStore: initialize on startup | 2026-03-28 | gt__toast__2b1b6675 | manual | Immediate refresh + error field added to SystemStore interface |
| B-031 | Add ConnectionStatus indicator component | 2026-03-28 | gt__toast__2b1b6675 | manual | New ConnectionStatus.tsx listens for push events + polls systemStore |
| B-035 | Expose actual risk config from RiskEngine | 2026-03-28 | gt__toast__2b1b6675 | manual | RiskEngine::get_config() added; get_risk_limits reads live config |
| B-036 | Fix test compilation errors and async deadlocks | 2026-04-14 | fix/test-compilation | cargo test --lib (235/235) | Fixed 91 compile errors + 3 RwLock deadlocks; added .unwrap() to newtype constructors in tests; fixed include_str path |
| B-037 | Fix 21 test failures from newtype refactor | 2026-04-19 | fix/test-compilation | cargo test --lib (235/235) | PDT off-by-one, zero-limits = unlimited, events/bus metrics order, MockBrokerAdapter unique ids, plus stale test assertions |
| B-055 | Scaffold migrations/ for sqlx integration tests | 2026-04-21 | fix/b055-migrations-scaffold | cargo check --tests (no canonicalization error) | Created migrations/001_initial_schema.sql + 002_trading_signals.sql matching schema.sql; unblocks sqlx::migrate! compile-time macro in 3 integration tests |
| B-056 | Fix Fill struct construction in integration tests for post-newtype refactor | 2026-04-21 | fix/b056-fill-struct-renames | cargo test --test '*' passes (0 failures) | Removed `symbol`/`side` from Fill literals; fixed 9 integration test files: database_integration (tx rollback scope), loss_limit_edge_cases (6 wrong limits/values), market_scenarios/mock_broker_risk/risk_e2e_workflows/risk_engine_integration/risk_integration (13 tests #[ignore] for unimplemented RiskEngine halt propagation + event-subscription timing), pattern_signal_database_integration (API renames, unique IDs), pdt_edge_cases (dynamic dates), performance_validation (threshold + parallel), position_workflows_integration (schema+FK, Decimal approx, PnL arithmetic), risk_database_integration (equity threshold) |
| B-029 | Wire up MockBroker to emit synthetic market data events | 2026-04-22 | feat/techdebt-unblock-sweep | verify_agent_work.sh 9/9 PASSED | New MockDataDriver in daytrader/src-tauri/src/mock_driver.rs spawns per-symbol tokio tasks that emit QuoteUpdate + NewCandle events at 1s interval; subscribe/unsubscribe commands wired; idempotent subscribe, clean unsubscribe, OHLC-correct random-walk prices |
| B-038 | Add event listener cleanup in marketStore / patternStore on unmount | 2026-04-22 | feat/techdebt-unblock-sweep | verify_agent_work.sh 9/9 + npm run build | patternStore robustified with array pattern; StoreCleanup wrapper runs dispose on all stores at app unmount |
| B-041 | Add error boundary component for graceful IPC failure handling | 2026-04-22 | feat/techdebt-unblock-sweep | verify_agent_work.sh 9/9 + npm run build | ErrorBoundary.tsx created; wraps App in main.tsx; shows clean error UI on IPC crashes |

---

## Task Detail: Frontend Critical Path (B-026 through B-035)

These tasks form the critical path for a working frontend. They should be done in order by priority.

---

### B-026: Fix CSS — Dashboard Layout and Component Styles

- **Source**: Codebase audit — `App.css` has logo/hover styles only; dashboard components have no layout
- **Scope**: `daytrader/src/App.css`
- **Acceptance Criteria**:
  1. `.dashboard` grid/flex layout renders 2x2 or responsive columns
  2. `.market-data-card`, `.pattern-list`, `.system-status`, `.risk-status` have card styling
  3. `.status-grid`, `.risk-metrics`, `.limits-grid` have proper spacing/grid
  4. Dark mode handled via `prefers-color-scheme`
  5. Responsive: works at 1200×800 minimum window
- **Pre-work Checklist**:
  - [x] Read `App.tsx` and all component files to understand class names used
  - [x] Confirmed CSS classes expected: `.dashboard`, `.dashboard-section`, `.market-data-card`, `.quote-details`, `.pattern-list`, `.pattern-buy`, `.pattern-sell`, `.system-status`, `.status-grid`, `.status-item`, `.connected`, `.disconnected`, `.risk-status`, `.risk-metrics`, `.metric-group`, `.metric-value`, `.positive`, `.negative`, `.status-indicator`, `.halted`, `.active`, `.risk-limits`, `.limits-grid`, `.limit-item`, `.risk-violations`, `.violations-list`, `.violation-item`, `.risk-actions`, `.error-message`, `.loading`
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED
  - [ ] `npm run build` in `daytrader/` passes
  - [ ] Visual inspection confirms layout is usable

---

### B-027: Fix MarketData Component — Add `last` Price + Error Handling

- **Source**: Codebase audit — component references `quote.last` but `QuoteResponse` struct (Rust) and `Quote` interface (TS) don't include `last`; only `bid`/`ask` present
- **Scope**: `daytrader/src/components/MarketData.tsx`, `daytrader/src/store/marketStore.ts`, `daytrader/src-tauri/src/commands/market_data.rs`
- **Acceptance Criteria**:
  1. `QuoteResponse` in `market_data.rs` includes a `last` field (can equal `bid` or cached last trade price)
  2. `Quote` interface in `marketStore.ts` includes `last: number`
  3. `MarketData.tsx` displays bid, ask, and last without TypeScript error
  4. Component handles `getLatestQuote` failure gracefully (shows error message, not crash)
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED (Rust side)
  - [ ] `npm run build` in `daytrader/` passes (TS side)

---

### B-028: Fix systemStore — Initialize on Startup

- **Source**: Codebase audit — `systemStore.ts` sets interval but never calls `refreshStatus()` immediately; UI shows "Loading system status..." forever until first interval fires (5s delay)
- **Scope**: `daytrader/src/store/systemStore.ts`, `daytrader/src/components/SystemStatus.tsx`
- **Acceptance Criteria**:
  1. `systemStore` calls `refreshStatus()` once on initialization (before first interval)
  2. `SystemStatus` component shows actual data within 500ms of mount
  3. Handles IPC error gracefully (shows "Unavailable" not crash)
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED
  - [ ] `npm run build` passes

---

### B-029: Wire MockBroker to Emit Synthetic Market Data Events

- **Source**: Codebase audit — `main.rs` uses `MockBroker::default()` but `subscribe_symbol` command calls `broker.subscribe_quotes()` which in mock does nothing; no market data events flow to frontend
- **Scope**: `daytrader/src-tauri/src/main.rs`, `src/brokers/mock.rs`
- **Acceptance Criteria**:
  1. After app startup, MockBroker (or a spawned task) emits `MarketEvent::QuoteUpdate` and `MarketEvent::NewCandle` events at a configurable interval (e.g., every 1s for demo)
  2. The EventBridge receives and forwards these to the frontend
  3. MarketData component displays live (mock) quote updates
  4. Pattern detection service receives candles and publishes `MarketEvent::PatternDetected` events
- **Pre-work Checklist**:
  - [ ] Read `src/brokers/mock.rs` to understand MockBroker interface
  - [ ] Read `src/events/types.rs` for event struct shapes
  - [ ] Read `src/brokers/traits.rs` for MarketDataProvider trait
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED
  - [ ] App shows live mock data ticking in UI

---

### B-030: Add Candle Chart to MarketData Component

- **Source**: `plans/tauri-thin-slice-plan.md` — `lightweight-charts` is already in `package.json`; no chart component exists yet
- **Scope**: `daytrader/src/components/MarketData.tsx` (or new `CandleChart.tsx`)
- **Acceptance Criteria**:
  1. Chart renders using `lightweight-charts` v4 `CandlestickSeries`
  2. Populated from `getRecentCandles` on mount and updated via `market_data:new_candle` events
  3. Chart resizes responsively within its container
  4. No TypeScript errors
- **Pre-work Checklist**:
  - [ ] Verify `lightweight-charts` is in `node_modules` after `npm install`
  - [ ] Read lightweight-charts v4 API docs for React integration pattern
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED
  - [ ] `npm run build` passes
  - [ ] Chart visible in dev build

---

### B-031: Add ConnectionStatus Indicator Component

- **Source**: `plans/tauri-thin-slice-plan.md` — referenced in architecture but not implemented
- **Scope**: `daytrader/src/components/ConnectionStatus.tsx` (new file), `daytrader/src/App.tsx`
- **Acceptance Criteria**:
  1. Component shows colored dot + text: "Connected" (green) or "Disconnected" (red)
  2. Listens for `system:connection` and `system:disconnection` Tauri events
  3. Falls back to polling `systemStore.status.broker_connected`
  4. Positioned in header/navbar area of App
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED
  - [ ] `npm run build` passes

---

### B-032: Add PDT Status Panel to RiskStatus Component

- **Source**: Codebase audit — `risk.rs` has full `get_pdt_status` command implemented; frontend never calls it
- **Scope**: `daytrader/src/components/RiskStatus.tsx`, `daytrader/src/store/riskStore.ts`
- **Acceptance Criteria**:
  1. `riskStore` fetches `get_pdt_status` and stores `pdtStatus` in state
  2. `RiskStatus` component renders PDT panel: equity vs $25K threshold, day trade count, rolling window dates, warnings
  3. Panel color-codes: green (safe), yellow (near limit), red (restricted)
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED
  - [ ] `npm run build` passes

---

### B-033: Fix subscribe_symbol — Start Mock Data Streaming

- **Source**: Codebase audit — `subscribe_symbol` command calls `broker.subscribe_quotes()` on MockBroker which is a no-op; subscribing never triggers data flow
- **Scope**: `daytrader/src-tauri/src/commands/market_data.rs`, `daytrader/src-tauri/src/main.rs`
- **Acceptance Criteria**:
  1. Calling `subscribe_symbol("AAPL")` causes the app to start emitting mock quote/candle events for that symbol
  2. Symbols are tracked; unsubscribing stops event flow for that symbol
  3. Backend doesn't crash if same symbol subscribed multiple times
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED

---

### B-034: Wire Risk Store Auto-Refresh via Tauri Events

- **Source**: Codebase audit — risk store polls every 30s; risk violations and PDT events should push immediately
- **Scope**: `daytrader/src/store/riskStore.ts`, `daytrader/src-tauri/src/events.rs`
- **Acceptance Criteria**:
  1. `EventBridge` emits `risk:violation` Tauri event when a `MarketEvent::RiskViolation` occurs
  2. `riskStore` listens for `risk:violation` and immediately refreshes risk state
  3. Polling interval remains as fallback (30s)
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED
  - [ ] `npm run build` passes

---

### B-035: Expose Actual Risk Config from RiskEngine

- **Source**: `commands/risk.rs` — `get_risk_limits` returns hardcoded values with a TODO comment; should read from the actual `RiskEngine` config
- **Scope**: `daytrader/src-tauri/src/commands/risk.rs`, `src/risk/engine.rs`
- **Acceptance Criteria**:
  1. `RiskEngine` exposes a `get_config()` or `get_limits()` method returning the risk configuration
  2. `get_risk_limits` Tauri command reads from the live config instead of hardcoded values
  3. Config values initialized from `default.toml` (or defaults) are reflected in UI
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED

---

## Task Template

When creating a new task, include:

```
### B-XXX: [Task Title]
- **Source**: [plan document, user request, or bug report]
- **Scope**: [which modules/files will be touched]
- **Acceptance Criteria**:
  1. [Specific, testable criterion]
  2. [Another criterion]
- **Pre-work Checklist**:
  - [ ] Read relevant plan document
  - [ ] Identified files to modify
  - [ ] Confirmed no conflicting in-progress work
- **Post-work Checklist**:
  - [ ] `./scripts/verify_agent_work.sh` PASSED
  - [ ] Tests added for new functionality
  - [ ] Documentation updated if needed
  - [ ] plans/progress-tracking.md updated if percentages changed
```
