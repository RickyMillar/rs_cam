---
name: sim-analysis
description: Interpret simulation diagnostic output — cut traces, issues, hotspots, collisions
---

# /sim-analysis — Simulation Diagnostic Interpretation

## Data Sources

| Trace | File | Contains |
|-------|------|----------|
| Cut trace | `rs_cam_core/src/simulation_cut.rs` | Per-sample cutting metrics, issues, hotspots, summaries |
| Performance trace | `rs_cam_core/src/debug_trace.rs` | Hierarchical timing spans, computation hotspots |
| Semantic trace | `rs_cam_core/src/semantic_trace.rs` | 26 structural kinds with move ranges, bounds, parameters |
| Collision | `rs_cam_core/src/collision.rs` | Holder/shank events, rapid collisions, min safe stickout |

State access: `rs_cam_viz/src/state/simulation.rs` — `SimulationState` methods.

## Cut Trace Metrics (`SimulationCutSample`)

| Field | Meaning |
|-------|---------|
| `axial_doc_mm` | Depth of cut — how deep the tool engages |
| `radial_engagement` | Fraction of tool diameter engaged (0.0–1.0) |
| `chipload_mm_per_tooth` | Commanded **advance per tooth** — `feed / rpm / flutes`. Kinematic, not a measured chip |
| `mrr_mm3_s` | Material removal rate |
| `is_cutting` | false = rapid/air move |
| `semantic_item_id` | Links sample to semantic structure |

## Read the triage block first

`SimulationTriage` (`ProjectSession::simulation_triage`, MCP
`get_diagnostics` → `resp["triage"]`, CLI `project`, GUI panel) is the
bounded typed answer and the one contract all four surfaces consume.
Order: `measurability` → `safety` → `actions` → `advisories`. Advisories
are capped (10/toolpath, 50/project) and spatially deduped; `truncated`
and `total_matching` tell you what was hidden. Safety events never share
a list with advisories.

`measurability` says when a metric could **not** be resolved at the
selected cell, in which case its gate abstains rather than publishing a
hard zero. The motivating case: a 0.02 mm-deep pass reads air 95.9% and
engagement 0.0000 while removing 63.7 mm³. Collision detection stays
live regardless.

Do not compare "issue counts" between surfaces — three different
quantities ship under that name.

## Issue Types

| Kind | Trigger | Cause | Fix |
|------|---------|-------|-----|
| `AirCut` | Engagement < 2% at feed rate | Retract too low, poor linking | Reduce retract height, enable keep-tool-down |
| `LowEngagement` | Engagement < 10% | Stepover too small, thin slivers | Increase stepover, use rest machining |

## Summary Methods on `SimulationState`

- `toolpath_cut_summary(id)` — aggregate stats per toolpath
- `semantic_cut_summary(id)` — per-semantic-item metrics
- `cut_worst_items(id, limit)` — worst items by wasted time
- `cut_hotspots(id, limit)` — hotspot regions sorted by duration
- `issues(job)` — all issues: hotspots, air cuts, low engagement, collisions

## Wood Routing Thresholds

| Metric | Good | Warning | Bad |
|--------|------|---------|-----|
| Air cut ratio (see note) | < 10% | 10–25% | > 25% |
| Avg engagement (rough) | 0.3–0.5 | 0.15–0.3 | < 0.15 |
| Avg engagement (finish) | 0.1–0.4 | 0.4–0.6 | > 0.6 |
| Advance per tooth (softwood) | 0.05–0.12 mm | 0.02–0.05 | < 0.02 or > 0.15 |
| Advance per tooth (hardwood) | 0.03–0.08 mm | 0.01–0.03 | < 0.01 or > 0.10 |

Two caveats on that table:

- **Air cut**: the shipped bars are per operation type against the
  total-runtime denominator, not one 10/25% band — see
  `OperationType::air_cut_high_threshold_pct` and the denominator note
  in `CLAUDE.md`. The row above is a rule of thumb for 2.5D clearing.
- **Chipload is advance per tooth**, `feed / (rpm x flutes)`, and so is
  the vendor LUT band the gate actually reads. Until 2026-08-06 the gate
  compared a measured arc-mean chip thickness against that band — a
  different axis, off by a per-row 2.4x-40.4x. Any older note quoting a
  "chipload" reading is not comparable to a current one.

## Semantic Trace

**Kind hierarchy** (typical nesting): Operation → DepthLevel → Region → Pass → (Entry | Contour | Raster | Row | ...)

**26 kinds:** Operation, DepthLevel, Region, Pass, Entry, SlotClearing, Cleanup, ForcedClear, Contour, Raster, Row, Slice, Hole, Cycle, Chain, Band, Ramp, Ring, Ray, Curve, Dressup, FinishPass, OffsetPass, Centerline, BoundaryClip, Optimization

Cross-reference `semantic_item_id` on cut samples to find which logical region is problematic.

## Performance Trace

- `ToolpathDebugSpan`: hierarchical timing with `elapsed_us`, `counters`, `exit_reason`
- `ToolpathHotspot`: computational bottleneck regions
- Cross-reference `debug_span_id` on semantic items to link structure → timing

## Collision Analysis

| Priority | Type | Meaning |
|----------|------|---------|
| Critical | `RapidCollision` | Tool/holder hits stock during G0 — crash risk |
| High | Holder collision (feed) | Holder contacts stock during cutting |
| Info | `min_safe_stickout_mm` | Minimum tool extension to avoid all collisions |

## Deviation Coloring (stock surface)

| Color | Meaning |
|-------|---------|
| Green | On target (within tolerance) |
| Blue | Material remaining (undercut) |
| Yellow | Slight overcut (0.1–0.3 mm past model) |
| Red | Significant overcut (> 0.3 mm — gouge) |

## Artifacts

- `SimulationCutArtifact`: JSON container with cut trace + stock metadata
- `ToolpathTraceArtifact`: JSON container with debug + semantic traces per toolpath
Both use `TOOLPATH_DEBUG_SCHEMA_VERSION` for forward compatibility.
