# P45: the strip pass of a 3D Rough on remaining stock (2026-11-21)

Status: the product form (build 3) is on the working tree, not committed. Every bar holds;
every red gate is a known red. HEAD is `409e7d474` (P44b landed as `9b5819afd`). The message
and the paths for the commit are in `p45/COMMIT_MSG.txt`.

## 0. What failed and what is not verified

RESULT: the strip pass is on the tree with 0 pieces in the corrected replay class. Three
builds were measured; build 3 is the product form.

Failed on the way (each is fixed in build 3):

- Build 1 and build 2 cut 6.75 mm deep at one place (2 simulation samples; 7 replay nodes
  0.003 to 0.061 mm inside the disc edge, 7.3 to 11.9 mm deep). The class of the bar was 0 in
  both builds, because the bar starts at 0.1 mm. Cause: the state counted a centre exactly on
  the disc edge as cut (3.2). Build 3: 0 such nodes, the deepest cleanup sample is 6.113 mm.
- The prediction for build 2 ("the hold of the shipped chords removes that place") was FALSE.
  The second part of the cause was found only after build 2.
- The prediction P3 of build 1 ("board pocket and cavity under P44 build 2, because the fix
  of the read also fixes D-3") was FALSE. The wood that P44 named for D-3 is the rim of the
  line margin (361.8 of 364.5 mm2 of the board pocket), not a strip that the Z rule holds.

Not verified:

- Three hunks have no test of their own and no red proof: `hold_shipped_chords`; the centre
  test in `wall_read` pass 1 of the strip pass (the EDGE proof reverts the test in
  `StripTops::lower_under` only); the start of the pass 3 ring of `wall_read` for the strip
  pass. All three are in the measured build 3. To remove one changes the G-code (D-7).
- The published rest regions were not checked against the strip pass. The strip pass can cut
  a piece that stays in `left_pieces` (a material cell that the plan left), so the op can
  publish a rest region that it then cut. The region contour removes such a piece (P44b
  D-15 c); the strip pass does not. The D44, D48 and D50 sentries are green (D-10).
- The effect of `hold_shipped_chords` alone is not
  measured: build 2 (with it, without the centre test) kept the deep place, and no build has
  the centre test without it. The G-code of build 2 differs from build 1, so it acts. From
  the trace of 3.2 both parts are needed at X 230.3; that is a reading of the numbers, not a
  measure of build 3 with the hold reverted.
- The wood left on the front: the front has no model frame in the replay instruments (as P43
  and P44). The rows, the replay class on a flat block, the collisions and the moves are
  measured.
- Build 3 was not run with `--sim-artifact`; the deepest cleanup sample (6.113 mm) is from
  the limit row of `summary.json`.
- The reasons of two small pieces that stay (8.6 mm2 and 2.5 mm2, section 5.5) are from the
  geometry, not from a trace per point.
- No GUI or MCP surface was run, and no `rs_cam_gui` was built: a running GUI is not this
  tree. `target/release/rs_cam_cli` is the build of this tree less the `cargo fmt` line.
- The op golden was not run on HEAD in this session; its 52 red cells have the names of
  `p44/golden_p44b_cells.txt` and its log is the P44b round 2 log line for line.
- The cause of D-5 (the lib parity fixture) is not found (1.4): only its direction, its
  count with and without the wall pass, and its place.

Rule breaks of this session (no harm found):

- One `cargo test` (the first run of the `_p45` tests) started with 15 GB of available
  memory; the bar is 18 GB. After that each job waited for 18 GB (`waitmem.sh`).
- One wait loop used `pgrep -f` (a forbidden form); I stopped it after seconds and used a
  file test.
- The build 2 runs of rivmap100 and of the back of the committed job did not run: no 18 GB of
  free memory for 3 hours (other sessions). Build 3 has all the runs.

The disk had 77 GB free at the end (`df -h .`); the bar is 15 GB.

Known reds, not from this phase: `feeds::envelope::tests::the_envelope_meets_its_time_budget`
(lib) and the 52 cells of the op golden.

## 1. Step 1: the trace of the 4 replay pieces of build 2

### 1.1 The method

- The probe is `p45/probe1.patch` on HEAD: the strip pass of `p44/not_landed/p44_build2.patch`
  (`strip_pass`, `grow_open_side`, `contour_line_length`), ported by hand with the OLD read,
  plus prints behind the env `P45PROBE` (probe only; it is not in the product).
- The probe is the same program as build 2 for id 2: the G-code of "Back rest 6 mm" of the
  probe run is the same as that of `p44/out_b2.nc`, line for line (18 742 lines). The replay
  class gives the same 4 pieces (`p45/deepc2_probe1.txt`).
- Per round of the strip pass the probe writes the planner stock before the round (the ray top
  and `DexelGrid::conservative_top` of each cell), each ring point (XY, its own Z read, the Z
  after the neighbour rule, standing, refused) and the runs that ship.
- `p45/trace.py` joins these with the replay (`p37/gapmap.py`, a point sample at each node).
  The planner grid has its origin at (-18.5, -8.5) in the setup frame and a cell of 0.2 mm, so
  each replay node (G-code frame, 0.2 mm) IS a planner cell centre.
- For each counted node the trace gives one of three classes. A: the planner ray top is under
  the replay top (a blended cell). B: no ring point holds the cell centre in its disc (the
  shipped chord left the ring). C: the reads hold the cell and the chord between two points
  is under them. Output: `p45/trace_probe1.txt`.

### 1.2 The result: class A for 11 of 11 nodes

All 4 pieces are moves of round 0 of the strip pass. 9 rounds run (13, 4, 2, 2, 2, 2, 2, 2,
0 runs; 38 123 ring points, 14 386 stand in round 0).

| Piece | Node (G-code frame) | Replay top | Planner ray top | `conservative_top` | Own Z of the ring points that hold the centre | Cut to | Lowered |
|---|---|---|---|---|---|---|---|
| 17425 (346.6, 97.48, 8.4) to (346.6, 98.08, 13.88) | (349.4, 98.6) | 16.55 | 13.31 | 16.55 | 8.40 to 13.88 (5 points) | 8.78 | 7.77 |
| 17475, its mirror at X 48.4 | (45.6, 91.4) | 16.55 | 13.31 | 16.55 | 8.40 to 13.88 | 8.78 | 7.77 |
| 17440 (230.2, 100.6, 11.19) to (229.8, 100.6, 17.75) | (228.6, 103.2) | 19.50 | 14.56 | 19.50 | 11.19 to 18.53 (7 points) | 12.89 | 6.61 |
| 17459 (50.8, 100.6, 11.0) to (50.48, 100.8, 11.0) | (49.8 to 51.2, 103.6), 8 nodes | 19.50 | 11.72 to 13.76 | 19.50 | 11.00 to 14.50 (4 to 7 points) | 11.00 | 8.50 |

Facts:

- At each of the 11 nodes the planner ray top is 3.24 to 7.78 mm UNDER the replay top. The
  ring points that hold the node in their disc read that ray top, so their Z is the ray top
  less one Depth/Pass (6 mm), not the wood less one Depth/Pass. Example, piece 17459: the ray
  tops are 11.72 to 13.76, so the Z is the floor 11.0; the wood stands at 19.50; the cut takes
  8.50 mm.
- `conservative_top` of each of the 11 cells is equal to the replay top (16.55 or 19.50).
- Why the ray top is low: earlier cuts of id 2 itself (the plan and the wall pass of levels
  19.5, 16.55, 13.5, 11.0, 8.55 and 8.4) passed with the disc edge 0.000 to 0.162 mm OUTSIDE
  the node. Each such cut covers the cell in part, and the stamp lowers the ray by the covered
  fraction of the stock above the cutter (`stock::dexel::ray_blend_above`). The node itself is
  outside each of those discs, so the replay does not lower it. Example, node (50.0, 103.6):
  3 moves at Z 19.50 with the node 0.087 to 0.105 mm outside the edge, 2 at 16.55 (0.000 mm),
  4 at 13.50 (0.000 to 0.161 mm), 3 at 11.00 (0.017 to 0.105 mm). The ray top went from 19.5
  to 12.09; the replay top stayed at 19.50.
- `conservative_top` is lowered only by a stamp that covers the cell in full
  (`dexel_stock::stamping::FULL_COVERAGE`). So it keeps the tall value of a part-covered cell.
- Classes B and C: 0 nodes. The 3 climbing pieces (17425, 17440, 17475) are also class A: the
  low end of the climb has the wrong Z because its ring points read the blended cell.
- So the reading of P44 section 6.3 is now a fact: the cause is the blended ray top of a cell
  that earlier discs covered in part, read by a contour on a NEW line that holds the cell
  centre inside its disc. The blend came from the cuts of id 2, not from the simulation stock
  of id 1.
- Why HEAD (P42 wall pass, P44b region contour) has 0 pieces with the same read: their line
  at one wall is the same line at each level, so a cell at the disc edge of one level is at
  the disc edge of the next level too. The strip pass runs on a new line (the grown band).

### 1.3 The size of the fault in round 0 (`p45/static_est.txt`)

A static read of round 0 on the probe dumps: with `conservative_top` in place of the ray top,
the Z of round 0 rises by more than 0.2 mm at 1 041 of the 14 386 standing points (by more
than 5 mm at 879). The replay class counts only 11 nodes because the bar ignores a node less
than 0.1 mm inside the disc edge. So build 2 cut deeper than one Depth/Pass into slivers on
about 208 mm of line; 4 pieces were wide enough to count.

At 1 189 standing points the tallest `conservative_top` is on a cell whose centre is in the
disc and that the disc covers only in part. No stamp on the same line ever covers such a cell
in full. So a read of `conservative_top` alone holds the Z of those points for all rounds
(one Depth/Pass under a sliver), and the strip under it is not cut. That is D-3 made worse.

### 1.4 D-5 (the lib parity fixture with a wall pass on)

`p45/d5_probe.sh` runs `planner_sim_dexel_parity_geodesic_adaptive_world_stock_declared` two
times with the map (`RS_CAM_PARITY_MAP`): as it is (no wall pass), and with
`OnePassHold::FreshStock` (a temporary edit of the test helper, restored after). Outputs
`p45/d5_off.txt`, `p45/d5_on.txt`.

| | cells that differ by more than one cell (0.53 mm) | interior | direction | largest |
|---|---|---|---|---|
| no wall pass (the test at HEAD) | 99 of 7 921 | 43 | the planner stock is TALLER in all 99 (the simulation removed more); 0 the other way | 12.924 mm |
| the fresh wall pass on | 47 of 7 921 | 24 | the planner stock is TALLER in all 47; 0 the other way | 12.924 mm |

- The direction is the opposite of the replay failure. There the planner stock was LOWER
  than the wood (a blend). Here the planner stock is taller than the simulated stock.
- The gap is not a gap of the `emit_runs` door: the fixture has 99 such cells with NO wall
  pass, and the wall pass takes 52 of them away. The record of P44 (section 0: "with a wall
  pass on ... 47 cells") gave the number with the pass on and did not give the number with
  the pass off.
- Where: single cells on the ring at the foot of the dome and on four arcs at the corners of
  the grid (the map in `p45/d5_on.txt`). Each is a cell at the edge of the cut area.
- The cause is not found. It is another mechanism than the blended read, and I left it.
  The sentence "no lib parity test runs with `one_pass_hold` on" of the folder file stays.

## 2. Step 2: the design (written before the build)

The read at its source: a contour point must read the stock at each cell centre in its disc,
not the blend. Two facts make an exact upper bound of the stock at a cell centre:

1. `conservative_top` is an upper bound of the stock anywhere in the cell (the stamp lowers
   it only at full cover). It is in the stock today; no new storage in the stock.
2. A fed chord lowers the stock at a cell centre to the cutter surface when the centre is
   inside the swept disc. That is geometry (the replay uses the same rule).

The strip state (`level_contour::StripTops`, new):

- At the start of the strip pass: `conservative_top` of each cell that holds stock (never
  under the ray top); the ray read for an empty cell.
- After each round: lowered under each cut that the round shipped, along the path that the
  emitter ships (`clearing::shipped_cut_path`: the simplified chord), at each cell whose
  centre is inside the swept disc, to the chord Z at the nearest point plus the profile
  height. The C4 band state has the same form (`BandEdge::scan`, `lower_along`).
- `wall_read` pass 1 of the strip pass reads the state in place of the ray top. The Z rule
  does not change: the level floor, raised to one Depth/Pass under the tallest read in the
  disc.
- The stock that a point of the strip pass reads as standing (`WallRead::disc_top`) counts
  each cell whose centre the disc holds (was: the cells that the disc covers in full). With
  the state, the cut of a round lowers each such centre, so the next round reads the cut and
  goes down one Depth/Pass. A sliver at the disc edge then costs one round; it does not hold
  the Z for all rounds (1.3).
- No constant: the tool radius, the cell and `conservative_top` are all in the tree.
- Scope: the strip pass only. The P42 wall pass, the P43 fresh wall pass and the P44b region
  contour keep their read, so ids 1 and 20 keep their bytes, and id 2 before its strip pass
  is the plan of HEAD. See decision D-1 (section 8).

D-3 (the Z rule beside a tall rim): the rim column at a wall is a cell whose centre is 0.1 mm
inside the disc of the line. It is part-covered, so the old read saw it lowered only by a
fraction per round, and the pass stopped. With the state the rim centre reads the cut of the
round before. So the fix of the read should also fix D-3 where the rim centre is inside the
shipped disc. This is a prediction (P3).

The strip pass itself is that of build 2: the standing cells of the bottom level with the
strip read (`standing::standing_cells`), the bands under the cut-length dial dropped and
recorded, the bands grown on the open side (`grow_open_side`), the region contour along each
band (`ContourRule::Region`), on remaining stock only (`OnePass::on`). A band with no room
for a line is recorded as `LeftWhy::NoLine` (`PlaceWhy::NoLine`).

### 2.1 Predictions (before the build; setup "1 Back", `p44/job_no41.toml`)

| # | Prediction |
|---|---|
| P1 | Replay class of id 2: 0 pieces (build 2: 4). Id 1: 0. Risk: a shipped chord that leaves the ring points by the cut tolerance at a corner (class B). |
| P2 | Rows of id 2: body 6.000 (not changed), cleanup at most 6.5, entry 1.885. Id 1: the same G-code as HEAD. |
| P3 | Wood where the 6 mm tool fits after id 2: total 1 100 to 1 500 mm2 (build 2: 1 475.8; P44b: 4 463.2). Board pocket and cavity under build 2 (the D-3 walls go down). Places 1 and 3 as build 2 (59.8, 121.8). |
| P4 | Id 2: more rounds than build 2 (9), feed time 950 to 1 100 s (build 2: 902.4; P44b: 676.9). |
| P5 | 0 collisions; the 8 gouge nodes of HEAD, no more. |
| P6 | rivmap100: the same bytes as HEAD (Contour Parallel; no strip pass). Ids 1 and 20: the same bytes as HEAD. Id 21 moves. |

## 3. Build 1 (the strip state; `p45/builds/b1.patch`), measured 2026-11-21

Setup "1 Back", `p44/job_no41.toml`. Outputs: `p45/meas_b1.txt`, `p45/stays_b1.txt`,
`p45/graze_b1.txt`, `p45/band_b1.txt`.

| | P44b (HEAD) | P44 build 2 | build 1 |
|---|---|---|---|
| replay class, id 1 and id 2 (pieces) | 0, 0 | 0, 4 | 0, 0 |
| wood where the 6 mm tool fits after id 2, total (mm2) | 4 463.2 | 1 475.8 | 1 485.0 |
| id 2 moves; fed; feed time | 17 294; 44.4 m; 676.9 s | 18 737; 52.7 m; 902.4 s | 19 115; 53.3 m; 908.5 s |
| id 2 body row, deepest; cleanup row, deepest; entry | 6.000, 6.130; 5.000, 6.001; 1.885 | 6.000, 6.130; 5.000, 6.320; 1.885 | 6.000, 6.130; 5.000, 6.750; 1.885 |
| id 2 gouge nodes 0.1 mm or more inside the edge | 8 | 8 | 8 |
| setup "1 Back" | 9 813.8 s | 10 003.2 s | 10 001.9 s |
| collisions, rapid collisions, failed checks | 0 | 0 | 0 |

Verdicts: P1 TRUE (0 pieces). P2 TRUE for the rows; the deepest cleanup sample is 6.750 mm
(see below). P3: the total is in the range (1 485.0); "board pocket and cavity under build 2"
is FALSE (364.5 against 364.2; 769.9 against 762.0): the wood that build 2 left at those walls
is the rim, not a strip that the Z rule holds (3.1). P4 FALSE: 908.5 s, under the range. P5
TRUE.

### 3.1 What stays is the rim (`p45/stays.py`)

After id 2, of the 1 485.0 mm2 where the tool fits, 1 379.5 mm2 are at most 0.4 mm from a wall
(a step of more than 1 mm in the reach surface): the margin of a contour line (D19, one
planner cell plus the cut tolerance; lead ruling D-12 of P44b: the line stays). The rest is
105.6 mm2 in 48 pieces. At P44b the rest is 2 535.8 mm2 in 260 pieces. So the D-3 reading of
P44 ("the Z rule beside a tall rim holds the pass at 19.5; 175 mm2 of the board pocket are
never cut") names the rim itself: 361.7 of the 364.5 mm2 of the board pocket are rim.

### 3.2 A second fault, found in build 1: a centre on the disc edge

`p45/graze_b1.txt`: 10 pieces of id 2 lower a node by more than 6.5 mm; no node is 0.1 mm
or more inside the edge, so the class is 0.

- 8 pieces (874 nodes): the node is exactly ON the disc edge (0.0000 mm inside). The line
  stands on a grid coordinate (X 348.6), the tool radius is 3.0, so the edge passes through a
  node column (X 351.6). The replay counts a node at the edge as cut (`d2 <= R * R`); no wood
  is there. 3 of these pieces are moves of the plan of HEAD.
- 2 pieces (7 nodes), 0.003 to 0.061 mm inside the edge, 7.28 to 11.86 mm deep: pieces 18784
  and 18792 at X 230.2 to 230.3, Y 56.8 to 61.4 (round 1 of the strip pass, Z 13.2). The
  product simulation shows the same place: 2 samples of move 18 928 at 6.75 mm (the deepest
  cleanup sample of the table; found with `--sim-artifact`).

The cause has two parts. Both are needed at this place.

1. The corner. The ring has a corner point at (230.200, 59.800). In round 1 the emitter ships
   the chord (230.311, 59.443) to (230.200, 61.387): its simplification (`simplify_path_3d`
   at the cut tolerance 0.1 mm) drops the corner point, which is 0.09 mm from the chord. In
   round 0 the Z of the points differs and the corner point ships. So the chord of round 1
   holds the 6 centres of the column X 233.2 (Y 60.4 to 61.4) 0.004 to 0.061 mm inside its
   edge.
2. The edge. Those 6 centres are exactly on the disc edge of the path of round 0 (line X
   230.2, radius 3.0, column X 233.2: 0.0000 mm). The state lowered them to the Z of round 0
   (the test was `d_sq <= r_sq`). The replay did not cut them in round 0: their tops before
   round 1 are 20.50 to 25.07. So in round 1 the state read them 6 mm lower than the wood.

Build 2 (`p45/builds/b2.patch`) added only part 1: `hold_shipped_chords`. Before a
run of the strip pass ships (in `emit_runs`, after the run has its direction and its start
point), the planner simplifies the run as the emitter does (`simplify_path_3d_keep_mask`,
the same tolerance), reads the one-pass Z of `wall_read` at one cell along each chord, and
raises each point under a chord with a higher read. It repeats on the raised run until no
chord changes. No constant.

The predictions for build 2 were: the replay class stays 0; the 2 pieces of 0.003 to
0.061 mm are gone; the deepest cleanup sample is at most 6.5 mm; the wood total moves by less
than 10 mm2; the feed time of id 2 moves by less than 20 s.

## 4. Build 2 (the hold of the shipped chords), measured 2026-11-21

Setup "1 Back", `p44/job_no41.toml` (`p45/meas_b2.txt`, `p45/graze_b2.txt`,
`p45/stays_b2.txt`, `p45/band_b2.txt`).

| | build 1 | build 2 |
|---|---|---|
| replay class, id 1 and id 2 (pieces) | 0, 0 | 0, 0 |
| wood where the 6 mm tool fits after id 2, total (mm2) | 1 485.0 | 1 485.0 |
| id 2 moves; deepest cleanup sample | 19 115; 6.750 | 19 115; 6.750 |
| pieces with a node over the bar, 0.003 to 0.061 mm inside the edge | 2 (7 nodes) | 2 (7 nodes), the same chord |
| setup "1 Back" | 10 001.9 s | 10 002.9 s |

Verdict: the prediction "the 2 pieces are gone" is FALSE. The G-code of id 2 differs from
build 1 (from motion line 17 360), so the hold acts; but the chord at X 230.3 is the same
chord at the same Z. The reason is part 2 of 3.2: the state already read the 6 centres as
cut, so the read along the chord saw nothing to hold. Part 2 was found after build 2, by
the distance of the 6 nodes to the path of round 0 (0.0000 mm).

Build 2 on the committed job (`planning/rivmap_block_job/nz_wanaka_21_lead_proposal.toml`),
setup "3 Front": id 20 keeps its motion lines (22 847, the same as HEAD); id 21 goes from
46 432 to 58 904 moves; setup 9 053.4 to 9 381.2 s; 0 collisions; no row of id 21 changes;
the replay class of id 21 (`p43/deepc.py`, bar 6.332, a flat block) has 0 nodes over the bar,
as HEAD. Build 2 was not run on the back of the committed job or on rivmap100 (no 18 GB of
free memory for 3 hours).

Build 2 holds each bar as written for the back with id 41 off. It is the fallback if build 3
fails a bar (`p45/builds/b2.patch`).

### 4.1 Build 3: a centre on the disc edge is on neither side (design, before the build)

A centre exactly on the disc edge is inside or outside by the last bit of the arithmetic, and
the G-code writer rounds each coordinate (the post sets the decimals). So "the cut lowered the
stock at this centre" needs a margin. The margin is from the stamp: its coverage reads 4 x 4
sub-samples per cell, and the four nearest the centre stand one eighth of a cell from it on
each axis (`dexel_stock::stamping::SUBSAMPLE_OFFSETS_FRAC`). New
`TriDexelStock::disc_holds_cell_core`: the disc holds a centre when those four sub-samples
are inside (0.025 mm on an axis, 0.035 mm on the diagonal, at a cell of 0.2 mm; under the
0.1 mm of the replay class).

- `StripTops::lower_under` lowers a centre only with that test (was `d_sq <= r_sq`).
- `wall_read` pass 1 of the strip pass counts a centre only with that test, so the read and
  the state name the same centres. A centre on the edge is in the ring of pass 3 (the
  held-stock read), which starts where pass 1 stops.
- No other read changes.

Predictions for build 3 (setup "1 Back", id 41 off):

| # | Prediction |
|---|---|
| Q1 | Replay class: 0 pieces for ids 1 and 2. |
| Q2 | `graze`: the 2 pieces at X 230.3 (0.003 to 0.061 mm inside) are gone. Pieces with nodes at 0.0000 mm stay (no wood there). A node up to 0.035 mm inside an edge can stay over the bar. |
| Q3 | The deepest cleanup sample of id 2 is at most 6.5 mm (was 6.750). |
| Q4 | Wood total within 20 mm2 of build 2 (1 485.0); id 2 feed time within 30 s of 909 s. |
| Q5 | Ids 1 and 20 and rivmap100 keep the bytes of HEAD. |

## 5. Build 3 (the product form), measured 2026-11-21

Build 3 is build 2 plus the centre test of 4.1. `p45/p45_product.patch` is the final tree
(the diff of `crates` and `FEATURE_CATALOG.md`); after the release build only `cargo fmt`
moved one line of `tests.rs`. Release CLI: scratch `p45/cli_b3`.

### 5.1 Setup "1 Back", `p44/job_no41.toml` (`p45/meas_b3.txt`, `p45/stays_b3.txt`)

Wood more than 0.3 mm over the target where the 6 mm tool fits, after id 2 (mm2):

| Place | P44b (HEAD) | P44 build 2 | P45 build 3 | bar | what stays (P45) |
|---|---|---|---|---|---|
| 1 antenna slot | 59.8 | 59.8 | 59.8 | | rim 45.3; 14.5 at Z 13.5 beside it (P44b D-12, the one-pass hold) |
| 2 control relief | 86.6 | 13.6 | 13.6 | 15.0 | rim only |
| 3 slit slot | 121.8 | 121.8 | 121.8 | | rim only |
| 4 ESP relief | 59.4 | 23.8 | 23.8 | 26.2 | rim only |
| 5 step shelf | 525.7 | 124.6 | 125.6 | 137.1 | rim only |
| 6 board pocket | 861.0 | 364.2 | 364.5 | 400.6 | rim 361.8; 2.8 in 3 pieces |
| 7 cavity | 2 735.8 | 762.0 | 771.2 | | rim 683.0; 88.2 (5.5 below) |
| other | 13.2 | 6.0 | 6.0 | | rim |
| total | 4 463.2 | 1 475.8 | 1 486.4 | 1 600 | rim 1 380.9 + the rest 105.5 |

"Rim": at most 0.4 mm from a wall (a step of more than 1 mm in the reach surface). At P44b
the rim is 1 927.4 mm2 and the rest is 2 535.8 mm2 in 260 pieces.

| | P44b (HEAD) | P44 build 2 | P45 build 3 |
|---|---|---|---|
| id 1 | 36 079 moves, 801.3 s | the same | the same motion lines as HEAD |
| id 1 body row, deepest; cleanup deepest; entry | 4.166, 4.166; 4.233; 1.969 | the same | the same |
| id 2 moves; fed; rapid; feed time | 17 294; 44.4 m; 7.4 m; 676.9 s | 18 737; 52.7 m; 9.7 m; 902.4 s | 19 346; 52.7 m; 8.5 m; 906.2 s |
| id 2 body row, deepest | 6.000, 6.130 | 6.000, 6.130 | 6.000, 6.130 |
| id 2 cleanup row (bound), deepest | 5.000 (3.429), 6.001 | 5.000, 6.320 | 5.000 (3.429), 6.113 |
| id 2 entry row, deepest | 1.885, 1.894 | 1.885, 1.894 | 1.885, 1.917 |
| id 2 link row | 6.000 | 6.000 | 6.000 |
| replay class, id 1 and id 2 (pieces) | 0, 0 | 0, 4 | 0, 0 |
| pieces of id 2 with a node over 6.5 mm (any distance to the edge) | | 28 (77 nodes) | 6 (38 nodes), each node 0.0000 mm inside the edge |
| id 2 gouge nodes 0.1 mm or more inside the edge; nearer | 8; 270 | 8 | 8; 270 |
| setup "1 Back", id 41 off | 9 813.8 s | 10 003.2 s | 10 000.4 s |
| collisions, rapid collisions, failed checks | 0, 0, 0 | 0, 0, 0 | 0, 0, 0 |

- The first 17 297 motion lines of id 2 are the lines of P44b: the strip pass is the last
  2 054 lines of the op. No row of the setup changes state against P44b
  (`p45/rows_scratch_job_b3.txt`).
- Verdicts of 4.1: Q1 TRUE. Q2 TRUE: the 2 pieces at X 230.3 are gone; the 6 pieces that stay
  have each node exactly on the disc edge (`p45/graze_b3.txt`; 3 are moves of the plan of
  HEAD, 3 are moves of the strip pass on the lines X 348.6, X 345.0 and X 50.0). Q3 TRUE
  (6.113). Q4 TRUE (1 486.4 mm2; 906.2 s). Q5 TRUE (5.2, 5.3).
- Other toolpaths against P44b: "Cavity rest pocket 6 mm" 875 to 167 moves (its entry bound
  4.44 to 16.44, within), and its header line `project.sim_outside_envelope` goes; "Lake
  outline channel 6 mm" 6 545 to 4 663 moves; "River channel 6 mm, pass 7 of 7" 46 983 to
  46 958 moves. No other header line changes.

### 5.2 The committed job (`planning/rivmap_block_job/nz_wanaka_21_lead_proposal.toml`)

| | P44b (HEAD) | P45 build 3 |
|---|---|---|
| setup "1 Back" | 9 847.8 s | 10 034.4 s |
| id 1 and id 2 | | the same motion lines as with id 41 off (36 080 and 19 351) |
| replay class, id 1 and id 2 (pieces); gouge nodes of id 2 in the class | 0, 0; 8 | 0, 0; 8 |
| setup "3 Front" | 9 053.4 s | 9 353.1 s |
| id 20 "Front bulk rough 18 mm" (fresh) | 22 842 moves | the same motion lines as HEAD |
| id 21 "Front 3D rough" moves; fed; rapid; feed time | 46 432; 64.8 m; 20.0 m; 1 020.1 s | 57 805; 78.5 m; 20.9 m; 1 228.8 s |
| id 21 body row, deepest | 4.701, 4.701 | 4.701, 4.701 |
| id 21 cleanup row (bound), deepest | 3.725 (3.012), 5.130 | 3.725 (3.012), 5.130 |
| id 21 entry row, deepest; link row, deepest | 1.716, 1.881; 0.480, 0.487 | 1.716, 1.881; 0.480, 0.487 |
| id 21 replay class (`p43/deepc.py`, bar 6.332, a flat block) | 0 nodes over the bar | 0 nodes over the bar |
| "Front scallop finish" deepest body sample (no bound) | 4.714 | 4.632 |
| collisions, rapid collisions, failed checks (both setups) | 0, 0, 0 | 0, 0, 0 |

No row changes state on either setup (`p45/rows_back_committed_b3.txt`,
`p45/rows_front_b3.txt`). No header line changes on the front. The finding of id 21 names
1 549 places with a record (43 794.4 mm2 of tool-centre area; HEAD: 607 places,
45 005.8 mm2) and 108 with no record (111.0 mm2; HEAD: 1 177, 411.8 mm2). The drop records go
from 1 279 to 3 323 (3 066 under the dial, 257 with no field): each band or run of the strip
pass under the dial writes a record, so a rim place that had no record now has one. That is
a reading of the counts (D-4). Id 2: 30 to 349 places with a record, 122 to 145 with none. The front has no model
frame in the replay instruments (as P43 and P44), so the wood left there is not measured.

### 5.3 rivmap100

`planning/fixtures/rivmap100/rivmap100_live_0925.toml`, read in place: the G-code of build 3
is the same as HEAD, byte for byte, the header included (`cmp` with scratch
`p44b/riv_head.nc`). The op is Contour Parallel on a fresh block: no strip pass runs.

### 5.4 The band at the bottom edge of the cavity (operator remark on the P44b render)

Window in the G-code frame (the frame of the render): X 45 to 350, Y 105 to 120.
Instrument: `p45/band.py` (outputs `p45/band_p44b.txt`, `band_build2.txt`, `band_b1.txt`,
`band_b2.txt`, `band_b3.txt`).

| | P44b (HEAD) | P44 build 2 | P45 build 1 | P45 build 2 | P45 build 3 |
|---|---|---|---|---|---|
| wood where the tool fits after id 2 (mm2) | 976.1 | 118.6 | 129.7 | 129.7 | 129.7 |
| tallest (mm) | 8.94 | 8.11 | 8.11 | 8.11 | 8.11 |
| by height: 0.3 to 1; 1 to 3; 3 to 6.5; over 6.5 mm | 185.5; 138.8; 637.1; 14.6 | 0.3; 2.8; 114.3; 1.2 | 0.2; 13.9; 114.4; 1.2 | the same | the same |
| fed runs of the strip pass in the window; entries in the window | no strip pass | 2; 0 | 2; 0 | 2; 0 | 2; 0 |
| the longest run; the others | | 309 mm; 4 mm | 309 mm; 4 mm | 309 mm; 4 mm | 309 mm; 4 mm |

- The "nibbled" wood of P44b is the strips that no level plans: 976.1 mm2, most of it 3 to
  6.5 mm tall.
- The strip pass takes it with ONE pass along the wall: the move (50.8, 113.4, 20.5) to
  (348.6, 113.4, 20.5), 297.8 mm in one G1 line, at the shelf floor Z 20.5, in one round (the
  wood is under one Depth/Pass). The run enters the window from outside (0 entries in it).
  The band is not split: not by the cut-length dial, not by `grow_open_side`, not by the
  rounds, not by the rim Z rule.
- What stays in the window (129.7 mm2): the rim. Two node columns along 305 mm: Y 110.2
  (5.0 mm tall, 0.2 mm from the wall of the stock top) and Y 118.2 (4.0 mm tall, at the step
  from Z 20.5 to 16.5); 114.4 mm2. That is the margin of the contour line (D19, ruling D-12).
  Plus 13.9 mm2, 2.1 mm tall, at X 281 to 290, Y 118.4 to 119.8, a lower step (floor 12.38).
  There the long run (3 041.9 mm, at Y 121.4, Z 16.5) dips to Z 14.5 from X 287.24 to 281.84:
  one Depth/Pass under the 20.5 of the shelf rim in its disc. No move of the strip pass at
  Z 12.4 follows on that line. A run of the next round there is 5.4 mm long, under the dial
  of 15 mm, and `contour_rounds` reads the dial on each run of each round. This fits the
  G-code; no record of that drop was read (D-9).
- The finding of the product names that rim as place 3: "X32.5..330.3 Y104.7..108.1 (setup
  frame), 60.3 mm2, 5.00 mm above the cutter: a cut there is shorter than Min Region Cut
  Length". The reason text is that of a record on its cells (a run under the dial at one end);
  the true reason of the rim is the line margin. See D-4.

### 5.5 The strip places that stay, with the reason (bar 6)

Of the 1 486.4 mm2, 1 380.9 mm2 are the rim: at most 0.4 mm from a wall. Reason: the contour
line stands one planner cell plus the cut tolerance inside the limit of the tool centre
(`contour_line_inset`, D19), so the disc edge stands 0.2 to 0.4 mm from each wall, at each
wall of each place. The lead ruled that the line stays (P44b D-12).

The other 105.5 mm2 are 46 pieces (`p45/stays_b3.txt` lists 30). Each piece over 1 mm2:

| Piece (G-code frame) | mm2 | tallest | Reason |
|---|---|---|---|
| X 88.6..97.8, Y 237.2..250.2 (a pocket 7.6 mm wide in the cavity, floor Z 7.27) | 71.2 | 4.99 mm at its wall; 1.28 mm over the pocket floor (Z 8.55 over 7.27 on the row Y 243.0) | NOT a strip. The plan cuts it to Z 8.55 (a Detect Flat level). At the last level its region is 1.6 mm wide and the plan has a drop record there (the finding does not say which of its 205 dial records and 4 no-field records). The product finding names it ("1.29 mm above the level floor at X72.70 Y239.10", setup frame). The same wood stands at P44b (87.2 mm2 in the box) |
| X 281.2..290.4, Y 118.6..120.8 | 8.6 | 4.13 mm | the lower step at the cavity edge: round 0 runs at Z 14.5 for 5.4 mm (one Depth/Pass under the shelf rim at 20.5); a run of the next round is 5.4 mm long, under the dial of 15 mm (5.4, D-9; read from the G-code, not from a record) |
| X 247.0..288.0, Y 62.4 (antenna slot, one node row) | 8.2 | 9.00 mm | the row beside the rim at Z 13.5: the one-pass hold of the levels (P44b D-12); the slot is a region, the strip pass makes no band there |
| X 350.4..350.6, Y 232.6..240.0 | 2.5 | 3.25 mm | a piece 0.4 to 0.6 mm from a wall, just outside the 0.4 mm rim window; not traced |
| X 223.8..225.0, Y 50.0..56.4 (board pocket) | 2.0 | 2.50 mm | not cut by id 2; the cause is not traced (a band of this length, 6.4 mm, is under the dial) |
| X 142.6..146.4, Y 167.0..169.2 | 1.8 | 4.94 mm | a pocket end as the first row (floor Z 7.46, cut to a level above it) |
| 4 corners of the antenna slot (X 244.2..246.6 and 288.4..291.0, Y 54.8..62.2) | 5.6 | 9.0 to 12.05 mm | the corners of the slot beside the rim (the one-pass hold, as the row above) |
| X 100.0..103.0, Y 234.4..236.4 | 1.0 | 4.99 mm | the same pocket as the first row |

The reasons of rows 2, 4 and 6 are from the geometry of the replay and the G-code, not from a
trace per point. The product finding (`p45/finding_text_b3.txt`) lists 349 places with a drop
record (867.2 mm2 of tool-centre area; P44b: 30 places, 3 860.5 mm2) and 145 with no record
(112.0 mm2).

## 6. Tests (bar 7) and the red proofs

New, in `adaptive3d/tests.rs` and `adaptive3d/level_contour.rs`:

| Test | What it holds | Hunk reverted alone (`p45/revert.py`) | Red output (`p45/red_proofs.txt`) |
|---|---|---|---|
| `a_strip_beside_a_wall_with_no_region_is_cut_p45` | a strip 2 mm wide and 6 mm tall at the foot of a wall is cut to the floor, one Depth/Pass per round; the control (no strip pass) keeps 6.00 | PASS: no strip pass call in `path.rs` | "the strip stands at Z 6.00 between X 8.0 and 9.4; the floor is 0.5" |
| `the_strip_pass_reads_the_stock_at_the_cell_centre_p45` | the fixture of the replay failure: a sliver column 9 mm tall whose ray top is a blend (Z 5.33; three earlier cuts with the disc edge 0.05 mm outside its centre); no fed move lowers a node more than 3.5 mm | READ: `wall_read` reads the ray top for the strip pass | "a move cuts a node 6.67 mm deep; one Depth/Pass and the clearance are 3.5 mm" |
| `the_strip_state_follows_each_round_p45` | on the same fixture the sliver column goes down to the floor | SCAN: no `StripTops::scan` after a round | "the strip stands at Z 6.00 between X 8.0 and 9.4; the floor is 0.5" |
| `the_strip_state_does_not_count_a_centre_on_the_disc_edge_p45` | `StripTops::lower_under` does not lower a centre exactly on the disc edge | EDGE: the test is `d_sq <= r_sq` again | "the centre on the disc edge keeps its stock: 4" |

- The depth of the two fixture tests is a point sample at each cell centre of the moves of
  the toolpath (the rule of `p43/deepc.py`, written in the test). It does not read the
  planner stock.
- After each proof `red.sh` restores the files and compares `git diff` before and after:
  "tree restored" four times.
- `hold_shipped_chords` has NO test of its own and no red proof. See section 0.
- Changed: the control of `each_level_cuts_to_the_wall_that_it_can_reach_p42` turns the strip
  pass off too (with the wall pass off, the strip pass cuts the strip at the end of the op:
  0.2 mm against 0.2 mm). The helper `Standing` literal of
  `stock_with_no_drop_record_does_not_raise_the_finding_p44` has the two fields again.

## 7. Gates and bars

Each gate ran as `timeout <s> scripts/cargo_lane.sh <args> -j 2` on the final tree, after the
last code edit (`p45/gates_p45.sh`; summary `p45/gates_b3.txt`). The hash of the diff of
`crates` and `FEATURE_CATALOG.md` is the same before and after the gates. The red proofs and
the D-5 probe ran before them and restored the tree.

| Gate | Result |
|---|---|
| `test -p rs_cam_core -q --lib` | 2 866 pass, 1 red: `feeds::envelope::tests::the_envelope_meets_its_time_budget` (known) |
| adaptive3d sentries (37 binaries) | 75 pass, 0 red |
| other core sentries (20 binaries) | 141 pass, 0 red |
| dressup sentries (19 binaries) | 119 pass, 0 red |
| FM1 `feeds_matrix_instrument_fm1` | 2 pass |
| op golden `every_op_keeps_its_toolpath_g_opgolden` | red, 52 cells: the same names as `p44/golden_p44b_cells.txt`, and the log is the same as the P44b round 2 log line for line (only the thread id differs). Not blessed |
| `test -p rs_cam_cli -q` | 113 pass, 0 red |
| `test -p rs_cam_mcp -q` | 39 pass, 0 red |
| viz `--lib -- --test-threads 2` (no `rs_cam_gui` process) | 705 pass, 0 red |
| full clippy (the command of the root `CLAUDE.md`) | clean, exit 0, on the final tree |
| `cargo fmt -p rs_cam_core -- --check` | clean, exit 0 |

| Bar | Result (build 3) |
|---|---|
| 1. wood after id 2: total at most 1 600 mm2 | HELD: 1 486.4 (`p44/job_no41.toml`). On the committed job ids 1 and 2 have the same motion lines, so the wood after id 2 is the same |
| 1. control relief, ESP relief, step shelf, board pocket at most build 2 plus 10 % | HELD: 13.6 (15.0), 23.8 (26.2), 125.6 (137.1), 364.5 (400.6) |
| 2. replay class 0 pieces, ids 1 and 2 | HELD: 0 and 0 (both jobs) |
| 2. id 1 body at most 4.666; id 2 body and cleanup at most 6.5; entry at most 2.5 | HELD: 4.166; 6.000 and 5.000 (deepest samples 6.130 and 6.113); 1.885 (deepest 1.917) |
| 3. 0 collisions, 0 rapid collisions; gouge nodes of id 2 in the class at most 8 | HELD: 0, 0; 8 |
| 4. front: no depth row worse; fresh roughs keep the bytes of HEAD | HELD: no row of id 21 changes; ids 1 and 20 keep each motion line. Id 21: 46 432 to 57 805 moves, feed time 1 020.1 to 1 228.8 s, setup +299.7 s |
| 5. rivmap100: the same bytes as HEAD | HELD |
| 6. the finding names each strip place that stays, with the reason | 5.5. Two small pieces have a reason that is not traced |
| 7. tests `_p45`, each proven red | 4 tests, 4 red proofs (section 6). `hold_shipped_chords` has none |
| 8. gates | every red is known |

## 8. Decisions for the lead

- D-1. The scope of the new read. The strip state (`StripTops`) and the hold of the shipped
  chords act in the strip pass only. The P42 wall pass and the P44b region contour keep the
  ray-top read. They have 0 replay pieces on the three runs, and their line is the same line
  at each level of a wall, which is why the blend does not bite. But the fault is in the read,
  not in the pass: any contour on a line that earlier cuts did not use can meet it. Choices:
  (a) leave it (this commit); (b) a phase that gives the two other contours the state of
  their op (the state must then live across the levels, in `ClearZLevelContext`; ids 2 and 21
  change before their strip pass; new measures of the three jobs).
- D-2. The margin of "the disc holds the centre" is one eighth of a cell on each axis (the
  inner sub-samples of the stamp; 0.025 mm on an axis at a cell of 0.2 mm). A centre nearer
  to the disc edge than that is not read and not lowered. On a rest op with a larger
  simulation cell the margin grows with the cell (0.0625 mm at 0.5 mm). The replay class
  counts from 0.1 mm. If the lead wants the margin tied to the cut tolerance and not to the
  cell, that is one function (`stamping::point_holds_cell_core`).
- D-3. A node exactly on the disc edge. 6 pieces of id 2 (38 nodes) lower a node by more than
  6.5 mm with the node 0.0000 mm inside the edge; 3 of them are moves of HEAD. No wood is
  there (the disc edge has no width), and the class does not count them. They exist because
  a line on a grid coordinate with a tool radius that is a multiple of the cell puts the edge
  on a node column. No change proposed.
- D-4. The reason text of a rim place. The finding gives a rim place the reason of a record
  on its cells ("a cut there is shorter than Min Region Cut Length" for the 298 mm rim of the
  cavity edge band). The true reason is the margin of the contour line. The strip pass adds
  many such records (id 21: 1 279 to 3 323), so more rim places carry that text. Choice: a reason of
  its own for a place that is all rim (`PlaceWhy`), which needs a wall test in `left_stock`;
  not built.
- D-5 of P44 (the parity fixture): see 1.4. The gap is there with no wall pass (99 cells) and
  is smaller with it (47). The planner stock is the taller one. It needs its own trace; it is
  not a fault of the `emit_runs` door.
- D-6. The pocket 7.6 mm wide in the cavity (71.2 mm2, 1.28 mm over its floor; 5.5): the
  cut-length dial drops its last level. The same at HEAD. A lower `min_region_cut_length_mm`
  on the job cuts it (P44 D-6).
- D-7. Three hunks have no test of their own (section 0): `hold_shipped_chords`, the centre
  test in the strip read, the pass 3 ring. Choices: (a) land them as measured (build 3 holds
  each bar with them); (b) a follow-up that builds a `ClearZLevelContext` for tests (no test
  helper builds one outside `path.rs` today) and gives each a fixture.
- D-9. The cut-length dial is read on each run of each round of a contour. A stretch that
  shipped inside a long run in round 0 can be a short run of its own in round 1, and the dial
  then drops it (13.9 mm2 in the cavity edge band, 5.4). Choices: (a) leave it (the operator
  dial); (b) a run of a later round ships when its points shipped in an earlier round (the
  entry is then the cost of finishing a place that is begun); not built, not measured.
- D-10. The strip pass and the published rest regions (section 0). Choice: remove a left
  piece that the strip pass cut, as D-15 (c) of P44b does for the region contour; it needs
  the piece-to-run join of `region_contour_pass` for the bands, and a look at a consumer job.
- D-8. With this change "Cavity rest pocket 6 mm" has 167 moves (875 at HEAD) and "Lake
  outline channel 6 mm" 4 663 (6 545): the strip pass takes wood that these ops took. The job
  file is not changed.
