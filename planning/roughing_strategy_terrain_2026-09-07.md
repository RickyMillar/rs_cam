# Roughing strategy for heightfield terrain — scoping (2026-09-07)

Read-only scoping. Nobody built or measured anything here. The measurement is a
follow-up. This document ranks a hypothesis, grounds each candidate in the code,
states what depth-of-cut variation exists, and designs the A/B run.

## The question

Is `adaptive3d` the most efficient rough for a terrain-relief surface — a
heightfield STL machined from the top with a flat 6 mm end mill? The operator
suspects a simpler raster pass, or a scallop / drop-cutter pass, with varied
depth of cut, roughs a monotone heightfield more efficiently than `adaptive3d`.

Baseline (wanaka200 front, 240×250 mm, 25 mm stock): `adaptive3d`
`contour_parallel`, stepover 1.2, `depth_per_pass` 4.2, 6 mm flat end mill.
The front rough measured **54 % air-cut of total runtime** — the worst ratio in
the project (`planning/airrun_2026-08-19/EFFICIENCY_CAMPAIGN_2026-08-23.md:18`;
the ~44 % in the task brief is an earlier run of the same op). Toolpath 5 (front
rough) emitted **42,667 mm of rapid** (`planning/airrun_2026-08-19/RUN_LOG.md:351`).

## Summary answer

1. On THIS machine and this terrain the 54 % air is **structural to
   `adaptive3d`**, not a feeds problem. Feed and DOC tuning is a measured
   negative for the front rough (see the evidence section). A strategy change is
   the only lever left.
2. Three of the four candidates the brief names are **not area-clearing roughs
   in this codebase**. Waterline emits contours, not a fill. Drop-cutter and
   scallop are surface-following skims, not volume clears. Zigzag is
   terrain-blind. Only `adaptive3d` clears terrain volumetrically today.
3. The operator's "simpler raster rough" maps to a **terrain-clipped
   boustrophedon fill per Z level inside `adaptive3d`** — new code on the
   existing scaffolding, not a new operation. This is the most promising lever
   and the top-ranked hypothesis, but it is not runnable today.
4. Runnable **today** without new code, in the order ranked below: `ByArea`
   region ordering with raised stay-down, `ContourSpiral` (gated on
   `recommend_clearing_strategy`, because this is a belt router), ramp entry, and
   a bigger-DOC / fewer-levels rebalance. Rank and run these first; they bound
   how much the new fill must beat.

## How adaptive3d roughs a heightfield

`adaptive3d` is a Z-level rough. It plans constant-Z levels from the stock top
down to `surface_bottom + stock_to_leave` (`adaptive3d/path.rs:481-497`,
`z_floor` clamp at `path.rs:487-489`). At each level it clears a 2D fill inside
the material footprint, then drops to the next level.

Two mechanisms make the emitted path fully 3D, and both produce air:

1. **Per-level footprint routes AROUND cleared cells.** A cell is "material" at
   a level only when stock still sits above
   `effective_floor = max(surf_z + stock_to_leave, z_level)`
   (`adaptive3d/clearing.rs:374`, grid build `clearing.rs:335-390`). On a
   heightfield the high levels touch only the peaks that rise above the level, so the
   footprint is many small scattered islands. The fill offsets inside each
   island and links or retracts between them → inter-region rapids and per-island
   entries.
2. **In-region Z drapes to the surface.** Where the surface sits above the
   level, the emitted cut Z blends from `z_level` down to `surf_z +
   stock_to_leave` (`adaptive3d/clearing.rs:749-765`, the Z-blended drape
   block). So the tool rides the terrain and can ride over already-cleared
   peaks between islands.

The measured evidence supports mechanism (1) directly. Widening the front-rough
stepover 1.2→2.4 tripled back-rough rapids, from 19.8 km to 55.2 km
(`planning/airrun_2026-08-19/OVERNIGHT_TUNING_2026-08-23.md:28`). The front
rough also emitted 42,667 mm of rapid (`RUN_LOG.md:351`). Mechanism (2) is
code-derived, not separately measured here.

Note: `crosses_standing_material` is NOT an air metric. It reports the peak
axial bite and the percent of samples over 3× the pass's own median bite
(`RUN_LOG.md:413-431`). It is a load and quality signal — a cut crossing uncut
stair-step material — not a measure of riding over cleared peaks. Do not read
its 22 % front-rough reading as air.

The machine adds to the penalty. The wanaka machine is a Shapeoko Pro XXL belt
router: XY acceleration 500 mm/s², **Z acceleration 270 mm/s²**, junction
deviation 0.02 mm (`planning/airrun_2026-08-19/wanaka200.toml:67-74`). On a
low-acceleration machine, long straight passes at cruising feed beat
corner-dense paths, and every retract or plunge is a slow Z move
(`strategy_advisor.rs:11-28`). Fragmentation and retracts are both costly on
this machine.

## The candidate strategies, grounded in the code

### adaptive3d clearing strategies (all volumetric terrain-aware roughs)

The enum is `ClearingStrategy3d` (`adaptive3d/mod.rs:63-84`), mapped from the op
config `ClearingStrategy` (`compute/operation_configs.rs:40-52`) at
`compute/execute.rs:1734-1744`. The per-level dispatch seam is
`clear_z_level_dispatch_no_marker` (`adaptive3d/clearing.rs:77-116`).

- **ContourParallel** (default) — concentric EDT offsets from the island
  boundary inward (`clear_z_level_contour_parallel`, `clearing.rs:646`). Fast to
  plan. On fragmented terrain it offsets inside each island and links between
  them.
- **Adaptive** — curvature-adjusted variable-offset EDT (`clearing.rs:101`).
  Shorter cutting distance on curved terrain than ContourParallel; same
  island-and-link topology.
- **AgentSearch** — per-step direction search with widen-band recovery
  (`clearing.rs:112`). Slow to plan; a fallback for geometry that defeats EDT.
- **ContourSpiral** — one continuous inside-out stay-down spiral per island
  (`adaptive3d/mod.rs:78-83`). It emits fewer retracts than ContourParallel, but
  it is corner-dense. Prior work found the spiral beats AgentSearch on
  wall-clock (memory: contour-spiral wall-clock), not that it beats
  ContourParallel. The ContourParallel-vs-ContourSpiral winner FLIPS with
  machine acceleration: on a low-accel belt router the parallel's straights win;
  on a rigid machine the spiral's shorter path wins (`strategy_advisor.rs:11-28`).
  The wanaka machine is the belt router, so the spiral is not a safe assumption
  here. `recommend_clearing_strategy` (MCP, `mcp_server.rs:512`) times both
  through the accel-aware integrator and settles it per machine.

All four share the per-level island-and-drape topology above. None sweeps the
footprint with a single serpentine. Note the ContourSpiral seam:
`adaptive3d/mod.rs:80-82` states it "routes through the AgentSearch slice
dispatch with the spiral as the 2D generator". So the per-level dispatch already
accepts a pluggable 2D fill on the marching-squares region polygon.

### Waterline — a profile, not a rough

`waterline_toolpath` emits **closed contours per Z level** through
`emit_closed_contour_with_intent` (`waterline.rs:138`, contours at
`waterline.rs:64`). It traces the terrain outline at each height; it does not
clear the interior. A "waterline rough" would leave every plateau standing. Not
a bulk-removal candidate.

### Zigzag — terrain-blind

`ZigzagParams` is one 2D polygon at one `cut_depth` (`zigzag.rs:17-31`). It has
no mesh, no drop-cutter, no per-XY Z. Depth-stepping it through
`depth::toolpath_at_levels` cuts flat planes; on a heightfield the flat plane
gouges everywhere the surface is above the plane. Not usable as-is.

### Drop-cutter / scallop — surface-following skims, not volume clears

`point_drop_cutter` (`dropcutter.rs:15`) drops the tool onto the mesh and rides
the surface in a serpentine (`raster_toolpath_from_grid`). This is a finish
pattern: one thin skim at the surface. `DropCutterConfig` carries `stepover,
feed_rate, plunge_rate, min_z, slope_from, slope_to, spindle_rpm, scallop_height`
and **no `stock_to_leave`** (`compute/operation_configs.rs:539-558`). So there is
no dial to lift the whole drop-cutter surface up and skim a cascade of higher
offset surfaces. A drop-cutter cascade rough — the literal "raster with varied
DOC following the surface" — is **not configurable today**; it needs a
`stock_to_leave` (Z offset) field plus multi-pass cascade orchestration, and
even then it terraces steep walls (each cascade step leaves an axial ring) and
does not guarantee bulk removal. On a mostly-shallow relief the terracing is
minor; on the river walls it is not. Scallop has the same skim shape and adds a
ball-tip requirement.

### Horizontal-finish — near-flat only

`horizontal_finish` (`horizontal_finish.rs`) cuts only near-flat areas. It is
useless on terrain (CLAUDE.md pitfall). It leaves every sloped face uncut. Not a
rough candidate.

## Depth-of-cut variation — what exists vs what is missing

Uniform depth stepping is `depth::DepthStepping` with `Even` and `Constant`
distribution (`depth.rs:15-115`). Both are uniform per operation; neither varies
the bite by depth.

`adaptive3d` adds three finer-level mechanisms:

- **`fine_stepdown`** — inserts intermediate Z levels at a fixed finer interval,
  uniformly over the whole part (`adaptive3d/mod.rs:135`).
- **`shallow_stepdown` + `mill_shallow_areas` + `shallow_angle_rad`** — inserts
  fine sub-passes on **low-slope** cells within each DOC descent, while steep
  cells keep the normal cadence (`adaptive3d/mod.rs:172-184`,
  `operation_configs.rs:648-650`). This is depth-of-cut variation keyed to
  **slope**, not depth.
- **`detect_flat_areas`** — inserts extra Z levels at shelf heights
  (`adaptive3d/mod.rs:137`, plan at `path.rs:510-541`).

**Missing:** the operator's depth-keyed schedule — big bites in the deep bulk,
small bites near the surface. No coarse-then-fine-by-depth ramp exists. The
proven proxy is E5: raise `depth_per_pass` and let fewer levels remove the bulk,
then a separate finish removes the skin. That is a manual two-op cascade, not one
op with a graded schedule.

## Ranked hypothesis (most efficient heightfield rough first)

1. **New: terrain-clipped boustrophedon fill per Z level, inside `adaptive3d`.**
   Sweep the whole per-level material footprint with long parallel serpentine
   passes, keep the drape and clip `adaptive3d` already computes, and lift only
   at the footprint edge. Rationale: it converts fragmentation rapids and
   island entries into in-plane feed, and long straights hold cruising feed on
   this low-accel machine (`strategy_advisor.rs:11-28`). It reuses the terrain
   grid, drape, and Z-level plan; only the fill generator is new. This is the
   operator's hypothesis. **Not runnable today.**
2. **ByArea + raised `max_stay_down_distance_mm` (runnable today).** Clear each
   island fully before moving, and replace inter-island retracts with stay-down
   links. It attacks the measured fragmentation rapids directly. The baseline
   runs `region_ordering = global`, so this is a genuine change.
3. **ContourSpiral (runnable today, accel-dependent).** Stay-down continuous
   spiral; fewer retracts than ContourParallel. The winner against
   ContourParallel flips with acceleration, and this is a belt router, so the
   spiral is not a safe bet. Run `recommend_clearing_strategy` first; run the
   full sim only if it favours the spiral.
4. **Bigger `depth_per_pass` + fewer levels (runnable today).** E5 gave −60 % on
   the back rough this way. The front terrain fragments more, so pair it with
   (2) or (3), not alone (E4 showed wider stepover alone backfires here).
5. **Ramp entry (runnable today).** `EntryStyle3d::Ramp` removes the per-island
   peck-ladder fed air (`path.rs:30-49`). A small, safe win that stacks with the
   others.

Least suitable: waterline (leaves plateaus), drop-cutter / scallop cascade
(skim, terraces, no offset dial, needs new code), zigzag (terrain-blind).

## A/B experiment plan

Split by feasibility.

### Runnable today (no new code)

Fix everything except the one variable per arm. Use the MCP live-control loop.

- **Arm A — baseline.** `adaptive3d` `clearing_strategy = contour_parallel`,
  stepover 1.2, `depth_per_pass` 4.2, 6 mm flat, F750, and these baseline field
  values: `region_ordering = global`, `entry_style = plunge`, `fine_stepdown =
  0.0`, `detect_flat_areas = false`, `mill_shallow_areas = false`
  (`planning/airrun_2026-08-19/OVERNIGHT_TUNING_2026-08-23.md:12-14`; field names
  from `compute/operation_configs.rs:597-666`). State each value so Arms C, D,
  and E are known changes, not no-ops.
- **Arm B — ContourSpiral.** Arm A with `clearing_strategy = contour_spiral`.
  Gate this arm on `recommend_clearing_strategy` first (belt-router accel).
- **Arm C — ByArea stay-down.** Arm A with `region_ordering = by_area` and a
  raised `max_stay_down_distance_mm`.
- **Arm D — ramp entry.** Arm A with `entry_style = ramp`.
- **Arm E — bigger DOC.** Arm A with `depth_per_pass` raised (e.g. 5.46, the E5
  value) plus `fine_stepdown` or `shallow_stepdown` near the surface.

MCP sequence per arm:
`load_project` → `get_operation_schema` (confirm the exact param keys
`set_toolpath_param` accepts) → `set_toolpath_param` (the one variable) →
`generate_all(simulation_resolution_mm = <fixed>)` → `run_simulation` →
`get_diagnostics` (read `triage` first) → `get_tool_load_report` →
`narrate_toolpath(index)` → `screenshot_simulation(include_rapids:false)`.
Before reading the intent split, confirm `runtime_by_intent` is `Some`; a
mutation-path defect once left it `None` (`RUN_LOG.md:485-506`).

The CLI batch alternative: `cargo run -p rs_cam_cli -- job <arm>.toml` then the
`project` report for the `ToolpathDiagnostic` rows. `crates/rs_cam_core/tests/param_sweep.rs`
is the wrong instrument here — it runs a synthetic hemisphere and fingerprints
one op; it does not give whole-board wall-clock.

### Requires implementation first

- **Arm F — terrain-clipped boustrophedon fill.** The per-level slice dispatch
  (`clear_z_level_agent_2d_slice`, `clearing.rs:1463-1560`) already extracts each
  region as a `Polygon2` via marching squares. `zigzag_lines_reported`
  (`zigzag.rs`) takes exactly that polygon, so the raster lines are cheap to
  produce. The remaining work is an adapter: convert the 2D lines into draped
  `Adaptive3dSegment`s with the surface drape (`clearing.rs:749-765`) and the
  entry / stay-down link handling the spiral and agent paths already build. The
  seam exists; the fill contract needs the adapter. Compare against the best
  runnable arm.
- **Arm G — drop-cutter cascade.** Needs a `stock_to_leave` (Z offset) field on
  `DropCutterConfig` plus cascade orchestration. Lower priority: it terraces
  steep walls and does not guarantee bulk removal. Run only if Arm F disappoints
  and the terrain is shallow enough that terracing is acceptable.

### What to measure (identical across arms)

- **Total wall-clock** through the accel-aware integrator
  (`SimulationCutTrace::toolpath_runtimes`) — never path length. Accel decides
  the winner on this machine.
- **`air_cut_pct_of_total_runtime`** — the tuned measure. Do NOT quote
  `narrate_toolpath`'s cutting-time air %.
- **Cutting vs rapid vs air split** — `runtime_by_intent`.
- **Retract count** — `ToolpathStats::retract_trips`. Prior work found this air
  is count-bound, not distance-bound (memory: link/reorder capabilities).
- **Remaining-stock uniformity for the finish** — `untouched_material_mm2` and
  `truncated_core_mm2`, **plus** `screenshot_simulation`. Never gate on an
  aggregate without rendering the surface.
- **Safety** — `rapid_collision_count` and holder strikes from `triage.safety`,
  at **one** `simulation_resolution_mm` for every arm; collision counts are
  resolution-scaled and cross-resolution comparison is meaningless.

### Controls

- One `stock_to_leave` for every arm (matched finish allowance).
- One 6 mm flat end mill for every arm (keeps engagement denominators fixed;
  flat tools read the envelope radius, unchanged by the 2026-08-28 denominator
  change).
- One `simulation_resolution_mm` for every arm.
- Re-generate after every `set_toolpath_param` (the toolpath goes stale).

## Verdict criterion

An arm wins if it lowers total wall-clock AND `air_cut_pct_of_total_runtime`
without raising collisions or worsening finish-stock uniformity
(`untouched`/`truncated` and the rendered surface). If no runnable arm reaches a
sane air ratio (the C2 outcome was an honest negative on feeds — see below),
that is the evidence to fund Arm F.

## Prior evidence this rests on

- Front rough 54 % air, worst in the project
  (`EFFICIENCY_CAMPAIGN_2026-08-23.md:18`).
- Wider stepover backfires on this terrain: back-rough rapids tripled,
  front standing-crossings 22 % (`OVERNIGHT_TUNING_2026-08-23.md:28`).
- E5: bigger DOC + fewer levels + RPM headroom gave −60 % on the back rough
  (`OVERNIGHT_TUNING_2026-08-23.md:29`).
- C2 front-rough feeds optimize was an honest negative — feed/DOC search cannot
  reach the structural air (`OVERNIGHT_TUNING_2026-08-23.md:145-150`).
- Front rough emitted 42,667 mm of rapid (`RUN_LOG.md:351`).
- Machine is a low-accel belt router, Z accel 270 mm/s²
  (`wanaka200.toml:67-74`).
