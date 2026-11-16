# P38: the rest rough cuts 20.7 mm deep with a stated depth per pass of 6 mm (2026-11-17)

Job: `planning/rivmap_block_job/nz_wanaka_21_lead_proposal.toml` at `c99a5ac37`, setup "1 Back"
alone, on scratch copies. All positions are in the setup frame unless the text says G-code frame
(G-code frame = setup frame + (18.5, 8.5); Z is the same).

Sections 1 to 4 were written before the fix was run or measured. The code of the rule and its
test were written before the note (the order the task asks for was not kept); no result of the
fix was known when the predictions were written. Section 5 holds the results.

## 1. The instruments

- `gapmap.py` of P37 (`phases/p37/gapmap.py`), not changed: the emitted G-code replayed on nodes
  of 0.2 mm against `back_target_s1.stl`. Bar 0.3 mm.
- `p38/deep.py` (new): the same replay, and for each fed piece of one op the most that the piece
  lowers a node (`before - after`). That is the axial depth that the piece cuts on the replay
  stock. A piece is "deep" above 6.3 mm (the depth per pass 6.0 plus the replay bar).
- `p38/attr.py` (new): for each node that a deep piece lowers by more than 6.3 mm, and for each
  level of the op above the piece, the class of the node at that level. The classes come from a
  dump of the planner (each region of each level with its outline, kept or dropped; each left
  piece), with each outline grown by the tool radius. The dump is probe code only; it is not in
  a patch.
- The product measure: `summary.json` of the release CLI, `per_toolpath[].limit_rows`: the row
  "the depth against the machine force budget" of the groups body, entry, cleanup and link gives
  the deepest simulated sample of each group. `lineage.measured` gives the count of samples
  outside the envelope. `p38/lim.py` prints them.

## 2. W0: the operator file has the same gaps

`nz_wanaka_21_operator.toml` (read only; a scratch copy), setup "1 Back", HEAD release CLI.
The tool numbers of its G-code are the tool numbers of the proposal (T42, T1, T6, T2, T3).

After the last op ("Day strip slit V60"), model area 158 158 mm2:

- 8 328.7 mm2 of stock is more than 0.3 mm above the target.
- 1 944.1 mm2 of it is by design (the 6 mm tool cannot reach it).
- 6 384.6 mm2 is above the reach surface: no toolpath cuts it.

| Where (G-code frame) | Area mm2 | Tallest mm | Volume mm3 |
|---|---|---|---|
| Cavity wall band (5 mm or less inside the outline of `cavity_d9.00.dxf`) | 3 125.3 | 12.28 at (87.8, 424.8) | 19 967 |
| Cavity interior (rings at the lakes, the hollow and the rivers) | 1 053.4 | 9.00 at (48.2, 418.2) | 3 712 |
| Step shelf (Y 104 to 118) | 690.3 | 5.00 | 3 134 |
| Slot X 243 to 292, Y 53 to 64 | 412.6 | 21.00 | 8 637 |
| Mech and slit pockets (Y < 104) | 1 103.0 | 20.70 | 12 758 |

- The two roughs (ids 1 and 2) are the same toolpaths as in the proposal: the same 19 919 fed
  pieces of id 2, the same 188 deep pieces, the same deepest cut (20.70 mm).
- The corner light hole has 25.50 mm of stock under its footprint before the Drill (the three
  other holes of P36: 16.73, 17.13, 17.52).
- The run also reports 573 holder collisions: "Lake pocket 2 mm, 11 lakes to Z 5.88" 463 and
  "Lake pocket 2 mm, 1 lake to Z 3.99" 110. The proposal has 0 (its lake relief band).
- The cavity interior is larger than in the proposal (1 053 against 664 mm2 in P37): the
  operator file has no "Lake outline channel" and no "Lake relief band".

So each gap of P37 section 2 is in the operator file too, and each finding of this note about
ids 1 and 2 applies to it.

## 3. W1: the cause (measured)

### 3.1 Which moves cut deep

HEAD, the proposal as it is. Product measure of id 2: body 20.7 mm, entry 14.5 mm, link 6.0 mm,
cleanup 3.0 mm; 6 715 of 57 556 samples outside the envelope. Fed time of id 2: 603 s (40.0 m
fed, 4.0 m of rapids).

On the replay, 188 of the 13 635 cutting pieces of id 2 are deep, 644 mm of feed. No piece is
between 6.0 and 6.3 mm: a cut is one pass or it is deep. 107 pieces are deeper than 12 mm. The
deepest is 20.70 mm at Z 4.8, X 134.7 to 136.2, Y 34.9 to 36.7 (stock top 25.5).

The depths are the stock top less a level: 8.95 = 25.5 - 16.55, 16.95 = 25.5 - 8.55, 18.0 =
25.5 - 7.5, 14.5 = 25.5 - 11.0 (the floor that the 18 mm rough left), 20.7 = 25.5 - 4.8 (a
pocket floor under level 1.5). So each deep piece is a lower level that meets stock which no
level above it cut.

The deep pieces lower 4 021 nodes by more than 6.3 mm: 160.8 mm2 and 2 043 mm3 of stock, 126.0
mm2 of it in the mech and slit pockets and 34.8 mm2 in the cavity wall band. The whole swath of
the 188 pieces is 448.6 mm2.

### 3.2 Why the wood above them stood

The class of the tall nodes of each deep piece at the highest level above it:

| Class at the level above | Pieces | Fed mm |
|---|---|---|
| In the reach of a region that the level dropped under the small-region bound (S2 of P37) | 114 | 119 |
| In the reach of a region that the level planned, and not cut ("kept") | 41 | 362 |
| In the reach of no region of the level (S1 of P37) | 33 | 59 |

- The lead hypothesis (small regions) is true for 114 of the 188 pieces. It is 18 % of the deep
  feed.
- The largest share of the deep feed is stock inside the reach of a planned region. The plan of
  a region does not cut all of its reach: the F-038 group filter drops 265 of the 397 cut groups
  of this op (S5), and the region outline is inset and smoothed (0.25 to 0.6 mm). One piece of
  114.5 mm runs along the wall Y 91.8 at Z 13.5, 11.9 mm deep.
- 33 pieces meet stock that was in no region: the strip at a wall that P37 calls S1.
- No deep piece has a "no field" piece or a forecast piece as its first class.
- 13 of the 188 pieces are descents: an entry goes down through the tall stock (the entry group
  reads 14.5 mm).

The planner has a rule for a deep step (`StepSplit::StockDepth`, D43: deeper than one Depth/Pass
plus the stay-down clearance). It reads only the steps that the path shape flags, and its answer
is a split: a new entry at that point. The entry then goes down through the same stock. A flat
step is read by its chip-area load only (P22f-2, ruling 1), so a thin tall strip passes: 0.6 mm
x 17 mm is under the pass ceiling. The wall cleanup (`waterline_cleanup`) and the perimeter sweep
have no depth read.

### 3.3 The small-region bound (D-3)

Source: commit `c1121c655` (finding batch F-001 to F-013), `detect_and_order_regions`. The
comment gives the reason: a terrain model gives many small regions at the top levels, each with
its own entry and perimeter sweep, and the Wanaka rough had 81 % air cut. The bound is
`(2 x D)^2 x min(1, depth per pass / R)`, floor `(D / 2)^2`: 144 mm2 for the 6 mm tool at 6 mm
per pass, 1 296 mm2 for the 18 mm tool. It protects the cycle time of a terrain rough. It reads
the area only. It does not read the height of the stock or the levels below.

### 3.4 What the evidence selects

- Three drop sites and two no-region causes give deep cuts. A change to the small-region bound
  alone removes 18 % of the deep feed. It is the local patch.
- A height test at the drop does not help: at the first level the stock in a dropped region is
  one pass tall. One level later the cut is already two passes deep.
- So the rule must hold where a level MEETS the stock, for every cause: at the one door that
  each planned segment passes before its stamp.

## 4. W1: the rule and the predictions

### R-1: no fed move meets stock that stands more than one Depth/Pass above the cutter

- "More than one Depth/Pass" is the bound that the planner already has for a deep step: Depth/Pass
  plus the stay-down clearance (`stay_down_clearance_mm`, the dexel allowance; 0.5 mm on this
  job, so 6.5 mm).
- The read: each planner cell whose centre is inside the tool disc, its stock top less the cutter
  surface there (the tool Z plus the profile height). A centre that the planner takes has no
  model above the cutter inside the disc (the drop-cutter floor), so a tall column there is
  stock. The read does not go past the disc: the part beside a wall would read as tall stock.
- The door: `push_segment_with_stamp` and `LevelSink::emit`, after the boundary clip
  (G-BOUNDARYPHANTOM) and before the stamp. It is the pattern of the boundary clip.
  - A cut goes out as the pieces between its kept-off points (read at each vertex and each
    planner cell along a chord). Each piece that starts away from the tool gets its own entry.
  - A link whose chord meets such stock becomes an entry at its end.
  - An entry whose disc meets such stock is not made.
- The rule removes cut and link moves and adds none. A plan that meets no such stock passes
  unchanged. (Corrected after the measurement: a cut that the rule splits gets an entry for each
  kept piece, so the plan can hold more entries; section 5.3.)
- Each cell that kept a move off is recorded. `geom.rough_left_stock` states the count of moves,
  the area that still stands and the tallest stock. So the op says what it left; it does not
  refuse.
- The rule does not cut the stock that the levels above left. That is D-1 (W4) and D-3: a design
  for the lead, section 7.

### Predictions (written before the fix ran)

- P-1 (lib test, red first): on a rest stock with a rib 1.5 mm wide and 12 mm tall on a block 3
  mm tall (Depth/Pass 3, clearance 0.5), the levels 9, 6 and 3 drop the rib (36 mm2 under the
  152.4 mm2 bound) and the level 0.5 meets it. Without the rule the deepest simulated sample is
  about 11.5 mm. With the rule it is at most 3.5 mm and the record states the rib (tallest 11.5
  mm above the cutter).
- P-2 (the job, id 2, product measure): the body, entry, link and cleanup depth rows are each at
  most 6.5 mm. The simulation needs no more bound: its cell is the planner cell (0.2 mm) and a
  column top is exact in Z.
- P-3 (replay cross-check): no piece of id 2 is deeper than 6.8 mm (6.5 plus the replay bar).
  Pieces between 6.0 and 6.5 mm can exist (the 17.2 mm cusps of the 18 mm rough over level
  11.0).
- P-4: `project.sim_outside_envelope` of id 2 stays (the stated cut, 6.0 x 1.5 mm, is itself over
  the 2.75 mm limit), with fewer than 6 715 samples outside.
- P-5 (what the op leaves): the op now leaves the stock that it cut too deep. The gap map area
  above the reach surface rises by at most 448.6 mm2 in all (the swath of the deep pieces), in
  the mech and slit pockets and the cavity wall band. The slot stays 412.6 mm2 (no level cuts it
  today). With the "Cavity rest pocket 6 mm" op the cavity wall band stays 62.1 mm2.
- P-6: the area cut below the target after the two roughs does not rise (21.4 mm2 today).
- P-7 (terrain crumbs): the Geodesic Adaptive copy of `rivmap100_live_0925.toml` gives the same
  G-code, byte for byte (6 710 moves of the rough, `total_runtime_s` 1 227): its 34 dropped
  regions stand at most 1.87 mm, under one pass (8 mm).
- P-8 (time of id 2): the fed time stays between 590 and 900 s (603 s today): at most 644 mm
  less feed, and at most one new entry for each kept-off run.
- P-9: each op of the setup keeps 0 collisions and 0 rapid collisions; the contact counts stay
  the two known ones.
- P-10: `geom.rough_left_stock` of id 2 states the kept-off moves and an area above 0.
- P-11 (goldens, FM1): no op golden cell outside the 46 known rows and no FM1 cell moves. A cell
  that moves is a cut deeper than one pass in that fixture, and it is listed, not blessed.

## 5. W1: results

Measured on 2026-11-17 with the release CLI built from the W1 tree, on the proposal as it is.

### 5.1 The code

- `adaptive3d/one_pass.rs` (new): the rule `clip_to_one_pass`, the read `stock_over_cutter` and
  the record `KeptOff`. `PlannerCursor::pass_gap` says that the rule dropped the move before
  the next segment.
- The two segment doors call it after the boundary clip: `push_segment_with_stamp` (Contour
  Parallel, the wall cleanup, the C4 contour) and `LevelSink::emit` (Geodesic Adaptive).
- `left_stock` reads the record at the end of the plan: `LeftStock::kept_off_moves`,
  `kept_off_area_mm2`, `kept_off_tallest_mm`, `kept_off_at`. `LeftStock::stands` is the one
  rule of "this op must state it" (the adapter and the export gate read it).
  `geom.rough_left_stock` has one more sentence.
- `--lib _p38` (2 tests). The red state: with `OnePass::limit_mm` infinite the test fails with
  "a sample cuts 11.50 mm deep; one Depth/Pass and the clearance are 3.5 mm". The red run was
  made after the code, with the rule switched off.

### 5.2 Predicted against measured

| Prediction | Result |
|---|---|
| P-1 lib test | true: 11.50 mm without the rule, green with it; the record states the rib (11.5 mm). |
| P-2 depth rows of id 2 | body TRUE: 20.7 to 6.0 mm. Link 6.0, cleanup 3.0 (not changed). Entry FALSE: 14.5 mm, not changed. |
| P-3 replay | FALSE: 80 pieces of id 2 are still deeper than 6.3 mm (188 before). Two kinds, read in the G-code: (a) entry moves: the legs of a ramp and the laps at the entry Z, at the ramp feed F4571 (section 5.3); (b) body cuts that lower a strip one node wide at the edge of the disc, for example the 20.7 mm piece (0.9 mm of feed, 12 nodes) on a circle at Z 4.8. The replay nodes are probably half a cell off the planner cells (18.5 / 0.2 = 92.5; the planner grid origin was not read), so such a strip is inside the replay disc and outside the planner read. The product simulation is on the planner grid and reads no body sample above 6.0 mm. The split of the 80 pieces between (a) and (b) is not exact: 9 flat pieces have a deep width of 0.3 mm or less. |
| P-4 envelope finding | true: it stays; 6 347 of 56 818 samples outside (6 715 of 57 556 before). |
| P-5 what the op leaves | true: the area above the reach surface rises by 48.2 mm2 (2 834.6 to 2 882.8), all in the mech and slit pockets (1 103.0 to 1 151.1 mm2; volume 12 758 to 13 891 mm3). The slot stays 412.6, the cavity wall band 62.1, the step shelf 690.3, the interior 566.7. |
| P-6 below the target | FALSE: after the two roughs 28.2 mm2 (21.4 before), +6.8 mm2 on 199 nodes. 64 moves make them, each the descent of a NEW entry: for example a feed from (331.0, 117.8, 25.5) to (326.6, 117.8, 16.55). Its start is 2.5 mm from the wall X 333.5 with a 3 mm tool radius, so the descent takes up to 1.4 mm off the top edge of the wall, at most one node (0.2 mm) past the wall line. With the target lowered over one node: 2.2 to 4.9 mm2; over two nodes: 0.8 to 0.8 mm2. So the patch adds a cut of the top edge of a wall within 0.2 mm of the wall line, by the entry emitter (5.3). The export warnings show no new finding for it. |
| P-7 rivmap copy | true: the same G-code, byte for byte (`cmp`); 6 710 moves, `total_runtime_s` 1 227. |
| P-8 time of id 2 | true (generation of id 2 after its height map: 16.5 s on the final build, about 15 s in P37, not at the same machine load): fed time 603 to 661 s (+58 s); fed length 40.0 to 43.3 m; rapids 3 965 to 3 967 mm; 19 992 to 22 046 moves; ramp entries 134 to 183. Setup `total_runtime_s` 9 216 to 9 297. |
| P-9 collisions | true: 0 collisions, 0 rapid collisions, 0 failed checks on each op. The export warnings are the same 24 lines by id and op. |
| P-10 the finding | true: "It did not make 955 moves that would meet stock more than one Depth/Pass above the cutter ... 138.9 mm2 of that stock still stands; the tallest stands 20.70 mm above the cutter at X136.70 Y31.30 (stock top Z 25.500)." The drop counts moved with the plan: 130 small regions (125), 7 forecasts, 23 no-field pieces, 294 dropped cut groups (265). |
| P-11 goldens, FM1 | FALSE for the goldens (9 cells of one row move), true for FM1: section 10. |

### 5.3 The entries are not fixed (D-9)

- The rule reads an entry at its own column, because the planner models an entry as a vertical
  descent at the entry XY (`stamp_emitted_segment`, G-SIMCORE).
- The emitter does not ship that. A ramp entry of a 3D Rough is `dressup::emit_ramp(.., dir =
  (1.0, 0.0), ..)` with no fold and no stock in scope: two legs along +X from the entry. At the
  3 degrees of this op a descent of 6 mm is a leg of 114 mm. The leg leaves the column that the
  planner read, and it meets the strips at their full height: the entry group reads 14.5 mm, and
  `project.entry_load` says so today.
- The same emitter has the answer for a rest-driven finish pass (G-ISOCLIPENTRY,
  G-ISOCLIPRAMPFALL: with a rest stock in scope it is the lap ladder or a plunge, never the long
  legs). The 3D Rough door gives it no stock.
- The rule made 49 more entries (134 to 183 ramps), so it made more such descents. The +6.8 mm2
  below the target is their mark (P-6: the moves are named).
- Not fixed here: the entry is planned in `plan_entry` and shaped in the emitter, and the
  planner stock at the time of each entry is not in the emitter. The fix is its own package.

So after W1 the body, the links and the wall cleanup of a 3D Rough hold one pass, and the
entries do not. The acceptance "entries included" is NOT met.

## 6. W2 (D-2, D-8): the coverage reading at a stock to leave of 0; the setup that is not identity

### 6.1 Why the abstention was there

`measure_rough_coverage` abstained when `cell > stock to leave`. The recorded reason is a
quantisation argument (`standing_islands.rs`, and `planning/adaptive_research_2026-10-23/
P15_A_coverage.md` lines 117 to 119): "The bar is then more cell than spec". No measured failure
is behind it. Its effect: a rough with a stock to leave of 0 is never measured, and the text asks
for "a cell at or under 0.000 mm".

### 6.2 The rule

- The bar stays `stock to leave + one cell`. The resolution supports it: a column top is exact in
  Z; the cell is the XY resolution, so a column on a face at slope `t` reads its top within
  `cell x tan t`, one cell or less up to 45 degrees; the target is never below the true reach.
  So an island above the bar stands above the spec at every stock to leave. A coarse cell loses
  only the band under the bar (an island lower than one cell above the leave). The reading states
  the bar and the cell.
- The abstention `CellCoarserThanStockToLeave` is removed (the variant too: a wire break).
- `SetupNotIdentity`: the checkpoint stock and the generation inputs are both in the frame of the
  op's setup (`SimCheckpointMesh::mesh_stock`, `resolve_generation_inputs`; the rest path gives
  the same stock to generation with no transform). So by reading, nothing blocks the reading in a
  setup that is not identity. Section 6.5 gives the run and the decision.
- D-8: on `diagnose_toolpath_with_evidence` a measured reading of a rough (with or without
  islands), from a run that simulated the toolpath as it is now, drops `geom.rough_left_stock`
  of that toolpath. The island finding names the id in its `supersedes`.
- The CLI prints each reading (`Rough coverage:`) and writes `summary.json` `rough_coverage`,
  the list that MCP gives (`SimulationTriage::coverage`).
- The text of `geom.rough_left_stock` no longer says "Report-only: no gate consumes this": the
  sentence was a literal of the adapter, and `Gate::RoughLeftStock` writes the finding in the
  G-code header.

### 6.3 Predictions (written before the W2 runs were read)

- Q-1: the G-code of the proposal on the W2 build is the G-code of the W1 build, byte for byte
  (W2 moves no tool).
- Q-2: ids 1 and 2 of setup "1 Back" read `measured` with a bar of 0.200 mm (cell 0.2, stock to
  leave 0).
- Q-3: on the copy without the "Cavity rest pocket 6 mm" op, the islands of id 2 hold the strips
  at the four cavity walls (P37: 3 027 mm2 against the 6 mm reach surface; the reading uses the
  op's own floor, so the figure is of that order, not equal). Island boxes reach X 24.5, X
  333.5, Y 418.5.
- Q-4: the tallest island of id 2 stands more than 6.2 mm (one pass plus one cell) above the
  target, so `project.rough_leaves_islands` is Critical for id 2 (the rule of the finding). The
  export still writes the file (no gate reads the finding).
- Q-5: islands one cell wide at a wall exist (the XY quantisation of a wall). Their count is
  measured, not predicted; if they are most of the islands, a width rule is needed.
- Q-6: the reading of id 1 (18 mm tool, floor 11.0) is of the order of the estimate of P37
  section 5.3 (6 885 mm2 in 12 islands at a bar of one cell).

### 6.4 Results (release CLI of the W1 + W2 tree, the proposal and its copy with no pocket op)

| Prediction | Result |
|---|---|
| Q-1 same G-code | true: `cmp` of the W1 and the W2 G-code of the proposal gives nothing. The rivmap copy is still the HEAD file. |
| Q-2 measured | true: ids 1 and 2 read `measured`, cell 0.2, bar 0.200 mm. Before: `not_measurable`, `cell_coarser_than_stock_to_leave`. |
| Q-3 the wall strips | true: the largest island of id 2 is 8 254.5 mm2, 20.70 mm tall, box X 24.3 to 333.3, Y 30.7 to 418.3: it holds the strips at the four cavity walls, the mech pockets and the step shelf as one connected island. "No centre in reach = not compared" does not hide them: the centres are every cell inside the containment, not the cells of a plan region. The reading is the same with and without the pocket op (it reads the stock after id 2). |
| Q-4 the class | true: both roughs are in "Safety" (Critical): id 2 has islands to 25.00 mm, id 1 to 14.50 mm, above one pass plus one cell. The export writes the file; the header has the same 24 warning lines (no gate reads `project.rough_leaves_islands`). |
| Q-5 one-cell islands | id 2: 17 of its 39 islands are one cell wide, all at a wall, 0.9 mm2 in all. Id 1: 0 of 7. So the wall quantisation is 0.006 % of the area and 44 % of the island count of id 2. No width rule was added (D-11). |
| Q-6 id 1 | 14 069.4 mm2 in 7 islands, all at a wall, the tallest 14.50 mm. The P37 estimate was 6 885 mm2 in 12. The reading also holds two bands outside the part, X -18.5 to -0.1 (1 412.4 mm2 each, 14.50 mm), which the estimate did not compare. |

Id 2 in full: 15 088.0 mm2 in 39 islands, 37 at a wall, 2 free (the tallest free one 9.23 mm
high, 7.72 mm wide at (92.3, 181.1)). It lists the slot as its own island: 413.8 mm2, 21.00 mm,
8.60 mm wide, box X 224.5 to 272.9, Y 45.7 to 54.3. It lists four bands of 905.7 mm2, 25.00 mm
tall, outside the part (X -18.5 to -1.5 and X 359.5 to 376.5): the reading compares the stock
that a centre inside the containment can reach, and the stock there is not cut by a rough.

Not verified in W2:

- D-8 on a surface: the CLI prints no per-toolpath diagnostics list. The rule is one `retain` on
  `diagnose_toolpath_with_evidence` with the freshness test. No test holds it. The island finding
  carries `supersedes` (the sentry `..._g_roughcov` holds that), but the reducer
  (`apply_supersession`) runs before the triage findings join, so on this door the entry does
  nothing by itself.
- D-8 is partial. These surfaces still show both findings: the GUI inspector (it builds its own
  inputs, `ui/properties/operations/validate.rs`), `diagnose_toolpath_with_held_trace` (the MCP
  walk of viz), and the export header (`Gate::RoughLeftStock`; `export_warnings` takes no
  evidence, and `project.rough_leaves_islands` has no gate). The pattern that closes it is the
  flute-reach one: an input on `ToolpathDiagnoseInputs` and a measured-clean row.
- The setup that is not identity: section 6.5.

### 6.5 `setup_not_identity`: it can be lifted, and it is NOT lifted in this patch

- What blocks it: one branch (`rough_coverage_at`: `checkpoint.mesh_frame.is_some()`), a scope
  decision of G-ROUGHCOV ("this pass reads the identity frame only"). No transform is missing:
  `mesh_stock` is the setup-local stock (`compute/simulate.rs`), `resolve_generation_inputs`
  gives the mesh, the heights and the boundary in the same frame, and a rest op of such a setup
  reads the same stock with no transform.
- One run with the branch removed (the W2 build before this decision; setup "3 Front" alone,
  416 s): id 20 "Front bulk rough 18 mm" reads measured, bar 0.700 (stock to leave 0.5), 29 882.7
  mm2 in 689 islands, the tallest 12.00 mm; id 21 "Front 3D rough" 27 541.5 mm2 in 1 535 islands.
  The largest island of each has the box X 41.2 to 353.8, Y 31.2 to 343.8: the footprint of the
  relief in the flipped setup. 530 of the 689 islands of id 20 and 1 110 of the 1 535 of id 21 are
  one cell wide (46.8 and 100.6 mm2): a terrain at a cell of 0.2 mm.
- That run does not prove the frame match. An island lies inside the target footprint in any
  frame, and 28 % of the compared area above the bar after a bulk rough is not explained.
- So the abstention is back in the code (the variant `SetupNotIdentity` stays). To lift it: a
  sentry with a model that is not symmetric in a flipped setup (a step on one side), a stock cut
  to its target, and a reading of zero; then the branch goes. A separate, small job (D-10).

## 7. W3: "Contour each level" on id 2 (diagnosis only; arm A of P37)

Measured on the G-code of arm A of P37 (`p37/out_armA.nc`) with `p38/w3.py` (the moves that
lower a node more than 0.3 mm below the target) and the gap map. Nothing is built.

### 7.1 The cut below the target is an instrument limit, not a gouge

- Arm A has 165.5 mm2 below the target after id 2; the proposal has 21.4 mm2.
- 390 moves of arm A make the area. Each one has its tool centre one tool radius from a wall
  line, and the node that it lowers is ON the wall line: tool Y 112.5 and node Y 109.5 (the
  cavity wall, target 20.5); tool Y 35.5 and node Y 32.5 (target 25.5); tool Y 415.5 and node
  Y 418.5; tool X 327.3 and node X 329.9 to 330.3. The target raster gives the wall top to a
  node on the line (the limit that P37 section 5.2 names for arms B and C).
- With the target lowered to the lowest node within one node (0.2 mm): the proposal has 2.2
  mm2 and arm A 4.3 mm2. Within two nodes: 0.8 and 1.0 mm2.
- So 161 of the 165.5 mm2 are nodes one node or less from a wall line. The 2.1 mm2 that arm A
  adds past one node is at the lake rims (the deepest 3.95 mm, on nodes two nodes from the
  line). The replay cannot tell a gouge of 0.4 mm at a vertical wall from its own raster. The
  product has a reading for this (`safety.cuts_into_model`); it was not run on arm A.

### 7.2 The band: the contour clears two walls of four

Cavity wall band above the reach surface after id 2 (before any later op):

| Wall | Proposal mm2 | Arm A mm2 | Stock top that stays in arm A |
|---|---|---|---|
| X 24.5 | 798.6 | 753.9 | 25.5 (never cut) |
| X 333.5 | 1 022.7 | 993.2 | 25.5 from X 331.9, the 17.2 cusps from X 330.3 |
| Y 109.5 | 784.4 | 67.8 | 13.5 |
| Y 418.5 | 784.4 | 25.2 | 16.5 |

- The level contours cut the walls Y 109.5 and Y 418.5 to size at levels 19.5 and 16.55 (the
  cross sections read 16.55 up to the wall line).
- No contour ran on the walls X 24.5 and X 333.5: the stock top there is the uncut 25.5.
- The cause of the difference is not found. The candidates, each in `waterline_cleanup` on the
  By Area path (C4b):
  1. `keeps_to_region`: a contour point is cut only when no cell in reach holds material of
     ANOTHER By Area job above the level. The wall X 333.5 has the job "region 2" (X 314.8 to
     330.4, the cusps of the 18 mm rough, 17.2 above level 16.55) along its full length. The
     wall X 24.5 has six small jobs (regions 4, 21, 24, 26, 30, 31).
  2. The contour is one closed ring of the model section, and the slope filter and the
     left-piece skip accept or reject a whole ring; they cannot give two walls of four.
- Next probe (cheap, one run): count the points that `keep` rejects per contour and per job at
  levels 19.5 and 16.55, with the job that owns the blocking cell.
- The cost (+3 327 s) is in the same place: the wall cleanup runs for each of the 38 jobs at
  each of its levels, on each ring of the model section at that Z, and the jobs are small.

### 7.3 Why the wall contour of the 18 mm rough runs only at 15.5 and 11.0

- Global ordering runs `waterline_cleanup` after each level, at the level Z. The levels of id 1
  are 15.5 and 11.0 (10 mm per pass from the stock top 25.5).
- The contour is the section of the tool-centre surface AT the level Z
  (`waterline_contours_with_cancel(.., z_level, ..)`). The cavity floor is at 16.5 and its wall
  runs from 16.5 to 25.5. Both are above 15.5, so the plane Z 15.5 does not cross the wall.
- The level itself drapes: it cuts the cavity floor at 16.5. The contour does not drape: it
  stays at the plane.
- The smallest correct fix: where a level drapes on a flat shelf above its plane, the wall
  contour of that level also runs on the section just above the shelf (the shelf Z plus the
  chord tolerance), inside the stock that stands there. `flat_shelf_levels` already finds the
  shelves. The one-pass rule (section 4) then holds each such contour to one pass: at the
  cavity wall the strip stands 9 mm above the shelf, more than one pass of 6.5 mm is not cut,
  so the contour needs the levels of the strip too. That is the design of section 8.

## 8. W4 (D-1): the rest rough reads the stock under the tool disc (design only)

### 8.1 The design

1. One read. `one_pass::stock_over_cutter(stock, tool, centre at its level floor)` gives the
   most that a column under the disc stands above the cutter. The material test of the level
   grid (`cell_has_material`) becomes: more than the material margin (0.01 mm). The one-pass
   rule is the same read with the bar at one Depth/Pass plus the clearance. So "material" and
   "too tall" cannot disagree.
2. The level anchor of a rest rough reads the same tops (`PriorStockTops`, which C4 already
   reads in the reach of the centres), so the first level is one pass under the tallest stock
   that the tool can meet.
3. A strip at a wall then makes a region: a band as wide as the strip, one tool radius from the
   wall. The geodesic field does not fit in it (research C38 section 16: edge work). So the
   design needs a planner for a narrow region: one contour along the band at each level, the
   C4 pass (`level_contour_pass`: Z from the tops in reach, the standing runs, the
   feed-or-retract merge, the load cap) run on the wall-side line of the region and not only
   on the containment ring.
4. A drop site (the small-region bound, the forecast, the group filter) keeps its rule for a
   crumb only: a region where no lower level comes (each cell has its level floor on the model).
   A region with a lower level under it is planned (D-3).
5. The one-pass rule stays as the guard: what the plan of points 3 and 4 does not cut is kept
   off and stated.

### 8.2 The blast radius

- Every 3D Rough on remaining stock: its regions grow by up to one tool radius around each
  standing edge, so its motion changes. Sentries that hold such motion: the D44, D48, D50, D55,
  C4 and C4b sentries of `adaptive3d/CLAUDE.md`, `adaptive3d_by_area_*`,
  `a_rest_op_slower_than_fresh_is_told_r3`.
- A 3D Rough on fresh stock changes only where a level left a sliver that a lower level can
  reach (the region grows around it). The op golden rows of 3D Rough and the Adaptive3d cells
  of the FM1 simulation subset can move.
- No dry measurement was made: the count of cells that change class needs planner code and one
  release build (16 minutes on this machine). The one-pass record of section 5 is a measured
  lower bound of what the design must cut on this job.

### 8.3 Against the acceptance of the lead

- Wall band at most 62 mm2 with no pocket op: the band is 3 027 mm2 at Z 25.5, 9 mm above the
  floor: two passes of 6 mm. The contour of point 3 is 1 236 mm per pass (the outline of
  `cavity_d9.00.dxf`, P37 arm B): 2 472 mm. At the feed of the op (5 279 mm/min, less the load
  cap at a half slot) that is 30 to 100 s, and the entries. Arm B of P37 (a Profile, 3 passes)
  measured +73 s for the same cut. So the 230 s bound holds if the contour runs once per level
  per wall, not once per job (the cost of arm A).
- The corners: one contour leaves the corner patch (P37: about 1.5 mm across), as arm B does.
  The Pocket (arm C2) clears it. So the design gives arm B, not arm C2: the corner light hole
  still starts in stock unless the narrow-region planner also cuts a second line. The Pocket
  stays the answer for the corners.
- 0 cut below the target: the contour runs on the tool-centre surface, as the wall cleanup does.

## 9. The Wanaka job before and after, for the operator (setup "1 Back", the proposal file)

"Before" is HEAD `c99a5ac37`. "After" is the W1 + W2 tree: a build that is NOT cleared to cut a
part (section 10: W1 is not ready to land, and its ramp entries are not safe). The G-code of ids 0 and 1 is the same
file; id 2 changes, and each later op that reads the remaining stock moves with it.

| | Before | After |
|---|---|---|
| Deepest cut of "Back rest 6 mm" in its clearing passes (stated 6 mm per pass) | 20.7 mm | 6.0 mm |
| Deepest cut of "Back rest 6 mm" in its ramp entries | 14.5 mm | 14.5 mm (NOT fixed) |
| Deepest cut in its links, and in its wall cleanup | 6.0 mm, 3.0 mm | 6.0 mm, 3.0 mm |
| Samples of "Back rest 6 mm" outside the safe envelope | 6 715 of 57 556 | 6 347 of 56 818 |
| Time of "Back rest 6 mm" (feed at the stated feeds) | 603 s, 134 ramp entries | 661 s, 183 ramp entries |
| Time of the setup (`total_runtime_s`) | 9 216 s | 9 297 s |
| Wood left, cavity walls (with the rest pocket op) | 62.1 mm2 | 62.1 mm2 |
| Wood left, cavity interior (rings at lakes, hollow, rivers) | 566.7 mm2, to 6.4 mm | 566.7 mm2 |
| Wood left, step shelf | 690.3 mm2, 5.0 mm | 690.3 mm2 |
| Wood left, the 8.6 mm slot | 412.6 mm2, 21.0 mm | 412.6 mm2, 21.0 mm |
| Wood left, mech and slit pockets | 1 103.0 mm2, to 20.7 mm | 1 151.1 mm2, to 20.7 mm |
| Cut below the target after the two roughs (replay, 0.2 mm nodes) | 21.4 mm2 | 28.2 mm2 (the top edge of a wall, within 0.2 mm of the wall line, by 64 new entry descents) |
| Collisions, rapid collisions, failed checks (each op) | 0, 0, 0 | 0, 0, 0 |
| Contact counts | "Pin drill, flip pins" flute top 5; "Day strip slit V60" flute top 7 | the same |
| What the job says about the wood it leaves | two `geom.rough_left_stock` lines; the coverage reading says "not measurable" | the same two lines (id 2 now also names 955 moves that it did not make and 138.9 mm2); the coverage reading runs: id 1 leaves 14 069 mm2 in 7 islands, id 2 15 088 mm2 in 39 islands, both Critical |

In plain words:

- With the W1 build the 6 mm rest rough does not drive its clearing passes through wood 20 mm
  tall: where a pass would meet such wood, the tool stays off it and the op says how much it
  left. That build is not ready (section 10).
- The ramp entries of that op are not safe yet: a ramp runs along +X from its entry and still
  meets the tall strips, 14.5 mm deep. The op has 49 more such entries than before.
- The 6 mm tool does not cut the 8.6 mm slot (a strip 2.6 mm wide for its centre, under the
  small-area bound at each level), the tall strips at the mech pocket walls, or the strips at
  the cavity walls. A 6 mm tool can reach all of them; the rough does not plan them. The cavity
  walls need the rest pocket op. The slot and the mech pocket walls need an op of their own
  (D-5 of P37).
- A 6 mm tool cannot reach 2 259 mm2 (inside corners and floors narrower than 6 mm).

## 10. Gates (the W1 + W2 tree, 2026-11-17): W1 is NOT ready to land

Run on the final tree (after the last crate edit), unless a line says "first pass".

- full clippy (`--workspace --all-targets`, the three core features, `-D warnings`): clean.
  `rustfmt --check` on each changed file: clean; no other file changed.
- core `--lib`: 2 844 passed, 2 failed, 13 ignored.
  - known: `feeds::envelope::tests::the_envelope_meets_its_time_budget`;
  - NEW, from W1: `adaptive3d::tests::planner_sim_dexel_parity_geodesic_adaptive`: "INTERIOR
    divergence is 70.00x lopsided: interior planner_higher 70 (simulator removed more), interior
    sim_higher 0".
- adaptive3d sentries (`--no-fail-fast`, 29 binaries with `heavy-tests`): 24 green, 5 red, 59
  tests passed, 7 failed, 2 ignored. All 7 are from W1:
  - `adaptive3d_boundary_clear_parity`: `boundary_clear_parity_geodesic_adaptive` (69
    in-containment cells `planner_higher`; the pin is 60);
  - `adaptive3d_by_area_matches_global_stock`: `by_area_final_stock_equals_global`
    (Contour Parallel: 9 cells differ, the worst by 1.5 mm);
  - `adaptive3d_emission_byte_parity`: `contour_parallel_by_area`, `geodesic_adaptive`,
    `geodesic_adaptive_boundary` ("the emitted motion moved");
  - `adaptive3d_plans_every_slice_piece_g_adapt3dpieces`: `geodesic_adaptive_clears_both_lobes`:
    "a rapid passes through stock (the move between the lobes)", 1 rapid collision. SAFETY;
  - `geodesic_adaptive_keeps_the_research_geometry_s7_4`: "island pocket, two levels".
- other core sentries (14 binaries, 86 tests): green (`..._g_roughcov` with the new arm,
  `findings_transport_join_h21`, `a_job_with_each_finding_exports_and_names_it_k6`,
  `measurability_abstention_r8`, `chipload_abstention_cannot_supersede_g_chipgate`, the others
  of the P37 list).
- op golden: red with 55 changed cells: the 46 known rows cell for cell, and 9 cells of ONE row
  from W1. Not blessed:

  | `adaptive3d/ramp_geodesic` | before | after |
  |---|---|---|
  | `arc_count` | 13 | 12 |
  | `cutting_length_mm` | 2283.957 | 2660.394 |
  | `linear_count` | 1091 | 1147 |
  | `move_count` | 1113 | 1168 |
  | `rapid_length_mm` | 114.908 | 112.388 |
  | `span_count` | 92 | 165 |
  | `gcode_hash` | `2ae632031de60cab` | `2a01bcfb800da443` |
  | `moves_hash` | `be39e8dff5872a09` | `3a88a13999fb1f88` |
  | `spans_hash` | `3398534541b08672` | `88876c69ae0aca1e` |

- FM1 (`feeds_matrix_instrument_fm1`, `heavy-tests`, first pass, before the last W2 edit, which
  moves no tool): 2 passed. No cell moves.
- `rs_cam_mcp`: 39 passed. `rs_cam_cli`: 17 binaries, 113 tests, green.
- viz `--lib` (`--test-threads 2`, no GUI process): 705 passed, 2 ignored.
  `mcp_wire_surface_pin`: 2 passed.

What the reds say:

- The rule fires on the test fixtures, not only on a rest rough. The record of the hemisphere
  fixtures: 22 to 37 moves kept off, stock to 15 mm above the cutter, at the edge of the mesh
  footprint (X or Y at 20.0 and past it): the band of stock beside the model that no centre
  takes. At HEAD a lower level probably met it at full height (the fixtures were not simulated at HEAD).
  So the emitted motion of many fixtures
  moves, and the byte sentries and the golden row need a ruling, not a bless by an agent.
- The parity reds (`planner_higher`: the shipped path removes more than the planner stamps)
  grow with the entry count. The entry model gap of section 5.3 is the probable cause; it is not
  measured.
- One fixture has a rapid through stock. It is not explained. The Wanaka runs have 0 rapid
  collisions, but this is a safety red.
- A possible false positive of the read was not ruled out: a cell whose centre is exactly one
  tool radius from the tool axis, on a wall line, counts (`d_sq > r_sq` is false there).

Not verified:

- The W1 tree alone was gated only by `--lib _p38 _p37` and the release build; every other gate
  ran on the W1 + W2 tree.
- D-8 on a surface (section 6.4). The finding texts on the GUI and through MCP.
- The release CLI of the job runs (`cli_w2`) was built before the last edits: the abstention
  for a setup that is not identity, two comments and one test. None moves a tool of setup
  "1 Back".

## 11. Decisions for the lead

- D-0 (the patch order): W2 is the part that can land, and `P38_W2.patch` applies only after
  `P38_W1.patch` (they share two source files and this note). A W2-only tree was not built or
  compiled, and the island figures of section 6.4 were measured on the stock of the W1 build.

- D-9 (entries, SAFETY, open): the acceptance "no sample deeper than one pass, entries
  included" is not met. The ramp entries of id 2 still cut 14.5 mm deep, and the rule adds 49
  entries. The cause is the entry emitter of the 3D Rough (two legs along +X, no stock in scope;
  the planner models a vertical descent). A ruling is needed on the fix: give the emitter the
  planner stock at the entry (the lap ladder or a plunge, as a rest-driven finish pass has), or
  let the planner shape the entry and read it with the one-pass read.
- D-12 (W1 is not ready to land): one lib test and 7 sentry tests are red from W1, and 9 cells
  of the golden row `adaptive3d/ramp_geodesic` move (section 10). One red is a rapid through
  stock. The rule fires on the fixtures at the band of stock beside the mesh footprint. The lead
  must rule: (a) keep the rule at the door and fix the entries first (D-9), then read each red;
  or (b) move the rule into the region grid (a centre whose disc meets tall stock is not a
  region cell), so the 2D plan avoids the stock and makes no extra entry; or (c) hold W1 until
  W4 gives the plan that cuts the stock.
- D-3 (the small-region bound): not changed. With the one-pass rule a dropped region is no
  longer cut too deep by a level below; it stays, and the level below stays off it. The slot
  (413.8 mm2, 21 mm) is the case. The rule that cuts it is point 4 of section 8.1 (a drop is for
  a crumb only: no level comes under it). It moves terrain goldens; it is part of W4.
- D-1 (W4): section 8. The design gives the cavity walls (arm B of P37), not the corners. The
  Pocket stays the answer for the corners.
- D-2: done for the stock to leave (the reading runs at 0). D-10: the setup that is not identity
  is not lifted (section 6.5); a small job with a sentry.
- D-8: partial (section 6.4). A ruling is needed: build the measured-clean row and the diagnose
  input (the flute-reach pattern), or accept the one door.
- D-11 (the reading at a wall): 17 of the 39 islands of id 2 are one cell wide at a wall (0.9
  mm2). No width rule was added. A rule "an island at a wall that is one cell wide is the wall
  quantisation" would take them out of the count; `min_width_mm` reads an even strip one cell
  narrow, so the rule would also hide a strip of two cells.
- D-13 (the class): at a stock to leave of 0 both roughs of the Wanaka back are Critical in the
  simulation triage (`project.rough_leaves_islands`: islands taller than one pass). That is the
  rule of the finding; before P38 it could not fire on this job. The export does not refuse, and
  the CLI verdict line and exit code are the same before and after.
- D-7 (W3): section 7. The cut below the target of arm A is the raster of the instrument. The
  contour clears two walls of four; the cause needs one probe run.
- The operator file: section 2. It has the same gaps, the same deep cuts in ids 1 and 2, and
  573 holder collisions in its two lake pocket ops.
