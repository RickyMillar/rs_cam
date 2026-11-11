# P29-2 design note: `Check` and `LimitId`

Written 2026-11-11 by the implementer, before any product edit. HEAD `2685c84f6` (P29-1).
Status: waits for the lead's answer. Sources: `P29_FINDINGS.md` 4.4, `P29_parts/A_cuts_and_limits.md`
(1.5), `P29_parts/B_metrics_and_display.md` (4.8, 5.1, 5.2). Paths are under
`crates/rs_cam_core/src/`.

## 1. `LimitId`: one list (28 members)

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
