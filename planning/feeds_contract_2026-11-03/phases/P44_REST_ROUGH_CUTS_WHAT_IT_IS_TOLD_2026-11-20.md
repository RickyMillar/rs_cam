# P44: a 3D Rough cuts each place where its tool fits, or names the place (2026-11-20)

Status: CLOSED, not landed (2026-11-21). Two product builds were measured. Bar 2 fails by 4
replay pieces, so the tree is back at HEAD and the patch is in `p44/not_landed/`.

Update, P44b (2026-11-21, section 10): the product form of build 3 (no strip pass) is applied
on the working tree and is not committed. Its measure bars hold. The lead ruled on D-10 to
D-16; round 2 (10.10) holds those rulings and its gates. Sections 0 to 9 are the record of
P44 as it closed; section 10 is the current state, and 10.0 is its summary.

## 0. What failed and what is not verified

RESULT: NOT LANDED. The tree is at HEAD. Two patches are in `p44/not_landed/`:
`p44_build2.patch` (H1 to H5; bar 2 fails) and `p44_build3_no_strip_pass.patch` (the same
with the strip pass H4 off; bar 2 HOLDS; section 6.6). Build 3 is a measure form (the strip
pass is behind the env `P44_STRIP_PASS`); it is not a product form and has no gate run.

- FAILED, bar 2 (the reason for the revert). The corrected replay class of id 2
  (`p43/deepc.py`, bar 6.5 mm, a node 0.1 mm or more inside the disc edge) has 4 pieces and
  11 nodes in build 2 (deepest 8.50 mm; the most inside node 0.199 mm). HEAD has 0. Build 1
  had 7 pieces. The depth ROWS of id 2 hold (body 6.000, deepest 6.130; cleanup deepest 6.320;
  entry 1.885). Each piece is a contour move of the strip pass where a node at most one planner
  cell (0.2 mm) inside the disc edge stood taller than the planner stock said (section 6.3).
  All 4 pieces are moves of the strip pass (moves 17 549 to 17 599 of id 2; the strip pass
  starts at the cleanup marker of move 17 291). Build 3, with no strip pass, has 0 pieces.
- FAILED, bar 1 for places 1 to 4 (aim: under 10 mm2 each). Build 2 after id 2: antenna slot
  59.8, control relief 13.6, slit slot 121.8, ESP relief 23.8 mm2. Places 1 to 3 are cut to
  the floor; the wood that stays is the rim of the plan margin at each wall (0.2 to 0.4 mm
  wide; D19: one planner cell plus the cut tolerance).
- FAILED in build 1 and fixed in build 2: the region contour on a fresh block cut 10.33 mm deep
  (id 1, limit 4.666). In build 2 the region contour runs only on remaining stock.
- Bar 3 does not hold at HEAD: `p43/gouge.py` on "Back rest" gives 8 nodes 0.1 mm or more
  inside the disc edge under the target at HEAD (deepest 3.60 mm, at the 8.55 to 12.45 step).
  Build 2 has the same 8 nodes. It adds none.
- NOT VERIFIED: the full gate list of bar 9. Run on the patched tree: `--lib adaptive3d::
  _p37 _p44` (93 pass, 0 red), rustfmt check of the core crate (clean), full clippy (section
  7). NOT run: the adaptive3d and dressup sentries, the other core sentries, the whole core
  `--lib`, FM1, the op golden, `rs_cam_cli`, `rs_cam_mcp`, viz `--lib`. Reason: the disk has
  4.3 GB free (`/` is at 100 %), each test build takes 1 to 4 GB, and I may not delete under
  `target/`. The golden and the byte pins were not read.
- NOT VERIFIED: that each `_p44` test is red at HEAD. Each test has a control inside it
  (`REGION_CONTOUR_OFF_FOR_CONTROL`: no region contour, no strip pass). No test has a control
  for H1 (the small-region bound) or for H2 (the forecast frame) alone.
- rivmap100 (`planning/fixtures/rivmap100/rivmap100_live_0925.toml`, read in place, build 2):
  the motion is the same as HEAD, line for line (9 108 moves, 1 663.2 s, body 8.0 mm). The
  G-code differs by ONE header line: the new finding fires on this Contour Parallel rough
  ("stock still stands at 293 places (1 424.0 mm2)", all "no rule dropped it"). See D-9.
- NOT DONE: `COMMIT_MSG.txt` and a product form of build 3. A product form must delete
  `strip_pass`, `grow_open_side`, `LeftWhy::NoLine` and the test
  `a_strip_beside_a_wall_with_no_region_is_cut_p44`, then pass the gates. The disk has 3.5 GB
  free, so the gate builds cannot run.
- OPEN, older than P44: with a wall pass on, the lib parity fixture
  `planner_sim_dexel_parity_geodesic_adaptive_world_stock_declared` shows the simulation
  removing more than the planner stamp (P43 wall pass alone, `OnePassHold::FreshStock`: 47
  cells, 24 interior, up to 12.9 mm; it passes only by its count bar). The cuts of `emit_runs`
  (C4, P42, P43 and the P44 contours) all go through that door. I did not find the cause.
- The instrument `p37/gapmap.py report` reads a hole of the model as the stock top
  (`reach`: a node with no triangle is `STOCK_TOP`). So it put the control relief (158 mm2) and
  the slit slot (1 782 mm2) in "a 6 mm tool cannot reach". The product reads a hole as the
  bounding-box floor. `p44/places.py` (`reach6h`) reads an enclosed hole as far under the part.
  All "fits" areas in this note use `reach6h`. The older notes (P37 to P43) used `reach6`.
- The cause classes come from a probe of the planner grid (per level call, per cell). A node of
  the replay takes the class of the tool-centre cell under it, or of a classed cell within one
  tool radius. This join is geometric; it is not a trace of single moves.
- The reason of a place in the new finding text is the drop record on its cells. A rim of the
  plan margin beside a recorded piece reads "the field does not fit". The position, the area
  and the height are right; the reason of a rim is not exact.

## 1. The measure before (HEAD `401634ee4`, the job with id 41 off, setup "1 Back")

Run: `p44/run_cli.sh head head` (CLI built from `4586e9bb7`, the product code of HEAD).
Replay: `p37/gapmap.py replay`. Places: `p44/places.py` (output `p44/places_head.txt`).
9 682.9 s, 0 collisions, 0 rapid collisions, 0 failed checks.

Wood more than 0.3 mm over the target (mm2). "fits": a 6 mm flat tool can cut it and not go
under the target.

| Place | after id 2: over | fits | corner | end of setup: over | fits | corner |
|---|---|---|---|---|---|---|
| 1 antenna slot | 415.5 | 412.8 | 2.7 | 415.5 | 412.6 | 3.0 |
| 2 control relief | 162.1 | 158.2 | 3.8 | 162.1 | 158.2 | 3.8 |
| 3 slit slot | 1 784.6 | 1 781.7 | 2.9 | 242.3 | 239.4 | 2.9 |
| 4 ESP relief | 60.3 | 59.4 | 0.9 | 60.3 | 59.4 | 0.9 |
| 5 step shelf | 797.9 | 795.1 | 2.8 | 688.7 | 684.5 | 4.2 |
| 6 board pocket | 1 002.4 | 957.4 | 45.1 | 962.4 | 953.4 | 9.1 |
| 7 cavity | 6 646.0 | 5 016.1 | 1 629.9 | 2 210.8 | 628.9 | 1 582.0 |
| other | 101.8 | 22.0 | 79.8 | 101.8 | 22.0 | 79.8 |
| total | 10 961.0 | 9 193.9 | 1 767.0 | 4 834.4 | 3 149.6 | 1 684.8 |

The 2.5D toolpaths after id 2 cut 4 387 mm2 of the 5 016 mm2 that id 2 leaves in the cavity
where its tool fits, and the V-bit cuts 1 542 mm2 of the slit slot. The bars of this phase read
the column "after id 2: fits": it is the work of the two 3D roughs alone.

## 2. Step 1: the cause per place

Probe: `p44/probe1.patch` (prints only; env `P44PROBE`). Per call of
`detect_and_order_regions` it writes one byte per planner cell (material cell, owned by the By
Area job, after the one-pass hold, after the rest region filter, after the region clip), one
line per region (kept or under the bound), per forecast, per planned region, and the planner
stock at the end of the op. Readers: `p44/probe_read.py` (tool-centre cells),
`p44/cause_nodes.py` (replay nodes). Raw log: `p44/probe1_log.txt`.

Levels of id 2: 19.5, 16.55, 13.5, 8.55, 7.5, 1.5, 0.5 (16.55 and 8.55 are Detect Flat
shelves; 0.5 is the bounding-box floor through the holes). 27 By Area jobs, 64 level calls.
Levels of id 1: 21.334, 17.168, 13.002, 11.0 (Global).

### 2.1 Id 2 "Back rest 6 mm", by mechanism (replay nodes where the 6 mm tool fits, mm2)

| Mechanism | Function that refuses | antenna | control | slit | ESP | shelf | board | cavity | all |
|---|---|---|---|---|---|---|---|---|---|
| A. small-region bound | `detect_and_order_regions`: region area under `(2 D)^2 x min(DPP / R, 1)` = 144 mm2 | 0 | 0 | 0 | 0 | 399.8 | 5.0 | 1 369.0 | 1 782.6 |
| B. F-038 forecast | `clear_one_region`: forecast under `min_region_cut_length_mm` = 15 | 0 | 0 | 1 779.4 | 0 | 0 | 0 | 1 048.4 | 2 827.7 |
| C. one-pass hold, after A at a level above | `OnePass::hold_off` | 412.6 | 158.2 | 2.3 | 0 | 4.6 | 154.4 | 321.1 | 1 053.3 |
| D. strip at a wall: no tool-centre cell holds material | `cell_has_material` (the level grid reads the stock at the centre cell only) | 0.2 | 0 | 0 | 59.4 | 380.3 | 799.6 | 2 178.2 | 3 430.8 |
| E. planned and cut, stock still stands | (the plan margin, D19) | 0 | 0 | 0 | 0 | 0 | 0 | 99.5 | 99.5 |
| rest region filter, region clip, By Area ownership, no field | | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| total | | 412.8 | 158.2 | 1 781.7 | 59.4 | 784.8 | 959.0 | 5 016.1 | 9 193.9 |

All of this is "the tool fits and the plan dropped it". The part "the tool cannot fit" is the
column "corner" of section 1 (1 767 mm2), and no mechanism of the plan is its cause.

The numbers per place:

1. Antenna slot (9 x 48 mm, target 4.5, stock 25.5). By Area job 3. The region is the slot
   less the tool radius on each side: 2.8 x 42.6 mm, 118.3 mm2 of cells. Level 19.5: under the
   bound 144 mm2, dropped (A). Levels 16.55 to 1.5: the stock stands more than 6.5 mm over the
   cutter, so the one-pass hold removes all 2 958 cells (C). No level plans it.
2. Control relief (15.4 x 14.6 mm ring, target 4.5, a through hole in the centre). Levels 19.5
   to 7.5: part of the board pocket region, cut to 7.5. Level 1.5: a region of its own, 9.2 x
   8.4 mm, 77.3 mm2, under 144 mm2 (A). Level 0.5: the cutter in the hole stands at 0.5, the
   stock at 7.5 is 7.0 mm over it, held (C).
3. Slit slot (7 mm wide, target 4.83). Levels down to 7.5: part of the board pocket region.
   Levels 1.5 and 0.5: a region of its own, 291.6 x 1.0 mm, 291.6 mm2 (over the bound). The
   forecast is 0.0 mm against 15 mm (B): the forecast insets the region by the tool radius plus
   0.25 mm, and the region is ALREADY the set of tool centres.
4. ESP relief: 59.4 mm2, strips at its walls (D). 31.5 mm2 stand at the stock top 25.5.
5. Step shelf: 9 By Area jobs of 9 to 1 801 cells (0.4 to 72 mm2), each under the bound at
   level 19.5 (A: 399.8 mm2); strips at the walls (D: 380.3 mm2).
6. Board pocket walls and corners: strips (D: 799.6 mm2) and the one-pass hold beside them (C).
7. Cavity: A 1 369.0 (15 regions at level 7.5 and 8.55, 0.1 to 48.7 mm2), B 1 048.4 (two
   regions of 244.6 and 183.9 mm2 with a forecast of 3.43 and 0.0 mm: rings and strips
   narrower than 6.5 mm), C 321.1, D 2 178.2.

Mechanism D by height over the reach surface: 0.3 to 1 mm 240 mm2; 1 to 3 mm 388 mm2; 3 to
6.5 mm 1 116 mm2; over 6.5 mm 1 687 mm2. Id 2 lowered 280 mm2 of it and did not touch
3 151 mm2. It is the wood that the 18 mm tool leaves at each wall and inside corner (the
corner of a 9 mm radius reaches 2.64 mm from each wall; the tool centre of the 6 mm tool
cannot stand over a strip narrower than 3 mm). The P42 wall pass cuts such a strip only along
a region that the level plans.

### 2.2 Id 1 "Back bulk rough 18 mm" (replay nodes over the reach surface of the 18 mm tool, not under its bottom 11.0, mm2)

| Mechanism | slit | ESP | shelf | board | cavity | all |
|---|---|---|---|---|---|---|
| A. small-region bound (599.9 mm2 at each level; 5 regions at level 11.0, 7.9 to 156.4 mm2) | 0 | 0 | 0 | 0 | 2 381.6 | 2 381.6 |
| A2. contour of one cell area or less (`single_loops` filter) | 0 | 0 | 0 | 0 | 292.5 | 292.5 |
| D. strip at a wall | 0 | 56.8 | 674.2 | 605.7 | 1 897.0 | 3 239.6 |
| E. planned and cut, stock still stands (planner cell 1.5 mm) | 989.6 | 0 | 0 | 2 045.0 | 0 | 3 052.1 |
| total | 989.6 | 56.8 | 674.2 | 2 650.7 | 4 571.2 | 8 965.8 |

Id 2 follows id 1 with the smaller tool and no stated bottom. So the wood at the end of the two
roughs is what id 2 leaves (2.1). Id 1 has no forecast drop, no one-pass hold (fresh stock).

Files: `p44/cause_r3.txt`, `p44/cause_r9.txt`, `p44/probe_r3.txt` (each dropped region with its
box, area, forecast and bound), `p44/probe_r9.txt`.

Note on the windows: place 5 starts at Y 101 (the lead's window) for the nodes with a target
over 12 (the shelf); the board pocket floor in Y 101 to 104 stays with place 6. The cause
tables of 2.1 and 2.2 were made with place 5 from Y 104 (11 mm2 moves from place 6 to place 5).

A check before code (`p44/strip_cells.py`, output `p44/strip_r3.txt`): on the planner stock at
the end of id 2, 3 411.2 mm2 of tool-centre cells are "strip cells": the cell holds no material
at its own position, and its disc covers in full a cell whose stock stands more than one
planner cell over the cutter. 3 293 mm2 of them are in 9 long bands (1.2 to 3.0 mm at the
widest point). 2 838.5 of the 3 430.8 mm2 of mechanism D nodes stand inside the full disc of
a strip cell. So the strip read of the P42 wall pass finds mechanism D with no new threshold.

## 3. Step 2: the design (written before code)

The fact that all five changes use: a region of the level grid is a set of TOOL CENTRES
(`adaptive3d/CLAUDE.md`: "A region is in tool-centre space"). A cell is in it only when the
tool fits there and stock stands over the level floor. So no region is "too small for the
tool". The only reason to drop a region is the time of its entry against the length that it
cuts, and that is the operator dial `min_region_cut_length_mm` (F-038).

- H1 (mechanism A, and C that follows it). Remove the small-region bound of
  `detect_and_order_regions`. It is an area in the material frame, `(2 D)^2 x min(DPP / R, 1)`
  with a floor of `D^2 / 4`, applied to a region in the tool-centre frame, and its numbers have
  no derivation. `LeftWhy::SmallRegion` goes with it. BREAK: a rough (fresh or remaining stock)
  plans the small regions that it dropped. The dial that stays is `min_region_cut_length_mm`.
- H2 (mechanism B). The F-038 forecast reads the region in its own frame. Today it insets the
  region by `R + 0.25` and plans it with the 2D agent planner, which insets by R again. The new
  forecast is the length of the contour line of the region: its outline moved inside by one
  planner cell plus the cut tolerance (`level_contour::contour_line_inset`, the line of the P42
  wall pass, D19). This is the length that H3 ships at least, so the forecast and the plan
  agree. The 2D agent forecast stays as the second term (invariant of the folder file).
- H3 (the cut of a region that the 2D plan cannot take). The geodesic plan needs room for its
  starter pocket; a slot region 1.0 or 2.8 mm wide has none (`plan_piece` gives `None`, the
  "no field" case that C38 section 16 leaves to "the rest pass of the job"; a rest rough has
  none). New: a region of a level whose plan ships no cut gets the region contour: ONE pass
  along the contour line of H2, through the code of the P42 wall pass (`wall_read` for Z: the
  level floor, raised to one Depth/Pass under the stock in the disc; the rounds; the one-pass
  guard `OnePass::blocks` and the held-stock margin; `run_feed_cap`; `emit_runs`). No second
  depth rule. It runs after the plan of the level commits its pending entry, and before the
  P42 wall pass; such a region is not a planned region, so the wall pass does not trace it
  again. On both stock classes.
- H4 (mechanism D). On remaining stock only (`OnePassHold::RemainingStock`: the planner cell
  is the simulation cell; a coarse cell cannot read a strip). After the last level of the op,
  one strip pass: the cells where the tool fits and stock still stands in the disc (a material
  cell of the bottom level, or a strip cell by the read of `wall_read` pass 2) make bands; each
  band grows three cells on its open side (so that the contour line of H2 fits); the region
  contour of H3 runs along each band, in rounds, one Depth/Pass per round. A band whose line is
  shorter than `min_region_cut_length_mm` is not cut (the operator dial). The pass is part of
  the one-pass rule as the wall pass is (ruling D-4): no dial of its own. It is not in the
  level grid: `cell_has_material`, the By Area masks and the level gate keep their read.
  Why a pass and not the grid: the centre-cell read is the base of the By Area ownership, the
  level gate, the job levels, the wall read and the one-pass rule; a disc read there changes
  every region of every level.
- H5 (the finding). At the end of the plan (after the strip pass) the op reads the stock that
  stands where its tool fits: the components of (material cells of the bottom level, and on
  remaining stock the strip cells). Each component is one place: its box, its tool-centre area,
  its tallest stock over the cutter and the reason (a record of a drop inside it: the F-038
  forecast, no field, the one-pass hold; a band with no contour line; else "no record").
  `LeftStock` carries the list (largest first, a stated cap on the count), the text of
  `geom.rough_left_stock` prints each place, and the export gate prints the same text. No GUI
  face.

Independent hunks for separate commits: (a) H5 moves no tool; (b) H1 + H2 + H3 are one hunk
(H1 alone gives regions that the forecast or the field then drop); (c) H4.

### 3.1 Predictions (before code; setup "1 Back", id 41 off)

| # | Prediction |
|---|---|
| P1 | After id 2, wood that fits: antenna slot under 10 mm2 (412.8), control relief under 10 (158.2), slit slot under 10 (1 781.7). With H1 to H3 alone (build 1). |
| P2 | With H4 (build 2): ESP relief under 10 mm2 (59.4); step shelf under 200 (795.1); board pocket under 300 (957.4); cavity under 1 500 (5 016.1). |
| P3 | Id 2 fed length 38.8 m to about 55 m; time 578 s to between 780 and 880 s; setup about +250 s. |
| P4 | Id 2 depth rows: body and cleanup at most 6.5 mm (the Z of each contour point is the rule of `wall_read`). Entry row at most 2.5 mm, with a risk: an entry into a slot region 2.8 mm wide may have no room for the ramp. |
| P5 | Id 1: moves change little (5 small regions at level 11.0; those with a contour line of 15 mm or more are cut): time within +20 s; deepest body sample stays at most 4.666 mm. |
| P6 | 0 collisions; no node 0.1 mm or more inside the disc edge under the target (the contour is at least one cell plus the tolerance inside the tool-centre limit and passes the same emission pipeline). |
| P7 | rivmap100 and setup "3 Front" move (H1 on fresh stock plans small regions that pass the forecast): more moves, more time; no depth row worse. |
| P8 | The finding of id 2 names each place that stays; the stated tool-centre area falls from 1 429.6 mm2 (material cells) plus 3 411.2 mm2 (strip cells). |

## 5. Build 1 (H1 to H5 together; `p44/build1.patch`), measured 2026-11-21

Setup "1 Back", id 41 off. Output `p44/meas_b1.txt`; the finding text `p44/finding_text_b1.txt`.

| Place, after id 2, fits (mm2) | HEAD | build 1 |
|---|---|---|
| 1 antenna slot | 412.8 | 59.8 |
| 2 control relief | 158.2 | 13.6 |
| 3 slit slot | 1 781.7 | 121.8 |
| 4 ESP relief | 59.4 | 22.8 |
| 5 step shelf | 795.1 | 123.0 |
| 6 board pocket | 957.4 | 369.3 |
| 7 cavity | 5 016.1 | 787.9 |
| total | 9 193.9 | 1 504.1 |

- Places 1, 2 and 3 are cut to the floor. What stays is a rim one or two replay nodes wide
  (0.2 to 0.4 mm) at each wall: the contour line stands one planner cell plus the cut tolerance
  (0.3 mm) inside the tool-centre limit (D19). Antenna: 114 mm of wall, 59.8 mm2. Slit: 595 mm
  of wall, 121.8 mm2. So prediction P1 (under 10 mm2) is FALSE by the plan margin.
- Id 2: 15 239 to 18 657 moves; fed 38.8 to 51.5 m; time 578 to 868 s. Setup 9 682.9 to
  9 967.8 s. 0 collisions.
- FAILED in build 1, bar 2: id 1 cleanup row 10.334 mm (limit 4.666). Cause: the region
  contour on a FRESH block. Id 1 at level 11.0 ran one contour in the board pocket (X 103 to
  105, Y 51 to 91) where a band of stock about 1 to 2 mm wide stood 10.3 mm tall beside the
  line; the coarse planner cell (1.5 mm) did not hold it. Fix for build 2: the region contour
  runs only on remaining stock (`OnePass::on`), as the strip pass does.
- FAILED in build 1, bar 2: the corrected replay class of id 2 has 7 pieces (35 nodes, deepest
  9.73 mm; HEAD 0). Each is a short climb of a strip-pass contour between two points with a
  different Z. Fix for build 2: a region contour point takes the highest Z of itself and its
  two ring neighbours.
- The strip pass: 9 bands cut, 135 bands under the cut-length dial, 18 runs. It is weaker than
  predicted. Cause (by reading, not traced per point): the Z of a point is one Depth/Pass
  under ALL stock whose cell centre is in the disc (`wall_read` pass 1). A rim at a wall that
  the disc covers only in part (0.2 to 0.4 mm, at the stock top 25.5) holds the Z at 19.5,
  so the strip under it is not cut deeper. That is the one-pass promise at work; no change in
  this phase.
- `geom.rough_left_stock` of id 2 names 12 of 425 places (1 140.0 mm2 of tool-centre area),
  each with its box, area, height and reason. The G-code header prints only the short face
  line; the full text is in the export warning list (the CLI log line `WARNING Part
  geom.rough_left_stock ..`), as at HEAD.

## 6. Build 2 (`p44/not_landed/p44_build2.patch`), measured 2026-11-21

Changes from build 1: the region contour runs only on remaining stock (`OnePass::on`); a
point of a region contour takes the highest Z of itself and its two ring neighbours; the
cut-length dial is read on each strip band before it grows (already in build 1).

### 6.1 Wood, setup "1 Back", id 41 off (`p44/meas_b2.txt`)

After id 2, wood more than 0.3 mm over the target where the 6 mm tool fits (mm2):

| Place | HEAD | build 2 | predicted | what stays |
|---|---|---|---|---|
| 1 antenna slot | 412.8 | 59.8 | under 10 | the rim of the plan margin at the walls, up to 21 mm tall |
| 2 control relief | 158.2 | 13.6 | under 10 | the rim |
| 3 slit slot | 1 781.7 | 121.8 | under 10 | the rim: one node each side on 595 mm of wall |
| 4 ESP relief | 59.4 | 23.8 | under 10 | strips at the walls (5.6 mm2 never cut) |
| 5 step shelf | 795.1 | 124.6 | under 200 | 64.8 mm2 never cut, 57.8 last cut by id 1 |
| 6 board pocket | 957.4 | 364.2 | under 300 | 175.1 mm2 never cut (strips at the stock top), 102.8 by id 1 |
| 7 cavity | 5 016.1 | 762.0 | under 1 500 | 172.5 never cut, 343.9 by id 1, 245.6 by id 2 |
| total | 9 193.9 | 1 475.8 | | |

End of setup, all toolpaths: over the target 4 834.4 to 2 897.1 mm2; where the tool fits
3 149.6 to 936.5 mm2. With the committed job (id 41 on; `p44/places_headc.txt`,
`p44/places_b2c.txt`): over 4 595.2 to 2 775.2 mm2; fits 2 912.0 to 816.2 mm2.

Prediction P1: FALSE (the rim of the plan margin). P2: TRUE for places 5 and 7, FALSE for 4
and 6. Places 5 and 6 are more than halved (bar 1).

### 6.2 Rows, moves and times (`p44/rows_scratch_job.txt`, `p44/rows_back_committed.txt`)

| | HEAD | build 2 |
|---|---|---|
| id 1 moves; fed; feed time | 36 079; 89.2 m; 801.3 s | the same (the G-code of id 1 is the same) |
| id 1 body, deepest; cleanup deepest | 4.166, 4.166; 4.233 | the same |
| id 2 moves; fed; rapid; feed time | 15 239; 38.8 m; 4.3 m; 579.7 s | 18 737; 52.7 m; 9.7 m; 902.4 s |
| id 2 body row, deepest | 6.000, 6.130 | 6.000, 6.130 |
| id 2 cleanup row, deepest | 6.000 (bound 4.620), 6.001 | 5.000 (bound 3.429), 6.320 |
| id 2 entry row, deepest | 1.885, 1.894 | 1.885, 1.894 |
| id 2 link row | 6.000 | 6.000 |
| setup "1 Back", id 41 off | 9 682.9 s | 10 003.2 s |
| setup "1 Back", committed job | 9 715.8 s | 10 037.2 s |
| collisions, rapid collisions, failed checks | 0, 0, 0 | 0, 0, 0 |

The feed time is the fed length over the emitted feed (`gapmap.op_times`), with no
acceleration. Prediction P3 (55 m, 780 to 880 s): the length is near; the time is 22 s over.

Rows of other toolpaths that change state (setup "1 Back"): "Lake outline channel 6 mm"
entry: within to no number (8 061 to 4 663 moves); "Lake relief band 6 mm, 11 lakes" body and
entry: within to no number (the stock that it cut is gone; 7 206 moves stay and cut air).
Other changes with no state change: "Cavity rest pocket 6 mm" 756 to 155 moves; "Day strip
slit V60" body 4.5 to 1.83 mm with id 41 off (id 2 cuts the slit slot to its floor, so id 41 is
no longer needed for the V-bit); "Slit slot rest pocket" 26 to 44 moves, deepest 2.067 to
3.570 (within its bound 3.0005 by the row value 2.067; the deepest sample is new).

### 6.3 The replay class of bar 2 (`p44/deepc2_b2.txt`)

- Id 1: 0 pieces (as HEAD).
- Id 2: 4 pieces, 11 nodes, deepest 8.50 mm, the most inside node 0.199 mm:
  - (346.6, 97.48, 8.4) to (346.6, 98.08, 13.88) and its mirror at X 48.4: a climb of 5.5 mm
    over 0.6 mm at the end of the slit slot. One node each, 7.77 mm, 0.152 mm inside.
  - (230.2, 100.6, 11.19) to (229.8, 100.6, 17.75): one node, 6.61 mm, 0.136 mm inside.
  - (50.8, 100.6, 11.0) to (50.48, 100.8, 11.0): 8 nodes, 8.50 mm, 0.199 mm inside.
- Reading: the contour line stands on a node coordinate (the raw outline is at half cells; the
  inset is 0.3 mm), so its disc edge passes exactly through a column of nodes. The replay cuts
  a node at the edge whole; the planner stock blends a cell that the disc covers in part, so
  the planner reads such a cell lower than the replay does. The Z rule of a point
  (`wall_read`) then does not see the tall node, and the Z reads make a ramp over two or
  three points. This is the planner cell (0.2 mm) against the bar (0.1 mm). P42 has the same
  door and reads; at HEAD it gives 0 pieces on this job.

### 6.4 Setup "3 Front" (the committed job; `p44/rows_front.txt`) and rivmap100

| | HEAD | build 2 |
|---|---|---|
| setup "3 Front" | 8 762.4 s | 9 183.4 s |
| collisions, rapid collisions, failed checks | 0, 0, 0 | 0, 0, 0 |
| id 20 "Front bulk rough 18 mm" (fresh) | | no row and no move count changes |
| id 21 "Front 3D rough" moves | 36 882 | 49 802 |
| id 21 body row | | not changed |
| id 21 cleanup row, deepest | 4.393 (bound 3.138), 5.176 | 3.725 (bound 3.012), 5.130 |
| id 21 entry row, deepest | 1.996, 1.996 | 1.716, 1.881 |
| "Front scallop finish" deepest body sample (no bound) | 6.011 | 4.632 |

No depth row is worse (bar 5). No row changes state. The front has no model frame in the
replay instruments, so wood left and nodes under the target are not measured there (as P43).
rivmap100: not run (section 0).

### 6.5 The finding (bar 7; `p44/finding_text_b2.txt`)

`geom.rough_left_stock` of id 2 in build 2: "the tool of this operation fits and stock still
stands at 421 places (1 185.5 mm2 of tool-centre area)", then the 12 largest, each with its
box, area, height and reason. The record is `LeftStock::places` (`LeftPlace`, `PlaceWhy`) in
`ToolpathStats::rough_left_stock`; the diagnostic adapter and the export gate
`Gate::RoughLeftStock` print the same text (`RoughLeftStockFinding::fmt`), so CLI, MCP and GUI
read one record. The G-code header prints only the short face line, as at HEAD. No GUI face
was built.

### 6.6 Build 3: no strip pass (`p44/meas_b3.txt`, `p44/deepc2_b3.txt`)

Setup "1 Back", id 41 off. H1, H2, H3 (remaining stock only) and H5; H4 off.

| | HEAD | build 2 | build 3 |
|---|---|---|---|
| 1 antenna slot, fits after id 2 (mm2) | 412.8 | 59.8 | 59.8 |
| 2 control relief | 158.2 | 13.6 | 86.6 |
| 3 slit slot | 1 781.7 | 121.8 | 121.8 |
| 4 ESP relief | 59.4 | 23.8 | 59.4 |
| 5 step shelf | 795.1 | 124.6 | 525.7 |
| 6 board pocket | 957.4 | 364.2 | 861.0 |
| 7 cavity | 5 016.1 | 762.0 | 2 735.8 |
| total | 9 193.9 | 1 475.8 | 4 463.2 |
| id 2 moves; fed; feed time | 15 239; 38.8 m; 579.7 s | 18 737; 52.7 m; 902.4 s | 17 294; 44.4 m; 676.9 s |
| id 2 body, deepest; cleanup deepest; entry | 6.000, 6.130; 6.001; 1.885 | 6.000, 6.130; 6.320; 1.885 | 6.000, 6.130; 6.001; 1.885 |
| replay class, id 1 and id 2 (pieces) | 0, 0 | 0, 4 | 0, 0 |
| nodes under the target 0.1 mm inside the edge, id 2 | 8 | 8 | 8 |
| setup "1 Back" | 9 682.9 s | 10 003.2 s | 9 813.8 s |
| collisions, rapid collisions, failed checks | 0 | 0 | 0 |

Build 3 was not run on the committed job, on setup "3 Front" or on rivmap100.

## 7. Gates (on the patched tree, before the revert)

| Gate | Result |
|---|---|
| `--lib adaptive3d:: _p37 _p44` | 93 pass, 0 red, 7 ignored (`p44/gate_lib_adaptive3d.log`). This run is from before the format edits, one lint attribute and the three `panic!` to `expect` edits in the tests. Clippy compiled the final tests; they did not run again. |
| rustfmt check, core crate | clean |
| full clippy (the command of the root `CLAUDE.md`) | clean, exit 0, 18 min 44 s (`p44/gate_clippy.log`); the first run found 3 `clippy::panic` in the new tests, fixed before this run |
| all other gates of bar 9 | NOT RUN (disk: 4.1 GB free) |

Lib tests changed for the patch: `a_small_region_the_plan_drops_is_stated_p37` is now
`a_region_under_the_cut_length_dial_is_stated_p37` (the dial drops the column; the bound is
gone); the P41 rib fixture and `the_one_pass_dial_off_keeps_the_plan_without_the_rule_p41`
run with the P44 contours off (they need stock that the levels above LEFT); the control of
`each_level_cuts_to_the_wall_that_it_can_reach_p42` turns the strip pass off too; the text
needles of `standing_left_stock_is_one_caution_p37`.
New: `a_small_region_is_cut_when_no_dial_drops_it_p44`,
`a_slot_narrower_than_the_field_is_cut_to_its_floor_p44`,
`the_forecast_reads_a_slot_region_in_its_own_frame_p44`,
`a_strip_beside_a_wall_with_no_region_is_cut_p44`, `the_sliding_maximum_reads_each_window_p44`.

## 8. Decisions for the lead

- D-0. The choice after build 3. H1 + H2 + H3 + H5 (no strip pass) holds bar 2 and bar 3 as
  HEAD does, cuts places 1 and 3 to the floor (less the rim) and halves the total (9 193.9 to
  4 463.2 mm2), for +97 s of id 2. It does not halve places 5 and 6, and place 4 does not
  change: those are strips at the walls (mechanism D), which only the strip pass cuts. Choices:
  land a product form of build 3 (to be written and gated); or wait for D-1.
- D-1. The replay bar of bar 2 against the planner cell. The patch holds the depth ROWS and
  leaves 4 replay pieces (11 nodes, each at most 0.199 mm inside the disc edge). Choices:
  (a) keep it not landed; (b) rule that on remaining stock the class counts a node one planner
  cell (0.2 mm) or more inside the edge, then land the patch (the 4 pieces are then 0; to be
  measured); (c) a new phase for the reads of `wall_read` at a blended cell.
- D-2. The rim of the plan margin (D19). A contour line stands 0.3 mm inside the tool-centre
  limit. So each wall keeps a rim 0.2 to 0.4 mm wide, as tall as the wall. Places 1 to 3 need
  a line nearer than one planner cell to reach "under 10 mm2"; D19 says that a nearer line
  reads the rim of the heightmap. Not tried.
- D-3. The Z rule next to a tall rim. `wall_read` holds each contour point one Depth/Pass
  under ALL stock whose cell centre is in the disc. A rim at the stock top then holds the strip
  pass at 19.5 at that wall. This is why 175 mm2 of the board pocket and 172 mm2 of the cavity
  are never cut. A read by the stamp coverage (the P41 measure) would cut more and would add
  to the class of D-1.
- D-4. Fresh stock. H1 and H2 act on a fresh rough too, and they change nothing on this job
  (id 1 keeps its G-code). The region contour (H3) on a fresh block cut 10.33 mm deep in
  build 1, because the coarse planner cell does not hold a band 1 to 2 mm wide that the levels
  above left. So a fresh rough still leaves a region with no room for the field; the finding
  names it. C38 section 16 stands on fresh stock.
- D-5. The parity gap of the `emit_runs` door (section 0). It is in HEAD for P42 and P43. It
  needs its own trace.
- D-6. The cut-length dial is now the only drop rule of a region, and its default (15 mm) is
  in the job file. 213 areas of id 2 are under it (per level). A lower dial cuts more and
  enters more.
- D-7. If the patch lands later: id 41 "Slit slot rest pocket 6 mm" is no longer needed for
  the V-bit load (4.5 to 1.83 mm with id 41 off), and "Lake relief band 6 mm, 11 lakes" cuts
  air.
- D-9. The finding now fires on almost every 3D rough (id 1 lists a place of 2.2 mm2;
  rivmap100 gets a new header line). Choices: a floor on the place area or height, or raise
  the finding only for a place with a drop record or a one-pass hold.
- D-8. The disk. `/` has 4.1 GB free. The lead gate cannot run until `target/` is pruned.

## 9. State at hand-off

- `crates/` is at HEAD `401634ee4` (`git status --short crates` prints nothing).
  `git apply --check p44/not_landed/p44_build2.patch` passes on it.
- `target/release/rs_cam_cli`: a rebuild from the HEAD tree was started after the last revert
  (scratch `p44/build_head2.log`, `build_head2.done`). Until `build_head2.done` says `rc 0`
  the binary is build 3. Check before any CLI number is trusted.
- New files of this phase are all under `planning/feeds_contract_2026-11-03/phases/`: this
  note and `p44/`. `p37/reach6.npy`, `p37/reach6h.npy` and `p37/target.npy` are caches of the instruments: do
  not commit them.

Predictions P4 to P8, verdicts (build 2): P4 TRUE for the rows, FALSE for the replay class
(4 pieces); the entry risk did not occur (1.885). P5 TRUE (id 1 does not change; in build 1
it was FALSE: 10.33 mm). P6 TRUE for collisions; the nodes under the target are the 8 of HEAD.
P7: "3 Front" moves (+421 s, id 21), no depth row worse, TRUE; rivmap100 does not move, the
prediction was FALSE. P8 TRUE: 421 places, 1 185.5 mm2 (strip cells and material cells).

## 10. P44b: the product form (2026-11-21)

Status: the product form of build 3 is applied on the working tree (not committed), with the
lead rulings of round 2 (10.10). HEAD is
`82cf231c7`. The lead rulings of 2026-11-21 are the base: no strip pass in the product, the
replay bar does not move, the finding rule of D-9, bar 1 for places 1 and 3, H3 off on fresh
stock, no new constant.

### 10.0 What failed and what is not verified (after round 2)

RESULT: every measure bar holds and no new gate is red. The code is applied on the working
tree and is ready for the commit of the lead (`p44/COMMIT_MSG.txt`). Nothing is committed.

- NOT DONE by ruling (D-10): the control relief keeps a strip of 63.9 mm2 (86.6 mm2 with the
  rim). It waits for the strip pass phase. The finding now names it by its real reason (the
  dropped cut groups, D-11).
- Bar 1, place 1 (the antenna slot): the slot is cut to its floor, and the rim is 0.2 to
  0.6 mm wide, not only the D19 margin. The line stays (ruling D-12). Place 3 holds.
- The gouge bar holds for its class (8 nodes, HEAD 8). The count of nodes nearer than 0.1 mm
  to the disc edge is 270 against 265.
- The fresh front rough (id 20) does not keep its G-code: the same 22 842 moves in another
  region order, and its fed length is 51.6 mm less (10.10). Each row value, bound and state
  is the same. The cause (H1) is from reading the code; no run with H1 reverted alone was
  made on the front.
- "Slit slot rest pocket 6 mm" (id 41): its deepest body sample is 3.570 mm against the bound
  3.0005; the row stays 2.067, within (10.3).
- The published rest regions of a Geodesic Adaptive rough change (BREAK, 10.10): a small
  region that the rough plans and cannot cut is stated; a no-field piece is stated in
  tool-centre space; a region that the region contour cut is not stated.
- NOT VERIFIED: the op golden against a HEAD run of this session. The red list (52 cells,
  names and values) is the same in round 1, in round 2 and in the last golden run of P43
  (scratch `p43/gate_b3_golden.log`). No cell is blessed.
- NOT VERIFIED: the replay class and the gouge nodes were not run again in round 2. The four
  G-code files of round 2 are the same bytes as those of round 1, so the round 1 results stand.
- NOT VERIFIED: the front in the replay instruments (no model frame, as P43). No GUI or MCP
  surface was run, and no `rs_cam_gui` was built.
- NOT RUN in round 2: the extra rough and trace sentries of round 1 (`p44/gate_p44b_more.log`:
  18 of 19 green, 1 known red). The lead list of round 2 does not name them.
- No red proof was made for the two assertions that round 2 adds to the lib tests. The slot
  assertion has its control in the test. The frame fix has the D44 sentry, red before the fix.
- KNOWN RED, not from this phase: `feeds::envelope::tests::the_envelope_meets_its_time_budget`
  (lib) and `a_clearing_cut_holds_the_pass_load_g_adaptpassload` (red since `c6826ad05`).
- The limit of the cause reading of 10.2: the debug lines are per By Area job and level. The
  moves inside the place come from the G-code. No probe per cell was run in P44b.
- Sections 10.2 to 10.8 keep the text of round 1. Where 10.10 differs (the finding counts of
  10.4, the gates of 10.6, the decisions of 10.8), 10.10 is the current fact.

### 10.1 What the product form is

- H1: no small-region bound. `LeftWhy::SmallRegion` is deleted. The level metric
  `dropped_micro_region_count` and its trace key are deleted (BREAK: the semantic key
  vocabulary has 75 keys, was 76).
- H2: the F-038 forecast of a region reads the length of its contour line
  (`level_contour::contour_line_inset`: one planner cell plus the cut tolerance).
- H3: `level_contour::region_contour_pass`, only with `OnePass::on` (remaining stock).
- H5: `standing.rs` and the places of `left_stock.rs`, with the finding rule of ruling 3
  (10.4).
- Deleted from build 3: `strip_pass`, `grow_open_side`, the env switch `P44_STRIP_PASS`,
  `LeftWhy::NoLine`, `PlaceWhy::NoLine`, `PlaceWhy::NoRecord`, `LeftStock::no_line`, the test
  `a_strip_beside_a_wall_with_no_region_is_cut_p44`, `contour_line_length` and two fields of
  `Standing` (dead with no strip pass). No env switch is in product code. The test control
  `REGION_CONTOUR_OFF_FOR_CONTROL` is `cfg(test)` only.
- `p44/not_landed/p44_build2.patch` and `p44_build3_no_strip_pass.patch` are not changed.

The G-code of setup "1 Back" with id 41 off is the same as build 3, byte for byte
(`cmp out_b3.nc out_p44b.nc`). So each measure of section 6.6 holds for the product form.

### 10.2 The two open points (work B)

Control relief, 86.6 mm2 in build 3 against 13.6 in build 2. The cause is the cut-length
dial inside the plan: the cut-group filter of F-038 drops 11 of the 13 cut groups of a region
that H1 now plans. The wood that stays is then a strip beside a wall, which no level can
plan. H1, H2 and H3 do not drop it, and the fix is not in P44b (D-10). The numbers:

- Level 1.5 of id 2. The region of the relief is 77.3 mm2 of tool centres (9.07 x 8.16 mm).
  H1 keeps it (HEAD dropped it under 144 mm2). The forecast passes. The region opens "grown,
  no sweep".
- The geodesic plan of the region has 13 cut groups. The cut-group filter of F-038
  (`filter_short_groups`, the dial `min_region_cut_length_mm` = 15 mm) drops 11 of them
  (debug line `F-038: dropped short cut groups .. z=1.5 region=2 dropped_groups=11
  group_total=13`). The 2 groups that ship are the ramp and the starter spiral: moves 10 622
  to 10 697 of id 2, a spiral to a radius of 3.0 mm about X 181.95 Y 58.35 at Z 4.5. They cut
  a disc of radius 6.0 mm.
- After that cut no tool centre of the region holds stock at its own cell, but two corner
  pieces of 0.70 and 0.42 mm2 (the regions of level 0.5; the forecast drops them). So the
  wood between the disc and the walls is a strip beside a wall (mechanism D): it makes no
  region at level 0.5.
- The P42 wall pass ships no point there: the wall is 3.0 mm (Z 7.5 to 4.5), not deeper than
  one pass (limit 6.5 mm). The region contour does not run: the plan of the region shipped a
  cut.
- The area split (`p44/rim_split.py`, `p44/place_map.py`): 86.6 mm2 = 22.7 mm2 at most 0.4 mm
  from a wall (the rim of the plan margin) + 63.9 mm2 of strip, 1.0 to 2.6 mm wide, at Z 7.5
  (3.0 mm over the floor). Build 2: 13.6 mm2, all at most 0.4 mm from a wall. The strip pass
  of build 2 cut the strip.
- Not fixed in P44b. A fix at the source is a contour on each region whose plan dropped cut
  groups. That is 15 of the 50 planned region-levels of id 2, on regions up to 7 812 mm2. It
  ships through the door of the strip pass, it is not what build 3 measured, and it needs a
  new measure. See decision D-10.
- The one-pass hold is not the cause at level 0.5: it holds 5 centres in the whole By Area
  job at that level (debug line `One-pass rule: .. z=0.5 held=5`).
- Evidence: `cli_b3` with `RUST_LOG=info,rs_cam_core::adaptive3d=debug` (scratch
  `p44/run_b3dbg.log`; its G-code is the same as `out_b3.nc`), and the moves of id 2 inside
  the place (`p44/box_moves.py`).

Antenna slot, 23.0 mm2 last cut by "none", 21 mm tall. It is the first node column at the
wall, and no level cuts it:

- 20.4 of the 23.0 mm2 are nodes 0.2 mm from the wall (`p44/place_map.py`, the row "never
  cut"); the wall of the slot is about 113 mm long, and 113 x 0.2 = 22.6 mm2.
- The contour line of levels 19.5, 16.55 and 13.5 is at Y 57.40 and 59.60 (X 246.80 to
  288.20). The tool-centre limit is Y 57.2 and 59.8. So the disc edge stands 0.2 mm from the
  wall at each level, and the node column on the wall side stays at the stock top (Z 25.5,
  21.0 mm over the floor 4.5). This is the D19 margin (one planner cell plus the cut
  tolerance).
- The other 36.8 mm2 (last cut by id 2): the node columns 0.4 and 0.6 mm from the wall,
  28.2 mm2 of them at Z 13.5. The lines of levels 8.55, 7.5 and 1.5 are one cell more inside
  (Y 57.60 and 59.40, X 247.00 to 288.00). The first column stands 17 mm over the cutter of
  level 8.55, so the one-pass hold removes the centres whose disc covers it. The line
  positions are measured from the G-code; the one-pass hold as the cause of the second line
  is a reading of the rule, not a trace.
- So the rim of place 1 is 0.2 to 0.6 mm wide (58.5 of 59.8 mm2 at most 0.6 mm from a wall).
  The slot is cut to its floor inside the rim.

### 10.3 The measures (release CLI of the product tree, scratch `p44b/cli_p44b`)

Setup "1 Back", `p44/job_no41.toml` (`p44/meas_p44b.txt`, `p44/places_p44b.txt`). Wood more
than 0.3 mm over the target where the 6 mm tool fits, after id 2 (mm2):

| Place | HEAD | product | what stays |
|---|---|---|---|
| 1 antenna slot | 412.8 | 59.8 | the rim at the walls, 0.2 to 0.6 mm wide (10.2) |
| 2 control relief | 158.2 | 86.6 | 22.7 rim + 63.9 strip (10.2) |
| 3 slit slot | 1 781.7 | 121.8 | the rim: 121.7 mm2 at most 0.2 mm from a wall |
| 4 ESP relief | 59.4 | 59.4 | strips at the walls (no strip pass) |
| 5 step shelf | 795.1 | 525.7 | strips at the walls |
| 6 board pocket | 957.4 | 861.0 | strips at the walls |
| 7 cavity | 5 016.1 | 2 735.8 | strips at the walls |
| total | 9 193.9 | 4 463.2 | |

End of setup, all toolpaths, id 41 off: over the target 4 834.4 to 3 889.1 mm2; where the tool
fits 3 149.6 to 2 074.3 mm2. Committed job (id 41 on; `p44/meas_p44bc.txt`, HEAD in
`p44/places_headc.txt`): over 4 595.2 to 3 767.3 mm2; fits 2 912.0 to 1 954.0 mm2.

| | HEAD | product |
|---|---|---|
| id 1 moves; fed; feed time | 36 079; 89.2 m; 801.3 s | the same G-code |
| id 1 body row, deepest; cleanup deepest; entry | 4.166, 4.166; 4.233; 1.969 | the same |
| id 2 moves; fed; rapid; feed time | 15 239; 38.8 m; 4.3 m; 579.7 s | 17 294; 44.4 m; 7.4 m; 676.9 s |
| id 2 body row, deepest | 6.000, 6.130 | 6.000, 6.130 |
| id 2 cleanup row (bound), deepest | 6.000 (4.620), 6.001 | 5.000 (3.429), 6.001 |
| id 2 entry row, deepest | 1.885, 1.894 | 1.885, 1.894 |
| id 2 link row | 6.000 | 6.000 |
| replay class, id 1 and id 2 (`p43/deepc.py`, pieces) | 0, 0 | 0, 0 |
| id 2 nodes under the target, 0.1 mm or more inside the disc edge (`p43/gouge.py`) | 8 | 8 |
| id 2 nodes under the target, nearer to the edge | 265 | 270 |
| setup "1 Back", id 41 off | 9 682.9 s | 9 813.8 s |
| setup "1 Back", committed job | 9 715.8 s | 9 847.8 s |
| collisions, rapid collisions, failed checks (both jobs) | 0, 0, 0 | 0, 0, 0 |

The HEAD run of the committed job on setup "1 Back" was made again in this phase (scratch
`p44/out_headc`; the output folder of the first run was not in scratch). Its total is the
9 715.8 s of section 6.2.

Limit rows that change state against HEAD, setup "1 Back" (`p44/rows_back_committed_p44b.txt`
for the committed job, `p44/rows_scratch_job_p44b.txt` for id 41 off; the same rows in both):

- "Lake outline channel 6 mm" (id 28), entry depth: within to no number (8 061 to 6 545
  moves).
- "Lake relief band 6 mm, 11 lakes to Z 13.5" (id 25): chip, depth, spindle power and tool
  deflection, body and entry (8 rows): within to no number. The stock that it cut is gone;
  its 7 206 moves stay. The G-code header gets three lines "not measured" for it.
- With id 41 off, the header loses the line `project.sim_outside_envelope` of "Day strip slit
  V60" (body 4.5 to 1.83 mm). In the committed job the header loses the line
  `project.sim_outside_envelope` of "Slit slot rest pocket 6 mm" (id 41). No other header
  line changes on the back.

No state change, but stated: "Slit slot rest pocket 6 mm" (id 41, committed job) 26 to 44
moves; its body row stays 2.067 (within 3.0005) and its deepest sample is 3.570 mm (was
2.067). "Cavity rest pocket 6 mm" 756 to 875 moves.

Setup "3 Front", committed job (`p44/rows_front_p44b.txt`):

| | HEAD | product |
|---|---|---|
| setup "3 Front" | 8 762.4 s | 9 053.4 s |
| collisions, rapid collisions, failed checks | 0, 0, 0 | 0, 0, 0 |
| id 20 "Front bulk rough 18 mm" (fresh) moves; fed; rapid | 22 842; 55.3 m; 3.9 m | 22 842; 55.2 m; 4.2 m |
| id 21 "Front 3D rough" moves; fed; rapid; feed time | 36 882; 54.0 m; 15.1 m; 849.6 s | 46 432; 64.8 m; 20.0 m; 1 020.1 s |
| id 21 body row | | not changed |
| id 21 cleanup row (bound), deepest | 4.393 (3.138), 5.176 | 3.725 (3.012), 5.130 |
| id 21 entry row, deepest | 1.996, 1.996 | 1.716, 1.881 |
| id 21 link row, deepest | 0.480, 0.480 | 0.480, 0.487 |
| "Front scallop finish" deepest body sample (no bound) | 6.011 | 4.714 |

No row changes state. No depth row is worse; one deepest sample is (the link of id 21, by
0.007 mm, under its bound 3.0005). The G-code of id 20 is not the same as HEAD: two blocks of
moves change places (the region order; H1 gives the order more regions), with the same move
count and the same rows. The front has no model frame in the replay instruments (as P43).

rivmap100 (`planning/fixtures/rivmap100/rivmap100_live_0925.toml`, read in place): the G-code
of the product is the same as HEAD, byte for byte, the header included (`cmp`; scratch
`p44b/riv_head.nc` from `cli_head`, made in this phase, and `p44b/riv_p44b.nc`).

### 10.4 The finding (ruling 3)

`LeftStock::stands()` is the one rule: a dropped piece still holds stock, or the one-pass rule
held the tool off stock that still stands (the two terms of HEAD), or a place with a drop
record stands. A place with no record does not raise it. `left_stock()` gives `None` when the
plan recorded nothing, as at HEAD, and reads the standing stock only when it has a record.

- The text lists the places with a record (the 12 largest) and gives one line for the others:
  "Stock also stands at N more places with no drop record (A mm2 of tool-centre area)".
- id 1: 5 places with a record (238.5 mm2), 3 with no record (229.5 mm2). id 2: 135 places
  with a record (3 895.6 mm2), 17 with no record (16.9 mm2). id 20: 81 and 55 (5 265.0 and
  438.8 mm2). id 21: 534 and 1 250 (44 631.7 and 786.0 mm2). Text: `p44/finding_text_p44b.txt`.
- The header lines `geom.rough_left_stock` are the same four as at HEAD (ids 1, 2, 20, 21).
  rivmap100 gets none, as at HEAD. No conflict with "HEAD already raised it" was found.
- Ruling 5: id 1 (fresh) names a place with "the Geodesic Adaptive field does not fit"
  (157.5 mm2 at X 125.5 to 140.5, Y 166.5 to 187.5).
- Limit of the record (D-11): the cut-group filter has no outline record. The reason of a
  place is that of a recorded piece on its cells, so a rim beside a recorded piece takes its
  reason (section 0).

### 10.5 The tests (work C), each seen red with its hunk reverted alone

`p44/revert.py` reverts one hunk by hand; `p44/red.sh` runs `--lib _p44`, restores the files
and compares `git diff` before and after ("tree restored" each time). Record:
`p44/red_proofs_p44b.txt`.

| Hunk reverted alone | Red tests | Red text |
|---|---|---|
| H1 (the bound is back) | `a_small_region_is_cut_when_no_dial_drops_it_p44`, `a_slot_narrower_than_the_field_is_cut_to_its_floor_p44`, `the_forecast_reads_a_slot_region_in_its_own_frame_p44` | "the column is cut"; "the centre line of the slot stands at Z 1.00" |
| H2 (the forecast insets by R + 0.25) | `the_forecast_reads_a_slot_region_in_its_own_frame_p44` | "the centre line of the slot stands at Z 10.00" |
| H3 (no region contour) | the three of H1 | "the column is cut"; "the centre line of the slot stands at Z 10.00" |
| the finding rule (a place with no record raises it) | `stock_with_no_drop_record_does_not_raise_the_finding_p44` | "no drop record: the finding is not raised" |

After the four proofs: `--lib _p44 _p37 _p41 _p42 _p43`, 21 pass, 0 red.

One fact that the build 3 text had wrong: with the bound back, the slot of the fixture is cut
to Z 1.00, not left at Z 10. The bound drops its region only at the last level. The test text
now says what the proof showed.

### 10.6 Gates

Each gate ran as `timeout <s> scripts/cargo_lane.sh <args> -j 2` on the product tree
(`p44/gates_p44b.sh`; logs `p44/gate_p44b_*.log`). The disk had 97 GB free at the end.

| Gate | Result |
|---|---|
| `test -p rs_cam_core -q --lib` | 2 862 pass, 1 red: `feeds::envelope::tests::the_envelope_meets_its_time_budget` (known) |
| op golden `every_op_keeps_its_toolpath_g_opgolden` | red, 52 cells (`p44/golden_p44b_cells.txt`). No new cell and no changed value against the last golden run of P43 (16 adaptive3d, 15 pencil, 6 scallop, 6 steep_shallow, 9 unified_finish). Not blessed |
| adaptive3d sentries (37 binaries) | 36 green (74 tests). 1 RED, new: `a_derived_rest_rough_cuts_only_the_leftovers_d44` |
| dressup sentries (19 binaries) | 119 pass, 0 red |
| other core sentries (20 binaries, `findings_transport_join_h21` included) | 141 pass, 0 red |
| extra rough and trace sentries (19 binaries, features `test-support,heavy-tests,research`) | 18 green (54 tests). 1 red, known: `a_clearing_cut_holds_the_pass_load_g_adaptpassload` |
| FM1 `feeds_matrix_instrument_fm1` | 2 pass |
| `test -p rs_cam_cli -q` | 113 pass, 0 red |
| `test -p rs_cam_mcp -q` | 39 pass, 0 red |
| viz `--lib -- --test-threads 2` (no `rs_cam_gui` process) | 705 pass, 0 red |
| full clippy (the command of the root `CLAUDE.md`) | clean, exit 0 |
| `cargo fmt -p rs_cam_core -- --check` | clean, exit 0 |

Two comment-only edits came after the gates: the doc of `GEOM_ROUGH_LEFT_STOCK` in
`diagnostics/ids.rs` and one sentence of `diagnostics/CLAUDE.md` (the finding rule of ruling
3). After them: `rustfmt --check` of `ids.rs` is clean and `cargo check -p rs_cam_core` is
clean (exit 0). No test ran again.

### 10.7 Bars

| Bar | Result |
|---|---|
| id 1 body row at most 4.666 | HELD: 4.166 (deepest 4.166; cleanup deepest 4.233) |
| id 2 body and cleanup rows at most 6.5 | HELD: 6.000 and 5.000 (deepest 6.130 and 6.001) |
| id 2 entry row at most 2.5 | HELD: 1.885 (deepest 1.894) |
| replay class 0 pieces, ids 1 and 2 | HELD: 0 and 0 |
| 0 collisions, 0 rapid collisions | HELD: back (both jobs) and front |
| gouge nodes not more than HEAD | HELD for the class of the bar (8 nodes 0.1 mm or more inside the edge, HEAD 8; the six that the instrument prints have the same coordinates). 5 more nodes nearer to the edge (270, HEAD 265) |
| rivmap100: the same bytes as HEAD | HELD |
| bar 1: places 1 and 3 cut to the floor, only the rim stays | Place 3: HELD (121.7 of 121.8 mm2 at most 0.2 mm from a wall). Place 1: cut to the floor; the rim is 0.2 to 0.6 mm wide, wider than the D19 margin by the cell that the one-pass hold keeps (D-12) |
| front: no depth row worse | HELD for the rows; the deepest link sample of id 21 is 0.007 mm more |
| gates | Round 1: one new red sentry (D-15). Round 2 (10.10): the sentry is green; each red is known |

### 10.8 Decisions for the lead (P44b)

All of these are ruled (10.10). The text below is the question as it stood after round 1.

- D-10. The control relief (63.9 mm2 of strip). The cut-group filter of F-038 drops 11 of the
  13 cut groups of a region that H1 now plans, and the rest is a wall strip. Choices: (a) it
  waits for the strip pass phase; (b) the region contour runs also on a region whose plan
  dropped cut groups: 15 of the 50 planned region-levels of id 2 dropped groups (231 groups),
  on regions of 77 to 7 812 mm2; not measured; it ships through the `emit_runs` door, as the
  strip pass did; (c) a lower `min_region_cut_length_mm` on the job (D-6).
- D-11. A dropped cut group has no outline record. A place where only that filter dropped
  the cut reads "no drop record". The control relief has a record only because the forecast
  drops two corner pieces at level 0.5. Choice: record the outline of the piece with the
  reason of the dial; then each standing cell of a large planned region takes that reason.
- D-12. Bar 1, place 1. The rim is 0.2 to 0.6 mm wide: 23.0 mm2 never cut (the D19 margin)
  and 36.8 mm2 that the one-pass hold keeps beside it at Z 13.5 to 19.5. The line was not
  moved (ruling 4).
- D-13. BREAK of the trace vocabulary: the key `dropped_micro_region_count` is deleted (75
  keys). The alternative is a key that always reads 0.
- D-14. H1 on a fresh rough changes the region order (id 20: two blocks change places). The
  op golden cells that move are in 10.6; none is blessed.
- D-15 (blocks the commit). The rest regions that a rough publishes change (10.0). Choices:
  (a) pin the D44 sentry again to the two regions (657.7 mm2); then a consumer of a slot
  region may put its tool centre up to about one tool radius outside the tool-centre region of
  the slot; (b) record a region with no plan as ONE left piece in its own frame, in the one
  list `left_pieces`; this changes the zone that the wall contour skips, so it needs a new
  measure of the three jobs; (c) a third fact for the same ruling: on remaining stock a slot
  that the region contour cut stays in `left_pieces`, so it is published and the wall contour
  skips it (from reading the code, not measured). BREAK either way: a rough publishes the
  small regions that it does not cut. Evidence for (b): with H2 reverted alone and H1 on, the
  sentry is green (its regions were not printed: `-q` hides the output of a green test).
- D-16 (does not block). `left_stock::PLACES_LISTED = 12` has no derivation (ruling 6). It
  came with build 3. It limits only the text of the finding, never a cut.
- D-7 stands: with this change id 41 is no longer needed for the V-bit load, and "Lake
  relief band 6 mm, 11 lakes" cuts air (8 rows read no number).

### 10.9 State at hand-off (P44b)

- `crates/` holds the product form. The diff of `crates` and `FEATURE_CATALOG.md`, with the
  new file `adaptive3d/standing.rs`, is `p44/p44b_product.patch`; `git apply -R --check` of
  it passes. Nothing is staged and nothing is committed.
- `target/release/rs_cam_cli` is the build of the product tree, not of HEAD (copy: scratch
  `p44b/cli_p44b`). No `rs_cam_gui` was built in this phase, so a running GUI is not this
  tree.
- Scratch: `p44b/` (builds, runs, gate logs, probes) beside `p44/` (the run outputs
  `out_p44b`, `out_p44bc`, `outF_p44bc` and their `.nc` and `.npz`). HEAD baselines made in
  this phase: `p44b/riv_head.nc` (rivmap100) and `p44/out_headc` (the committed job, setup
  "1 Back"), both from `p44/cli_head`.
- Do not commit `p37/*.npy` or the `__pycache__` folders. `p44/COMMIT_MSG.txt` has the
  message and the paths.
- Round 2: scratch `p44b/cli_p44b2` is the release CLI of the final tree, and
  `target/release/rs_cam_cli` is that build. The second runs are `out_p44b2`, `out_p44b2c`,
  `outF_p44b2c` (scratch `p44/`) and `p44b/riv_p44b2.nc`.

### 10.10 Round 2: the lead rulings on D-10 to D-16 (2026-11-21)

Rulings: D-15 choice (b) and (c) with it; D-11 a drop record for a dropped cut group; D-10 the
control relief strip waits for the strip pass phase; D-12 the antenna rim stays; D-13 the
metric and the trace key stay deleted; D-16 `PLACES_LISTED = 12` stays as a text cap.

What changed in the code:

- D-15 (b). One frame for every `leave_piece` call. The no-field path of `plan_one_outline`
  records the part of the region inside the planner outline
  (`region_polygon.intersection(polygon_for_adaptive)`), which is in tool-centre space, as the
  forecast path is. It recorded the planner outline (the region grown by about the tool
  radius) before.
- D-15 (c). `clear_z_level_slice` keeps, for each region whose plan ships no cut, the ranges
  of its records in `left_pieces` and `unplanned`. `region_contour_pass` now says for each
  region whether a run of its line shipped. The records of a region that the contour cut are
  removed: it is not left, it is not published, and the wall contour does not skip there.
- D-11. `filter_short_groups` gives the XY box of the cut of each dropped group.
  `plan_one_outline` records it as `LeftWhy::ShortGroup` (each round of the filter). It is in
  `unplanned` only, never in `left_pieces`, so it changes no published region and no skip.
  `left_stock` gives that reason to the standing cells of the box and one cell round it (the
  box of a straight cut has no area). It is not a standing piece: `standing_pieces` and the
  two terms of HEAD in `LeftStock::stands` read as before. The place takes `PlaceWhy::ShortGroup`
  ("the plan dropped cut groups shorter than Min Region Cut Length").
- D-16. The comment of `PLACES_LISTED` says that it limits the text only and that the text
  gives the sum of all recorded places.
- The D44 sentry (`a_derived_rest_rough_cuts_only_the_leftovers_d44`). After the frame fix the
  rough states 2 regions, 284.5 mm2: the channel 249.1 mm2 (box X -27.75 to 27.75, Y -2.25 to
  2.25) and a corner 35.4 mm2 (box X -31.75 to -25.75, Y -21.75 to -13.25). The test reads the
  channel region alone against the math of the fixture (253.0 mm2, plus or minus 20 %, as
  before). The corner is not a function of the fixture constants: the plan of the first level
  rounds the corners of the wide pocket, stock stands on a few tool centres there, H1 plans
  that region at the second level and the field has no room in it. So the test asserts each
  other region by its box: inside the tool-centre region of the wide pocket (two planner cells
  of slack), at a corner of it, clear of the channel, and at least one such region. The
  unchanged sentry was red before the fix (657.7 mm2) and green after it; the new form is
  green.
- Lib tests: `a_slot_narrower_than_the_field_is_cut_to_its_floor_p44` also asserts that the
  cut slot is not a left piece (`no_field == 0`; the control with no region contour has
  `no_field > 0`). `stock_with_no_drop_record_does_not_raise_the_finding_p44` also asserts
  the cut-group reason on a box with no area.

Id 20 "Front bulk rough 18 mm" (fresh), why two blocks of moves change places: H1. HEAD
removed the regions under the bound before `detect_and_order_regions` put the regions of a
level in nearest-neighbour order. Now those small regions are in that list (the forecast or
the field drops them after the order is made), so the walk visits the large regions in
another order. This is from reading the code and from the counts (the 121 dropped
areas of id 20 at HEAD, 120 under the bound and 1 with no field, are 98 forecast drops and 23
no-field pieces now); no run with H1
reverted alone was made on the front. The move count is the same (22 842). The fed length is
NOT the same: 55 288.0 to 55 236.4 mm (51.6 mm less; the links at the joins differ), rapid
3 936.5 to 4 247.3 mm, feed time 490.6 to 490.2 s. Each of its 21 limit rows has the same
value, bound and state (`p44/rows_one_op.py`); only sample counts and sample indexes differ.

The measures again (release CLI of the final tree, scratch `p44b/cli_p44b2`):

| Run | G-code against the first product run | Against HEAD |
|---|---|---|
| rivmap100 | the same bytes | the same bytes, header included |
| setup "1 Back", `p44/job_no41.toml` | the same bytes | as 10.3 |
| setup "1 Back", committed job | the same bytes | as 10.3 |
| setup "3 Front", committed job | the same bytes | as 10.3 |

So the rows, the times, the places table, the replay class (0 and 0), the gouge nodes (8) and
the collisions (0) of 10.3 stand, and they were not run again. Setup totals of the second
runs: 9 813.8 s, 9 847.8 s and 9 053.4 s, as before.

What does change is the finding text (`p44/finding_text_p44b.txt`, the second runs):

| Rough | Places with a record | Places with no record | Forecast drops; no-field pieces; dropped groups |
|---|---|---|---|
| id 1 (fresh) | 5, 238.5 mm2 | 3, 229.5 mm2 | 4; 1; 2 |
| id 2 | 30, 3 860.5 mm2 | 122, 52.1 mm2 | 53; 4; 231 |
| id 20 (fresh) | 81, 5 289.8 mm2 | 55, 414.0 mm2 | 98; 23; 87 |
| id 21 | 607, 45 005.8 mm2 | 1 177, 411.8 mm2 | 1 022; 257; 1 929 |

- id 2: the no-field pieces go from 39 to 4. The other 35 are regions that the region contour
  cut. The control relief is place 7 (X 159.5 to 168.5, Y 45.9 to 54.1, 45.6 mm2), and its
  reason is now "the plan dropped cut groups shorter than Min Region Cut Length".
- The header lines `geom.rough_left_stock` are the same four as at HEAD.

Gates of round 2, after the last code edit (`p44/gate_p44b2_*.log`):

| Gate | Result |
|---|---|
| `test -p rs_cam_core -q --lib` | 2 862 pass, 1 red: `feeds::envelope::tests::the_envelope_meets_its_time_budget` (known) |
| adaptive3d sentries (37 binaries; D44, D48 and D50 included) | 75 pass, 0 red |
| other core sentries (20 binaries, `findings_transport_join_h21` included) | 141 pass, 0 red |
| dressup sentries (19 binaries) | 119 pass, 0 red |
| FM1 `feeds_matrix_instrument_fm1` | 2 pass |
| op golden | red, 52 cells; the same names and values as round 1 and as the last P43 run. Not blessed |
| `test -p rs_cam_cli -q` | 113 pass, 0 red |
| `test -p rs_cam_mcp -q` | 39 pass, 0 red |
| viz `--lib -- --test-threads 2` (no `rs_cam_gui` process) | 705 pass, 0 red |
| full clippy (the command of the root `CLAUDE.md`) | clean, exit 0 |
| `cargo fmt -p rs_cam_core -- --check` | clean, exit 0 |

No file under `crates/` changed after these gates started (the hash of the `crates` diff is
the same before and after). The disk had 91 GB free at the end.
