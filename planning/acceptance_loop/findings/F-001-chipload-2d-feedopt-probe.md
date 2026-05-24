# F-001 — Chipload "2D dead" is a feedopt sequencing bug

- **Stage:** sim
- **Severity:** high
- **Status:** landed
- **First found in:** round-01 (2026-05-24)
- **Effort:** S (~10-30 LOC)
- **Linked PRs:** —
- **Verified in round:** —
- **Source audits:** chipload-root-cause + unification

## Evidence

`crates/rs_cam_core/src/compute/execute.rs:1570-1578` probes
`nominal_feed_rate` from "the first Linear move in the dressed
toolpath". But the entry dressup (step 1 of `apply_dressups`) prepends
a ramp / helix / plunge at `plunge_rate` (typically 20–30% of cutting
feed). Feedopt then rewrites every cutting move to
`plunge_rate × RCTF(engagement)`. In sim, the chipload classifier at
`tool_load/chipload.rs:184-200` filters samples by
`s.feed_rate_mm_min ≥ 0.95 × operation_feed_rate_mm_min`. With the
above rewrite, no sample matches — `steady_samples.is_empty()` returns
`SteadyStateSamplesNotPresent`.

3D ops (`adaptive3d`, `drop_cutter`, `scallop`, `waterline`,
`horizontal_finish`, etc.) skip feedopt entirely via
`catalog.rs:feed_optimization_unavailable_reason` at lines 1581–1601,
so they pass the filter and the gate fires correctly.

## Smoke evidence

Cases AS001 / AS002 / AS003 / AS005 / AS007 / AS008 in
`rounds/round-01-2026-05-24/baseline.md` all returned `chipload:
Unmodeled` with reason `steady_state_samples_not_present`, including
on a 5,998-move 2D adaptive and a 136,210-move V-carve. Cases AS013 /
AS014 / AS015 (3D) fired correctly with `exceeds_low` against vendor
LUT bounds.

## Acceptance test

1. **Unit test** (must be in PR): add a test under
   `crates/rs_cam_core/src/compute/execute.rs` that constructs a pocket
   op with `feed_rate=900`, ramp entry, and `plunge_rate=350`; runs
   `apply_dressups`; asserts the `feedopt::OptimizeFeedRatesParams`
   passed downstream has `nominal_feed_rate == 900.0` (not 350.0).
2. **Smoke verification** (auditor's next round): cases AS001 / 2 / 3 /
   5 / 7 must report `chipload: Within` OR `chipload: Exceeds(side: …)`
   — NOT `Unmodeled / steady_state_samples_not_present`.

## Files

- `crates/rs_cam_core/src/compute/execute.rs:1570-1578` — the probe
- `crates/rs_cam_core/src/compute/execute.rs:201` — `execute_operation` caller; needs to pass `op.feed_rate()` through
- `crates/rs_cam_core/src/feedopt.rs:116-184` — `OptimizeFeedRatesParams` consumer
- `crates/rs_cam_core/src/tool_load/chipload.rs:184-200` — filter that rejects rewritten samples

## Fix shape

Thread `op.feed_rate()` (already exposed by `OperationParams` trait)
through `apply_dressups` and into the feedopt params struct as the
nominal feed source. Stop probing the post-dressup move stream.

## Risk

Low. Threading is mechanical. Only way to break things is if some op
intentionally relied on the post-dressup probe — none should.

## Notes

- **Defensive alternative** considered but rejected: make
  `steady_state_samples_for_toolpath` use `MoveIntent::OperationCut` /
  `ClearingCut` span ancestry instead of feed comparison. That would
  decouple chipload from feedopt entirely, but it's multi-file and
  requires guaranteeing intent tags on all 2D ops first. Single-site
  fix here is the cleaner first move; defensive fix can follow if the
  feed-comparison heuristic keeps biting.
- **Out of scope for this fix:** the chipload classifier's bounds /
  heuristics themselves. Don't tune.
