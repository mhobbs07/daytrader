---
paths:
  - "src/**/*.rs"
---

# Concurrency Rules

This is a Tokio async runtime project. These rules prevent deadlocks and runtime blocking:

1. **`Arc<T>` for shared ownership** across async task boundaries.
2. **`DashMap`** for concurrent hash maps (established pattern in this codebase).
3. **`tokio::sync`** for all async locks (`Mutex`, `RwLock`, `Semaphore`). NEVER use `std::sync::Mutex` or `std::sync::RwLock` — they block the Tokio runtime.
4. **EventBus subscribers MUST handle `RecvError::Lagged`** — dropped messages are expected under load. Never ignore this variant.
5. **Prefer channels over shared state** when communicating between tasks.
