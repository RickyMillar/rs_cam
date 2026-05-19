# Priority 3 RCA — Suppress peak-axial-DOC on transit/link spans

**Date:** 2026-05-19
**Sources:** `crates/rs_cam_core/src/simulation_cut.rs`, `crates/rs_cam_core/src/dexel_stock/simulation.rs`, `crates/rs_cam_core/src/toolpath_spans.rs`, `WANAKA_ASSESSMENT_2026-05-19.md`

## Symptom

Wanaka TP3 (Project Curve, 6 mm EM) reports `peak_axial_doc_mm = 18.59 mm` on a 6 mm tool at z=6.41, position (110.6, 75.8). Commanded `depth_per_pass` is per-segment for project_curve (no fixed DOC), but the cutter is 6 mm so 18.59 mm is physically impossible — the dexel is reporting `stock_top − cutter_z` over uncleared neighbouring stock during a transit/link move between disjoint river segments.

Wanaka TP6 (3D Rough adaptive3d, 6 mm EM) reports peak DOC 5.5 mm vs commanded 2.0 mm — same pattern, smaller magnitude, same source (lift-bridge over uncleared stock during a link bridge between regions).

This makes the verdict say "WARNING: cutter is engaging 9× deeper than commanded" when in reality the cutter is bridging cleared air at the configured rapid feed.

## Root cause

In `simulation_cut.rs:480`:
```rust
self.peak_axial_doc_mm = self.peak_axial_doc_mm.max(sample.axial_doc_mm.max(0.0));
```

The accumulator takes the max of `axial_doc_mm` across **every** sample regardless of whether the sample is in a cutting context. `axial_doc_mm` is computed by the dexel as `material_height_above_cutter` for lateral feeds — accurate for steady-state cutting, but for samples inside an `Entry`, `LinkBridge`, `LeadOut`, `WaterlineCleanup`, or `DressupArtifact` span the cutter is in transit and the reading reflects "stock height we're flying over", not engagement.

`is_cutting` is `false` only for `MoveType::Rapid`. Link bridges typically use `MoveType::Linear` at a programmed feed rate (so `is_cutting = true`), and `WaterlineCleanup` spans contain feed moves over partially-cleared regions — both register as cutting samples in the metrics accumulator even though they're not steady-state engagement.

The span context is **already present** in each `SimulationCutSample` as `span_path: Vec<SpanId>` (populated by the simulator from `AnnotatedToolpath::span_paths_by_move()`). What's missing is:

1. The accumulator doesn't know which `SpanId`s map to transit kinds (it has indices, not kinds).
2. The trace builder receives samples but not the `Vec<Span>` to resolve indices.

## Fix

Smallest viable change: pre-compute a per-sample `in_transit_span: bool` at simulator emission time, where the spans are still in scope. Store it on `SimulationCutSample` (with `#[serde(default)]` so existing JSON traces still deserialize cleanly). The `SummaryAccumulator` gates `peak_axial_doc_mm` and `peak_chipload_mm_per_tooth` updates on `!sample.in_transit_span`.

**Transit span kinds** (set at sample-emission time):

- `Entry` — lead-in transitions (helix, ramp, plunge)
- `LeadOut` — lead-out transitions
- `LinkBridge` — linker bridge between regions / segments
- `WaterlineCleanup` — transient cleanup over partial-cleared regions
- `DressupArtifact` — dressup-introduced replacement segments

`Operation`, `DepthPass`, `Region`, `RapidOrderBarrier` are **not** transit. The cutter is in steady-state cutting inside these.

## Wire path

1. `simulate_toolpath_with_lut_metrics_cancel` (dexel_stock/simulation.rs) accepts a new `transit_moves: &[bool]` slice indexed by `move_index`. Each sample's `in_transit_span` is set from the slice during emission.
2. The caller in `compute/simulate.rs` builds `transit_moves` from `entry.annotated.spans` filtering for the transit kinds above.
3. `SummaryAccumulator::observe` skips `peak_axial_doc_mm` and `peak_chipload_mm_per_tooth` updates when `sample.in_transit_span`. (Other metrics like cutting_runtime, engagement histograms, air_cut_time stay unchanged — those are runtime-distribution metrics, not extreme-value metrics, and treating transit samples like cutting samples for time-weighted averages is fine.)

## Tests

Negative cases (signal — peak DOC SHOULD be reported):
- Sample at `axial_doc=3.0` with empty span_path → peak DOC 3.0
- Sample at `axial_doc=3.0` in DepthPass span → peak DOC 3.0
- Sample at `axial_doc=3.0` in Region span → peak DOC 3.0

Positive cases (artifact — peak DOC should NOT be reported):
- Sample at `axial_doc=18.0` in LinkBridge span → peak DOC stays at prior value
- Sample at `axial_doc=18.0` in Entry span → peak DOC stays at prior value
- Sample at `axial_doc=18.0` in WaterlineCleanup span → peak DOC stays at prior value
- Mixed stream (transit 18 mm + cutting 3 mm) → peak DOC reads 3.0 mm

## Out of scope

The same lift-bridge artifact also inflates `peak_chipload_mm_per_tooth` (the cutter "engaging" 18 mm in transit produces a fake chipload number). The fix piggybacks: gate `peak_chipload_mm_per_tooth` too. Other metrics (engagement bins, air-cut time) accumulate time-weighted averages that naturally dilute single-sample anomalies, so they're not affected by this fix.

`average_engagement` for adaptive3d remains cylinder-volume — that's a different calibration issue documented in CLAUDE.md and a deeper structural fix.
