# AI Machinist Analysis Reference

Comprehensive reference for AI-assisted toolpath quality analysis in rs_cam. This document consolidates simulation capabilities, diagnostic data sources, interpretation thresholds, and analysis workflows into one place.

> Reconciled against the tree on 2026-08-04 by the L1 documentation sweep. For superseded finishing-strategy verdicts (speed/quality comparisons between operations, measured through instrument defects since fixed), see `planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md`.

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
cargo test --test param_sweep                          # All 56 sweeps
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
| Stock simulation | `dexel_stock/` (directory module: `mod.rs`, `simulation.rs`, `stamping.rs`, `cut_direction.rs`) | Volume removal, per-sample metrics | GUI + CLI |
| Cut trace | `simulation_cut.rs` | Chipload, engagement, MRR, air cut detection | GUI + JSON |
| Collision detection | `collision.rs` | Events, min safe stickout, rapid collisions | GUI + CLI |
| Performance trace | `debug_trace.rs` | Timing spans, computation hotspots | GUI + JSON |
| Semantic trace | `semantic_trace.rs` | 26-kind structural hierarchy with move ranges | GUI + JSON |
| Deviation analysis | `render/sim_render.rs` (color mapping) + `app/gpu_upload.rs` (render wiring) + `compute/simulate.rs` (per-column instrument) | Per-vertex surface deviation colors (display) + per-column `ColumnDeviation` (quality metrics) | GUI |
| Fingerprinting | `fingerprint.rs` | Move counts, distances, feeds, bbox, stock metrics | CLI + JSON |
| Feed optimization | `feedopt.rs` | Engagement-based adaptive feed rates | GUI + CLI |
| Parameter sweeps | `fingerprint.rs` + `sweep.rs` | Diffs, SVGs, stock PNGs, G-code variants | CLI + test harness |

---

## 1. Stock Simulation (Tri-Dexel)

**Engine:** `crates/rs_cam_core/src/dexel_stock/` (directory module: `mod.rs`, `simulation.rs`, `stamping.rs`, `cut_direction.rs`; 2882 lines total)

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
Default ~0.5mm cell size. Finer = more accurate but slower. Dexel rays are `SmallVec<[DexelSegment; 1]>` (`crates/rs_cam_core/src/dexel.rs`), which keeps the common single-segment case off the heap; no committed benchmark backs a specific throughput percentage against a raw heightmap, so none is quoted here.

---

## 2. Cut Trace Analysis

**Source:** `crates/rs_cam_core/src/simulation_cut.rs` (2706 lines)

The cut trace captures per-sample metrics at ~mm intervals along every toolpath move.

### Per-Sample Metrics (`SimulationCutSample`)

| Field | Meaning | Units |
|-------|---------|-------|
| `axial_doc_mm` | Legacy wire name for axial cutting engagement; pure-vertical plunges report `0.0` here | mm |
| `axial_engagement_mm` | Maximum material height engaged by lateral/arc/helix cutting at this sample — the axis deflection/chip-geometry gates consume | mm |
| `plunge_descent_mm` | Z descent from a pure-vertical plunge sample; lateral/arc samples leave this at zero | mm |
| `engagement.radial_woc_fraction` | Cylinder-side width-of-cut as a fraction of cutter diameter. **Replaces the removed `radial_engagement` scalar** (dropped from `SimulationCutSample` 2026-05-20; a same-named field still exists on the unrelated `SimulationCutIssue` struct) | 0.0–1.0 |
| `engagement` | Full structured `Engagement` vector (radial WOC fraction, axial DOC fraction, arc radians, mean chip thickness) — see `Engagement` in `simulation_cut.rs:66-68` | struct |
| `cut_kinematics` | `CutKinematics` classification of this move (lateral/arc/helix/plunge/…) | enum |
| `in_transit_span` | True when the move sits in a transit-style span (Entry, LeadOut, LinkBridge, WaterlineCleanup, DressupArtifact); the dexel reading there reports `stock_top − cutter_z` over neighbouring stock, not steady-state engagement — extreme-value metrics (`peak_axial_doc_mm`, `peak_chipload_mm_per_tooth`) skip these samples | bool |
| `chipload_mm_per_tooth` | Material removed per flute per revolution | mm |
| `mrr_mm3_s` | Material removal rate | mm³/s |
| `removed_volume_est_mm3` | Cumulative volume removed | mm³ |
| `is_cutting` | false = rapid/air move | bool |
| `semantic_item_id` | Links sample to semantic structure | ID |
| `position` | Tool center position | (x, y, z) |
| `feed_rate_mm_min` | Commanded feed rate | mm/min |
| `spindle_rpm` | Spindle speed | RPM |
| `cumulative_time_s` | Time from start | seconds |

### Issue Detection

| Issue Kind | Trigger | Typical Cause | Recommended Fix |
|-----------|---------|---------------|-----------------|
| `AirCut` | Engagement < 2% at feed rate | Retract too low, poor linking, geometry gaps | Reduce retract height, enable keep-tool-down linking |
| `LowEngagement` | Engagement 2–10% | Stepover too small, thin slivers | Increase stepover, use rest machining |

### Aggregate Summaries

- `SimulationToolpathCutSummary` — per-toolpath: total time, cutting/rapid split, air cut time, avg engagement, avg chipload, `peak_axial_doc_mm` / `peak_plunge_descent_mm` (lateral-engagement peak vs vertical-plunge peak, kept separate so deflection gates don't consume peck descent as cutter engagement), `metrics_not_applicable` (true for drill-kinematics toolpaths — the radial-WOC axis doesn't apply to Z-only moves; consult `drill_summaries` instead, §2.1 below), and `per_kinematics: BTreeMap<CutKinematics, KinematicsSummary>` for axis-aware reporting (axial-DOC, arc, chip thickness, leading-edge speed) broken out by kinematics class instead of one blended scalar (`simulation_cut.rs:474-534`)
- `SimulationSemanticCutSummary` — per-semantic-region: same metrics scoped to logical structure
- `SimulationCutHotspot` — spatial engagement/computation bottlenecks

**`average_engagement` is scalar and radial-WOC-only.** It is a time-weighted mean of `engagement.radial_woc_fraction` — it says nothing about axial DOC, arc, or chip thickness. For those, read `per_kinematics`. For drill toolpaths, `average_engagement` and `air_cut_time_s` are suppressed via `metrics_not_applicable`; read §2.1 instead.

### 2.1 Drill toolpaths — a separate metrics family

`metrics_not_applicable` means "no ENGAGEMENT metrics apply", not "no metrics at all." Drill ops (dexel polygon-to-material init can't see Z-only moves) produce their own metrics family instead:

- `SimulationCutTrace::drill_summaries: Vec<DrillToolpathSummary>` — per-peck `DrillSample`, per-toolpath `DrillToolpathSummary` (peck adequacy, chip-welding risk, cycle time). Look up by `toolpath_id`, or use `SimulationCutTrace::drill_summary_for(toolpath_id)`.
- `ToolpathLoadVerdict::drill_gates` (`crates/rs_cam_core/src/tool_load/drill_gates.rs:97-135`) — three gates: chip welding, peck adequacy, plunge feed sanity.

FEATURE_CATALOG.md's Drill row already documents this correctly; nothing here should contradict it.

### State Queries (GUI)
```
SimulationState methods:
  toolpath_cut_summary(id)      — aggregate stats per toolpath
  semantic_cut_summary(id)      — per-semantic-item metrics
  cut_worst_items(id, limit)    — worst items by wasted time
  cut_hotspots(id, limit)       — hotspot regions sorted by duration
  current_cut_sample()          — current sample at scrubber position
  issues(&mut self, gui: &GuiState, max_feed_mm_min: f64) — all issues aggregated
    (crates/rs_cam_viz/src/state/simulation.rs:1485; NOT `issues(job)`)
```

---

## 3. Collision Detection

**Source:** `crates/rs_cam_core/src/collision.rs`

### Collision Types (Priority Order)

| Priority | Type | Risk | Description |
|----------|------|------|-------------|
| Critical | `RapidCollision` | Machine crash | Tool/holder hits stock during G0 rapid moves |
| High | Holder/shank collision (feed) | Tool damage, marks | Holder contacts stock during cutting moves |
| Info | `min_safe_stickout` (Rust field name; `min_safe_stickout_mm` is only the JSON wire key) | Advisory | Minimum tool extension to avoid all collisions |

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
- `check_rapid_collisions_against_stock(toolpath: &Toolpath, z_grid: &DexelGrid)` (`collision.rs:450`) — G0 moves checked against the DEXEL GRID, not a bounding box; there is no `check_rapid_collisions(toolpath, assembly, bbox)` function

### Output: `CollisionReport`
`CollisionReport` (`collision.rs:146-152`) carries exactly two fields:
- `collisions: Vec<CollisionEvent>` (move_index, position, penetration_depth, segment_name)
- `min_safe_stickout: f64` — calculated minimum tool extension
- `is_clear()` — true if no collisions detected

`rapid_collisions` is **not** on `CollisionReport` — it lives on `SimulationResult` (`crates/rs_cam_core/src/compute/simulate.rs:295-299`), as `rapid_collisions: Vec<RapidCollision>` plus `rapid_collision_move_indices: Vec<usize>` for timeline markers.

---

## 4. Performance Tracing

**Source:** `crates/rs_cam_core/src/debug_trace.rs` (676 lines)

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

**Source:** `crates/rs_cam_core/src/semantic_trace.rs` (1768 lines)

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

**Source:** color mapping `crates/rs_cam_viz/src/render/sim_render.rs:118` (`deviation_colors()`); render wiring `crates/rs_cam_viz/src/app/gpu_upload.rs:66-77` (`StockVizMode::Deviation`); pointwise instrument `crates/rs_cam_core/src/compute/simulate.rs:1034` (`collect_column_deviations()`).

Computes surface deviation between target model and simulated stock result.

**Measurement domain: this is a VERTICAL (Z) deviation, not surface-normal.** `deviation_colors()` computes `sim_z − model_z` (`sim_render.rs:105`), and the pointwise `ColumnDeviation.dev` is `column_top_z − model_z` in world frame (`compute/simulate.rs:262`) — both are Z-only. On a sloped face a Z deviation UNDERSTATES the true normal gouge by a factor of `cos(slope)`; steep terrain can carry a real normal-direction defect that reads small in Z. Mixing a Z-domain deviation with a surface-normal or projected-area domain has already produced one retracted number — see `planning/review_2026-07-29/MEASUREMENT_DOMAINS.md` X-1.

### Deviation Color Scheme

Per `render/sim_render.rs:105-132`. Green is a FIXED ±0.1 mm band around zero — **not** "within the operation's tolerance"; there is no tolerance parameter in this path.

| Color | Meaning | Threshold |
|-------|---------|-----------|
| Green | On target | Fixed ±0.1 mm band (not the operation's tolerance) |
| Blue | Material remaining (leftover) | deviation > 0.1 mm |
| Yellow | Slight overcut | 0.1–0.5 mm overcut |
| Red | Significant overcut (gouge) | > 0.5 mm overcut |

### The COLUMNS instrument — use this for quality metrics, not the vertex colors above

`SimulationResult::column_deviations: Option<Vec<ColumnDeviation>>` (`compute/simulate.rs:245-315`, populated by `collect_column_deviations()` at `:1034`) is the **unaveraged per-dexel-column** deviation reading. Every `ColumnDeviation` carries `top_z`, world XY, a `group` (setup ordinal), and row/col indices — index results by row/col, never by inverse-transforming XY back to a grid cell.

**Rule (`simulate.rs:245-255`): the per-vertex color path above averages and is for DISPLAY ONLY.** Vertex heights are corner-bilinear averages over 2×2 dexel columns (`dexel_mesh_mc::z_grid_marching_cubes`) — fine for a viewport, but the averaging filters machined micro-texture unevenly: grid-locked ridge patterns survive the average while phase-diverse ones cancel, so two surfaces with identical real texture can histogram very differently (P2.g Task 1, 2026-07-09). Quality metrics and fidelity histograms must read the unaveraged `column_deviations` samples instead.

**Prerequisites before trusting a `column_deviations` read:** the sim cell must sit well below the tool's TIP radius (e.g. 0.1 mm for a Ø1 tip), and `SimulationResult::resolution_clamped` must be `false` — a `true` value means the requested resolution was coarsened to fit grid limits, and `column_grid_cell_mm` (`compute/simulate.rs:317`, the effective sampled cell size) then reads larger than requested. A column population scales with cell⁻², so comparing on-size percentages or collision counts across two different effective cells compares two different populations.

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
Reduces feed rate in light-engagement regions and raises it in heavier ones to hold a more consistent chip load, which eliminates burn marks from dwelling in light cuts. The "15-30% faster cycle times" figure that used to appear here traces to an unmeasured assertion in the `feedopt.rs` module doc (`crates/rs_cam_core/src/feedopt.rs:11`) — no fixture, no baseline, no date attached. The capability is real; the number is not measured and is not quoted here.

---

## 8. Fingerprinting & Parameter Sweeps

**Source:** `crates/rs_cam_core/src/fingerprint.rs` (1253 lines) + `crates/rs_cam_cli/src/sweep.rs`

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
cargo test --test param_sweep                    # All 56 sweeps across 23 user-facing operations
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

## 8.5 Literature-Matrix Feeds Validation

**Source:** `crates/rs_cam_core/tests/literature_matrix/` (`cells.toml`, `sources.toml`) and the `_litmatrix_*.rs` sentry tests.

The literature-matrix is the canonical correctness gate for the feeds engine. Each row in `cells.toml` is a cited (material, tool, operation, vendor-or-handbook) cell; the suite cross-checks engine output against the cited band and emits a per-cell verdict.

### Verdict bands

| Verdict | Meaning |
|---------|---------|
| Within | Engine output lies inside the cited vendor / handbook envelope |
| Edge | Output sits on the band boundary (within tolerance) |
| Exceeds | Output is outside the cited band — engine bug or stale citation |

### Invariants exercised (19 across 56 cells)

Covers: chipload scaling with diameter and hardness, RPM-tiered diameter ceilings, Janka-band drill plunge envelopes, scallop tool-class refusal, rubbing-floor clamps, vendor-LUT-only RPM rows, drill plunge-feed sanity, and feed-modulation bounds.

### Provenance maintenance

`sources.toml` is the citation registry. The source-freshness reporter flags warn/stale vendor URLs; the `/refresh-lit-matrix` skill walks through stale rows and re-verifies or replaces them. New engine work that touches feeds output must keep the matrix green (`cargo test -p rs_cam_core --test literature_matrix`).

---

## 9. Wood Routing Thresholds

Reference benchmarks for 3-axis wood router analysis.

### Efficiency

**Air cut ratio has no single fixed band — it requires naming a denominator, and the codebase publishes two.** `air_cut_time_s` becomes a percentage of either `air_cut_pct_of_total_runtime` (cutting + rapids) or `air_cut_pct_of_cutting_time` (rapids excluded, always ≥ the total-runtime reading) — see `AirCutRatios` and its doc comment at `crates/rs_cam_core/src/simulation_cut.rs:537-566, 592-595`. Every SHIPPED threshold uses the total-runtime reading, and it is set PER OPERATION TYPE, not one fixed band — see `OperationType::air_cut_high_threshold_pct` (`crates/rs_cam_core/src/compute/catalog.rs:456-491`): `None` (suppressed) for Drill/AlignmentPinDrill, 97.0 for ProjectCurve, 30.0 for the 3D finish family, 40.0 for 2.5D clearing/rough and 2D contour ops.

| Metric | Good | Warning | Bad |
|--------|------|---------|-----|
| Avg engagement (roughing) — `engagement.radial_woc_fraction` (radial-WOC axis only; not axial DOC, arc, or chip thickness) | 0.3–0.5 | 0.15–0.3 | < 0.15 |
| Avg engagement (finishing) — same axis caveat | 0.1–0.4 | 0.4–0.6 | > 0.6 |

Drill toolpaths suppress both `average_engagement` and air-cut readings via `metrics_not_applicable` — see §2.1.

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
| `engagement.radial_woc_fraction ~ 1.0` | Full-width slotting | High forces — consider adaptive clearing |
| `axial_doc > cutting_length` | Over-depth | Tool damage, shank contact |
| `engagement.radial_woc_fraction < 0.02 at feed` | Air cutting | Wasted time, unnecessary wear |

---

## 10. Analysis Checklist

Use this checklist when analyzing a toolpath program:

### Safety (Critical)
- [ ] **Rapid collisions**: Any G0 moves through stock? (`RapidCollision` in collision report)
- [ ] **Holder/shank collisions**: Holder contacting stock during cuts? (`CollisionEvent`)
- [ ] **Min safe stickout**: Is current stickout sufficient? (`CollisionReport::min_safe_stickout`; `min_safe_stickout_mm` is the JSON wire key only)
- [ ] **Plunge rate**: Is plunge feed appropriate? (not faster than cutting feed without reason)
- [ ] **Depth of cut**: Does `axial_doc_mm` exceed tool cutting length anywhere?

### Efficiency
- [ ] **Air cutting ratio**: What % is air-cut, and against which denominator? Total-runtime (`air_cut_pct_of_total_runtime`) is what every shipped threshold uses — see the per-operation values in `OperationType::air_cut_high_threshold_pct` (`crates/rs_cam_core/src/compute/catalog.rs:456-491`), not a single fixed target
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
- [ ] **Tool-operation compatibility** — note: the feeds engine refuses non-curved tools on Scallop and refuses tool-class × operation mismatches at the suggest path; expect a `FeedsError`, not a silent miscompute.
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

Access via `SimulationState::issues(&mut self, gui: &GuiState, max_feed_mm_min: f64)` (`crates/rs_cam_viz/src/state/simulation.rs:1485`) in GUI, or parse JSON artifacts from CLI.

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
| `crates/rs_cam_core/src/dexel_stock/` (directory: `mod.rs`, `simulation.rs`, `stamping.rs`, `cut_direction.rs`) | Tri-dexel stock simulation engine |
| `crates/rs_cam_core/src/simulation_cut.rs` | Cut trace metrics, issues, hotspots |
| `crates/rs_cam_core/src/collision.rs` | Collision detection (holder, rapid) |
| `crates/rs_cam_core/src/debug_trace.rs` | Performance tracing, computation hotspots |
| `crates/rs_cam_core/src/semantic_trace.rs` | 26-kind structural hierarchy |
| `crates/rs_cam_core/src/fingerprint.rs` | Toolpath/stock fingerprinting and diffing |
| `crates/rs_cam_core/src/feedopt.rs` | Engagement-based feed optimization |

### GUI Integration
| File | Purpose |
|------|---------|
| `crates/rs_cam_viz/src/app/simulation.rs` | Simulation orchestration (checkpoint loading, playback) |
| `crates/rs_cam_viz/src/render/sim_render.rs` | Deviation color mapping (`deviation_colors()`) |
| `crates/rs_cam_viz/src/app/gpu_upload.rs` | Deviation render wiring (`StockVizMode::Deviation`) |
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

## 16. Operations Reference (23 user-facing operations)

Per `crates/rs_cam_core/src/compute/catalog.rs:149-181`: 24 `OperationType` variants total = 11 2.5D + 12 3D + 1 system-only (`AlignmentPinDrill`, auto-generated for stock alignment pin holes, in neither user menu). 23 user-facing operations, matching `FEATURE_CATALOG.md`.

### 2.5D Operations (11)
Face, Pocket, Profile, Adaptive, VCarve, Rest, Inlay, Zigzag, Trace, Drill, Chamfer

### 3D Operations (12)
3D Raster Finish (DropCutter), 3D Adaptive Rough, Waterline, Pencil, Scallop, Unified Finish, Steep/Shallow, Ramp Finish, Spiral Finish, Radial Finish, Horizontal Finish, Project Curve

### Tool Families (5)
Flat end mill, Ball end mill, Bull nose, V-bit, Tapered ball nose

### Dressup System
Post-generation modifications: entry style (plunge/ramp/helix), dogbones, lead-in/out, link moves, arc fitting, feed optimization, retract strategy, rapid order optimization, tabs, machining boundary.
