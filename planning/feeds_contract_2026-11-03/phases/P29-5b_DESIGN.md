# P29-5b design: the filing flags of the cut list are true

Written 2026-11-12 on HEAD `8413af726`. No product code changed. The lead reviews this note,
commits it with the two prediction files, and then starts an implementer. The result of each
commit is judged against section 8. A mismatch stops the work.

Files of this step:

- this note;
- `P29-5b_predictions_fm1.csv`: every FM1 cell value that moves, per commit (P29-6 columns).
  65 rows, all in the matrix file. A cell value that is not in this file must not move.
- `P29-5b_predictions.csv`: table `S` (Suggest on the default machine: the region Face), table
  `W` (the two wanaka files, read-only loads, options A and B of Q1), table `G` (the op golden),
  table `F` (the FM1 simulation file), table `H` (hand checks).

Order assumed for the three notes: P29-5b, then P29-5c, then P29-7. This note predicts on HEAD.

Rulings in force: R3 (a slot is a slot under every op), R1 (the force budget, 35.3224 N on the
default machine; the operator parked its tuning on 2026-11-12). Every finding warns; no job is
refused. No legacy (2026-09-16): a rule that no reader reads after the step is deleted.

## 1. Scope

### 1.1 The sources, and where they disagree

| Source | What it puts in P29-5b |
|---|---|
| Change list (`P29_FINDINGS.md` section 6) | "Filing flags fixed (Profile intent, Face region, sweep, Bore depth, VCarve stepover, Inlay one pass): rows `M11`, `P28-12`, `U-PerimeterSweep`. Moves FM1 power and speeds columns." |
| Row `M11` | "wrong first-pass filings (region Face, 3D sweep, Bore depth, VCarve stepover, Inlay one pass)", `closes_in` "P29-5b (filing)" |
| Row `P28-12` | "the first slots of 3D Adaptive, region Face and 2D Adaptive without slot clearing" |
| Row `U-PerimeterSweep` | "the full-slot perimeter sweep that opens a 3D Adaptive level" |
| Row `P28-32` (closes in P29-9) | "a helix pitch, a chamfer width and a peck give no depth" |

The disagreements:

1. **"Profile intent" has no exemption row.** The Profile emitter writes `FinishingCut` on every
   level (`ops/profile.rs:226`, `:285`), and the op is filed Roughing. The readers of the intent
   (`dressup/arcfit.rs`, `surface/chord_refine.rs`, `metrology/spacing.rs`, `compute/spans.rs:342`,
   `tool_load/locality.rs:139`, `limits/sample.rs:251`) treat `ClearingCut` and `FinishingCut`
   alike, except `chord_refine` and `metrology/spacing`, which read only `FinishingCut`. So the
   fix is a label: it moves no FM1 cell (measured) and the `moves_hash` of six Profile arms of
   the op golden (measured).
2. **"Bore depth" is also row `P28-32`.** `CutSpec::calc_inputs` gives no depth for
   `Axial::HelixPitch`, so the calculator sizes a Bore at the family default depth (Pocket,
   Roughing, `ap_factor` 0.70: 4.2 mm on a 6 mm tool). A helix of pitch `p` engages `p` of
   material at its leading edge, at a full slot width. Proposal: 5b gives the helix pitch to the
   calculator and to the declared depth, and `P28-32` narrows to "a chamfer width and a peck"
   (Q3).
3. **"VCarve stepover" and "Inlay one pass" move no number.** A V-bit's engaged section is the
   cone at the depth (`feeds::section::EngagedSection::VGroove`); it does not read `ae`. The
   power slot case of a V groove is the nominal case. The filing fix declares the first V slot
   (so P29-7 sees it) and moves no FM1 cell (measured: VCarve and Inlay rows are byte-equal).
4. **"2D Adaptive without slot clearing" (row `P28-12`) is not a slot.** The planner holds every
   fed step to the pass ceiling `c D` (`adaptive/CLAUDE.md`); the opening is a helix bore with a
   flat lap, which the op declares as `Entry(Helix)` (its entry contract), and the steps out of a
   straight plunge are the departure window, a capped step (row `U-CappedStep`, P29-7). Proposal:
   2D Adaptive keeps `FirstPass::NotSlot`, and the row text records why (Q2).
5. **"Region Face" depends on the selection.** `FaceConfig::selection` is a config field
   (`Selection::is_stock`). A stock Face grows the stock region by `stock_offset`, so its first
   row is open on one side. A Face of any other region keeps the tool inside the outline "as in a
   pocket" (ruling O-3), so its first row is a slot. FM1 and both wanaka files use the stock
   selection, so the fix moves no FM1 or wanaka cell.
6. **"Sweep" depends on the strategy.** The 3D Adaptive perimeter sweep is a plain cut at the
   op feed (`adaptive3d/clearing.rs:3467-3530`): a full slot at the full depth and at the LIFTED
   feed. Under `ContourParallel` (the default, `operation_configs.rs:1968`) it opens every level.
   Under `GeodesicAdaptive` it runs only where the grow refuses (`grown_outlines`, P22f-1; sentry
   `a_geodesic_slice_cuts_no_full_slot_sweep_p22f`). Every wanaka 3D rough is geodesic. This is
   the main trap of 5b for P29-7: a slot declared at the op feed becomes a full-slot depth cap
   in P29-7. Question Q1 decides it.

**Proposed scope:** the six filing fixes of the change list, with the readings of items 1 to 6:
Profile intent (label), Bore depth (helix pitch), first-pass filings of region Face, 3D Adaptive
under `ContourParallel`, VCarve and Inlay. Rows `M11` and `P28-12` close. `U-PerimeterSweep`
narrows to the geodesic residual sweep and closes in P29-7 (option A of Q1).

### 1.2 Visible product surfaces

- The lineage card prints `first_pass` (`trace/lineage.rs:1251`, "slot" / "not_slot"): it moves
  for a region Face, a 3D Adaptive under ContourParallel, VCarve and Inlay.
- The Suggest card power figure and its case name (`power_kw`, `power_case`) move for the same
  ops where the slot is wider than the nominal cut (FM1: the 24 Adaptive3d cells; a region Face).
- The Bore card shows the depth of one helix turn in place of the family default depth.

## 2. The behaviour changes, each as the smallest edit

### 2.1 Profile intent (5b-1)

`ops/profile.rs` takes the body intent from the caller in place of the literal `FinishingCut`
(`profile_path_open_to_toolpath` and the closed-ring emitter, `ClosedPassMoves::body_intent`).
The Profile adapter passes `MoveIntent::ClearingCut`. Chamfer, which emits through the same
ring function (measured: the throwaway edit of the shared literal moved `chamfer/fresh` and
`chamfer/rest`), keeps `FinishingCut`. A spring pass of a Profile keeps the intent of its emitter
(not changed here; row `U-SpringPass`, P29-7).

Why `ClearingCut`: a Profile level is a full slot into fresh stock (D3), and the op is filed
Roughing; the intent names the cut, not the last level.

### 2.2 Bore depth (5b-2)

`compute/catalog/cut_spec.rs` `CutSpec::calc_inputs`:

```rust
Axial::HelixPitch(s) => Some(s.mm()),
// EXEMPT(P28-32)
Axial::Peck(_) | Axial::ChamferWidth(_) | Axial::SurfaceOffset(_) => None,
```

`compute/catalog/cuts.rs` `OperationConfig::declared_depth`:

```rust
Axial::HelixPitch(s) => DeclaredDepth::Stated(s.mm()),
Axial::Peck(_) | Axial::ChamferWidth(_) => not_stated,
```

The envelope already reads the pitch as the stated depth (`envelope.rs` `axes_plan`,
`Axial::HelixPitch(s) => (true, Some(s.mm()), s.mm())`), so the calculator and the envelope then
read one depth. `operation_feeds_hints` equals `calc_inputs` (sentry
`every_op_states_its_capabilities_w2_4::the_feeds_hints_read_the_ops_own_fields`).

### 2.3 First-pass filings (5b-3)

`compute/catalog.rs` `OperationConfig::cut_spec`:

| Op | Today | After |
|---|---|---|
| Face | `FirstPass::NotSlot` under `EXEMPT(P28-12, M11)` | `if c.selection.is_stock() { NotSlot } else { Slot }` |
| Adaptive (2D) | `Slot` if `slot_clearing`, else `NotSlot` under `EXEMPT(P28-12)` | unchanged; the marker goes, a comment names the pass ceiling and the entry |
| VCarve | `NotSlot` under `EXEMPT(M11)` | `Slot` (the first row is a full V slot) |
| Inlay | `NotSlot` under `EXEMPT(M11)` | `Slot` (the wall pass is a full V slot at the pocket depth) |
| Adaptive3d | `NotSlot` under `EXEMPT(P28-12, M11)` | option A: `Slot` under `ContourParallel`, `NotSlot` under `GeodesicAdaptive` with `// EXEMPT(U-PerimeterSweep)`; option B: `Slot` always |

InlayPlug and Chamfer keep `NotSlot`: the plug clears around a raised plug, and a chamfer cuts an
edge with air on one side. Neither is in the row lists.

The readers that follow: `OperationConfig::cuts` (`declare_side_cuts` adds
`CutKind::FirstPass(FirstPassKind::Slot)`), `PowerCases::of_cuts` (the slot case), the lineage
card. No generator reads `first_pass`.

## 3. Commits

Three commits. Each passes the gate set alone.

| Commit | Content | Moves (FM1) | Rows |
|---|---|---|---|
| 5b-1 | Profile intent (2.1) | 0 cells; op golden: `moves_hash` of 6 Profile arms (field-only bless, `RS_CAM_BLESS_OPGOLDEN_FIELDS=moves_hash`, `RS_CAM_BLESS_OPGOLDEN_ARMS=profile/...`) | none |
| 5b-2 | Bore depth (2.2) | 41 cells in 22 Bore rows: `r_axial_depth_mm` 16, `force_n` 12, `suggest_warnings` 7, `cap_mm` 6 | `P28-32` narrows (text) |
| 5b-3 | first-pass filings (2.3) | 24 cells: `power_kw` of the 24 Adaptive3d rows | `M11`, `P28-12` close; `U-PerimeterSweep` narrows (option A) or closes (option B) |

Gates of every commit: fmt check; the named sentries `the_exemption_list_only_shrinks_p29_3`,
`every_limit_is_a_limit_id_p29_2`, `every_op_declares_its_cuts_p29_3`,
`a_slot_reads_the_force_budget_under_every_op_p29_5`, `every_op_states_its_cut_k1`,
`every_op_states_its_capabilities_w2_4`, `pre_sim_power_ships_the_worst_case_k7`,
`one_engagement_before_and_after_the_simulation_k7`, `the_lift_reads_the_pass_ceiling_width_p23a`,
`the_power_curve_picks_the_arm_at_the_feed_cap_q1`, `the_force_line_is_printed_per_family_b6`,
`no_depth_is_proposed_that_cannot_be_modelled_p16`, `a_bore_cuts_the_stated_size_p6_6`,
`every_op_keeps_its_toolpath_g_opgolden` (diff against
`planning/feeds_contract_2026-11-03/opgolden_known_46_rows.txt`); `--doc cut_spec_refusals`;
`cargo test -p rs_cam_core -q --lib`; FM1 (`--features heavy-tests`); full clippy; viz `--lib`
and `mcp_wire_surface_pin` (the lineage `first_pass` key), `p17_visual_snapshots` (no viz test
while `pgrep -ax rs_cam_gui` prints).

## 4. Exemption rows and pins

Pins today: ROWS 51, PHYSICAL 8, RULED 3, MAX_NEEDS_RULING 40, ROWS_WITH_A_FILE 43.

| After | Rows closed | `ROWS` | `MAX_NEEDS_RULING` | `ROWS_WITH_A_FILE` | `CLOSED` gains |
|---|---|---|---|---|---|
| 5b-1 | none | 51 | 40 | 43 | - |
| 5b-2 | none (`P28-32` narrows) | 51 | 40 | 43 | - |
| 5b-3, option A (recommended) | `M11`, `P28-12` | 49 | 38 | 42 | `("M11", "P29-5b")`, `("P28-12", "P29-5b")` |
| 5b-3, option B | `M11`, `P28-12`, `U-PerimeterSweep` | 48 | 37 | 41 | the same and `("U-PerimeterSweep", "P29-5b")` |

`M11` and `P28-12` name a file, so each lowers all three counts. Under option A,
`U-PerimeterSweep` gains its first file (the new marker below), so `ROWS_WITH_A_FILE` falls by
one only (43 -> 42). Under option B it closes with an empty `where_`, so it lowers `ROWS` and
`MAX_NEEDS_RULING` only. `PHYSICAL` 8 and `RULED` 3 stay. The array length equals `ROWS`.

Rows that stay and change (text only):

- `P28-32`: `who` "a chamfer width and a peck give no depth"; `where_` unchanged
  (`src/compute/catalog/cut_spec.rs`).
- `U-PerimeterSweep` (option A): `who` "the residual full-slot sweep of a geodesic 3D Adaptive
  level where the grow refuses (a generation fact); the gates judge it after the simulation";
  `where_` `["src/compute/catalog.rs"]` (the new marker on the geodesic arm); `closes_in`
  "P29-7 (a declared cut that may occur, judged as a warning)".

Markers that go: `EXEMPT(P28-12, M11)` at `catalog.rs:1193` and `:1288`; `EXEMPT(P28-12)` at
`:1217`; `EXEMPT(M11)` at `:1227` and `:1234`.

## 5. Tests that go red

Found by `rg` for `FirstPass::`, `HelixPitch`, `PowerCase::PlannerCap`, `calc_inputs`,
`MoveIntent::FinishingCut` and by the runs. A red outside this list in the gate set is a
mismatch.

**5b-1:**

- `tests/every_op_keeps_its_toolpath_g_opgolden.rs`: `moves_hash` of `profile/each_face`,
  `profile/fresh`, `profile/inside`, `profile/on_closed`, `profile/on_open`, `profile/rest`
  (measured). Field-only bless; the bless log names P29-5b-1. `gcode_hash` stays.
- At risk (not run): any test that pins `FinishingCut` on a Profile move. `rg` finds the literal
  in 30 test files; the implementer lists those that build a Profile.

**5b-2:**

- FM1 matrix golden (41 cells, section 8.1).
- `src/compute/catalog/cut_spec.rs`: the doc of `calc_inputs` ("A peck, a helix pitch and a
  chamfer width are not a side-cut depth").
- At risk: `tests/a_bore_cuts_the_stated_size_p6_6.rs` if it pins a Bore force or depth figure;
  any `feeds/tests.rs` test that reads a Bore default depth.

**5b-3:**

- `tests/pre_sim_power_ships_the_worst_case_k7.rs` `an_op_one_rough_ships_the_planner_cap`: the
  default 3D Rough (ContourParallel) now ships `PowerCase::Slot`. New claim: a ContourParallel
  rough ships the slot case (its perimeter sweep); a geodesic rough ships the planner cap
  (option A). Under option B both ship the slot case.
- `tests/every_op_states_its_cut_k1.rs`: the first-pass check names Adaptive only (line 359); it
  stays green; add the Face region and the Adaptive3d strategy states (new claim, not a red).
- `tests/the_exemption_list_only_shrinks_p29_3.rs`: the pins and `CLOSED`.
- FM1 matrix golden (24 cells).
- viz `mcp_wire_surface_pin` if the snapshot holds a lineage reply of one of the moved ops (not
  checked).

## 6. Docs to change

- `FEATURE_CATALOG.md`: the pre-sim power sentence (the slot case of a region Face and of a
  ContourParallel 3D rough); the Bore sentence (the calculator reads one helix turn as the
  depth).
- `crates/rs_cam_core/src/compute/CLAUDE.md`: no rule changes; the K1 line stays.
- `crates/rs_cam_core/src/limits/CLAUDE.md`: no change.
- `P29_FINDINGS.md` section 6, the P29-5b row: the scope of section 1 (the six items, what each
  moves).

## 7. Risks and questions for the lead

Most important first. Each question has my choice.

**Q1. The sweep of a geodesic 3D rough.** Option A (my choice): file the slot only under
`ContourParallel`, where the sweep opens every level. The geodesic sweep is a residual case
that the plan cannot see before generation (`Produced::region_openings` records it after); the
gates judge its samples after the simulation (P29-6, Body group). Option B: file the slot for
both strategies. Run B on the operator file: the Suggest feed of idx 1 at the STATED cut
(10 x 5.5 mm, 18 mm) falls from 9123.0 to 2624.0 mm/min (the slot needs 2.64 kW, the machine has
1.5 kW), and in P29-7 the proposal of every geodesic rough falls to the full-slot budget depth
(for example operator idx 2: 3.5932 mm to 2.7306 mm). That is a large cut for a cut that rarely
occurs. A third option for later: the planner feeds the sweep at the unlifted feed (the P23d rule
for a full slot); it is a generator change and not in this step.

**Q2. Row `P28-12` and 2D Adaptive.** My choice: close the row; 2D Adaptive without slot clearing
cuts no body slot (section 1.1 item 4). The opening is a declared entry; the departure out of a
plunge is row `U-CappedStep` (P29-7).

**Q3. Bore depth owns a part of `P28-32`.** My choice: 5b-2 takes the helix pitch out of
`P28-32` (the row narrows); the chamfer width and the peck stay with P29-9.

**Q4. Profile intent.** My choice: the caller passes the intent; Profile passes `ClearingCut`,
Chamfer keeps `FinishingCut`. The other choice (re-file Profile as a finish op) contradicts D3
and R3.

**Q5. A region Face moves no FM1 cell.** The FM1 Face cells use the stock selection. My choice:
add one region-Face row to `every_op_states_its_cut_k1` in 5b-3 so the claim is pinned; table
`S` holds its numbers.

Risks:

- The Profile intent reaches `dressup/arcfit.rs` (arcs fit across one intent run). The op golden
  run showed only `moves_hash` moving, so no arc count changed.
- No op golden arm holds a Profile with tabs (wanaka idx 17 has tabs). Reasoning, from a search
  for readers that single out `ClearingCut` or `FinishingCut` (`rg` over `dressup/`, `stock/`,
  `session/`, `gcode/`, `export/`, `machine/`): `dressup/tabs.rs` keeps the intent of each move it
  rewrites (CUT-08); `session/compute.rs:2185` (the planner floor of the modulator),
  `export/ribbon.rs:105`, `machine/kinematics.rs:1299` and the gates treat the two intents alike;
  only `metrology/spacing.rs` (a finish spacing instrument with no G-code effect) and
  `surface/chord_refine.rs` (not on a Profile) read `FinishingCut` alone. So the G-code and the
  feeds of idx 17 do not move. Not run on idx 17.
- `PowerCases` reads the slot at the op feed. For a ContourParallel sweep that is true (the sweep
  is emitted at the op feed). For a contour Pocket the ring is fed unlifted (P23d), so the Pocket
  slot case overstates its power today; not changed here (finding F3).

## 8. Predictions

Computed on HEAD `8413af726` with a throwaway edit (sections 2.1 to 2.3, option B for the
Adaptive3d filing, the Profile intent as a shared literal) and a throwaway probe test. The files
were restored from byte copies, checked by `sha256sum -c` and touched; the probe file was
deleted; `git status --short crates/` is clean; the FM1 fixtures hash as before. FM1 ran without
the update variable, with `RS_CAM_FEEDS_MATRIX_OUT` pointing at the scratchpad, so no fixture was
written; the walk copies were diffed offline.

Which numbers come from a run: every cell of 8.1, table `S`, the option B rows of table `W`, the
op golden arms of 5b-1. Which come from reasoning: the option A rows of table `W` (no geodesic
slot, so equal to HEAD), the two Chamfer arms (the design passes the intent; the run used a
shared literal), the test list except the measured reds.

Calibration: the probe on HEAD reproduced the P29-5 wanaka ceilings (2.6974, 5.5414, 4.5107),
and on the edited tree it reproduced every FM1 proposal of the walk on the same tree (0
mismatches over 430 cells).

### 8.1 FM1

Matrix file: 46 rows, 65 cells.

| Commit | Rows | Cells | Columns |
|---|---|---|---|
| 5b-1 | 0 | 0 | - |
| 5b-2 | 22 (Bore) | 41 | `r_axial_depth_mm` 16 (2.2225 or 4.2000 -> 1.0000, the fixture pitch), `force_n` 12, `suggest_warnings` 7 (`UnsourcedRowsAccepted` goes: the default-depth row is no longer read), `cap_mm` 6 (Bore V-bit: the calculator chip at the new depth) |
| 5b-3 | 24 (Adaptive3d) | 24 | `power_kw` (for example EndMill 6 mm hardwood 0.0478 -> 0.1147) |

No speeds, proposal, binding or warning column of an Adaptive3d cell moves: the slot power stays
under the rated power on every cell. VCarve and Inlay: 0 cells (the V section reads no width).
Face: 0 cells (stock selection).

Simulation file: 0 cells (measured, every commit together).

### 8.2 Suggest on the default machine (table `S`)

A Face of a picked region, EndMill and BullNose, 3.175 and 6 mm, softwood, hardwood, MDF: the
power rises by the slot case (for example 6 mm hardwood 0.1119 -> 0.1394 kW, case Nominal ->
Slot); rpm and feed stay on every cell.

### 8.3 The two wanaka files (table `W`)

Option A: no op of either file moves (every 3D rough is geodesic; the Faces use the stock
selection; no Bore, VCarve or Inlay; the Profile intent moves no number).

Option B (run): the four Adaptive3d ops of each file change their power case to Slot. Operator
idx 1 (18 mm, stated 10 x 5.5 mm): power 0.9400 -> 1.5000 kW (capped), feed 9123.0 -> 2624.0
mm/min, with a `PowerLimited` warning (2.64 kW required). The other seven ops: power only
(for example operator idx 2: 0.1383 -> 0.4481 kW), rpm and feed equal. No proposal moves.

### 8.4 What stays byte-identical

- Every toolpath geometry (no generator reads `first_pass` or the calculator depth).
- Every G-code. The op golden `gcode_hash` of every arm.
- Every post-simulation number (no gate reads `first_pass`).
- Every envelope edge and proposal (the envelope already read the Bore pitch; no FM1 or wanaka
  proposal moved in the run).

## 9. What changes for the operator's cut

Nothing that they cut with, under option A. The wanaka operator file keeps every depth, width,
feed and speed before and after a simulation. The Profile (idx 17) carries a new move label that
no number reads.

Under option B, the Suggest card of the 18 mm back rough (idx 1) asks for a much lower feed at
the stated 10 mm depth (2624 in place of 9123 mm/min), because it now sizes the power at a full
slot. At the proposed depth (2.09 mm) the slot power is far under the spindle, so a full apply
(depth and speeds) changes nothing. P29-7 would then lower every 3D rough depth to the full-slot
depth (section 7 Q1).

## 10. Findings outside the scope

- **F1.** The geodesic perimeter sweep is emitted at the full lifted feed when it runs
  (`adaptive3d/clearing.rs:3527`). A full slot at a lifted feed is a chip above the band by the
  lift. A planner rule (the unlifted feed, as P23d) would remove the case; it is not in P29.
- **F2.** `trace/lineage.rs:1251` prints `first_pass`; the MCP lineage reply moves with 5b-3.
- **F3.** `PowerCases` sizes the slot at the op feed. The contour Pocket ring is fed unlifted
  (P23d), so its pre-sim power case is high by the lift. Not measured.
- **F4.** InlayPlug clears an `outer_margin` around the plug with a V-bit (`ops/inlay.rs:27-31`)
  under `Radial::Groove`; whether that clearing is a groove or a one-sided cut was not checked.
- **F5.** The finish ops' `stock_to_leave` fields are not a radial leave
  (`radial_stock_to_leave_mm` reads only Bore). A semi-finish that leaves stock for a final pass
  has no leave fact. P29-7 R5 reads this function.

## 11. Lead rulings (2026-11-12)

- **Q1: option A.** The slot is filed only under `ContourParallel`. Row `U-PerimeterSweep` narrows
  to the geodesic residual sweep and stays open (see the P29-7 rulings: it does not close in 7b).
- **Q2, Q3, Q4, Q5: the note's choice.**
- **Added gate of 5b-1.** The claim that the tabbed Profile of the operator file (idx 17) keeps
  its G-code is reasoned, and no op golden arm has tabs. The implementer loads
  `planning/rivmap_block_job/nz_wanaka_21_operator.toml` read-only before and after the edit,
  generates idx 17, and compares the G-code bytes. A difference stops the commit.
- The option A cell count of 5b-3 (24 `power_kw` cells) holds because the FM1 Adaptive3d rows use
  the default strategy, `ContourParallel`. The run used the option B filing. A different count is
  a mismatch.

## 12. Result (2026-11-12)

The FM1 diffs equal the prediction files on every commit: 5b-1 0 cells, 5b-2 41 cells in 22 Bore
rows, 5b-3 24 `power_kw` cells. The simulation file stays byte-equal. The op golden moved only the
`moves_hash` of the six Profile arms. The G-code of the tabbed Profile of the operator file
(idx 17) is byte-equal before and after 5b-1.

Section 5 missed two tests:

- 5b-2: `feeds::suggest::tests::operation_feeds_hints_is_an_explicit_per_op_decision` pinned the
  Bore depth hint as `None`. The arm now reads the pitch (lead ruling).
- 5b-3: `the_lift_reads_the_pass_ceiling_width_p23a` (the contour arm) and
  `the_envelope_finding_reads_the_gate_feed_p20i::one_engagement_gives_one_kw_through_every_door_b`
  (no grid point in the planner cap case). Both are re-claimed: p23a holds the contour lift at a
  depth under the rated power and asserts that the deep cut ships `Slot` with `PowerLimited`;
  p20i reads the planner cap of a geodesic rough.

Sections 8.1 and 9 hold only for the FM1 cuts and the two wanaka files (every wanaka 3D rough is
geodesic). Under option A a deep stated cut of a 3D Rough under `ContourParallel` gets the option B
number: the p23a cut (18 mm tool, white oak, 10 x 5.5 mm, wanaka-like machine) ships case `Slot`,
2.68 kW required against 1.5 kW, and the Suggest feed falls from 9123 to 2624 mm/min
(`PowerLadderReducedCut`, feed factor 0.281). The lead accepted this (R3: the sweep is a full slot
at the full depth and the op feed on every level).

Finding outside the scope: under `ContourParallel` the slot case (width D) is at least the
`CapMult` planner cap case (width `min(m ae, D)`) at the same feed, so the `CapMult` case can no
longer be the largest case. Rows `P28-13` and `P28-14` and the power use of `trochoid_cap_mult`
may now have no effect on a shipped number. Not changed here.
