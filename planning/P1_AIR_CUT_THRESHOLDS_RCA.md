# Priority 1 RCA — Op-kind-aware air-cut thresholds

**Date:** 2026-05-19
**Sources:** `crates/rs_cam_core/src/session/compute.rs`, `crates/rs_cam_core/src/simulation_cut.rs`, `WANAKA_ASSESSMENT_2026-05-19.md`

## Symptom

`project_summary` MCP / GUI verdict reads **"WARNING: high air cutting"** on `wanaka_full.toml` (and similar projects) even when `rapid_collision_count = 0` and every TP is acceptable for its op-kind.

The Wanaka Phase 1 table:

| TP | Op | Air-cut % | Was warned? |
|---|---|---|---|
| 0 Pin Drill | AlignmentPinDrill | n/a (100%, dexel artifact) | rolled into project total |
| 3 Rivers EM (copy) | ProjectCurve (rough) | 92.1 % | yes — intrinsic to sparse rivers |
| 4 Rivers TB | ProjectCurve (engrave) | 83.9 % | yes — intrinsic |
| 5 Lakes TB | ProjectCurve (engrave) | 78.2 % | yes — intrinsic (32 s op) |
| 6 3D Rough 6 | Adaptive3d | 51.9 % | yes — borderline; band is 10-25 % |
| 7 3D Finish 6 | DropCutter | 11.5 % | no — in band |
| 1 Back Rough | Adaptive3d | 28.5 % | no — borderline-high |

Project-aggregate air-cut percentage is over 40 % → blanket verdict fires, but the verdict doesn't say **which** TP is the problem and doesn't account for **what's normal for each op-kind**.

## Root cause

`Session::diagnostics()` (`session/compute.rs:1166-1180`) composes the verdict with a single threshold:

```rust
let verdict = if total_collision_count > 0 { ... }
else if total_rapid_collision_count > 0 { ... }
else if air_cut_percentage > 40.0 {
    format!("WARNING: {air_cut_percentage:.1}% air cutting")
}
```

`air_cut_percentage` is **project-aggregate** — `sim.cut_trace.summary.air_cut_time_s / total_runtime_s`. So a project full of efficient TPs plus one short project_curve op with intrinsic 90 % air-cut can tip the aggregate over 40 % and produce a useless verdict.

Per-TP data is **already available** in `cut_trace.toolpath_summaries: Vec<SimulationToolpathCutSummary>` — each carries its own `air_cut_time_s` and `total_runtime_s`. The verdict just doesn't use it.

## Op-kind air-cut baselines

Calibrated from `WANAKA_ASSESSMENT_2026-05-19.md` expectation bands + FSWizard reference (Phase 3):

| OperationType | Expected band | High threshold | Rationale |
|---|---|---|---|
| `AlignmentPinDrill`, `Drill` | n/a | suppressed | dexel can't measure Z-only kinematics (P4) |
| `Adaptive`, `Pocket`, `Face`, `Zigzag`, `Rest` | 5–20 % | **40 %** | 2.5D clearing with boundary overshoot |
| `Adaptive3d` | 10–25 % | **40 %** | 3D rough, edges + Z-level transitions |
| `DropCutter`, `Scallop`, `Waterline`, `Pencil`, `HorizontalFinish`, `SteepShallow`, `RampFinish`, `SpiralFinish`, `RadialFinish` | 5–15 % | **30 %** | 3D finish — close contact expected |
| `Profile`, `Chamfer`, `Inlay`, `VCarve`, `Trace` | 5–20 % | **40 %** | 2D contour-style ops |
| `ProjectCurve` | 50–95 % | **97 %** | inherently sparse: rivers/curves are tiny features in big stock; rapids dominate by construction |

The `ProjectCurve` threshold is deliberately permissive: per Phase 1 the op intrinsically reads 78–92 % air-cut for sparse patterns. Only flag a TP that approaches near-total air (~97 %+), which indicates the path has nearly zero cutting and is probably misconfigured (e.g., wrong depth, wrong surface model). At-baseline ≠ defect.

## Fix

In `Session::diagnostics()`:

1. Compute per-TP air-cut % from `cut_trace.toolpath_summaries` rather than relying only on the project aggregate.
2. For each TP, look up its op-kind threshold via `OperationType::air_cut_high_threshold_pct()` (new method).
3. Build a list of TPs whose air-cut % exceeds their own threshold.
4. Compose verdict: if any TPs exceed thresholds, name them; otherwise drop the air-cut warning entirely.

Keep the project-aggregate `air_cut_percentage` field unchanged (callers / serializers depend on it).

## Tests

Negative cases (noise — should NOT warn):
- ProjectCurve TP at 92 % air-cut → silent
- Adaptive3d TP at 28 % air-cut → silent
- DropCutter TP at 15 % air-cut → silent

Positive cases (signal — SHOULD warn, naming the TP):
- Adaptive3d TP at 60 % air-cut → warn, name it
- DropCutter TP at 40 % air-cut → warn, name it
- ProjectCurve TP at 99 % air-cut → warn, name it

## Out of scope (deferred)

- 2D drill suppression (`AlignmentPinDrill`, `Drill`) → Priority 4
- Plunge-stress warning → Priority 2
- Peak-DOC lift-bridge artifact → Priority 3
