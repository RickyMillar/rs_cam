# P43: the one-pass promise on a fresh rough (2026-11-20)

Job: `planning/rivmap_block_job/nz_wanaka_21_lead_proposal.toml` at `463be53ee` (HEAD), read in
place. Positions are in the G-code frame (setup frame + (18.5, 8.5)) unless the text says
"setup frame".

Scratch dir (a restart of the machine deletes it):
`/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/ce846d39-41d4-4e44-be56-d5dd5295a423/scratchpad/p43/`.
`phases/p43/` holds the instruments of this note (`strip.py`, `classes.py`, `samples.py`,
`run_cli.sh`), the probe patches (`probe2.patch`, not in the tree) and their outputs. Run the
instruments from a scratch dir that holds copies of `p37/gapmap.py`, `p41/deep.py`,
`p41/lim.py`, `p40/meas.py`, `p40/left.py` (`gapmap.py` writes `target.npy` beside itself).

Sections 1 to 4 were written before a line of the rule was written. Section 0 is the summary
and was written last.

## 0. Summary: what failed, and what is not verified

NO code of P43 is in the tree. The tree is HEAD (`463be53ee`) plus this note and `phases/p43/`.
The rule failed one bar of the acceptance after the second measured build, and the brief says:
reverse. The patch is `phases/p43/not_landed/p43_fresh_wall_pass.patch` (the whole `crates`
diff of build 2 against HEAD, with its two lib tests and the `adaptive3d/CLAUDE.md` text).

What failed (build 2, setup "1 Back"):

- Id 1 replay: 2 pieces deeper than 4.966 mm (11 nodes, the deepest 8.62 mm). The bar is 0.
  HEAD has 36 pieces (6 953 nodes, 14.50 mm). Both pieces are the wall line of level 13.002 at
  two corners of the frame: its ring is skewed by 0.047 mm against the ring of the two levels
  above (section 6.2).
- Id 2 replay: 1 piece deeper than 6.3 mm (2 nodes, 8.86 mm). The bar is 0, and HEAD has 0. It
  is a graze at the foot of a wall (section 6.3).
- Cut below the target with one node lowered: 0.1 mm2 (2 nodes, to 0.41 mm). HEAD: 0.0. The
  bar ("not worse than 0.1 mm2") holds with no margin. A climb of the level plan cuts them,
  not the wall line; HEAD retracts at that place (section 6.4).

What holds (build 2): the body row of id 1 and its deepest sample are 4.166 mm (9.0 at HEAD);
no depth sample of id 1 is over 4.233 mm; id 2 body 6.0 mm (deepest 6.13), entry 1.89 mm;
0 collisions, 0 rapid collisions, 0 failed checks; setup "3 Front" keeps each row.

What is weaker than asked, or was wrong:

- The lib test does not show the 9.0 mm cut. It holds the wall line (red without it). Where
  the last wrap of a 2D plan stands depends on the region shape, and no small fixture gave a
  level that runs on a limit that the level above did not reach.
- Predictions that were false: 2 (0 replay pieces), 3 in part (10 wall runs, not 20 to 60;
  4 282 more moves, not 800 to 3 000), 6 in part (the id 2 replay piece; the wood moves by 9
  to 25 %, in the good direction), 8 in part (id 20 has no more time), 10 (no golden cell
  moves). And one before the design: "the P42 wall pass ships nothing on the 1.5 mm cell".
- The premise of the brief on rivmap100 is not so: no stored rivmap100 file has a fresh
  Geodesic rough (each rough is Contour Parallel on remaining stock). The stored file and two
  scratch copies with a fresh Geodesic rough keep their G-code, byte for byte.
- The wall line stands nearer to the raw boundary than the one planner cell of D19 (1.2 mm on
  a 1.5 mm cell). Decision D-1.
- The cleanup row of id 1 changes its state from within to exceeds (advisory): the wall line
  takes 4.166 mm at a width of the strip.

Not verified:

- The deepest body sample of "Front bulk rough 18 mm" (id 20) is 5.73 mm at HEAD and in
  build 2, over its limit of 4.666 mm. Its row reads 4.16 mm. The place was not traced.
- A wall that is not on a grid axis. The frame of this job is. On a curved or oblique wall the
  rings of two levels can differ by the cut tolerance (section 6.2), and each difference is a
  graze on the replay.
- A region that keeps the perimeter sweep, a tool with a planner cell under ten cut
  tolerances, Contour Parallel: no wall line or a smaller shift (section 3.6).
- The GUI and MCP surfaces. One run of each build; the times are simulated times of one run.


## 1. The state at HEAD (the lead's run of 2026-11-20, CLI md5 `a03f43666a235ee8efb68f618e333515`)

"Back bulk rough 18 mm" (id 1): fresh stock, Geodesic Adaptive, tool radius 9.0 mm, Depth/Pass
4.166 mm, stay-down clearance 0.5 mm, tolerance 0.1 mm. The one-pass limit is
4.166 + 0.5 = 4.666 mm. The replay bar is 4.666 + 0.3 = 4.966 mm. Levels: Z 21.334, 17.168,
13.002, 11.0. The planner cell is 9.0 / 6 = 1.5 mm; the simulation cell is 0.2 mm.

- Depth rows of id 1: body 9.0 mm (deepest sample 9.0, sample 268 462 in the sample file of
  ids 0 and 1), entry 1.97, cleanup 2.59, link 4.166. 31 797 moves.
- Replay (`p41/deep.py out_head.nc "Back bulk" .. 4.966`): 36 pieces deeper than 4.966 mm,
  606 mm of feed, 6 953 nodes, the deepest 14.50 mm. (At the bar 4.5 of the lead: 37 pieces,
  610 mm.)
- Two classes (`classes.py`: the clearance of the worst node of each piece from the fed path
  of each level above):
  - Strips: 30 pieces lower more than 5 nodes (to 2 865 nodes). The level above passed 0.02 to
    2.6 mm short of the node. All are at Z 16.5, on the shelf, at the four walls of the frame
    (model walls: the target is 25.5 beside 16.5).
  - Grazes: 7 pieces lower 1 to 5 nodes. The three deepest (14.50, 13.94, 13.94 mm; level
    11.0) lower one or two nodes that stand 5 to 12 micrometres inside the edge of the disc;
    the three levels above passed 12 to 17 micrometres short of the same nodes.

## 2. The cause (measured)

### 2.1 Which level left the strip, and which producer

Probe 1 and probe 2 (`probe2.patch`; the G-code of the probe build is the HEAD file, `cmp`).
The window is the west wall of the frame, setup frame Y 205 to 240.

- The level grid is the same at the wall at levels 21.334, 17.168 and 13.002: the first
  material cell is at X 34.0 (setup frame), the raw region boundary at X 33.25. The opening
  record is the same too ("grown, no sweep", measured margin 1.510 mm), so the 2D tool-centre
  region of each level ends at X 34.76.
- The planner stock row shows the strip on the planner cell: after level 21.334 the cell at
  X 28.0 is at 21.334 and the cell at X 26.5 is at 25.5. After level 17.168: 17.168 and 25.5.
  Level 13.002 lowers the cell at X 26.5 from 25.5 to 16.5: 9.0 mm.
- The stage log names the producer. At levels 21.334 and 17.168 the 2D plan of the region
  (`geodesic::slice_segments`, the field and its wraps) has its least X at 36.282 in the
  window, and the 2D cleanup adds no point nearer to the wall (36.282 after it). At level
  13.002 the same two stages read 34.760: the plan runs on the limit of its region.
- So the cause is not the boundary clip, not the drape and not the region grid. The 2D plan
  does not cut to the limit of its own tool-centre region: its last wrap can stand one 2D cell
  (1.52 mm here) inside the limit, and the 2D cleanup does not see a residue of one cell. Where
  the wraps end depends on the shape of the region. Levels 21.334 and 17.168 have the same
  region (86 851 mm2) and the same plan. Level 13.002 has another region (84 243 mm2), its
  plan runs on the limit, and the edge of its disc takes the strip that two levels left.
- The grazes are the same defect at the size of the emission tolerance: two levels with the
  same plan line ship lines that differ by micrometres.

A fix at the source (choice (a) of the brief) is a change of the 2D plan in
`adaptive/geodesic` or of the shared 2D cleanup. It moves the 2D pins and the bytes of each
rest rough. It is not open in this phase.

### 2.2 Why the deepest replay piece is 14.50 mm and the row says 9.0 mm

HEAD again with `--sim-artifact`, ids 0 and 1 (`samples.py`). The samples of the three pieces
at Z 11.0 (moves 29 454 to 29 457) read 3.18, 3.37 and 3.88 mm (`axial_engagement_mm`), not
14.5. The deepest sample of id 1 is 9.0 mm, at (54.39, 240.14, 16.50), on the shelf at the
west wall: a strip piece.

The instruments differ, as P42 5.1 found: the replay lowers a node only when its centre is
inside the disc, and then by the whole height; the simulation lowers a cell at the disc edge by
its covered part at each stamp. The node of piece 29 409 is 5 micrometres inside the disc, and
12 micrometres outside the discs of the three levels above. The simulation had lowered that
cell in parts at each level. The truth is between: the tool meets a sliver of micrometres at
the full height. The row is right for the force. The replay bar counts the sliver, so the
design must remove the grazes too.

### 2.3 Why the P41 rule made a fresh rough worse (reproduced at Depth/Pass 4.166)

The probe build with the environment switch `P43MODE` (not in the tree), one run each:

| | HEAD | `guard`: the P41 guard on, no wall pass | `p42`: the guard and the P42 wall pass as built |
|---|---|---|---|
| id 1 body row; deepest sample | 9.0; 9.0 mm | 10.33; 10.33 mm | 10.57; 10.94 mm |
| id 1 cleanup row | 2.59 mm | 6.17 mm | 6.17 mm |
| id 1 link row; deepest | 4.166; 4.166 mm | 4.166; 4.166 mm | 7.15; 7.36 mm |
| id 1 moves | 31 797 | 34 514 | 94 680 |
| Replay pieces deeper than 4.966 mm; feed; deepest | 36; 606 mm; 14.50 | 39; 304 mm; 10.33 | 80; 581 mm; 10.61 |
| Centres held off, to the end of each level | 0 | 0, 442, 1 748, 2 419 | 0, 157, 1 012, 1 489 |
| Wall pass runs per level | 0 | 0 | 11, 17, 14, 7 |
| Setup `total_runtime_s` | 9 588 | 9 747 | 10 318 |

- The guard reads the planner stock cell by cell. On a 1.5 mm cell the read is not the cut:
  a cell at the disc edge is half covered, and the read is the covered part times the height.
  A strip narrower than a cell is a part of one cell.
- The guard holds 442 centres at level 17.168. The region of that level is then not the
  region of level 21.334, the 2D plan is another plan, and it meets the strips of level 21.334
  at other places (section 2.1: where the wraps end depends on the region).
- The P42 wall pass does not repair it on this cell. Its line is one planner cell plus the cut
  tolerance inside the raw boundary; the plan limit is one planner cell (1.510) inside. So the
  line is not nearer to the wall than the plan (P42 section 6 said this for a 0.5 mm cell). Its
  standing test needs a cell that the disc covers in full, and it uses the cell as a height
  margin (1.5 mm is 36 % of this pass). The runs it ships leave stock in parts of a cell
  (node tops 22.28 and 23.58 before a later cut), and the moves go from 31 797 to 94 680.
- A prediction of mine that was false: "the P42 wall pass ships nothing on the 1.5 mm cell".
  It ships 49 runs.

## 3. The design

### 3.1 Choice

- (a) the source: not open (2.1).
- (b) the P41 guard and the P42 wall pass on fresh stock: measured, worse (2.3). The guard and
  the standing test of the wall pass both read parts of a coarse cell.
- (c) a finer planner cell for the read: the read is not what fails first. The strip exists
  because no level cuts to the limit of its region; a read can only find it afterwards, and a
  second stock on the simulation cell costs a simulation inside the planner. Not built.

What is built is the wall pass of (b) with no stock read in its ship rule, on a line that
the geometry of the plan gives:

R-43. A level of a fresh Geodesic rough ends with one contour along the limit of the 2D
tool-centre region of each region that it planned, at each wall deeper than one pass. So each
level cuts to the limit that any later level can reach, and a later level meets at most what
one level leaves: one Depth/Pass.

### 3.2 The line

- The 2D tool-centre region of a region is its grown outline inset by the tool radius
  (`grown_outlines`, C31; the 2D planner insets by R). The wall line is the grown outline
  inset by `R - d`: the limit of the tool-centre region moved `d` toward the wall.
- Why `d` is not 0: a later level with the same plan line ships a line that differs by the
  emission tolerance (the grazes of section 1). The stack is the chord tolerance of the plan
  cut (`t`, `emission_chord_tolerance`), the simplification of the wall line (`t`) and the
  chord tolerance of the wall cut (`t`): `d = 3 t`. Id 1: 0.3 mm.
- The bound of `d`: the least margin of the plan from the raw boundary is one planner cell
  (`grown_outlines` refuses a region under it). The research note that gave that rule
  (`P3_C22_3d_turns.md` 11.8) derives the geometry: the raw boundary is within half a cell of
  the true tool-centre limit, and the nearest-cell read of the lift is inside the raw polygon
  by at most cell / (2 sqrt 2). So a centre half a cell or more inside the raw boundary is a
  legal centre and reads a region cell. The wall line keeps that half cell after its own two
  tolerances: `d = min(3 t, cell / 2 - 2 t)`, never under 0. Id 1: min(0.3, 0.55) = 0.3 mm;
  the line is 1.2 mm (nominal) and 1.0 mm (least) inside the raw boundary.
- This is nearer than the one cell that the lead ruled for the PLAN margin (D19). It is a
  decision for the lead (D-1). The path that ships passes the drape and the chord lift of
  every cut, so a point over a wall is lifted, not cut into the wall.
- No line for a region that keeps the perimeter sweep (the grow refused): its plan stands a
  tool radius inside the region, and a contour at the limit there is a full slot.
- The ring is simplified at the cut tolerance and sampled at one planner cell, as the C4 and
  P42 lines (`simplified_ring`, `resample_ring`).

### 3.3 Where a point ships (no read of a strip on the coarse cell)

The ONE ship rule is `wall_point_ships` (P42): (1) the strip top is more than the one-pass
limit above the floor of the next level at the point, or (2) the wall top is more than the
limit above the floor of this level. The fresh variant gives it reads that are correct on a
coarse cell:

- The wall top: the highest tool-centre surface plus the stock to leave over the cells within
  two planner cells of the point (the point is at most one cell inside the raw boundary, and
  the centres of the cells outside it are at most one cell beyond).
- The strip top: the highest stock top over the cells that the disc touches (any covered
  part) and that are not material cells of the level. A strip narrower than a cell does not
  show as a cell; the wall cells behind it carry the height that the levels above left.
- Rule (1) is read only where the next level floor at the point is under this level floor
  (by `MATERIAL_MARGIN_MM`). On a tread of a draped slope the two floors are the same and the
  cells uphill under the disc are the part, not a strip.
- So the pass ships at a wall with no test that stock stands in front of it. Where the plan
  of the level ran on the limit, the pass cuts a sliver `d` wide. That is the cost of a read
  that cannot see under a cell.

### 3.4 The Z of a point, the rounds, the guard

- Z: the level floor. It is raised to one Depth/Pass under the highest stock over the cells
  whose centre is inside the disc only when that stock is more than the one-pass limit above
  the floor (the promise itself; no cell margin).
- One round: a point is cut once per level. A fresh rough cuts each level from the top, so a
  strip is one pass tall.
- `WallRead::beside_held_stock` stays: a point within one cell of material taller than the
  limit (a region that no plan cut) is not cut.
- The P41 guard (`hold_off`, `blocks`) is OFF in the fresh variant: its read is wrong on the
  coarse cell (2.3).
- Runs, the gap merge, the load cap, the entries, the stamp and the marker are those of P42
  (`merge_short_gaps`, `run_feed_cap`, `emit_runs`, `WaterlineCleanup`).

### 3.5 The switch

`linking.one_pass_hold` gets a third variant: `OnePassHold::FreshStock { link_kinematics }`.
The session sets it on a 3D Rough with no seed stock (`RemainingStock` with one, as today).
`Off`: no rule. `RemainingStock`: the guard reads the planner stock, which is the simulated
stock; the wall pass reads the strip. `FreshStock`: no guard; the wall pass reads the wall.

### 3.6 What it does not do

- Contour Parallel gets no wall pass (as P42).
- A region that no level plans (under the small-region bound, an F-038 drop, a left piece)
  keeps its stock; a later level beside it can cut it deeper than one pass. On remaining stock
  the guard holds that case; on fresh stock nothing does.
- A tool whose planner cell is under 10 t gets a smaller `d` than the tolerance stack
  (0.05 mm for a 6 mm tool at t 0.1). A graze is then possible. Not measured.

## 4. Predictions (written before the code)

1. Id 1 body row and its deepest sample: 4.17 mm (at most 4.666).
2. Id 1 replay: 0 pieces deeper than 4.966 mm (36 at HEAD); 0 grazes.
3. Id 1: 20 to 60 wall runs in all; 6 to 14 m more feed; 60 to 180 s more; 800 to 3 000 more
   moves.
4. Id 1 cleanup row (the wall pass is in the cleanup group): depth 4.17 mm; its ratio to its
   own cap is under the ratio of the body row.
5. Id 1 link row: 4.166 mm (no change).
6. Id 2: body 6.0, entry at most 2.5, 0 replay pieces deeper than 6.3 mm; its time moves by
   less than 30 s; the wood in the mech and slit pockets, the step shelf, the slot and the
   cavity interior moves by less than 5 %.
7. Cut below the target, one node lowered: no change above 0.1 mm2. 0 collisions.
8. Setup "3 Front": id 20 gets wall runs; its body row stays 4.16 mm; 30 to 150 s more. Id 21
   reads the stock of id 20 and moves by less than 30 s.
9. rivmap100: the stored file moves only if its rough is a fresh Geodesic rough; then under
   5 % more time and no deeper body sample.
10. Tests: the op golden cells and pins of a fresh Geodesic rough that goes through the
    session move where the model has a wall deeper than one pass. A direct caller of
    `adaptive_3d_*` keeps `Off` and its bytes.

## 5. The rule as built (one change to section 3, after a measurement)

Build 1 (`b1`, CLI md5 `6d38747100cbd757d2cc3c3b8ae5c968`) is section 3 as written. Build 2
(`b2`, md5 `c452b317eebd57e3a5ffe1302220f97a`) changes the line.

### 5.1 What build 1 measured

Id 1: body row 4.166 mm, cleanup row 4.0 mm (deepest sample 4.23), 36 087 moves. Replay: 4
pieces deeper than 4.966 mm, 1 606 nodes, the deepest 9.00 mm. All four are wall lines:

- The ring of levels 21.334 and 17.168 runs on X 53.002 and Y 415.998; the ring of level
  13.002 runs on X 52.986 and Y 416.014: 0.016 mm nearer to the wall.
- The cause: `grown_outlines` insets the smoothed outline of each region by its own shortfall
  (0.982 mm at the upper levels, 0.960 mm at level 13.002; the measured margin is 1.510 mm at
  both, at the nearest point of each region). On the straight wall the limit of the plan is
  then 1.532 and 1.510 mm from the raw boundary. A line that follows that limit moves with it,
  and the lower wall line cuts a sliver 0.016 to 0.022 mm wide, 9.0 mm tall, along the wall.

### 5.2 The change: the line reads the raw outline

The line is one planner cell less `d` inside the RAW outline of each region that the plan
opened on its grown outline (`fresh_line_shift`: `d = min(3 t, cell / 2 - 2 t)`, as 3.2). The
raw outline at a wall is the same cells at each level. The plan stands one cell or more inside
it, so the line is nearer to the wall than each plan line by `d` or more. Id 1: 1.2 mm inside
the raw boundary.

What the change gives up: `grown_outlines` closes each gap of the region boundary narrower
than about two tool radii, and at such a place the plan can stand nearer to the raw boundary
than one cell (its guard admits one cell times the boundary length; measured here: 1.96 and
6.22 mm2 of 1 750 and 501 mm2 allowed). The line of 3.2 followed that; the raw line does not.

### 5.3 The code (in the patch)

- `adaptive3d/mod.rs`: `OnePassHold::FreshStock { link_kinematics }`, and the text of what
  each variant reads.
- `adaptive3d/level_contour.rs`: `WallRule` (`RemainingStock`, `FreshStock`) on `WallPass`;
  `WallRegions` (the planned regions, and of them the regions opened on the grown outline);
  `fresh_line_shift`; `wall_read` takes the rule (the Z, the strip top and the wall top of
  3.3 and 3.4); `region_wall_pass` chooses the line and the standing test by the rule.
- `adaptive3d/clearing.rs`: `LevelEmission::opened_on_grown`; `clear_z_level_slice` collects
  the opened regions. `adaptive3d/path.rs` builds the context. `compute/execute/rough_3d.rs`
  sets `FreshStock` with no seed stock.
- `adaptive3d/tests.rs`: `--lib a_fresh_level_cuts_to_the_wall_line_that_no_plan_passes_p43`
  (a wall four passes deep, a fresh block, an 18 mm tool; simulated on a 0.25 mm cell, a sixth
  of the planner cell: the wood at the foot of the wall is 1.75 mm wide with the rule and
  2.25 mm with the dial off; red without the wall pass) and
  `--lib the_fresh_wall_line_keeps_half_a_cell_from_the_raw_boundary_p43`.
- `RemainingStock` keeps its code path: `wall_read` and the standing test read the same
  values as before for that rule. Ids 2 and 21 cannot show it, because the stock that they
  read moves; section 6.7 holds the check that exists.
- `not_landed/b1_line_on_the_grown_outline.patch` is build 1 (it holds a temporary exploration
  test, `p43_explore`). `not_landed/p43_fresh_wall_pass.patch` is build 2 plus comment lines
  and the `adaptive3d/CLAUDE.md` text (checked with `diff` against the build 2 diff).

## 6. Measured (the proposal file; HEAD against build 2)

One run of each. Files: `phases/p43/meas_all_head.txt`, `meas_all_b2.txt`, `deep1_*.txt`,
`deep2_b2.txt`, `left_b2.txt`.

### 6.1 Setup "1 Back"

| | HEAD | Build 2 | Bar |
|---|---|---|---|
| id 1 body row (judged over its cap); deepest sample | 9.000 / 2.397 = 3.76 (exceeds, advisory); 9.000 mm | 4.166 / 2.345 = 1.78 (exceeds, advisory); 4.166 mm | at most 4.666: holds |
| id 1 cleanup row; deepest sample | 2.592 / 3.533 = 0.73 (within); 2.592 mm | 4.166 / 2.523 = 1.65 (exceeds, advisory); 4.233 mm | at most 4.666: holds |
| id 1 entry row; link row | 1.969; 4.166 mm | 1.969; 4.166 mm | |
| id 1 replay pieces deeper than 4.966 mm; nodes; feed; deepest | 36; 6 953; 606 mm; 14.50 mm | 2; 11; 576 mm (two whole wall sides); 8.62 mm | 0: FAILS |
| of them strips (more than 5 nodes); grazes | 30; 6 | 1 (7 nodes); 1 (4 nodes) | |
| id 1 time; fed; rapids; moves; entries | 689 s; 78.69 m; 2 581 mm; 31 797; 107 | 801 s; 89.16 m; 2 594 mm; 36 079; 104 | report |
| id 1 wall runs per level | 0 | 2, 3, 3, 2 | |
| id 2 body row; deepest sample | 6.000 / 3.197 (exceeds, advisory); 6.000 mm | the same row; 6.130 mm | at most 6.5: holds |
| id 2 entry row; deepest | 1.908; 1.915 mm | 1.885; 1.894 mm | at most 2.5: holds |
| id 2 cleanup row; link row | 6.000 / 4.540; 6.000 / 3.423 | 6.000 / 4.620; 6.000 / 3.412 | |
| id 2 replay pieces deeper than 6.3 mm; deepest | 0; 6.20 mm (3 between 6.0 and 6.3) | 1 (2 nodes, 1 mm of feed); 8.86 mm (1 between 6.0 and 6.3) | 0: FAILS |
| id 2 time; fed; rapids; moves; ramp entries | 579 s; 38.89 m; 4 157 mm; 14 982; 131 | 580 s; 38.80 m; 4 326 mm; 15 239; 124 | report |
| Below the target after id 1: raw; one node lowered | 5.0 mm2; 0.0 mm2 | 5.4 mm2; 0.1 mm2 (2 nodes, to 0.41 mm) | not worse than 0.1: holds, no margin |
| Below the target at the end of the measure (4 ops): raw; one node lowered | 20.8 mm2; 0.0 mm2 | 21.6 mm2; 0.1 mm2 (the same 2 nodes) | |
| Wood left at the end, wall band | 63.7 mm2 | 62.0 mm2 | report |
| Wood left, cavity interior; slot | 570.4; 412.6 mm2 | 570.4; 412.6 mm2 | |
| Wood left, step shelf | 829.8 mm2, 3 687 mm3 | 670.6 mm2, 2 480 mm3 | |
| Wood left, mech and slit pockets | 1 003.9 mm2, 12 443 mm3 | 912.2 mm2, 9 944 mm3 | |
| Wall band after id 2 (before the later ops) | 2 772.2 mm2 | 2 070.2 mm2 | |
| Wood that build 2 leaves above HEAD after id 2 | | 106.9 mm2, 368 mm3 (85.6 mm2 still above the target at the end) | |
| "Day strip slit V60": wood over the tip; moves above the flute | 7.44 mm; 8 | 7.44 mm; 8 | |
| Collisions, rapid collisions, failed checks (each op) | 0, 0, 0 | 0, 0, 0 | holds |
| Setup `total_runtime_s` | 9 588.1 | 9 682.9 | report |
| Row states of the setup (exceeds; within) | 6; 94 | 7; 93 (the cleanup row of id 1) | |
| Export warning lines | 24 | 24 | |

- The stay-down clearance of id 1 is 0.5 mm, so the one-pass limit is 4.666 mm and the replay
  bar 4.966 mm.
- The plan of each level is shorter with the rule (level 17.168: 11 349 to 7 536 moves; level
  13.002: 8 868 to 7 176). The wall line of a level has many moves: its entries are ramps of a
  tool that is not centre-cutting, and a step down of the line is spread along the line by the
  descent clamp (6.5).
- Id 2 keeps its time. It leaves less wood, because id 1 leaves less at each wall.

### 6.2 The two replay pieces of id 1

Pieces 30 828 and 30 829: the wall line of level 13.002 on the shelf (Z 16.5), the east and
the north side of the frame. They lower 4 and 7 nodes at the two north corners (the nodes
stand 0.007 and 0.010 mm inside the disc; the levels above passed 0.010 and 0.031 mm short).

- The rings: levels 21.334 and 17.168 run (341.503, 416.050) to (52.950, 416.003); level
  13.002 runs (341.550, 416.003) to (52.997, 416.050). Each side is skewed by 0.047 mm over
  288 mm, and the two rings are skewed the other way.
- The corner points of the two rings differ by (0.047, -0.047) mm: the two ends of a chamfer
  at 45 degrees.
- The cause, by reading: marching squares cuts each corner of the raw outline with a diagonal
  of half a cell. After the inset the corner has two vertices 0.047 mm apart. The
  simplification of the ring at the cut tolerance (0.1 mm, `simplified_ring`) keeps one of
  them; which one depends on where the ring starts, and the ring of level 13.002 starts at
  another vertex because its region is another polygon.
- The simulation reads no deep sample there: the deepest cleanup sample of id 1 is 4.233 mm.
- Not built (the two builds are used): a ring that does not depend on its start (no
  simplification of the fresh line before the emitter, or a corner made of one vertex), or a
  line of each level that stands inside the line of the level above by more than the
  tolerance. Decision D-2.

### 6.3 The replay piece of id 2

Piece 2 569 of "Back rest 6 mm": (98.476, 100.400, 16.55) to (97.000, 100.400, 16.55). It
lowers 2 nodes at (98.2, 103.4) from 25.41 by 8.86 mm. The nodes are 3.0 mm from the piece, on
the edge of the 3.0 mm disc, at the foot of the wall of the step shelf. The deepest body sample
of id 2 is 6.13 mm. This is the graze class of P42 5.1 (the simulation lowers a cell at the
disc edge by its covered part, the replay by the whole height). Id 2 moved because the stock
that id 1 leaves moved. HEAD has no such piece; it has 3 pieces between 6.0 and 6.3 mm.

### 6.4 The two nodes under the target

G-code (54.6, 118.0) and (54.8, 118.0): the stock is 20.10 and 20.09 mm, the target 20.5 mm
(the edge of the step shelf; the next node of the column has no target). The piece that cuts
them is a climb of the plan of level 17.168: (54.782, 127.782, 17.168) to
(54.782, 126.891, 20.5), piece 12 315, 3.3 mm up in 0.89 mm. It is not a wall line (the wall
line of that level climbs the same step at X 52.95 and is 9.49 mm or more from the nodes). At
HEAD the plan leaves this place with a retract straight up, (54.782, 127.782, 17.168) to
Z 25.782, and the two nodes stay at 20.5. Why build 2 feeds the climb was not traced; the
stock of the level differs after the wall line of the level above. The chord of the climb
passes 0.4 mm under the edge of the step: the wall chord gap of the cut pipeline that P40
left open.

### 6.5 The wall line and the descent clamp (seen, not measured as a cost)

Two wall pieces of build 2 descend along a whole side: (341.549, 128.978, 20.313) to
(341.503, 416.050, 17.168) at level 17.168, and (105.278, 128.989, 19.5) to
(341.503, 128.950, 16.5) at level 13.002. The ring crosses the step shelf (floor 20.5) and
then drops to the level floor; the tool is not centre-cutting, so `clamp_descents_to_limit`
spreads the drop along the line. The side is then cut on a slope, up to 3.1 mm above the
level at its start. The level plan splits such a step with an entry; `emit_runs` (the C4 and
P42 emit loop) does not. No sample is over the limit here. Decision D-3.

### 6.6 Setup "3 Front"

| | HEAD | Build 2 |
|---|---|---|
| id 20 body row; deepest sample | 4.164 / 2.345 (exceeds, advisory); 5.732 mm | the same row; 5.732 mm |
| id 20 entry; cleanup; link rows | 1.941; 4.166; 3.666 mm | the same |
| id 20 time; fed; rapids; moves | 480 s; 54.75 m; 3 635 mm; 22 670 | 480 s; 54.81 m; 3 882 mm; 22 798 |
| id 21 body row; deepest | 4.308 / 3.048 (exceeds, advisory); 4.308 mm | the same |
| id 21 entry row; cleanup row (deepest); link (deepest) | 1.590; 4.210 (5.176); 0.480 (0.497) | 1.560; 4.210 (5.176); 0.480 (0.480) |
| id 21 time; fed; rapids; moves | 914 s; 58.69 m; 15 512 mm; 39 788 | 910 s; 58.60 m; 15 726 mm; 39 588 |
| Row states (exceeds; within) | 5; 58 | 5; 58 |
| Collisions, rapid collisions, failed checks | 0, 0, 0 | 0, 0, 0 |
| Setup `total_runtime_s` | 8 833.9 | 8 831.6 |

Id 20 gets 128 moves of wall line and no more time. No row is worse.

### 6.7 rivmap100

- No stored file has a fresh Geodesic rough: the rough of `rivmap100.toml`,
  `rivmap100_live_0925.toml` and the other copies is Contour Parallel on remaining stock.
- `planning/fixtures/rivmap100/rivmap100_live_0925.toml`, read in place: the same G-code as
  HEAD, byte for byte (`cmp`). 9 108 moves, the rough 546 s, body 8.0 mm (deepest 8.0),
  `total_runtime_s` 1 663.2, 0 collisions. Cut below the target: not changed (the G-code is
  the same bytes).
- A scratch copy with the rough on fresh stock and Geodesic Adaptive (no such file is in
  `planning/fixtures/`), Depth/Pass 8.0: the same G-code, byte for byte. 7 530 moves, 406 s,
  body 6.0 mm, `total_runtime_s` 1 329.9.
- The same copy at Depth/Pass 2.0: the same G-code, byte for byte. 15 766 moves, 1 068 s, body
  2.0 mm (deepest sample 2.838), `total_runtime_s` 2 088.0.
- So the benchmark does not move: the terrain has no wall that the ship rule reads as deeper
  than one pass.
- The `RemainingStock` path, which the patch also edits: a scratch copy of the stored file
  with only `clearing_strategy = "geodesic_adaptive"` (the rough reads remaining stock) keeps
  its G-code, byte for byte (6 780 moves, body 8.0 mm, `total_runtime_s` 1 229.3). The P42 note
  says that this copy ships no wall pass, so it proves the guard path and not the wall read.
  The lib tests `_p41` and `_p42` pass on the patch (7.1); they pin the wall read of P42.

### 6.8 Predicted against measured

| # | Prediction | Result |
|---|---|---|
| 1 | Id 1 body row and deepest sample 4.17 mm | TRUE (4.166) |
| 2 | 0 replay pieces deeper than 4.966 mm, 0 grazes | FALSE (2 pieces, 11 nodes) |
| 3 | 20 to 60 wall runs; 6 to 14 m more feed; 60 to 180 s more; 800 to 3 000 more moves | runs FALSE (10); feed TRUE (10.47 m); time TRUE (112 s); moves FALSE (4 282) |
| 4 | Cleanup row 4.17 mm, its ratio under the body ratio | TRUE (1.65 against 1.78) |
| 5 | Link row 4.166 mm | TRUE |
| 6 | Id 2 rows; 0 replay pieces; under 30 s; wood within 5 % | rows TRUE; replay FALSE (1 piece); time TRUE (1 s); wood FALSE (less by 9 to 25 %) |
| 7 | One node under the target: no change above 0.1 mm2; 0 collisions | TRUE at the bar (0.0 to 0.1 mm2) |
| 8 | Id 20 gets wall runs, body row the same, 30 to 150 s more; id 21 under 30 s | runs and rows TRUE; time of id 20 FALSE (0 s); id 21 TRUE (4 s less) |
| 9 | rivmap100 moves only with a fresh Geodesic rough; then under 5 % | no file has one; the scratch copies do not move |
| 10 | Golden cells of a fresh Geodesic rough move | FALSE (the 52 red cells of HEAD, line for line) |

## 7. Gates

### 7.1 Gates of the patch (HEAD plus `not_landed/p43_fresh_wall_pass.patch`)

Logs: the scratch dir, `gate_b2_*.log`.

The baseline of each gate is the gate log of P42 round 3 (`../p42/gate_r3_*.log`, the build of
`e2e10271f`). `git diff --stat e2e10271f HEAD -- crates` is empty, so that build is HEAD code.
No HEAD gate ran in this session. No red is new against that baseline.

| Gate | Result with the patch |
|---|---|
| core `--lib` | 2 856 passed, 1 failed, 13 ignored: the known red `feeds::envelope::tests::the_envelope_meets_its_time_budget`. The two `_p43` tests are the 2 more passes (HEAD: 2 854). |
| adaptive3d sentries (37 binaries) | 75 passed, 0 failed, 7 ignored (the P42 counts) |
| dressup sentries (19 binaries) | 119 passed, 5 ignored |
| other core sentries (20 binaries, with `folder_maps_name_real_files_f140`) | 141 passed |
| op golden | red with the 52 cells of HEAD, cell for cell and value for value (`diff` of the cell lines against the P42 log: empty). No cell moves. Not blessed. |
| FM1 | 2 passed |
| `rs_cam_cli` (17 binaries); `rs_cam_mcp` | 113 passed; 39 passed |
| viz `--lib` (`--test-threads 2`, no `rs_cam_gui` process) | 705 passed, 2 ignored |
| full clippy | clean |
| `rustfmt --check` on the 7 changed `.rs` files | clean |

So no test or golden cell pins a fresh Geodesic rough with a wall deeper than one pass through
the session: the exception class of the brief is empty.

### 7.2 Gates of the tree as left

The tree is HEAD in `crates/`. `git status --short -- crates` is empty after the reverse
(`patch -R`), and the patch applies again clean (`patch --dry-run -p1`). No gate ran on it: no
line of code differs from HEAD. The release CLI was built again after the reverse:
`target/release/rs_cam_cli` is md5 `9ee6810046cb5c42e3391916c6f1e0bd`. It is not the md5 of the
lead's binary (`a03f4366...`), because the binary holds its build identity
(`463be53ee-dirty`; the lead's is a build of `e2e10271f`). Its G-code of setup "1 Back" is the
HEAD file, byte for byte (`cmp`, one run).

## 8. Decisions for the lead

- D-0 (land or not). The rule removes the defect of the brief by the simulation (body 9.0 to
  4.166 mm; 6 953 replay nodes to 11) and fails the replay bar by 11 nodes at two corners
  (id 1) and 2 nodes (id 2). The choices: land the patch as it is with the replay bar read by
  class; a third build for the corner (D-2); or leave it.
- D-1 (the line and D19). The wall line of a fresh rough is 1.2 mm inside the raw boundary on
  a 1.5 mm cell; the lead ruled one cell for the plan margin. The derivation is 3.2 (the half
  cell of `P3_C22_3d_turns.md` 11.8 after the two tolerances of the line). Measured: 2 nodes
  under the target, cut by a climb of the plan and not by the line (6.4). A wall line point
  that the drape lifts onto a wall top was not searched for. If one cell must hold for the
  line too, the plan of a fresh rough must stand inside it (every fresh Geodesic rough then
  moves).
- D-2 (the corner). Section 6.2 names two forms. Neither is built or measured.
- D-3 (the descent clamp on a contour run, 6.5). `emit_runs` does not split a step down of a
  ring. It applies to the C4 contour and the P42 wall pass too, for a tool that is not
  centre-cutting.
- D-4 (the cleanup row). The wall line makes the cleanup row of id 1 exceed, advisory (4.166
  against 2.523 mm, ratio 1.65; the body row is 1.78). It is inside the D-10 bar of P42.
- D-5 (what the rule does not hold, 3.6 and 5.2): a region that no level plans, a region on
  the perimeter sweep, a closed gap of `grown_outlines`, Contour Parallel, a small tool.
- D-6 (id 20). Its deepest body sample is 5.73 mm at HEAD, over 4.666 mm, and the rule does
  not change it. The cause is not known.
- D-7 (the brief). rivmap100 has no fresh Geodesic rough. If the benchmark must hold this
  rule, it needs such a file in `planning/fixtures/`.

## 9. Round 2: four traces and a proposal for build 3 (2026-11-20, after `9a513376f`)

The lead ruled: build 2 does not land as it is; the replay bar of the first brief was wrong;
four traces before a ruling on a third build. No product code was built in this round. The
tree is HEAD in `crates/` (`git status --short -- crates` is empty). The release CLI was
built again from it at the end (md5 `16c3ac2cb14cd4cae1a06285b9ff7e6a`); the binary holds no
probe text (`strings`). Its G-code was not compared with HEAD on the job again.

Three probe builds were made, and this is said first, as the ruling asks. Two questions had no
answer in a file or in a run of an existing binary:
- trace 1, "which read changed": the reads of the step split are not in any output. `cli_p3h`
  is HEAD plus one print; `cli_p3b` is build 2 plus the same print.
- trace 4, "which term of the ship rule refuses": `cli_p4` is build 2 plus the step print and
  one print of the ship terms per level.
The probes print only. The motion of id 1 of each probe run is the motion of its build, line
for line (feeds aside). `phases/p43/probe4_on_build2.patch` is the whole diff of the `cli_p4`
tree against HEAD (the not-landed patch plus the two prints); `probe_round2_output.txt` holds
the lines.

### 9.0 What failed, and what is not verified

- Trace 3 for id 20 is partial. The instruments hold no model in the frame of setup "3 Front",
  so the clearance of its wall line from the model wall is NOT measured. Its one wall move
  was read against the plan points beside it.
- Trace 2 gives the class from the opening record and the reach of each level. It was not
  shown that a wall line on that region would ship at the place of the sample (9.2).
- Trace 1: the value 6.448 mm of the HEAD read is measured; which planner cell holds it was
  not read.
- The proposal for build 3 (9.6) is not built. One of its own predictions is that it breaks
  the D-10 bar of P42 on id 20.
- One run of each probe.

### 9.1 The corrected replay class

`deepc.py` (the replay of `p41/deep.py`): a node counts for a piece when the piece lowers it
by more than the bar AND the node stands the cut tolerance (0.1 mm) or more inside the disc
edge of that piece (tool radius less the XY distance from the node to the piece). The class is
geometry, not a node count. File `deepc_all.txt`.

| Pieces over the bar | HEAD: all | HEAD: corrected class | Build 2: all | Build 2: corrected class |
|---|---|---|---|---|
| id 1 (bar 4.966 mm) | 36 (6 953 nodes) | 31 (6 310 nodes, deepest 9.00 mm) | 2 (11 nodes) | 0 |
| id 2 (bar 6.3 mm) | 0 | 0 | 1 (2 nodes) | 0 |

One piece by path, piece 30 828 of id 1 of build 2, (341.503, 128.950, 16.5) to
(341.550, 416.003, 16.5), tool radius 9.0: it lowers 4 nodes by 8.62 mm, (350.4, 417.6),
(350.2, 418.4), (349.6, 420.0), (349.4, 420.4). They stand 0.0071, 0.0240, 0.0123 and
0.0024 mm inside the disc edge. None counts. Piece 30 829: 7 nodes, 0.0041 to 0.0412 mm
inside. Piece 2 569 of id 2: 2 nodes, 0.0000 mm inside (on the edge).

### 9.2 Trace 1: the two nodes under the target

- What id 1 must leave. The target of the instrument is `back_target_s1.stl`, and that file
  is the model of id 1 (`model_id = 0`). Id 1 has `stock_to_leave_axial = 0.0`. So 20.5 is
  the finished surface and the leave is 0: the cut is 0.41 mm under the model plus the leave.
  While the tool is under the target, the two nodes stand at most 0.093 and 0.095 mm inside
  the disc edge.
- Why build 2 feeds the climb. The function is `clearing.rs::plan_and_emit_region`, the arm
  `StepSplit::StockDepth if shape_split`. The step is (36.282, 119.282, 17.168) to
  (36.282, 117.782, 20.5) in the setup frame, level 17.168. The read that changed is
  `step_axial_depth` (the conservative stock top over the disc less the tool Z):

  | | `step_axial_depth` | limit | load; ceiling | `large_z` |
  |---|---|---|---|---|
  | HEAD (`cli_p3h`) | 6.448 mm | 4.666 mm | 0.000; 0.333 | true: retract and entry |
  | Build 2 (`cli_p3b`) | 3.332 mm | 4.666 mm | 0.134; 0.333 | false: fed |

  At HEAD the disc holds stock that level 21.334 left at the wall, so the step is deep and is
  split. In build 2 the wall line of level 21.334 cut that stock; the read is the step itself
  (20.5 - 17.168), and the step is fed. It is not a D65 capped step (no cap marker).
- The same chord exists at HEAD. `gouge.py` reads each fed piece against the target:

  | Nodes that a fed piece of id 1 passes more than 0.3 mm under the target | HEAD | Build 2 |
  |---|---|---|
  | All | 125 (5.00 mm2), deepest 2.57 mm | 136 (5.44 mm2), deepest 2.57 mm |
  | 0.1 mm or more inside the disc edge | 0 | 0 |
  | 0.05 to 0.10 mm inside | 24 | 31 |
  | Under 0.01 mm inside | 101 | 105 |

  At HEAD, at the same step edge (Y 118.0): node (110.2, 118.0), 0.41 mm under, 0.094 mm
  inside, piece 11 041, (108.883, 127.782, 17.168) to (110.260, 126.891, 20.5); node
  (334.0, 118.0), 0.33 mm under, 0.100 mm inside, piece 15 421. The HEAD replay stock reads
  20.09 and 20.17 mm at these nodes.
- Why the measure of the first round read 0.0 at HEAD. Its rule lowers the target over one
  node. At X 110.2 and 334.0 the next node (Y 118.2) has the target 16.5, so the rule hides
  the node. At X 54.6 and 54.8 the next node has no target (no triangle over it), so the
  rule cannot lower, and the node shows. The two nodes are not a new class.
- The class and its size. The chord lift tests each chord with the reduced tool of the model
  gouge guard (`surface/chord_lift.rs::lift_chords_to_leave`, the probe of
  `compute/model_gouge.rs::allowance_probe_radial_tolerance_mm`: the cutter less the chord
  tolerance, 0.1 mm for id 1, on its radius). So a chord can pass with the full tool up to
  0.1 mm over a vertical edge, at any depth under it. Measured: no node 0.1 mm or more inside
  the edge at HEAD or in build 2. It is the stated tolerance of the guard, not a gap of the
  pipeline beyond it. The "wall chord gap that P40 left open" was not found as a larger cut.
- The smallest change, if the lead wants this class gone: in the same arm of
  `plan_and_emit_region`, a shape-split step that is NOT split and that climbs goes out as two
  moves (up at the first XY, then over at the upper Z). Then the full tool is never under the
  upper floor inside its disc. Not built, not measured, and not in the proposal of 9.6.

### 9.3 Trace 2: the 5.73 mm sample of id 20

`cli_head` with `--sim-artifact`, setup "3 Front", ids 18 to 20. One sample of 233 341 is over
4.666 mm:

- Sample 232 512, move 22 635, (340.461, 44.550, 15.602) in the setup frame, depth 5.732 mm,
  arc 1.211 rad, 3 127 mm/min, a clearing cut, not in a transit span. It is the first cut
  move after a ramp entry of level 13.5, draped at Z 15.602. 5.732 = 21.334 - 15.602: the
  tool meets stock that stands at the floor of level 21.334.
- The reach of each level at this place (`front_trace.py`, fed points with X over 300, Y 20
  to 70): level 21.334 to X 344.15; level 17.168 to X 335.00; level 13.5 to X 341.15.
- The class: the perimeter sweep (D-5). The opening record of level 17.168 is, for both of
  its regions, "the shortfall inset split the outline: shipped sweep" (the levels 21.334 and
  13.5 are "grown, no sweep"). On the sweep path the plan stands a tool radius plus 0.25 mm
  inside the region (`clear_one_region`, the perimeter sweep). So level 17.168 leaves a band
  at the limit of its regions, at 21.334: its fed points end 9.15 mm short of level 21.334
  here. Level 13.5 opens on its grown outline, reaches 6.15 mm farther than level 17.168, and
  takes the band in one cut (width 5.83 mm).
- It is not the strip of 2.1 (one 2D cell), and not a region that no level plans.
- Why R-43 ships no line there: the fresh rule gives a line only to a region that the plan
  opened on its grown outline. The ship-term probe states it: level 17.168, "planned regions
  2, opened 0, rings 0". (Level 21.334: 844 points, each a tread, largest wall top less floor
  2.40 mm. Level 13.5: 483 points, 2 over the limit at 4.71 mm, 1 run.)
- What the promise needs there: the level that keeps the sweep must cut its own band. Two
  ways. (a) The wall line on a sweep region too: one contour 8.05 mm wide for this tool
  (the sweep reaches 0.25 mm inside the raw boundary, the line 7.8 mm beyond it), at one pass,
  with the load cap of `run_feed_cap`. (b) No sweep on a fresh rough: `grown_outlines` plans
  the pieces of a split outline on their grown outlines. (b) is the source and changes the
  plan of each such level; (a) is one line.
- Not shown: that the ship rule is true at this place at level 17.168. By reading it is rule
  1: the next floor (15.602) is under the floor (17.168), and the strip top (21.334) less the
  next floor is 5.73 mm.

### 9.4 Trace 3: D-1, the wall line against the model

`wall_line.py`, build 2. The wall-line moves are the fed moves of the first cleanup item of
each level, less its entries, sampled each 0.5 mm. File `wall_line_b2.txt`.

| id 1 | Cut length | Samples | Lifted | Least clearance |
|---|---|---|---|---|
| Level 21.334 | 1.50 m | 3 050 | 0 | 0.934 mm |
| Level 17.168 | 1.49 m | 3 038 | 2 | 0.934 mm |
| Level 13.002 | 1.47 m | 2 974 | 0 | 0.934 mm |
| Level 11.0 | 0.31 m | 655 | 0 | 0.934 mm |

- Clearance: the least XY distance from a sample to a target node that stands more than
  0.05 mm above the tool tip, less the tool radius. The least is 0.934 mm, at
  (224.550, 88.497, 21.334). So the tool on the line is never nearer than 0.93 mm to the
  model wall at its own height.
- Lifted: a sample that rides the model where a point 0.3 mm farther from every wall would
  not (the highest target within R is over the level and over the highest target within
  R - 0.3 by more than 0.1 mm, and the Z is on it). 2 of 9 717 samples: (341.55, 126.98) and
  (52.95, 126.99), Z 20.5, lift 3.33 mm. Both are the place where the ring crosses the edge
  of the step shelf and climbs onto it. No sample along a wall is lifted.
- Id 20 has one wall move in build 2: (341.539, 138.187, 14.978) to (341.539, 139.682,
  14.944), level 13.5, 1.5 mm long. Its Z is the Z of the plan points beside it
  ((341.15, 138.35, 14.944), 0.39 mm away). Its clearance from the model is not measured
  (9.0).
- So 1.2 mm inside the raw boundary holds on this job with 0.93 mm to spare. D-1 stays as
  written; the measure speaks for 1.2 mm.

### 9.5 Trace 4: rivmap100, the dial and the ship terms

`cli_p4` on the two fresh Geodesic scratch arms; the G-code is the build 2 file, byte for
byte. The probe prints the rule of each level, and each level says `FreshStock`. A second
proof needs no probe: `cli_b2` with `RUST_LOG=rs_cam_core::adaptive3d=debug` prints "P42 wall
pass of the level ... runs=0" at each level (the line is after the test of the dial);
`cli_head` prints none.

| Arm | Level | Opened of planned | Ring points | Rule 2 true (largest wall top less floor) | Rule 1 read | Runs |
|---|---|---|---|---|---|---|
| Depth/Pass 8 (limit 8.5) | 6.0 | 1 of 1 | 814 | 0 (0.00 mm) | 0 | 0 |
| | 0.5 | 2 of 2 | 942 | 0 (1.79 mm) | 0 | 0 |
| Depth/Pass 2 (limit 2.5) | 12.0 | 1 of 1 | 814 | 0 (0.00 mm) | 0 | 0 |
| | 10.0 | 1 of 1 | 806 | 0 (1.87 mm) | 0 | 0 |
| | 8.0 | 1 of 1 | 1 188 | 0 (1.82 mm) | 0 | 0 |
| | 6.0 | 0 of 2 | 0 | | | 0 |
| | 4.0 | 2 of 2 | 1 100 | 0 (1.79 mm) | 0 | 0 |
| | 2.0 | 1 of 1 | 214 | 0 (1.44 mm) | 0 | 0 |
| | 0.5 | 0 of 0 | 0 | | | 0 |

- The term that refuses: no wall deeper than one pass. Rule 2 is false at each point (the
  tool-centre surface within two cells stands at most 1.87 mm above the floor). Rule 1 is
  read at no point: each ring point is a tread (its next floor is its floor).
- Level 6.0 of the Depth/Pass 2 arm has two regions on the perimeter sweep and so no ring:
  the class of trace 2 exists on the benchmark too.
- For comparison, id 1: 1 016, 1 016, 1 003 and 233 points; rule 1 true at 1 016 points of
  level 21.334 (8.33 mm), rule 2 true at each point of level 17.168 (8.33 mm).

### 9.6 Proposal for build 3 (not built)

The change, on top of build 2: ONE line of the rule. The wall line of a fresh rough runs on
each region whose plan cut a segment, not only on a region opened on its grown outline
(`WallRegions::opened` and `LevelEmission::opened_on_grown` go away). The line, the reads,
the ship rule and the single round stay.

- Why a contour at the limit of a sweep region is not the full slot that 3.2 refused: the
  band is the tool radius plus 0.25 mm less the inset of the line (9.25 - 1.2 = 8.05 mm for
  the 18 mm tool), 45 % of the diameter, and the run has the load cap of `run_feed_cap`.
- No change for the corner (6.2): its nodes are 0.002 to 0.041 mm inside the disc edge, the
  graze class of the corrected bar.
- No change for trace 1: the class is at HEAD and inside the stated tolerance of the guard.

Predictions (written before any code of build 3):

1. Id 1: the G-code of build 2, byte for byte (each of its regions opens on the grown
   outline). So each number of 6.1 holds, and the corrected replay class reads 0 pieces for
   id 1 and id 2.
2. Id 20: wall runs at level 17.168 on both regions; its deepest body sample falls from 5.73
   to 4.17 mm (at most 4.666).
3. Id 20 cleanup row: depth 4.17 mm at a width near 8 mm. Its ratio to its own cap is OVER
   the ratio of the body row (1.78). This breaks the D-10 bar of P42 round 3. The lead must
   rule on it before the build: allow it for a sweep region, or choose way (b) of 9.3.
4. Id 20: 60 to 200 s more, 300 to 1 500 more moves. Id 21: under 30 s of change.
5. rivmap100, the fresh Depth/Pass 2 arm: level 6.0 gets rings; rule 2 stays false (the
   terrain rise is 1.9 mm against 2.5), so the same bytes. The stored file: the same bytes.
6. Golden: a cell of a fresh Geodesic rough moves only if it has a sweep region at a wall
   deeper than one pass. I expect none (build 2 moved none), but I did not read the cells.
7. Cut below the target for id 20: not predicted (no instrument in that frame).

What build 3 still does not hold:

- A region that no level plans (the small-region bound, an F-038 drop, a left piece with no
  field): id 20 has 116 such areas at HEAD. A later level beside one can cut it deeper than
  one pass. On this job the tallest stands 2.96 mm above its floor.
- A closed gap of `grown_outlines` (5.2), Contour Parallel, a tool with a planner cell under
  ten cut tolerances.
- A step down of a ring is spread along the line by the descent clamp (6.5, D-3).
- The load of the band of a sweep region is held by the feed cap only, not by a pass width.
- A wall that is not on a grid axis: two rings can differ by the cut tolerance. The corrected
  replay class then reads 0, and the simulation rows are the judge.
