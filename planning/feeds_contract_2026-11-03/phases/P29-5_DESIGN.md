# P29-5 design: a slot or a groove reads the force budget under every op

Written 2026-11-12 on HEAD `6d1db7e3b`. No product code changed. The lead reviews this note,
commits it with the two prediction files, and then starts an implementer. The implementer's
result is judged against section 6. A mismatch stops the work.

Files of this step:

- this note;
- `P29-5_predictions_fm1.csv`: every FM1 cell value that moves (long form: file, cell key,
  column, committed, predicted). 110 rows. A cell value that is not in this file must not move.
- `P29-5_predictions.csv`: the Suggest table (rows `S`), the wanaka slot and groove ops (rows
  `W`) and the check of the printed slot rows (rows `SP`).

Rulings in force: R3 (operator, 2026-11-11: "slot is a slot! it can vary in z depth due to rest
and similar though"; before the simulation the limit reads the stated depth, after it each
sample is judged at its real depth). R2 (2026-09-23: a finish pass has no axial depth ceiling).
R1 (the force budget, 35.3224 N on the default machine). No job is refused: every finding
warns. No legacy: a rule that no reader reads after the step is deleted.

## 1. Scope

### 1.1 The three sources disagree

| Source | What it puts in P29-5 |
|---|---|
| Change list (`P29_FINDINGS.md` section 6) | slot and groove ops (Trace, VCarve, Inlay, Chamfer, Project Curve) read the budget **and the pre-sim deflection**; filing flags fixed (Profile intent, Face region, sweep, Bore depth) |
| Exemption rows that name P29-5 | `P28-03`, `F-DepthRule` ("P29-5 (P28-03)"), `M11` (wrong first-pass filings), `P28-12` ("P29-5 / P29-7"), `S-NotJudgedHere` ("P29-5 / P29-10"), `U-SpringPass`, `U-ResidueCleanup` ("P29-5 / P29-6"), `U-PocketZigzag`, `U-BoreWallFinish`, `U-RowConnector`, `U-PerimeterSweep` |
| Row `P28-15` | the pre-sim deflection of a finish cut closes in **P29-7**, not P29-5 |

These are four different behaviour changes. Each moves different numbers:

1. The depth limit (force budget) on a slot or groove cut that is filed as a finish pass
   (`P28-03`, `F-DepthRule`). This note.
2. The filing fixes (`M11`, `P28-12`, `U-PerimeterSweep`): they move the pre-sim power slot
   case (`FirstPass::Slot`) of Face, Adaptive without slot clearing, 3D Adaptive, Bore, VCarve
   and Inlay. That moves FM1 power and speeds columns, not the depth limit.
3. The undeclared cuts (`U-*`): each needs a declared cut and a sample kind that the gates
   judge. That is the work of P29-6 (sample kinds in the gates) and P29-7 (declared cuts read
   by force and deflection).
4. The pre-sim deflection of a slot finish (`P28-15`): it is the "one deflection limit" step,
   P29-7, ruling R5.

**Proposed scope: P29-5 is item 1 only.** Items 2 to 4 get new `closes_in` text (Q2). The
step then has one behaviour change, one sentry, and predictions that a run of the edited code
proved.

### 1.2 The one behaviour change

Today `RigidityProfile::depth_limit` (`machine/mod.rs:294`) keys on the family and the role
only:

```rust
// EXEMPT(P28-04)
(OperationFamily::Drill, _) => DepthLimit::NoLimit(NoDepthLimit::DrillCycle),
// EXEMPT(F-DepthRule)
(_, PassRole::Roughing) => DepthLimit::ForceBudget,
// EXEMPT(P28-02, P28-03, S-NoBound)
(_, PassRole::SemiFinish | PassRole::Finish) => DepthLimit::NoLimit(NoDepthLimit::FinishPass)
```

The edit gives the match a third input: the material sides of the body cut. A slot or a
groove has material on both sides, so it reads the force budget whatever its role:

```rust
match (family, pass_role, sides) {
    // EXEMPT(P28-04)
    (OperationFamily::Drill, _, _) => DepthLimit::NoLimit(NoDepthLimit::DrillCycle),
    (_, PassRole::Roughing, _) => DepthLimit::ForceBudget,
    // R3: a slot is a slot under every op.
    (_, PassRole::SemiFinish | PassRole::Finish, BodySides::Both) => DepthLimit::ForceBudget,
    // EXEMPT(P28-02, S-NoBound)
    (_, PassRole::SemiFinish | PassRole::Finish, BodySides::One) => {
        DepthLimit::NoLimit(NoDepthLimit::FinishPass)
    }
}
```

The parts of the edit:

| Part | Where | What |
|---|---|---|
| `BodySides { Both, One }` | `limits/cut.rs` (re-export in `limits/mod.rs`) | a geometry fact; names no op, family or role (limits invariant 1) |
| `RadialRule::body_sides()` | `compute/catalog/cut_spec.rs` | `Slot \| Groove` give `Both`; `Stepover`, `Scallop`, `ScallopElseStepover`, `OffsetStepoverElseSurface`, `AngularStep`, `SurfaceFollow`, `NoRadial` give `One`. One match, no wildcard |
| `OperationType::body_depth_limit()` | `compute/catalog.rs` | `depth_limit(spec.feeds_family, spec.feeds_pass_role, self.capabilities().cut_rule.radial.body_sides())`. THE one filler: it reads only the registry row, so a reader with the op kind and no config can call it |
| `label_facts` | `compute/catalog/cuts.rs:434` | `depth_rule: self.op_type().body_depth_limit()` |
| Gate readers | `tool_load/depth.rs:234-236` and `:273-276`, `tool_load/cut_force.rs:153-157` | `machine.rigidity.depth_cap_of(kind.body_depth_limit())` in the place of `depth_cap_rule(spec.feeds_family, spec.feeds_pass_role)` |
| `force_budget::rigidity_depth_cap` | `feeds/force_budget.rs:510-521` | takes `depth_limit: DepthLimit` in the place of `family, pass_role` |
| Envelope role guard | `feeds/envelope.rs:3221-3224` | delete `// EXEMPT(P28-02) if !matches!(role, PassRole::Roughing) { return None; }` and the then unused `role` parameter of `rigidity_limit`. Today the guard is redundant (a finish role already gives `NoLimit`); after the edit it would block the new arm |
| `RigidityProfile::depth_cap_rule(family, role)` | `machine/mod.rs:282` | **deleted in commit A**. With two inputs it cannot give the new answer (it does not know the body sides), and a door that gives the old answer would lie for six ops. No product reader calls it after the rows above. Its test callers re-point in commit A (6.4, group A1) |
| Groove reason (Q0, narrow choice) | `feeds/envelope.rs` `rigidity_limit` | a body cut with `Radial::Groove` pushes an `Unmodelled` `Rigidity` entry with a new true reason (for example `force_budget::GROOVE_WIDTH_AFTER_SIMULATION`, "a groove engages its whole cone: the width is known after the simulation") and returns, under `// EXEMPT(P28-03)`. Not in the probe run; see Q0 |

Which ops change: the registry rows with `RadialRule::Slot | Groove` are Profile, Trace, Bore,
Project Curve (slot) and VCarve, Inlay, InlayPlug, Chamfer (groove). Profile and Bore are
Roughing already. So the change reaches exactly the six ops of row `P28-03`: Trace, Project
Curve, VCarve, Inlay, InlayPlug, Chamfer. Pencil (Trace family, Finish) has
`OffsetStepoverElseSurface` and keeps `NoLimit`. No other op moves.

Why not the smaller edit `(OperationFamily::Trace, _) => ForceBudget`: it gives Pencil the
budget (R2 says a finish pass has none), and it is a label read again. Why not an edit in
`label_facts` only: the depth gate, the force card and `rigidity_depth_cap` read
`depth_cap_rule(family, role)` from the registry spec, not the cut list, so they would not
follow (the P29-3 "WAITS" readers). `OperationType::body_depth_limit` reads the registry row,
which the gate context holds, so every reader follows and the WAITS residue of the
`depth_cap_rule(` needle goes.

### 1.3 Each reader that follows, and why

| Reader | Path | After the step |
|---|---|---|
| Envelope (pre-sim depth) | `feeds/envelope.rs` `rigidity_limit` | A slot op (Trace, Project Curve) is an `AxialBar` at `ae = D`: the `Rigidity` limit is an `ApCeiling` at the budget depth at full width and the curve chip (the operator chip when the op states feed and RPM, else the grid chip at the smallest depth). A groove op (VCarve, Inlay, InlayPlug, Chamfer) gets an `Unmodelled` `Rigidity` entry with the reason "no force-budget limit below the flute": its envelope width is the calculator groove width (0.02 D: 0.127 mm on a 6.35 mm V-bit, 0.254 mm on 12.7 mm), and at that width no depth meets the budget (the run). Under Q0 narrow the product prints a true reason in its place. See finding F1 and Q0. |
| Suggest proposal | `SafeEnvelope::propose` (unchanged) | `min(stated, every ap ceiling)`. The new ops keep `propose`: the removal-rate pick needs `PassRole::Roughing` (`envelope.rs:1320`, `EXEMPT(P28-05, P28-06)`, unchanged). So a Trace proposal lowers a stated depth to the budget depth and never raises it (K2). Project Curve keeps `ProposedDepth::None(AfterSimulation)`; it gains a drawn edge only. |
| Depth gate (post-sim) | `tool_load/depth.rs` | `rigidity_depth_cap` at the peak-depth sample's measured width and gate chip: a `ForceBudget` cap with `BoundSource::MachineForceBudget` (non-gating), `Reported` when no root lies below the flute, `Unmodeled` with no force line. A V-bit sample reads the cone through `Engagement::from_sample`, so a groove op gets a number here. |
| Modulator | `session/compute.rs:2314-2316` (`cuts().body_facts().depth_rule`), `dressup/feed_modulation.rs` `ForceBudgetScope::of` | `ForceBudgetScope::Limit`: the force-budget feed cap runs on every move of the six ops, groove ops included (the move engagement reads the V section). |
| Force card, force field, chart contours | `tool_load/cut_force.rs` `budget_line` | `Some(35.3224)`: the card prints the budget line and a Within or Exceeds state in the place of "no force limit on this op family"; the viz force field and the sim chart contours draw (they read `budget_line`). |
| FM1 cap columns | `tests/feeds_matrix_instrument_fm1.rs:687-694` `cap_columns` | reads `op.cuts().body_facts().depth_rule` (test edit, signature change). `cap_cut_of_operation` gives `ae = D` for a slot and `None` for a groove, so only Trace and Project Curve rows print a cap. |
| `project.sim_outside_envelope` | `compute/simulate.rs` `envelope_placement` | can now fire on the six ops: each steady sample is placed against the drawn `Rigidity` edge. |
| `limits::project::of_envelope` / MCP `checks` | `limits/project.rs` | the `Rigidity` row of the new ops is a number (slot) or `Unmodelled` (groove), not absent. |

## 2. The traps of P29-4, for these ops

1. **The card and the proposal read two chips** (`placement_of` and `judged_ceiling` take
   `min(CurveChip, Pick)`, Pick = `max(stated chip, suggested chip)`; `propose` reads the
   stated chip). The code path is family-blind, so the new ops hit it when an op states a chip
   below the suggested chip. Not on FM1 (every cell has no RPM, so no operator chip and one
   chip). Not on wanaka: idx 10 and 13 sit Inside with margins 0.409 and 0.864 (computed);
   idx 5 and 9 have no stated depth to place. Reasoning, not a run: a slot op that states a
   low chip and a depth between the two budget depths shows "Outside Rigidity" on the card
   while the proposal keeps the depth. The fix waits for its own step, as in P29-4.
2. **`propose` checks ceilings, not the Runtime width floor.** Not reached, by the code:
   `runtime_limit` (`envelope.rs:3406`) returns `None` when the width axis is not free, and a
   slot op is an `AxialBar` (width fixed at D). Also, the six ops use
   `MoveCountModel::FeatureDriven`, and `predict::move_count_reads_depth` is `false` for it, so a
   lower depth adds no move to the budget model. A groove envelope is the same kind of bar.
3. **The step of the budget depth over the width.** A slot sits at one width (D), so a stepover
   change cannot cross the cliff. The cliff shows as a jump between two tools or two chips:
   Project Curve 3.175 mm flat in softwood (curve chip 0.250 mm, the probe's calculator chip)
   gives 4.0576 mm, Trace with the same tool and material (0.1016 mm) gives 10.1652 mm. The flat stretch of the force over the
   depth makes a 2.5x chip ratio a 2.5x depth ratio here.
4. **A slot has full width.** The budget is the largest peak force of the six printed hobby
   slot rows (`HOBBY_SLOT_POINTS`). Computed (run with the edit; table `SP`): at each printed
   chip and full width the budget depth is at or above the printed depth.

   | Printed row | D, Z | Printed depth (mm) | Peak at the printed depth (N) | Budget depth (mm) |
   |---|---|---|---|---|
   | sienci_quarter_softwood | 6.35, 2 | 5.40 | 30.25 | 6.48 |
   | sienci_quarter_mdf | 6.35, 2 | 5.40 | 35.32 (binding) | 5.3987 |
   | sienci_quarter_hardwood | 6.35, 2 | 2.50 | 24.67 | 3.63 |
   | carbide3d_s3_mdf | 6.35, 3 | 7.62 | 32.40 | 8.35 |
   | carbide3d_s3_pine | 6.35, 3 | 10.16 | 30.99 | 11.66 |
   | carbide3d_s3_mahogany | 6.35, 3 | 2.54 | 14.03 | 6.81 |

   No contradiction. The binding row solves 1.3 um under its printed depth: the step of the
   depth scan in `depth_at_budget`, not a model error. At the app's own chips the slot budget
   depths of the common tools are (FM1 rows, flat 2F): 6 mm softwood 6.0752, hardwood 3.4058,
   MDF 4.5584; 3.175 mm softwood 10.1652, hardwood 4.9846, MDF 7.5729. The 6 mm hardwood value
   (3.41 mm at 0.127 mm) is above the Sienci 1/4" hardwood row (2.5 mm at 0.114 mm), as the
   derivation requires.
5. **The modulator after a simulation.** With modulation on, the modulator lowers the feed of
   a move over the budget on the six ops. A load cap below the chip-min floor wins over the
   floor (P20g; the summary prints `LOAD_CAP_BELOW_CHIP_FLOOR_NOTE`). It reaches the wanaka ops
   of section 6.3. FM1 sim cells: none (the sim subset holds no slot or groove finish op).

## 3. Exemption rows

Rows that close in P29-5:

| Row | Class | Why it closes | Markers that go |
|---|---|---|---|
| `P28-03` (wide choice of Q0 only) | NeedsRuling | R3: a slot or groove cut reads the budget under every op | `machine/mod.rs:305` loses the id (`// EXEMPT(P28-02, S-NoBound)`) |
| `F-DepthRule` | NeedsRuling | the depth limit is no longer set from the family and the role. The family read left is the drill (row `P28-04`, physical); the role read left is a one-sided finish pass (row `P28-02`, ruled R2); the rest is geometry (`BodySides`). See Q1. | `machine/mod.rs:303`, `compute/catalog/cuts.rs:433`, `tool_load/cut_force.rs:156`, `feeds/force_budget.rs:520`; `tool_load/depth.rs:234` becomes `// EXEMPT(P28-02)` |

Rows that stay and change:

- `P28-03` (narrow choice of Q0, recommended): stays, narrowed. `who`: "groove cuts before the
  simulation (VCarve, Inlay, InlayPlug, Chamfer): the envelope reads no groove width"; `where_`:
  `src/feeds/envelope.rs` (the new marker; `machine/mod.rs:305` loses the id); `closes_in`:
  "P29-5c". The slot ops (Trace, Project Curve) leave it, and every op of the row reads the
  budget after the simulation.
- `P28-02`: `where_` loses `src/feeds/envelope.rs` (its only marker there goes with the guard).
- `P28-11` (Project Curve, every pre-sim limit that reads a depth): stays. The new edge reads no
  stated depth; the stated cut of a Project Curve is still not placed (its placement is
  `Inside { margin: 1.0 }` with no depth, today and after). Q5.
- `M11`, `P28-12`, `U-PerimeterSweep`: `closes_in` "P29-5b (filing)". `U-SpringPass`,
  `U-PocketZigzag`, `U-BoreWallFinish`, `U-RowConnector`: "P29-7". `U-ResidueCleanup`: "P29-6 /
  P29-7". `S-NotJudgedHere`: "P29-10". Text only; no pin moves for these (Q2).

New pins of `tests/the_exemption_list_only_shrinks_p29_3.rs`, by the choices of Q0 and Q1:

| Pin | Today | Q0 narrow, Q1 close (recommended) | Q0 wide, Q1 close | Q0 narrow, Q1 keep | Q0 wide, Q1 keep |
|---|---|---|---|---|---|
| `ROWS` | 64 | 63 | 62 | 64 | 63 |
| `PHYSICAL` | 8 | 8 | 8 | 8 | 8 |
| `RULED` | 3 | 3 | 3 | 3 | 3 |
| `MAX_NEEDS_RULING` | 53 | 52 | 51 | 53 | 52 |
| `ROWS_WITH_A_FILE` | 56 | 55 | 54 | 56 | 55 |
| `EXEMPTIONS: [Exemption; N]` | 64 | 63 | 62 | 64 | 63 |
| `CLOSED` | 3 entries | unchanged | adds `("P28-03", "P29-5")` | unchanged | adds `("P28-03", "P29-5")` |

## 4. Commits

Two commits. A moves the numbers. B moves no output.

**Commit A: the behaviour.**

- Section 1.2 in full; section 3 rows and pins; the `CLOSED` entry.
- New sentry `tests/a_slot_reads_the_force_budget_under_every_op_p29_5.rs`, three claims:
  1. Every registry row with `RadialRule::Slot | Groove` and a non-drill family has
     `body_depth_limit() == ForceBudget`; every row with `One` and a finish role has
     `NoLimit(FinishPass)` (the scan reaches Pencil and Trace both: non-vacuity).
  2. Trace, Profile and Project Curve with one tool, one material and one stated chip give the
     same `rigidity_depth_cap` at `ae = D` (bit-equal), and the same envelope `Rigidity`
     ceiling at D.
  3. `OperationConfig::cuts().body_facts().depth_rule == op_type().body_depth_limit()` for
     every op fixture (the config and the kind cannot disagree).
  Inject the defect once (drop the `Both` arm) and confirm red.
- Delete `depth_cap_rule(family, role)` (section 1.2) and re-point every test of section 6.4
  group A1.
- `every_op_declares_its_cuts_p29_3.rs:357-376`: the needle `depth_cap_rule(` goes with its
  three WAITS residue rows (`tool_load/depth.rs`, `tool_load/cut_force.rs`,
  `feeds/force_budget.rs`), in commit A, because the scan would find no reader.
- Re-bless FM1; the diff must equal `P29-5_predictions_fm1.csv` exactly (Q0 narrow; Q0 wide
  needs a new run first).
- Gates: fmt check; named sentries `the_exemption_list_only_shrinks_p29_3`,
  `every_limit_is_a_limit_id_p29_2`, `a_sample_states_its_cut_kind_p29_3`,
  `every_op_declares_its_cuts_p29_3`, the new P29-5 sentry,
  `the_roughing_depth_meets_the_force_budget_p13c`,
  `the_depth_that_cut_is_a_measured_load_g_s3depth`, `finish_depth_is_reported_not_capped_fm6`,
  `the_modulator_reads_the_depth_gate_force_budget_rule_p16`,
  `the_session_modulator_binds_on_the_force_budget_p16`,
  `the_cut_force_is_the_deflection_force_p17`, `the_envelope_keeps_each_limit_k2b`,
  `suggest_never_raises_a_stated_depth_k2`, `an_envelope_edge_does_not_move_with_the_stated_cut_p19`,
  `no_depth_is_proposed_that_cannot_be_modelled_p16`, `project_curve_plunges_at_the_axial_chip_q8`,
  `one_cut_gives_one_load_on_every_door_p29_1`, `the_hobby_force_budget_recomputes_from_the_printed_rows_p13c`;
  `cargo test -p rs_cam_core -q --lib`; FM1; full clippy; viz `--lib` and the viz tests
  `p17_visual_snapshots`, `the_sim_chart_seeks_on_a_dot_p17`,
  `the_envelope_plot_shows_only_what_limits_the_cut_p5c`, `every_limit_is_drawn_and_named_p5d`,
  `readiness_shows_the_limits_g_readylimits` (no viz test while `pgrep -ax rs_cam_gui` prints).

**Commit B: the card text (only if Q3 is taken).**

- The text `RULE_NO_LIMIT` ("no force limit on this op family") becomes "no force limit on this
  cut"; the viz uses at `sim_diagnostics.rs:2445`, `:2507` follow.
- No dead code is left for B: the label door goes in A (it cannot stay with a true answer).
- Proof: FM1 byte-equal to the CSVs of commit A, both files.
- Gates: as commit A.

## 5. Risks and questions for the lead

Most important first. Each question has my choice.

**Q0. Does `P28-03` close when four of its six ops get no number before the simulation?** The
row names Trace, VCarve, the Inlay ops, Chamfer and Project Curve. After the edit as run, the
slot ops (Trace, Project Curve) read the budget before and after the simulation. The groove ops
(VCarve, Inlay, InlayPlug, Chamfer) read it after the simulation only (depth gate, card,
modulator at the measured cone). Before the simulation the envelope reads a groove at the
calculator width 0.02 D (0.127 mm on a 6.35 mm V-bit), finds no root, and prints "no
force-budget limit below the flute". That reason is false: a V-bit in a groove engages its whole
cone at the depth, the section of a Trace with that V-bit at `ae = D`. FM1 prints no cap for a
groove (`cap_cut_of_operation` gives `None` for `Radial::Groove`).

- **Narrow (my choice).** Commit A as run, plus the true groove reason (section 1.2, last row).
  `P28-03` stays, narrowed to the groove ops before the simulation, and closes in a step
  "P29-5c" that gives `Radial::Groove` the slot width in `cap_cut_of_operation` and in the
  `Rigidity` width of the envelope, with its own predictions. Pins: column 3 of the table in
  section 3. FM1 and every number of section 6 stand; only the reason text of the 50 groove
  rows of table `S` changes (reasoning, not a run).
- **Wide.** The groove reads the slot width in commit A. `P28-03` closes. It moves FM1 cap cells
  of VCarve, Inlay, InlayPlug and Chamfer and the VCarve, Inlay and InlayPlug proposals (for
  example VCarve 6.35 mm V-bit hardwood at a stated 6.35 mm would fall to 5.0842 mm, by the
  Trace row; reasoning, not a run). It needs a new probe run before the predictions are
  committed (the edit script and the probe are kept in the scratchpad of this session).

**Q1. Close `F-DepthRule`?** After A the role still decides a one-sided cut (rough: budget;
finish: none). That read is the R2 ruling, which row `P28-02` already holds as `Ruled`; the
family read left is the drill (`P28-04`, physical). Choice: close `F-DepthRule`. The other
choice keeps it with `closes_in` "P29-7 (a depth limit from the leave)".

**Q2. The scope of P29-5.** Choice: the depth limit only (section 1.1). The filing fixes become
"P29-5b" with their own predictions (they move FM1 power and speed columns of Face, Adaptive,
3D Adaptive, Bore, VCarve and Inlay). The `U-*` rows go to P29-6 / P29-7; `S-NotJudgedHere`
to P29-10. The pre-sim deflection of a slot finish stays at P29-7 (row `P28-15`). The change
list row of P29-5 is rewritten to match.

**Q3. The card text.** `RULE_NO_LIMIT` says "no force limit on this op family". After A the
limit is a property of the cut. Choice: "no force limit on this cut" in commit B (a viz text
moves with it; `sim_diagnostics.rs:2445`, `:2507`).

**Q4.** Merged into Q0.

**Q5. Project Curve draws an edge it does not judge.** After A the envelope of a Project Curve
holds a `Rigidity` `ApCeiling` (wanaka idx 5: 2.6974 mm), but no proposal and no placement of
the stated cut (P28-11). After the simulation, `project.sim_outside_envelope` places each sample
against that edge, so idx 5 gains the finding. Choice: keep the edge (the simulation needs it,
and it tells the operator how deep the slot can go), keep `P28-11` for the stated cut.

**Q6. The exported feed of the six ops falls after a simulation.** As P29-4 Q3: the modulator
caps the feed of every move over the budget, and export writes the modulated feed. On wanaka
idx 5 (Project Curve 6 mm), P29-0 measured 92 % (operator) and 88 % (work) of the steady samples
over the budget, up to 3.61 x. Its feed will fall far, below the chip-min floor where the cap
binds (rub risk note). Choice: accept, as for the Pocket and Adaptive families; the lead tells
the operator before the cut. The real cure for idx 5 is its geometry (it cuts up to 14.3 mm
deep under standing stock; P29-0 section 5), not its feed.

**Q7. The chip gate's 95 % filter after the modulation.** P29-0 section 4 found that the chip
gate drops the body of an op whose stated feed is above the modulated feed. After A, idx 5's
modulated feed falls far below its stated feed, so its chip gate may judge almost no samples and
still print "Within". Choice: no action in P29-5 (row `M1`, P29-6); name it in the commit.

Risk: the hand check of the 12 mm Trace rows at a stated 24 mm. The calculator at 24 mm ships a
chip of 0.025 mm (it lowers the chip at that depth), and the force at the predicted ceiling at
that chip is 16.39 N (hardwood) and 13.30 N (MDF), not the budget. The envelope solves at the
grid chip of the smallest depth (0.1448 mm, 0.2438 mm), which gives 35.31 N and 35.29 N. The
envelope rule is the P13c rule, unchanged; the hand check that matches it uses the curve chip.

## 6. Predictions

Computed on HEAD `6d1db7e3b` with a throwaway edit (section 1.2, minus the docs) and a
throwaway test (`tests/zz_p29_5_probe.rs`, deleted). Both runs used the same probe: one on the
edited code ("after") and one on HEAD ("today"). Every file was restored from a byte copy and
compared (`cmp`), the FM1 fixtures by `sha256sum`; `git status --short crates/` is clean. Note
for the next probe: a restore with `cp -p` keeps the old mtime, so cargo does not rebuild; the
first "today" run read the stale edited build until the restored files were touched.

Which numbers come from a run: every number in 6.1, 6.2, the `SP` table and the pre-simulation
half of 6.3. Which come from reasoning: the post-simulation half of 6.3, the test list of 6.4
(except the FM1 diff and the compile reds that the edited build printed), the groove reason
text of Q0 narrow, and the Q0 VCarve example.

The post-simulation readers (the depth gate, `cut_force::budget_line`, the modulator scope) were
not exercised on the six ops: the FM1 sim subset holds none of them, and the probe did not
simulate. That they follow is from the code (section 1.3: each reads the body depth limit
through `depth_cap_of`), not from a run. The red or green of the `tool_load/depth.rs` lib test
`a_finishing_pass_reports_its_depth_with_no_bound` is not computed for the same reason.

Calibration before any new number was used:

- Project Curve 6 mm flat hardwood (chip 0.1492 mm, slot): predicted cap 3.0603 mm equals the
  committed Profile row (same tool, chip and width: 3.0603). Softwood 5.3893 = Profile 5.3893,
  MDF 5.1617 = Profile 5.1617.
- Hand checks, one per moved op family: the peak force at the predicted ceiling,
  `force_budget::peak_force_n(tool, material, ceiling, D, chip)`, against 35.3224 N:

  | Op family | Cell | Ceiling (mm) | Chip (mm) | Peak (N) |
  |---|---|---|---|---|
  | Trace, flat | 6 mm hardwood | 3.4056 | 0.1270 | 35.3058 |
  | Trace, V-bit | 6.35 mm hardwood | 5.0842 | 0.0635 | 35.3079 |
  | Project Curve, flat | 6 mm hardwood | 3.0574 | 0.1492 | 35.2965 |
  | Project Curve, V-bit | wanaka idx 9 (3.175 mm, white oak) | 5.5414 | 0.1270 | 35.3087 |
  | Trace, V-bit | wanaka idx 10 (6.35 mm, white oak) | 4.5107 | 0.0635 | 35.3060 |
  | Project Curve, flat | wanaka idx 5 (6 mm, white oak) | 2.6974 | 0.1492 | 35.2765 |

  Each is under the budget by the scan step (the solver returns the shallowest depth at or just
  under the crossing). The envelope ceiling (3.4056) and the FM1 cap (3.4058) of the 6 mm
  hardwood Trace differ in the fourth decimal: the envelope reads the grid chip at the smallest
  depth, FM1 the calculator chip at the op (`cap_cut_of_operation`); the Profile rows show the
  same pair today (proposal 3.0574, cap 3.0603).
- Groove ops have no hand check: they have no pre-sim number (Q0).

### 6.1 P1: FM1

**Pre-simulation file (`matrix_2026-09-23.csv`).** 55 rows move: 28 Trace rows and 27 Project
Curve rows. Two columns move: `cap_factor` (empty -> 35.3224) and `cap_mm` (empty -> the budget
depth). 110 cell values. 945 rows stay byte-equal, and every other column of the 55 rows stays
byte-equal: no proposal, binding, speeds, warning, diagnostic, `cap_family` or `cap_role`
column moves.

Why no proposal moves: every shipped FM1 Trace cell states a depth of 0.2646 to 1.0583 mm
(the add door), under every budget depth (the smallest is 3.0855 mm, BullNose and
TaperedBallNose 6 mm hardwood). The 8 refused Trace rows print cap columns too, as in P29-4. Project Curve proposes no depth
(`AfterSimulation`). The groove ops print no cap (`cap_cut_of_operation` gives `None` for a
groove) and their proposal does not bind on an `Unmodelled` entry.

The rows that do not move, of Trace and Project Curve: the 20 plywood rows (no force line),
and 5 rows with no root below the flute (Trace BallNose 3.175 softwood, Trace TaperedBallNose
3.175 softwood, Project Curve BallNose 3.175 softwood and MDF, Project Curve TaperedBallNose
3.175 softwood).

| Column | Rows | Example (committed -> predicted) |
|---|---|---|
| `cap_factor` | 28 Trace + 27 Project Curve | empty -> 35.3224 |
| `cap_mm` | 28 Trace + 27 Project Curve | Trace EndMill 6 mm hardwood: empty -> 3.4058; Project Curve EndMill 6 mm hardwood: empty -> 3.0603; Trace VBit 12.7 mm softwood: empty -> 8.5217 |

**Simulation file (`matrix_2026-09-23_sim.csv`).** 0 rows move (measured: in update mode the
sim test writes `MATRIX_SUMMARY.md`, then `gate` writes the CSV and panics by design, so the
"2 failed" of the re-bless is expected; the written sim CSV was byte-equal to the committed one,
and `MATRIX_SUMMARY.md` changed, which proves the write ran). The subset holds Pocket, Profile, Adaptive, Adaptive3d, Waterline,
DropCutter and Scallop only.

Counts: 55 FM1 rows move, 983 stay byte-equal (945 + 38). 110 cell values move.
`MATRIX_SUMMARY.md` is rewritten on the re-bless; it is not compared.

### 6.2 P2: Suggest on the default machine

`MachineProfile::generic_wood_router()`, the FM1 tool builder (`tool_of`: cutting length
`max(3 D, 12)`), 2 flutes, no RPM (so the curve chip), generic softwood, generic hardwood, MDF.
Ops: Trace and Project Curve with a flat end mill (3.175, 6, 12 mm) and a 60 degree V-bit
(6.35, 12.7 mm); VCarve, Inlay, InlayPlug, Chamfer with the V-bit. Stated depth 0.5 D, 1 D,
2 D (Project Curve: the offset; Chamfer: its default width). Full table, 134 rows that are not
refused: `P29-5_predictions.csv`, rows `S`.

24 proposals move, all Trace (binding -> `Rigidity`). Trace, flat end mill, proposal depth
(today -> after; "=" no change):

| D | Material | 0.5 D | 1 D | 2 D |
|---|---|---|---|---|
| 3.175 | softwood | = | = | 3.1750 = (VendorDepthTable) |
| 3.175 | hardwood | = | = | 6.3500 -> 4.9846 |
| 3.175 | MDF | = | = | 3.1750 = (VendorDepthTable) |
| 6 | softwood | = | = | 6.0000 = (VendorDepthTable) |
| 6 | hardwood | = | 6.0000 -> 3.4056 | 6.0000 -> 3.4056 |
| 6 | MDF | = | 6.0000 -> 4.5581 | 6.0000 -> 4.5581 |
| 12 | softwood | 6.0000 -> 5.1933 | 12.0000 -> 5.1933 | 12.0000 -> 5.1933 |
| 12 | hardwood | 6.0000 -> 3.0636 | 12.0000 -> 3.0636 | 12.0000 -> 3.0636 |
| 12 | MDF | 6.0000 -> 3.2225 | 12.0000 -> 3.2225 | 12.0000 -> 3.2225 |

Trace, 60 degree V-bit (MDF 6.35 mm refuses, no basis):

| D | Material | 0.5 D | 1 D | 2 D |
|---|---|---|---|---|
| 6.35 | softwood | = | = | 6.3500 = (VendorDepthTable) |
| 6.35 | hardwood | = | 6.3500 -> 5.0842 | 6.3500 -> 5.0842 |
| 12.7 | softwood | = | 12.7000 -> 8.5217 | 12.7000 -> 8.5217 |
| 12.7 | hardwood | 6.3500 -> 5.0858 | 12.7000 -> 5.0858 | 12.7000 -> 5.0858 |
| 12.7 | MDF | 6.3500 -> 6.2730 | 12.7000 -> 6.2730 | 21.1667 -> 6.2730 |

No proposal moves on Project Curve, VCarve, Inlay, InlayPlug or Chamfer. Project Curve gains a
drawn `Rigidity` `ApCeiling` (42 rows); the four groove ops gain an `Unmodelled` `Rigidity`
entry (50 rows).

### 6.3 P3: the two wanaka projects

**Before the simulation (exact, a run).** Read-only loads of
`planning/rivmap_block_job/nz_wanaka_21_operator.toml` and `nz_wanaka_21_6mm_rough_work.toml`;
no save. Every proposal, binding and placement of all 36 ops is byte-equal today and after.
What changes: the envelope of idx 5, 9, 10, 13 (both files) gains a `Rigidity` edge.

| File | idx | Op, tool | Stated | Proposal (today = after) | Rigidity ceiling after | Card placement (today = after) |
|---|---|---|---|---|---|---|
| both | 5 | Project Curve, 6 mm 2F flat | offset 4.0, slot 6.0 | none (after simulation) | 2.6974 (work 2.6972) | Inside, no depth placed |
| both | 9 | Project Curve, 3.175 mm 20 deg V-bit | offset 2.418, slot 3.175 | none (after simulation) | 5.5414 | Inside, no depth placed |
| both | 10 | Trace, 6.35 mm 60 deg V-bit | 2.6 x 6.35 | 2.6000 | 4.5107 | Inside, margin 0.409 |
| both | 13 | Trace, 6.35 mm 60 deg V-bit | 0.6 x 6.35 | 0.6000 | 4.5107 | Inside, margin 0.864 |

These ceilings equal the budget depths of `P29-0_probes.md` section 2.2 (2.70, 5.54, 4.51,
4.51).

**After the simulation (reasoning, from `P29-0_probes.md` section 5; no simulation run).**

- The depth gate, the force card and the modulator read the budget on idx 5, 9, 10, 13.
- Modulator binds `ForceBudget`: idx 5 on most moves (both files); idx 10 on some moves (1 144
  of 3 492 samples over, 1.02 x); operator idx 9 on the moves of at most 9 samples. No bind
  predicted on work idx 9 (max 0.53 x) and idx 13 (max 0.13 x), unless the emitted feed is far
  above the reached feed (the P29-4 caveat: the cap acts on the emitted feed).
- The counts of samples over the budget are at most the P29-0 counts (operator 20 973, 9, 1 144,
  0; work 19 875, 0, 1 144, 0): the step changes no geometry, and a new cap only lowers a feed.
- `project.sim_outside_envelope`: predicted on idx 5 (steady depth p95 5.7 mm against a 2.70 mm
  edge) on both files. Not predicted on idx 10 (its depth 4.60 mm is above 4.51 mm, but the
  sample's arc width and the drawn chip decide, and the modulator lowers the chip) or idx 9.
- Not predicted: the depth verdict of idx 5 and 10 (the gate solves at the modulated chip of the
  peak-depth sample), the exported feeds, the exact counts.

### 6.4 P4: tests that turn red

**A1. Commit A, red by construction (each asserts that a slot finish has no depth limit):**

- `tests/feeds_matrix_instrument_fm1.rs`: the matrix golden (re-bless; 6.1). The sim golden
  stays green. Also a compile edit (`cap_columns` reads the cut's depth limit).
- `tests/the_exemption_list_only_shrinks_p29_3.rs`: the five pins, the array length, `CLOSED`.
- `tests/every_op_declares_its_cuts_p29_3.rs`: `claim_2_the_facts_equal_the_label_functions`
  (its written-out table `old_facts`, line 216, gains the `BodySides` arm).
- `tests/the_depth_that_cut_is_a_measured_load_g_s3depth.rs`: the test at lines 391-410 asserts
  `depth_cap_rule(Trace, Finish).is_none()` and a `Reported` Trace verdict with no bound. New
  claim: a one-sided finish (Scallop or Waterline) reports with no bound; a Trace reads the
  budget.
- `tests/the_modulator_reads_the_depth_gate_force_budget_rule_p16.rs`: line 140 uses
  `(Trace, Finish)` as "the finish pass with no rule". New claim: the same with a one-sided
  finish op; add that a Trace reads the budget.
- `src/tool_load/depth.rs` lib test `a_finishing_pass_reports_its_depth_with_no_bound` (line
  736 runs `OperationType::Trace`). Red or green by the sample's root (not computed); either
  way the fixture moves to a one-sided finish op so the claim keeps its meaning.
- Compile reds from the `rigidity_depth_cap` signature (the four fm6 sites measured with the
  edited build; the rest by `rg`): `tests/finish_depth_is_reported_not_capped_fm6.rs:370, 459,
  500` (`the_gate_reads_the_cone_diameter_fm6` and the tapered-ball tests near them);
  `tests/the_depth_that_cut_is_a_measured_load_g_s3depth.rs:285, 794`;
  `src/tool_load/depth.rs:475, 534` (lib tests); `rs_cam_viz/tests/limits_fixture/mod.rs:142`.
- Compile reds from the deletion of `depth_cap_rule` (by `rg`):
  `tests/finish_depth_is_reported_not_capped_fm6.rs:303`
  (`a_finishing_depth_ships_above_the_old_cap_with_no_clamp_fm6`);
  `tests/the_depth_that_cut_is_a_measured_load_g_s3depth.rs:395, 844`;
  `tests/the_cut_force_is_the_deflection_force_p17.rs:259, 267`
  (`the_budget_line_follows_the_depth_cap_rule`);
  `tests/the_modulator_reads_the_depth_gate_force_budget_rule_p16.rs:134, 137, 140`
  (`the_modulator_reads_the_depth_gate_force_budget_rule_p16`);
  `tests/the_session_modulator_binds_on_the_force_budget_p16.rs:131`;
  `rs_cam_viz/tests/p17_visual_snapshots.rs:312`;
  `rs_cam_viz/tests/the_sim_chart_seeks_on_a_dot_p17.rs:66`.
  The behaviour of each stays (each names a roughing family, a drill or a one-sided finish),
  except the two behaviour reds above (g_s3depth 391-410, p16 line 140).

**A2. Commit A, at risk (not run; a red here is no mismatch, the implementer fixes and names
it):** `rs_cam_viz/src/ui/properties/pills.rs:622`
(`vcarve_max_depth_falls_back_to_the_raw_value_labelled_not_clamped`: a VCarve envelope gains an
`Unmodelled` `Rigidity` entry); any viz snapshot of a Trace, Project Curve or groove envelope or
force card; `the_envelope_plot_shows_only_what_limits_the_cut_p5c` and
`every_limit_is_drawn_and_named_p5d` if they hold such a fixture. Outside the gate set: every
integration test that simulates a Trace-family slot or groove op with modulation on in a
material with a force line and pins a feed, a time or G-code (for example
`p1_headless_ab_wanaka`, `swept_wanaka_ab_s1`, `wanaka_e2e_chipload_gate`,
`wanaka_suggest_integration`, if their fixture holds such an op; not checked).

**B. Commit B (Q3):** a viz test that pins the text "no force limit on this op family", if one
exists (`rg` finds the constant only in `sim_diagnostics.rs`).

Search behind this list: `rg` over core and viz `src` and `tests` for the six op names, for
`depth_cap_rule(`, `rigidity_depth_cap(`, and for every file that loops over ops
(`fixture_op(`, `OperationType::ALL`, `PassRole::Finish`) and names `Rigidity`, `budget_line`,
`NoBound`, `RULE_NO_LIMIT`, `depth_rule` or `DepthLimit`. The loop files found
(`an_envelope_edge_does_not_move_with_the_stated_cut_p19`, `the_envelope_keeps_each_limit_k2b`,
`every_limit_is_a_limit_id_p29_2`, `a_pick_outside_the_envelope_warns_k2b`, the p13a pair,
`the_envelope_plot_shows_only_what_limits_the_cut_p5c`) use Pocket, Adaptive or Adaptive3d
fixtures only.

"Nothing else moves": in the gate set, a red outside A1, A2 and B is a mismatch.

### 6.5 P5: what stays byte-identical

- Every op that is not one of the six (Suggest, envelope, FM1 945 + 38 rows, gates, modulator,
  cards, MCP, CLI). Pencil included.
- The toolpath geometry of every op: no generator reads the depth limit.
- The G-code of every op from a generation with no simulation after it.
- Not identical after a simulation with modulation on: the feeds of the six ops on moves over the
  budget (Q6). Wanaka: idx 5 (both), idx 10 (both), operator idx 9.
- Commit B: every output byte-identical; one card text changes if Q3 is taken.
- Under Q0 narrow, the reason text of the groove `Rigidity` entry differs from the run
  ("no force-budget limit below the flute" in the run; the new true reason in the product).

## 7. Docs to change (commit A unless named)

- `FEATURE_CATALOG.md` line 72: "A finishing or semi-finishing pass has no axial depth ceiling
  (ruling R2)" gains "unless the cut is a slot or a groove (ruling R3, P29-5: Trace, Project
  Curve, V-Carve, Inlay, Inlay plug, Chamfer read the force budget; a groove gets its number
  after the simulation)"; the P13c sentence "The roughing depth of the Pocket family ... and of
  Profile" gains the six ops; the modulator sentence of P29-4 covers them. Line 206 (the force
  card): "against the machine force budget ... only under the `ForceBudget` depth-cap rule, else
  'no limit'" stays true; the caption text changes with Q3 (commit B).
- `crates/rs_cam_core/src/machine/CLAUDE.md`: the `depth_cap_rule` sentence (the rule reads the
  body sides; `OperationType::body_depth_limit` is the one filler).
- `crates/rs_cam_core/src/feeds/CLAUDE.md`: the P13c invariant (a slot or groove reads the budget
  under every role, R3).
- `crates/rs_cam_core/src/tool_load/CLAUDE.md`: the g_s3depth sentry text and the "budget line
  exists only where the op has a `DepthCapRule`" line (now: where the body cut has the budget).
- `crates/rs_cam_core/src/limits/CLAUDE.md`: `BodySides` in the file map of `cut.rs`; the open
  item "Commit B ... the gate readers wait" loses the depth readers.
- `planning/feeds_contract_2026-11-03/phases/P29_FINDINGS.md` section 6, the P29-5 row (Q2).

## 8. Findings outside the scope

- **F1** (Q0): a groove has no pre-sim force number; the envelope prints a false reason ("no
  force-budget limit below the flute") because it reads the groove at the calculator width
  0.02 D, and FM1 prints no cap for a groove.
- **F2**: Project Curve 3.175 mm flat reads a different vendor row from Trace (row M6): chip
  0.3048 mm softwood, 0.0227 mm hardwood, 0.2794 mm MDF (FM1 column `r_chip_load_mm`; the
  envelope curve chip of the probe is 0.250 mm softwood, the calculator chip at the op), a 13x spread
  between softwood and hardwood, against Trace 0.1016 / 0.0889 / 0.1270. The budget depth
  follows the chip (Project Curve softwood 4.0576 mm against Trace 10.1652 mm, hardwood 10.1466
  against 4.9846). The row choice is suspect, not the budget.
- **F3**: the binding printed slot row (Sienci 1/4" MDF) solves at 5.3987 mm against its
  printed 5.40 mm: the scan step of `depth_at_budget`. Harmless; name it if a sentry ever pins
  "budget depth >= printed depth" exactly.
- **F4**: the FM1 cap and the envelope ceiling of one cell read two chips (calculator chip at
  the op, grid chip at the smallest depth), so they differ in the third or fourth decimal
  (Profile 6 mm hardwood: 3.0603 against 3.0574). Old, now on six more ops.
- **F6**: the modulator, the depth gate and the card read the budget on groove ops after the
  simulation, while the envelope before it gives no number (Q0). Under Q0 narrow this split is
  named by row `P28-03` until P29-5c.
- **F5** (probe hygiene): `cp -p` on restore keeps the old mtime; cargo then keeps the edited
  build. Touch restored sources before a "today" run.
