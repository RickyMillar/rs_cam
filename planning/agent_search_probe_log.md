# AgentSearch Probe Log

Chronological record of probe runs against `terrain_small.stl` with
`clearing_strategy=agent_search`. One row per run. Paired with the
diagnosis plan in `agent_search_diagnosis_plan.md`.

## Fixed setup (unless noted)

- Fixture: `fixtures/terrain_small.stl` — 100×73.3×52.6mm, 40K tris
- Stock: 110×83×55mm
- Tool: 6mm end mill
- Operation: adaptive3d, `debug_enabled=1`, `stock_top_z=55`
- `depth_per_pass=20` (yields 3 Z-levels — reduced from default 3mm to
  keep debug trace size manageable; full 18-level runs pending once
  memory permits)
- All other params default

## Diagnostic counters glossary

- `pass_count`: total `adaptive_pass` spans across all Z-levels
- `looped_passes`: `exit_reason` contained "loop" (spiral tail met head)
- `idle_passes`: `exit_reason` contained "idle" (tool stuck with no material)
- `low_yield_passes`: yield_ratio < 0.1 (wandering)
- `avg_yield_ratio`: mean across passes with yield counter
- `rapid_coll`: simulation `rapid_collision_count`
- `air_cut %`: simulation `air_cut_percentage`

## Runs

| # | Date | Label | Code/param delta | passes | loop | idle | low-yield | avg_yield | rapid_coll | air_cut% | artifacts | visual note |
|---|------|-------|------------------|--------|------|------|-----------|-----------|------------|----------|-----------|-------------|
| 1 | 2026-04-15 | baseline | none (fresh tree @ `04d3782`) | 107 | 10 | 0 | 0 | 7.60 | 34 | 94.0 | `probe_artifacts/2026-04-15_01_baseline/` | classic zigzag/scribble top-down; visible oval loops mid-stock; stalagmite stringers of uncut material in iso views |
| 2 | 2026-04-15 | h1_angle0.20 | search.rs:255,299 `ad * 0.12` → `ad * 0.20` (both narrow+coarse phase) | 73 | 8 | 0 | 0 | 8.31 | 20 | 85.3 | `probe_artifacts/2026-04-15_02_h1_angle0.20/` | visibly smoother arcs, less criss-cross density; still some tangles but longer arc segments; simulation shows deeper material removal and fewer stalagmite stringers; move_count -72%, runtime -72%, avg_engagement 3x higher |
| 3 | 2026-04-15 | h1_angle0.30 | `ad * 0.30` (both phases) | 60 | 20 | 0 | 0 | 7.16 | 19 | **40.0** | `probe_artifacts/2026-04-15_03_h1_angle0.30/` | **large smooth sweeping arcs, dramatically less crossings** — closest to "classic adaptive arcs" reference. 20 loop_closed = productive spiral closures (not sticky loops; `search_evaluations`/step low). avg_engagement=0.150 (15× baseline), cutting_distance=6.7m (vs 105m baseline), runtime=5min (vs 71min). engagement-meters ≈ baseline, so same material removed with ~5% the moves. |
| 4 | 2026-04-15 | h1_angle0.40 | `ad * 0.40` (both phases) | 57 | 18 | **1** | 0 | 7.07 | 24 | 87.8 | `probe_artifacts/2026-04-15_04_h1_angle0.40/` | **regressed from 0.30** — move_count 19.6k (6x of 0.30), engagement dropped to 0.029, air_cut back to 88%. First `idle_passes: 1` — at this weight the scorer over-constrains, tool runs straight lines then gets stuck. 4 passes hit 5001-step cap. Visual top-down shows more zigzag density than 0.30. Weight too high; 0.30 is the sweet spot. |
| 5 | 2026-04-15 | validate_3mm_dp5 | `ad * 0.30`; switched to **3mm end mill**, `depth_per_pass=5` (11 Z-levels) | 525 | 36 | 0 | 0 | 37.41 | 188 | 83.1 | `probe_artifacts/2026-04-15_05_validate_3mm_dp5/` | loop rate 6.9% (lower than baseline's 9.3%), idle still 0, no 5001-caps. Per-pass path shape is fine. **But sim reveals coverage problem:** scattered patches of cut material, stalagmite stringers on side walls, no systematic terraced clearance. Angle-weight fix is necessary but not sufficient. |
| 6 | 2026-04-15 | contour_3mm_ref | `clearing_strategy=contour_parallel` (reference), 3mm tool, dp=5 | n/a | n/a | n/a | n/a | n/a | **3044** | 62.1 | `probe_artifacts/2026-04-15_06_contour_3mm_reference/` | reference: what "clean" looks like for this fixture. Toolpath shows concentric contour rings at each Z-level. Sim shows terraced horizontal bands with terrain shape emerging cleanly. But 88m rapid distance and 3044 rapid collisions — the rapid-heavy overhead is its own problem. AgentSearch should be able to match contour's systematic coverage while keeping short rapids. |
| 7 | 2026-04-15 | ported_3mm_dp5 | **Ported 2D scoring structure**: angle weight `0.30 → 0.03`; added `min_frac`/`max_frac` ±5% tolerance band; `best_good` (in-tolerance) + `best_any` (fallback) acceptance pattern. `adaptive3d/search.rs`. 3mm tool, dp=5. | 678 | 16 | 0 | 0 | 44.36 | 208 | 89.4 | `probe_artifacts/2026-04-15_07_ported_3mm_dp5/` | **Coverage improved dramatically** — sim top-down now shows systematic gradient across whole stock (terrain shape emerging), vs run #5's scattered patches. Loop closures halved (36→16). But: move_count doubled (93k→185k), engagement halved (0.025→0.013), preflight skips more than doubled (52→118). The tolerance band is correctly accepting more candidates but algorithm takes many small steps instead of large commits. Stalagmite stringers at stock edges unchanged — needs wall_bias (H4 territory). |
| 8 | 2026-04-15 | ported_wallbias_3mm | **Added wall_bias**: per-Z-level EDT via `distance_transform_2d` in `clear_z_level`, stored as `BoundaryField` struct. Scorer adds `(1-alignment)*0.15` penalty when candidate lands within `2*tool_radius` of a boundary and diverges from tangent. | 617 | 21 | 0 | 0 | 46.77 | 179 | 84.2 | `probe_artifacts/2026-04-15_08_ported_wallbias_3mm/` | **Big efficiency jump** — move_count -38% (185k→115k), engagement +36%, preflight skips -45% (118→65), air_cut -5pp. Sim top-down is now deep blue/green with terrain shape clearly visible (vs #7's lighter gradient). Side-view stalagmite stringers STILL present at stock corners — geometric limit: 3mm tool can't reach within 1.5mm of stock edge (plus stock_to_leave margin). That's a ~3mm unreachable strip = the residual red. Not a scoring fix. |
| 9 | 2026-04-15 | arc_baseline | **Infrastructure added**: (1) live tuning atomics — 4 knobs settable via `set_toolpath_param agent_*` with no restart; (2) per-pass arc counters (`mean_angle_delta`, `angle_delta_std`, `sign_flip_rate`, `path_length`, `sinuosity`) + aggregated in `arc_quality` diagnostic block. Run at defaults (angle_weight=0.03, wall_bias=0.15). | 617 | 21 | 0 | 0 | 46.77 | n/a | n/a | `probe_artifacts/2026-04-15_09_arc_baseline/` | **Smoking gun found**: 274/617 passes (44%) have `sign_flip_rate > 0.3`. Worst offenders: 5001-step passes in **0.13mm × 0.37mm bboxes**, `sign_flip_rate = 0.9998`, `mean_angle_delta = π rad (180°)`. The tool is literally 2-cycle ping-ponging thousands of times. `max_sinuosity = 1876`. **This is the "stuck in corner" you saw.** |
| 10 | 2026-04-15 | uturn_combo | Added U-turn surcharge knobs: `(ad - uturn_threshold) * uturn_weight` added to score when `ad > threshold`. Running with `uturn_threshold=0.7`, `uturn_weight=10`, `angle_weight=0.15`. | 571 | 27 | 0 | 0 | — | 196 | 82.7 | `probe_artifacts/2026-04-15_10_uturn_combo/` | **Ping-pong eliminated in worst cases.** Top-3 worst arc passes now have sign_flip_rate 0.40-0.48 (was 0.99-1.00). max_sinuosity 355 (was 1876, -81%). avg_mean_angle_delta 0.20 rad (was 0.35). avg_sinuosity 7.37 (was 18.77). Move count 89k (was 115k default, -22%). Visual: distinct structured passes now visible in toolpath top-down (not uniform green mess). Sim: terrain shape clearly emerging, good coverage. Still 266/571 zigzag passes (47%) — individual passes are more structured but the algorithm still chooses many turn-heavy paths. |

## Angle weight + U-turn sweep summary (all at 3mm tool, dp=5, default wall_bias 0.15)

| angle_w | uturn | max_sinuosity | avg_sinuosity | avg_mean_delta | zigzag | moves |
|---------|-------|---------------|---------------|----------------|--------|-------|
| 0.03 | off (5.0@0.85 = default) | 1876 | 18.77 | 0.35 | 274 | 115k |
| 0.08 | off | 361 | 8.73 | 0.26 | 281 | 129k |
| 0.15 | off | 377 | 7.34 | 0.21 | 261 | 111k |
| 0.25 | off | 268 | 5.62 | 0.18 | 246 | 80k |
| 0.03 | 5.0@0.85 | 750 | 12.39 | 0.29 | 293 | 140k |
| 0.03 | 10@0.70 | 290 | 8.96 | 0.28 | 283 | 143k |
| **0.15** | **10@0.70** | **355** | **7.37** | **0.20** | 266 | 89k |

Best single-knob setting: angle_weight=0.25 (lowest zigzag_passes at 246, and smooth metrics).
Best combined (w/ visible structure): 0.15 + uturn 10@0.70.

## Overnight iteration (2026-04-15 → 2026-04-16)

**Morning report**: `planning/agent_search_morning_report_2026-04-16.md`.

**Key finding via research**: The 2D adaptive's `find_entry_point` has a
Phase 1 that walks the material polygon boundary — this is the "outside-
in" anchor that makes spirals form. The 3D had only the Phase 2 fallback
(grid scan). That's why 3D produces scattered patches rather than one
expanding spiral.

## 2.5D slice architecture (2026-04-16 afternoon)

Replaced the 3D agent-based `clear_z_level` (700+ LOC that produced
scattered/wandering spirals) with a 2.5D slice wrapper that calls the
proven 2D `adaptive_toolpath` on each per-Z-level material polygon.
See `agent_search_morning_report_2026-04-16.md` and the "Run #17" entry
below.

**Metrics vs prior best (run #13 baseline)**:

| metric | #13 (old agent) | #17 (2.5D slice) | delta |
|---|---|---|---|
| move_count | 34k | **17k** | −50% |
| air_cut % | 57.6 | **40.8** | −17pp |
| hotspot_count | 500+ | 7 | −99% |
| visual | scattered spirals | **layered clean adaptive** | ✓ |

Remaining knowns:
- 552 rapid_collisions (vs 188 baseline). Caused by project default
  `safe_z=10` being below the 55mm stock top — every rapid retract
  lands inside stock. Not an algorithm issue; user-configurable.
- 2D adaptive emits many Rapid segments between disjoint slice
  fragments (peaks poking through upper Z-levels). Changed Link→Rapid
  so every reposition goes through safe_z; needs safe_z ≥ stock_top
  to be collision-free.

**Cleanup (same commit)**: deleted ~2000 LOC of dead code now that the
old agent algorithm is gone:
- `clear_z_level` (3D agent pass loop)
- `pre_stamp_thin_bands`
- `BoundaryField`, `SearchDirection3dResult`
- `search_direction_3d`, `search_direction_3d_with_metrics`
- `find_entry_3d`, `scan_entry_3d_bounds`
- `compute_engagement_3d` (2D slice uses 2D adaptive's engagement)
- `path_bounds_3d`, `local_material_sum`
- Entire `tuning.rs` module (7 atomic tuning knobs + dispatch)
- Old engagement/search/find-entry tests (7 tests)

All 27 remaining adaptive3d tests pass. Clippy clean on core + viz.

## Run #15 — breakthrough (2026-04-16)

Combined: Phase 1 boundary walk (from overnight) + new
`agent_max_z_descent_ratio=1.0` knob (limits per-step Z drop to
`step_len × ratio` — previously the tool could plunge up to
`depth_per_pass` per 0.5mm XY step, a 20:1 ramp).

Tuning: angle_weight=0.15, uturn 10@0.70, tolerance=0.3,
min_engagement_floor=0, **max_z_descent_ratio=1.0**.

| metric | run #13 | #15 breakthrough |
|---|---|---|
| pass_count | 566 | 472 |
| productive passes | 402 | 215 |
| moves/productive pass | ~85 | ~470 (5.5x longer) |
| avg_mean_engagement | 0.10 | 0.09 (similar) |
| max_engagement | 0.52 | 0.58 |
| air_cut % | 57.6 | 80.2 |
| **visual** | scattered patches | **visible concentric spirals** |

`probe_artifacts/2026-04-16_15_zlimit_breakthrough/toolpath.png` shows
circular arc patterns (concentric contours forming) and
`simulation.png` shows full terrain coverage with emerging shape.

Remaining issues:
- 257 preflight_skip still wasteful
- avg_sinuosity 20.7 (longer passes wander more)
- engagement 0.09 vs target 0.167 (still drifting from frontier)
- Needs DOC in engagement calc (binary XY test only — axial depth
  not factored)

## Overnight / earlier

- Added `polygons: Vec<Vec<P2>>` field to `BoundaryField` — populated
  via `marching_squares_bool_grid` in `clear_z_level`.
- Added Phase 1 to `find_entry_3d`: walks each boundary polygon edge,
  samples at `cell_size * 2`, picks the point with highest
  `compute_engagement_3d` that isn't near a prior endpoint.
- Falls back to existing Phase 2 (grid scan) if Phase 1 finds nothing.
- Test callers of `find_entry_3d` pass `None` (unchanged behavior).

**Build**: clean. **Clippy**: clean. **41 adaptive3d tests**: pass.

Expected effect: passes chain into longer outward spirals instead of
scattered inward ones. `pass_count` should drop and `avg_sinuosity`
should approach 1.0.

## Runs 11, 12 — tool size and frontier entry

| # | setup | pass_count | max_sinuosity | avg_sinuosity | zigzag | air_cut% | engagement |
|---|-------|-----------|---------------|---------------|--------|----------|------------|
| 11 | 2mm tool, stepover 0.5, dp=10 (baseline entry) | 873 | 237 | 4.95 | 518 | 76.0 | 0.08 |
| 12 | 2mm + **frontier entry**: `scan_entry_3d_bounds` ranks candidates by combined dist + engagement-gap (ideal=0.4) instead of pure nearest-material | 883 | 214 | 4.31 | 526 | 73.8 | 0.08 |

Frontier-entry change: **modest improvement**. avg_sinuosity 4.95→4.31 (-13%). max_sinuosity 237→214 (-10%). Preflight skips went up (78→104) — the engagement-preferring entry occasionally chose sites the search couldn't immediately spiral from.

But pass_count stayed at 883 — the algorithm still produces many short passes rather than one continuous outward spiral. **The visual "soup" isn't from bad individual passes; it's from 883 valid arcs drawn on the same canvas.** Individual passes with `sinuosity < 1.0` are clean sweeping arcs — we just can't see them in the overlap.

## Protocol for adding a run

After each probe:

1. Copy the tool-result debug trace file to
   `planning/probe_artifacts/<date>_<nn>_<label>/debug_trace.json`
2. Copy toolpath + simulation screenshots into the same dir
3. Append one row to the table above. Use short labels; put detail
   in the run's dir if needed.
4. If the trace response was too large and got jq-extracted,
   include the `passes_by_exit_reason` breakdown as a sub-bullet:

   - **run #N breakdown**: `no material: X`, `preflight skip: Y`, `loop closed: Z`, `no entry: W`

## Exit reason breakdowns

- **#1 baseline**: `no material: 85`, `preflight skip: 11`, `loop closed: 10`, `no entry: 1`
- **#2 h1_angle0.20**: `no material: 53`, `preflight skip: 11`, `loop closed: 8`, `no entry: 1` — plus 3 passes hit the 5001-step cap (not visible in counters directly; visible in annotations: Pass 19 @ z=34.7, Pass 26 @ z=18.5, Pass 14 @ z=4.1). These are long productive spirals, not wandering.
- **#3 h1_angle0.30**: `no material: 37`, `loop closed: 20`, `preflight skip: 2`, `no entry: 1` — no 5001-step caps this time. Loop closures are at all Z-levels, reasonably short passes (30–280 steps). The big drop in preflight skips (11→2) suggests the algorithm is now finding viable directions on entry where before it was giving up.
- **#4 h1_angle0.40**: `no material: 33`, `loop closed: 18`, `preflight skip: 4`, `idle: 1`, `no entry: 1`. Preflight skips bounced back up (2→4). New `idle` exit type appeared (Pass 5 @ z=15, 78 steps). 4 passes hit 5001-step cap (Pass 20 z=30.1, Pass 14 z=7.0, Pass 15 z=6.1, Pass 19 z=7.9). The weight over-constrains direction choice and the algorithm thrashes.

## Sweep summary — angle weight

| weight | move_count | air_cut% | engagement | rapid_coll | verdict |
|--------|-----------|----------|------------|------------|---------|
| 0.12 (baseline) | 61,802 | 94.0 | 0.010 | 34 | zigzag scribble |
| 0.20 | 17,545 | 85.3 | 0.031 | 20 | smoother, still tangled |
| **0.30** | **3,278** | **40.0** | **0.150** | 19 | **sweet spot — clean arcs** |
| 0.40 | 19,647 | 87.8 | 0.029 | 24 | over-constrained, idle bails |

Recommend settling on **0.30** as the new default. Consider: does the sweet spot shift under different `depth_per_pass` or `stepover` values? Worth sweeping on #5+.
