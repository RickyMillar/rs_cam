---
name: lint-fix
description: Fix common clippy lint violations with project-approved patterns
allowed-tools: Read, Grep, Edit, Bash
---

# /lint-fix — Resolve Clippy Lint Violations

This skill provides the approved fix patterns for each denied lint in this workspace. Use these patterns instead of inventing ad-hoc solutions.

## Quick reference

Run clippy to see violations:
```bash
cargo clippy --workspace --all-targets -- -D warnings
```

---

## unwrap_used / expect_used

**Problem:** `.unwrap()` or `.expect()` called on Option/Result.

**Fixes (in order of preference):**

1. **Propagate with `?`** — if the function returns Result/Option:
   ```rust
   let val = something.ok_or(MyError::Missing)?;
   ```

2. **Use safe alternatives:**
   ```rust
   let val = opt.unwrap_or(default);
   let val = opt.unwrap_or_else(|| compute_default());
   let val = opt.unwrap_or_default();
   ```

3. **Allow with safety comment** — only when provably safe:
   ```rust
   // SAFETY: vec is non-empty — initialized with [0.0] above
   #[allow(clippy::expect_used)]
   let last = cum_dist.last().expect("non-empty");
   ```

**Test code:** Test modules already carry `#[allow(clippy::unwrap_used, clippy::expect_used)]`.

---

## indexing_slicing

**Problem:** `arr[i]` or `&arr[start..end]` can panic on out-of-bounds.

**Fixes (in order of preference):**

1. **Use iterators** for sequential access:
   ```rust
   // Before:
   for i in 0..points.len() - 1 {
       process(points[i], points[i + 1]);
   }
   // After:
   for pair in points.windows(2) {
       // SAFETY: windows(2) guarantees 2 elements
       #[allow(clippy::indexing_slicing)]
       process(pair[0], pair[1]);
   }
   ```

2. **Use `.first()` / `.last()`** for endpoint access:
   ```rust
   let first = points.first().copied().unwrap_or_default();
   let last = points.last().copied().unwrap_or_default();
   ```

3. **Destructure fixed-size arrays:**
   ```rust
   // Before: vertices[tri[0]], vertices[tri[1]], vertices[tri[2]]
   let [i0, i1, i2] = *tri;
   ```

4. **Allow with safety comment** for bounded loops:
   ```rust
   // SAFETY: row < rows, col < cols — bounded by loop
   #[allow(clippy::indexing_slicing)]
   for row in 0..rows {
       for col in 0..cols {
           let cell = &grid[row * cols + col];
       }
   }
   ```

---

## panic / todo / unimplemented

**Problem:** `panic!()`, `todo!()`, `unimplemented!()` in non-test code.

**Fixes:**
- Replace `todo!()` with actual implementation or return an error
- Replace `panic!("unreachable")` with `unreachable!()` (which is allowed)
- Replace `unimplemented!()` with a proper error return

---

## print_stdout / print_stderr

**Problem:** `println!()` or `eprintln!()` in non-CLI code.

**Fix:** Use tracing:
```rust
tracing::info!("message");
tracing::warn!("warning: {}", detail);
tracing::error!("failed: {error}");
```

**Exception:** The CLI crate (`rs_cam_cli`) allows `print_stderr` for user-facing diagnostic output.

---

## map_err_ignore

**Problem:** `.map_err(|_| ...)` discards the original error.

**Fix:** Capture the error even if unused:
```rust
// Before:
.map_err(|_| "operation failed".to_string())
// After:
.map_err(|_e| "operation failed".to_string())
```

Or better, include it in the message:
```rust
.map_err(|e| format!("operation failed: {e}"))
```

---

## redundant_clone

**Problem:** `.clone()` on a value that's about to be moved/consumed.

**Fix:** Remove the `.clone()`:
```rust
// Before:
let result = detect_containment(vec![poly.clone()]);
// After:
let result = detect_containment(vec![poly]);
```

---

## needless_pass_by_value

**Problem:** Function takes `Vec<T>` or `String` by value but only reads it.

**Fix:** Change to a borrow:
```rust
// Before:
fn process(items: Vec<Item>) { for item in &items { ... } }
// After:
fn process(items: &[Item]) { for item in items { ... } }
```

Common conversions: `Vec<T>` → `&[T]`, `String` → `&str`

---

## Adding #[allow] to test modules

When creating a new test module, include the standard test allows:
```rust
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    // ...
}
```

For standalone test files (`tests/*.rs`), use inner attributes:
```rust
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
```
