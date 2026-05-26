# F-036b — Feature flag plumbing + simulator integration (deferred from F-036)

- **Stage:** simulator → modulation → emitter wiring
- **Severity:** medium (blocks F-036 reaching production; depends on F-036a)
- **Status:** landed 2026-05-26 (8/9 acceptance tests; AB5 deferred to F-036b1)
- **First found in:** F-036 implementation session, 2026-05-26
- **Effort:** M (feature-flag wiring + per-move engagement aggregation)
- **Linked PRs:** (this commit)
- **Workstream:** Feed Modulation (see `planning/feed_modulation_roadmap.md`)
- **Depends on:** F-036 algorithm + **F-036a G-code emission** landed

## Symptom

F-036 landed the modulation algorithm but no production code path invokes it. The algorithm requires:

1. A feature flag (`SimulationOptions::adaptive_feed_modulation: bool`, default `false`) — F-035's `use_predicted_feed_in_gates` is the precedent.
2. Per-move engagement aggregation from the simulator's `SimulationCutSample` stream into the `&[PerMoveEngagement]` shape `adaptive_feed_modulate` consumes.
3. The modulation runs AFTER simulation (samples → aggregation → modulation → re-emit G-code) — an architectural shift. Currently `ProjectSession::generate_toolpath` → `emit_gcode` is one-shot.

This finding plumbs the flag and stages the post-modulation pipeline; F-036a's per-move F-word emitter is the load-bearing dependency.

## Hypothesised root cause

Not a bug — missing wiring. The decoupling decision in F-036's algorithm
(taking `&[PerMoveEngagement]` instead of `&[SimulationCutSample]`) means
this finding's only complexity is the aggregation pass + the
two-pass pipeline ordering.

## Fix shape

1. Add `SimulationOptions::adaptive_feed_modulation: bool` (default `false`). Mirror F-035's `use_predicted_feed_in_gates` plumbing into `KinematicsContext` if needed (or just into `SimulationOptions`).
2. After simulation completes, aggregate per-toolpath samples into a `Vec<PerMoveEngagement>` keyed by `move_index`. Reuse the existing `radial_woc_fraction` + `axial_doc_fraction` Engagement axes.
3. Call `adaptive_feed_modulate(&mut toolpath, &engagements, &ctx)` for each toolpath where the flag is on AND the machine carries `kinematics`.
4. Re-emit G-code from the now-modulated toolpath (the IR's per-move feeds carry the modulation forward).
5. GUI: add a checkbox in the simulation-options UI that flips the flag.

## Acceptance tests

The seven tests originally proposed in F-036's finding file, namespaced to this sub-finding:

```rust
// crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs

#[test]
fn flag_off_emits_identical_gcode_to_pre_f036() {
    // Generate AS001 with adaptive_feed_modulation = false.
    // Assert: emitted G-code byte-identical to the F-035 baseline.
}

#[test]
fn modulated_path_has_per_segment_feed_variation() { /* ... */ }

#[test]
fn modulated_gates_within_constant_chipload_band() { /* ... */ }

#[test]
fn modulated_cycle_time_lower_than_unmodulated() { /* ... */ }

#[test]
fn modulated_path_never_emits_below_min_chipload() { /* ... */ }

#[test]
fn modulated_path_preserves_f024_axial_engagement_invariant() { /* ... */ }

#[test]
fn modulated_path_preserves_zero_rapid_collision_invariant() { /* ... */ }
```

## Files

- `crates/rs_cam_core/src/session/mod.rs` — `SimulationOptions::adaptive_feed_modulation`
- `crates/rs_cam_core/src/session/compute.rs` — invoke modulation post-sim
- `crates/rs_cam_core/src/compute/simulate.rs` — wire flag through `KinematicsContext` if needed
- `crates/rs_cam_viz/src/ui/...` — GUI checkbox
- `crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs` — new

## Risk

M. The two-pass simulate → modulate → re-emit ordering touches the session-compute pipeline. Mitigation: flag default `false`, F-036b's `flag_off_emits_identical_gcode_to_pre_f036` is the load-bearing regression check. The F-036 algorithm itself is already pinned by 12 unit tests; the integration tests here pin only the plumbing.

## Notes

- F-036c (real-machine verification) blocks behind this — without the production path the user can't measure cycle time on hardware.
- Modulation needs a `ChiploadBand`. Source: vendor LUT lookup for the active tool/material. The algorithm rejects degenerate bands (`min > max`, non-finite). If the LUT lookup returns `None`, fall through (no modulation, no error).
