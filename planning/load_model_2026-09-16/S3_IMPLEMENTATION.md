# S3 — depth of cut as a post-simulation, non-gating criterion

Written 2026-09-18. Step S3 of `SURFACE_IMPL.md` §1, on the decision in
`PLAN.md` §11 (Reading A).

Depth of cut sets the depth on nearly every recipe this engine ships, and
before this step the operator could not see it as a limit. The cap lived in
one place: `feeds::suggest::invariants::clamp_dpp_to_rigidity`, which lowered
the depth per pass and pushed a `RoughingDepthClampedToRigidity` warning.
After the clamp the number disappeared.

**S3 adds no number.** Every bound it publishes is the factor the machine
profile already carries times the tool diameter, through one helper the
Suggest clamp now calls too.

---

## 1. The helper, and the three factors

`crates/rs_cam_core/src/machine/mod.rs`:

```rust
pub struct RigidityDepthCap {
    pub factor: f64,
    pub diameter_mm: f64,
}
impl RigidityDepthCap {
    pub fn cap_mm(&self) -> f64 { self.factor * self.diameter_mm }
}

impl RigidityProfile {
    pub fn depth_cap_mm(
        &self,
        family: crate::feeds::OperationFamily,
        pass_role: crate::feeds::PassRole,
        diameter_mm: f64,
    ) -> Option<RigidityDepthCap>;
}
```

| Family / role | Factor |
|---|---|
| `Drill` | none (`None`) |
| `Adaptive` | `adaptive_doc_factor` |
| any other, `Roughing` | `doc_roughing_factor` |
| any other, `SemiFinish` or `Finish` | `doc_finishing_factor` |

**The cap is not stored.** `RigidityDepthCap` holds the factor and the
diameter, and `cap_mm()` multiplies them. That is the same pair
`BoundSource::RigidityRuleOfThumb { factor, diameter_mm }` carries, so the
row's bound and the row's provenance are one value seen twice. Nothing can
drift.

**The type the helper keys on is the clamp's own pair**, `feeds::OperationFamily`
and `feeds::PassRole`, not the vendor-LUT mirrors. No new classification and
no inverse mapping was invented: the gate reads
`ctx.operation_kind.spec().feeds_family` and `.feeds_pass_role`, which is the
same `OperationSpec` the clamp reads through `operation.spec()`. The two
readers cannot classify one operation differently.

### The clamp, and why every Suggest number is byte-identical

`clamp_dpp_to_rigidity` keeps its roughing-only gate, its `current.is_finite()`
guard, its `cap.is_finite() && cap > 0.0` guard and its warning. Only the
factor selection moved:

```rust
if let Some(current) = operation.depth_per_pass()
    && matches!(pass_role, PassRole::Roughing)
    && let Some(cap) = machine.rigidity.depth_cap_mm(
        operation.op_type().spec().feeds_family, pass_role, tool.diameter)
{
    let cap = cap.cap_mm();
    …unchanged…
}
```

`cap_mm()` is `factor * diameter_mm`, the same expression in the same order,
so the value is bit-identical, not merely equal.

**The one new arm is the drill family's `None`, and it is unreachable from
Suggest.** `DrillConfig` and `AlignmentPinDrillConfig` declare no
`depth_per_pass` in `impl_operation_params!`, so `operation.depth_per_pass()`
is `None` on a drill op and the `if let` never binds. Before S3 the clamp
would have used `doc_roughing_factor` there; the arm was dead then and is dead
now. The sentry asserts both halves of that (`depth_cap_mm(Drill, …) == None`
and `OperationConfig::Drill(..).depth_per_pass() == None`).

### The one judgement the plan did not name: `SemiFinish`

The plan named roughing, adaptive, finishing and drill. `PassRole` has three
variants and the profile publishes two milling factors, so `SemiFinish` had to
land somewhere. It reads `doc_finishing_factor`, because the profile's split
is roughing against not-roughing and a semi-finish pass is not roughing. One
shipped operation is affected: `Waterline` (`Contour` / `SemiFinish`). The
clamp is unaffected — it only ever asks with `PassRole::Roughing`.

`doc_finishing_factor` had **no reader in core** before this step. Its only
consumer was the MCP profile dump (`rs_cam_viz/src/app/mcp/project.rs:513`).
S3 is its first.

---

## 2. Why `axial_engagement_mm` and not `axial_doc_mm`

`SimulationCutSample` carries both.

- `axial_doc_mm` is documented at `stock/simulation_cut.rs:173` as the
  **legacy wire name** for axial cutting engagement, and it reads `0.0` on a
  pure-vertical plunge, where the descent goes to `plunge_descent_mm` instead.
- `axial_engagement_mm` (`:177`) is "maximum material height engaged by
  lateral/arc/helix cutting at this sample", and the doc names its consumers:
  the deflection and chip-geometry gates.

Three reasons the criterion reads the second:

1. The rigidity cap bounds how much cutting edge is buried in the work. That
   is the engaged height, which is what this field measures.
2. `session::compute` folds the same field to a maximum for the toolpath's own
   axial figure, so the criterion and the toolpath statistic describe one
   quantity rather than two that usually agree.
3. The deflection gate already reads it, and a load tier whose rows read
   different depth axes is the defect class this programme exists to remove.

The sentry's fixture sets `axial_doc_mm: 0.0` and `axial_engagement_mm: depth`
on every sample, so an arm that reads the wrong field fails rather than
passing by coincidence. That defect was injected and confirmed (§6).

---

## 3. The gate

`crates/rs_cam_core/src/tool_load/depth.rs`, the shape of `deflection.rs`:
`pub fn evaluate(ctx: &ToolpathLoadContext, env: &GateEnv) -> DepthVerdict`.

### Population handling

A sample contributes when it is for this toolpath, `is_cutting`, reports
`axial_engagement_mm > 0.0`, and passes `locality::is_steady_state_for_gate`
— the one predicate the folder invariant names, which is
`!is_phantom_transit && !is_configured_entry`.

The phantom-transit half matters more here than anywhere else in the folder. A
phantom transit sample's dexel reads `stock_top − cutter_z` over
**neighbouring** uncleared stock, so its `axial_engagement_mm` is exactly the
inflated reading this gate would otherwise publish as the cut's depth. A unit
test in the module pins it: a 9.0 mm in-transit sample beside a 0.8 mm cutting
sample yields a peak of 0.8 mm and a population of 1 of 2.

`offered` counts every sample for this toolpath; `contributing` counts what
reached the comparison. `GatePopulation::new(contributing, offered, Samples)`
rides on `SampleEvidence` on both decided arms. **Zero contributing samples is
a vacuous `Within`, not a refusal** — copied from `deflection.rs`, because with
`peak_mm == 0.0` a `Within` at zero depth is the most reassuring thing this
gate can print, so the population has to say it rests on nothing.

### The statistic — one refinement

The brief asked for statistic `PeakHigh`. `CriterionStatus` carries no
statistic field; the only typed slot is `SampleEvidence::statistic`, an
`Option<ChiploadStatistic>`. The gate fills it with the chipload gate's own
two-arm contract — `PeakHigh` when the peak trips the high bound,
`PeakInRange` when it does not — rather than `PeakHigh` unconditionally. The
statistic is the same one either way (a peak read against a high bound, so one
deep excursion is the finding); this reports which side of the bound it landed
on, which is what those two variants mean. `deflection.rs` sets no statistic at
all, so this is the more informative of the two shapes available.

### Refusals

| Case | Verdict |
|---|---|
| drill cycle (`operation_kind.is_drill_kinematics()`) | `Unmodeled(NotApplicableForOp("drill cycle — no continuous engagement"))` — the same clause the three milling gates use |
| no simulation trace | `Unmodeled(SimulationRequired)` |
| no `MachineProfile` | `Unmodeled(NotImplemented)`, mirroring `power.rs` |
| family with no axial cap | `Unmodeled(NotApplicableForOp(…))` — unreachable, the drill arm takes it first; kept so a family added without a factor surfaces as "doesn't apply" rather than as a fabricated bound |
| cap not finite or not positive | `Unmodeled(NotImplemented)` |

### The bound comparison

`boundary::exceeds_high(peak_mm, cap_mm, 0.0)` — the crate's shared boundary
contract, inclusive with the relative epsilon. The tolerance term is `0.0`
because `ToleranceBands` carries no depth dial and S3 adds no axis. A cut
landing exactly on the cap is `Within`, which a module test pins.

`Confidence` is `Approximate` on both decided arms, with a clause formatted
from `peak_mm`, `factor` and `diameter_mm`. The approximation is a property of
the bound — a rule of thumb — not of this cut.

---

## 4. The row, and the tier

`CriterionKind::DepthOfCut`, `label() = "depth of cut"`, `unit() = "mm"`,
placed after `Deflection` and before `GantryPush`.
`is_unmodeled_by_design()` is **false**: this is a real gate with a real
producer, unlike the gantry row beside it.

`ToolpathLoadVerdict` gains `pub depth: DepthVerdict` after `deflection`, and
`milling_criteria()` pushes it after deflection and before the gantry row. It
is assembled at the single assembly site, `tool_load::evaluate_toolpath`, so
the optimizer, the export path and the GUI see one depth verdict.

`DepthVerdict::as_criterion_status` fills `bound = Some(cap.cap_mm())`,
`bound_source = Some(RigidityRuleOfThumb { factor, diameter_mm })`,
`display_peak = Some(peak_mm)` and, on `Exceeds`,
`exceeded = Some(ExceededCriterion::depth_of_cut())`. That new
`ExceededCriterion` states in its `remedy` that the bound is advisory, because
an operator who reads "exceeded" beside an export that went ahead is owed the
reason in the same place.

### Non-gating, end to end

S4 had already written the policy: `RigidityRuleOfThumb::gates_export()` is
false, `CriterionStatus::refuses_export()` reads it, and
`gcode::enforce_exceedance_policy` reports without refusing. S4 had to test
that against hand-built rows because nothing produced the variant. **S3 is its
first producer**, and the sentry's arm (d) now exercises it from a real
report: a hand-set 3.000 mm cut against a 1.5875 mm cap yields `Exceeds`,
`exceeded_criteria()` lists it, and `enforce_load_policy` with
`accept_exceeded: false` returns `Ok(Some(note))` whose note names "depth of
cut" and says "rule of thumb". Add a power exceedance and the same report
refuses, naming both.

`gcode/mod.rs` was not touched beyond one test fixture.

### What the Readiness pill shows on a hand-set deep cut

`ToolLoadReportSummary::exceeds` counts a toolpath with **any** `Exceeds` row,
and the depth row is a real row, so a hand-set deep cut moves that toolpath
out of `within` and into `exceeds`. The Readiness pill therefore reads one
more `exceeds` than it did before S3, and the export still goes ahead with the
note.

The plan accepted this (`PLAN.md` §11): the operator sees the row that is
actually binding. It is the point of the step, not a side effect — a depth
that was silently clamped, or silently taken deeper than the machine's own
rule of thumb, now says so on the face. V3 replaces the pill with the
per-limit rows, at which point the operator sees *which* row it was without
opening anything.

`modeled_count()` also rises by one on a simulated milling toolpath, and
`fully_unmodeled` / `not_applicable` are unchanged: on a drill cycle the depth
row takes `NotApplicableForOp` beside the other three, so
`all_not_applicable` still partitions the same way.

---

## 5. Every construction site and match arm changed

### The one rule for a fixture

A hand-built `ToolpathLoadVerdict` fixture states its gate outcomes by hand.
The depth row takes the **posture of the gates beside it**, so adding the
field changes no fixture's meaning:

- siblings `Unmodeled(reason)` → `DepthVerdict::Unmodeled { reason }`, the same
  reason;
- siblings hand-built `Within` → `DepthVerdict::fixture_within()`, a
  `#[cfg(test)] pub(crate)` constructor on `DepthVerdict` holding a 0.25 factor
  on a Ø6 tool and an empty `SampleEvidence`. It is one constructor rather than
  twenty-four inline literals, and no model reads its numbers;
- fixtures built through the shipped evaluators → the real
  `depth::evaluate(&ctx, &env)`.

### Sites

| File | Sites | What each took |
|---|---|---|
| `tool_load/verdict.rs` (tests) | 16 | 7 mirrored a sibling's `Unmodeled` reason, 9 took `fixture_within()` |
| `tool_load/optimize/strategy/retarget.rs` | 10 | `fixture_within()` — all are hand-built `Within` fixtures |
| `tool_load/optimize/tests.rs` | 5 | `fixture_within()` |
| `tool_load/optimize/strategy/headroom.rs` | 4 | `fixture_within()` |
| `tool_load/optimize/narrative.rs` | 2 | `fixture_within()` |
| `tool_load/optimize/{delta,rank}.rs`, `strategy/grid.rs` | 1 each | `fixture_within()` |
| `diagnostics/tests.rs` | 4 | mirrored: `NotApplicableForOp` (drill), `SimulationRequired` ×2, `StaleSimulation` |
| `gcode/mod.rs` (tests) | 1 | `fixture_within()` — the export-gate fixture's own posture |
| `tests/a_criterion_carries_its_own_bound_g_s4bound.rs` | 2 | the real gate on the measured fixture; `Unmodeled(na())` on the drill one |
| `tests/an_absent_limit_is_visibly_absent_g_gantry.rs` | 2 | same two |
| `tests/gate_population_vacuity_xvac.rs` | 2 | the real gate through the `verdicts` helper (now a 3-tuple); the drill fixture mirrored |
| `tests/{chipload_advisory_disclosure_h4,chipload_abstention_cannot_supersede_g_chipgate,a_weak_bound_cannot_refuse_an_export_g_s4weak}.rs` | 1 each | mirrored `SimulationRequired` |
| `tests/drill_evidence_wording_d3.rs` | 1 | mirrored `NotApplicableForOp("drill cycle")` |

`tests/feed_explanation_record_t1.rs` and
`tests/chipload_report_wording_t12_t15.rs` build through
`evaluate_toolpath`, so neither needed an edit.

### Exhaustive `CriterionKind` matches

Three, all inside `verdict.rs` (`label`, `unit`, `is_unmodeled_by_design`),
plus **one in viz**: `criterion_short_label` at
`crates/rs_cam_viz/src/ui/sim_op_list.rs:1211`, which took
`CriterionKind::DepthOfCut => "depth"` beside `"defl"` and `"gantry"`. No
layout changed. `git status --short` showed the file clean before the edit.

`rs_cam_viz/src/ui/sim_diagnostics.rs` needed **no** edit: its two `kind`
matches are non-exhaustive (`match (status.kind, burn_risk)` with a fallback,
and a `==` comparison). `rs_cam_cli`, `rs_cam_mcp` and
`diagnostics/adapters/from_tool_load.rs` needed none either.

### Two assertions re-blessed, both with the cause

1. `verdict.rs` `modeled_count_ignores_unmodeled`: `2` → `3`. The fixture's
   chipload and deflection rows were modelled before and the depth row joined
   them. **The count moved because a real row joined the tier**, which is the
   change; no verdict moved.
2. `tests/an_absent_limit_is_visibly_absent_g_gantry.rs`
   `the_three_milling_criteria_are_still_there_and_at_least_one_is_modelled`:
   the pinned row ORDER gains `DepthOfCut` between `Deflection` and
   `GantryPush`. The arm's claim is unchanged — the gantry row is still LAST,
   which is what it pins: a row with no bound sits after every row that has
   one.

**No snapshot was re-blessed.** `rs_cam_viz/tests/mcp_wire_surface_pin.rs`
passes unchanged (`2 passed`): it pins MCP tool INPUT schemas, and S3 changes
an output shape.

---

## 6. The sentry, red-first, and two injected defects

`crates/rs_cam_core/tests/the_depth_that_cut_is_a_measured_load_g_s3depth.rs`,
11 arms.

| Arm | Test |
|---|---|
| a | `a_roughing_depth_row_states_the_measured_peak_and_its_bound` |
| a | `the_depth_bound_does_not_gate_and_the_kind_is_not_a_known_absence` |
| b | `each_family_reads_the_factor_its_own_pass_role_implies` |
| c | `a_drill_cycle_carries_the_same_not_applicable_clause_as_the_milling_gates` |
| d | `a_hand_set_depth_above_the_cap_exceeds_but_does_not_refuse_the_export` |
| d | `a_power_exceedance_beside_the_depth_row_refuses_and_names_both` |
| e | `a_trace_with_no_cutting_samples_is_vacuous_not_a_green_zero` |
| f | `the_measured_arm_is_a_modelled_within_with_a_positive_peak` |
| f | `the_measured_trace_varies_so_a_peak_means_something` |
| g | `the_suggest_clamp_still_fires_at_the_helper_s_own_cap` |
| g | `a_drill_operation_has_no_depth_per_pass_for_the_clamp_to_read` |

The measured trace's twelve samples cut 0.40 to 0.95 mm in 0.05 mm steps, so
the peak is a real maximum over a varying population; arm (f) asserts that the
fixture varies before arm (a) reads a peak off it. Arm (b) asserts the three
factors the three families read are three DIFFERENT numbers on the fixture
profile, so it cannot pass while all three arms read one field.

Arm (g) runs the whole Suggest door — `FeedsPreview::build` →
`suggest::apply` → `enforce_invariants` — on a Ø12 pocket asking for 36 mm on
`generic_wood_router`, and asserts the clamp's `capped` and the shipped
`depth_per_pass()` both equal `depth_cap_mm(...).cap_mm()` **exactly**
(`assert_eq!` on `f64`, not a tolerance). Byte-identity of the other Suggest
figures is carried by the apply-door targets in §7.

### Red-first, by construction

```
error[E0432]: unresolved import `rs_cam_core::tool_load::verdict::DepthVerdict`
error[E0433]: cannot find `depth` in `tool_load`
error[E0599]: no variant, associated function, or constant named `DepthOfCut` found for enum `CriterionKind` in the current scope
error[E0599]: no method named `depth_cap_mm` found for struct `RigidityProfile` in the current scope
error: could not compile `rs_cam_core` (test "the_depth_that_cut_is_a_measured_load_g_s3depth") due to 9 previous errors
```

### Defect 1 — the adaptive family reads the roughing factor

`depth_cap_mm`'s adaptive arm returns `self.doc_roughing_factor`. This is BUG 1
of the Suggest audit (workflow `w39ma2j1y`) re-committed on the criterion side.

```
assertion `left == right` failed
  left: 0.25
 right: 2.0
failures:
    each_family_reads_the_factor_its_own_pass_role_implies
test result: FAILED. 10 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

### Defect 2 — the gate reads `axial_doc_mm`

Both the sample predicate and the peak fold read the legacy field.

```
assertion `left == right` failed: the row must report the maximum axial_engagement_mm over the trace's cutting samples
  left: Some(0.0)
 right: Some(0.9500000000000001)
failures:
    a_hand_set_depth_above_the_cap_exceeds_but_does_not_refuse_the_export
    a_power_exceedance_beside_the_depth_row_refuses_and_names_both
    a_roughing_depth_row_states_the_measured_peak_and_its_bound
    the_measured_arm_is_a_modelled_within_with_a_positive_peak
test result: FAILED. 7 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out
```

Both defects were reverted and the file went green.

`depth.rs` also carries nine module unit tests: the peak is a maximum, a cut
past the cap exceeds, a cut exactly on the cap is `Within`, an empty
population is vacuous, a transit sample never sets the peak, no trace asks for
a simulation, a drill does not apply, no machine profile is `NotImplemented`,
and a finishing pass reads the finishing factor.

---

## 7. Verification

Every command through `scripts/cargo_lane.sh`; viz through `-j 2`.

| Command | Result |
|---|---|
| `fmt --all -- --check` | exit 0, no diff |
| `test -p rs_cam_core -q --test the_depth_that_cut_is_a_measured_load_g_s3depth` | `ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_core --lib -q` | `ok. 2530 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out` |
| `--test a_criterion_carries_its_own_bound_g_s4bound` | `ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test an_absent_limit_is_visibly_absent_g_gantry` | `ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test a_weak_bound_cannot_refuse_an_export_g_s4weak` | `ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test chipload_abstention_cannot_supersede_g_chipgate` | `ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test chipload_advisory_disclosure_h4` | `ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test chipload_report_wording_t12_t15` | `ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test drill_evidence_wording_d3` | `ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test feed_explanation_record_t1` | `ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test gate_population_vacuity_xvac` | `ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test predicted_feed_gates_f035` | `ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test step_project_load` | `ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test gcode_phase0_capture` | `ok. 0 passed; 0 failed; 16 ignored; 0 measured; 0 filtered out` |
| `--test a_clamped_feed_ships_at_or_below_its_ceiling_g_feeddown` | `ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test a_feed_lift_caps_at_the_cutting_ceiling_g_t18` | `ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test arc_fit_disposition_a5` | `ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test suggest_feed_matches_final_geometry` | `ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test suggest_power_ceiling_after_pass9_g_suggest_powerstale` | `ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test a_rescaled_feed_stays_inside_the_power_ceiling_g_t15` | `ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test a_published_power_is_at_the_depth_that_cuts_g_s2` | `ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test literature_matrix` | `ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `--test literature_parity` | `ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `clippy -p rs_cam_core --all-targets --features heavy-tests,research,test-support -- -D warnings` | `Finished dev profile [unoptimized + debuginfo] target(s) in 22.53s` — no error, no warning |
| `check -p rs_cam_cli --all-targets` | `Finished dev profile … in 9.49s` |
| `check -p rs_cam_mcp --all-targets` | `Finished dev profile … in 9.06s` |
| `check -p rs_cam_viz -j 2` (lib) | `Finished dev profile … in 0.18s` |
| `test -p rs_cam_viz -j 2 -q --test mcp_wire_surface_pin` | `ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `test -p rs_cam_cli -q` (4 binaries) | `ok. 35 passed`, `ok. 4 passed`, `ok. 9 passed`, `ok. 2 passed`; all `0 failed` |
| `test -p rs_cam_mcp -q` (2 binaries) | `ok. 31 passed; 0 failed`, `ok. 0 passed; 0 failed` |

The core full gate, the whole viz suite and `wanaka_suggest_integration` were
not run.

`rg -l "ToolpathLoadVerdict|CriterionKind|criteria\(\)|fully_unmodeled|ToolLoadReportSummary"
crates/rs_cam_viz/tests/` returns nothing, so `mcp_wire_surface_pin` is the
only viz target this step names — the same result S4 recorded.

`gcode_phase0_capture` and `step_project_load` report zero passing tests on a
plain run: the first is fully `#[ignore]`d and the second is feature-gated.
Per `tests/CLAUDE.md` that marks an instrument, not a break.

### Two interruptions worth recording

1. **`check -p rs_cam_viz --all-targets` is red for reasons outside this
   step.** Another session is mid-flight in `rs_cam_viz/src/controller/**`,
   `mcp_bridge.rs`, `ui/properties/toolpath_panel.rs`,
   `src/controller/tests/mod.rs` and a new `tests/one_start_from_row_g_startfrom.rs`.
   Their errors name `plan_fixpoint`, `PendingGenerateAll`, `FixpointPlan`,
   `rest_heatmap_on`, `PencilConfig` and a `ToolpathId` arity change — none
   touch `CriterionKind`, `ToolpathLoadVerdict` or anything S3 changed. The viz
   LIB compiles clean, and `mcp_wire_surface_pin` builds and passes against it.
2. **`cargo fmt --all` was run in its writing form once**, not only as
   `--check`, to format the new `depth.rs`. It is workspace-wide by
   construction. The peer's in-flight files were already formatted, so no diff
   of theirs gained a hunk: their diffs are content-only (a 47-line addition in
   `session/generation_plan.rs`, an argument change in `ui/overlays/panel.rs`).
   Prefer `fmt -p rs_cam_core` while a tree is shared.

---

## 8. What the plan did not anticipate

1. **`SemiFinish` had no factor named.** §1 states the judgement and the one
   shipped operation it moves (`Waterline`). If a semi-finish DOC factor is
   ever added to `RigidityProfile`, the helper is the one place to change.
2. **`ToolDefinition` has no `diameter` field.** It implements
   `MillingCutter::diameter()`, so the gate calls that. The Suggest clamp reads
   `ToolConfig::diameter`. The two are the same number on a milling cutter, but
   they are different doors, and the helper takes the diameter as an argument
   rather than a tool, so neither door leaks into `machine/`.
3. **`CriterionStatus` carries no statistic**, so the brief's "statistic
   `PeakHigh`" had to land on `SampleEvidence::statistic`, whose type is named
   for the chipload gate. §3 states what shipped and why.
4. **One register row, T-19.** `enforce_load_policy`'s unmodelled branch still
   enumerates three typed verdicts by hand to build its message, while the
   decision to refuse comes from `criteria()`. S1 found the shape and S4
   recorded it; S3 makes it reachable, because there are now five milling rows
   and two of them are invisible to the message builder. A toolpath whose only
   unmodelled row is the depth row refuses and names no criterion. Added as
   **T-19** in `planning/TECH_DEBT_REGISTER.md`, with the fix (one loop over
   `criteria()`).
5. **`crates/rs_cam_core/src/tool_load/CLAUDE.md` is at its 40-line ceiling**,
   so `depth.rs` is not in its file map and the new sentry is not in its
   sentry list. S1 and S4 left the same note. The file is outside this step's
   set; it needs one editing pass that covers all three steps.

---

## 9. Files changed

- `crates/rs_cam_core/src/machine/mod.rs`
- `crates/rs_cam_core/src/feeds/suggest/invariants.rs`
- `crates/rs_cam_core/src/tool_load/depth.rs` (new)
- `crates/rs_cam_core/src/tool_load/mod.rs`
- `crates/rs_cam_core/src/tool_load/verdict.rs`
- `crates/rs_cam_core/src/gcode/mod.rs`
- `crates/rs_cam_core/src/diagnostics/tests.rs`
- `crates/rs_cam_core/src/tool_load/optimize/delta.rs`
- `crates/rs_cam_core/src/tool_load/optimize/narrative.rs`
- `crates/rs_cam_core/src/tool_load/optimize/rank.rs`
- `crates/rs_cam_core/src/tool_load/optimize/tests.rs`
- `crates/rs_cam_core/src/tool_load/optimize/strategy/grid.rs`
- `crates/rs_cam_core/src/tool_load/optimize/strategy/headroom.rs`
- `crates/rs_cam_core/src/tool_load/optimize/strategy/retarget.rs`
- `crates/rs_cam_core/tests/the_depth_that_cut_is_a_measured_load_g_s3depth.rs` (new)
- `crates/rs_cam_core/tests/a_criterion_carries_its_own_bound_g_s4bound.rs`
- `crates/rs_cam_core/tests/a_weak_bound_cannot_refuse_an_export_g_s4weak.rs`
- `crates/rs_cam_core/tests/an_absent_limit_is_visibly_absent_g_gantry.rs`
- `crates/rs_cam_core/tests/chipload_abstention_cannot_supersede_g_chipgate.rs`
- `crates/rs_cam_core/tests/chipload_advisory_disclosure_h4.rs`
- `crates/rs_cam_core/tests/drill_evidence_wording_d3.rs`
- `crates/rs_cam_core/tests/gate_population_vacuity_xvac.rs`
- `crates/rs_cam_viz/src/ui/sim_op_list.rs`
- `planning/TECH_DEBT_REGISTER.md`
- `planning/load_model_2026-09-16/S3_IMPLEMENTATION.md` (this file)

Not committed.
