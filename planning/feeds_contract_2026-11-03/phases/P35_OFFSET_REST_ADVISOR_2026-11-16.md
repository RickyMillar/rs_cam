# P35: a radial offset on Profile, Project Curve passes on the remaining stock, the advisor reads the switch (2026-11-16)

Three parts. The operator ruled on each (2026-11-15 and 2026-11-16). Base commit `17b059da4`.

This file holds the rules and the predictions first (sections 1 to 3, written before the code),
then the results (section 4 on).

## 1. Part A: a radial stock to leave on Profile

Need (`planning/rivmap_block_job/LEAD_PROPOSAL_2026-11-15.md`): a relief band that reaches 1.6 mm
outside a lake outline with the 6 mm tool. The tool centre is then 1.4 mm inside the outline.

### 1.1 The name and the sign, and their sources

- Found in the code: no 2.5D op has a radial leave. The 3D ops have `stock_to_leave_axial`
  (positive leaves material). `rough_3d.rs:32` says that L2 deleted an inert `stock_to_leave_radial`
  dial of the 3D Rough. Bore has `finish_allowance` (radial, positive leaves material).
- Found in `research/`: `fusion360_ui_reference.docx` prints "Stock to Leave (Radial): Wall material
  remaining for finishing" and "Stock to Leave (Axial)". `fusion360_cam_reference.docx` prints
  "Stock to Leave (Radial): Material left on walls for finishing".
- The name is `stock_to_leave_radial`: the word of the sibling `stock_to_leave_axial` and of the
  Fusion row.
- The sign: a positive value leaves material on the wall (the two documents above). A negative
  value cuts past the line. No document in `research/` prints the negative case; its source is the
  brief of this step ("Fusion radial stock to leave: a negative value cuts past the line").
- The range is -10.0 to 10.0 mm: the magnitude of the sibling `stock_to_leave_axial` (0 to 10), in
  the two directions.

### 1.2 The rules

1. New key `stock_to_leave_radial` on Profile, optional. No key (or 0): the path of today, byte
   for byte. No file gains the key on a save.
2. Where the tool centre goes, for a value `s` and a tool radius `r`:

   | Side | Tool centre | A positive `s` moves the centre |
   |---|---|---|
   | `outside` | the loop moved out by `r + s` | away from the part (out) |
   | `inside` | the loop moved in by `r + s` | away from the wall (in) |
   | `on` | each closed ring moved out by `s` | out of the area that the ring closes |

   Side `on` has no wall, so "leave" gives no direction. The rule built reads the area that the ring
   closes as the part, as `outside` does (the default side). This is a decision for the lead.
   The lake band: side `on` with `s = -1.4`, or side `inside` with `s = -1.6`.
3. An open path (side `on` only) with a value that is not 0 refuses the op. An open path closes no
   area, so the value has no direction. Trace cuts beside an open path (`left`, `right`).
4. The offset of one loop must give one ring. A loop that collapses or that splits refuses the op,
   and the text names the start point of the loop and the count of rings. No ring is dropped
   without a report, and no path is made from a part of a loop. (The op with no key keeps its
   behaviour: it cuts the first ring of the offset and reports nothing.)
5. Tabs: the tab placer measures on the cut ring (`profile_ring`), so the tabs sit on the moved
   ring. A hand tab snaps to the drawn loop, as today.
6. Lead-in and lead-out: no change. A lead goes on a closed pass of `outside` or `inside` and
   curls away from the part. Side `on` takes no lead.
7. Compensation `in_computer`: rule 2. Compensation `in_control`: the programmed line is the loop
   moved by `s` alone to the side of the tool, and the controller adds its radius. The sum is
   defined, so the op does not refuse. Side `on` with `in_control` refuses, as before.
8. The value is a parameter of `ProfileParams` (`ops/profile.rs`), so the generation, the tab plan
   and the `tab_layout` query read one ring.

### 1.3 Predictions

| # | Prediction |
|---|---|
| A1 | No key: the op golden does not move (the 46 known rows, the same rows). |
| A2 | Red first. A 40 mm square, side `on`, 6 mm tool, `s = -1.4`: before the code the door refuses the key. After it, each fed point is 1.4 mm inside the square, and the cut reaches 1.6 mm outside and 4.4 mm inside the line. |
| A3 | Side `outside`, `s = 0.5`: the centre is 3.5 mm outside. Side `inside`, `s = -1.6`: the centre is 1.4 mm inside. |
| A4 | A ring drawn clockwise and a Bottom setup give the same side as A2. |
| A5 | A 2 mm square with side `on` and `s = -1.4` refuses (the offset collapses). An open path with `s = -1.4` refuses. |
| A6 | `in_control`, side `outside`, `s = 0.5`: the programmed ring is 0.5 mm outside the loop. |
| A7 | `set_param_refuses_absent_field_n5` moves by the one new name. The MCP wire pin does not move (it holds no op parameter, P33 P-A6). FM1 does not move. |

## 2. Part B: Project Curve passes on the remaining stock

Operator: "Can we not do something clever with rest? stock top but cull the passes that are air
cuts?".

### 2.1 What the code does today (read before the change, not run)

- The one door is `stock_source = "from_remaining_stock"`. The generation plan puts a `Simulate`
  step before the op, and the op gets the simulated stock of the ops before it: the planner reads
  `Territory::unguarded_stock`, the dressup pipeline reads `DressupContext::prior_stock`.
- The dressup pipeline of EVERY op from remaining stock runs the air-cut filter
  (`dressup::filter_air_cuts`, called in `compute/execute/dressup_apply.rs`). It drops a fed move
  whose whole cutter sweep is in air over the prior stock (the whole cutter, not the centre point),
  with a tolerance of 0.1 mm (the value at that call). A dropped run becomes a retract bridge only
  when the bridge is faster than the feed through the air (operator ruling 2026-10-15).
- So the cull that the operator asks for is in the code. The river op of the job has
  `stock_source = "fresh"`, so the filter does not run on it.
- The ladder top is `heights.top_z`. A Project Curve has the rest rule `RestUse::LinkSafetyOnly`
  ("the cut paths do not read it").

### 2.2 The rules

1. No second cull. The air-cut filter is the one rule that drops a sample in air. It reads the
   stock BEFORE the op, so a sample under which the op itself cut in an earlier pass is not air.
2. New: with remaining stock the ladder top is the stock top under the curve: the highest material
   top in the footprint of the tool (`standing_material::column_top`: each cell centre within
   `R + cell` of the sample) over every fed sample of the full-depth path. `heights.top_z` is not
   read by the ladder then. Where no sample has a stock answer, the ladder reads `heights.top_z`.
3. So no sample cuts more than one step: the first pass is one step below the highest stock top,
   each later pass is one step lower, and the filter drops only samples that cut nothing.
4. The rest rule of a Project Curve becomes a new named rule, `RestUse::LadderTop`: the remaining
   stock sets the top of the pass ladder and the link ceiling. With one pass (no `depth_per_pass`)
   the planned cut is the Fresh cut.
5. `stock_source = "fresh"`: nothing changes, byte for byte.
6. A departure from the brief: the brief says "the deepest pass is never culled". The filter is one
   rule for every op and every pass, and it drops a sample of the deepest pass when the whole
   cutter is in air there (it does so today for a one-pass Project Curve from remaining stock). A
   second rule for the last pass is not built. Such a sample cuts no material above the tolerance.

### 2.3 Predictions

| # | Prediction |
|---|---|
| B1 | `fresh`: the op golden does not move. |
| B2 | Fixture: a straight curve, full depth at Z 8, under a stock with a floor at Z 12 over most of the length and the full height Z 20 over a short part, `depth_per_pass = 2.0`. With remaining stock and the automatic top (20), BEFORE the code: the filter drops the passes above the floor over the floor part, and no simulated sample cuts more than one step (plus the cell). Predicted green before the code. |
| B3 | Red first. The same fixture with a stated top Z of 12 (the operator case "top 16.5"): before the code the ladder has 2 passes and the samples at the full height part cut up to 10 mm. After the code the ladder starts at 20, and no sample cuts more than one step. |
| B4 | A curve wholly under the floor, automatic top 20: before the code 6 levels (step 2.0), after the code 2 levels from 12 (step 2.0). The fed samples are the same count. |
| B5 | `every_op_conforms_w4` check 2 holds for the new rule (the fixture has one pass). The capability table test moves by the one cell. |
| B6 | The wanaka river op with remaining stock, against the proposal as committed (12 passes from Z 25.5: 121519 moves, 39094 mm cut, 41154 mm rapid): the cut length falls to between 22000 and 27000 mm (7 of 12 passes are 22800 mm; the parts next to the clamp areas add to it), and the deepest simulated sample is at most one step (1.94 mm) plus the cell and the Z change of the surface over one sample step. |

## 3. Part C: the strategy advisor reads the project switch

Operator ruling 2026-11-16: "Read the switch" (`P32_CAP_ONLY_2026-11-15.md` section 7 item 7).

### 3.1 The rules

1. The advisor job captures the project switch (`FeedModulationSwitch`) with its other session
   reads (`AdvisorContext`).
2. `optimized_candidate` reads `FeedModulationSwitch::mode()`, the one switch-to-mode rule:
   `cap_only` and `full` modulate the candidate in that mode; `off` modulates nothing, and the
   candidate is timed with the feeds of its planned path.
3. The candidate path of the advisor has no dressup (the code says "the raw path is what we
   time"). So with the switch off its feeds are the planned feeds.

### 3.2 Predictions

| # | Prediction |
|---|---|
| C1 | Red first. On the terrain fixture of `advisor_modulates_candidate_feeds_before_timing`, before the code the candidate time is equal for `cap_only` and `full`. After the code the `full` time is shorter than the `cap_only` time, and the `cap_only` path has no feed above the planned feed. |
| C2 | Switch off: the candidate path is the planned path, move for move. |
| C3 | The advisor time against a `run_simulation` of that candidate in the same mode: NOT equal in general. The advisor simulates the candidate alone, on fresh stock, at the automatic resolution, with no dressup; `run_simulation` reads the dressed result at the stored resolution. The test asserts equality on a session that removes those differences where it can, and section 4 states the gap that stays. |
| C4 | `planning/fixtures/rivmap100/rivmap100.toml`: the choice in `cap_only` against `full` is measured and reported; no prediction of the winner. |

## 4. Part A: what is built, and the results

- `ProfileConfig::stock_to_leave_radial: Option<f64>` (range -10.0 to 10.0 mm; not written when
  empty). GUI row "Radial stock to leave" in the Advanced section of the Profile editor; empty
  shows "on the line". The row is NOT seen on screen (no GUI run in this step).
- `ops/profile.rs`: `ProfileParams::stock_to_leave_radial`; `profile_path_reported` and
  `on_line_contours` move the ring (rule 2) and return `RadialLeaveRefusal` (rules 3 and 4).
  `profile_path_reported` now returns a `Result`.
- `dressup/tabs.rs`: `plan_profile_tabs` returns a `Result`. `ProfilePass::prepare` and
  `ProjectSession::tab_layout` give the refusal as `OperationError::InvalidParam`.
- A ring of side `On` comes in the drawn winding. The offset runs on a counter-clockwise copy, and
  the moved ring takes the drawn winding back, so the cut direction does not change.

| # | Result |
|---|---|
| A1 | See section 7 (the op golden runs once on the full tree). |
| A2 | Held. `side_on_with_minus_1_4_cuts_1_6_outside_and_4_4_inside_p35`: each fed point is 1.4 mm inside (within 0.011 mm, the arc tolerance of the offset); on the simulated stock (cell 0.5 mm) the cell one cell inside each band edge is cut and the cell one cell outside it is whole. The red state was shown with the offset turned off (`moves_the_ring` false): 6 of 7 tests failed, this one with "0.0000 mm from the line; expected -1.4". The prediction "the door refuses the key" was not run: the test was written after the field. |
| A3 | Held. `outside_and_inside_move_by_the_radius_plus_the_value_p35`. |
| A4 | Held. `a_clockwise_ring_and_a_bottom_setup_keep_the_side_p35` (the Bottom setup arm compares the ring area on the line and moved, on the standard fixture, in the two frames). |
| A5 | Held. `a_collapsed_loop_and_an_open_path_refuse_p35`; a loop with a 2 mm neck refuses with "2 rings". |
| A6 | Held. `in_control_programs_the_loop_moved_by_the_value_p35`. |
| A7 | WRONG for `set_param_refuses_absent_field_n5`: it did not move (7 passed, no pin changed). The MCP wire pin and FM1: section 7. |

Not predicted: the tabs. `the_tabs_follow_the_ring_and_the_door_takes_the_key_p35` reads
`tab_layout`: each of the 4 automatic tabs is 1.4 mm inside the line. The same test holds the door:
`SetToolpathParam` takes the key, refuses 10.5 (outside the range) and clears it with `null`.

Decisions for the lead (part A):

1. The direction of side `on` (section 1.2 rule 2).
2. A loop that splits refuses the op. The other choice is to cut each ring of the split.
3. The tab side rule of a NESTED ring of side `on` reads the moved ring as its outline (the drawn
   ring before). The exterior ring reads the drawn outline.

## 5. Part B: what is built, and the results

- `compute/execute/curve.rs`: `ladder_top_z` and `stock_top_under` (rule 2). `run_curve` gives the
  result to `CurveOp::passes` in place of `heights.top_z`.
- `RestUse::LadderTop` (rule 4), on the Project Curve row. `Territory::unguarded_stock` names the
  new reader in its closed list.
- No cull is built. The air-cut filter of the dressup pipeline is the cull (rule 1).

The fixture (`project_curve_passes_read_the_remaining_stock_p35`, simulation cell 0.5 mm, step
2.0 mm), measured with the simulation:

| Arm | Levels (Z) | Deepest sample | Fed moves above the floor over the floor part | Air share of the fed time |
|---|---|---|---|---|
| Fresh, top 20 | 18 to 8 (6) | 1.75 mm | 321 | 0.558 |
| Fresh, top 12 | 10, 8 | 8.75 mm | 0 | 0.203 |
| Remaining stock, top 20, before the code | 18 to 8 (6) | 1.75 mm | 0 | 0.223 |
| Remaining stock, top 12, before the code | 10, 8 | 8.75 mm | 0 | 0.061 |
| Remaining stock, top 20 or top 12, after the code | 18 to 8 (6) | 1.75 mm | 0 | 0.223 |
| Remaining stock, a curve under the floor, before the code | 12, 10, 8 | 1.75 mm | 0 | 0.383 |
| Remaining stock, a curve under the floor, after the code | 10, 8 | 1.75 mm | 0 | 0.105 |

| # | Result |
|---|---|
| B1 | See section 7 (the op golden runs once on the full tree). |
| B2 | Held. Green before the code: the filter drops the passes above the floor. |
| B3 | Held. Red before the code (8.75 mm at X 17.8, bound 2.5 mm); green after it. |
| B4 | WRONG in the count before the code: 3 levels (12, 10, 8), not 6. The filter drops the passes at 18, 16 and 14, and it keeps the pass at Z 12 on the floor at Z 12. After the code: 2 levels, as predicted. |
| B5 | Held. `every_op_conforms_w4` check 2 and check 2b are green (11 passed; check 6 is the known red). `every_op_states_its_capabilities_w2_4` moves by the one cell. |
| B6 | WRONG in the cut length (it is lower). See the table below. |

A limit of the filter, found by the fixture: a move is air only when the whole cutter has 0.1 mm of
clearance over the stock. A pass that runs ON a floor (the same Z) is fed. That is the air share
that stays in the rows "top 20" (0.223). On the operator job the air time of the river op falls
from 919 s to 128 s (see the table below); the 128 s that stay were not split by cause.

### The wanaka river op (read only)

Instrument: the release CLI of this tree, `rs_cam_cli project <copy> --setup "1 Back" --skip
25,26,27,12,13,14,15,16 --summary`, on two copies of
`planning/rivmap_block_job/nz_wanaka_21_lead_proposal.toml` in the scratch folder (the model paths
of the file are absolute). The copy "rest" differs in one line: `stock_source =
"from_remaining_stock"` on toolpath 11. Simulation cell 0.2 mm (the project value), feed modulation
cap only (the project value). Each run took about 60 s and 8.1 GB. The op time is the simulated
time of the setup less the time of a third run that also skips toolpath 11 (1594 s).

| "River channel 6 mm, pass 7 of 7" | As committed (fresh, top 25.5) | Remaining stock |
|---|---|---|
| Moves | 121519 | 48023 |
| Cut length | 39094 mm | 13273 mm |
| Rapid length | 41154 mm | 49903 mm |
| Plunges | 1141 | 438 |
| Simulated op time | 2012 s | 1185 s |
| Air time of the op (the air time of the setup less the 886 s of the run with no river op) | 919 s | 128 s |
| Deepest body sample (the depth row of the force budget) | 2.045 mm | 2.045 mm |
| Samples that cut | 57130 of 401164 | 57130 of 284651 |
| Collisions, rapid collisions | 0, 0 | 0, 0 |
| Feeds above the stated feed | 0 | 0 |

- The first column is the figure of the proposal (121519 moves, 39094 mm, 41154 mm), so the
  instrument is the instrument of the lead.
- The samples that cut are the same 57130, and the deepest sample is the same. The op from
  remaining stock removes the same material in 827 s less.
- The cut length is 13273 mm, below the predicted 22000 to 27000 mm. The prediction counted 7 of
  12 passes. The rest op before the river also cut the hollow and the lake pockets below the cavity
  floor, so samples of the lower passes are in air too.
- The rapid length is 8749 mm more: a retract bridge replaces each long run in air.
- The stock top under the curve is 25.5, the stated top, so the ladder of this op did not move.

Decisions for the lead (part B):

1. Rule 6 of section 2.2: no exemption of the deepest pass from the filter.
   A second departure from the brief ("chains split where samples drop, with the entry and link
   rules of the op"): where the filter drops a run inside a chain, it bridges with a retract, a
   rapid at the retract plane and a RAPID descent to the resume point (the bridge of every op from
   remaining stock), not with the straight plunge of the P33 entry. On the wanaka run the plunges go
   from 1141 to 438, and the op has 0 collisions and 0 rapid collisions. No fixture test asserts
   the bridge descent.
2. The proposal file is not changed. The change is the one line above.
3. With remaining stock a stated top Z is not read by the ladder (rule 2). A stated top above the
   stock adds no pass; a stated top below the stock no longer cuts deep.

## 6. Part C: what is built, and the results

- `AdvisorContext::feed_modulation`, captured in `capture_recommend_clearing_strategy`
  (`session/compute/params.rs`).
- `optimized_candidate` (`session/compute.rs`) reads `context.feed_modulation.mode()`, the rule
  that `modulate_simulation_trace` reads. `None` (off): the function returns before the candidate
  simulation, and the caller times the planned path, as it does for each candidate that the
  advisor cannot modulate. The line that stated `ModulationMode::Full` is deleted.

Tests (`session/compute/tests.rs`; the terrain fixture with a 3 mm stepover and 6 mm levels, so a
debug build runs the two tests in about 100 s with the old advisor test):

| # | Result |
|---|---|
| C1 | Held. Before the code: full 9397.01 s, cap only 9397.01 s, and 2114 feeds of the cap-only candidate above the planned feed (the red assertion). After the code (light fixture): full 9264.58 s, cap only 9362.99 s, planned 9233.40 s; feeds above the planned feed: full 695, cap only 0. |
| C2 | Held. Off: `optimized_candidate` gives `None`, and the caller times the planned path. |
| C3 | WRONG: the two times are equal. `the_advisor_time_is_the_time_of_a_simulation_in_the_same_mode_p35`: with every dressup off and the default simulation options, cap only gives 9362.99 s against 9362.99 s and full gives 9264.58 s against 9264.58 s (19908 moves each). The test holds the equality to a relative 1e-6. On this fixture the session stages and the resolution add no difference. |
| C4 | `planning/fixtures/rivmap100/rivmap100.toml`, toolpath "3D Rough", release build, evidence test `the_advisor_reads_the_switch_on_rivmap100_p35` (ignored in a gate run): the choice does NOT change. See the table. |

| Switch | Chosen | Geodesic Adaptive | Contour Parallel | Ratio | Regime |
|---|---|---|---|---|---|
| off | Geodesic Adaptive | 1233.8 s | 1903.6 s | 1.543 | Unconstrained |
| cap only | Geodesic Adaptive | 1344.5 s | 2009.7 s | 1.495 | Machine limited |
| full | Geodesic Adaptive | 1344.5 s | 2009.7 s | 1.495 | Machine limited |

- Cap only and full give the same times on rivmap100 to 0.1 s. NOT verified why. The terrain
  test shows that the mode reaches the modulator (695 feeds lifted in full, 0 in cap only).
- The equality of C3 holds for the candidate op itself with no dressup. In the full advisor job
  each candidate is the Suggest proposal of the op with another strategy, and the stored toolpath
  has its dressups, so the advisor time is not the time of the stored toolpath after a
  simulation. That difference is of before P35 and is not changed.

Decision for the lead (part C): with the switch off the regime of each candidate reads
`Unconstrained` (the label of a path that the advisor did not modulate).

## 7. Gates (the full tree, parts A, B and C)

One run of each gate on the tree that holds the three parts, 2026-11-16, debug build.

| Gate | Result |
|---|---|
| Full clippy (`--workspace --all-targets`, the three core features, `-D warnings`) | Clean. |
| Core `--lib` | 2836 passed, 1 failed: `feeds::envelope::tests::the_envelope_meets_its_time_budget` (the known red of a parallel run). |
| `rs_cam_mcp` tests | Passed (2 binaries). |
| `rs_cam_cli` tests | Passed (17 binaries). |
| Viz `--lib` (`--test-threads 2`) | 705 passed, 2 ignored. |
| Viz `mcp_wire_surface_pin`, `side_on_greys_in_control_p6_7`, `the_help_key_is_the_param_name_ui04`, `operations_registry_ui05`, `the_inspector_draws_only_kit_rows_w6`, `folder_maps_name_real_files_f140` | Passed. The wire pin did not move. |
| `every_op_keeps_its_toolpath_g_opgolden` | 46 changes: the 46 rows of `opgolden_known_46_rows.txt` (sorted diff empty). Every `profile/*` and `project_curve/*` arm reads "ok". A1 and B1 held. |
| FM1 (`--features heavy-tests --test feeds_matrix_instrument_fm1`) | 2 passed. No CSV moved. Not blessed. |
| `perf_golden_sim_metrics` (`--features test-support`) | 5 passed. Not blessed. |
| `set_param_refuses_absent_field_n5` | 7 passed. No pin moved. |
| The three new test files and the two new lib tests | Passed (the rivmap100 evidence test is ignored in a gate run). |
| 34 folder sentries of `ops/`, `compute/`, `dressup/` and `session/` (the list is in the gate script of the hand-off) | Passed, but for the three files below. |
| `every_op_conforms_w4` | 11 passed, 1 failed: check 6 (`contour_each_level`), the known red. |
| `layered_framework_owns_the_ladder_w3` | 5 passed, 2 failed: the known red, with the list of `KNOWN_REDS_TRIAGE.md` ("zigzag", "trace", "profile tabs", "profile tabs spring"). The Profile hashes of this file were not compared with a run of the base commit. |
| `simulation_issue_channel_m1` | 2 failed: the known red (`NotSet` stepover of the fixture). |
| `strategy_advisor_smoke` (`--features heavy-tests`) | 1 passed, in 336 s. `Cargo.toml` says 43 s. The time was not measured on the base commit. |

Not run: `a_clearing_cut_holds_the_pass_load_g_adaptpassload` (a known red that no part touches),
the whole viz crate, the core full gate.

## 8. After the gates: the lake bands of the operator job with the new key (read only)

A scratch copy of the proposal with `stock_to_leave_radial = -1.4` on the three band ops (side
`on`), through the release CLI of the tree with parts A and B, setup "1 Back":

- "Lake relief band 6 mm, 10 lakes to Z 13.5" REFUSES: "stock_to_leave_radial = -1.400 mm moves
  the loop that starts at (146.184, 167.260) to 4 rings (the loop splits)". One lake has parts
  narrower than 2.8 mm.
- "Lake relief band 6 mm, lake at X325 Y220 to Z 14.9" generates (366 moves, 0 collisions).
- "Lake relief band 6 mm, 1 lake to Z 11.4" generates (49 moves, 0 collisions).

So decision 2 of part A is a decision of the job: with the refusal as built, the operator takes
that lake out of the 10-lake op, or the lead rules that a split loop cuts each ring.

Not verified in this step: a `LadderTop` op on a Bottom setup; the plunge descent of the river op
(the deepest figure is the depth row of the body cut); the GUI row of part A on screen; cap only
against full through the public advisor door on a project where the two differ; the Profile hashes
of `layered_framework_owns_the_ladder_w3` against the base commit; the 336 s of
`strategy_advisor_smoke` against the base commit. The MCP description of
`recommend_clearing_strategy` (`rs_cam_viz/src/mcp_server.rs`) does not name the mode; it was not
changed. `target/release/rs_cam_cli` is the build of the tree with parts A and B (no part C), not
of HEAD.

## 9. Part D: a split loop cuts each ring (lead ruling 2026-11-16)

Ruling: a loop that the radial value splits into several rings does not refuse. Profile cuts each
ring that the offset gives back, for every side, and reports the split. A loop that collapses to
nothing still refuses. This replaces rule 4 of section 1.2 and decision 2 of section 4.

Predictions (before the code):

| # | Prediction |
|---|---|
| D1 | Red first. A dumbbell (two 20 mm squares, a 2 mm neck), side `on` with -1.4 and side `inside` with -1.6: before the code the op refuses ("2 rings"). After it the op cuts two rings, each cut point is 1.4 mm inside the loop, and no fed move crosses the neck. |
| D2 | One warning names the start point of the loop and the ring count. |
| D3 | A 2 mm square with -1.4 still refuses. No key: the op golden does not move. |

What is built:

- `ops/profile.rs`: `profile_path_reported` returns `ProfileRings` (the rings, the offset failure
  count, the split). `on_line_contours` returns the contours and the splits. The refusal
  `RadialLeaveRefusal::NotOneRing` is now `Collapsed` (no ring only). With no value the function
  gives no ring or the first ring of the offset, as before.
- The report. The Profile has no channel for collapsed geometry (a collapse with no value cuts
  nothing and says nothing). Its one per-loop report is the note list of the adapter
  (`ProfileReport::tabs`, `TabPlacementFinding`). The split is a new note there,
  `TabNote::SplitLoop { start, leave, rings }`, and no new stats field is built.
- The diagnostic. A split is not a tab remark, and the tab remark has the anchor `Note`, which is
  Info at every site (W1). So the adapter gives the notes as two findings: `geom.tab_placement`
  (Info, the tab remarks) and the new id `geom.profile_loop_split` (Caution, `warn(Danger::Part)`,
  anchor `Operation`, face "A loop splits: its narrow parts are not cut"). The message names each
  split loop: "The loop from X.. Y.. splits into N rings with a radial stock to leave of .. mm".
- The tabs on a split loop (`dressup/tabs.rs`): the first ring is the ring of the region. It reads
  the drawn outline for the side rule and takes the hand tabs of the loop. Each later ring has no
  region: it takes automatic tabs by its own sides (`tab_count`), as a nested ring of side `on`
  does. The `tab_layout` query (the tab editor) gives marks only for a ring with a region, so the
  editor shows the tabs of the first ring only. The cut has the tabs of every ring.

Results:

| # | Result |
|---|---|
| D1 | Held. Red before the code: "Operation failed: Profile: stock_to_leave_radial = -1.400 mm moves the loop that starts at (0.000, 0.000) to 2 rings (the loop splits)". After it `a_split_loop_cuts_each_ring_and_warns_p35d` is green for the two sides. An outward offset (side `outside` with a positive value) does not split a loop, so it has no arm. |
| D2 | Held. `stats.tab_placement` holds one `SplitLoop` (start 0, 0; 2 rings), and the diagnostics hold `geom.profile_loop_split` (Caution) and no `geom.tab_placement`. With `tab_count = 2` the cut lifts over a tab on each ring. |
| D3 | Held (`a_collapsed_loop_and_an_open_path_refuse_p35`; the op golden: section 11). |

The "10 lakes" band of the proposal (read only; a scratch copy with `stock_to_leave_radial = -1.0`,
side `on`, no exclude, floor Z 13.5; release CLI of this tree for the simulation, and the evidence
test `a_lake_band_reports_its_split_loops_p35d` for the notes):

- The op generates: 7206 moves, 6252 mm cut, 4320 mm rapid, 0 collisions, 0 rapid collisions.
- Three of the eleven lakes split. The other eight are one ring each. 15 rings in all.

| Lake (start of the loop) | Rings |
|---|---|
| X146.18 Y167.26 | 3 |
| X105.35 Y222.91 | 2 |
| X70.75 Y414.49 | 2 |
| each of the other eight | 1 |

- Warnings of the op: one `geom.profile_loop_split` (Caution) with the three notes above. The CLI
  `summary.json` lists the id under the findings of the toolpath; it does not print the note text.
- The ring count of a lake with no note is 1 by the rule (a collapse refuses); it was not counted
  on the path.

Decisions for the lead (part D): the split has its own id and the anchor `Operation`; the tab
editor does not show the tabs of the later rings of a split loop.

## 10. Part E: the rapid re-entry of the air-cut filter on a `LadderTop` op

Departure 3 of the step: where the filter drops a run inside a chain, it comes back with a retract,
a rapid at the retract plane and a RAPID descent to the resume point, not with the P33 plunge.

Prediction (before the test): the resume point is the end of the last move that the filter calls
air, and the filter calls a move air only when the whole cutter has 0.1 mm of clearance over the
stock before the op. So the rapid descent ends in air, and the first fed move after it starts in
air. No rapid enters stock, and no fix of the filter is necessary.

The test: `a_rapid_re_entry_after_a_dropped_run_stays_out_of_the_stock_p35e` (in
`project_curve_passes_read_the_remaining_stock_p35`). The stepped-stock fixture with the pocket in
the MIDDLE of the curve (X -30 to X 30, floor Z 12; the curve X -45 to X 45, full depth Z 8, step
2.0 mm, cell 0.5 mm). Each pass above the floor (Z 18, 16, 14) cuts at the two ends and is in air
between them, so the filter bridges it.

| Claim | Measured |
|---|---|
| Non-vacuity: the stored path holds rapid descents that end below the stock top | 3 (one for each pass above the floor) |
| Each such rapid is vertical | Held |
| No cell of the stock before the op, with its centre in the tool disc, stands above the end of the rapid | Held for the 3 descents |
| The simulation of the session finds no rapid into stock | 0 rapid collisions |
| The first fed move after each descent cuts no more than one step | 3 samples, the deepest 0.00 mm (the move is still in air; the cut starts on a later move) |
| No sample of the op cuts more than one step (2.0 mm plus the cell) | Held |

The prediction held. The filter is not changed.

Not covered: a tool that is not a flat end mill (the footprint check of the test reads the disc of
a flat tool), and a Bottom setup.

## 11. Gates of parts D and E (one run, the tree with D and E, 2026-11-16)

| Gate | Result |
|---|---|
| Full clippy | Clean. |
| Core `--lib` | 2836 passed, 1 failed: `the_envelope_meets_its_time_budget` (the known red of a parallel run). The `_w1` rows of the diagnostics table passed with the new id. |
| `rs_cam_mcp`, `rs_cam_cli` tests | Passed. |
| Viz `--lib` (`--test-threads 2`) | 705 passed, 2 ignored. |
| Viz `mcp_wire_surface_pin`, `one_place_per_diagnostic_w2b`, `the_inspector_bad_states_w0`, `folder_maps_name_real_files_f140`, `a_job_with_each_finding_exports_and_names_it_on_every_surface_k6` | Passed. |
| `every_op_keeps_its_toolpath_g_opgolden` | 46 changes: the 46 known rows (sorted diff empty). Every `profile/*` arm reads "ok". |
| 29 core sentries (the two P35 test files, the Profile and tab sentries, the `dressup/` and `diagnostics/` sentries, `findings_transport_join_h21`, the folder map and test hygiene sentries) | Passed, but for the two known reds: `every_op_conforms_w4` check 6 and `layered_framework_owns_the_ladder_w3` (the same two tests and the same hashes as in section 7). |

Not run for D and E: FM1 and `perf_golden_sim_metrics` (no code of a Fresh op with no key moved;
the op golden holds that), `strategy_advisor_smoke`.
