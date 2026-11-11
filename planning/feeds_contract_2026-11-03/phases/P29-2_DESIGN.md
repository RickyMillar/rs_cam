# P29-2 design note: `Check` and `LimitId`

Written 2026-11-11 by the implementer, before any product edit. HEAD `2685c84f6` (P29-1).
Status: approved 2026-11-11 with four answers (section 9). P29-2a is implemented and its gates
pass (section 10); it is not committed by the implementer. P29-2b is not started. Sources: `P29_FINDINGS.md` 4.4, `P29_parts/A_cuts_and_limits.md`
(1.5), `P29_parts/B_metrics_and_display.md` (4.8, 5.1, 5.2). Paths are under
`crates/rs_cam_core/src/`.

## 1. `LimitId`: one list (29 members)

| `LimitId` | Old variants it replaces |
|---|---|
| `SpindlePower` | `LimitKind::Power`, `CriterionKind::Power`, `BindingConstraint::PowerMax`, `RpmLimit::PowerAtRpm` |
| `ToolDeflection` | `LimitKind::Deflection`, `CriterionKind::Deflection`, `BindingConstraint::DeflectionMax`, `AxialBindingConstraint::Deflection` |
| `ForceBudget` | `BindingConstraint::ForceBudget`; `LimitKind::Rigidity` and `CriterionKind::DepthOfCut` under the force-budget rule |
| `DepthFactor` | `LimitKind::Rigidity` and `CriterionKind::DepthOfCut` under the Factor rule (2 x D) |
| `ChipMax` | `LimitKind::ChipBand`, `CriterionKind::Chipload` (high side), `BindingConstraint::ChiploadMax` |
| `ChipMin` | `CriterionKind::Chipload` (low side), `BindingConstraint::ChiploadMin` |
| `RubbingFloor` | `LimitKind::RubbingFloor` |
| `FluteGuard`, `CuttingLength`, `VendorDepthTable`, `CornerCoverage` | the `LimitKind` of the same name |
| `VendorAxial` | `LimitKind::VendorAxial`, `AxialBindingConstraint::VendorAp` |
| `ScallopDepth` | `AxialBindingConstraint::Scallop` |
| `ToolDiameter` | `LimitKind::Diameter` |
| `MoveBudget` | `LimitKind::Runtime` |
| `EngagementFloor` | `LimitKind::EngagementFloor` |
| `CuttingFeedCap` | `LimitKind::FeedCap`, `BindingConstraint::MachineMaxFeed`, `RpmLimit::CuttingCeiling(_)` |
| `PlannerFeedCap` | `BindingConstraint::FeedCap` (`Move::feed_cap`) |
| `KinematicReach`, `PlungeRate` | the `BindingConstraint` of the same name |
| `SpindleMaxRpm`, `ZAxisRate`, `OperatorRpmCeiling`, `ChartRpm`, `VendorRpmMax` | `RpmLimit::{SpindleMax, ZAxisRate, OperatorCeiling, ChartRpm, VendorRpmMax}` |
| `GantryPush`, `DrillChipWelding`, `DrillPeckAdequacy`, `DrillPlungeFeed` | the `CriterionKind` of the same name |

Old variants with NO clean member (the mapping function takes a second argument for each):

1. One old name, two limits: `LimitKind::Rigidity` and `CriterionKind::DepthOfCut` (force budget
   or Factor: the rule or the `BoundSource` decides); `LimitKind::ChipBand` and
   `CriterionKind::Chipload` (maximum or minimum: the `ChipSide` decides);
   `LimitKind::EngagementFloor` (a depth floor and a width floor: one id, the axis is in `unit`).
2. Two old names, one limit: the four names of spindle power; `LimitKind::FeedCap` =
   `BindingConstraint::MachineMaxFeed` = `RpmLimit::CuttingCeiling`.
3. One word, two different limits: `LimitKind::FeedCap` (the machine ceiling) and
   `BindingConstraint::FeedCap` (the planner's `Move::feed_cap`).
4. `DepthVerdict::Reported` (a finish pass, R2): a reading with no bound. Proposal: `limit:
   ForceBudget`, `bound: None`, `state: NoNumber(NoBoundOnThisCut)`. It is a scope exemption of
   P28, not a geometry reason, so it must not be `NotApplicable`. P29-3 lists it for a ruling.
5. `AxialBindingConstraint::Unbounded` and `SafeBandEmpty` are states, not limits: no member.
6. `RpmLimit::CuttingCeiling(source)`: the source (machine or the op dial) goes to the basis.
7. `BoundSource` (7) is not a limit list. It joins the basis (section 3).

## 2. `Check`

```rust
pub struct Check {
    pub limit: LimitId,
    pub value: Option<f64>,   // the number that was judged, in `unit`
    pub bound: Option<f64>,   // the limit at that cut, in `unit`
    pub unit: &'static str,
    pub side: BoundSide,      // Ceiling | Floor (the sign rule of the margin)
    pub margin: Option<f64>,  // share of the bound, positive inside: `Limit::margin`'s rule
    pub state: CheckState,    // Within | Exceeds | NoNumber(NoNumberReason) | NotApplicable(Geometry)
    pub basis: Option<Basis>, // where the BOUND comes from
    pub binding: bool,        // this limit set the value (envelope pick, RPM choice, modulator)
    pub population: Option<GatePopulation>, // samples contributing / offered; None for a planned cut
}
```

- `value` for a gate verdict over samples is the statistic the gate judged: the peak, or the
  median for the chip low side. It is today's `CriterionStatus::display_peak`. The histogram
  (`MetricDistribution`) is not in `Check`; it is built from per-sample checks later (4.5).
- `population` is the existing `GatePopulation` (contributing, offered, unit), unchanged.
- `NotApplicable(Geometry)`: closed enum `NoRadialEngagement | NotAnOffsetRing | NoVerticalFeed`.
- `NoNumberReason`: `NoForceLine | NoSimulation | StaleSimulation | NoVendorRow |
  NoSteadySample | NoArc | NoToolBody | NotModelled | NoBoundOnThisCut`. The mapping from
  `UnmodeledReason` (11 variants) names every variant. `NotApplicableForOp(text)` on a drill
  maps to `NotApplicable(NoRadialEngagement)`; `NotImplemented(text)` maps to `NotModelled`.
- `Check` is `Serialize` only (`Basis` holds `&'static str`).

## 3. The basis join (`Basis::of_bound(&BoundSource)`, exhaustive)

| `BoundSource` | `Basis` | `setting` | `gates_export` |
|---|---|---|---|
| `MachinePowerCurve { rpm }` | `Machine(PowerCurve)` | machine | yes |
| `VendorChipBand { .. }` | `Printed(RowRef(source.row_id()))` | vendor row | yes |
| `DeflectionBudget` | `Rule(DEFLECTION_BACKOFF_TARGET_ROW)` | tool | yes |
| `StockToLeave { mm }` | `Operator { law: "radial stock to leave" }` | operation | yes |
| `RigidityRuleOfThumb { .. }` | `Machine(Rigidity)` | machine | no |
| `MachineForceBudget { .. }` | `Machine(Rigidity)` (the envelope's `budget_basis` needs the machine) | machine | no |
| `DrillEnvelope` | `Derived { law: "drill envelope of the material", from: [] }` | material | yes |

`Basis` gains two methods, `setting()` and `gates_export()`; the sentry holds them equal to
`BoundSource` for the 7 variants. No `Basis` variant is added (its JSON is on the wire).
NOT joined: the numbers inside `BoundSource` (rpm, mm, factor, the budget's ae and fz) that
`BoundSource::clause()` prints. In this step the old type stays the renderer. Question (a).

## 4. Where it lives; what projects

New folder `limits/` (`mod.rs`: `LimitId` and the mappings; `check.rs`: `Check`, the states;
`project.rs`: the projections; `CLAUDE.md`). `P29_FINDINGS` 4.5 names `limits::check_all`. It
reads `feeds` and `tool_load`; neither reads it in this step. The crate file's folder table
gets one row.

Projections in this step (old type -> `Check`; no old type is built from `Check`, because a
rebuild of `CriterionStatus` or `Placement` risks a byte):

- `CriterionStatus::check()` (covers the six gate verdicts and the drill gates).
- `SafeEnvelope::checks_at(CutPoint)`: one `Check` per `Limit`, with `Limit::margin`,
  `judged_value` and the `Placement` binding.
- `RpmChoice::checks()`: one per `RpmLimitValue`, `binding` on the chosen one.
- `ModulationSummary`: only `LimitId::of_binding` (a share per limit, no value and bound).

## 5. Commits

- P29-2a: `limits/`, the four mappings (plus `AxialBindingConstraint`), `Basis::of_bound`,
  the three projections, the sentry. No surface reads it. Moved pins: none expected.
- P29-2b: MCP additive. `get_tool_load_report` gains `checks` per toolpath;
  `get_safe_envelope` gains `checks` at the stated point; `get_suggest_rationale` gains the
  RPM checks. The keys are added in the viz handler JSON, not inside `ToolLoadReport` or
  `SafeEnvelope` (their serde output is compared in tests). The only moved pin:
  `crates/rs_cam_viz/tests/snapshots/mcp_wire_surface.json`, and only if the three tool
  descriptions name the new key (the snapshot holds input schemas and descriptions, not
  replies). I will `rg` the viz and cli tests for whole-reply comparisons before the edit.

## 6. Sentry (2a): `tests/every_limit_is_a_limit_id_p29_2.rs`

1. Each old enum maps to `LimitId` in a match with no wildcard; the test walks `ALL` of each
   enum (adds `ALL` where an enum has none) and asserts every `LimitId` has at least one source.
2. Fixtures (a simulated pocket, the envelope of that pocket, its `RpmChoice`): each projected
   `Check` equals the old record on value, bound and state; `binding` equals the old binding;
   the margin equals `Limit::margin`.
3. `Basis::of_bound(b).setting() == b.setting()` and the same for `gates_export`, 7 variants.

## 7. Gates (lane script, `-j 2`, foreground)

| # | Command | Time |
|---|---|---|
| 1 | `cargo fmt --all -- --check` | 10 s |
| 2 | the new sentry | 1 min build, seconds to run |
| 3 | `--test a_criterion_carries_its_own_bound_g_s4bound --test the_envelope_keeps_each_limit_k2b --test every_rpm_cap_is_a_named_limit_p21 --test folder_maps_name_real_files_f140` | about 1 min |
| 4 | `test -p rs_cam_core -q --lib` | 2 min 12 s (measured in P29-1) |
| 5 | FM1 (`--features heavy-tests`) | 1 min 15 s (measured) |
| 6 | full clippy | 3 min (measured) |
| 2b | `test -p rs_cam_viz -q -j 2 --test mcp_wire_surface_pin --test the_limit_rows_read_their_own_bound_g_ownbound --test ribbon_and_mcp_diagnostic_ids_n4`; clippy | viz build unknown, I estimate 5 to 10 min; I ask first |

## 8. Questions

(a) The numbers inside `BoundSource` have no place in `Basis`. Options: a transitional field
`Check::bound_detail: Option<BoundSource>` until P29-10 (my choice: lossless, and the card text
keeps one writer), or a new `Basis` variant now.
(b) `DepthVerdict::Reported`: `NoNumber(NoBoundOnThisCut)` as in item 4, or another state?
(c) `AxialBindingConstraint` in the mapping now (my choice: yes, it is 5 variants), or later?
(d) The name: `limits/` as a new core folder, or `feeds/check.rs`?

## 9. The lead's answers (2026-11-11)

(a) `Check::bound_detail: Option<BoundSource>`, transitional; it goes at P29-10.
(b) `DepthVerdict::Reported` is `NoNumber(NoBound)`: a P28 scope exemption, not geometry; P29-3
lists it; it is never `NotApplicable`.
(c) `AxialBindingConstraint` is in the mapping.
(d) A new folder `limits/`. No signature of the folder holds an operation type, a family or a
pass role; the folder imports nothing from `compute/` or `session/`. The sentry scans it.
Also: the list has 29 members (the note said 28). A second mapping argument is a typed value.

## 10. 2a result (2026-11-11, on top of `2685c84f6`; not committed)

### What changed

New: `crates/rs_cam_core/src/limits/` (`mod.rs`, `check.rs`, `project.rs`, `CLAUDE.md`) and
`crates/rs_cam_core/tests/every_limit_is_a_limit_id_p29_2.rs`.

Edited (7 files; `git diff --stat -- crates`: 182 insertions, 58 deletions):

| File | Change |
|---|---|
| `src/lib.rs` | `pub mod limits;` |
| `src/feeds/envelope.rs` | `LimitKind::ALL`; `Edge::is_floor` (the floor test of `place`, moved); `place` now folds the margins of `each_placing_margin` (the same loop, the same order, the same comparisons); `placing_margins` and `planned_reading` (`pub(crate)`, for `limits::of_envelope`); `axis_reading` (the match of `judged_value`, moved); `RIGIDITY_FACTOR_LAW` (the law text of the Factor limit, as a constant) |
| `src/feeds/sourced.rs` | `Basis::setting`, `Basis::gates_export`, `DRILL_ENVELOPE_LAW` |
| `src/feeds/mod.rs` | `RpmLimit::ALL` |
| `src/feeds/cutter_constraints.rs` | `AxialBindingConstraint::ALL` |
| `src/tool_load/verdict.rs` | `CriterionKind::ALL`, `BindingConstraint::ALL` |
| `crates/rs_cam_core/CLAUDE.md` | the `limits/` row; 29 folders |

No surface reads `limits/`. No public signature of an old type changed. No viz, cli or mcp
file changed.

### Differences from sections 1 to 8

- `NoNumberReason` has two more members than the note: `NoMaterialRemoved`
  (`AllSamplesAirCutOrRapid`) and `NotJudgedHere` (an envelope limit that `place` does not
  judge at the point: a fixed axis, a per-point limit, the drawn move-budget curve). The second
  is a scope fact of today, like `NoBound`; P29-3 lists it.
- `basis_of_bound` is a function of `limits/` (not a method of `Basis`), so `feeds/sourced.rs`
  does not name a gate type. `Basis::setting` and `Basis::gates_export` are methods of `Basis`.
- `of_envelope` takes the margins from the envelope's own placement walk. For that, `place`
  was split into the walk and the fold. This is the one edit of a product code path in 2a.
- The envelope check of the exact move budget has a margin and no value or bound (the
  envelope holds `budget / moves - 1` only).
- `ModulationSummary` has no projection function: `LimitId::of_binding` is the whole mapping.
- `LimitKind::ChipBand` maps to `ChipMax` only (the envelope judges the peak chip against the
  band; its floor is the `RubbingFloor` kind).

### Gates (each `timeout <s> scripts/cargo_lane.sh <args> -j 2`, foreground)

| # | Command | Result | Time |
|---|---|---|---|
| 1 | `cargo fmt --all -- --check` | pass | seconds |
| 2 | `--test every_limit_is_a_limit_id_p29_2` | 7 passed | 27 s build, 0.1 s |
| 3 | `-q --test every_limit_is_a_limit_id_p29_2 --test a_criterion_carries_its_own_bound_g_s4bound --test the_envelope_keeps_each_limit_k2b --test every_rpm_cap_is_a_named_limit_p21 --test folder_maps_name_real_files_f140 --test a_pick_outside_the_envelope_warns_k2b --test the_envelope_finding_reads_the_gate_feed_p20i --test one_cut_gives_one_load_on_every_door_p29_1` | 8 binaries, 40 passed, 0 failed | 30 s |
| 4 | `test -p rs_cam_core -q --lib` | 2795 passed, 1 failed, 13 ignored. The red is `the_envelope_meets_its_time_budget` (the timing trap). Alone, three runs: pass, 126, 120 and 119 ms (119 and 130 ms before this step; budget 300 ms) | 1 min 51 s |
| 5 | `--features heavy-tests --test feeds_matrix_instrument_fm1` | 2 passed: the 1000 cells and the simulation subset equal the committed files | 1 min 10 s |
| 6 | full clippy (root `CLAUDE.md` command) | clean | 2 min 50 s |

Inject check of the sentry: `of_criterion` with the ceiling as the bound of a low-side chip
row turns claim 2 red; `of_envelope` with `binding` on the wrong limits turns claim 3 red.
Both were restored.

No pin moved. No FM1 cell moved.

### Open items

- `Check::bound_detail` goes at P29-10.
- `NoBound` and `NotJudgedHere` are scope facts for the P29-3 list.
- `Basis::setting` is exact only for a basis from `basis_of_bound`: every `Rule` gives "tool".
- Not projected: `EntrySpike`, `EntryLoadObservation`, `PlungeStressWarning`,
  `CutForceReading`, `CutEfficiency`, `RuleResult`, `PowerFigure`, `DeflectionPrediction`,
  `MeasuredPower` (part B section 5.2). They hold a value and a bound and no state of the
  three; they join when their surface reads `Check` (P29-10) or when P29-6 judges the entries.
- P29-2b (MCP additive) is not started. Before it: `rg` the viz and cli tests for a
  comparison of a whole reply.

## 11. 2b (2026-11-11, on top of `8c8671f07`; not committed)

### Plan (the lead's rulings)

- Additive `checks` keys in three MCP replies, built in the viz handler from
  `rs_cam_core::limits`. The old fields stay; no serde of `ToolLoadReport` or `SafeEnvelope`
  changes.
- One sentence about `checks` in each of the three tool descriptions. The wire snapshot
  `crates/rs_cam_viz/tests/snapshots/mcp_wire_surface.json` then moves by those three strings
  only. The lead sees the diff before any bless.
- Gates: the named viz tests, the viz `--lib` once, full clippy. Each viz test run waits for
  the operator's go.

### What is implemented

| File | Change |
|---|---|
| `crates/rs_cam_viz/src/app/mcp/generation.rs` | Three builders: `load_report_checks_json(&ToolLoadReport)` (per toolpath `{ toolpath_id, checks }`, the checks in the order of `criteria()`, each from `limits::of_criterion`); `envelope_checks_json(&SafeEnvelope)` (`limits::of_envelope` at `envelope.stated`, `null` with no stated point); `rpm_checks_json(Option<&RpmChoice>)` (`limits::of_rpm_choice`, an empty list with no choice). Each handler adds the key `checks` at the top level of its reply. |
| `crates/rs_cam_viz/src/mcp_server.rs` | One sentence at the end of the description of `get_tool_load_report`, `get_safe_envelope` and `get_suggest_rationale`. |
| `crates/rs_cam_viz/src/app/mcp/tests.rs` | Three lib tests (the sentry of 2b): `the_load_report_checks_repeat_the_criterion_rows_p29_2b`, `the_envelope_checks_repeat_the_operator_placement_p29_2b`, `the_rpm_checks_repeat_the_rpm_limits_p29_2b`. |

`git diff --stat -- crates`: 3 files, 234 insertions, 3 deletions. No core file changed.

The reply shapes:

- `get_tool_load_report`: `{ summary, load_report, per_depth_pass, checks: [ { toolpath_id, checks: [Check] } ] }`.
- `get_safe_envelope`: `{ toolpath_id, toolpath_name, envelope, checks: [Check] | null }`.
- `get_suggest_rationale`: `{ toolpath_id, toolpath_name, basis, rationale, checks: [Check] }`.
- A `Check` on the wire: `limit` (snake_case `LimitId`), `value`, `bound`, `unit`, `side`
  (`ceiling` or `floor`), `margin`, `state` (`{ "state": "within" | "exceeds" | "no_number" |
  "not_applicable", "reason": .. }`), `basis`, `bound_detail` (only when a gate row has a bound
  source), `binding`, `population`.

Difference from the plan: the sentry is three lib tests in `app/mcp/tests.rs`, not a new
integration test file. The handlers are methods of `RsCamApp`, and the builders are private to
`app::mcp`; a lib test can call them, an integration test cannot. So the viz `--lib` run is the
gate that holds 2b.

### State

- `cargo fmt --all -- --check`: pass.
- `cargo check -p rs_cam_viz --all-targets -j 2`: pass (45 s).
- NOT RUN (they wait for the operator's go): `test -p rs_cam_viz -j 2 --test
  mcp_wire_surface_pin --test the_limit_rows_read_their_own_bound_g_ownbound --test
  ribbon_and_mcp_diagnostic_ids_n4`; `test -p rs_cam_viz -j 2 --lib`; full clippy.
- Expected: `mcp_wire_surface_pin` goes RED (three description strings). No bless before the
  lead sees the diff.

### Docs

No folder file names the reply keys of these three tools (`rs_cam_mcp/CLAUDE.md`,
`rs_cam_viz/src/app/CLAUDE.md`, `app/mcp/CLAUDE.md` searched). `FEATURE_CATALOG.md` names
`get_tool_load_report` in the kinematic utilization line only. A catalogue line for the
`checks` keys is the lead's call (the catalogue defines shipped claims).

### 2b gates (2026-11-11, the operator's go; no `rs_cam_gui` running)

| # | Command (`timeout <s> scripts/cargo_lane.sh ..`, `-j 2`, foreground) | Result | Time |
|---|---|---|---|
| 1 | `test -p rs_cam_viz -j 2 --test mcp_wire_surface_pin --test the_limit_rows_read_their_own_bound_g_ownbound --test ribbon_and_mcp_diagnostic_ids_n4` | 2 + 10 + 6 = 18 passed, 0 failed | 54 s |
| 2 | `test -p rs_cam_viz -j 2 --lib` | 706 passed, 0 failed, 2 ignored; the three `_p29_2b` tests pass | 50 s |
| 3 | full clippy (root `CLAUDE.md` command) | clean | 3 min 03 s |

`cargo fmt --all -- --check`: pass.

Correction of the plan: NO pin moved. `mcp_wire_surface_pin` is GREEN. The snapshot
`mcp_wire_surface.json` holds each tool's name and input schema (with the doc text of the
parameter types), not the tool descriptions; the three description strings are not in it
(`rg` for "Per-toolpath tool-load report" in the snapshot: no match). So the description
sentences are in the code and no snapshot diff exists to review or to bless.

State: P29-2b is complete in the tree (3 viz files), not committed by the implementer.
