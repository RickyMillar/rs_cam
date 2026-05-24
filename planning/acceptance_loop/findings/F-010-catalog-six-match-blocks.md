# F-010 — Catalog has six 23-arm `match` blocks

- **Stage:** substrate
- **Severity:** low
- **Status:** open — re-evaluate after F-003 lands
- **First found in:** round-01 (2026-05-24)
- **Effort:** M (~150 LOC, mostly trait plumbing)
- **Linked PRs:** —
- **Source audits:** suggest-pipeline + unification

## Evidence

`crates/rs_cam_core/src/compute/catalog.rs` has six 23-arm `match`
blocks covering every `OperationType` variant:

- `spec` (line 200-443)
- `new_default` (line 1429)
- `apply_stock_defaults` (line 1440-1474)
- `as_params` (line ~1495)
- `as_params_mut` (line ~1520)
- `kind_str`

Plus a mirrored hint table at
`crates/rs_cam_viz/src/ui/properties/mod.rs:1051`
(`operation_feeds_hints`).

Adding any new field that's per-op touches N (≈ 23) sites in 6
locations. `OperationConfig` is essentially an enum-of-structs that
costs O(N_variants × N_consumers) per change.

## Acceptance test

1. **Pre-fix**: count distinct 23-arm `match` blocks in `catalog.rs`
   and `properties/mod.rs`. Baseline = 7.
2. **Post-fix**: count is ≤ 2 (`new_default` likely justified; one
   other if genuinely op-specific). All others collapse via the
   `OperationParams` trait or similar.

## Files

- `crates/rs_cam_core/src/compute/catalog.rs:200-443, 1429-1601`
- `crates/rs_cam_viz/src/ui/properties/mod.rs:1051`
- `crates/rs_cam_core/src/compute/operation_configs.rs` — 23 op
  configs that need the trait impl

## Fix shape

After F-003's `suggest_params` lands and absorbs `apply_stock_defaults`
+ `operation_feeds_hints`, audit which of the remaining match blocks
can be replaced by trait dispatch on `OperationParams`. Hint:
`as_params` / `as_params_mut` might already be the trait; `spec` and
`kind_str` may genuinely need an enumeration.

## Risk

Low. Pure refactor. Tests exist (the smoke suite exercises every op
kind).

## Notes

- **Blocked on F-003 landing.** Don't attempt before — many of the
  match arms are about to disappear when `apply_stock_defaults`
  moves.
- **Out of scope:** restructuring `OperationConfig` itself.
