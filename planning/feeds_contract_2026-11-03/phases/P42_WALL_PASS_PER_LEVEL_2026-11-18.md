# P42: each level of a rest rough cuts to the wall that it can reach (2026-11-18)

Job: `planning/rivmap_block_job/nz_wanaka_21_lead_proposal.toml` at `03762ea4e` (HEAD, P41), setup
"1 Back", read in place. Positions are in the G-code frame (setup frame + (18.5, 8.5)) unless the
text says otherwise.

Scratch dir (not in the repo, a restart of the machine deletes it):
`/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/ce846d39-41d4-4e44-be56-d5dd5295a423/scratchpad/p42/`.
`phases/p42/` holds the instruments of this note and the two commit messages. The instruments of
P37 to P41 (`gapmap.py`, `entries.py`, `meas.py`, `left.py`, `deep.py`, `sect.py`, `lim.py`,
`run_meas.sh`) are used as P41 left them; `p42/lim.py` is the P41 file plus the deepest sample.

Sections 1 to 4 were written before a line of the wall pass was written, and they are not
changed. The pass as built is not the pass of section 3 in three points; section 5 says which
and why. Section 0 is the summary and was written last.

State of the tree after round 3 (section 11): the wall pass IS in the tree, in the form of
round 3. Sections 1 to 10 are the record of rounds 1 and 2; where they say "the tree `w6`", "the
final tree" or name a patch `w6_...` or `r2_...`, read: a trial that the code of round 3
replaces (those two patches are deleted; `git show b68262ebc:<path>` gives them).

## 0. Summary: what failed, and what is not verified

State after round 3 (the last, section 11): each bar of the round 3 acceptance holds, and the
wall pass is in the tree. Step 0 is committed (`1ef06d1fe`).

What failed, or is weaker than asked (round 3):

- "Front 3D rough" is NOT the HEAD G-code. Rule (2) of the ship rule ships the pass where the
  terrain has a wall deeper than one pass and the limit: 885 more moves, 868 to 914 s. The bar
  (at most 930 s) holds with 16 s to spare. Prediction 1 said at most 900 s.
- The wall cleanup depth row changes state on both ops, as the round 3 ruling allows: id 2 from
  within to exceeds, advisory (6.0 against 4.54 mm, ratio 1.32; body 1.88); id 21 a new row that
  exceeds (4.21 against 3.72 mm, 1.13; body 1.41). The judged sample of id 2 is 4.27 mm wide:
  the pass is not a thin strip at each place (10.1). No rule of round 3 bounds that width.
- The read of the wall top is the MODEL wall top (the tool-centre surface over the strip
  cells), not the seed stock top that the ruling names first. The planner does not keep the
  seed stock top (11.1).
- The cavity interior keeps its wood (570.4 mm2, as at HEAD; the round 1 trial cut it to
  408.4): its walls are not deeper than one pass and the limit.
- The goal "the guard has little to hold" is met in part: the stock that the guard holds falls
  from 710.8 to 553.3 mm2. The wall line follows the planned region (5.4, D-3 is open).
- Not covered, as before: the 8.6 mm slot (412.6 mm2), the step shelf (773.8 mm2), the regions
  under the small-region bound. The slit V-bit has 8 moves with wood above the flute (8 at HEAD).
- Full clippy was red on the first run of round 3 (`region_wall_pass` had 8 arguments). The Z
  and the next Z are now one value (`WallLevel`); no logic changed. After that change: full clippy clean, `--lib -- _p42 _p41 _c4 _p37` 15 passed, and the
  release CLI built again (md5 `a03f43666a235ee8efb68f618e333515`, the binary in
  `target/release`) gives the `r3` G-code of setup "1 Back", byte for byte. The other gates and
  setup "3 Front" were not run again after it.

Not verified:

- The GUI and MCP surfaces (no GUI was run).
- Setup "3 Front" on the G-code replay (the replay instruments hold the target of "1 Back").
- The ship rule on a real slope in a lib test: the lib test holds the rule as a function, and
  the Front run is the evidence on terrain.
- That the wall pass ran inside the session rest sentries (D44, D48, D50, D55); they are green.
- `beside_held_stock` alone on the final line; a run of one point (`emit_runs` gives it a second
  point one cell along X); a tool that is not flat; Contour Parallel on remaining stock.
- The time of each op is one run, read from the emitted feeds.

What holds (HEAD against the tree; section 11.3 has the table): id 2 body row and deepest sample
6.0 mm, entry 1.91 mm, 0 replay pieces deeper than 6.3 mm; mech and slit pockets 1 631.5 to
1 004.0 mm2; the slit V-bit 10.60 to 7.44 mm; one node below the target 0.2 to 0.1 mm2 (raw 12.1
to 12.2); 0 collisions and 0 rapid collisions on each op; id 1 the same bytes; id 2 522 to
578 s; id 21 868 to 914 s; rivmap100 and its Geodesic copy the same bytes; no new red.

## 1. Step 0: why the depth row read 6.0 mm beside two samples of 8.00 mm

Run `a1` (the first build of P41, `cli_a1`, kept in the scratch dir) again with
`--sim-artifact`, ops 0 to 2. The sample file holds 164 156 samples of id 2.

| Sample | Move | Depth (`axial_engagement_mm`) | Arc | Feed | In a transit span | Intent |
|---|---|---|---|---|---|---|
| 62 484 | 4 645 | 8.00 mm | 2.500 rad | 1 623 mm/min | no | clearing cut |
| 62 542 | 4 654 | 8.00 mm | 1.844 rad | 2 068 mm/min | no | clearing cut |

- No filter drops the two samples. `groups::populations` gives each removing sample to its
  group, and `depth::is_depth_sample` keeps a sample that removes material at a depth above 0.
  Both samples are in the population of the body group. There is no transit filter in the
  depth gate.
- The rule that keeps them out of the row is the judged-sample rule (`depth.rs`, P29-6 6c,
  ruling Q3): each sample is judged against ITS OWN cap, the depth where the peak force at its
  own width and chip meets the force budget. The row prints the sample with the largest depth
  over its own cap.
- The two samples run at 1 623 and 2 068 mm/min (a load cap of the planner slowed them; the
  stated feed is 5 279). Their chip is about a third of the chip of the judged sample
  (0.045 and 0.057 mm against 0.1285 mm), so their own cap is deeper, and 8.00 mm over that cap
  is a smaller ratio than 6.00 mm over 3.197 mm.
- The row text already names them: "deepest engaged depth 8.000 mm". The number is in the
  detail sentence only. `value` on the wire is 6.0, and `lim.py` reads `value`.
- A second rule can keep a deep sample out of the row: a sample with no arc has no width, so it
  has no number (44 607 such samples in this run). It did not apply to these two samples.

Verdict: both rules are correct by design (the row is a force judgement, and a slow deep cut
does have less force). The defect is that no field states the deepest sample. A reader of the
row takes `value` as "the deepest cut".

The fix (commit 0): `tool_load::verdict::DeepestSample { depth_mm, sample_index }`.

- The depth gate puts it on `SampleEvidence::deepest` of every verdict that holds evidence
  (`Within`, `Exceeds`, `Reported`): the largest `axial_engagement_mm` of every sample of the
  group that removes material at a depth, with or without a width.
- `CriterionStatus::deepest` carries it; `limits::CheckRow` has the field `deepest` on the wire
  (`limit_rows[..].deepest.depth_mm`, absent on a row with none). `value`, `bound`, `margin`,
  the state, the face and the detail do not change.
- Tests: `--lib the_verdict_states_the_deepest_sample_beside_the_judged_one_p42` (a deeper
  sample at a tenth of the feed, and a deeper sample with no arc: the row value stays the
  judged sample, the record names the deepest) and
  `--lib a_depth_row_states_the_deepest_sample_p42` (the wire key). Red proof: with the record
  not filled in `depth.rs`, the first test fails at "the verdict states the deepest sample of
  its group".
- On the job (the tree of step 0 alone, release CLI md5 `88310715e4c8e54727e3d46196569fcc`,
  setup "1 Back"): the G-code is the HEAD file, byte for byte (`cmp`); each limit row is the
  HEAD row plus the new key (20 rows hold it). The body row of id 2 reads 6.0 mm and its
  deepest sample 6.2 mm; every other row of ids 1, 2 and 40 has the deepest sample equal to
  its value.

## 2. Step 1: the cause, measured on HEAD and on the tree before P41

`sect.py` on the stored G-code of the tree before P41 (`p41/out_head.nc`, `7b4a65d2f`) and of
HEAD (`p41/out_b1.nc`): the stock at the end of the setup on the line X 300, and the nearest
pass of each level of id 2 to the point (300, 102.0). The wall is at Y 104.0 (target 25.5); the
shelf at its foot is at Z 11.0.

| Level Z | Pass line Y, before P41 | Disc edge | Pass line Y, HEAD | Disc edge |
|---|---|---|---|---|
| 19.5 | 100.35 | 103.35 | 100.35 | 103.35 |
| 16.55 | 100.35 | 103.35 | 99.95 | 102.95 |
| 13.5 | 100.35 | 103.35 | 99.95 | 102.95 |
| 11.0 | 100.35 | 103.35 | 99.64 | 102.64 |

Stock on that line at the end of the setup:

- before P41: 25.5 from Y 103.4 to 103.8 (3 nodes), 11.0 from Y 103.2 down;
- HEAD: 25.5 from Y 103.4 to 103.8, 19.5 at Y 103.0 and 103.2, 13.5 at Y 102.8.

What this says:

1. Before P41 every level runs the SAME line. No margin and no cell rule differs between the
   levels. The plan keeps its centre 0.65 mm from the tool-centre limit of the wall, at each
   level, so a strip 0.65 mm wide stands from the stock top. That strip is the margin of the
   plan (`smoothed_region`: an inset of 0.3 mm and an RDP of 0.3 mm; then
   `PERIMETER_INSET_MARGIN_MM`, 0.25 mm).
2. At HEAD the lower levels stand further off because of the P41 guard itself. At level 16.55
   the strip is 8.95 mm tall, more than the limit of 6.5 mm, so the guard removes each centre
   whose disc meets it. The region boundary of the level is then the pass line of the level
   above, and the plan keeps its margin inside that line again. Measured: 0.4 mm at the second
   level, 0.71 mm at the fourth.
3. So no fix at the source of one margin gives the answer. With equal levels the state is the
   state before P41: the strip stands from the top (1 155.6 mm2 in the mech and slit pockets)
   and a lower level that touches it cuts it at its full height. The acceptance asks for less
   wood than that. The strip itself must be cut, one pass per level.
4. The margin cannot go to zero in the plan. `grown_outlines` says why: the region boundary is
   the tool-centre limit only to the grid, and a centre nearer than one planner cell reads the
   rim (D19). The Geodesic plan needs its smoothed outline.

## 3. The design

### 3.1 The rule (R-42)

After the plan of a level, the tool runs one contour along the boundary of each region that
the plan cut, where stock stands in its disc on cells where no centre of the level has
material to plan. Name: the wall pass of the level.

- The line. Each ring (the exterior and each hole) of the raw region polygon (the marching
  squares outline of the level grid), moved inside by half a planner cell. The raw boundary is
  at the middle of an in cell and an out cell; half a cell inside is the line of the outermost
  centres that the level grid itself accepted. So no accepted centre of this level or of the
  next is nearer to the wall than the line (the P41 guard then has nothing to hold at the same
  wall). The ring is simplified at the cut tolerance of the op and sampled at one cell, as the
  C4 band line is (`simplified_ring`, `resample_ring`). No new number: the cell and the cut
  tolerance.
- The floor of a point is the level floor (`lift_to_level`, the ONE level floor). The shipped
  path then passes the drape and the chord lift of every cut (`emitted_cut_path`), so the model
  plus the stock to leave is held as on every cut.
- The Z of a point. `z = max(floor, top - Depth/Pass)`, where `top` is the highest stock top
  less the profile height over the cells whose centre is inside the disc. So the pass never
  takes more than one Depth/Pass from a cell whose centre it covers. A cell that the disc
  covers in part is read by the P41 rule (`OnePass::blocks`): a point that it blocks is not
  cut.
- Where it runs ("stands"). A point stands when a cell that the disc covers in full holds
  stock more than one planner cell above the cutter surface (the margin of the C4 band read),
  and the cell is NOT a material cell of the level (`cell_has_material` on the stock after the
  plan). A material cell is the work of a plan: the region of this job, the region of another
  By Area job, or a region that the plan holds (the small-region bound, the rest region filter,
  the P41 guard). The wall pass does not take that work. On a slope the stock under the disc is
  on material cells, so the pass runs only at a wall that the tool cannot follow.
- Multiple passes. The read repeats on the stock after each round: where stock still stands
  and the last round lowered it, the next round runs one Depth/Pass lower. A point whose stock
  did not fall in the last round is not cut again (the chord lift held the path up, or the
  guard blocked it), so the loop ends with no count that is typed.
- Runs. The standing points of a ring make runs; `merge_short_gaps` (the one feed-or-retract
  rule) joins two runs over a short gap. Each run goes out as an entry and a cut through
  `push_segment_with_stamp`, so the planner stamps the path that the emitter ships, the entry
  gets its floor and proof from `plan_entry`, and the boundary clip applies.
- The load. Each move of a run is read by the one load (`step_load_on`) on the planner stock. A
  run over the pass ceiling goes out between `FeedCapStart` and `FeedCapEnd`
  (`FeedCapSource::LevelContour`), the pattern of the C4 contour. The emit loop of the C4
  contour and of the wall pass is ONE function.
- The marker. The runs follow a `WaterlineCleanup` marker, so the simulation reads them in the
  cleanup group (the wall cleanup of the op), with its own depth row.

### 3.2 Where it lives, and the switch

- Owner: `adaptive3d/level_contour.rs` (`region_wall_pass`), beside the C4 contour.
  `clear_z_level_slice` (Geodesic Adaptive) calls it after the regions of the level, for the
  regions whose plan cut a segment.
- Switch: the dial of P41, `Adaptive3dLinking::one_pass_hold`. `RemainingStock` now means: the
  level holds its tool off stock that is too tall, AND the level cuts to its walls. Reason: the
  wall pass needs the same premise (the planner cell is the simulation cell; on a fresh rough a
  planner cell of `R / 6` cannot show a strip 0.6 mm wide). No new dial, so no params literal
  moves. A fresh rough keeps its bytes.
- Contour Parallel on remaining stock does not get the pass in this package (its rings run to
  one cell of the boundary, and it has its own clean-up raster). The stored rivmap100 file is
  Contour Parallel.
- No depth rule at a segment door. The Z rule is in the plan of the pass.

### 3.3 What it does not do (stated before the code)

- A region under the small-region bound, a region that the F-038 forecast drops and a strip
  narrower than the tool have no plan, so they get no wall pass. Their cells are material
  cells, so a wall pass of a neighbour region does not take them (it stays one pass above
  them).
- The residual at a wall is the distance of the line from the true tool-centre limit: up to
  half a cell from the grid, plus the cut tolerance.
- The corner of two walls keeps its corner patch (a round tool).

## 4. Predictions (written before the code of the wall pass)

Setup "1 Back", HEAD against the tree.

1. Id 2 rows: body at most 6.5 mm, and its deepest sample at most 6.5 mm; entry at most
   2.5 mm; link 6.0 mm. The cleanup row rises from 3.0 mm to at most 6.5 mm (the wall pass is
   in that group and takes up to one Depth/Pass).
2. Replay: 0 pieces deeper than 6.3 mm. Some pieces between 6.0 and 6.3 mm (6 at HEAD).
3. Wood left at the end of the setup, mech and slit pockets: under 900 mm2 (1 631.5 at HEAD,
   1 155.6 before P41). The strip at each wall that a planned region touches goes; the dropped
   small regions and the corners stay.
4. Step shelf: under 773.8 mm2, not under 600 (the patch of 83 mm2 is a P41 hold on material
   cells; the wall strip of the shelf goes). The 8.6 mm slot: 412.6 mm2 within 10 (a region
   that no plan cuts). Cavity interior: at most 570 mm2. Cavity wall band: within 30 mm2 of
   63.7.
5. The centre cells that the P41 guard holds (the sum over the level grids, 79 545 at HEAD)
   fall to under half.
6. "Day strip slit V60": at most 7.44 mm of wood over its tip, and at most 7 moves with wood
   above the flute. This one is the least sure: the wood at (345.8, 95.0) is not located
   against a region boundary.
7. Cut below the target after the two roughs: the raw figure RISES above 12.1 mm2 (to between
   12 and 40 mm2), because a pass one tool radius from a wall line lowers replay nodes on the
   line (the instrument limit of P38 section 7.1). With the target lowered over one node: at
   most 0.4 mm2 (0.2 at HEAD). So the raw bar of the acceptance is predicted to fail by the
   raster, not by a gouge.
8. Time of id 2: 600 to 760 s (522 s at HEAD). Ramp entries: 200 to 350 (118). The setup rises
   by the same seconds.
9. Collisions 0, rapid collisions 0, failed checks 0 on each op. Id 1: the same bytes.
10. rivmap100: the stored file (Contour Parallel) keeps its bytes. The Geodesic Adaptive
    scratch copy moves (its rest rough has walls).
11. Setup "3 Front", id 21: body at most 4.31 mm; time 868 s to at most 930 s (terrain: the
    pass runs only at steep walls).
12. Gates. Fresh-stock fixtures do not move: op golden (no cell outside the 52), FM1, the byte
    parity sentry. Sentries that run a session rest rough can move, because the dial is on
    there: the D44, D48, D50 and D55 sentries, `a_rest_op_slower_than_fresh_is_told_r3`. A red
    there is read before any change.

## 5. The wall pass as built (three changes to section 3, each after a measurement)

Runs, all on setup "1 Back": `w1` is section 3 as written. `w2` adds change 1. `w3` adds
change 2. `w4` is a trial that is NOT in the tree (5.4). `w6` is the final tree (change 3); its
G-code of setup "1 Back" is the `w3` file, byte for byte (`cmp`). CLI md5 of `w6`:
`f018d13d6f4a3fbfa01837ea3bf01542`.

### 5.1 What `w1` measured (the line half a cell inside the raw outline)

- Simulation: the deepest sample of id 2 is 6.0 mm in each group; 0 samples over 6.5 mm.
- Wood: mech and slit pockets 773.8 mm2, cavity interior 309.8 mm2; the slit V-bit 7.44 mm and 7
  moves. Time of id 2: 653 s.
- Replay: 15 pieces deeper than 6.3 mm (27 nodes, the deepest 19.92 mm). Each node is 2.92 to
  3.00 mm from its piece: the node at the edge of the 3.0 mm disc.
- The cause. The simulation samples at those moves read 0 to 2.5 mm of depth: the simulated
  stock has no tall stock in those discs, and the replay stock has. The two stock models differ
  at a cell on the edge of an earlier disc: the simulation lowers such a cell by its covered
  part at each stamp, the replay lowers a node only when its centre is inside. The truth is
  between: the tool meets a sliver, a part of one cell wide, at the full height.
- Why HEAD has none: its plan stands 0.55 to 0.75 mm inside the raw region boundary, three
  nodes. P41 states it: the one-pass rule has no margin of its own and counts on that margin.
  The line of section 3.1 had only half a cell. It also broke the rule of `grown_outlines`
  (D19): a centre nearer than one planner cell to the raw boundary reads the rim. `w1` shows
  it: the wall line hops from Z 8.55 to 12.33 and back inside 0.33 mm.

### 5.2 Change 1: the wall pass keeps one planner cell off held stock

A point is not cut when a MATERIAL cell within one planner cell of the disc stands more than the
one-pass limit above the cutter (`WallRead::beside_held_stock`): stock that a plan holds (the
P41 guard, the small-region bound, another By Area job). `w2`: 9 deep pieces on the replay (14
nodes), pockets 777.2 mm2. Kept, but not enough: the other pieces meet tall stock on cells that
are not material cells (a strip at a wall where no upper level ran).

### 5.3 Change 2: the line is one planner cell plus the cut tolerance inside the raw outline

The inset is `cell + cut tolerance`, so that after the simplification each point is one planner
cell or more inside the raw boundary: the least distance of a centre from that boundary
(`grown_outlines`, D19), and the margin that the P41 rule counts on. `w3`: 0 deep pieces on the
replay (the deepest 6.00 mm). The price: the pass reaches less, by half a cell plus the cut
tolerance, and the pockets hold 964.0 mm2 (777 with the half cell).

### 5.4 A trial that is not in the tree: the line on the grid with no centre held off (`w4`)

The lib fixture (section 6) showed a weakness of 5.3. The line follows the boundary of the
PLANNED region. Where the P41 guard holds centres off a strip, that boundary is further from
the wall, and the wall pass moves with it. So the pass cuts the margin of each level, but it
does not reach the strip that the level above left.

- The trial: the line on the level grid built with NO centre held off (the same line at each
  level of one wall). On the fixture: 0.2 mm of wood at the foot (0.6 mm with change 2).
- On the job (`w4`): pockets 848.4 mm2, and 5 pieces deeper than 6.3 mm on the replay (25 nodes,
  to 14.23 mm). Four of them are steps of the wall line from Z 11.0 to 19.5 inside 0.2 mm: the
  line goes through centres that the guard holds, the Z rule lifts a point over tall stock, and
  the point beside it is not lifted. It fails the replay bar, so it is not in the tree.
- The patch is `phases/p42/not_landed/w4_wall_line_before_the_hold.patch` (the whole `crates`
  diff of that tree against HEAD). To land it, the Z of a point must read the tall stock that
  the next points of the line read (decision D-3).

### 5.5 Change 3: the wall pass runs only where the disc holds a wall

Measured on setup "3 Front" (terrain) with `w3`: id 21 went from 868 s to 1 397 s. On a slope
the stock that a flat tool leaves between two draped levels stands on cells that are not
material cells, so section 3.1 reads the whole stair as a strip.

- The rule: a point stands only when a cell of the slope map of the tool-centre surface inside
  the tool disc is as steep as the wall cleanup asks (`beside_a_wall`,
  `clearing::WALL_STEEP_ANGLE_RAD`: the 30 degrees of `waterline_cleanup`, now one constant for
  both).
- Result (`w6`): setup "1 Back" keeps its bytes. Id 21 is 1 333 s. So the rule takes off only
  64 s of the 529 s: the terrain has slopes of 30 degrees or more at most places where the
  pass ran. This is decision D-1.

### 5.6 The code (in the patch `not_landed/w6_wall_pass.patch`)

- `adaptive3d/level_contour.rs`: `region_wall_pass`, `wall_read` (`WallRead`), `beside_a_wall`,
  `WallPass`. The cap read and the emit loop of the C4 contour are now `run_feed_cap` and
  `emit_runs`; `level_contour_pass` and the wall pass both call them. A thread-local test
  control (`WALL_PASS_OFF_FOR_CONTROL`, `cfg(test)` only) gives the lib test its red state.
- `adaptive3d/clearing.rs`: `clear_z_level_slice` collects the regions whose plan cut a segment
  and calls the pass after the level; `ClearZLevelContext::wall_pass` and `slope_map`;
  `WALL_STEEP_ANGLE_RAD`; `cell_has_material` and `lift_to_level` are `pub(super)`.
- `adaptive3d/mod.rs`: `OnePassHold::RemainingStock { link_kinematics }` (the variant now
  carries the link timing of the gap merge, so the type is no more `Copy` and `Eq`).
  `compute/execute/rough_3d.rs` fills it; `path.rs` builds the context.
- `--lib each_level_cuts_to_the_wall_that_it_can_reach_p42`.

## 6. The lib fixture (`wall_strip_fixture_p42`)

A floor at Z 0, a vertical wall at X 10 to Z 9, a block of remaining stock 9 mm tall in front of
it; cell 0.2 mm, tool 6.35 mm, Depth/Pass 3 mm, levels 6, 3 and 0.5. Measured after a
simulation of the toolpath: the width of the wood on the line Y 0 that stands more than 1 mm
above the last floor, in front of the wall.

| State | Wood at the foot of the wall | Deepest sample |
|---|---|---|
| The dial off (no guard, no wall pass) | 0.4 mm | 3.0 mm |
| The guard, no wall pass (HEAD) | 1.2 mm | 3.0 mm |
| The guard and the wall pass (change 2; the final tree holds the same bound in its test) | 0.6 mm | 3.0 mm |
| The trial of 5.4 | 0.2 mm | 3.0 mm |

- The test holds: with the pass at most three cells (0.6 mm), and with no pass wider by two
  cells or more. The second assert is the red state: the test fails without the wall pass.
- What the table says against the goal: the wall pass halves the wood that the guard added. It
  does not bring the wall back to the state with no guard (0.4 mm), because the line follows the
  planned region (5.4).
- A fixture on a 0.5 mm cell shows nothing: there the margin of the plan (0.55 mm) is not more
  than one cell plus the tolerance, so the wall line is not nearer to the wall than the plan.
  The wall pass gains only where the planner cell is under about 0.4 mm.

## 7. Measured (setup "1 Back", the proposal file; HEAD against the final tree `w6`)

| | Before (HEAD) | After |
|---|---|---|
| Deepest cut of "Back rest 6 mm" in its clearing passes: the depth row; the deepest sample | 6.0 mm; 6.2 mm | 6.0 mm; 6.0 mm |
| The same in its ramp entries | 2.00 mm | 1.908 mm; deepest 1.915 mm |
| The same in its links | 6.0 mm | 6.0 mm; 6.0 mm |
| The same in its wall cleanup (the wall pass is in this group) | 3.0 mm | 6.0 mm; 6.0 mm |
| Pieces of that op deeper than 6.3 mm on the replay of the G-code | 0 (the deepest 6.23 mm; 6 between 6.0 and 6.3) | 0 (the deepest 6.00 mm; 0 between 6.0 and 6.3) |
| Time of "Back rest 6 mm" (feed at the emitted feeds) | 522 s, 35.45 m fed, 3 461 mm of rapids | 591 s, 39.37 m, 4 152 mm |
| Its moves; its ramp entries; entries that the model clip touched | 14 409; 118; 3 | 15 901; 122; 2 |
| Time of the setup (`total_runtime_s`) | 9 112.8 s | 9 187.7 s |
| "Back bulk rough 18 mm" (fresh) | 14 626 moves | the same G-code section, line for line |
| "Cavity rest pocket 6 mm" | 238 s, 6 321 moves, body 2.25 mm | 238 s, 6 321 moves, body 2.25 mm; the same section |
| Cut below the target after the two roughs: raw | 12.1 mm2 (302 nodes) | 12.2 mm2 (306 nodes) |
| The same with the target lowered over one node | 0.2 mm2 (4 nodes, to 0.66 mm) | 0.1 mm2 (2 nodes, to 0.49 mm) |
| Ramp nodes of id 2 under the target | 0 | 0 |
| Collisions, rapid collisions, failed checks (each op) | 0, 0, 0 | 0, 0, 0 |
| Export warning lines | 24 | 24 |
| Wood left at the end, mech and slit pockets | 1 631.5 mm2, to 20.70 mm, 17 228 mm3 | 964.0 mm2, to 20.70 mm, 12 259 mm3 |
| Wood left, cavity interior | 570.2 mm2 | 408.4 mm2 |
| Wood left, step shelf | 773.8 mm2 | 773.8 mm2 |
| Wood left, the 8.6 mm slot | 412.6 mm2, 21.0 mm | 412.6 mm2, 21.0 mm |
| Wood left, cavity wall band | 63.7 mm2 | 63.7 mm2 |
| Stock that the P41 guard still holds at the end of the plan of id 2 (the finding text) | 710.8 mm2 | 553.3 mm2 |
| Flute contact, "Day strip slit V60" | 8 moves, 10.60 mm of wood over the tip | 8 moves, 7.44 mm |
| Flute contact, "Pin drill, flip pins" | 5 moves, 30.00 mm | 5 moves, 30.00 mm |

Before P41 the mech and slit pockets held 1 155.6 mm2 and the slit V-bit met 7.44 mm on 7 moves.

Notes on the table:

- The raw figure below the target is 4 nodes more (6 new nodes, 2 gone). Each new node is on a
  step line of the target raster: the target at the node is 11.0, 8.5 or 12.27 mm and the
  lowest target within one node is 8.4, 3.5 or 7.27 mm. This is the raster limit of P38
  section 7.1. With the target lowered over one node the figure is better than HEAD (2 nodes,
  both on the step shelf at (54.6, 118.0), there at HEAD too). The export of both runs holds no
  `safety.cuts_into_model` warning; the 24 warning lines are the same text.
- One later section of the G-code changes with the same number of lines (the 2 mm tool: its
  ops read the stock that id 2 leaves). The other later sections are the HEAD sections.
- Every limit row of every op was compared (`summary.json`, 300 rows, state, value and bound).
  Rows of id 2 only move. One state changes: the wall cleanup depth row, within (3.0 mm against
  3.0005 mm) to exceeds, advisory (6.0 mm against 4.54 mm). The entry depth row goes from 2.00
  to 1.91 mm; the cleanup power and deflection rows rise and stay within (0.142 to 0.164 kW,
  0.014 to 0.016 mm). No row of a later op moves.
- The step shelf, the slot and the wall band do not move: no region of id 2 was planned beside
  that wood, or the wood stands on material cells that the guard holds (the shelf patch, P41
  D-6).

### 7.1 Setup "3 Front" (id 21, "Front 3D rough": remaining stock, By Area, 5.532 mm per pass)

| | Before (HEAD) | After (`w6`) |
|---|---|---|
| Id 21 body row; deepest sample | 4.31 mm; not stated | 4.02 mm; 4.31 mm |
| Id 21 entry row; deepest sample | 1.56 mm | 1.82 mm; 1.88 mm |
| Id 21 wall cleanup row; deepest sample | no sample | 4.01 mm; 5.40 mm |
| Id 21 link row | 0.48 mm | 0.49 mm; 0.50 mm |
| Id 21 time, fed length, rapids, moves | 868 s, 55.78 m, 12 464 mm, 38 903 | 1 333 s, 85.27 m, 18 218 mm, 62 820 |
| Id 20 (fresh): rows, moves | body 4.16 mm, 22 670 | not changed |
| Id 23, the scallop finish: body, entry, link rows; time | 7.53, 5.78, 4.07 mm; 4 630 s | 7.53, 2.31, 3.15 mm; 4 630 s |
| Setup `total_runtime_s`; collisions | 8 750.0 s; 0, 0, 0 | 9 443.2 s; 0, 0, 0 |

- The one-pass limit of id 21 is 6.03 mm; its deepest sample is 5.40 mm.
- The wall pass costs 465 s on this op (54 %). It cuts the stock that the flat tool leaves on
  the steep slopes between two levels. The finish then meets less wood at its entries (5.78 to
  2.31 mm) and links (4.07 to 3.15 mm); its time does not change.
- Every limit row was compared (126 rows). Id 21: the four cleanup rows are new (the group had
  no sample): chip within, depth exceeds advisory (4.01 mm against 3.48 mm), power and
  deflection within. Its body depth row stays exceeds (4.31 against 3.05 to 4.02 against 3.00).
  Id 23: no state changes; its entry and link values fall.
- The replay instruments (`deep.py`, `gapmap.py`) hold the target of setup "1 Back" only. No
  replay was made of setup "3 Front".

### 7.2 rivmap100

- `planning/fixtures/rivmap100/rivmap100_live_0925.toml` (Contour Parallel, remaining stock):
  the same G-code as HEAD, byte for byte. The wall pass is in the Geodesic slice only.
- The Geodesic Adaptive scratch copy (`clearing_strategy = "geodesic_adaptive"`; no such file is
  in `planning/fixtures/`): the G-code moves. 6 780 to 6 937 moves, `total_runtime_s` 1 229.3 to
  1 244.8, body 8.0 mm and entry 1.45 mm as before, a wall cleanup row of 0.82 mm, 0
  collisions. The cause is the wall pass: 157 moves along the walls of the rough.

### 7.3 Predicted against measured

| # | Prediction | Result |
|---|---|---|
| 1 | id 2 rows and deepest samples at most 6.5 (entry 2.5); the cleanup row rises to at most 6.5 | TRUE: 6.0, 1.91, 6.0, and cleanup 3.0 to 6.0 |
| 2 | 0 deep pieces on the replay; some between 6.0 and 6.3 | TRUE for the final tree (0, and 0 between). FALSE for the design as written: `w1` had 15 (5.1) |
| 3 | mech and slit pockets under 900 mm2 | FALSE: 964.0 mm2. It is under the value before P41 (1 155.6). The line as written gave 773.8 and failed the replay |
| 4 | step shelf under 773.8 and not under 600; slot within 10 of 412.6; interior at most 570; wall band within 30 of 63.7 | FALSE for the step shelf (773.8, no change). TRUE for the slot (412.6), the interior (408.4) and the band (63.7) |
| 5 | the held centre cells fall to under half | NOT MEASURED as a count (the CLI log has no line of the guard at its level). The held stock in the finding text falls from 710.8 to 553.3 mm2, 22 %: the sense of the prediction is false |
| 6 | the slit V-bit: at most 7.44 mm and at most 7 moves | TRUE for the wood (7.44 mm). FALSE for the moves (8, as at HEAD; 7 before P41) |
| 7 | raw below the target rises to 12 to 40 mm2 by the raster; lowered at most 0.4 mm2 | TRUE: 12.2 mm2, each new node on a step line; lowered 0.1 mm2 |
| 8 | id 2: 600 to 760 s; 200 to 350 ramp entries | FALSE on both, in the good sense: 591 s, 122 entries |
| 9 | collisions 0; id 1 the same bytes | TRUE |
| 10 | rivmap100 keeps its bytes; the Geodesic copy moves | TRUE |
| 11 | id 21: body at most 4.31; time at most 930 s | TRUE for the depth. FALSE for the time: 1 333 s (5.5, D-1) |
| 12 | fresh fixtures do not move; session rest roughs can move | section 8 |

## 8. Gates

### 8.1 Gates of the patch (HEAD plus step 0 plus `w6_wall_pass.patch`)

Tree `w6` (two test lines of step 0 changed after it, see below). Logs: the scratch dir,
`gate_w6_*.log` and `gate_w6b_clippy.log`. No red is new against HEAD.

| Gate | Result |
|---|---|
| core `--lib` | 2 853 passed, 1 failed, 13 ignored. The red is the known one, `feeds::envelope::tests::the_envelope_meets_its_time_budget`. |
| adaptive3d sentries (37 binaries, `test-support,heavy-tests`, `--no-fail-fast`) | 75 passed, 0 failed, 7 ignored |
| dressup sentries (19 binaries) | 119 passed, 5 ignored |
| other core sentries (20 binaries: the 11 of P41, 8 of `tool_load` and `limits` for step 0, `a_rest_op_slower_than_fresh_is_told_r3`) | 141 passed |
| op golden | red with 52 cells: the 46 known cells and the 6 P39 cells of `adaptive3d/ramp_geodesic`, the cells of HEAD line for line (`diff` against the P41 list). Not blessed. |
| FM1 (`feeds_matrix_instrument_fm1`, `heavy-tests`) | 2 passed; no cell moves |
| `rs_cam_cli` (17 binaries) | 113 passed |
| `rs_cam_mcp` | 39 passed |
| viz `--lib` (`--test-threads 2`, no `rs_cam_gui` process) | 705 passed, 2 ignored |
| full clippy (`--workspace --all-targets`, the three core features, `-D warnings`) | first run red in two tests of step 0 (an index of a JSON value, an `expect`); both test lines were written again with no change of an assert; second run clean |
| `rustfmt --check` on each changed `.rs` file (12 files) | clean; no other file was formatted |
| `folder_maps_name_real_files_f140` (after the last text of `adaptive3d/CLAUDE.md`) | 4 passed |

After the gate set only those two test lines changed. Run again on the last tree:
`--lib -- _p42 tool_load::depth limits:: _p41 _c4` (31 passed) and full clippy (clean). The
release CLI in `target/release` has the md5 of the measured build `w6`.

Prediction 12 against this: the fresh fixtures did not move (true). No sentry of a session rest
rough went red (the prediction named them as possible; none moved its asserts).

Step 0 alone: its lib tests and 8 sentries of `tool_load` and `limits` ran green on the tree
that held only step 0, before the wall pass was written (`s0_lib.log`, `s0_sentries.log`).
Section 8.2 holds the full gate set of step 0 alone, after the two lint lines.

### 8.2 Gates of the tree as left (HEAD plus step 0; the wall pass reversed)

Logs: the scratch dir, `gate_s0_*.log`. No red is new against HEAD.

| Gate | Result |
|---|---|
| core `--lib` | 2 852 passed, 1 failed, 13 ignored: the known red `feeds::envelope::tests::the_envelope_meets_its_time_budget` |
| adaptive3d sentries (37 binaries) | 75 passed, 0 failed, 7 ignored |
| dressup sentries (19 binaries) | 119 passed, 5 ignored |
| other core sentries (20 binaries, the 8 of `tool_load` and `limits` in them) | 141 passed |
| op golden | red with the 52 cells of HEAD, line for line. Not blessed. |
| FM1 | 2 passed |
| `rs_cam_cli` (17 binaries); `rs_cam_mcp` | 113 passed; 39 passed |
| viz `--lib` (`--test-threads 2`, no `rs_cam_gui` process) | 705 passed, 2 ignored |
| full clippy | clean |
| `rustfmt --check` on the 6 changed `.rs` files | clean |
| Release CLI of this tree, setup "1 Back" | the G-code of HEAD, byte for byte; md5 `88310715e4c8e54727e3d46196569fcc` is the binary in `target/release` |

## 9. Decisions for the lead

- D-1 (the cost on terrain; the first thing to rule). On "Front 3D rough" the wall pass adds
  465 s (868 to 1 333 s). The slope rule of the wall cleanup takes off only 64 s. The pass is
  correct there by its rule (stock on cells where no centre has material, one pass deep), and
  the finish meets less wood. The choices: keep it; make the wall pass an option of the op
  beside "Contour each level" (default to rule); or ask for a steeper wall, which needs a number
  with a source (none is in the repo).
- D-2 (the acceptance, raw figure). Cut below the target, raw: 12.2 mm2 against the bar of 12.1
  (4 nodes, each on a step line of the target raster). With one node lowered: 0.1 mm2 against
  0.2. The bar failed as written, so the code is reversed. The miss is the raster limit that
  prediction 7 named. If the lead rules that the one-node figure is the bar, the patch lands
  with `git apply phases/p42/not_landed/w6_wall_pass.patch`; its commit message and path list
  are beside it, and its gates are section 8.1.
- D-3 (the wall line and the guard). The line follows the planned region, so where the guard
  holds centres the pass does not reach the strip of the level above (5.4, section 6). The
  trial on the grid with no hold gives 848 mm2 in the pockets (964 in the tree) and 0.2 mm on
  the fixture, and fails the replay at the Z steps of the line. Next step if wanted: the Z of a
  wall point reads the stock inside the discs of its neighbours on the line, then the trial
  again.
- D-4 (the switch). The wall pass is on with `OnePassHold::RemainingStock`; there is no dial of
  its own, and the variant now carries the link timing. A separate dial needs one line in each
  params literal (23 test files).
- D-5 (what the wall pass does not cover; measured, not forced). The 8.6 mm slot (412.6 mm2,
  21 mm tall), the step shelf (773.8 mm2) and the wall band are as at HEAD. 553 mm2 of stock is
  still held by the guard. These need the planner for a narrow region (P38 W4 point 3) or the
  op of P37 D-5.
- D-6 (Contour Parallel). A Contour Parallel rough on remaining stock gets no wall pass. Not
  measured whether it needs one.
- D-7 (the slit V-bit). The wood over its tip is back to 7.44 mm. It has 8 moves with wood above
  the flute, as at HEAD (7 before P41).
- D-8 (step 0, the surfaces). The deepest sample is on the wire (`limit_rows[..].deepest`) and
  in the detail sentence. No row face shows it. If the operator must see it in the row, the
  face rule of `limits/row.rs` needs a ruling (pins in viz tests hold the faces).
- D-10 (the wall cleanup depth row). With the patch that row of id 2 exceeds the force budget
  (6.0 mm against 4.54 mm), advisory, as the body row does at HEAD. The wall pass cuts a strip
  about 0.4 mm wide one Depth/Pass deep; the load cap reads its chip area, not the force budget
  depth. If that row must stay within, the Z rule of the wall pass needs a lower step than one
  Depth/Pass, and a rule for it.
- D-9 (the P41 note). Its text says the replay nodes are half a planner cell from the planner
  cells. The wall lines of this job lie on multiples of 0.2 mm in the G-code frame, so the
  replay nodes are at planner cell centres or the grid origin is not what that text assumes.
  Not settled here; it does not change a figure of this note.

## 10. Round 2: the lead rulings (2026-11-18)

Start: HEAD `1875dd3f8` (step 0 `1ef06d1fe` and the record are committed) plus
`not_landed/w6_wall_pass.patch`. Sections 10.1 to 10.3 were written before the code of round 2.
Where this section and an earlier section differ, this section is the state of the trial.

### 10.1 D-10 first: why the wall cleanup row read 6.0 mm against a cap of 4.54 mm (measured)

The patch build again with the sample file (ops 0 to 2). 14 203 removing samples of id 2 are in
wall cleanup moves; 4 704 of them have an arc.

- The judged sample: depth 6.00 mm, width 4.27 mm (from its arc), 0.0606 mm/tooth (2 182
  mm/min: the load cap of the run was on). The cap is the depth where the peak force at that
  width and chip meets 35.3 N: 4.54 mm.
- So the cause is the WIDTH, not the feed. The pass is thin at most samples (median width 0.2
  to 0.4 mm), but 16 % of the samples with an arc are wider than 1.05 mm, and the widest is
  5.8 mm. 214 of them are deeper than 3 mm. These are places where the plan left more than its
  margin strip (the end of a run in a corner, a neck).
- The force model at one pass of depth (a probe of `feeds::force_budget::peak_force_n`, 6 mm
  flat tool, white oak, 6.0 mm deep, the stated 0.1285 mm/tooth; budget 35.3 N):

| Width | Peak force at the stated chip | Chip that meets the budget |
|---|---|---|
| 0.4 mm | 17.5 N | over 0.2 mm |
| 0.65 mm | 24.0 N | over 0.2 mm |
| 1.0 mm | 32.2 N | 0.160 mm |
| 1.5 mm | 42.7 N | 0.078 mm |
| 2.54 mm | 56.2 N | 0.040 mm |
| 4.27 mm | 66.1 N | 0.030 mm |

- A strip up to about 1.1 mm wide is within the budget at the stated feed and one full pass. A
  wider cut needs a third to a quarter of the stated chip.
- The existing cap law cannot give that. It holds the chip-area load at the pass ceiling
  (`ceiling / load`), and the body cut at that ceiling is itself over the force budget (the
  body row: 6.0 mm against 3.2 mm, at HEAD too). The planner holds no force model: the machine
  budget does not reach `ExecutionContext` (29 literals build it).
- The fix at the cause: the wall pass cuts the margin strip of the plan and nothing wider. A
  point is not cut when stock that stands more than one planner cell above the cutter reaches
  deeper into the disc than the widest strip that the margin of the plan leaves:
  `SIMPLIFY_INSET + SIMPLIFY_TOLERANCE + PERIMETER_INSET_MARGIN_MM` (0.85 mm, the three numbers
  of the plan; no new number). Wider stock is the work of a plan, not of the wall pass. At
  0.85 mm, and one cell more for the read of the simulation, the force at one pass is under the
  budget on this job (32 to 33 N). The load cap of the C4 pattern stays.
- This is NOT the fix that the ruling names (a feed cap). The reason is the two bullets above.

### 10.2 D-1: the wall pass ships only where the next level would meet the strip too tall

- The rule. A point of the wall line of level N stands only when the strip top is more than
  the one-pass limit above the cutter of the NEXT level at that point:
  `OnePass::over_limit(strip top - floor of the point at the next level)`. The floor is the ONE
  level floor (`intended_floor::level_floor_z`, through `lift_to_level`) at the Z of the next
  level of the op (the next level of the By Area job). With no next level the floor is the
  floor of this level: the last level ships only where the strip is itself taller than the
  limit (it then goes down in rounds).
- `OnePass::over_limit` is the read of `one_pass.rs` (the limit against stock above a cutter);
  `deep_cell` and the wall pass both call it.
- On a slope a draped point has the same floor at the next level, and the tread of the stair is
  one Depth/Pass or less: no pass. At a pocket wall two levels deep the strip of level N stands
  two passes above the next floor: the pass ships.
- The slope rule of round 1 (`beside_a_wall`, `WALL_STEEP_ANGLE_RAD`, the slope map in the
  context) is removed.
- A consequence, stated before the measure: the LAST level at a wall ships no pass (its strip
  is one pass tall or less). So the margin strip of the last level stays at the foot of each
  wall, as wide as before P41 (0.65 mm), one pass tall.

### 10.3 Predictions of round 2 (before the code)

1. "Front 3D rough": 868 to 930 s. No wall cleanup row, or a row that is within.
2. Setup "1 Back", id 2: 540 to 590 s (fewer wall points than the patch: no last level, no wide
   point).
3. Mech and slit pockets: 1 050 to 1 200 mm2. The instrument counts area. The foot strip of the
   last level has the footprint of the strip before P41, so the area comes back toward
   1 155.6 mm2 while the volume falls. The bar "under 1 155.6" can fail on this figure.
4. The wall cleanup row of id 2: within (depth at most 6.0 mm against a cap over 6.0 mm, or the
   HEAD judged sample 3.0 against 3.0005).
5. Id 2 body row and deepest sample 6.0 to 6.2 mm; entry at most 2.5 mm; 0 replay pieces deeper
   than 6.3 mm.
6. The slit V-bit: 7.44 to 10.60 mm. Not sure: the wood at the end of the slit may be a place
   where only the last level reaches.
7. One node below the target: at most 0.2 mm2. Collisions 0. Id 1 and rivmap100 the same bytes.

### 10.4 Round 2 measured: two bars fail, the code is reversed

Build `r2` (HEAD `1875dd3f8` plus `not_landed/r2_wall_pass_next_level_rule.patch`; CLI md5 in the
scratch dir, `cli_r2`). One run of each setup.

Setup "1 Back", HEAD against `r2` (the round 1 patch `w6` beside it):

| | HEAD | Round 1 patch (`w6`) | Round 2 (`r2`) | Bar |
|---|---|---|---|---|
| id 2 body row; deepest sample | 6.0; 6.2 mm | 6.0; 6.0 mm | 6.0; 6.0 mm | at most 6.5: holds |
| id 2 entry row; deepest sample | 2.00; 2.00 mm | 1.91; 1.92 mm | 1.89; 1.92 mm | at most 2.5: holds |
| id 2 wall cleanup row | within, 3.0 against 3.0005 mm | exceeds, 6.0 against 4.54 mm (width 4.27 mm, 0.0606 mm/tooth) | exceeds, 6.0 against 4.06 mm (width 3.10 mm, 0.0812 mm/tooth; 295 samples judged) | within: FAILS |
| Replay pieces deeper than 6.3 mm | 0 (deepest 6.23) | 0 (6.00) | 0 (6.20; 2 between 6.0 and 6.3) | 0: holds |
| Wood left, mech and slit pockets | 1 631.5 mm2, 17 228 mm3 | 964.0 mm2, 12 259 mm3 | 1 387.5 mm2, 13 667 mm3 | under 1 155.6 mm2: FAILS |
| Wood left, cavity interior | 570.2 mm2 | 408.4 mm2 | 570.4 mm2 | |
| Wood left, step shelf; slot; wall band | 773.8; 412.6; 63.7 mm2 | the same | the same | |
| Stock that the guard still holds (the finding text) | 710.8 mm2 | 553.3 mm2 | 578.1 mm2 | |
| "Day strip slit V60": wood over the tip; moves above the flute | 10.60 mm; 8 | 7.44 mm; 8 | 7.44 mm; 8 | at most 7.44: holds |
| Below the target: raw; one node lowered | 12.1; 0.2 mm2 | 12.2; 0.1 mm2 | 12.1; 0.1 mm2 (2 nodes, to 0.49 mm) | one node at most 0.2: holds |
| Collisions, rapid collisions (each op) | 0, 0 | 0, 0 | 0, 0 | holds |
| id 1 | | the same section | the same section, line for line | holds |
| Time of id 2; fed; rapids; moves; ramp entries | 522 s; 35.45 m; 3 461 mm; 14 409; 118 | 591 s; 39.37 m; 4 152 mm; 15 901; 122 | 633 s; 42.91 m; 6 288 mm; 15 194; 166 | |
| Setup `total_runtime_s` | 9 112.8 | 9 187.7 | 9 272.1 | |
| Export warning lines | 24 | the same 24 | the same 24 | |

Setup "3 Front": the G-code of `r2` is the HEAD file, byte for byte (`cmp`). Id 21: 868 s,
38 903 moves, no wall cleanup sample; 0 collisions. The bar (at most 930 s, rows not worse) holds.

rivmap100 was not run on `r2` (a bar had failed).

What the two failures say:

- The pocket wood (prediction 3 was 1 050 to 1 200 mm2; measured 1 387.5). The next-level rule
  takes the pass away at the last level of each wall, and the width rule takes it away at each
  wide place. Both leave area. The volume still falls (17 228 to 13 667 mm3), and the slit
  V-bit is at 7.44 mm. The wood is 244 mm2 under HEAD and 232 mm2 over the value before P41.
  The share of each rule was not measured (one round).
- The wall cleanup row (prediction 4 was "within"). The planner read of the width (the cell
  nearest to the tool axis that stands more than one cell above the cutter, at each point of
  the line) kept each point under 0.85 mm. The simulation still reads a sample 3.10 mm wide at
  6.0 mm of depth. So the two reads of the width do not agree at some place. Not found which:
  the candidates are the first move of a run after its entry, the chord between two points,
  and stock under one cell tall beside taller stock. The run is capped (0.0812 mm/tooth, 63 %
  of the stated chip); the force budget at 3.10 mm asks for about 30 %.
- Why a feed cap by the existing law cannot close it (10.1): the law holds the chip-area load
  at the pass ceiling, and the body cut at that ceiling is over the force budget at one
  Depth/Pass (the body row exceeds at HEAD: 6.0 against 3.2 mm). A cap that makes each wall
  sample within needs the force model in the planner: the feed factor is the chip that meets
  the budget at the width and depth of the move, over the stated chip (the table of 10.1). The
  machine budget, the tool and the material are not in the planner today.
- The entries: 166 against 122. Each refused wide point ends a run, and the next run gets an
  entry. That is the 42 s over the round 1 patch.

Predictions of round 2 against the measure: 1 TRUE (868 s, the HEAD file). 2 FALSE (633 s).
3 FALSE (1 387.5). 4 FALSE (exceeds). 5 TRUE. 6 TRUE (7.44). 7 TRUE for the part that ran.

### 10.5 The state after round 2

- The tree is HEAD `1875dd3f8`: `git diff HEAD -- crates` is empty. The release CLI was built
  again from it, and its G-code of setup "1 Back" was compared with the HEAD file (section 0
  holds the result).
- `not_landed/` holds three patches, each applies on HEAD (`git apply --check`):
  `w6_wall_pass.patch` (round 1: the pass at each level, the slope rule),
  `r2_wall_pass_next_level_rule.patch` (round 2: the next-level rule, the margin strip rule, no
  slope rule, `OnePass::over_limit`, `LevelSlot::next_z`), and
  `w4_wall_line_before_the_hold.patch` (the failed trial of 5.4).
- No gate set ran on `r2` (a bar failed first). Only `--lib -- _p42 _p41 _c4` ran on it: 10
  passed after the test bound was set to the measured 0.8 mm.

### 10.6 Open after round 2

- D-10 is the blocker. The row stays within only with (a) the force model in the planner for
  the cap of a wall run, or (b) a wall pass that never takes one full Depth/Pass (the ruling
  forbids a second step-down rule), or (c) a ruling that the row may exceed, advisory, as the
  body row does. For (a) the session must hand the planner the machine budget, the tool and the
  material; `ExecutionContext` has 29 literals.
- D-1 as ruled is met on terrain. On the pockets it costs 424 mm2 against the round 1 patch,
  together with the width rule. To know the share of each, run the round 2 patch with the
  width rule off.
- D-3, D-5, D-6, D-7, D-8, D-9: open, as section 9 states them. Not touched in round 2.

## 11. Round 3 (the last): the lead corrects D-10 and extends D-1 (2026-11-18)

Start: HEAD `b68262ebc` plus `not_landed/r2_wall_pass_next_level_rule.patch`. Sections 11.1 and
11.2 were written before the code of round 3.

### 11.1 The rules of round 3

- D-10 (new bar): the wall cleanup depth row may exceed, advisory. Its excess (judged depth over
  its own cap, the ratio of the row) must not be more than the excess of the body row of the
  same op in the same run. The width rule of round 2 (`PLAN_MARGIN_STRIP_MM`,
  `WallRead::width_mm`) is removed. No force model, no second step-down rule.
- D-1 (extended): a point of the wall line of level N ships when EITHER
  1. the strip top is more than the one-pass limit above the cutter of the next level at the
     point (round 2), OR
  2. the wall at the point is taller than one pass above the floor of this level:
     `OnePass::over_limit(wall top - floor of the point at this level)`.
- The read of the wall top (the lead asks which read exists): the MODEL wall top, not the seed
  stock top. It is the tool-centre surface plus the stock to leave at the strip cells
  (`SurfaceHeightmap::z_or_bbox_floor_at`, the read of the level grid): the highest of it over
  the cells that `wall_read` counts as the strip. A strip cell is nearer to the wall than the
  tool-centre limit, so a centre there stands on the wall top. The seed stock top is not kept by
  the planner after the first stamp (`PriorStockTops` exists only with "Contour each level").
- Why a stair ships nothing under (2): at a draped point the strip cells stand uphill by at
  most the slope times the tool radius. That is over the limit (6.03 mm on id 21) only where the
  terrain rises more than 6 mm inside 3 mm, a slope over 63 degrees.
- One function holds the rule (`level_contour::wall_point_ships`), with a lib test.

### 11.2 Predictions of round 3 (before the code)

1. "Front 3D rough": 868 to 900 s; rule (2) ships a pass only at a slope over about 63 degrees.
   It can be the HEAD file.
2. Mech and slit pockets: 960 to 1 050 mm2 (the round 1 patch gave 964.0; walls under one pass
   and the limit now ship nothing).
3. Id 2: 570 to 600 s; 120 to 135 ramp entries.
4. The wall cleanup row of id 2: exceeds, with a ratio of 1.2 to 1.5 (the round 1 patch: 6.0
   against 4.54, 1.32). The body row: 6.0 against 3.2, 1.88.
5. Id 2 body and deepest sample 6.0 to 6.2 mm, entry at most 2.5 mm, 0 replay pieces deeper than
   6.3 mm; the slit V-bit 7.44 mm; one node below the target at most 0.2 mm2; 0 collisions;
   id 1 and rivmap100 the same bytes; the Geodesic copy of rivmap100 moves by under 200 moves.

### 11.3 Round 3 measured: each bar holds

Build `r3` (HEAD `b68262ebc` plus the round 3 code; CLI md5 `7c4f9c3bfc89d1c68688b4528f2f11c4`). One run of each setup.

Setup "1 Back", HEAD against `r3`:

| | HEAD | Round 3 (`r3`) | Bar |
|---|---|---|---|
| id 2 body row; deepest sample | 6.0; 6.2 mm | 6.0; 6.0 mm | at most 6.5: holds |
| id 2 entry row; deepest sample | 2.00; 2.00 mm | 1.91; 1.92 mm | at most 2.5: holds |
| id 2 link row | 6.0 mm | 6.0 mm | |
| Replay pieces deeper than 6.3 mm | 0 (deepest 6.23) | 0 (deepest 6.20; 3 between 6.0 and 6.3) | 0: holds |
| id 2 body row: judged depth over its cap | 6.0 / 3.197 = 1.877 (exceeds, advisory) | 6.0 / 3.197 = 1.877 | |
| id 2 wall cleanup row: judged depth over its cap | 3.0 / 3.0005 = 1.00 (within) | 6.0 / 4.540 = 1.322 (exceeds, advisory; width 4.27 mm, 0.0606 mm/tooth; 2 080 samples judged) | not over the body ratio: holds |
| Wood left, mech and slit pockets | 1 631.5 mm2, 17 228 mm3 | 1 004.0 mm2, 12 363 mm3 | under 1 155.6: holds |
| Wood left, cavity interior | 570.2 mm2 | 570.4 mm2 | |
| Wood left, step shelf; slot; wall band | 773.8; 412.6; 63.7 mm2 | the same | |
| Stock that the guard still holds (the finding text) | 710.8 mm2 | 553.3 mm2 | |
| "Day strip slit V60": wood over the tip; moves above the flute | 10.60 mm; 8 | 7.44 mm; 8 | at most 7.44: holds |
| Below the target: one node lowered; raw | 0.2 mm2 (4 nodes); 12.1 mm2 | 0.1 mm2 (2 nodes, to 0.49 mm); 12.2 mm2 | one node at most 0.2: holds |
| Collisions, rapid collisions, failed checks (each op) | 0, 0, 0 | 0, 0, 0 | holds |
| id 1 | | the same G-code section, line for line | holds |
| Time of id 2; fed; rapids; moves; ramp entries | 522 s; 35.45 m; 3 461 mm; 14 409; 118 | 578 s; 38.87 m; 4 158 mm; 14 987; 131 | at most 620 s: holds |
| Setup `total_runtime_s` | 9 112.8 | 9 185.3 | |
| Export warning lines | 24 | the same 24 | |

- One state changes among the 300 limit rows: the wall cleanup depth row of id 2 (the D-10 bar
  of round 3 allows it). One later G-code section changes with the same number of lines (the
  2 mm tool reads the stock that id 2 leaves); the other later sections are the HEAD sections.
- The cavity interior keeps its wood (570.4 mm2; the round 1 patch cut it to 408.4): its walls
  are not deeper than one pass and the limit, so the ship rule gives them no pass.

Setup "3 Front", HEAD against `r3`:

| | HEAD | Round 3 | Bar |
|---|---|---|---|
| Id 21 time; fed; rapids; moves | 868 s; 55.78 m; 12 464 mm; 38 903 | 914 s; 58.69 m; 15 512 mm; 39 788 | at most 930 s: holds |
| Id 21 body row (judged over cap); deepest sample | 4.308 / 3.048 = 1.413 (exceeds, advisory); 4.31 mm | the same row; 4.31 mm | |
| Id 21 entry row; link row | 1.56 mm; 0.48 mm (within) | 1.59 mm; 0.48 mm (within) | |
| Id 21 wall cleanup row | no sample | 4.210 / 3.719 = 1.132 (exceeds, advisory); deepest sample 5.18 mm; chip, power and deflection within | not over the body ratio: holds |
| Id 23 (the finish): entry and link depth values; time | 5.78, 4.07 mm; 4 630 s | 2.57, 3.66 mm; 4 630 s | |
| Collisions | 0, 0, 0 | 0, 0, 0 | holds |

- The G-code of id 21 is NOT the HEAD file: rule (2) ships the pass where the terrain has a
  wall deeper than one pass and the limit (885 more moves, 46 s). The lead named a second form
  of the rule for the case that the Front ships passes; the time bar holds, so rule (2) stays.
- "Rows not worse": no row of the HEAD run changes its state. The entry depth value rises from
  1.56 to 1.59 mm, within. The wall cleanup rows are new.
- The one-pass limit of id 21 is 6.03 mm; its deepest sample is 5.18 mm.

rivmap100: the stored file (Contour Parallel) is the HEAD G-code, byte for byte. The Geodesic
scratch copy is ALSO the HEAD G-code, byte for byte: its rough has no wall that the rule ships.

Predictions of round 3 against the measure: 1 FALSE by 14 s (914 s, predicted at most 900).
2 TRUE (1 004.0). 3 TRUE (578 s, 131 entries). 4 TRUE (1.322). 5 TRUE, but the last part: the
Geodesic copy of rivmap100 does not move.

### 11.4 Gates of round 3 (the tree as left)

Logs: the scratch dir, `gate_r3_*.log`, `gate_r3b_clippy.log`, `r3b_lib.log`. No red is new
against HEAD.

| Gate | Result |
|---|---|
| core `--lib` | 2 854 passed, 1 failed, 13 ignored: the known red `feeds::envelope::tests::the_envelope_meets_its_time_budget` |
| adaptive3d sentries (37 binaries) | 75 passed, 0 failed, 7 ignored |
| dressup sentries (19 binaries) | 119 passed, 5 ignored |
| other core sentries (20 binaries) | 141 passed |
| op golden | red with the 52 cells of HEAD, line for line. Not blessed. |
| FM1 | 2 passed |
| `rs_cam_cli` (17 binaries); `rs_cam_mcp` | 113 passed; 39 passed |
| viz `--lib` (`--test-threads 2`, no `rs_cam_gui` process) | 705 passed, 2 ignored |
| full clippy | first run red: `region_wall_pass` had 8 arguments (limit 7). `WallLevel` now holds the Z and the next Z. Second run clean. |
| `rustfmt --check` on the 7 changed `.rs` files | clean |
| `folder_maps_name_real_files_f140` | in the other sentries: green |

### 11.5 Open after round 3

- D-3 (the wall line and the guard), D-5 (the slot, the step shelf, narrow regions), D-6
  (Contour Parallel), D-7 (the slit moves), D-8 (the row face of the deepest sample), D-9 (the
  grid origin text of the P41 note): open, as section 9 states them.
- New: the width of the wall pass at a corner or a neck (to 4.27 mm at one full pass on id 2).
  It is inside the D-10 bar of round 3. A force cap for it needs the force model in the planner
  (10.4).
- New: on terrain the pass ships at walls deeper than one pass (46 s on id 21). If that is not
  wanted, the second form of the ruling is "the level above shipped a pass within one tool
  radius".
