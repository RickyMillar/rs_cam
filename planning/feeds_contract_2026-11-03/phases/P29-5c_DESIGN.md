# P29-5c design: a groove reads the slot width before the simulation

Written 2026-11-12 on HEAD `8413af726`. No product code changed. The lead reviews this note,
commits it with the two prediction files, and then starts an implementer. The result is judged
against section 8. A mismatch stops the work.

Files of this step:

- this note;
- `P29-5c_predictions_fm1.csv`: every FM1 cell value that moves. 162 rows, all in the matrix
  file. A cell value that is not in this file must not move.
- `P29-5c_predictions.csv`: table `S` (Suggest on the default machine: VCarve, Inlay, InlayPlug
  and Chamfer with a 60 degree V-bit, 6.35 and 12.7 mm, three materials, three stated depths),
  table `W` (the wanaka files), table `F` (the FM1 simulation file), table `H` (hand checks).

Order assumed: P29-5b (option A), then this step, then P29-7. The predictions stand on the tree
after 5b: the run applied the 5b edit first, and "today" in table `S` is the 5b tree. 5b moves no
groove number (the V section reads no width), so "today" equals HEAD for every groove cell.

Rulings in force: R3 (operator, 2026-11-11: "slot is a slot! it can vary in z depth due to rest
and similar though"; before the simulation the limit reads the stated depth), R1 (the force
budget, 35.3224 N; tuning parked by the operator on 2026-11-12). Every finding warns.

## 1. Scope

### 1.1 The sources

| Source | What it puts in P29-5c |
|---|---|
| Change list | "A groove reads the slot width before the simulation (`cap_cut_of_operation`, the envelope Rigidity width): row `P28-03`. Moves groove caps and proposals." |
| Row `P28-03` | "groove cuts before the simulation (VCarve, Inlay, InlayPlug, Chamfer): the envelope reads no groove width", marker in `src/feeds/envelope.rs`, `closes_in` "P29-5c" |
| P29-5 Q0 | the wide choice, deferred: the groove reads the slot width in `cap_cut_of_operation` and in the `Rigidity` width of the envelope |

The disagreement: **Chamfer is not a groove of the same kind.** Its envelope depth axis is the
chamfer WIDTH (`axes_plan`: `Axial::ChamferWidth(s) => (true, Some(s.mm()), s.mm())`), and its
cut has air beyond the edge on one side. A depth ceiling drawn on that axis would be compared with
a width. Row `P28-32` already names "a chamfer width gives no depth" (closes in P29-9).

**Proposed scope:** VCarve, Inlay and InlayPlug read the slot width (the full cone at the depth)
in the envelope `Rigidity` limit and in `cap_cut_of_operation`. Chamfer moves from `P28-03` to
`P28-32` with a true reason. Row `P28-03` closes. The envelope radial axis of a groove stays at
the calculator width (no speeds, power or other limit moves): only the force-budget solve reads
the slot width (Q1).

### 1.2 Why the slot width is the groove width

A V-bit in a groove engages its cone on both sides up to the depth. `Engagement::from_tool_cut`
clamps the width at the engaged diameter of the cone (`EngagedDiameter`), so `ae = D` on a V-bit
is the full cone at the depth, the same section a Trace with that V-bit reads (P29-5 Q0). The
calibration in section 8 confirms it: every groove ceiling equals the Trace ceiling of the same
tool, material and chip.

## 2. The behaviour change, as the smallest edit

1. `compute/catalog/cuts.rs` `declare_side_cuts`: the body width of `Radial::Groove` is
   `DeclaredWidth::Slot` (today `None(NoStatedWidth)`). A groove engages its whole cone; the
   engagement clamps at the cone (`EngagedDiameter::of_hint`). No reader of the body width
   exists today except the two below, so this line moves nothing alone.
2. `feeds/envelope.rs` `rigidity_limit`: the solve width of a column is the body's declared width
   when the envelope width is fixed (`AxialBar`, `Fixed`), else the column width. One helper,
   `fn budget_width(&OwnedInputs, ae_column) -> f64`, read by `rigidity_limit` (both solves) and
   by `budget_depth_at` (the exact and the pick solves). For a slot op the declared width is D,
   the axis width is D: bit-equal. For a groove the axis is the calculator width 0.02 D and the
   solve reads D.
3. `feeds/envelope.rs`: the `// EXEMPT(P28-03)` block goes. In its place, an arm on the axial
   cut: `Axial::ChamferWidth(_)` pushes an `Unmodelled` `Rigidity` entry with the clause
   `force_budget::CHAMFER_STATES_NO_DEPTH` ("a chamfer states its width, not a depth: the force
   budget is judged after the simulation") under `// EXEMPT(P28-32)`. The constant
   `GROOVE_WIDTH_AFTER_SIMULATION` is deleted (no reader).
4. `feeds/force_budget.rs` `cap_cut_of_operation`: the width is the body's declared width
   (`op.cuts()` body: `Stated(s)` gives `s`, `Slot` gives D, `None` gives no cap), and a Chamfer
   gives no cap (`Axial::ChamferWidth`, `// EXEMPT(P28-32)`). This replaces the match on
   `Radial` with the declared cut, the P29-7 reader shape. One arm stays as today: a drill
   (`Radial::Axial`, body width `None`) keeps the width D, because
   `rs_cam_viz/tests/limits_fixture/mod.rs:134` calls the function too; the drill has no depth
   limit, so its cap stays `None` in every reader.

The readers that follow: Suggest's proposal (`propose`: the `Rigidity` ceiling now has a number
on a groove, so a stated depth above it is lowered, K2), the card placement, the FM1 cap columns,
`limits::project::of_envelope` (the `Rigidity` row of a groove is a number, not `Unmodelled`),
`project.sim_outside_envelope` after a simulation (a groove sample is placed against the drawn
edge).

Not changed: the depth gate, the force card and the modulator. They already read the measured
cone after the simulation (P29-5).

## 3. Commits

One commit.

| Commit | Content | Moves (FM1) | Rows |
|---|---|---|---|
| 5c | section 2 | 162 cells in 81 rows: `cap_factor` 81 (empty -> 35.3224), `cap_mm` 81 (empty -> the budget depth); VCarve 27 rows, Inlay 27, InlayPlug 27 | `P28-03` closes; `P28-32` gains `ForceBudget` and two files |

Gates: fmt check; named sentries `the_exemption_list_only_shrinks_p29_3`,
`every_limit_is_a_limit_id_p29_2`, `every_op_declares_its_cuts_p29_3`,
`a_slot_reads_the_force_budget_under_every_op_p29_5`, `the_roughing_depth_meets_the_force_budget_p13c`,
`the_envelope_keeps_each_limit_k2b`, `suggest_never_raises_a_stated_depth_k2`,
`an_envelope_edge_does_not_move_with_the_stated_cut_p19`, `no_depth_is_proposed_that_cannot_be_modelled_p16`,
`a_vbit_row_is_read_at_its_printed_key_b4`, `one_cut_gives_one_load_on_every_door_p29_1`;
`cargo test -p rs_cam_core -q --lib`; FM1; full clippy; viz `--lib`, `pill_writes_clamped_value_g_pillclamp`
(core), and the viz tests `the_envelope_plot_shows_only_what_limits_the_cut_p5c`,
`every_limit_is_drawn_and_named_p5d`, `p17_visual_snapshots` (no viz test while
`pgrep -ax rs_cam_gui` prints).

## 4. Exemption rows and pins

Pins after 5b (option A): ROWS 49, PHYSICAL 8, RULED 3, MAX_NEEDS_RULING 38, ROWS_WITH_A_FILE 42.

| After | Rows closed | `ROWS` | `MAX_NEEDS_RULING` | `ROWS_WITH_A_FILE` | `CLOSED` gains |
|---|---|---|---|---|---|
| 5c | `P28-03` | 48 | 37 | 41 | `("P28-03", "P29-5c")` |

(After 5b option B the same step gives 47, 36, 40.)

Rows that stay and change:

- `P28-32`: `limit` gains `LimitId::ForceBudget`; `who` "a chamfer width and a peck give no
  depth" (5b-2 took the helix pitch out); `where_` adds `src/feeds/envelope.rs` and
  `src/feeds/force_budget.rs` (the two new markers); `closes_in` "P29-9".

## 5. Tests that go red

- `tests/feeds_matrix_instrument_fm1.rs`: the matrix golden (162 cells). The sim golden stays.
- `tests/the_exemption_list_only_shrinks_p29_3.rs`: the pins, the array length, `CLOSED`.
- At risk (not run): `rs_cam_viz/src/ui/properties/pills.rs:622`
  `vcarve_max_depth_falls_back_to_the_raw_value_labelled_not_clamped` (a VCarve envelope now holds
  a `Rigidity` ceiling, so a large max depth can clamp); `tests/pill_writes_clamped_value_g_pillclamp.rs`
  if it holds a groove op; any viz snapshot of a VCarve or Inlay envelope; MCP `get_safe_envelope`
  replies of a groove op in a wire snapshot (the `Rigidity` row changes from `unmodelled` to a
  ceiling).
- Not red: `a_slot_reads_the_force_budget_under_every_op_p29_5` (its claims read the depth limit
  and the slot ops); add one claim: a groove and a Trace with one V-bit, one material and one chip
  give the same envelope `Rigidity` ceiling (bit-equal).

## 6. Docs to change

- `FEATURE_CATALOG.md`: the P29-5 sentence "a groove gets its number after the simulation" becomes
  "a groove reads the full cone at the depth before the simulation (P29-5c); a chamfer is judged
  after it".
- `crates/rs_cam_core/src/feeds/CLAUDE.md`: the P29-5 invariant (the envelope gives a groove an
  `Unmodelled` row until P29-5c) becomes the slot-width rule; the Chamfer reason.
- `crates/rs_cam_core/src/compute/CLAUDE.md`: no change.
- `P29_FINDINGS.md` section 6: the P29-5c row (Chamfer to `P28-32`).

## 7. Risks and questions for the lead

**Q1. Which width moves: only the force-budget solve, or the whole radial axis?** My choice: only
the solve (section 2). The other choice fixes the groove axis at D in `axes_plan`. That moves the
speeds grid to `ae = D` and the printed proposal width (`proposal_ae_mm` of 81 groove rows would
read D in place of 0.02 D), and an apply could write that width into the VCarve row stepover
(`VCarveConfig::stepover` is a row spacing, not a cut width). Not run.

**Q2. Chamfer.** My choice: `P28-32` (no depth axis), with a true clause. Finding F1: a chamfer has
air beyond the edge, so `RadialRule::Groove` (both sides) may be the wrong filing for it; a
one-sided filing would take the force budget off a Chamfer after the simulation too (R2). That is
a ruling for later, not this step.

**Q3. The VCarve proposal binds `VendorAxial` today on some cells.** After 5c the lower of
`VendorAxial` and `Rigidity` binds (for example VCarve 6.35 mm hardwood at a stated 12.7 mm:
`VendorAxial` 6.3500 today, `Rigidity` 5.0842 after). No choice needed; named for the reviewer.

Risks:

- An apply of a groove proposal writes the lower depth into `VCarveConfig::max_depth`,
  `InlayConfig::pocket_depth` or `InlayPlugConfig::depth_per_pass`. A VCarve with a capped max
  depth cuts a flat floor where the V would go deeper: the toolpath changes on an apply, not
  before. That is K2 behaviour (the proposal never raises).
- The trap of P29-4 (the card and the proposal read two chips) reaches the groove ops: a groove
  op that states a chip below the suggested chip can show "Outside Rigidity" while the proposal
  keeps the depth. No FM1 or wanaka cell states such a chip. P29-7 decides the fix.

## 8. Predictions

Computed on HEAD `8413af726` plus the 5b throwaway edit, then the 5c throwaway edit (section 2 in
a shorter form: a groove solve width of D inside `rigidity_limit` and `budget_depth_at`, the
Chamfer arm, the `cap_cut_of_operation` arm). A throwaway probe test printed table `S` on the 5b
tree and on the 5b+5c tree. Every file was restored from a byte copy (`sha256sum -c` OK, then
touched); the probe file was deleted; `git status --short crates/` is clean; the FM1 fixtures hash
as before. FM1 ran without the update variable and wrote its walk copies to the scratchpad.

Which numbers come from a run: every FM1 cell, every row of table `S`, the hand checks. Which
come from reasoning: the test list (except the FM1 diff), the product form of section 2 (the
declared width in place of the radial match: the same width, D, on every groove cell).

Calibration: the groove ceilings equal the P29-5 Trace predictions of the same tool and material
(6.35 mm hardwood 5.0842, 12.7 mm softwood 8.5217, 12.7 mm hardwood 5.0858, 12.7 mm MDF 6.2730),
and the probe's Trace rows on the same run print the same values.

### 8.1 FM1

Matrix file: 81 rows, 162 cells, two columns: `cap_factor` (empty -> 35.3224) and `cap_mm`
(empty -> the budget depth). VCarve, Inlay and InlayPlug, 27 rows each: every tool type and
diameter in softwood, hardwood and MDF, where a force line and a root below the flute exist (the
rows of refused cells print cap columns too, as in P29-5). Examples: VBit 6.35 mm hardwood
5.0842 (the Trace value), EndMill 6 mm hardwood 5.8797, BullNose 3.175 mm hardwood 4.5321. Chamfer:
0 cells. No proposal, binding or speeds column moves: every FM1 groove cell states a depth under
its ceiling.

Simulation file: 0 cells (measured; the subset holds no groove op).

### 8.2 Suggest on the default machine (table `S`)

60 degree V-bit, FM1 tool builder, no RPM (curve chip), generic softwood, generic hardwood, MDF;
VCarve, Inlay, InlayPlug at a stated depth of 0.5 D, 1 D and 2 D; Chamfer at its default width.
32 proposals move, all to `Rigidity`:

| D (mm) | Material | Ceiling (mm) | Ops and stated depths that move |
|---|---|---|---|
| 6.35 | softwood | 9.6183 | Inlay and InlayPlug at 12.7 |
| 6.35 | hardwood | 5.0842 | VCarve, Inlay, InlayPlug at 6.35 and 12.7 |
| 12.7 | softwood | 8.5217 | VCarve, Inlay, InlayPlug at 12.7 and 25.4 |
| 12.7 | hardwood | 5.0858 | every op at 6.35, 12.7 and 25.4 |
| 12.7 | MDF | 6.2730 | every op at 6.35, 12.7 and 25.4 |

6.35 mm MDF refuses today and after (no basis). Chamfer keeps its proposal (1.0000 x the
calculator width) and gains the true clause in place of the groove clause. VCarve at 6.35 mm
softwood, 12.7 mm: `VendorAxial` binds at 6.3500 today and after (under 9.6183).

### 8.3 The wanaka files (table `W`)

No op of either file is a groove op (the V-bit ops are Trace and Project Curve, slots since
P29-5). Run: every op of both files equal on the 5b and the 5b+5c trees.

### 8.4 Hand checks (table `H`)

The peak force at the predicted ceiling, `force_budget::peak_force_n(tool, material, ceiling, D,
chip)`: VCarve 6.35 mm hardwood at 5.0842 mm and 0.0635 mm gives 35.3079 N; VCarve 12.7 mm MDF at
6.2730 mm and 0.083210 mm gives 35.3198 N; each under 35.3224 N by the scan step, and over it
0.01 mm deeper (35.3773 N, 35.3761 N).

### 8.5 What stays byte-identical

- Every op that is not VCarve, Inlay or InlayPlug (Chamfer included: only its clause text).
- Every toolpath and G-code from a generation with no apply.
- Every post-simulation number.
- The envelope radial axis, speeds grid and every other limit of a groove op.

## 9. What changes for the operator's cut

Nothing. The wanaka files hold no VCarve, Inlay or Inlay plug op. For a future job: a V-carve or
an inlay pocket that states a depth deeper than the machine budget allows now gets a lower depth
proposal before the simulation, the same number a Trace with that V-bit gets. Every value
depends on the 35.3 N budget, which the operator parked for a later tune.

## 10. Findings outside the scope

- **F1.** Chamfer is filed `RadialRule::Groove` (both sides). A chamfer has air beyond the edge.
  The P29-5 force budget on a Chamfer after the simulation may judge a one-sided cut as a slot.
- **F2.** `VCarveConfig::stepover` (the row spacing) reaches no limit: the V section reads no
  width, and the rows overlap. Not a defect for the force (the full cone is the worst case).

## 11. Lead rulings (2026-11-12)

- **Q1: only the force-budget solve reads the slot width.** The radial axis does not move.
- **Q2: Chamfer moves to `P28-32`.** The filing of a Chamfer as a groove (F1) is a later ruling.
- **Q3: noted.**
