# C4, "Contour each level": pre-registration (for approval before code)

Date: 2026-10-28. Base: `geodesic-integration` `d303f4103` (C3). Work in an
isolated detached worktree with its own lane script and target directory.
Rulings: OPERATOR_DECISIONS_2026-10-28.md, "Operator rulings on D69": one
visible 3D Rough option, off by default, covering walls and the boundary
band; it fixes G-RESTDOC when on. Inputs: phase3/d68 (option 2, the anchor
fix, many regions), phase3/d69 §1 (the merge rule, +15 to 21 % cost).

## 0. One change, not C4a + C4

The D63 anchor fix (`805368f98`) changes shipped output alone (CP rest ops
on remaining stock with a containment and stock in reach), and D68 §0
measured that alone it makes Corne WORSE: over-DPP volume 6750 → 14062 mm³,
max axial 8.96 → 12.0. So I port it INSIDE C4, behind the same option. With
the option off, the anchor reads the stock as today. No C4a.

## 1. The config field

- `Adaptive3dConfig::contour_each_level: bool`.
- Serde: `#[serde(default, skip_serializing_if = "std::ops::Not::not")]`
  (the existing idiom, `operation_configs.rs:879`). A project without the
  key loads `false`; a saved project with the option off has no key, so
  saved files do not change.
- `#[param(help = …)]` for the schema (`OpParams` derive), so MCP
  `get_operation_schema` and `set_toolpath_param contour_each_level` see it
  through the existing door. Proposed help: "After each level, contour the
  edge of the machining boundary where stock the tool can reach still stands
  above the level, so no later level cuts deeper than Depth/Pass there.
  Off by default. Costs time on rest ops with stock beyond their boundary."
- Proposed GUI label for I4: **"Contour each level"** (a checkbox, 3D Rough
  only).
- Planner side: `Adaptive3dLinking::level_contour: Option<LevelContour>`,
  where `LevelContour { link_kinematics: Option<LinkKinematics> }`. `None`
  = the option off. `compute/execute/rough_3d.rs` fills it from the config
  and `ExecutionContext::link_kinematics`. One field in one sub-struct, so
  each struct literal gains one line.

## 2. The mechanism (option on)

Active only on a 3D Rough on remaining stock (`initial_stock` is `Some`)
with a centre containment (`centre_boundary` not empty). Otherwise the
option adds nothing, and the output equals the output with the option off.

1. **Anchor (D63 port).** The level anchor and the D50 guard read
   `PriorStockTops`: the column tops of the cells the tool can reach
   (`tool_reach_cells`: inside the centre containment grown by R + cs/2),
   read after the border clear and BEFORE the boundary clear. The port is
   the code of `805368f98` (`level_anchor_z`, `rest_ceiling_keeps_doc`,
   `tool_reach_cells`, `PriorStockTops`). With the option off, the anchor
   reads as today (after the boundary clear, every cell).
2. **The band.** The band cells are the reach cells OUTSIDE the centre
   containment that hold stock in `PriorStockTops`. The band state is their
   top heights. Every emitted cut and link lowers the band tops under the
   tool disc (the D68 prototype's `lower_along`, cell reach R + cs/2).
3. **The pass.** After each level's clearing and the existing wall cleanup,
   and before the next level, one contour runs on the containment rings
   wherever the band stands above the cut:
   - the line: each containment ring, moved `polygon::UNTOLERANCED_MM`
     (10 µm, the offset library's own bound on a point inside an arc join)
     inside, so the centre clip, which uses the same polygons, keeps it;
   - the Z of each point: `max(floor, top_in_reach − DPP − tolerance)`.
     `floor` is the level, draped on the surface plus stock to leave.
     `top_in_reach` is the highest of the band state and the planner stock
     within R + cs/2. This is the D68 rule, plus the planner stock, so the
     pass never cuts more than one Depth/Pass under any stock it meets
     (this makes it safe in By Area order too, see 4);
   - a point is "standing" when the band top in reach is more than the
     simulation cell above the pass Z (the prototype used 0.05 mm, a typed
     number; I replace it with the planner cell size);
   - each run of standing points is one cut. The cuts enter through the
     existing `plan_entry` (`RapidWithFloor`, contact read) and link with
     the existing stay-down rule (`max_stay_down_distance_mm`). The order
     is nearest end first. Every cut is stamped in the planner stock, as
     all cuts are.
4. **Which levels, which ops.** Every 3D Rough strategy that ships
   (Contour Parallel, Geodesic Adaptive). Global order: after each level.
   By Area: after each level of each region. The Z rule of 3 reads the
   planner stock, so a run beside a region that is not roughed yet stays
   one Depth/Pass under that region's stock.
5. **Short-segment merge rule (derived, no typed length).** On one ring, the
   gap between two runs is fed through at its own Z when
   `machine::kinematics::retract_saves_time` (the one feed-or-retract rule,
   W6.1; Waterline uses it) says that the feed is not slower than a retract,
   a rapid and a re-entry. Its inputs are the gap path, the op's feed, the
   safe Z, `RetractDescent::Plunge { plunge_rate }` and the machine's
   `LinkKinematics`. With no machine kinematics, the runs do not merge.
   Runs on different rings do not merge (D69 §1).
6. **Walls.** The walls are the existing per-level waterline cleanup
   (`clearing.rs::waterline_cleanup`). It already runs after EVERY level in
   Global order on every 3D Rough, on walls steeper than 30°. In By Area it
   runs once at the bottom (F9: a per-region run would cut the walls of
   regions not roughed yet). **I propose no change to the walls in C4**:
   Global already contours the walls each level, and the By Area change is
   F9, a separate defect. So on a FRESH rough the option changes nothing.
   **Question for the lead:** is that the ruling's "covering walls", or do
   you want more in C4 (for example, the walls part in By Area per region
   with a cell mask, which fixes F9)?

## 3. Sentries

1. `contour_each_level_is_off_by_default_c4`: the default is `false`; a
   project without the key loads `false`; a save with the option off writes
   no key; the schema lists the field with its help;
   `set_toolpath_param contour_each_level true` sets it.
2. Byte parity and op golden: unchanged with the option off (all cases).
3. `a_rest_op_holds_one_depth_at_the_band_c4`: the D63 fixture
   (`a_rest_op_starts_one_depth_under_the_band_it_meets_d63.rs`, ported).
   - Option ON: the first planned level is at most one Depth/Pass under
     the band top, and the simulated axial engagement of the rest op is at
     most DPP + one simulation cell (the D63 rule). This is D63's known red
     (`g_restdoc_the_band_is_still_cut_deeper_than_one_depth_d63`) turned
     green. If a residual stays (D68 saw 268 mm³ up to 12 mm on Corne, cause
     not isolated), I report it with a measured bound, and I do not weaken
     the rule silently.
   - Option OFF: the output equals today's (the known red stays,
     documented as an inverted assertion with today's measured value).
4. `the_band_pass_merges_short_gaps_c4` (unit, `adaptive3d`): two runs on
   one ring with a gap below the `retract_saves_time` crossover become one
   cut; a gap above it stays two; no kinematics: two.
5. `a_fresh_rough_ignores_contour_each_level_c4`: a fresh rough with a
   boundary and the option on gives the same moves as with the option off.
6. Planted reds: the band pass turned off with the option on (sentry 3
   fails); the merge turned off (sentry 4 fails).

## 4. Predicted re-bless list

- **Option off: none.** No parity fixture, no op-golden row, no CLI test,
  and no saved-file fixture changes. The MCP wire snapshot
  (`rs_cam_viz/tests/snapshots/mcp_wire_surface.json`) lists no op field,
  so it does not change. The schema text changes through the derive only.
- Mechanical test-source edits, not re-blesses: 19 files build
  `Adaptive3dLinking { … }` from a full literal and gain
  `level_contour: None` (4 in `src/`, 15 under `tests/`).
- Option on: no shipped fixture uses it, so no re-bless. The measurements of
  sentry 3 are new.

## 5. Not in C4

- D68 option 1 (the region rule) and option 3 (refuse over DPP).
- A load check of its own for the band pass (D68 risk: the pass is a
  slot-like cut on the disc edge). The pass is at most one DPP axial and at
  most R radial; I report its measured peak radial engagement on sentry 3.
- The Corne and wanaka play jobs (D68/D69 measured them with the prototype).
  On approval I can re-measure Corne with the product code (one release run,
  about 10 min).

## 6. Size and order of work

About 500 lines in `adaptive3d/` (anchor port, band state, pass, merge),
30 in `compute/` (field, help, wiring), 19 mechanical test edits, 3 new
test files. Stop points: the anchor port with the option off (gates
unchanged), then the pass, then the merge.
