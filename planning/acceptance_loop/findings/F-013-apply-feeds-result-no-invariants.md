# F-013 — `apply_feeds_result_to_op` never enforces invariants

- **Stage:** suggest
- **Severity:** high
- **Status:** landed (with F-003)
- **First found in:** round-01 (2026-05-24)
- **Effort:** S (rolled into F-003)
- **Linked PRs:** —
- **Source audits:** suggest-pipeline

## Evidence

`crates/rs_cam_viz/src/ui/properties/mod.rs:95
apply_feeds_result_to_op` never:

- Checks `plunge_rate ≤ feed_rate` (the historic
  "plunge 527 > feed 385" bug returns trivially anywhere this
  function runs)
- Re-applies machine clamp on stepover after a LUT row overrides it
- Warns when `axial_depth_mm` came from a fallback formula vs a real
  vendor row

Combined with F-007 (drill `set_plunge_rate` no-op), this is why
suggestions land outside envelopes even though the LUT was consulted.

## Acceptance test

Covered by F-003's invariant tests:

1. `suggest_params` returns `plunge_rate ≤ feed_rate` for all ops
2. `suggest_params` returns `stepover ≤ tool.diameter`
3. `suggest_params` returns `depth_per_pass ≤ machine.rigidity.doc_*_factor × tool.diameter` for roughing
4. `suggest_params` returns `depth_per_pass ≤ tool.cutting_length`

## Files

- `crates/rs_cam_viz/src/ui/properties/mod.rs:95` — deleted by F-003
- Replaced by invariant-enforcing logic inside `core::feeds::suggest::suggest_params`

## Fix shape

Rolled into F-003. The new `suggest_params` function enforces all
four invariants before returning. The old `apply_feeds_result_to_op`
helper disappears.

## Risk

None standalone — closes with F-003.

## Notes

- **Rolled into F-003.** Keep this finding for traceability — when
  F-003 lands, the auditor can confirm each invariant has a passing
  unit test and close both findings together.
