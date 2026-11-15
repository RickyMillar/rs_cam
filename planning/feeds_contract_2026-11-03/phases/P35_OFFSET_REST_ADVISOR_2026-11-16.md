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
