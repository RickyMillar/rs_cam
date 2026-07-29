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
| `chipload_mm_per_tooth` | Material per flute per revolution |
| `mrr_mm3_s` | Material removal rate |
| `is_cutting` | false = rapid/air move |
| `semantic_item_id` | Links sample to semantic structure |

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
| Air cut ratio | < 10% | 10–25% | > 25% |
| Avg engagement (rough) | 0.3–0.5 | 0.15–0.3 | < 0.15 |
| Avg engagement (finish) | 0.1–0.4 | 0.4–0.6 | > 0.6 |
| Chipload (softwood) | 0.05–0.12 mm | 0.02–0.05 | < 0.02 or > 0.15 |
| Chipload (hardwood) | 0.03–0.08 mm | 0.01–0.03 | < 0.01 or > 0.10 |

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
