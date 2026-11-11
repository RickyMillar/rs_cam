# P29-4 design: the Adaptive family reads the force budget

Written 2026-11-11 on HEAD `34e678d5f`. No product code changed. The lead reviews this note,
commits it with the two prediction files, and then starts an implementer. The implementer's
result is judged against section 6. A mismatch stops the work.

Files of this step:

- this note;
- `P29-4_predictions_fm1.csv`: every FM1 cell value that moves (long form: file, cell key,
  column, committed, predicted). 220 rows. A cell value that is not in this file must not move.
- `P29-4_predictions.csv`: the Suggest proposals of P2 and the wanaka proposals of P3.

Rulings in force: R1 (the force budget, 35.3224 N on the default machine, sets the depth of an
Adaptive rough; accepted 2026-11-11). R2 (lead default: Suggest keeps a stated stepover and
caps the depth; with no stepover it proposes the best pair inside the limits, with a width
ceiling of 0.5 D). No job is refused: every finding warns. No legacy: a rule that no reader
reads after the step is deleted, and the commit names the break.

## 1. Scope

### 1.1 The one behaviour change

`RigidityProfile::depth_limit` (`crates/rs_cam_core/src/machine/mod.rs:305`) is the one table
of the depth limit. Every reader reaches it, through `depth_cap_rule(family, role)` or through
`depth_cap_of(cuts().body_facts().depth_rule)`. Delete its arm:

```rust
// EXEMPT(P28-01, F-DepthRule)
(OperationFamily::Adaptive, _) => DepthLimit::Rule(DepthRule::Factor),
```

After the deletion, (Adaptive, Roughing) falls to `(_, PassRole::Roughing) => ForceBudget`.
(Adaptive, SemiFinish | Finish) falls to `FinishPass`. No operation has those two pairs today
(the registry files Adaptive and 3D Adaptive as Roughing), so they change nothing.

Each reader then reads the force budget for an Adaptive op with no code change of its own:

| Reader | Path, symbol | What it reads after the step |
|---|---|---|
| Pre-simulation depth (envelope) | `feeds/envelope.rs` `rigidity_limit` | the `ForceBudget` arm: an `ApCurve` over the width at the curve chip (the operator chip when the op states a feed and an RPM, else `chip_of(calc_at(0.05 mm, ae))`). `propose` lowers the stated depth to the exact budget depth at the stated width (`judged_ceiling`, `ExactRole::CurveChip`). |
| Suggest proposal | `SafeEnvelope::propose` (unchanged) | `min(stated or default depth, every ap ceiling)`. The Adaptive family keeps `propose`; it does not take `propose_removal_rate` (the condition at `envelope.rs:1320` is unchanged by this step). |
| Post-simulation depth gate | `tool_load/depth.rs` | `rigidity_depth_cap` at the peak-depth sample's width and gate chip: a `ForceBudget` cap, `BoundSource::MachineForceBudget`, `Reported` when the budget has no root below the flute, `Unmodeled` with no force line. |
| Modulator | `session/compute.rs:2314`, `dressup/feed_modulation.rs` `ForceBudgetScope::of` | `ForceBudgetScope::Limit`: the force-budget feed cap now runs on Adaptive moves. |
| Force card, force field, chart contours | `tool_load/cut_force.rs` `budget_line` | `Some(budget_n)`: the card prints the budget line and a Within or Exceeds state; the viz force field and the sim chart contours draw (they read `budget_line`; no viz code names the rule). |
| FM1 cap columns | `force_budget::rigidity_depth_cap` | the `ForceBudget` arm. |

### 1.2 What goes, and why

After 1.1 no product path produces the Factor rule. Each item below has no producer, so it is
deleted (operator rule: no legacy).

| Item | Path | Break |
|---|---|---|
| `DepthRule::Factor`, and then `DepthRule` itself (one member tells nothing) | `limits/mod.rs:224` | none on a wire (`Limit::depth_rule` is `#[serde(skip)]`). See Q4. |
| `DepthLimit::Rule(DepthRule)` becomes `DepthLimit::ForceBudget` | `limits/cut.rs:218` | none |
| `LimitId::DepthFactor` (29 members become 28) | `limits/mod.rs:60, 115` | MCP wire: the `Check.limit` id `depth_factor` goes |
| `DepthCapRule::Factor`; `DepthCapRule` keeps one member, see Q4 | `machine/mod.rs:268` | none |
| `RigidityDepthCap::Factor` | `machine/mod.rs:227` | MCP wire: a depth verdict bound of `kind = "factor"` goes |
| `BoundSource::RigidityRuleOfThumb` (only producer: `verdict.rs:2401`) | `tool_load/verdict.rs:1098` | MCP wire: `bound_source.kind = "rigidity_rule_of_thumb"` goes |
| `RIGIDITY_FACTOR_LAW` and the Factor arm of `rigidity_limit` | `feeds/envelope.rs:2936, 3254` | none (the law text was never on a wire field of its own) |
| `suggest::rigidity_cap_mm` and `deepest_depth_within_cap` (only reader: the envelope Factor arm) | `feeds/suggest/invariants.rs:209-275`, re-export `suggest.rs:37` | none |
| The Factor arms of `rigidity_depth_cap`, `budget_line`, `ForceBudgetScope::of`, the diagnostic text "rule-of-thumb limit" | `force_budget.rs:529`, `cut_force.rs:160`, `feed_modulation.rs:463`, `diagnostics/adapters/from_tool_load.rs:754` | the diagnostic text of a Factor exceedance goes |
| `limits::check::basis_of_bound` arm of `RigidityRuleOfThumb` | `limits/check.rs:210` | none |
| viz arm `BoundSource::RigidityRuleOfThumb` | `rs_cam_viz/src/ui/sim_diagnostics.rs:1223` | none |
| The machine field `adaptive_doc_factor`, its wire field, its `values()` row, its default 2.0 | `machine/mod.rs:76, 91, 117, 358, 661, 677` | **file format**: `[job.machine.rigidity] adaptive_doc_factor` and a machine-library `[rigidity] adaptive_doc_factor` become retired keys (refused with the fix named). MCP `inspect_machine`: the key `rigidity.adaptive_doc_factor` goes (`rs_cam_viz/src/app/mcp/project.rs:601`). GUI machine panel: the row goes (`values()` becomes `[RigidityValue; 2]`). |
| The default-depth floor that reads `adaptive_doc_factor` | `feeds/mod.rs:2893-2917` (`rigidity_ap_floor`) | the default depth of an Adaptive op with NO stated depth falls from 2.0 D to `1.20 D x` the material scale (P2). See 1.3. |
| `ADAPTIVE_RIGIDITY_DOC_DIVISOR` (1.50, Unsourced) and its F201 row | `feeds/mod.rs:2677, 2517` | the F201 Unsourced pin falls 142 -> 141 |

What stays:

- `adaptive_woc_factor`. The calculator width (`feeds/mod.rs:2931-2933`), the move predictor
  (`predict.rs:366, 734, 961`), the add-door stepover and `compute/validate.rs:296` read it.
  It is the subject of the R2 width step, not of this one.
- `operation_default_profile` row (Adaptive, Roughing) `ap_factor: 1.50`. The V-bit arm of
  `default_engagement` and the P5 card rows still read it.
- `LimitKind::Rigidity`. It now names only the force-budget limit. The label "Rigidity DOC"
  (`rs_cam_viz/src/ui/feeds/envelope.rs:166`) stays.

### 1.3 The default depth (commit C)

`default_engagement` (`feeds/mod.rs:2788`) gives an Adaptive op with no stated depth
`ap = max(ADAPTIVE_FLAT_AP 1.20 x scale, adaptive_doc_factor x 1.50 / 1.50)`. On the default
machine the floor wins, so the default is 2.0 D, equal to the old Factor cap. When the floor
goes, the default is `1.20 x scale x D` (scale 0.80 hard, 0.90 medium, 1.05 soft, 1.0 else,
from `ADAPTIVE_*_AP_SCALE` and `feed_scale_factor()`; on the three P2 materials the scale is
1.0 for generic softwood, 0.80 for generic hardwood, 0.90 for MDF). The envelope then caps it.
No add door reaches this default: `geometry_default::ADAPTIVE` and `ROUGH_3D` write a stated
depth (`2/6 D` and `3/6 D`). So FM1 does not move in commit C.

### 1.4 Visible product surfaces

- Feeds card and envelope plot of an Adaptive op: the Rigidity edge is a curve over the width,
  not a flat line at 2 D. The proposal can now bind on it.
- Simulation: the force card of an Adaptive op prints the budget line and a state; the force
  field and the chart contours draw; the depth row reads the force budget.
- `project.sim_outside_envelope` can now fire on an Adaptive op whose samples sit above the
  budget curve (it judges each steady sample against the drawn envelope).
- Docs to update in the same commits: `FEATURE_CATALOG.md` (the sentence "The Adaptive family
  keeps `adaptive_doc_factor`" and the rigidity list "adaptive depth 2.0 D"), `feeds/CLAUDE.md`
  (the P13c invariant), `machine/CLAUDE.md` (the `depth_cap_rule` text), `tool_load/CLAUDE.md`
  (the g_s3depth sentry text "the Adaptive factor stays"), `limits/CLAUDE.md` (29 members,
  `DepthRule`), `rs_cam_viz/src/state/simulation/force_field.rs:33` (doc).

## 2. R2: the behaviour of Suggest

**With a stated stepover.** Suggest keeps the stepover. The depth proposal is
`min(stated depth, every ap ceiling at the stated width)`, and the force budget is one of the
ceilings. This is `SafeEnvelope::propose` as it stands; P29-4 adds no code for it. K2 holds: the
proposal never raises a stated depth.

**With no stepover.** Today the envelope takes the calculator width `base.radial_width_mm`
(`0.14 D x` scales, raised for a wood flat or bull tool to `adaptive_woc_factor x D`, then
capped at it: 0.25 D on the default machine) and caps the depth at that width.

**The 0.5 D ceiling and its derivation.** `feeds::geometry::peak_chip_thinning_lift`
(`geometry.rs:30`) is `1 / sin(psi)` with `cos psi = 1 - ae / r`. At `ae = r = D / 2`,
`psi = pi / 2`, so `sin psi = 1` and the lift is 1. For `ae > D / 2` the code returns 1 (Garr
prints no change there). So 0.5 D is the narrowest width at which radial chip thinning stops.
The number comes from geometry, not from a source; the choice to stop the width there is a
ruling (R2).

**Proposed scope: P29-4 caps the depth only. The no-stepover proposal waits for its own step.**
Reasons:

1. No product door reaches the no-stepover branch. The add door (`geometry_default::ADAPTIVE`,
   `ROUGH_3D`) writes a stepover from `adaptive_woc_factor`. Every FM1 Adaptive cell and every
   wanaka Adaptive op states one. A behaviour that no door reaches cannot be predicted against
   a product result.
2. The ceiling is a new limit. It needs a new `LimitKind` and `LimitId` member, a drawn edge,
   and the removal-rate pick for the Adaptive family (`envelope.rs:1320`). That is a second
   behaviour change with its own pins (`the_pocket_proposal_is_the_removal_rate_maximum_p13c`,
   the corner-coverage rule that must stay Pocket-only).
3. The honest no-stepover answer also depends on the add door: if the add door keeps writing
   0.25 D, the branch stays unreached. The R2 step must decide the add door too (Q2).

So P28-05 and P28-08 stay open. M7 closes for another reason (section 3).

## 3. Exemption rows

Rows that close in P29-4:

| Row | Class | Why it closes | Markers that go |
|---|---|---|---|
| `P28-01` | NeedsRuling | R1: the Adaptive family reads the force budget | `machine/mod.rs:314` (deleted with the arm); `envelope.rs:3247` loses the id |
| `P28-34` | NeedsRuling | the modulator, the force card and the depth gate keep no scope of their own: each reads the one depth limit. The finish, slot-as-finish and drill exemptions (P28-02, P28-03, P28-04) live on `depth_limit` | `suggest/invariants.rs:226` (file part deleted), `depth.rs:243`, `cut_force.rs:156`, `feed_modulation.rs:462`, `envelope.rs:3247`, `force_budget.rs:526`, `machine/mod.rs:316`, `session/compute.rs:2316` |
| `M7` | NeedsRuling | its read, `family != OperationFamily::Adaptive` in the removal-rate condition (`envelope.rs:1321`), is unreachable: both Adaptive ops have `Radial::Width(_)`, so their envelope is a `Plane`, never an `AxialBar`. The deletion is byte-identical | `envelope.rs:1319` becomes `// EXEMPT(P28-05, P28-06)` |

Rows that stay and change:

- `F-DepthRule`: the depth limit is still set from the family and the role. Its `limit` list
  loses `DepthFactor` (it becomes `&[LimitId::ForceBudget]`), and `closes_in` becomes "P29-5
  (P28-03)". Its marker moves to the Roughing arm of `depth_limit`. The label reads of the gate
  context (`depth.rs:243`, `cut_force.rs:156`, `force_budget.rs:526`: they call
  `depth_cap_rule(family, role)` because the gate context holds no config until P29-6) get the
  marker `F-DepthRule`, and the row's `where_` gains those three files. `depth.rs:243` becomes
  `// EXEMPT(P28-02, F-DepthRule)`.
- `GATES` (`exemptions.rs:79`) loses `LimitId::DepthFactor`.

New pins of `tests/the_exemption_list_only_shrinks_p29_3.rs`:

| Pin | Today | After P29-4 |
|---|---|---|
| `ROWS` | 67 | 64 |
| `PHYSICAL` | 8 | 8 |
| `RULED` | 3 | 3 |
| `MAX_NEEDS_RULING` | 56 | 53 |
| `ROWS_WITH_A_FILE` | 59 | 56 |
| `EXEMPTIONS: [Exemption; N]` | 67 | 64 |

Claim 4 of that sentry asserts that each of the 34 P28 ids and the 12 M ids is a row. Three ids
leave. Add `const CLOSED: &[(&str, &str)] = &[("P28-01", "P29-4"), ("P28-34", "P29-4"),
("M7", "P29-4")]`, and assert that each audit id is a row or in `CLOSED`, never both (Q6).

## 4. Commits

Three commits. A moves the numbers. B and C move no output that a product path makes today.

**Commit A: the behaviour.**

- Delete the Adaptive arm of `depth_limit` and its markers; close P28-01, P28-34 and M7
  (section 3; the M7 clause deletion is here because it sits on the same marker lines).
- Re-point every test of section 6.4 group A1.
- Re-bless FM1 (`RS_CAM_UPDATE_FEEDS_MATRIX=1 ... --features heavy-tests --test
  feeds_matrix_instrument_fm1`); the diff must equal `P29-4_predictions_fm1.csv` exactly.
- The Factor types stay in this commit. They are dead at its end; commit B deletes them.
- Gates: fmt check; named sentries `the_exemption_list_only_shrinks_p29_3`,
  `every_limit_is_a_limit_id_p29_2`, `a_sample_states_its_cut_kind_p29_3`,
  `every_op_declares_its_cuts_p29_3`, `the_roughing_depth_meets_the_force_budget_p13c`,
  `the_depth_that_cut_is_a_measured_load_g_s3depth`, `finish_depth_is_reported_not_capped_fm6`,
  `the_modulator_reads_the_depth_gate_force_budget_rule_p16`,
  `the_session_modulator_binds_on_the_force_budget_p16`,
  `the_cut_force_is_the_deflection_force_p17`, `the_envelope_keeps_each_limit_k2b`,
  `suggest_never_raises_a_stated_depth_k2`, `an_envelope_edge_does_not_move_with_the_stated_cut_p19`,
  `a_pick_outside_the_envelope_warns_k2b`, `the_pocket_proposal_is_the_removal_rate_maximum_p13c`,
  `the_add_door_stepover_is_the_proposal_p13c`, `no_depth_is_proposed_that_cannot_be_modelled_p16`,
  `a_weak_bound_cannot_refuse_an_export_g_s4weak`, `an_unmodelled_refusal_names_every_row_g_t19`,
  `one_cut_gives_one_load_on_every_door_p29_1`; `cargo test -p rs_cam_core -q --lib`; FM1; full
  clippy; viz `--lib` and the viz tests `p17_visual_snapshots`,
  `the_sim_chart_seeks_on_a_dot_p17`, `the_envelope_plot_shows_only_what_limits_the_cut_p5c`,
  `every_limit_is_drawn_and_named_p5d`, `readiness_shows_the_limits_g_readylimits` (a viz
  image and the rivmap envelope move; section 6.4 group A2).

**Commit B: delete the dead Factor chain (section 1.2, every row but the machine field, the
floor and the divisor).**

- `DepthRule` (Q4), `DepthCapRule::Factor`, `RigidityDepthCap::Factor`,
  `BoundSource::RigidityRuleOfThumb`, `LimitId::DepthFactor`, `RIGIDITY_FACTOR_LAW`,
  `rigidity_cap_mm`, `deepest_depth_within_cap`, each Factor arm and the diagnostic text.
- `DepthVerdict::fixture_within` (`verdict.rs:2337`, `#[cfg(test)]`) takes a `ForceBudget` bound.
- Proof: FM1 byte-equal to the CSVs of commit A, both files.
- Gates: as commit A (viz `--lib` and the named viz tests: `sim_diagnostics.rs` and two viz
  tests name a deleted variant).

**Commit C: delete the machine field `adaptive_doc_factor`, the default-depth floor and
`ADAPTIVE_RIGIDITY_DOC_DIVISOR`.**

- `RigidityProfile`, `RigidityProfileWire`, `values()`, `Default`, `MachineProfileWire`
  conversion, the `INDUSTRIAL_NEEDS_RIGIDITY` text; two retired-key rows (the project table
  `["job", "machine", "rigidity"]` and the machine-library `[rigidity]`, as `slot_doc_factor`);
  MCP `inspect_machine`; F201 pin 142 -> 141.
- Delete the key line from the 19 fixture files that hold it (section 6.4 group C2) and from the
  inline TOML of `tests/step_project_load.rs:204`.
- The two wanaka files hold the key (`planning/rivmap_block_job/nz_wanaka_21_operator.toml:74`,
  `nz_wanaka_21_6mm_rough_work.toml:74`). After commit C neither file loads until the line goes.
  The operator file needs the operator's say (Q1).
- Proof: FM1 byte-equal (every FM1 Adaptive cell states its depth). Measured: the probe walked
  the FM1 matrix (`walk_matrix`) on a machine with `adaptive_doc_factor = 0.0`, which makes the
  floor zero (the same as its deletion) and removes the Factor limit from the envelope
  (`rigidity_cap_mm` filters a zero cap). Against the committed CSV only `cap_factor` and
  `cap_mm` of the 80 Adaptive rows differ; commit A has already moved those two columns.
- Gates: as commit A, plus `every_feeds_formula_constant_states_its_source_f201`,
  `an_industrial_machine_states_its_rigidity_p11c`, `the_rigidity_default_is_hobby_grade_p11c`,
  `a_value_outside_its_domain_is_refused_at_every_door_w6b`, `step_project_load`,
  `--lib io::machine_library`, viz `--lib` (the machine panel and the MCP reply).

## 5. Risks and questions for the lead

Most important first. Each question has my choice.

**Q1. The operator file holds `adaptive_doc_factor`.** Commit C makes it refuse to load. The
rule "never save over `nz_wanaka_21_operator.toml` without the operator's say" covers a
one-line delete too. Choice: land A and B now; hold C until the operator says yes to the
one-line delete in both wanaka files. A and B leave the field in place and read (by the floor),
so no legacy rule is broken between A and C.

**Q2. The 0.5 D no-stepover proposal waits (section 2).** Choice: a separate step "P29-4w"
after P29-6, which also decides whether the add door stops writing `adaptive_woc_factor x D`.
P28-05, P28-08 and `adaptive_woc_factor` close there.

**Q3. "A limit warns and does not change a stated cut" is false after a simulation.** The
modulator now caps the feed of every Adaptive move that is over the budget
(`ForceBudgetScope::Limit`). `apply_adaptive_feed_modulation` writes the modulated toolpath into
the result slot (`session/compute/simulation.rs:835`), and export reads that slot
(`session/compute/export.rs:500`, `result.toolpath()`). So the exported feeds of an Adaptive
op over the budget fall after a simulation with modulation on (the default). The geometry and
the depth stay; the feed falls. This is the same rule the Pocket family has had since P16 item
7. Choice: accept it as the P16 behaviour, and state it in the commit message and the catalog.
The operator's cut runs at those lower feeds unless the stated cut changes.

**Q4. One-member enums.** After commit B, `DepthRule` and `DepthCapRule` each hold one
member. Choice: delete `DepthRule` and `Limit::depth_rule`; make `DepthLimit` hold
`ForceBudget` and `NoLimit(NoDepthLimit)`; keep `DepthCapRule` as a struct
`DepthCapRule { budget_n }` or replace `Option<DepthCapRule>` by `Option<f64>` named
`force_budget_n`. The P29-2 claim 7 test (`the_depth_rule_of_a_limit_is_a_typed_field`) is then
deleted with its field, by the "a retired claim is deleted" rule.

**Q5. The budget depth jumps with the width.** On a 6 mm two-flute end mill in generic hardwood
at the curve chip, the budget depth is 17.54 mm at 0.6 mm width, 16.35 mm at 1.2 mm and 3.78 mm
at 2.4 mm (P2). The force model (helix engagement) has a cliff. A small stepover change can move
the proposal by a factor of four. `refine_budget_columns` draws the cliff; the proposal reads
the exact value. Choice: no action in P29-4; name it in the commit and carry it to P27.

**Q6. Closed audit ids in the exemption sentry.** Choice: a `CLOSED` list in the sentry
(section 3), not a deletion from `P28_IDS` and `M_IDS`, so the audit lists stay the record.

**Q7. The `RigidityRuleOfThumb` example in `a_weak_bound_cannot_refuse_an_export_g_s4weak`.**
The test uses it as "the bound that does not gate". Choice: use `MachineForceBudget`, which
also does not gate (`verdict.rs:1227`); rename `only_the_rule_of_thumb_declines_to_gate` to
`only_the_machine_bounds_decline_to_gate`.

**Q8. The wanaka acceptance table.** It reads PASS 98 / FAIL 0 on `6f586706c`. After commit A
the Adaptive rows of both files gain findings (the depth gate on the force budget,
`project.sim_outside_envelope`, a lower modulated feed). Choice: re-grade after P29-6, not
inside this step; P29-4 does not simulate wanaka.

Risk: the FM1 sim cell `Adaptive3d / EndMill / hardwood` sits at a 3.05 % margin under the
budget curve (my exact solve). The product judges the drawn curve, which interpolates between
refined columns. If the interpolation passes under a sample, the cell gains the id
`project.sim_outside_envelope`. My prediction is "no new id". If the id appears, the cause is
the drawn interpolation, not the cut; the lead decides.

## 6. Predictions

Computed on HEAD `34e678d5f` with a throwaway test (`tests/zz_p29_4_probe.rs`, deleted;
`git status --short crates/` is clean). The probe called only existing functions. The arm that
P29-4 gives the Adaptive family exists today as the arm of the Pocket family, so the probe called
`force_budget::rigidity_depth_cap(machine, OperationFamily::Pocket, Roughing, ..)` on the
Adaptive op's own cut (`cap_cut_of_operation`). Calibration, before any Adaptive number was
used:

- The FM1 `cap_factor` and `cap_mm` columns of all 160 Face, Pocket, Adaptive and Adaptive3d
  cells, recomputed with today's family: 0 mismatches at four decimals (Face 3.175 mm: 10.6196,
  8.6383).
- The envelope chip door: for every Face and Pocket cell my exact budget depth at the stated
  width equals `SafeEnvelope::judged_ap_ceiling(LimitKind::Rigidity, ae)` of today's envelope
  to four decimals.
- The FM1 sim depth gate: the Pocket `depth_cap_mm` (8.0194, 4.0751, 5.5040) is reproduced from
  the peak sample of my own simulation run.

### 6.1 P1: FM1

**Pre-simulation file (`matrix_2026-09-23.csv`).** 80 rows move: every Adaptive and Adaptive3d
row (5 tool types x 2 diameters x 4 materials x 2 ops), the 20 refused rows included (they print
cap columns too). Two columns move: `cap_factor` and `cap_mm`. 920 rows stay byte-equal, and
every other column of the 80 rows stays byte-equal: no depth, width, proposal, binding, speeds,
warning or diagnostic column moves.

Why no proposal moves: every FM1 Adaptive cell states its stepover and its depth (the add door),
and at every one the budget depth at the stated width is above the stated depth (the smallest
gap: Adaptive / BullNose 6 mm / hardwood, stated 2.0 mm, budget 4.09 mm; Adaptive3d / EndMill 6
mm / hardwood, stated 3.0 mm, budget 4.29 mm). The plywood rows keep an empty proposal depth (no
force line: `NoDepthProposal::NoForceLine`, as today).

The predicted values, per row: `P29-4_predictions_fm1.csv`, `file = matrix`. Summary of the
pattern:

| Rows | cap_factor | cap_mm |
|---|---|---|
| a force line and a root below the flute | 2.0000 -> 35.3224 | `2 x D_eng` -> the budget depth (for example Adaptive EndMill 6 mm hardwood 12.0000 -> 4.3924; Adaptive3d EndMill 6 mm hardwood 12.0000 -> 4.2886; Adaptive EndMill 3.175 mm hardwood 6.3500 -> 10.8641) |
| a force line and no root below the flute (most softwood and MDF rows, every ball and tapered ball row but 6 mm hardwood ball) | 2.0000 -> empty | `2 x D_eng` -> empty |
| plywood (no force line) | 2.0000 -> empty | `2 x D_eng` -> empty |

**Simulation file (`matrix_2026-09-23_sim.csv`).** 10 rows move: Adaptive x {EndMill, BullNose}
x 4 materials, and Adaptive3d / EndMill x {softwood, hardwood}. 28 rows stay byte-equal (every
Pocket, Profile, Waterline, DropCutter and Scallop row, and the skipped rows). Every speeds,
stepover, depth, chip, power, deflection and force-peak column of the 10 rows stays byte-equal;
the one peak that moves is `depth_peak_mm` of the two plywood rows (an `Unmodeled` row prints
no peak). The modulator's new force cap binds no move. The check reads the feed the cap acts on:
the emitted feed of each move (`SimulationCutTrace::modulated_feeds`, else the sample's
commanded feed). In each of the 10 cells, 0 cutting samples are over the budget at that feed
(the largest force: 25.9494 N, Adaptive3d hardwood). The reached feed
(`effective_feed_for_sample`) gives the same 0 and the same maxima.

| Cell | force_state | force_budget_n | force_detail | depth_state | depth_peak_mm | depth_cap_mm | depth_cap_factor | depth_detail | diagnostic_ids_after_sim |
|---|---|---|---|---|---|---|---|---|---|
| Adaptive EndMill softwood | Reported -> Within | -> 35.3224 | -> RULE_FORCE_BUDGET text | Within | 1.8000 | 12.0000 -> empty | 2.0000 -> empty | -> Reported | `load.depth.within` -> `load.depth.reported` |
| Adaptive EndMill hardwood | Reported -> Within | -> 35.3224 | -> RULE_FORCE_BUDGET text | Within | 1.8000 | 12.0000 -> 17.6593 | 2.0000 -> 35.3224 | empty | no change |
| Adaptive EndMill mdf | as softwood | | | | | | | | |
| Adaptive EndMill plywood | n/a (no change) | | | Within -> Unmodeled | 1.8000 -> empty | 12.0000 -> empty | 2.0000 -> empty | -> NotImplemented | `load.depth.within` -> `load.depth.unmodeled` |
| Adaptive BullNose softwood, mdf | as EndMill softwood | | | | | | | | |
| Adaptive BullNose hardwood | Reported -> Within | -> 35.3224 | -> RULE_FORCE_BUDGET text | Within | 1.8000 | 12.0000 -> 17.1952 | 2.0000 -> 35.3224 | empty | no change |
| Adaptive BullNose plywood | as EndMill plywood | | | | | | | | |
| Adaptive3d EndMill softwood | Reported -> Within | -> 35.3224 | -> RULE_FORCE_BUDGET text | Within | 3.0000 | 12.0000 -> 6.7487 | 2.0000 -> 35.3224 | empty | no change |
| Adaptive3d EndMill hardwood | Reported -> Within | -> 35.3224 | -> RULE_FORCE_BUDGET text | Within | 3.0000 | 12.0000 -> 3.7314 | 2.0000 -> 35.3224 | empty | no change (risk, section 5) |

RULE_FORCE_BUDGET text = "force budget of the machine (the depth-cap rule of this family)". The
severities column does not move (every new id is `Info`, as on the Profile and Pocket rows).

Counts: 90 FM1 rows move (80 + 10), 948 stay byte-equal. 220 cell values move. No column of
section 6.1 needs a value I could not compute. `MATRIX_SUMMARY.md` is rewritten on the re-bless;
it is not compared.

### 6.2 P2: Suggest on the default machine

Matrix: 2D Adaptive and 3D Adaptive; flat two-flute end mill (`tool_of` of FM1: cutting length
`max(3 D, 12)`) at 3.175, 6 and 12 mm; generic softwood, generic hardwood, MDF; stated stepover
0.1 D, 0.2 D, 0.4 D. Each value is the envelope proposal depth in mm (`proposal.ap_mm`). "Stated
2 D": the op states a depth of 2 D. "Unstated": the op states no depth (the calculator default).
After A = after commit A; after C = after commit C. Full table with the binding limit, the curve
chip and the budget depth: `P29-4_predictions.csv`, rows `P2`.

2D Adaptive, stated depth 2 D (today -> after A):

| D | Material | 0.1 D | 0.2 D | 0.4 D |
|---|---|---|---|---|
| 3.175 | softwood | 3.6907 = | 3.5878 = | 3.3189 = |
| 3.175 | hardwood | 3.6907 = | 3.5878 = | 3.3189 = |
| 3.175 | MDF | 3.6909 = | 3.5878 = | 3.3189 = |
| 6 | softwood | 6.9751 = | 6.7802 = | 6.2719 = |
| 6 | hardwood | 6.9751 = | 6.7802 = | 6.2719 -> 3.7808 |
| 6 | MDF | 6.9756 = | 6.7802 = | 6.2719 -> 5.4331 |
| 12 | softwood | 24.0000 = | 24.0000 -> 4.8668 | 24.0000 -> 3.7124 |
| 12 | hardwood | 24.0000 -> 3.5906 | 24.0000 -> 2.6545 | 22.0557 -> 2.1565 |
| 12 | MDF | 24.0000 -> 5.8829 | 24.0000 -> 4.1656 | 24.0000 -> 3.3279 |

3D Adaptive, stated depth 2 D (today -> after A):

| D | Material | 0.1 D | 0.2 D | 0.4 D |
|---|---|---|---|---|
| 3.175 | all three | 3.1750 = | 3.1750 = | 3.1750 = |
| 6 | softwood | 6.0000 = | 6.0000 = | 6.0000 = |
| 6 | hardwood | 6.0000 = | 6.0000 -> 4.7663 | 6.0000 -> 3.7519 |
| 6 | MDF | 6.0000 = | 6.0000 = | 6.0000 -> 5.3828 |
| 12 | softwood | 12.0000 = | 12.0000 -> 6.3645 | 12.0000 -> 5.5355 |
| 12 | hardwood | 12.0000 -> 3.6849 | 12.0000 -> 3.3631 | 12.0000 -> 3.1566 |
| 12 | MDF | 12.0000 -> 5.8829 | 12.0000 -> 4.1656 | 12.0000 -> 3.3382 |

"=" means no change. Where the depth moves, the binding limit becomes `Rigidity`; where it does
not, it stays `VendorDepthTable` (2D, 3.175 and 6 mm), `VendorAxial` (3D) or none (2D 12 mm,
where the stated 24 mm equals today's Factor cap). Today the Factor cap binds no cell of this
matrix: the vendor depth limits sit under 2 D.

Unstated depth. Today the default is 2.0 D (the floor), so "unstated today" equals "stated 2 D
today" on every cell, and "unstated after A" equals "stated 2 D after A". After C the default is
`1.20 D x scale`. Measured, not only derived: `feeds_result_for_operation` on a machine with
`adaptive_doc_factor = 0.0` gives 3.175 mm: softwood 3.8100, hardwood 3.0480, MDF 3.4290; 6 mm:
7.2000, 5.7600, 6.4800; 12 mm: 14.4000, 11.5200, 12.9600 (2D and 3D the same; stepover 0.2 D). The proposal after C is the smaller of that default
and the caps; the cells where it moves against "after A" are listed in the CSV
(`proposal_unstated_after_C`; for example 2D 6 mm hardwood 0.1 D: 6.9751 -> 5.7600; 2D 12 mm
softwood 0.1 D: 24.0000 -> 14.4000).

### 6.3 P3: the two wanaka projects (from code and from `P29-0_probes.md`; no simulation)

**Before the simulation (exact).** The probe loaded each file read-only and asked today's
envelope. The predicted proposal is `min(stated, every other ap ceiling, the budget depth at the
stated width and the operator chip)`. Binding after A: `Rigidity` on every row where the depth
moves.

| File | idx | Tool | Stated ae x ap | Proposal today (binding) | Budget depth at the stated width | Proposal after A |
|---|---|---|---|---|---|---|
| operator | 1 | 18 mm 2F | 5.5 x 10.0 | 10.0000 (none) | 2.0924 | 2.0924 |
| operator | 2 | 6 mm 2F | 1.5 x 6.0 | 6.0000 (none) | 3.5932 | 3.5932 |
| operator | 14 | 18 mm 2F | 3.6 x 4.166 | 4.1660 (none) | 2.4547 | 2.4547 |
| operator | 15 | 6 mm 2F | 1.382 x 5.532 | 5.5320 (none) | 3.6983 | 3.6983 |
| work | 1 | 6 mm 2F | 1.2 x 6.0 | 6.0000 (none) | 3.9543 | 3.9543 |
| work | 2 | 6 mm 2F | 1.5 x 6.0 | 6.0000 (none) | 3.6696 | 3.6696 |
| work | 14 | 6 mm 2F | 1.2 x 3.0 | 3.0000 (none) | 3.9543 | 3.0000 (no change) |
| work | 15 (disabled) | 6 mm 2F | 1.382 x 5.532 | 5.5320 (none) | 3.7837 | 3.7837 |

The note says "the 6 mm idx 1 budget depth is 3.95 mm at 1.2 mm width". Recomputed:
3.9543 mm (`depth_at_budget`, white oak, ae 1.2, fz 0.153562, 35.3224 N). The two agree. The
stated depth is the shipped depth; Suggest writes the proposal only on an apply with a pick.

**After the simulation (conditional).** Today's depth verdicts (`P29-0_probes.md` section 6):
operator idx 1 Within (10.0 of 36.0), idx 2 Exceeds (20.7 of 12.0), idx 14 Within, idx 15
Within; work idx 1 Within (12.0 of 12.0), idx 2 Exceeds (16.95 of 12.0), idx 14 Within.

What I predict:

- Every one of these ops reads the force budget in the depth gate, the force card and the
  modulator after A.
- The count of steady samples over the budget at the gate's cut is at most the count in
  `P29-0_probes.md` section 8 (operator 38 181, 3 797, 27 596, 157; work 59 809, 92, 0). Reason:
  the step changes no geometry, and a new modulator cap can only lower a feed, so no sample's
  force rises.
- Work idx 14: not predicted. P29-0 measured 0 samples over at the reached feed, but its worst
  sample is 35.28 N against 35.32 N (a 0.04 N margin), and the modulator's cap acts on the
  emitted feed, which is at or above the reached feed. Without a simulation I cannot say
  whether the cap binds, so its exported feeds and its depth verdict are not predicted.
- The other six ops: the modulator binds `ForceBudget` on at least one move each (P29-0 counts
  over-budget samples at the feed the modulator chose without this cap). Their exported feeds
  fall after a simulation (Q3).

Not predicted (needs a wanaka simulation): the depth verdict of operator idx 1, 2, 14, 15 and
work idx 1, 2, and the exact counts after the modulation. The gate solves its cap at the
modulated chip of the peak-depth sample, and the modulator now lowers that chip toward the
budget, so the verdict depends on the modulator's per-move result.

### 6.4 P4: tests, pins and goldens that turn red or need a re-bless

**A1. Commit A, red by construction (each asserts today's Factor rule on an Adaptive op):**

- `tests/feeds_matrix_instrument_fm1.rs`: both golden tests (re-bless; section 6.1).
- `tests/the_exemption_list_only_shrinks_p29_3.rs`: the four pins and claim 4 (section 3).
- `tests/every_op_declares_its_cuts_p29_3.rs`: `claim_2_the_facts_equal_the_label_functions`
  (its own copy of the table, line 217).
- `tests/every_limit_is_a_limit_id_p29_2.rs`: `the_depth_rule_of_a_limit_is_a_typed_field`
  (line 651 asserts `factor >= 1` over the Pocket and Adaptive envelopes; after A no envelope
  holds a Factor limit). Commit B deletes the test with its field (Q4).
- `rs_cam_viz/tests/the_sim_chart_seeks_on_a_dot_p17.rs`:
  `a_limit_above_the_range_extends_the_axis_or_keeps_a_quiet_note_p17b`. Its `rough_env`
  (line 566) expects a flat Rigidity ceiling on the rivmap100 3D Rough. Measured: today the edge
  is `ApCeiling { mm: 9.0 }` (the fixture states `adaptive_doc_factor = 1.5`, D 6 mm); after A it
  is a curve (white oak, operator chip 0.06667 mm: 19.06 mm at 0.6 mm width, 17.18 mm at 1.2 mm,
  5.44 mm at 1.8 mm, 4.34 mm from 4.2 mm up), under the vendor DOC (6.0 mm) from 1.8 mm. The
  `expect("the rigidity DOC is flat")` panics.
- `tests/the_roughing_depth_meets_the_force_budget_p13c.rs`:
  `the_adaptive_family_keeps_its_factor_this_phase_p13c` (delete: a retired claim).
- `tests/the_depth_that_cut_is_a_measured_load_g_s3depth.rs`:
  `each_family_reads_the_factor_its_own_pass_role_implies`.
- `tests/finish_depth_is_reported_not_capped_fm6.rs`: the tapered-ball Adaptive test at line
  372 and `the_gate_reads_the_cone_diameter_fm6` (both expect `RigidityDepthCap::Factor`).
- `tests/the_modulator_reads_the_depth_gate_force_budget_rule_p16.rs`: the one test (line 140).
- `tests/the_cut_force_is_the_deflection_force_p17.rs`: `the_budget_line_follows_the_depth_cap_rule`
  (it asserts, lines 262-270 and 350-370, that an Adaptive has no budget line, no card state and
  no bound source).
- `tests/the_envelope_keeps_each_limit_k2b.rs`: `the_spindle_power_binds_and_names_the_curve_k2b`
  (12 mm, oak, 15 x 8 mm: the budget depth 1.3279 mm binds before power at 12.4636 mm) and
  `the_deflection_target_binds_and_names_its_constant_k2b` (budget 1.4496 mm binds before
  deflection at 2.9150 mm). The other three Adaptive cells of that file stay green (budget no
  root below the flute, or a stated depth of 0.5 mm under 6.0767 mm).
- `src/tool_load/depth.rs` lib tests (the fixture runs `OperationType::Adaptive` at a 0.25
  factor; after A the cap is the force budget, and on `Material::default()` (generic softwood)
  at ae 2.4 mm, fz 0.041667 mm the budget has no root below the flute, so the gate returns
  `Reported`): `the_peak_is_the_maximum_engaged_depth`, `a_depth_past_the_cap_exceeds`,
  `a_cut_exactly_on_the_cap_is_within`, `a_trace_with_no_cutting_samples_is_vacuous`,
  `a_transit_sample_never_sets_the_peak`, `f32_noise_at_the_cap_is_within`,
  `a_depth_past_the_dexel_resolution_exceeds`. Seven red. `no_trace_asks_for_a_simulation`,
  `a_drill_cycle_does_not_apply`, `no_machine_profile_is_not_implemented`,
  `a_finishing_pass_reports_its_depth_with_no_bound` and `the_pocket_reads_the_force_budget_p13c`
  stay green.

**A2. Commit A, at risk.** I could not run them (viz tests are outside my rules). A red or a
green here is no mismatch; the implementer fixes a red and names it.

- `rs_cam_viz/tests/p17_visual_snapshots.rs`: the rough chart images (`rough_chart_set`, the
  rivmap envelope).
- `rs_cam_viz/tests/the_envelope_plot_shows_only_what_limits_the_cut_p5c.rs` and
  `every_limit_is_drawn_and_named_p5d.rs`: the drawn-kinds lists of the rivmap 3D Rough (the
  Rigidity edge changes from a flat line to the curve above; the stated point 2.0 x 1.2 mm stays
  inside it, and the proposal stays 2.0 mm with no binding).
- Outside the gate set (not run by the gate; listed so a later red has a named cause): every
  integration test that simulates an Adaptive op with modulation on in a material with a force
  line and pins a feed, a time, a binding count or G-code (for example `kinematics_histogram`,
  `kinematic_utilization_p2`, `p1_headless_ab_wanaka`, `swept_wanaka_ab_s1`,
  `wanaka_e2e_chipload_gate`, `wanaka_suggest_integration`).

**B. Commit B, compile reds (each names a deleted item):**
`tests/every_limit_is_a_limit_id_p29_2.rs` (lines 60, 100 `ALL.len() == 29`, 271, 281, 554,
618-645), `tests/an_unmodelled_refusal_names_every_row_g_t19.rs:119`,
`tests/the_depth_that_cut_is_a_measured_load_g_s3depth.rs:307-310, 381, 465`,
`tests/finish_depth_is_reported_not_capped_fm6.rs:408, 445`,
`tests/a_weak_bound_cannot_refuse_an_export_g_s4weak.rs:50` (Q7),
`tests/feeds_matrix_instrument_fm1.rs:704` (the `cap_columns` match; FM1 output unchanged),
`src/tool_load/verdict.rs:2337` (`fixture_within`),
`rs_cam_viz/tests/p17_visual_snapshots.rs:312`, `rs_cam_viz/tests/the_sim_chart_seeks_on_a_dot_p17.rs:70`,
`rs_cam_viz/src/ui/sim_diagnostics.rs:1223`. Comments that name the variant:
`rs_cam_viz/tests/limits_fixture/mod.rs:93`, `readiness_shows_the_limits_g_readylimits.rs:135`.

**C1. Commit C, compile reds (each sets or reads the field):** `src/feeds/suggest/tests.rs`
lines 1046, 1113, 1144, 1297, 1405, 1478, 1550 (behaviour green: the speeds chain reads no depth
cap since P5); `src/tool_load/depth.rs:482, 515`;
`tests/suggest_never_raises_a_stated_depth_k2.rs:56`; `tests/the_depth_that_cut_is_a_measured_load_g_s3depth.rs:468`;
`tests/finish_depth_is_reported_not_capped_fm6.rs:390`;
`tests/the_roughing_depth_meets_the_force_budget_p13c.rs:214, 319`;
`rs_cam_viz/src/app/mcp/project.rs:601`; `rs_cam_viz/src/ui/properties/machine_panel.rs:250`
(the loop reads `values()`; the array length changes, the code compiles);
`tests/every_feeds_formula_constant_states_its_source_f201.rs:110` (pin 142 -> 141).

**C2. Commit C, load reds (the key is refused):** `tests/fixtures/test_job.toml`,
`tests/fixtures/p_thin_005_2026-11-07_3fc31cc8/project.toml`,
`tests/fixtures/rivmap100_rough_2026-11-04_7effc804/rivmap100.toml`,
`tests/fixtures/t3b_r10_scallop_islands_2026-09-08_e4d817e9.toml`,
`tests/fixtures/testpiece_mini_2026-10-20_c3808eb0/testpiece_mini.toml`,
`tests/fixtures/wanaka100/wanaka_full_tuned.toml`, `tests/fixtures/wanaka_2026-08-16_afc6ecd4.toml`,
`tests/fixtures/wanaka_airrun_2026-06-01_bb713634.toml`,
`rs_cam_viz/tests/fixtures/stale_cards/face_and_rest_rough.toml`, the ten `test_data/ux_*.toml`
files, and the inline project of `tests/step_project_load.rs:204`. Each needs its key line
deleted; no other value of those files changes. `tests/literature_matrix/cells.toml:168` names
the key in a comment only.

Screened and predicted green: the 13 calculator lib tests of `src/feeds/tests.rs` that run an
Adaptive input with no stated depth (their assertions are orderings or caps that the new
default `1.20 D x scale` keeps: adaptive deeper than pocket, rough at or above finish, the flute
guard at 4.0 mm); `compute/geometry_default.rs:320` (it reads the Rigidity limit only for an
`EnvelopeProposal` rule, which no Adaptive add door has);
`the_suggest_clamp_still_fires_at_the_helper_s_own_cap` (a Pocket).

"Nothing else moves": in the gate set (fmt, the named sentries of section 4, core `--lib`, FM1,
clippy, viz `--lib` and the named viz tests), a red outside A1, A2, B, C1 and C2 is a mismatch.

### 6.5 P5: what stays byte-identical

- Every non-Adaptive op on every surface (Suggest, envelope, FM1 920 + 28 rows, gates,
  modulator, cards, MCP, CLI). The step changes one arm of one table, and only the Adaptive
  family reaches it.
- The toolpath geometry of every op, the Adaptive ops included: no generator or planner reads
  the depth rule or `adaptive_doc_factor` (`rg` over `src/adaptive*`, `src/ops`, `src/finish`:
  no reader).
- The G-code of every op from a generation with no simulation after it.
- **The claim "G-code of every op at a stated depth stays, since a limit warns" is false after a
  simulation** (Q3). An Adaptive op whose moves exceed the budget gets lower modulated feeds,
  and export writes them. True for: the 10 FM1 Adaptive sim cells (0 samples over the budget at
  the emitted feed). False for: operator idx 1, 2, 14, 15 and work idx 1, 2. Not predicted:
  work idx 14 (a 0.04 N margin, section 6.3).
- Commit B: every output byte-identical; only the wire variants of section 1.2 leave the
  schema (no product path made them after A).
- Commit C: every op with a stated depth byte-identical (FM1 both files). An Adaptive op with
  no stated depth gets the default of section 1.3. A file with the key refuses to load.

## 7. Contradictions found in today's code

1. The change list says "up to 48 FM1 cells" (from P26). With R2 (the stated stepover stays)
   and the add door that states both axes, no proposal, depth or speeds column moves. The cells
   that move are 80 pre-simulation rows (cap columns only, refused rows included) and 10
   simulation rows.
2. "A limit warns and does not change a stated cut": the modulator lowers exported feeds after
   a simulation (Q3, section 6.5).
3. `adaptive_doc_factor` has a second reader that the change list does not name: the
   default-depth floor of `default_engagement` (`feeds/mod.rs:2905`). The field can only go
   with that floor (commit C), and that moves the default depth of an op with no stated depth.
4. The Factor cap binds almost nowhere today: in the P2 matrix the vendor depth table or the
   vendor axial bound sits under 2 D on every 3.175 mm and 6 mm cell, and on 12 mm the stated 2 D
   equals the cap. R1 is the first rule that limits the Adaptive depth on hardwood and on large
   tools.
5. The M7 read (`family != OperationFamily::Adaptive` on an axial bar) is unreachable today.
   It closes in P29-4 with no output change, although the change list ties it to the width
   rule.
6. The P29-0 over-budget counts of the Adaptive ops were measured with the modulator out of
   scope for those ops. After A they are upper bounds, not the post-change counts.

## 8. Lead rulings (2026-11-12, night)

The operator ruled on 2026-11-11: "implement P29-4 after the predictions are committed". This
note and its two CSV files are those predictions. A result that differs from them stops the
work, and nothing is committed.

- Q1. Commits A and B land now. Commit C waits for the operator: it needs a one-line delete in
  `nz_wanaka_21_operator.toml`. The field `adaptive_doc_factor` stays until then, read only by
  the default-depth floor.
- Q2. P29-4 caps the depth only. The proposal with no stepover (the 0.5 D ceiling) is its own
  step after P29-6. P28-05 and P28-08 stay open.
- Q3. Accepted. After a simulation with modulation on, the feed of an Adaptive move over the
  budget falls, as for the Pocket family since P16. The commit message and the catalog state
  it. The lead tells the operator before the cut.
- Q4. Delete `DepthRule` and `Limit::depth_rule`. `DepthLimit` holds `ForceBudget` and
  `NoLimit(NoDepthLimit)`. `DepthCapRule` becomes a struct with the one field `budget_n`; a
  bare `f64` is not allowed.
- Q5. No action in P29-4. The cliff of the budget depth over the width goes to P27, and the
  lead shows it to the operator.
- Q6. A `CLOSED` list in the sentry. The commit text names ruling R1 for each closed row.
- Q7. As chosen.
- Q8. As chosen. P29-4 does not simulate wanaka.
- The risk cell (3D Adaptive, end mill, hardwood, 3.05 % under the curve): a new
  `project.sim_outside_envelope` id there is a mismatch. The implementer stops and reports.
