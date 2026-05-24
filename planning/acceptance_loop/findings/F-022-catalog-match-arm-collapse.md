# F-022 — Collapse remaining catalog `match` blocks after F-003

- **Stage:** substrate
- **Severity:** low
- **Status:** deferred — depends on F-003
- **First found in:** round-01 (2026-05-24)
- **Effort:** M (~100 LOC, trait dispatch)
- **Linked PRs:** —
- **Source audits:** suggest-pipeline

## Evidence

F-010 documents that `catalog.rs` has six 23-arm `match` blocks. Two
of those will collapse via F-003 (`new_default` partially +
`apply_stock_defaults` fully + `operation_feeds_hints` via F-011).

This finding tracks the audit work to do AFTER F-003 lands: review
the remaining `match` blocks and decide which can collapse via the
`OperationParams` trait or other dispatch mechanisms.

## Acceptance test

1. **Count test**: count distinct 23-arm `match` blocks in
   `catalog.rs` and `properties/mod.rs`. Target after this fix: ≤ 2.
2. **Trait coverage**: each `OperationConfig` variant implements
   every method that previously required a match arm. The 23-arm
   structure remains only for `new_default` (legitimately) and
   possibly `kind_str` (enumeration is fine).

## Files

- `crates/rs_cam_core/src/compute/catalog.rs:200-443, 1429-1601`
  (remaining match blocks after F-003)
- `crates/rs_cam_core/src/compute/operation_configs.rs` — trait impls
- `crates/rs_cam_core/src/compute/params.rs` (if it exists) —
  `OperationParams` trait

## Fix shape

After F-003 lands:

1. Run `rg "match op|match operation_type|match kind" crates/rs_cam_core/src/compute/`
2. For each remaining match block:
   - Decide if it's genuinely op-specific (keep) or trait-dispatchable (collapse)
   - If trait-dispatchable, add the method to `OperationParams`
   - Replace match arm with `op.method()`
3. Verify smoke + param_sweep still pass

## Risk

Low. Pure refactor. Smoke suite covers every op kind.

## Notes

- **Strictly blocked by F-003.** Do not start before F-003 lands.
- **Out of scope:** restructuring `OperationConfig` enum or
  `OperationParams` trait shape.
