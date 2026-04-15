---
paths:
  - "src/**/*.rs"
  - "daytrader/src-tauri/**/*.rs"
---

# Financial Security Rules

This is an algorithmic trading platform. Security violations can cause real financial loss.

1. **API keys never in source code**. Use `keyring` crate or environment variables.
2. **All broker communication over TLS** (`rustls`). No plaintext HTTP for financial APIs.
3. **PDT compliance checks MUST NEVER be bypassed or disabled.** Pattern Day Trader rules are SEC-mandated. The risk engine enforces these — do not add escape hatches.
4. **Risk limits validated BEFORE order submission** (pre-trade, not post-trade). An order that reaches the broker has already passed all risk checks.
5. **Never commit config files containing credentials** (`.env`, `credentials.json`, API keys).
6. **All external inputs validated at the boundary** before entering business logic. This includes broker API responses, user input from Tauri frontend, and database query results.
