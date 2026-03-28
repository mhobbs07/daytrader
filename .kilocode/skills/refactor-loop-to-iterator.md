---
name: refactor-loop-to-iterator
version: 1.0
description: Convert an imperative for-loop to an iterator chain
applies_to: ["*.rs"]
trigger: "for loop building collection or computing aggregate"
---

# Skill: Refactor Imperative Loop to Iterator Chain

Use this skill when you find a `for` loop that builds a collection, computes an aggregate, or searches for a value. Iterator chains are preferred: they are more declarative, eliminate mutable intermediate state, and compose cleanly.

## Decision Tree

Identify your loop type and apply the corresponding transformation:

### Loop pushing to Vec

```rust
// BEFORE
let mut result = Vec::new();
for item in items {
    result.push(transform(item));
}

// AFTER
let result: Vec<_> = items.iter().map(|item| transform(item)).collect();
// or with method reference if transform takes the item directly:
let result: Vec<_> = items.iter().map(transform).collect();
```

### Loop with filter

```rust
// BEFORE
let mut result = Vec::new();
for item in &items {
    if predicate(item) {
        result.push(item.clone());
    }
}

// AFTER
let result: Vec<_> = items.iter().filter(|item| predicate(item)).cloned().collect();

// BEFORE (filter + transform combined)
let mut result = Vec::new();
for item in &items {
    if let Some(value) = maybe_transform(item) {
        result.push(value);
    }
}

// AFTER
let result: Vec<_> = items.iter().filter_map(|item| maybe_transform(item)).collect();
```

### Loop accumulating a scalar

```rust
// BEFORE
let mut sum = Decimal::ZERO;
for item in &items {
    sum += item.value;
}

// AFTER
let sum: Decimal = items.iter().map(|item| item.value).sum();

// BEFORE (general fold)
let mut acc = initial_value;
for item in &items {
    acc = combine(acc, item);
}

// AFTER
let acc = items.iter().fold(initial_value, |acc, item| combine(acc, item));
```

### Loop counting matches

```rust
// BEFORE
let mut count = 0u32;
for item in &items {
    if predicate(item) { count += 1; }
}

// AFTER
let count = items.iter().filter(|item| predicate(item)).count();
```

### Loop over index with window (period-based calculations)

```rust
// BEFORE
let mut result = Vec::new();
for i in (period - 1)..prices.len() {
    let slice = &prices[i.saturating_sub(period - 1)..=i];
    let avg: Decimal = slice.iter().sum::<Decimal>() / Decimal::from(period);
    result.push(avg);
}

// AFTER
let result: Vec<Decimal> = prices
    .windows(period)
    .map(|window| window.iter().sum::<Decimal>() / Decimal::from(period))
    .collect();
```

### Stateful loop (each value depends on previous) — use `.scan()`

```rust
// BEFORE (EMA calculation)
let mut ema = initial_sma;
let mut result = vec![ema];
for &price in &prices[period..] {
    ema = price * multiplier + ema * (Decimal::ONE - multiplier);
    result.push(ema);
}

// AFTER
let result: Vec<Decimal> = std::iter::once(initial_sma)
    .chain(prices[period..].iter().scan(initial_sma, |ema_prev, &price| {
        let ema = price * multiplier + *ema_prev * (Decimal::ONE - multiplier);
        *ema_prev = ema;
        Some(ema)
    }))
    .collect();
```

### Loop with early return

```rust
// BEFORE
for item in &items {
    if predicate(item) { return Some(item); }
}
None

// AFTER
items.iter().find(|item| predicate(item))

// BEFORE (any/all check)
for item in &items {
    if !condition(item) { return false; }
}
true

// AFTER
items.iter().all(|item| condition(item))
```

### Loop over pairs of consecutive elements

```rust
// BEFORE
for i in 1..items.len() {
    process(&items[i-1], &items[i]);
}

// AFTER
items.windows(2).for_each(|w| process(&w[0], &w[1]));
// or if collecting results:
let result: Vec<_> = items.windows(2).map(|w| compute(&w[0], &w[1])).collect();
```

### Loop zipping two parallel collections

```rust
// BEFORE
let mut result = Vec::new();
for i in 0..a.len() {
    result.push(combine(a[i], b[i]));
}

// AFTER
let result: Vec<_> = a.iter().zip(b.iter()).map(|(x, y)| combine(*x, *y)).collect();
```

### Loop building a HashMap

```rust
// BEFORE
let mut map = HashMap::new();
for item in &items {
    map.insert(item.key.clone(), item.value);
}

// AFTER
let map: HashMap<_, _> = items.iter()
    .map(|item| (item.key.clone(), item.value))
    .collect();
```

### Pruning a collection in place

```rust
// BEFORE
let mut i = 0;
while i < vec.len() {
    if should_remove(&vec[i]) { vec.remove(i); } else { i += 1; }
}

// AFTER
vec.retain(|item| !should_remove(item));
```

## Async Rule Evaluation (special case)

`for` loops over `async fn` calls cannot be replaced with synchronous `.all()`. Use:

```rust
// If futures crate is available:
let results = futures::future::join_all(
    rules.iter().map(|rule| rule.check(context))
).await;
let all_pass = results.into_iter().all(|r| r);

// If futures is not available, keep the loop but add a comment:
// Async iteration; sequential evaluation required; .all() not directly applicable
for rule in &self.rules {
    if !rule.check(context).await { return false; }
}
true
```

## Steps

1. **Identify the loop type** — match against the decision tree above
2. **Apply the transformation** — replace the `let mut` + `for` block with the iterator chain
3. **Verify output type matches** — the `collect()` target type must be the same as the original `Vec` type; annotate if the compiler cannot infer it
4. **Run `cargo check`**:
   ```bash
   cargo check
   ```
5. **Run tests for the affected module**:
   ```bash
   cargo test --lib -- <module_name>
   ```
   Output values must be identical — iterator refactors must not change behavior.
6. **Run full verification**:
   ```bash
   ./scripts/verify_agent_work.sh
   ```

## Common Mistakes

- **Ownership**: `.iter()` borrows, `.into_iter()` consumes. Use `.iter()` when the original `Vec` is still needed after; `.into_iter()` when it is not.
- **`.cloned()` vs `.copied()`**: Use `.copied()` for `Copy` types (`f64`, `u32`, `Decimal`); use `.cloned()` for `Clone` types (`String`, `Vec`).
- **`.windows()` requires a slice** — it works on `&[T]`, not on iterators. Call it directly on a `Vec` or slice: `vec.windows(n)`.
- **`.scan()` state** — the initial state is moved into the closure; pass it by value and update `*state` inside.
- **Empty collection edge cases** — verify behavior when the input is empty. `windows(n)` on an empty slice returns no windows; `fold` returns the initial accumulator.
