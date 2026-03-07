# DayTrader Agent Backlog

Last Updated: 2026-03-07

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
| B-007 | Agent governance framework | In Progress | 2026-03-07 | claude/strange-dubinsky | CLAUDE.md, verification scripts, backlog, hooks |

## Pending Tasks

| ID | Task | Priority | Source | Scope |
|----|------|----------|--------|-------|
| B-008 | Fix PDT holiday edge cases | High | progress-tracking.md | risk/pdt, utils/holidays |
| B-009 | Time zone handling (DST transitions) | High | progress-tracking.md | risk/pdt, utils |
| B-010 | Rate limiting improvements | Medium | progress-tracking.md | brokers/alpaca |
| B-011 | Database performance optimization | Medium | progress-tracking.md | db/ |
| B-012 | Event filtering completion | Medium | progress-tracking.md | events/filters |
| B-013 | React component development | Medium | progress-tracking.md | daytrader/src-tauri |
| B-014 | State management (Zustand stores) | Medium | progress-tracking.md | daytrader/src-tauri |
| B-015 | IPC commands for backend integration | Medium | progress-tracking.md | daytrader/src-tauri |
| B-016 | Mock data generator improvements | Low | progress-tracking.md | tests/ |
| B-017 | Load testing implementation | Low | progress-tracking.md | tests/ |
| B-018 | Release automation pipeline | Low | progress-tracking.md | .github/workflows, .gitlab-ci.yml |
| B-019 | Production monitoring setup | Low | progress-tracking.md | infrastructure |
| B-020 | User acceptance testing (PDT) | High | progress-tracking.md | tests/, risk/ |
| B-021 | Interactive Brokers integration | Low | progress-tracking.md | brokers/ |
| B-022 | Phase 5: UI development | Medium | plans/progress-tracking.md | daytrader/ |
| B-023 | Phase 6: Backtesting & validation | Medium | plans/progress-tracking.md | tests/, patterns/ |
| B-024 | Fix trivial test in src/risk/mod.rs (assert!(true)) | Low | governance audit | risk/ |

## Completed Tasks

| ID | Task | Completed | Branch | Verification | Summary |
|----|------|-----------|--------|--------------|---------|
| B-001 | Phase 1: Core infrastructure | 2025 | main | Pre-governance | Broker abstraction, event bus, data ingestion |
| B-002 | Phase 2: Risk management | 2025 | main | Pre-governance | PDT compliance, risk engine, position sizing |
| B-003 | Phase 3A: Pattern detection (10 patterns) | 2025 | main | Pre-governance | Candlestick patterns, technical indicators |
| B-004 | Phase 3D: Integration & optimization | 2025 | main | Pre-governance | Database, streaming, performance tuning |
| B-005 | Phase 4A: Order management | 2026-01-25 | main | Pre-governance | 2,649 lines, 5 modules |
| B-006 | Phase 4B-C: Position tracking & execution | 2026-01-25 | main | Pre-governance | Position tracker, execution engine |

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
