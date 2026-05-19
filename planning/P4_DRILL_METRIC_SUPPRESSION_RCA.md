# Priority 4 RCA — Suppress air-cut / engagement metrics for drill ops

**Date:** 2026-05-19
**Sources:** `crates/rs_cam_core/src/simulation_cut.rs`, `crates/rs_cam_core/src/narrate.rs`, `WANAKA_ASSESSMENT_2026-05-19.md`, `CLAUDE.md`

## Symptom

Drill (`Drill`) and Pin Drill (`AlignmentPinDrill`) op kinds always report:

- `air_cut_percentage`: ~100% (every sample reads `is_cutting && radial_engagement < 0.02`)
- `average_engagement`: ~0.0
- `issue_count`: thousands of "air_cut" entries (one per sample) — emission noise

The dexel's radial-engagement metric is **XY cylinder side-engagement**: the planar cross-section of the cutter at the cell row, intersected with the remaining stock. For drill cycles where every move is `MoveType::Linear` along Z (or zero-XY-displacement plunges), the cylinder's XY footprint sweeps no horizontal distance, so the radial-engagement reads 0 even when material is actively being removed by the flute tip.

Per `CLAUDE.md` April-2026 caveats:
> `average_engagement` is **unusable for 2D SVG operations** — always reports ~0 even when the stock is visibly cut. Known simulator issue with the polygon-to-dexel material initialization.
>
> `issue_count` with thousands of `air_cut` entries per run is **emission noise**, not a signal.

Wanaka TP0 (Pin Drill) and TP2 (Holes drill) hit this exactly. P1's verdict already silences drill ops in the warning logic. P4 closes the remaining loopholes:

1. The sim still emits `SimulationCutIssue` entries (one per cutting sample with low engagement) — this inflates `issue_count` to the thousands for a 30K-move project and drowns the hotspot list.
2. The per-toolpath summary (`SimulationToolpathCutSummary`) still carries `air_cut_time_s / total_runtime_s = ~1.0` for drill ops. Downstream consumers (GUI banner, MCP per-TP table, narrate) that compute per-TP air-cut % see a 100% number with no context.

## Root cause

`SimulationCutTrace::from_samples_with_semantics` builds issues per sample:

```rust
let kind = if !sample.is_cutting {
    None
} else if sample.radial_engagement < 0.02 {
    Some(SimulationCutIssueKind::AirCut)
} else if sample.radial_engagement < 0.10 {
    Some(SimulationCutIssueKind::LowEngagement)
} else {
    None
};
```

No op-kind awareness. For drill samples, the threshold fires on every sample, producing a `SimulationCutIssue` segment per coalesced run.

## Fix

Two pieces, scoped to the smallest change:

1. **New `drill_toolpath_ids: &BTreeSet<usize>` parameter** on the trace builder, populated by `compute/simulate.rs` from toolpath configs. The accumulator skips the issue-emission `kind` classification for samples whose toolpath is in this set. The per-sample air-cut / low-engagement TIME totals still accumulate (so downstream callers that compute `air_cut_time_s / total_runtime_s` still get the same number — we don't lose data), but no `SimulationCutIssue` is emitted.

2. **New `metrics_not_applicable: bool` field** on `SimulationToolpathCutSummary` (with `#[serde(default)]`). The trace builder sets this for drill-kind toolpaths. Downstream consumers (`narrate`, GUI sim diagnostics, MCP responses) that already check op-kind context can read this flag instead of repeating the op-type lookup.

The verdict logic (P1) already returns `None` from `OperationType::air_cut_high_threshold_pct` for drill kinds, so the project-level warning never fires on drill ops. P4 is the supporting structural cleanup — silence the issue spam, mark the per-TP summaries.

## Wire path

1. `compute/simulate.rs` builds the drill-toolpath-id set from the toolpath configs before calling `from_samples_with_semantics`.
2. The trace builder accepts the set as a new parameter (or via a small builder struct for forward-extensibility).
3. The accumulator's per-sample issue gate adds `if drill_toolpath_ids.contains(&sample.toolpath_id) { return None }` (or equivalent).
4. After the per-TP summaries are built, the builder sets `metrics_not_applicable = true` on drill TPs.

## Tests

Negative cases (signal):
- Adaptive3d sample with `radial_engagement = 0.01` → `SimulationCutIssue` emitted
- DropCutter sample with `radial_engagement = 0.05` → `SimulationCutIssue` emitted (low engagement)

Positive cases (artifact — should NOT emit issue):
- Drill sample with `radial_engagement = 0.0` → no issue (the toolpath is in the drill set)
- PinDrill sample with `radial_engagement = 0.0` → no issue
- A trace mixing 1 adaptive + 1 drill toolpath: only the adaptive's air-cut samples emit issues

Annotation:
- Per-TP summary for drill toolpath has `metrics_not_applicable = true`
- Per-TP summary for non-drill toolpath has `metrics_not_applicable = false`

## Out of scope

- The structural dexel fix (modeling Z-only kinematic engagement) — planning/DEXEL_Z_ONLY_INVESTIGATION.md placeholder
- Suppressing project-aggregate `air_cut_percentage` in `ProjectDiagnostics` — that's a project-wide metric and consumers that care can iterate per-TP and skip ops with `metrics_not_applicable = true`
- Re-running narrate's existing F4 path — that's already correct
