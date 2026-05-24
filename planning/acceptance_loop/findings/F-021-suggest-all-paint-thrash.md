# F-021 — Suggest All button recomputes LUT every paint

- **Stage:** viz (performance + state churn)
- **Severity:** low
- **Status:** open
- **First found in:** round-01 (2026-05-24, suggest-pipeline audit)
- **Effort:** S (~50 LOC)
- **Linked PRs:** —
- **Source audits:** suggest-pipeline

## Evidence

`crates/rs_cam_viz/src/ui/properties/mod.rs:2752` — the Suggest All
button on the Params tab (PR-2C Phase 1 work) recomputes the LUT
each frame and writes back into `entry.feeds_result` **even when not
clicked**. The auditing agent flagged this as inconclusive but
worth investigating because it could:

- Trigger UI flicker or stale-flag churn on every paint
- Silently invalidate sim caches every frame (cost: a full re-sim
  next request)
- Mask the actual user-initiated Suggest action behind constant
  recomputation

## Acceptance test

1. **Instrumentation test**: log LUT lookup count during 60 paint
   frames with no user interaction. Expected: 0 lookups (or 1 cached
   hit). Actual today: suspected N lookups (one per frame).
2. **Cache test**: assert `entry.feeds_result` is not mutated unless
   the user clicked Suggest All or a dependent input changed.
3. **Stale-flag test**: assert that idle frames do not cause any
   toolpath's `stale` flag to flip.

## Files

- `crates/rs_cam_viz/src/ui/properties/mod.rs:2752` — Suggest All
  button rendering
- Surrounding `draw_params_tab` / `draw_feeds_card` (`:1088`) — paint
  cycle that includes the button

## Fix shape

Compute LUT result lazily — cache by (op_kind, tool, material,
workholding) tuple and only recompute when one of those inputs
changes. Render uses the cached value. The button click forces a
recompute.

## Risk

S. Pure viz performance fix. Touch only the Suggest All path.

## Notes

- **Out of scope:** the broader paint-cycle audit of the Params tab.
- Watch for the same pattern in other "Suggest" pills
  (`crates/rs_cam_viz/src/ui/properties/operations/mod.rs:127` inline
  ⚡ suggest_pill). Same risk class, possibly already cached.
