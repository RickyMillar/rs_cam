# F-011 — `operation_feeds_hints` split across crates

- **Stage:** suggest
- **Severity:** low
- **Status:** open — lands with F-003
- **First found in:** round-01 (2026-05-24)
- **Effort:** S (~30 LOC move)
- **Linked PRs:** —
- **Source audits:** suggest-pipeline

## Evidence

The `operation_feeds_hints` table at
`crates/rs_cam_viz/src/ui/properties/mod.rs:1051` carries the
axial / radial / scallop hint per op kind used to bridge between
`OperationConfig` and `FeedsInput`. But the related `catalog::spec`
table lives in `crates/rs_cam_core/src/compute/catalog.rs:200-443`.

Adding a new op kind requires editing both crates. This is the same
shape as F-010 (catalog match blocks) but specific to the
viz↔core split.

## Acceptance test

1. **Single-source**: assert that the hint table for op kind K exists
   in exactly one location (likely core) after fix.
2. **API**: callers can reach the hints via a core function without
   re-importing viz types.

## Files

- `crates/rs_cam_viz/src/ui/properties/mod.rs:1051` — source location
- `crates/rs_cam_core/src/compute/catalog.rs:200-443` — destination
  context (or sibling)
- `crates/rs_cam_core/src/feeds/` — likely target home (next to
  `feeds::calculate` and the new `suggest_params`)

## Fix shape

Move the table into `core::feeds::operation_hints` (or absorb into
`suggest_params` directly per F-003). Delete the viz copy. Update
any viz call sites to import from core.

## Risk

S. Pure move. Tests cover the data via the smoke suite.

## Notes

- **Lands with F-003.** Don't do this independently — the new
  `suggest_params` will use it.
