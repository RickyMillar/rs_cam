# AI Machinist Analysis Reference

Comprehensive reference for AI-assisted toolpath quality analysis in rs_cam. This document consolidates simulation capabilities, diagnostic data sources, interpretation thresholds, and analysis workflows into one place.

---

## Quick Start

### GUI Analysis
1. Open project in `rs_cam_gui` (`cargo run -p rs_cam_viz --bin rs_cam_gui`)
2. Generate toolpaths (select operation, press G or use auto-regen)
3. Enter Simulation workspace (workspace bar)
4. Run simulation — produces cut trace, collision report, performance data
5. Review diagnostics panel: issues, hotspots, collisions, semantic trace

### CLI Analysis
```bash
# Run a job with simulation and collision checks
cargo run -p rs_cam_cli -- <subcommand> [options]

# Parameter sweep with simulation
cargo run -p rs_cam_cli -- sweep job.toml --param stepover --values "1.0,2.0,3.0" --output out/ --simulate

# Automated parameter sweeps (test harness)
cargo test --test param_sweep                          # All 54 sweeps
cargo test --test param_sweep sweep_pocket             # One operation family
cargo test --test param_sweep sweep_pocket_stepover    # One specific parameter
```

### Agent Analysis
- `/sim-analysis` skill — interpret simulation diagnostic output
- `sim-diagnostics` agent — specialist for analyzing sim data
- `cam-navigator` agent — find code, trace pipelines, locate modules

---

## Analysis Capabilities Overview

| Capability | Source | Produces | Access |
|-----------|--------|----------|--------|
| Stock simulation | `dexel_stock.rs` | Volume removal, per-sample metrics | GUI + CLI |
| Cut trace | `simulation_cut.rs` | Chipload, engagement, MRR, air cut detection | GUI + JSON |
| Collision detection | `collision.rs` | Events, min safe stickout, rapid collisions | GUI + CLI |
| Performance trace | `debug_trace.rs` | Timing spans, computation hotspots | GUI + JSON |
| Semantic trace | `semantic_trace.rs` | 26-kind structural hierarchy with move ranges | GUI + JSON |
| Deviation analysis | `app/simulation.rs` | Per-vertex surface deviation colors | GUI |
| Fingerprinting | `fingerprint.rs` | Move counts, distances, feeds, bbox, stock metrics | CLI + JSON |
| Feed optimization | `feedopt.rs` | Engagement-based adaptive feed rates | GUI + CLI |
| Parameter sweeps | `fingerprint.rs` + `sweep.rs` | Diffs, SVGs, stock PNGs, G-code variants | CLI + test harness |

---

## 1. Stock Simulation (Tri-Dexel)

**Engine:** `crates/rs_cam_core/src/dexel_stock.rs` (1776 lines)

The simulation uses a tri-dexel volumetric representation — three orthogonal grids (X, Y, Z) where each cell stores a list of material segments. This supports cuts from any cardinal direction and multi-setup carry-forward.

### Key APIs
- `TriDexelStock::from_bounds(bbox, resolution)` — create stock
- `simulate_toolpath(toolpath, cutter, direction)` — full simulation
- `simulate_toolpath_with_metrics_cancel(...)` — simulation with per-sample cutting metrics
- `simulate_toolpath_range(start, end, ...)` — incremental range (for playback scrubbing)

### Stock Cut Directions
`FromTop`, `FromBottom`, `FromLeft`, `FromRight`, `FromFront`, `FromBack`

For 3-axis routers, only the Z-grid (FromTop) is typically needed. Multi-setup adds other grids.

### Resolution
Default ~0.5mm cell size. Finer = more accurate but slower. The SmallVec fast path keeps single-setup within ~20% of raw heightmap performance.

---

## 2. Cut Trace Analysis

**Source:** `crates/rs_cam_core/src/simulation_cut.rs` (1220 lines)

The cut trace captures per-sample metrics at ~mm intervals along every toolpath move.

### Per-Sample Metrics (`SimulationCutSample`)

| Field | Meaning | Units |
|-------|---------|-------|
| `axial_doc_mm` | Depth of cut — how deep the tool engages | mm |
| `radial_engagement` | Fraction of tool circumference in material | 0.0–1.0 |
| `chipload_mm_per_tooth` | Material removed per flute per revolution | mm |
| `mrr_mm3_s` | Material removal rate | mm³/s |
| `removed_volume_est_mm3` | Cumulative volume removed | mm³ |
| `is_cutting` | false = rapid/air move | bool |
| `semantic_item_id` | Links sample to semantic structure | ID |
| `position` | Tool center position | (x, y, z) |
| `feed_rate` | Commanded feed rate | mm/min |
| `spindle_rpm` | Spindle speed | RPM |
| `cumulative_time_s` | Time from start | seconds |

### Issue Detection

| Issue Kind | Trigger | Typical Cause | Recommended Fix |
|-----------|---------|---------------|-----------------|
| `AirCut` | Engagement < 2% at feed rate | Retract too low, poor linking, geometry gaps | Reduce retract height, enable keep-tool-down linking |
| `LowEngagement` | Engagement 2–10% | Stepover too small, thin slivers | Increase stepover, use rest machining |

### Aggregate Summaries

- `SimulationToolpathCutSummary` — per-toolpath: total time, cutting/rapid split, air cut time, avg engagement, avg chipload
- `SimulationSemanticCutSummary` — per-semantic-region: same metrics scoped to logical structure
- `SimulationCutHotspot` — spatial engagement/computation bottlenecks

### State Queries (GUI)
```
SimulationState methods:
  toolpath_cut_summary(id)      — aggregate stats per toolpath
  semantic_cut_summary(id)      — per-semantic-item metrics
  cut_worst_items(id, limit)    — worst items by wasted time
  cut_hotspots(id, limit)       — hotspot regions sorted by duration
  current_cut_sample()          — current sample at scrubber position
  issues(job)                   — all issues aggregated
```

---

## 3. Collision Detection

**Source:** `crates/rs_cam_core/src/collision.rs`

### Collision Types (Priority Order)

| Priority | Type | Risk | Description |
|----------|------|------|-------------|
| Critical | `RapidCollision` | Machine crash | Tool/holder hits stock during G0 rapid moves |
| High | Holder/shank collision (feed) | Tool damage, marks | Holder contacts stock during cutting moves |
| Info | `min_safe_stickout_mm` | Advisory | Minimum tool extension to avoid all collisions |

### Tool Assembly Model
```
ToolAssembly {
    cutter_radius,      // Cutting tool radius
    cutter_length,      // Exposed cutting length
    shank_diameter,     // Shank diameter (above cutter)
    shank_length,       // Shank length
    holder_diameter,    // Holder/collet diameter
    holder_length,      // Holder length (default 40mm in CLI)
}
```

### APIs
- `check_collisions_interpolated(toolpath, assembly, mesh, index, step)` — sampled collision check (0.1–2mm steps)
- `check_rapid_collisions(toolpath, assembly, bbox)` — G0 moves vs stock bounding box

### Output: `CollisionReport`
- `collisions[]` — list of `CollisionEvent` (move_index, position, penetration_depth, segment_name)
- `rapid_collisions[]` — list of `RapidCollision` (move_index, start, end)
- `min_safe_stickout` — calculated minimum tool extension
- `is_clear()` — true if no collisions detected

---

## 4. Performance Tracing

**Source:** `crates/rs_cam_core/src/debug_trace.rs` (800+ lines)

Hierarchical timing traces of toolpath generation algorithm phases.

### Structures
- `ToolpathDebugSpan` — single phase: id, kind, label, elapsed_us, xy_bbox, z_level, move_start/end, counters, exit_reason
- `ToolpathHotspot` — spatial bottleneck: center, bucket_size, elapsed_us, span_count, step_count
- `ToolpathDebugAnnotation` — per-move label ("start depth level", "boundary clip")
- `ToolpathDebugTrace` — complete: spans[], hotspots[], annotations[], summary

### Reading Hotspots
Hotspots group overlapping spans by spatial bucket, accumulating timing. Sort by `elapsed_us` to find the slowest regions. Cross-reference `debug_span_id` on semantic items to map hotspots to logical structure.

### Exit Reasons
Spans record why an algorithm phase ended — boundary hit, iteration limit, convergence, etc. High counts of `low_yield_exit` indicate algorithm struggling with geometry.

---

## 5. Semantic Tracing

**Source:** `crates/rs_cam_core/src/semantic_trace.rs` (802 lines)

Captures the logical structure of toolpath generation — what the algorithm was doing and why.

### 26 Semantic Kinds
```
Operation → DepthLevel → Region → Pass → {
    Entry, SlotClearing, Cleanup, ForcedClear,
    Contour, Raster, Row, Slice, Hole, Cycle,
    Chain, Band, Ramp, Ring, Ray, Curve,
    Dressup, FinishPass, OffsetPass, Centerline,
    BoundaryClip, Optimization
}
```

### Typical Hierarchy (Pocket)
```
Operation
  ├─ DepthLevel (Z=0 to -5mm)
  │   ├─ Region (island 1)
  │   │   ├─ Pass (rough pass 1)
  │   │   │   ├─ Entry (ramp/helix/plunge)
  │   │   │   ├─ Contour
  │   │   │   │   ├─ Row 1
  │   │   │   │   ├─ Row 2
  │   │   └─ Cleanup
  │   └─ Region (island 2)
  └─ DepthLevel (Z=-5 to -10mm)
```

### Per-Item Data
Each `ToolpathSemanticItem` has: id, parent_id, kind, label, move_start, move_end, xy_bbox, z_min, z_max, params (stepover, depth, entry height, etc.), debug_span_id.

### Cross-Referencing
- `semantic_item_id` on `SimulationCutSample` links cutting metrics to logical structure
- `debug_span_id` on semantic items links structure to timing data
- This enables: "Region 2 at depth level 3 has 40% air cutting because the entry is too high"

---

## 6. Deviation Analysis

**Source:** `crates/rs_cam_viz/src/app/simulation.rs`

Computes surface deviation between target model and simulated stock result.

### Deviation Color Scheme

| Color | Meaning | Threshold |
|-------|---------|-----------|
| Green | On target | Within tolerance |
| Blue | Material remaining (undercut) | Stock above model surface |
| Yellow | Slight overcut | 0.1–0.3 mm past model |
| Red | Significant overcut (gouge) | > 0.3 mm past model |

### Access
GUI: Select "Deviation" in stock visualization mode dropdown. Computed per-checkpoint during playback.

---

## 7. Feed Rate Optimization

**Source:** `crates/rs_cam_core/src/feedopt.rs`

Post-dressup that adjusts feed rates based on real-time material engagement.

### How It Works
1. Samples 24 points on tool circumference at each move position
2. Checks each point against tri-dexel stock to measure engagement fraction
3. Applies Radial Chip Thinning Factor (RCTF) to maintain consistent chip load
4. Ramps feed rate changes to prevent abrupt acceleration/deceleration

### Parameters
- `nominal_feed_rate` — base feed at full engagement
- `max_feed_rate` — ceiling for light engagement
- `min_feed_rate` — floor
- `ramp_rate` — max change per mm of travel
- `air_cut_threshold` — below this engagement, use max feed (air cutting)

### Benefits
15–30% faster cycle times, eliminates burn marks from dwelling in light cuts, consistent chip load across varying engagement.

---

## 8. Fingerprinting & Parameter Sweeps

**Source:** `crates/rs_cam_core/src/fingerprint.rs` (1170 lines) + `crates/rs_cam_cli/src/sweep.rs`

### Toolpath Fingerprint
Single-pass extraction of toolpath metrics: move counts (by type), distances (cutting/rapid), Z levels, feed rates, bounding box, rapid/cutting fractions.

### Stock Fingerprint
Post-simulation stock state: cells with material, empty cells, surface Z stats, cut fraction, deviation stats.

### Fingerprint Diffing
`diff_fingerprints(base, variant)` — reports changed fields with before/after values and `delta_percent`. Sensitivity: absolute delta > 0.001 AND relative delta > 0.1%.

### CLI Parameter Sweep
```bash
rs_cam_cli sweep job.toml --param stepover --values "1.0,2.0,3.0" --output out/ --simulate
```

**Output per variant:**
- `variant_<val>.json` — fingerprint
- `variant_<val>_diff.json` — diff from baseline
- `variant_<val>.svg` — toolpath visualization (800x600, Z encoded as stroke color)
- `variant_<val>.nc` — G-code
- `variant_<val>_stock.json` — stock fingerprint (if --simulate)

### Test Harness Sweeps
```bash
cargo test --test param_sweep                    # All 54 sweeps across 22 operations
cargo test --test param_sweep sweep_pocket       # One operation family
```

Output goes to `target/param_sweeps/{op}/{param}/` with JSON fingerprints, diffs, toolpath SVGs, and 6-view composite stock PNGs.

### Sweep Analysis
```bash
python3 toolpath_stress_test/agents/analyze_sweep.py target/param_sweeps/
```

### Diff Interpretation
```json
{
  "changed_fields": {
    "total_cutting_distance_mm": {
      "before": 1234.5, "after": 2468.9, "delta_percent": 100.0
    }
  },
  "unchanged_fields": ["total_rapid_distance_mm", "z_levels", "bounding_box"]
}
```

**Expected effects by parameter:**
| Parameter | Should Change | Should NOT Change |
|-----------|--------------|-------------------|
| stepover | move_count, cutting_distance | z_levels, feed_rates |
| feed_rate | feed_rates only | move_count, distances |
| depth_per_pass | z_levels, move_count | per-level pattern |
| safe_z / retract | rapid_distance | cutting_distance |
| tool diameter | everything | — |

---

## 9. Wood Routing Thresholds

Reference benchmarks for 3-axis wood router analysis.

### Efficiency

| Metric | Good | Warning | Bad |
|--------|------|---------|-----|
| Air cut ratio | < 10% | 10–25% | > 25% |
| Avg engagement (roughing) | 0.3–0.5 | 0.15–0.3 | < 0.15 |
| Avg engagement (finishing) | 0.1–0.4 | 0.4–0.6 | > 0.6 |

### Chip Load (mm/tooth)

| Material | Good | Warning | Bad |
|----------|------|---------|-----|
| Softwood (pine, cedar) | 0.05–0.12 | 0.02–0.05 | < 0.02 or > 0.15 |
| Hardwood (oak, maple) | 0.03–0.08 | 0.01–0.03 | < 0.01 or > 0.10 |
| MDF/plywood | 0.04–0.10 | 0.02–0.04 | < 0.02 or > 0.12 |

### Cutting Interpretation

| Condition | Symptom | Risk |
|-----------|---------|------|
| `chipload < 0.02` | Rubbing, not cutting | Heat buildup, burn marks, premature wear |
| `chipload > 0.15` | Aggressive cutting | Tool breakage, tearout, chatter |
| `radial_engagement ~ 1.0` | Full-width slotting | High forces — consider adaptive clearing |
| `axial_doc > cutting_length` | Over-depth | Tool damage, shank contact |
| `engagement < 0.02 at feed` | Air cutting | Wasted time, unnecessary wear |

---

## 10. Analysis Checklist

Use this checklist when analyzing a toolpath program:

### Safety (Critical)
- [ ] **Rapid collisions**: Any G0 moves through stock? (`RapidCollision` in collision report)
- [ ] **Holder/shank collisions**: Holder contacting stock during cuts? (`CollisionEvent`)
- [ ] **Min safe stickout**: Is current stickout sufficient? (`min_safe_stickout_mm`)
- [ ] **Plunge rate**: Is plunge feed appropriate? (not faster than cutting feed without reason)
- [ ] **Depth of cut**: Does `axial_doc_mm` exceed tool cutting length anywhere?

### Efficiency
- [ ] **Air cutting ratio**: What % of cutting time is through air? (target < 10%)
- [ ] **Low engagement**: What % of time has engagement < 10%? (target < 25%)
- [ ] **Rapid optimization**: Are rapid moves minimized? (TSP ordering enabled?)
- [ ] **Link moves**: Are keep-tool-down linking moves used where appropriate?
- [ ] **Retract height**: Is retract height as low as safely possible?

### Quality
- [ ] **Surface deviation**: Any gouges (red) in deviation view?
- [ ] **Scallop height**: Is stepover appropriate for desired surface finish?
- [ ] **Engagement consistency**: Are there engagement spikes causing marks?
- [ ] **Entry strategy**: Ramp/helix vs plunge — appropriate for operation?
- [ ] **Feed rate consistency**: Stable chipload through varying engagement?

### Parameter Validation
- [ ] **Tool-operation compatibility**: Ball nose on pocket? End mill on scallop?
- [ ] **Stepover vs diameter**: Stepover > 50% on finish pass?
- [ ] **Heights cross-check**: bottom < top? feed_z above stock top? retract > clearance?
- [ ] **Feed math**: RPM x flutes x chipload = reasonable feed rate?

### Operation Sequencing
- [ ] **Roughing before finishing**: Stock reduction adequate before finish pass?
- [ ] **Rest machining**: Are corners/pockets fully cleared for smaller finishing tools?
- [ ] **Depth stepping**: Appropriate depth per pass for material and tool?

---

## 11. Issue Aggregation

The simulation state aggregates all issues into a unified list:

```
SimulationIssueKind:
  Hotspot          — Computation or engagement bottleneck
  Annotation       — Algorithm milestone marker
  AirCut           — Engagement < 2% at feed rate
  LowEngagement    — Engagement 2–10%
  RapidCollision   — G0 move through stock (crash risk)
  HolderCollision  — Shank/holder contacts stock during feed
```

Access via `SimulationState::issues(job)` in GUI, or parse JSON artifacts from CLI.

---

## 12. Artifacts & Serialization

All diagnostic data is JSON-serializable for offline analysis:

- `SimulationCutArtifact` — cut trace + stock metadata + request snapshot
- `ToolpathTraceArtifact` — debug trace + semantic trace per toolpath
- `ToolpathFingerprint` — compact toolpath metrics
- `StockFingerprint` — post-simulation stock metrics
- `CollisionReport` — collision events and rapid collisions

Schema version: `TOOLPATH_DEBUG_SCHEMA_VERSION` for forward compatibility.

---

## 13. Engagement Heatmap (GUI)

**Source:** `crates/rs_cam_viz/src/render/toolpath_render.rs`

Toolpath lines colored by feed rate relative to nominal:
- **Green**: Light engagement (ratio >= 1.0, feed at or above nominal)
- **Yellow**: Medium engagement (ratio 0.5–1.0)
- **Red**: Heavy engagement (ratio < 0.5, feed significantly below nominal)

Toggle via viewport overlay "Engagement" checkbox.

---

## 14. Collision Density Heatmap (GUI)

**Source:** `crates/rs_cam_viz/src/app/gpu_upload.rs`

Collision markers colored by spatial density (5mm clustering radius):
- **Yellow**: Isolated collision (single event in radius)
- **Orange**: Moderate cluster
- **Red**: Dense collision cluster (many events nearby)

---

## 15. Key File Paths

### Core Simulation & Analysis
| File | Purpose |
|------|---------|
| `crates/rs_cam_core/src/dexel_stock.rs` | Tri-dexel stock simulation engine |
| `crates/rs_cam_core/src/simulation_cut.rs` | Cut trace metrics, issues, hotspots |
| `crates/rs_cam_core/src/collision.rs` | Collision detection (holder, rapid) |
| `crates/rs_cam_core/src/debug_trace.rs` | Performance tracing, computation hotspots |
| `crates/rs_cam_core/src/semantic_trace.rs` | 26-kind structural hierarchy |
| `crates/rs_cam_core/src/fingerprint.rs` | Toolpath/stock fingerprinting and diffing |
| `crates/rs_cam_core/src/feedopt.rs` | Engagement-based feed optimization |

### GUI Integration
| File | Purpose |
|------|---------|
| `crates/rs_cam_viz/src/app/simulation.rs` | Simulation orchestration, deviation |
| `crates/rs_cam_viz/src/state/simulation.rs` | SimulationState queries, issue aggregation |
| `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` | Diagnostic panel UI |
| `crates/rs_cam_viz/src/ui/sim_timeline.rs` | Timeline controls |
| `crates/rs_cam_viz/src/render/toolpath_render.rs` | Engagement heatmap coloring |

### CLI & Testing
| File | Purpose |
|------|---------|
| `crates/rs_cam_cli/src/main.rs` | CLI with collision checks |
| `crates/rs_cam_cli/src/sweep.rs` | Parameter sweep command |
| `crates/rs_cam_core/tests/param_sweep.rs` | Automated sweep test harness |

### Agent & Skill Definitions
| File | Purpose |
|------|---------|
| `.claude/skills/sim-analysis/SKILL.md` | Simulation diagnostic interpretation |
| `.claude/agents/sim-diagnostics.md` | Specialist diagnostic analysis agent |
| `.claude/agents/cam-navigator.md` | Codebase navigation agent |

---

## 16. Operations Reference (22 Operations)

### 2.5D Operations (11)
Face, Pocket, Profile, Adaptive, VCarve, Rest, Inlay, Zigzag, Trace, Drill, Chamfer

### 3D Operations (11)
3D Raster Finish (DropCutter), 3D Adaptive Rough, Waterline, Pencil, Scallop, Steep/Shallow, Ramp Finish, Spiral Finish, Radial Finish, Horizontal Finish, Project Curve

### Tool Families (5)
Flat end mill, Ball end mill, Bull nose, V-bit, Tapered ball nose

### Dressup System
Post-generation modifications: entry style (plunge/ramp/helix), dogbones, lead-in/out, link moves, arc fitting, feed optimization, retract strategy, rapid order optimization, tabs, machining boundary.
