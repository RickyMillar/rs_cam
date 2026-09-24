# Adaptive 3D step-ladder roughing — plan (2026-09-24)

State: **PLAN, not started.** Operator rulings of 2026-09-24 are in section 5.
Every other "recommend" is a proposal.

## 1. Goal

The operator wants ONE 3D Rough toolpath that does this, pocket by pocket:

1. It clears the bulk at the main depth of cut (`depth_per_pass`).
2. Where the main step does not fit above the part, it leaves a band wide
   enough for a finer step to cut a linked pass.
3. It removes that band with the finer step, in the same pocket, before it
   travels to the next pocket.
4. It does not leave scattered islands that need many new entries.

The aim is sustained useful engagement: long cuts, few entries, few retracts.
The aim is not maximum load everywhere.

The operator watched the current rough cut and saw that it could remove much
more material in one pass if it committed to larger first cuts. The current
rivmap100 3D Rough runs Depth/Pass 2.0 mm on 25 mm stock (about 12 levels),
Global ordering, helix entry, contour parallel.

The operator compares this with the Fusion 360 pocket roughing "Fine
Stepdown" behaviour (intermediate steps up along the tool axis, after the
main step).

## 2. What the code does today (verified 2026-09-24)

| Control | Config key | Behaviour | Source |
|---|---|---|---|
| Depth/Pass | `depth_per_pass` | Main ladder from stock top to `surface_bottom + leave`, plus one short last level. | `adaptive3d/path.rs:568-580` |
| Fine Stepdown | `fine_stepdown` | Inserts extra levels in EVERY interval of the ladder. Each extra level is a normal full-footprint clear. The whole rough runs at the fine step. | `path.rs:608-639` |
| Mill Shallow / Shallow Angle / Shallow Step | `mill_shallow_areas`, `shallow_angle_deg`, `shallow_stepdown` | A static slope mask (`slope < angle`). After each main level, sub-passes run on mask cells DOWN to the next main level, through virgin bulk. | `path.rs:431-446`, `path.rs:838-865`, `clearing.rs:341-380` |
| Detect Flat | `detect_flat_areas` | Adds a full-footprint level at each shelf height. | `path.rs:588-606` |
| By Area | `region_ordering` | Detects connected material regions ONCE from the initial stock. Then it runs region → all levels → next region. | `path.rs:709-910`, `clearing.rs:154-265` |
| Z Blend | `z_blend` | Terrain-following contour Z. Leave off for pockets. | `adaptive3d/mod.rs` |

Findings:

- **F1 — Fine Stepdown is global.** The catalog help says "finer Z step for
  final passes" (`compute/catalog/registry.rs:302-303`). The code applies it
  to the whole descent. This is the "fine stepdown affects the full stepdown"
  report. Treat it as a product defect.
- **F2 — Mill Shallow cuts bulk.** Its sub-passes run BEFORE the next main
  level. At that time the material test
  (`effective_floor = max(surf + leave, z)`, `clearing.rs:374`) sees virgin
  stock above the sub-level, so the sub-pass clears bulk on every low-slope
  cell. This is the "chips away where it could plunge deep" report on open
  floors.
- **F3 — The main level already drapes to the surface.** Every cut point is
  raised to `drop_cutter + stock_to_leave` (`drape_path_to_leave`,
  `path.rs:1207-1285`). The planner stamps the same drape
  (`stamp_emitted_segment`, `clearing.rs:459-500`). Thus, where the surface
  lies inside a slab, the main level cuts that slab down to the leave in one
  bite. A simple reorder of the sub-levels gives them NO material. The
  coarse step must be clipped to where it fits (section 3.2).
- **F4 — By Area uses a bounding box, not the region label.** The region
  filter in `build_material_bool_grid` keeps every cell inside the region's
  row/col box (`clearing.rs:360-364`). Two pockets whose boxes overlap leak
  into each other. Regions are also never re-detected at lower levels, so
  pockets joined by stock above a wall stay one region.
- **F5 — Sub-passes are invisible.** Shallow sub-passes emit through
  `clear_z_level_dispatch_no_marker`. No runtime event and no per-level
  counter reports them (`planning/ab_instrument_flags_2026-09-08.md`).
- **F6 — Stock to leave is Z only.** `Adaptive3dDepth::stock_to_leave` is a
  vertical offset. The planner cannot represent a radial (wall) buffer.
- **F8 — The sub-pass loop ignores the real ladder.** Both loops stop at
  `next_main_z = z_level - depth_per_pass` (`path.rs:848`, `path.rs:994`),
  not at the next ladder entry. With Detect Flat or the short last level, the
  sub-passes run into the next slab or below `z_bottom`.
- **F9 — By Area runs the waterline cleanup once, at the end.** It runs after
  all regions (`path.rs:884-910`), so the tool returns to every pocket.
- **F7 — CLI side defect.** `order_by = "depth"` maps silently to Global
  (`crates/rs_cam_cli/src/job.rs:1354-1358`).

Existing workaround (no code change): a bulk 3D Rough (Fine Stepdown 0, Mill
Shallow off, By Area, Stock to Leave = buffer), then a second 3D Rough with
Geometry → Start from → "After previous ops" and a smaller step. Generate All
runs the prefix simulation. This is two operations and bulk-first, not
pocket-local.

## 3. Proposed design (revised 2026-09-24 after the operator's step-ladder ruling)

The operator's picture: the result looks like a normal 5 mm rough, but where
a whole 10 mm slab fits above the part, the planner cuts that 10 mm in one
pass. Optionally a 1 mm step then refines the last band.

### 3.1 A step ladder replaces three dials

Keep `depth_per_pass` as the BASE step (the last, finest-that-drapes
entry). Add the coarser steps above it in `Adaptive3dDepth` (CUT-04: the
`depth` group). Replace `fine_stepdown` and `ShallowTier`:

```rust
/// Coarser steps above `depth_per_pass`, coarsest first, e.g. [10.0]
/// with depth_per_pass 5.0, or [10.0, 5.0] with depth_per_pass 1.0.
/// Empty = today's single-step behaviour.
pub coarse_steps: Vec<f64>,
```

Decision D7 (recommend; measured 2026-09-24): keep the `depth_per_pass`
key. `rg` shows it in the feeds axial envelope
(`feeds/suggest/axial_envelope.rs:249-282`), `feeds/cutter_constraints.rs`,
the GUI panel, `param_sweep.rs` and seven adaptive3d sentries
(`adaptive3d_commanded_ladder`, `_interior_cell_parity_f029`,
`_keep_down_link_f038b`, `_subtool_channel_gouge`, `_planner_stock_xy_f027`,
…). A new `coarse_steps` key leaves all of them valid with an empty list.

Trap: every consumer that reads `depth_per_pass` as "the deepest axial
bite" must read `max(coarse_steps ∪ {depth_per_pass})` instead. At least:

- `Adaptive3dGeometry::tool_radius` (the contact radius at the pass depth,
  for tapered cutters),
- the feeds axial envelope and the DOC-derating load gate,
- the ladder sentries' "emitted span ≤ commanded DPP" ceiling.

Otherwise a 10 mm bite is invisible to the gate.

- Each coarse step must be larger than the next one and than
  `depth_per_pass`. The adapter refuses a zero step or a list that is not
  in descending order.
- Recommend: each coarser step is a whole multiple of the next finer step
  (10/5/1). Then every coarse level is also a finer level, and the terraces
  line up. Decision D4 (open): refuse a ladder that is not a multiple, or
  allow a short last step in each slab.

Operator ruling 2026-09-16 (no legacy, breaking OK) applies: delete
`fine_stepdown`, `mill_shallow_areas`, `shallow_angle_deg` and
`shallow_stepdown` outright. State the break in the commit. Do not add a
migration.

### 3.2 The fit rule: a coarse step cuts only where it fits

This is the new part. Today a level at `Z` cuts every cell with material
above `max(surf + leave, Z)`, and the drape lifts the tool onto the surface
(F3). So a 10 mm Depth/Pass today takes 10 mm draped bites down the walls.
It does NOT clip back to a smaller step.

New rule per tier:

- **Coarse tiers (every entry except the last): clip.** A cell is eligible
  at level `Z` only if the whole slab fits above the part there:
  `tool_rest_z(cell) + leave <= Z`, where `tool_rest_z` is the
  radius-aware drop-cutter height (the surface heightmap). Other cells are a
  keep-out, not air. The tool does not drape onto them.
- **The base tier (the last entry): drape.** It behaves like today. It cuts
  down to `surf + leave` and follows the surface. Its bite is at most its
  own step, because the coarser tiers have already opened the slab above it.

The drape stays on for every tier as the gouge guard. On a clipped tier it
does not move the tool, because the tool stays over cells where the slab
fits. Thus the per-segment leave trap of the first draft goes away: every
tier drapes and stamps with the one real `stock_to_leave`.

The fit test is footprint-aware already. `SurfaceHeightmap` is a per-cell
drop-cutter with the operation's cutter (`path.rs:405`), so `surf_hm(cell)`
is the tool-centre rest height for the whole footprint. A tool centred on a
fits-cell at `Z` cannot touch the part. The keep-out is therefore in
tool-centre space: the tool centre may reach the keep-out edge.

Trap: contour parallel starts its first contour at
`min(tool_radius, stepover/2)` from the material boundary
(`clearing.rs:726`), measured with one EDT. With a keep-out there are two
boundary kinds. The air boundary keeps today's offset. The keep-out
boundary needs only the link margin (3.3), not `tool_radius`. The EDT must
treat the two sources separately, or the band next to each wall grows by a
tool radius.

Also check: contour parallel sets `target_z = surf + leave` and clamps with
`max(target_z)` (`clearing.rs:757-763`). On a fits-cell `target_z <= Z`, so
the emitted Z is exactly `Z`. A2 tests this.

### 3.3 The link margin (fixed, not a dial)

The operator ruling: a fixed value, large enough that the next tier can cut
a linked pass. The coarse tier's eligible area shrinks (EDT erosion) by the
link margin from the keep-out cells. The band that the coarse tier leaves
next to a wall is therefore at least one margin wide. The next tier cuts
that band as a closed loop, not as slivers.

- Proposal: link margin = one tool diameter. Phase 1 measures it. It is
  not a universal formula.
- A coarse-fit area that is smaller than the tool diameter after the
  erosion is dropped from that tier. The next tier takes it. So a coarse
  tier never makes a deep entry for a small patch.

### 3.4 Slab order (RULED 2026-09-24: each slab, step DOWN)

The order is nested, not flat. One recursive function does it:

```text
clear_slab(tier, z_top, z_bot, region):
    cut level z_bot with tier's rule        # clip, or drape if base tier
    if tier is not the base tier:
        for each finer sub-slab (t, b) of (z_top, z_bot), top DOWN:
            clear_slab(tier + 1, t, b, region)
```

The top call runs `clear_slab(0, …)` for each coarsest slab, top down, per
region (By Area) or for the whole part (Global).

Example, ladder [10, 5, 1] from Z = 0:
`10@-10 clip → { 5@-5 clip → 1@-1 … 1@-5 drape } → { 5@-10 clip → 1@-6 …
1@-10 drape }`. It is NOT "all 5 mm levels, then all 1 mm levels".

The coarse cut runs first in each slab. The finer levels then enter from
the space it opened, so most of their entries come from the side.

A slab boundary is also a boundary at each shelf level of Detect Flat (F8:
a slab is two consecutive ladder entries, not `z_level - depth_per_pass`).

Example, ladder [10, 5], cell with the surface 7 mm below the slab top:
the 10 mm level does not fit (keep-out). The 5 mm level at `-5` fits and
cuts. The 5 mm level at `-10` is the base tier: it drapes and cuts down to
the surface (2 mm of material). A cell in the open pocket floor 30 mm down
gets 10 mm bites only.

### 3.5 The optional fine tier (the "1 mm for the last bit")

With ladder [10, 5, 1] the 5 mm tier also clips, and the 1 mm tier is the
base tier. The rough then leaves at most 1 mm terraces for the finisher,
at the cost of up to four extra levels per 5 mm band. Decision D5 (open,
decided by measurement): default ladder [10, 5] or [10, 5, 1]. Phase 5
scores both, including the Scallop Finish time after them.

### 3.6 Load on the coarse tier

A 10 mm bite with a 6 mm cutter is a heavy cut. Contour parallel works
outside-in, so the first ring of each coarse area is close to a full-width
slot at the full coarse step; the next rings take one stepover. The coarse tier cuts at
the operation's stepover and feed. The tool-load gates and the simulated
chipload must pass for that tier (memory: trust the simulated chipload, not
the static Suggest). If the gates fail, the fix is a smaller coarse step or
a per-tier feed. Decision D6 (open): add a per-tier feed factor in v1, or
only report the tier's load.

### 3.7 Strategy scope

The ladder with more than one entry runs on `contour_parallel` only (RULED
2026-09-24). With more than one entry and another strategy selected, the
adapter refuses the operation with a message. It does not fall back
silently. The other three strategies stay live with a one-entry ladder
(`adaptive3d/CLAUDE.md`); this ruling limits the scope, it does not delete a
strategy.

### 3.8 What stays and what goes

| Control | Proposal |
|---|---|
| Depth/Pass | Becomes the step ladder. One entry = today. |
| Fine Stepdown | Deleted. |
| Mill Shallow + Angle + Step | Deleted. The fit rule replaces the slope mask: a step that does not fit near a wall is not cut there. |
| Detect Flat | Stays a separate dial (RULED 2026-09-24). A shelf level is a slab boundary (3.4). |
| By Area | Stays. Phase 3 hardens it. |
| Z Blend | Stays. No change. |

### 3.9 How much new logic

Moderate. The pieces:

1. One slab loop that recurses over the ladder. It replaces the two copied
   sub-pass loops in `path.rs` (ByArea and Global), so the branch count
   goes down.
2. A clip mode in `build_material_bool_grid`: a keep-out mask from
   `tool_rest_z + leave > Z`.
3. The keep-out distance and the erosion in contour parallel. The EDT
   (`geometry::grid_field::distance_transform_2d`) already exists.
4. No change to the drape or to `stamp_emitted_segment`.

The hardest part is item 3 (the keep-out boundary must not let the first
contour overlap the wall). The fixture tests in phase 2 target it.

## 4. How we score a winner

The operator's test: time the arms, and read the average engagement over the
WHOLE cycle, rapids and transitions included.

Trap: the shipped `average_engagement` divides by the cutting time only
(`stock/simulation_cut/accumulate.rs:157-163`). Rapids, retracts and links
do not count. An arm with many short finer passes and many retracts can
show a high `average_engagement` and still lose. Do not use it alone.

Two instruments. Each keeps its own time base; do not mix them in one ratio.

1. **Cycle time (the headline).** `compute_cycle_time_breakdown`
   (`machine/kinematics.rs:881`) integrates the machine acceleration. It
   gives `total_s` and the buckets `cutting_s`, `entry_s`, `linking_s`,
   `retract_s`, `rapid_s`, `unknown_s`. Report `total_s` and the duty
   `cutting_s / total_s`. On intricate pockets many short moves cost
   acceleration time, so the nominal `length / feed` time is not enough.
   MCP does not expose this function. `rs_cam_cli` `nc_replay` calls it, so
   export the `.nc` and replay it. Untagged vertical descents land in
   `unknown_s` (`adaptive3d/CLAUDE.md`); report that bucket, do not drop it.
2. **Engagement over the whole cycle.** From the simulation summary:
   `average_engagement × cutting_runtime_s / total_runtime_s`. Both times
   here are nominal `length / feed` (`dexel_stock/simulation.rs:940`). This
   number is comparative only (`stock/CLAUDE.md`). Also report the
   time-weighted axial DOC fraction and `peak_axial_doc_mm`, because the
   point of the tier is a larger first cut.

The removed volume is almost the same for every arm (same stock, same final
leave). Thus the effective removal rate is `volume / total_s` and ranks the
arms the same as `total_s`. Do not add a third headline number.

Also report per arm: entry count, retract count, and the fragment count per
finer level (the "islands" question, section 6 Q6; no gate on it yet).

Secondary check: re-generate and time the Scallop Finish that follows the
rough. A rough that leaves a worse surface can move time into the finish.

Rules: paired arms only (same tool, feeds, stock source, machine profile,
sim resolution). The rivmap100 rough starts `from_remaining_stock` after the
Face operation; keep that for every arm. Render the stock after each arm
before any aggregate is believed.

## 5. Phases

Every phase: core first, no GUI change until phase 4. All three clearing
strategies stay live. No second stay-down dial (`max_stay_down_distance_mm`
is the one dial).

### Phase 0 — observability (do first)

1. Add a runtime event for every level with its tier (coarse, finer,
   base), region and Z. The old shallow sub-passes get the same event.
2. Add per-tier counters: cut volume, cut time, entries and retracts per
   tier, and for the waterline cleanup.
3. Report the peak and mean axial bite per tier from the simulation.
4. Build the scoring instrument of section 4 as one command: it takes a
   project and a toolpath, and prints `total_s` with its buckets, the duty,
   the whole-cycle engagement, the axial DOC figures, and the entry, retract
   and fragment counts. Recommend a `rs_cam_cli` subcommand next to
   `nc_replay`, so the same numbers come from the GUI project and from a job
   file. Commit it as soon as it lints clean, before any arm runs.

Acceptance: a debug trace shows each level with its tier and its own volume.

### Phase 1 — synthetic fixture and baseline

Build one fast, sim-free fixture in the pattern of
`tests/adaptive3d_commanded_ladder.rs`:

- a deep flat-floor pocket (the "plunge deep" case),
- a pocket with one 20° floor and one 60° wall,
- two pockets that stock joins above a dividing wall, with overlapping
  bounding boxes.

Record today's numbers for: Fine Stepdown on, Mill Shallow on, both off.
The numbers are: level count, main-role volume fraction, entries, retracts,
cut length, estimated time.

Then run the zero-code arms on rivmap100 (`~/Downloads/aspiring/rivmap100/rivmap100.toml`),
scored by section 4:

- **Arm A:** the current 3D Rough as it is (Depth/Pass 2.0).
- **Arm A':** Arm A with By Area only, to separate the ordering gain.
- **Arm B5:** one 3D Rough, Depth/Pass 5, By Area.
- **Arm B10:** one 3D Rough, Depth/Pass 10, By Area. This shows today's
  draped 10 mm bites on the walls (3.2). Expect a load-gate failure.
- **Arm Bx (proxy of the ladder):** a 3D Rough with Depth/Pass 10 and Stock
  to Leave 5.5, then a second 3D Rough, Start from "After previous ops",
  Depth/Pass 5, Stock to Leave 0.5. Score the two operations as one. The
  drape makes the first one cut near the walls too, so the proxy is only
  an estimate of the ladder's gain.

Gate: if neither B5 nor Bx beats Arm A on `total_s`, larger first cuts do
not pay on this model. Stop and report before phase 2.

### Phase 2 — step ladder (core)

1. Add `step_ladder` to `Adaptive3dDepth`. Delete the old fields and the
   old sub-pass loops in both branches of `path.rs`.
2. Add the clip mode and the keep-out mask (3.2).
3. Add the keep-out distance and the link-margin erosion to contour
   parallel (3.2, 3.3).
4. Implement the slab loop (3.4) as one function that both the ByArea and
   Global branches call.
5. For By Area, run the waterline cleanup per region, after the region's
   last slab (F9). If that is not possible in v1, state it as a known limit.

Acceptance on the phase 1 fixture:

- A1: on the deep flat pocket, ladder [10, 5] gives 10 mm bites in the
  open floor and no 5 mm level cuts there.
- A2: no clipped-tier cut point is lifted by the drape by more than one
  cell of Z (the keep-out works; the tool does not ride the wall).
- A3: the maximum axial bite of each tier ≤ its step + one cell of Z,
  measured on the emitted path against the planner stock.
- A4: the band left next to a wall by a coarse tier is at least the link
  margin wide, and the next tier cuts it as closed loops.
- A5: the final stock matches the one-entry ladder [5] within the leave.

Sentries to update: `adaptive3d_emission_byte_parity` exercises
`fine_stepdown`, so it needs an intentional re-bless with the reason in the
commit. Run the five folder sentries in `adaptive3d/CLAUDE.md`.

### Phase 3 — pocket identity (gated)

Do this phase only if the phase 2 fixture still shows cross-pocket travel.

1. Filter a region by its label mask, not its bounding box (F4). This is
   small. It can move into phase 2 if the fixture shows the leak.
2. Re-detect regions at each main level from the level's material grid.
   A region that splits gives child regions. The planner finishes one child
   (all its slabs) before it goes to a sibling.
3. Order children by nearest entry to the current tool position.

Limits to state in the result: a split can make a child that no entry style
reaches safely. Such a child falls back to the next safe entry, not to a
plunge. The plan does not promise "zero islands"; some topology makes an
island unavoidable.

Acceptance: on the joined-pockets fixture, each pocket's cuts are one contiguous run in the emitted order, and the retract count does
not rise against phase 2.

### Phase 4 — surfaces and docs

Wire the tier through the existing path only (config → `finish_3d.rs`
adapter → worker → UI; mutations through `ProjectSession::apply`):

- `Adaptive3dConfig` in `compute/operation_configs.rs`,
- the GUI 3D Rough panel `rs_cam_viz/src/ui/properties/operations/surface_3d.rs`
  ("Terrace step", "Buffer", "Max slope"),
- the catalog help in `compute/catalog/registry.rs`,
- the CLI job keys in `rs_cam_cli/src/job.rs`; fix F7 in the same package
  (refuse an unknown `order_by`),
- the MCP wire types in `rs_cam_mcp`,
- `FEATURE_CATALOG.md` and the product docs.

The MCP and CLI keys change. State the break.

### Phase 5 — live check on rivmap100 (needs the operator)

- **Arm C:** the ladder [10, 5], one operation, contour parallel, By Area.
- **Arm C1:** the ladder [10, 5, 1].

Score Arms C and C1 against Arms A and B with section 4. Winner rule (proposal):
the lowest accel-aware `total_s` for the rough plus the finish, with no new
collision and no gouge below the leave. Then the operator looks at the cut
in the GUI. This is a plan-level comparison; it is not a cutting
certification.

### Phase 2b — graded link margin (small)

Tier 1 of [10, 5] + 1 cut almost nothing (RESULTS "Phase 2 result"): both
clip tiers eroded by the same margin from the same keep-out. Clip tier k of
n uses (n − k) tool diameters, so each finer tier gets its own band. One
clip tier keeps today's single margin. Needed before any multi-step ladder
can be measured.

### Phase 2c — fit rule v2 and ladder anchor (RULED 2026-09-24)

After the demo ("it only did one pocket"), the operator ruled:

- Fit rule v2, "pocket depth fits": a pocket whose floor lies inside the
  coarse slab gets ONE coarse pass down to its floor; only steep wall cells
  (slope ≥ a named constant, 30° to start) are keep-out and get the finer
  steps. Cells whose floor is below the slab are cut at the slab bottom as
  before. This replaces 3.2's "whole slab fits" test for coarse tiers.
- Anchor: when the operation reads prior stock, the ladder starts at the
  top of the remaining material it can reach, not at the stock box top.

### Phase 6 — how to choose the ladder (experiment, later)

Operator 2026-09-24: "10 5 1 is just a demo. … if you are given the
smallest step you just double and double until max or something? thats
something to test later." Candidate rules, scored with `rough-score` on
rivmap100 and the deep fixture:

- doubling from the base step up to the axial cap (1, 2, 4, 8);
- a two-step ladder: base + the cap;
- today's single step.

The axial envelope cap bounds every step (operator 2026-09-24: "its ok that
it caps it. if its too deep, its too deep"; the feeds session clamps a
step over the cap and the card says so).

## 6. Rulings and open questions

Ruled 2026-09-24:

- D1: step DOWN inside a slab. D2: each slab (slab-local).
- Link margin: a fixed value, enough to link up a pass (3.3).
- The design is a step ladder with a fit rule (3.1-3.4): the coarse step
  cuts only where it fits.
- D3: Detect Flat stays a separate dial.
- Q5: the tier uses `contour_parallel`; other strategies refuse (3.6).
- Q7: the acceptance model is rivmap100 (deep, intricate pockets).
- Scoring: accel-aware time plus engagement over the whole cycle (section 4).

Open:

1. D4: refuse a ladder that is not whole multiples, or allow short steps?
2. D5: default ladder [10, 5] or [10, 5, 1]? Phase 5 decides by measure.
3. D6: a per-tier feed factor in v1, or report the tier load only?
4. Q6: what counts as "no islands"? The operator is not sure. Phase 1 and
   phase 5 report the fragment count per finer level; nothing gates on it.
5. The ladder for Arm C (proposal [10, 5]; the tool has 25 mm of cutting
   length and the stock is 25 mm deep).

## 7. Constraints carried from the instruction files

- Keep core GUI-free. The toolpath IR stays the boundary.
- All three `ClearingStrategy3d` variants stay live.
- The engine emits untagged vertical descents; a guard that reads intent
  alone misses them (`dressup/CLAUDE.md`).
- Stock to Leave is Z only. The link margin is a planning keep-out, not a
  radial finish allowance. Do not claim a radial allowance.
- Tests: folder sentries, the new fixture test, `cargo test -p rs_cam_core
  -q --lib` only with the operator's go-ahead, and clippy. No full heavy gate.
  Ask before any run over three minutes. Run every cargo command through
  `scripts/cargo_lane.sh`.

## 8. Sources

- Research transcript (Pi, 2026-09-24):
  `~/.pi/agent/sessions/--home-ricky-personal_repos-rs_cam--/2026-09-24T02-32-11-118Z_*.jsonl`
- Prior evidence: `planning/roughing_strategy_ab_2026-09-07/TERRAIN.md`
  (depth-keyed schedule named as missing) and `RESULTS.md` (By Area −8.1 %;
  residual air is in-cut drape, not rapids).
- Fusion reference: Autodesk "3D Offset Roughing reference" (Fine Stepdown,
  Order by Area).
