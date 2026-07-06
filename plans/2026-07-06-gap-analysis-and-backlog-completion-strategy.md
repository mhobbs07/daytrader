# Gap Analysis & Backlog Completion Strategy

**Date:** 2026-07-06
**Author:** Orchestrator session (full-repo audit)
**Inputs:** 3 parallel audits (branch landscape, backend, frontend) + verification pipeline runs
**Status:** Executing

---

## 1. Executive Summary

The project is much further along than BACKLOG.md (last updated 2026-04-19) suggests. The
single biggest problem is not missing code — it is **unmerged, unreconciled work scattered
across ~10 branches**, a stale backlog, and governance tooling that doesn't enforce the
project's own most safety-critical rules. Several "pending" backlog items (including the only
Critical one, B-029) are already implemented and verify-green on `feat/techdebt-unblock-sweep`.

Strategy: **land what exists → correct the record → implement what's genuinely missing in
priority waves → harden the governance gate so drift can't recur.**

---

## 2. Gap Inventory

### 2.1 Process/branch gaps (root cause of most confusion)

| Gap | Evidence | Fix |
|-----|----------|-----|
| `main` is 3 commits ahead of `staging` | PRs #4/#5 merged to `main` directly, violating branching.md | Land via techdebt-sweep (contains main's tip as ancestor) |
| 11-commit verified superset branch unmerged for 10+ weeks | `feat/techdebt-unblock-sweep`: B-029, B-033, B-038, B-040, B-041, B-047, B-055, B-056 + src/data gitignore bug fix | PR → staging (this session) |
| `feat/b042-pdt-reconcile` is a mislabeled duplicate of `main` | Identical SHA; no PDT content | Delete branch ref |
| `fix/b057-decimal-privacy` claims Done in BACKLOG but commit says "UNVERIFIED / do not merge" | Commit fc7018f message | Do not merge; re-derive if still needed after techdebt lands |
| Real B-042 fix stranded on `feature/b008-pdt-holiday-edge-cases` | Commit 20cb752 integrates MarketHolidayCalendar into PdtEnforcer; never merged | Cherry-pick/reimplement on fresh branch (Wave 2) |
| `fix/review-mcp-security-hardening` (8 commits, 6 MCP servers, +14.7k lines) failed 5 verification checks | Plan doc note 2026-05-26 | Out of app-critical path; triage separately, needs security review |
| 7 stale worktrees, 2 with uncaptured work (B-036 types module, B-047) | worktree list audit | Salvage B-036 commit; prune the rest |
| BACKLOG.md stale: B-045/B-046 listed pending but already done; B-055–B-057 missing entirely | Backend audit | Rewrite BACKLOG.md (this session) |

### 2.2 Product gaps — the app doesn't visibly work

| Gap | Status |
|-----|--------|
| No market data flows (MockBroker no-op) → dashboard permanently empty | **Fixed on techdebt-sweep** (MockDataDriver, B-029+B-033); needs landing + e2e verify |
| No candle chart (B-030) | Pending — lightweight-charts installed but never imported |
| PDT panel absent (B-032) | Pending — 5 backend PDT commands fully implemented, zero frontend consumers |
| Risk updates poll-only (B-034) | Pending — needs new event emit in EventBridge + riskStore listener |
| Single hardcoded AAPL symbol (B-039), no pattern filters (B-040: done on techdebt), no types module (B-036: salvageable commit exists), no hooks (B-037) | Pending |
| `get_recent_risk_violations` is a stub returning `[]` and masks DB errors as "no violations" | Not in backlog — add task (risk-dashboard correctness) |

### 2.3 Compliance/safety gaps (highest severity relative to attention)

| Gap | Detail |
|-----|--------|
| PDT enforcer ignores market holidays | `PdtState::is_business_day` is weekday-only; display commands ARE holiday-aware → enforcement/display inconsistency. B-042. |
| Day-trade detection stubbed to `false` in RiskEngine | `risk/engine.rs:254,310` — day trades may not be counted at all in engine path; two parallel unreconciled implementations (RiskEngine vs PdtEnforcer) |
| Emergency liquidation unimplemented | `risk/engine.rs:589` — configured feature (`emergency_liquidate_threshold`) with no execution path |
| ET session boundaries use fixed UTC offsets | No `chrono-tz`; open/close detection off by 1h for part of the year (B-043 real content) |
| Production `.unwrap()` on config-supplied values | `risk/limits.rs:57-63` panics on NaN/inf config; also `alpaca/rest.rs:27` expect |
| No Alpaca rate limiting / fake exponential backoff | B-044; README claims backoff that doesn't exist |

### 2.4 Governance gaps (why drift happened silently)

1. Verify script's test counter uses GNU-only `grep -oP` → always shows "0 passed" on macOS.
2. No check for `.unwrap()`/`.expect()`/`panic!()` in production code (stated forbidden rule).
3. No check for `std::sync` primitives (stated forbidden; violated in `db/mock.rs`, `utils/holidays.rs`).
4. Clippy warnings non-blocking; 21 test failures permanently baselined with no burn-down.
5. Integration tests skip when nothing is *staged* — easy to silently skip.
6. Zero frontend tests, and no frontend check in the pipeline at all (`npm run build` not gated).

---

## 3. Execution Plan

### Wave 0 — Land existing work (this session, sequential)
1. Verify `feat/techdebt-unblock-sweep` (fmt/build/lib tests ✅ done; integration + npm build in flight) → PR → merge to staging. Lands B-029, B-033, B-038, B-040, B-041, B-047, B-055, B-056 + main reconciliation.
2. PR `chore/governance-audit-overhaul` → staging (pure tooling; merge after techdebt to resolve `.gitignore`/settings overlaps).
3. Delete duplicate/stale branch refs (`feat/b042-pdt-reconcile`, merged worktree branches); prune stale worktrees.
4. Rewrite BACKLOG.md to match reality; mark B-045/B-046 done; register B-055–B-058+ and new gap tasks.

### Wave 1 — Make the app demonstrably work (parallel agents, sonnet, ≤4)
- B-030 candle chart (consumes candles already streamed by MockDataDriver)
- B-032 PDT status panel (backend commands already exist)
- B-034 risk violation push events (EventBridge variant + riskStore listener) **+ fix `get_recent_risk_violations` stub**
- B-036 TypeScript types module (salvage worktree commit 7074a2a, refresh, verify)

### Wave 2 — Compliance correctness (parallel agents, sonnet)
- B-042 PDT holiday integration (start from stranded commit 20cb752, re-verify, extend tests)
- B-043 ET session handling via `chrono-tz`
- Risk engine day-trade detection TODO + violation-type TODO (new task B-058)
- Production unwrap/expect fixes in risk/limits, risk/engine, alpaca/rest (new task B-059)

### Wave 3 — Quality & hardening (parallel agents, sonnet)
- B-037 custom hooks, B-039 multi-symbol watchlist
- B-044 Alpaca rate limiting (token bucket + real Retry-After + exponential WS backoff)
- Governance hardening (new task B-060): fix `grep -oP`, add unwrap/std-sync checks, gate `npm run build`, known-failures burn-down policy
- B-048 mock data generator consolidation

### Deferred (documented, not silently dropped)
- B-049 load testing, B-050 release automation, B-051 monitoring, B-052 UAT, B-053 IBKR, B-054 backtesting — infra-scale items; sequenced after the app demonstrably works end-to-end.
- MCP server suite (`fix/review-mcp-security-hardening`) — not in BACKLOG, large new attack surface, failed verification; needs its own security-reviewed track.
- Emergency liquidation implementation — needs a design decision (auto-liquidation in a real account is dangerous); flagged as B-061 requiring human sign-off on semantics before implementation.

### Rules of engagement
- All work on `feat/*`/`fix/*` branches → PR → staging. Never commit to staging/main directly. No force-pushes.
- Every branch must pass `./scripts/verify_agent_work.sh` (with DATABASE_URL set) + `npm run build` before PR.
- Subagents: sonnet, max 4 concurrent, worktree-isolated.
- staging → main promotion remains a human decision.
