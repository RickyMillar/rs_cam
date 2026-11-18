# P41: the one-pass rule in the region grid of each level (2026-11-18)

Job: `planning/rivmap_block_job/nz_wanaka_21_lead_proposal.toml` at `7b4a65d2f` (HEAD), setup
"1 Back", on scratch copies. Positions are in the G-code frame (setup frame + (18.5, 8.5))
unless the text says otherwise.

Scratch dir (not in the repo, a restart of the machine deletes it):
`/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/ce846d39-41d4-4e44-be56-d5dd5295a423/scratchpad/p41/`.
`phases/p41/` holds the instruments of this note (`deep.py`, `sect.py`, `cover.py`, `lim.py`,
`run_meas.sh`) and `COMMIT_MSG.txt`. Run the instruments from a scratch dir that holds copies
of `p37/gapmap.py`, `p39/entries.py`, `p40/meas.py` and `p40/left.py` in those sub-folders
(`gapmap.py` writes `target.npy` beside itself), and for each run `<tag>` the folder
`out_<tag>/` and the file `out_<tag>.nc` of the CLI.

Sections 1 to 4 were written before a line of the rule was written. Section 5 and later hold
the results. The rule as built is not the rule of section 3 in three points; section 5 says
which and why. Section 0 is the summary.

## 0. Summary (after round 2): what failed, and what is not verified

Round 1 is sections 1 to 9. Round 2 (the lead rulings R1 to R8, after the operator accepted the
rule as a guard) is section 10; it changes the switch, one read and two tests. Where section 10
and an earlier section differ, section 10 is the state of the tree.

The rule holds the bar on the job: the body row of id 2 falls from 20.7 to 6.0 mm, the entry
row is 2.00 mm, the deepest simulation sample of id 2 is 6.2 mm, and the replay has no piece
deeper than 6.3 mm (170 before).

What failed, or is weaker than asked:

- R4 (the lowest cutter read over the cells where a centre can stand) holds the acceptance
  and gains nothing on the job: the G-code of setup "1 Back" is the round 1 file, byte for
  byte. The step shelf keeps 773.8 mm2 and the mech and slit pockets 1 631.5 mm2.
- R3: the D55 sentry is green with a changed fixture (a slot 7 mm wide in place of the pocket
  40 mm wide). Its non-vacuity assert is not changed. It was NOT shown that the new fixture is
  red without the D55 fix of its own package.
- R7: the depth row CAN pass a short move into tall stock. In run `a1` two samples of id 2
  read 8.00 mm in the sample file and the body row read 6.0 mm (section 10.7).
- The price of the rule is not changed: wood in the mech and slit pockets, and "Day strip slit
  V60" with 8 moves above its flute (7 before) and 10.60 mm of wood over its tip (7.44 mm).
- No source fix landed (section 6): it is the next phase (D-3, D-4).
- 23 test files outside `adaptive3d/` get one line each (`one_pass_hold: ..::Off`): the params
  literal needs the new field to compile. No assert of them is changed.
- Predictions of round 1 that were false: 2 (in part), 3, 4, 5 (the step shelf), 10, 12.

Not verified:

- The finding text of id 2 with the new sentence: the CLI prints the header line only.
- A model wall with its face on a line of cell centres, more than twice the limit tall, with
  the tangent centre on a cell centre. By reading: the cell on the face is half covered, so the
  rule can hold the tangent centre. The simulation reads the same cell the same way (P40 B1.1
  point 2). No fixture has this geometry, and the grid origin of the job was not read.
- Which rule of the depth row drops the two 8.00 mm samples (section 10.7 gives a reading).
- The generation time of id 2: one run each, not at the same machine load.
- The GUI and MCP surfaces.

## 1. The state at HEAD (measured again, 2026-11-18)

Release CLI of HEAD (`cli_head`, md5 `46c78e2d5cc663e8959d97c98c14bba5`), setup "1 Back".

- Depth rows of id 2 (`summary.json`, "the depth against the machine force budget"): body
  20.7 mm, entry 1.99 mm, cleanup 3.0 mm, link 6.0 mm. Setup `total_runtime_s` 9 186.
- Replay (`deep.py`, the instrument of P38 written again on `p37/gapmap.py`): 170 of the 13 728
  cutting pieces of id 2 lower a node by more than 6.3 mm, 512 mm of feed, 3 084 nodes, the
  deepest 20.70 mm. No piece is between 6.0 and 6.3 mm.
- Id 2: 585 s of feed, 39.70 m fed, 3 964 mm of rapids, 136 ramp entries, 3 entries that the
  model clip touched, 0 ramp nodes under the target.
- Cut below the target after the two roughs: 12.6 mm2 raw, 0.2 mm2 with the target lowered
  over one node.
- Wood above the 6 mm reach surface at the end of the setup: cavity wall band 62.1 mm2, cavity
  interior 570.4, step shelf 690.3, slot 412.6, mech and slit pockets 1 155.6 (to 20.70 mm).
  After id 2 alone: 3 321.1, 3 207.7, 690.3, 414.2, 1 158.2.

## 2. How a level builds its grid and its regions (read in the code)

- `build_material_bool_grid` (clearing.rs) is the ONE grid of a level. A cell is a TOOL-CENTRE
  cell. It is material when the stock in its own column stands above its own floor
  (`cell_has_material`: `stock top > level_floor_z(surface, leave, z_level) + 0.01`). The
  surface is the drop-cutter height of the op's cutter, so a centre in the grid never has the
  model inside its disc. With a By Area mask only the owned cells can be material. The rest
  region filter (`rest_region_filter`) then removes whole regions over its bound.
- `detect_and_order_regions` (Geodesic Adaptive): the grid, the boundary mask of the arm
  (`region_clip`), marching squares, containment, the small-region bound, the visit order.
- `clear_one_region`: the F-038 forecast, then the opening. `grown_outlines` gives the 2D
  planner the smoothed region grown by `R - m`, so its tool-centre region is the smoothed
  region inset by `m` (`PERIMETER_INSET_MARGIN_MM`, 0.25). The smoothing is an inset of 0.3 mm
  and an RDP of 0.3 mm; the measured margin to the raw boundary is one planner cell or more.
  Where the grow refuses, the perimeter sweep runs `R + m` inside the region and
  `planner_outlines` gives the region itself.
- Contour Parallel reads the same grid twice (the rings, the clean-up raster).
- `centre_boundary` (`centre_clip.rs`) clips each segment before its stamp. The wall contour
  (`waterline_cleanup`) cuts the section of the model at the level Z; it skips a contour
  inside a left piece, and with C4b it keeps only the points that `keep` accepts.
- `left_stock.rs` records each region or piece that the plan holds and does not plan
  (`LeftWhy`), and reads the outlines on the planner stock at the end of the plan.

So the planner reads two things for a region: the stock top of a cell (`stock_top_z_at`) and
the floor of a centre (`level_floor_z`). No stage reads the stock that the DISC of a centre
meets. That is the gap.

## 3. The design

### 3.1 The rule (R-41)

A tool-centre cell `c` is not a region cell of a level when the tool at `c`, at its level
floor, takes more than one Depth/Pass plus the stay-down clearance from one stock cell.

- The limit is `depth_per_pass + stay_down_clearance_mm`: the bound of the deep step
  (`StepSplit::StockDepth`, D43) and of the held links. No new constant.
- The read: for each stock cell `t` under the disc of `c`, `coverage(c, t) x (stock top(t) -
  cutter surface over t)`. The cutter surface is `level_floor_z(c)` plus the profile height at
  the distance of `t` (the table of the cut stamp, `PlannerLuts::cut`). The stock top is
  `stock_top_z_at`, the read of the material test. The coverage is the function of the stamp
  (`dexel_stock::stamping::point_cell_coverage`, the covered part of a cell by the disc).
- Why this read and not "a cell centre inside the disc": it is the quantity that the planner
  stamp and the simulation remove from a cell in one stamp (`ray_blend_above` with the
  coverage; the depth of a simulation sample is the largest of these). P40 B measured that no
  inset from the disc edge agrees with the simulation, and that this read does.
- Why the model does not count as stock: a centre of the grid has its floor from the
  drop-cutter, so no model is inside its disc above its cutter. A column that stands above
  the cutter inside the disc is stock.
- No margin of its own. The 2D plan already keeps its tool centre one planner cell or more
  inside the raw region boundary (the measured margin of `grown_outlines`; `R + m` for the
  sweep; one cell for the Contour Parallel rings). The boundary of marching squares is half a
  cell outside the last region cell. So a planned point is nearer to the region interior than
  the outermost centre that the rule accepted. The margin is the cell size and the existing
  smoothing values; no number is typed.
- The cut region of a level is then: the material cells, less the cells of another By Area
  job, less the cells that the rule holds off, less the rest region filter; then the regions
  of marching squares as today. A hole in a region is a set of held centres.

### 3.2 Where it enters

`build_material_bool_grid`, after the material test and before the rest region filter. It is
the one grid, so Geodesic Adaptive, the Contour Parallel rings and the Contour Parallel
clean-up raster all read it. The same read at a point (`OnePass::blocks`) masks the points of
the wall contour (`waterline_cleanup`): the wall contour is the one fed path that does not
come from the grid.

Cost: a cell can hold a centre off only when its top is above `z_level + limit`. With none in
reach of a material cell the rule reads nothing. The first level of a fresh rough has none.

### 3.3 Why entries, links, the order and the parity hold

- The plan gets a smaller region. Each entry, link and cut comes from `clear_one_region` as
  today. No segment is split after the plan, so no entry is added at a split.
- An entry stands inside its region; `plan_entry` reads its stock as today (P39).
- A held link reads `keep_down_excess` at `z_level + depth_per_pass + clearance` as today, so
  a link through a hole goes out as an entry.
- The rule reads the stock at the start of the level-region, as the material test does. By
  Area and Global differ only where they differ today: in the stock that the order has cut
  before the level comes.
- The planner stamps what it ships. The rule changes what is planned, not how it is stamped.

### 3.4 The record

- (As designed, with a reason `LeftWhy::OverOnePass`; round 2 removed it, section 10.4.)
  The record is cells, not an outline: each stock cell
  that held a centre off, with the lowest cutter surface that it would have met.
- `LeftStock`: `held_off_centres` (centre cells held off, summed over the levels),
  `held_off_area_mm2` (the stock cells that still stand more than the limit above that cutter
  at the end of the plan), `held_off_tallest_mm`, `held_off_at`. `LeftStock::stands` is the ONE
  rule of "this op must state it"; the adapter and the export gate read it.
- `geom.rough_left_stock` gets one sentence.

### 3.5 The three classes of P38: should the upper level have cut the stock?

Measured at HEAD on the replay (`sect.py`: the stock and the target on a line before one
piece, and the nearest pass of each level).

- Class "inside the reach of a planned region, not cut" (41 pieces, 362 mm in P38). The two
  longest deep pieces of the op are of this class: 116.6 mm at Z 11.0 and 114.5 mm at Z 13.5,
  both on the line Y 100.35 along the wall Y 104.0 (a shelf at Z 11.0, 2.4 mm wide). They
  lower 26 nodes and 1 node by more than 6.3 mm. The section at X 300 before the first one:
  the stock stands at 25.5 from Y 103.4 to 103.8 and the wall is at Y 104.0. Each level
  (19.5, 16.55, 13.5, 11.0) has a pass on the same line Y 100.35, so its disc ends at Y
  103.35. So the mechanism is: the plan keeps its centre 0.55 to 0.75 mm from a wall (the
  inset and the RDP of the smoothing, plus `m`), so a strip of that width stands at each wall
  from the top, at each level. The strip is by design (the wall contour or the finish cuts
  it). A lower level whose smoothed outline is nearer to the wall by a part of the RDP
  tolerance, or whose pass runs further along the wall, takes the edge of the strip at its
  full height. The cut is a sliver one or two nodes wide and 9 to 20 mm tall.
  Verdict: the upper level rightly left the strip (it is the margin of the plan). The lower
  level must not touch it: the rule is the answer. The source of the strip is the missing
  wall contour at each level of a By Area rest rough (P38 section 8, W4); it is not fixed
  here.
- Class "a region dropped under the small-region bound" (114 pieces, 119 mm). On a rest rough
  the top levels hold small patches of remaining stock (the cusps of the 18 mm rough), under
  the 144 mm2 bound. A lower level has a large region there and meets the patch two levels
  tall. Verdict: a planning defect. The bound was made for a terrain crumb, where no lower
  level comes. Section 6 gives what was done.
- Class "stock in no region, the strip at a wall" (33 pieces, 59 mm). A strip narrower than R
  at a wall is in no centre cell (S1 of P37). Verdict: the upper level could not see it; the
  rule is the answer, and W4 is the source fix.

### 3.6 The known cost of the rule: a step at each wall

At a wall, the raw region boundary of level 2 is no more the wall limit. It is the line where
the disc touches the strip that level 1 left, which is the pass line of level 1. The plan then
keeps its own margin (0.55 to 0.75 mm) inside that line. So each level below the first stands
about one margin further from a wall than the level above it, and the strip at the foot of a
wall grows by that step per level. This is the price of a rule in the grid: the margin of the
plan applies again at each level. Section 5 gives the measured wood.

## 4. Predictions (written before the code)

1. Id 2 depth rows: body at most 6.5 mm (20.7 today); entry at most 2.5 mm (1.99); link 6.0;
   cleanup at most 3.0.
2. Replay: fewer than 30 pieces deeper than 6.3 mm (170 today). Each one lowers a strip one
   node wide at the edge of the disc: the replay nodes are half a planner cell from the
   planner cell centres (18.5 / 0.2 = 92.5).
3. Time of id 2: between 540 and 660 s of feed (585 s today). The regions are smaller, and a
   region that the rule cuts in two gets one more entry.
4. Ramp entries of id 2: between 120 and 180 (136 today). 0 ramp nodes under the target.
5. Wood left at the end of the setup (above the 6 mm reach surface): the mech and slit pockets
   rise from 1 155.6 mm2 by 200 to 1 200 mm2 (the step of 3.6 at walls of two levels or more,
   and the patches that a lower level cut too deep today). The cavity wall band stays within
   30 mm2 of 62.1 (the rest pocket op cuts it). The slot stays 412.6 and the step shelf 690.3
   within 5 mm2. The cavity interior stays within 150 mm2 of 570.4.
6. Cut below the target after the two roughs: at most 12.6 mm2 raw and 0.2 mm2 lowered.
7. Collisions: 0, 0 and 0 failed checks on each op.
8. `geom.rough_left_stock` of id 2 states the held centres and an area above 0.
9. The lib test of the rule (the rib fixture of P38): 11.5 mm deep without the rule, at most
   3.5 mm with it.
10. Sentries. Predicted red, because their fixtures have two levels or more beside stock that
    no centre takes, and the step of 3.6 moves the lower levels:
    `adaptive3d_emission_byte_parity` (some of its cases) and the golden row
    `adaptive3d/ramp_geodesic`. Predicted green: every other adaptive3d sentry, the dressup
    sentries, core `--lib` (but the known red), the planner and simulation parity tests (the
    rule plans less; it does not change a stamp).
11. Golden cells: only rows of `adaptive3d/*` move outside the 46 known cells and the 6 P39
    cells.
12. rivmap100, the Geodesic Adaptive copy: the G-code moves (not the same bytes): levels below
    the first stand off the stock that the dropped small regions and the wall margins hold.
13. FM1: no cell moves.

## 5. The rule as built (three changes to section 3, each after a measurement)

Runs: `a1` is the rule of section 3 as written. `a2` is the rule as built. `a3` is the final
tree (the format, the lint changes and change 3 below); its G-code of setup "1 Back" and of
setup "3 Front" is the `a2` file, byte for byte (`cmp`). CLI md5 of `a3`:
`3aab2bb421267632ac3fb6cf04a01185`.

### 5.1 Change 1: a stock cell is read against the LOWEST cutter that can cover it

Run `a1` (the read of section 3.1: each centre against its own cutter): id 2 body 6.0 mm in
the simulation, and 3 pieces on the replay 8.0 mm deep (37 nodes; not a strip one node wide).

- The place: G-code (223.4 to 224.8, 100.0 to 101.4), a patch at Z 16.55 over the pocket floor
  (8.4), beside a shelf at Z 11.0. The level 8.55 cut it at 8.0 mm.
- The probe (temporary code, not in the tree): at level 8.55 the planner stock had the patch
  at 16.55 and the rule held the pocket centres (floor 8.55, 8.0 mm under the patch). It did
  not hold the shelf centres (floor 11.0, 5.55 mm under the patch: one pass). So the held set
  was a part of a disc.
- Geodesic Adaptive opens a region on its grown outline (`grown_outlines`): the smoothed region
  grown by `R - m`, which the 2D plan insets by R. That closes each gap of the region boundary
  narrower than about 2 R; the closing guard admits one cell times the boundary length (here
  10.6 mm2 of 141.8 mm2). The closing gave the held pocket centres back to the plan.
- The fix: the height of a stock cell is read against the lowest cutter surface that any centre
  of the grid can have over it (`lowest_cutter_over`). Then the centres held off one cell are a
  disc of about the tool radius around the cell, the held set is a union of such discs, and a
  closing with a smaller disc (`R - m`) cannot fill it.
- The cost: a centre that could cut such a cell at one pass (the shelf centre) is held too.
- Why the body row read 6.0 mm where the replay read 8.0 mm: measured in round 2 (section
  10.7). The simulation samples read 8.00 mm; the depth row does not state them.

### 5.2 Change 2: the rule is on only for a rough on remaining stock

Run `a1` also changed id 1 (the fresh 18 mm rough): its body row went from 10.0 to 14.5 mm
(14 626 to 14 882 moves; 3 to 8 deep pieces on the replay, 4 to 124 nodes).

- A fresh rough plans on its own cell (`tool radius / 6`: 1.5 mm for id 1). The simulation
  cell is 0.2 mm. The rule held 179 centres of level 11.0. The plan of level 11.0 was then not
  the plan of level 15.5 (at HEAD the two regions are the same polygon, so the two plans run
  the same lines). The new plan touches the wall line Y 51.34 at other places and takes the
  strip at the wall there, 14.5 mm tall. A cell of 1.5 mm cannot show a strip 0.6 mm wide.
- On remaining stock the planner stock IS the simulated stock, on the simulation cell. There
  the read is exact.
- So the rule is on only for a rough on remaining stock. Round 1 took
  `params.initial_stock.is_some()` as the switch; round 2 made it a dial that the session sets
  (section 10.1). Id 1 keeps its G-code, byte for byte.
- A fresh rough can still cut what its own upper level left. HEAD has the case: id 1 has 3
  pieces 14.5 mm deep on the replay (4 nodes). Decision D-2.

### 5.3 Change 3: a wall contour point outside the boundary clip is not read

The stored rivmap100 file (Contour Parallel on remaining stock) got one new header line
(`geom.rough_left_stock`) with the same motion: the rule had "held off" wall contour points
that the boundary clip drops in any case. `waterline_cleanup` now reads a point only when the
clip keeps it. The file is then the HEAD file, byte for byte.

### 5.4 The code

- `adaptive3d/one_pass.rs` (new): `OnePass` (`hold_off`, `blocks`, the record `HeldOff`),
  `lowest_cutter_over`, `deep_cell`.
- `clearing.rs`: `build_material_bool_grid` calls `hold_off` before the rest region filter (3
  call sites: the Geodesic regions, the Contour Parallel rings, its clean-up raster);
  `waterline_cleanup` masks its points with `blocks`. `ClearZLevelContext::one_pass`.
- `dexel_stock/mod.rs`: `TriDexelStock::disc_cell_coverage` (the coverage of the stamp, for the
  planner).
- `left_stock.rs`: `LeftStock::held_off_*`, `LeftStock::stands` (round 1 also had
  `LeftWhy::OverOnePass`; round 2 removed it). The
  finding text, the adapter and the export gate read `stands`.
- `--lib _p41` (3 tests). Red proof: with `OnePass::limit_mm` infinite,
  `a_level_region_keeps_off_what_the_levels_above_left_p41` fails with "a sample cuts 11.50 mm
  deep; one Depth/Pass and the clearance are 3.5 mm", and the By Area case of
  `every_strategy_and_order_reads_the_one_pass_grid_p41` fails the same way.

## 6. The source of the tall stock: what was done

No source fix is in the tree. The reasons, class by class:

- The margin strip at a wall ("inside the reach of a planned region", and "in no region"):
  the source fix is a wall contour at each level of each By Area region (P38 section 8, W4).
  The option exists (`level_contour`, C4b); P37 arm A measured it on this job: +3 327 s, two
  walls of four, 144 mm2 more below the target. It is not a change for this package.
- The region under the small-region bound: on a rest rough a patch under the bound often
  holds no geodesic field and no 15 mm of forecast cut, so the F-038 forecast or the "no
  field" rule drops it after the bound is lifted. A lift of the bound alone moves terrain
  roughs (a narrow pit deeper than one level gets an entry at each level) and does not cut
  the patch. The fix is the planner for a narrow region of W4.
- So the guard carries all three classes, and section 7 states the wood that it leaves.

## 7. Measured (setup "1 Back", the proposal file; HEAD against the final tree)

| | Before (HEAD) | After |
|---|---|---|
| Deepest cut of "Back rest 6 mm" in its clearing passes (stated 6 mm per pass) | 20.7 mm | 6.0 mm |
| The same in its ramp entries | 1.99 mm | 2.00 mm |
| The same in its links, and in its wall cleanup | 6.0 mm, 3.0 mm | 6.0 mm, 3.0 mm |
| Pieces of that op deeper than 6.3 mm on the replay of the G-code | 170 (512 mm of feed, to 20.70 mm) | 0 (the deepest 6.23 mm) |
| Time of "Back rest 6 mm" (feed at the emitted feeds) | 585 s, 39.70 m fed, 3 964 mm of rapids | 522 s, 35.45 m, 3 461 mm |
| Its ramp entries; entries that the model clip touched | 136; 3 | 118; 3 |
| Time of the setup (`total_runtime_s`) | 9 186 s | 9 113 s |
| "Back bulk rough 18 mm" (fresh) | body 10.0 mm, 14 626 moves | the same G-code, byte for byte |
| "Cavity rest pocket 6 mm" | 231 s, 5 969 moves, body 2.25 mm | 238 s, 6 321 moves, body 2.25 mm |
| Cut below the target after the two roughs: raw; target lowered one node | 12.6 mm2; 0.2 mm2 (5 nodes, to 1.00 mm) | 12.1 mm2; 0.2 mm2 (4 nodes, to 0.66 mm) |
| Ramp nodes of id 2 under the target | 0 | 0 |
| Collisions, rapid collisions, failed checks (each op) | 0, 0, 0 | 0, 0, 0 |
| Export warning lines | 24 | the same 24 |
| Wood left at the end, cavity wall band | 62.1 mm2 | 63.7 mm2 |
| Wood left, cavity interior | 570.4 mm2, to 6.39 mm | 570.2 mm2 |
| Wood left, step shelf | 690.3 mm2, 5.0 mm | 773.8 mm2, 5.0 mm |
| Wood left, the 8.6 mm slot | 412.6 mm2, 21.0 mm | 412.6 mm2, 21.0 mm |
| Wood left, mech and slit pockets | 1 155.6 mm2, to 20.70 mm, 13 684 mm3 | 1 631.5 mm2, to 20.70 mm, 17 228 mm3 |
| Flute contact, "Pin drill, flip pins" | 5 moves | 5 moves |
| Flute contact, "Day strip slit V60" | 7 moves, 7.44 mm of wood over the tip | 8 moves, 10.60 mm |

In plain words:

- The 6 mm rest rough no longer drives a clearing pass through wood taller than its stated
  pass. Where a pass would meet such wood, the tool stays off it.
- The op is 63 s shorter, because it cuts less.
- The price is wood. After the op, 1 022 mm2 more stands than before (5 218 mm3). 718 mm2 of
  it is still above the target at the end of the setup, most of it in the mech and slit
  pockets, at the foot of their walls: 488 mm2 of the 1 022 mm2 is within 1.6 mm of the foot of
  a wall. Before, a strip about 0.6 mm wide stood at each such wall from the top; now each
  level under the first stands 0.2 to 0.4 mm further from the wall, so the strip is about
  1.2 mm wide at the foot (measured at G-code X 300, wall Y 104.0: 0.6 mm at 25.5 before; 0.6 mm
  at 25.5, 0.4 mm at 19.5 and 0.2 mm at 13.5 after).
- No later op of the setup cuts the mech and slit pocket walls. That was true before.
- The step shelf holds 83.4 mm2 more, in one patch (G-code X 336 to 351, Y 110.8 to 117.8, 5 mm
  of stock on the shelf at 20.5, beside the cavity corner). At HEAD a lower level cut it from a
  shelf centre, one pass deep. Now the cells of the patch are read against the lowest cutter
  that can cover them, which is a cavity centre at Z 16.55 (8.95 mm under the stock top), so
  no centre takes them. This is the cost of change 1 (section 5.1).

### 7.1 Which later op meets the wood that the rule leaves (`p40/left.py head a3`)

| Later op | Area of that wood that it cuts | Tallest column that it takes there | Its depth row (before, after) |
|---|---|---|---|
| Cavity rest pocket 6 mm | 285.7 mm2 | 9.00 mm (stock 25.5 to 16.5), in passes of 2.25 mm | body 2.25, 2.25 |
| River channel 6 mm, pass 7 of 7 | 42.4 mm2 | 4.83 mm | body 1.84, 1.84 |
| Lake outline channel 6 mm | 3.5 mm2 | 4.23 mm | body 1.59, 1.59 |
| Lake pocket 2 mm, 11 lakes | 8.5 mm2 | 6.41 mm | body 0.66, 0.66 |
| ANTENNA_D5.00 pocket 2 mm | 0.1 mm2 | 4.44 mm | body 0.50, 0.50 |
| V rivers and lakes | 13.0 mm2 | 6.52 mm | body 2.24, 2.24 |
| Day strip slit V60 | 7.2 mm2 | 9.33 mm at G-code (345.8, 95.0), stock 13.5 to 4.17 | body 4.60, 4.60 |

- No depth row of a later op moves. The simulation reads each of those cuts inside the pass of
  its op.
- The slit V-bit is the one op that is worse: one more move with wood above its 5.5 mm flute
  (8, was 7), and the most wood over its tip is 10.60 mm (was 7.44 mm). The rest rough used to
  cut that wood too deep; now it stands at 13.5 at the end of the slit.

### 7.2 Setup "3 Front" (id 21, "Front 3D rough": remaining stock, By Area, 5.532 mm per pass)

| | Before | After |
|---|---|---|
| Id 21 body row | 6.41 mm | 4.31 mm |
| Id 21 entry row, link row | 1.56 mm, 0.48 mm | 1.56 mm, 0.48 mm |
| Id 21 time, fed length, rapids, moves | 870 s, 55.90 m, 12 734 mm, 38 790 | 868 s, 55.78 m, 12 464 mm, 38 903 |
| Id 20 (fresh) and the scallop finish (id 23): rows and moves | | not changed |
| Setup `total_runtime_s`; collisions | 8 756 s; 0, 0, 0 | 8 750 s; 0, 0, 0 |

### 7.3 rivmap100

- `planning/fixtures/rivmap100/rivmap100_live_0925.toml` (Contour Parallel, remaining stock):
  the same G-code, byte for byte.
- The Geodesic Adaptive copy (a scratch copy with `clearing_strategy = "geodesic_adaptive"`;
  no such file is in `planning/fixtures/`): the same G-code, byte for byte (6 780 moves, body
  8.0 mm, `total_runtime_s` 1 229). Its rough reads remaining stock, so the rule is on; no
  stock stands more than 8.5 mm above a cutter.

### 7.4 Predicted against measured

| # | Prediction | Result |
|---|---|---|
| 1 | id 2 rows: body at most 6.5, entry at most 2.5, link 6.0, cleanup at most 3.0 | TRUE: 6.0, 2.00, 6.0, 3.0 |
| 2 | fewer than 30 deep pieces on the replay, each a strip one node wide | TRUE for the count (0). FALSE as a reading of the first build: run `a1` had 3 pieces that were NOT a one-node strip (section 5.1). The half-cell offset explains no piece of the final run; there is none. |
| 3 | time of id 2 between 540 and 660 s | FALSE: 522 s. The op cuts 4.25 m less. |
| 4 | 120 to 180 ramp entries; 0 ramp nodes under the target | FALSE for the entries (118); true for the nodes (0) |
| 5 | mech and slit pockets +200 to +1 200 mm2; wall band within 30; slot and shelf within 5; interior within 150 | TRUE for the pockets (+475.9), the band (+1.6), the slot (0.0), the interior (-0.2). FALSE for the step shelf: +83.5 mm2. |
| 6 | cut below the target at most 12.6 raw, 0.2 lowered | TRUE: 12.1, 0.2 |
| 7 | collisions 0, 0, 0 | TRUE |
| 8 | the finding of id 2 states the held centres | NOT VERIFIED: the CLI prints the header line only, and the line was there before (the dropped regions). The log has 45 "One-pass rule" lines for id 2, 79 545 held centre cells summed over the level grids. |
| 9 | lib test: 11.5 mm without the rule, at most 3.5 mm with it | TRUE |
| 10 | sentries | section 8 |
| 11 | golden cells | TRUE: no cell outside the 46 known cells and the 6 P39 cells |
| 12 | rivmap100 Geodesic copy moves | FALSE: the same bytes |
| 13 | FM1 | section 8 |

## 8. Gates of round 1 (section 10.6 holds the gates of the final tree)

Tree `a3` (the final tree but one line of the finding text, see the end of this section).
Logs: the scratch dir, `gate_fin_*.log`.

| Gate | Result |
|---|---|
| core `--lib` | 2 847 passed, 2 failed, 13 ignored. Known: `feeds::envelope::tests::the_envelope_meets_its_time_budget`. NEW: `adaptive3d::tests::a_small_region_the_plan_drops_is_stated_p37`. |
| adaptive3d sentries (37 binaries, `test-support,heavy-tests`, `--no-fail-fast`) | 73 passed, 2 failed, 7 ignored. NEW reds: `a_rest_entry_does_not_peck_through_cleared_air_d55` and `adaptive3d_boundary_clear_parity` (`boundary_clear_parity_geodesic_adaptive`). |
| dressup sentries (19 binaries) | 119 passed, 5 ignored |
| other core sentries (11 binaries: the findings join, K6, G-ROUGHCOV, F140 and others) | 75 passed |
| op golden | red with 52 cells: the 46 known cells and the 6 P39 cells of `adaptive3d/ramp_geodesic`, each with its part A value. No other cell. Not blessed. |
| FM1 (`feeds_matrix_instrument_fm1`, `heavy-tests`) | 2 passed; no cell moves |
| `rs_cam_cli` (17 binaries) | 113 passed |
| `rs_cam_mcp` | 39 passed |
| viz `--lib` (`--test-threads 2`, no GUI process) | 705 passed, 2 ignored |
| full clippy (`--workspace --all-targets`, the three core features, `-D warnings`) | clean |
| `rustfmt --check` on each changed file | clean; no other file was formatted |

Prediction 10 against this: FALSE in both directions. `adaptive3d_emission_byte_parity` and
the golden row stay green (their fixtures plan on fresh stock, where the rule is off). The D55
sentry and the boundary clear parity sentry, predicted green, are red.

After the gate set one line changed: the new sentence of the finding prints only when held
stock still stands (`held_off_area_mm2 > 0.0`; before, on `held_off_centres > 0`, it could
print "0.0 mm2" and X0.00 Y0.00). On that last tree (CLI md5
`9a2faf53ce954280d235e121993de40a`):

- `--lib -- _p41 _p37 from_generation toolpath_stats`: 11 passed, 1 failed (the P37 test of
  section 0);
- full clippy: clean; `rustfmt --check` on the file: clean;
- the G-code of setup "1 Back", of the rivmap100 file and of its Geodesic Adaptive copy: the
  files of tree `a3`, byte for byte (`cmp`).

The other gates were not run again on the last tree. Setup "3 Front" was not run again on it.
During the work one experiment edited `one_pass.rs` after the gate set (the cells with their
centre inside the disc only, section 0); the file was then put back, and its md5 is the md5
of the gate tree (`0ec97e3a5883af24abee9e294e86eea3`).

## 9. Decisions for the lead after round 1 (section 10.8 holds the state after round 2)

- D-1 (the three reds, section 0). Each needs a ruling, not a bless by an agent:
  the P37 lib test states a figure that the rule moves to another field; the D55 fixture
  needs a deep entry column that is not corner stock; the parity pin needs its cause first.
  The first thing to try for the parity red: let `lowest_cutter_over` read only the centres
  that the boundary clip allows (today it reads every grid cell, so band stock that no allowed
  centre can cover is held at each level).
- D-2 (fresh stock). The rule is off on fresh stock, because the planner cell there is not
  the simulation cell (section 5.2). The choices: leave it (a fresh rough keeps its bytes and
  its sentries); or plan a fresh rough on the simulation cell; or make the lower level of a
  fresh rough keep the pass lines of the level above at a wall.
- D-3 (the wood). The rule leaves 476 mm2 more in the mech and slit pockets and a step of 0.2
  to 0.4 mm per level at the foot of each wall deeper than one pass. The fix at the source is
  W4 of P38 (a wall contour at each level of each By Area region, and a planner for a narrow
  region). Until then the proposal needs an op for the mech and slit pocket walls (D-5 of P37);
  it needed one before.
- D-4 (the slit V-bit). "Day strip slit V60" meets 10.60 mm of wood over its tip at the end of
  the slit (7.44 mm before). The flute contact finding names it; it is advisory. The op before
  it that could clear that wood is the missing op of D-3.
- D-5 (the instrument). The body row of the simulation read 6.0 mm in run `a1` where the replay
  read 8.0 mm on three short pieces (section 5.1). If the reading of the code is right, the
  depth row can pass a short move into tall stock. A run with `--sim-artifact` on `cli_a1`
  (kept in the scratch dir) settles it.
- D-6 (the lowest cutter). The rule holds a centre that could cut a cell at one pass when
  another centre could cut it deeper (the step shelf patch, 83 mm2). A read against each
  centre's own cutter is exact, and the closing of `grown_outlines` then gives held centres
  back (section 5.1). To get the exact read the closing guard must know the held cells.
- D-7 (`LeftWhy::OverOnePass`). It is a label: no code constructs it, because the record of
  the rule is stock cells and not an outline. Remove it or keep it as the name of the reason.

## 10. Round 2: the lead rulings R1 to R8 (2026-11-18)

The operator accepted the rule as a guard ("Guard now, wall passes next"). This section is the
state of the tree. Final CLI md5 `8a009e62ff75c73522940692e3265490` (runs `b1`, `b2`).

### 10.1 R1: the switch is a dial, not `initial_stock`

- `Adaptive3dLinking::one_pass_hold` (`OnePassHold`: `Off`, the default, or `RemainingStock`),
  beside `rest_region_filter`. The session adapter (`compute/execute/rough_3d.rs`) sets
  `RemainingStock` when the op has a seed stock (`inputs.seed_stock`), the line under the rest
  region filter. A caller that builds the params directly and does not set the dial keeps the
  plan without the rule.
- Why `initial_stock.is_some()` was wrong: `adaptive3d_boundary_clear_parity` seeds a fresh
  uniform block on the cell `TOOL_RADIUS / 6`. That is the fresh-stock case of section 5.2.
- The params literal has no default, so each file that builds one gets the line
  `one_pass_hold: ..::OnePassHold::Off` (23 test files, `path.rs` and `tests.rs`). No assert of
  those files is changed.
- Green from R1 alone, with no edit of the test:
  - `--lib a_small_region_the_plan_drops_is_stated_p37` (it builds its params directly);
  - `adaptive3d_boundary_clear_parity` (`boundary_clear_parity_geodesic_adaptive`).
- Not green from R1: the D55 sentry (it runs a session, so the dial is on). R2 was not needed.
- The `_p41` tests set the dial. New: `the_one_pass_dial_off_keeps_the_plan_without_the_rule_p41`
  (dial off: no centre held; dial on: centres held and another plan).

### 10.2 R3: the D55 fixture

- The fixture was a pocket 60 x 40 mm. Its one deep entry site was a corner where op 0 (Contour
  Parallel, 6 mm tool) left real stock 6 mm tall, two passes (section 0 of round 1: the dump).
  So the deep column of that site was not air, and the rule holds the tool off it.
- The fixture is now a slot 60 x 7 mm (`POCKET_HALF_Y` 20.0 to 3.5) for the same 6 mm tool. A
  centre of the slot stands beside the model wall on both sides, so the rapid floor of its
  entry is the wall top: the measured site is 1 stroke from Z 10.5 to 4.0, 6.5 mm, more than
  one Depth/Pass (3 mm). The rule is on (a session) and does not hold that site, so the stock
  under the disc is at most one pass: the column is air beside the model.
- The non-vacuity assert and the stroke assert are not changed. A slot of 8 mm gave a column of
  3.5 mm (it also passes); 7 mm was taken because its column is two passes.
- NOT shown: that the new fixture is red without the D55 fix (the peck ladder from the rapid
  floor). The old code was not built.

### 10.3 R4: the lowest cutter reads only the cells where a centre can stand

- `lowest_cutter_over` now reads a cell only when it is a material cell of the level grid as
  it came in (before the hold-off) and inside the boundary clip where the op has one. The
  hold-off collects its cells and removes them after the loop, so the read sees one grid.
- Result on the job: the acceptance holds (0 pieces deeper than 6.3 mm on the replay, deepest
  6.23 mm; body row 6.0 mm). It gains nothing: the G-code of setup "1 Back" is the round 1
  file, byte for byte, and the rule holds the same 79 545 centre cells. The step shelf keeps
  773.8 mm2 and the mech and slit pockets 1 631.5 mm2.
- Why: the cavity centre that reads the shelf patch 8.95 mm deep is itself a material cell of
  that level, inside the clip. So the patch is still read against it.
- The change is kept (the ruling: keep it if it holds). It was not measured on a job where it
  changes a plan.

### 10.4 R5 and R6

- `LeftWhy::OverOnePass` is removed. The record of the rule is stock cells
  (`LeftStock::held_off_*`), with no `LeftWhy`.
- `--lib the_wall_contour_mask_holds_a_point_off_tall_stock_p41`: a point with a strip 9 mm
  tall inside its disc is masked and recorded; the same point with a strip 3 mm tall is not;
  the tall strip 6 mm from the centre does not mask; with the rule off nothing is masked. Red
  proof: with `blocks` forced to `false` the test fails at "9 mm of stock above the cutter
  inside the disc: the point is masked".

### 10.5 The measures again (HEAD against the final tree)

Setup "1 Back": the table of section 7 holds line for line (the G-code is the same file).
Checked again on the final build: body 6.0, entry 2.00, link 6.0, cleanup 3.0 mm; 0 deep pieces
on the replay; 522 s; 118 entries; below the target 12.1 mm2 raw and 0.2 mm2 lowered; 0
collisions, 0 rapid collisions, 0 failed checks; id 1 the same bytes as HEAD; wood at the end:
wall band 63.7, interior 570.2, step shelf 773.8, slot 412.6, mech and slit pockets
1 631.5 mm2.

New in round 2, from the sample file of the final build (`--sim-artifact`, ops 0 to 2): the
deepest sample of id 2 is 6.2 mm in a clearing cut, 6.0 mm in a link and 2.0 mm in a ramp
entry; 0 of its 163 392 samples are over 6.5 mm.

Setup "3 Front": the same G-code as round 1. Id 21 body 6.41 to 4.31 mm, entry 1.56 mm, 870 to
868 s; 0 collisions.

rivmap100: the stored file and the Geodesic Adaptive scratch copy give the HEAD G-code, byte
for byte.

### 10.6 Gates on the final tree

Logs: the scratch dir, `gate_b2_*.log`. No red is new against HEAD.

| Gate | Result |
|---|---|
| core `--lib` | 2 850 passed, 1 failed, 13 ignored. The red is the known one, `feeds::envelope::tests::the_envelope_meets_its_time_budget`. |
| adaptive3d sentries (37 binaries, `test-support,heavy-tests`, `--no-fail-fast`) | 75 passed, 0 failed, 7 ignored |
| dressup sentries (19 binaries) | 119 passed, 5 ignored |
| other core sentries (11 binaries) | first run: 74 passed, 1 failed (`folder_maps_name_real_files_f140`: the new text of `adaptive3d/CLAUDE.md` named `rough_3d.rs`, a file of another folder). The text was corrected (no code) and the sentry ran again: 4 passed. |
| op golden | red with 52 cells: the 46 known cells and the 6 P39 cells of `adaptive3d/ramp_geodesic`, the cells of round 1 line for line. Not blessed. |
| FM1 (`feeds_matrix_instrument_fm1`, `heavy-tests`) | 2 passed |
| `rs_cam_cli` (17 binaries) | 113 passed |
| `rs_cam_mcp` | 39 passed |
| viz `--lib` (`--test-threads 2`, no `rs_cam_gui` process) | 705 passed, 2 ignored |
| full clippy (`--workspace --all-targets`, the three core features, `-D warnings`) | clean |
| `rustfmt --check` on each changed `.rs` file | clean; no other file was formatted |

After the gate set only `adaptive3d/CLAUDE.md` changed (the correction above). The release CLI
in `target/release` has the md5 of the measured build.

### 10.7 R7: the depth row can pass a short move into tall stock (measured)

`cli_a1` (the first build, section 5.1) with `--sim-artifact`, ops 0 to 2. The sample file
holds 164 156 samples of id 2.

- Two samples of id 2 read `axial_engagement_mm` 8.00: move 4645 at (205.767, 88.614, 8.55)
  and move 4654 at (206.933, 89.567, 8.55), setup frame. Both are `clearing_cut`, not in a
  transit span, on the short pieces that the replay found (0.3 to 1.5 mm long). So the
  simulation DOES read the cut; the reading of the code in section 5.1 was wrong.
- The body row of the same run reads 6.0 mm (bound 3.197 mm, margin -0.877, 55 059 of 164 156
  samples contributing).
- So the depth row is not the deepest sample of its group. By reading of the row: its margin is
  `(bound - value) / bound` and the bound moves with the sample, so the row is probably the
  sample with the worst margin, and a deeper sample with a higher bound of its own does not
  show. Which rule drops the two samples was not traced.
- The consequence for this package: "body row at most 6.5 mm" does not prove that no sample is
  deeper. The final build was read in the sample file too (10.5), and on the replay.
- No fix here (the ruling). It is a defect of the instrument: the row named "depth" should
  state the deepest sample, or a second row should.

### 10.8 The decisions after round 2

- D-1 is closed (the three reds are green; see the caveat of R3).
- D-2 stays: the rule is off on fresh stock.
- D-3 and D-4 are the next phase: the wall contour at each level and the planner for a narrow
  region. The slit V-bit regression (10.60 mm of wood over its tip) waits for it.
- D-5 is answered (10.7); the fix of the depth row is a new item for the lead.
- D-6: R4 was tried and gains nothing on this job (10.3). The exact per-centre read still needs
  the closing guard of `grown_outlines` to know the held cells.
- D-7 is done (R5).
- New, D-8: the D55 sentry with its new fixture was not shown red without the D55 fix.
