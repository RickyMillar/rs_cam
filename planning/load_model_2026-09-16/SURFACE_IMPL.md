# The limits surface — implementation plan

Written 2026-09-18. This is the document `PLAN.md` §10 said was missing: the
mechanism per step, the sentry each step needs, and the blast radius. It
builds on `PLAN.md` §3–§7 and §11 (Reading A), `SURVEY_UI.md` §4–§5 and
`REVIEW_DESIGN.md` §5–§6, and it does not repeat them. Read those first when a
step needs its reasons.

The operator's requirements, in force throughout:

- Every limit is the same kind of thing: **0 to limit**.
- "Limit calculated from X" belongs in a hover `(i)`, not on the face.
- Every limit **traces back to the setting that set it**.
- *"A multimeter, not a verbal explanation."*
- After a simulation: the measured value and the limit. Before one: the
  prediction. Two stages, two places, no shared axis (`PLAN.md` §3).

The two rulings of 2026-09-18: **drop the `≈` from the face**; **reinstate the
power bar as a 0-to-limit bar**.

---

## 0. The two halves and when each may start

| Half | Files | May start |
|---|---|---|
| Core | `tool_load/verdict.rs`, `tool_load/*` gates, `feeds/suggest/*`, `gcode/mod.rs` (`enforce_load_policy`) | after T-15 lands |
| Viz | `ui/sim_diagnostics.rs`, `ui/readiness_panel.rs`, `ui/feeds/compare.rs`, `ui/properties/*` | after the `gen_sim_rest_ux_2026-09-18` wave lands its W2 (`ui/properties/`), and coordinated on file ownership |

The other session's plan leaves `feeds/` and `tool_load/` to this programme
(`gen_sim_rest_ux_2026-09-18/PLAN.md:24`). Its viz footprint is the
controller, the MCP bridge and the rest-analysis rows of the properties panel.
It does not name `ui/feeds/` or `sim_diagnostics.rs`.

---

## 1. Core — in order

### S2. Power at the operating point that ships (PLAN step 4; class 1)

**Defect.** `FeedsResult.power_kw` (`feeds/mod.rs:2487`) is computed inside
`calculate` at the calculator's geometry. `clamp_dpp_to_rigidity` then lowers
the DPP in `enforce_invariants`. The published power describes a depth that
will not be cut — on a Ø12 roughing cut, 3.5× deeper. `explore.rs:1276` reads
it.

**Mechanism.** One public core function,
`feeds::power_at_operating_point(operation, tool, material, machine) ->
Result<PowerFigure, PowerUnmodeled>` where `PowerFigure { required_kw,
available_kw }`. It builds `tool_load::power::PowerTerms` from the operation's
FINAL `ap`, `ae`, RPM and feed with the same `effective_d` binding `calculate`
passes to `power_model_terms`, and states the ceiling as
`power_at_rpm(rpm) × safety_factor` (COMMANDED axis). It refuses, typed, when
the material has no `Kc` or the operation has no DPP. **T-15's pass 10 is
built on this same function**; if the T-15 agent wrote a private helper, this
step makes it public and gives it the doc. `FeedsResult.power_kw`'s doc then
says plainly: *at the calculator's geometry; for the shipped figure call
`power_at_operating_point` on the final operation.*

**Sentry.** `a_published_power_is_at_the_depth_that_cuts_g_<code>.rs`: a Ø12
roughing fixture where the rigidity clamp moves the DPP; the function's
`required_kw` at the final operation is lower than `FeedsResult.power_kw` by
the depth ratio (show the arithmetic); the drill family refuses typed; a
material with no `Kc` refuses typed.

**Blast radius.** New function; one doc change; no caller moves yet. The bar
in V4 is its first display reader.

### S1. Gantry push as a visibly absent row (PLAN 4b)

**Mechanism.** `CriterionKind::GantryPush` with `label() = "gantry push"`,
`unit() = "N"`. `ToolpathLoadVerdict::criteria()` pushes one
`CriterionStatus` for it with `state: Unmodeled`,
`unmodeled_reason: NotImplemented(<clause>)`. The clause cites the register
item — *"no machine-side thrust rating; register T-10"* — never a
`planning/…` path (`REVIEW_DESIGN.md` §6.3: 117 of 231 cited paths are dead).
`lateral_cutting_force` stays where it is; this step adds no physics.

**Blast radius.** `CriterionKind::` has 47 references in core and 9 in viz.
Every exhaustive match fails to compile until it names the new variant; about
a dozen need a decision (label, unit, MCP wire label). `tests/mcp_wire_surface_pin.rs`
pins the wire shape — the new row is an additive change; the pin is updated
with the cause named, not widened.

**Sentry.** `an_absent_limit_is_visibly_absent_g_<code>.rs`: the row is in
`criteria()` for a milling toolpath; its state is `Unmodeled`; its clause
names T-10; `enforce_load_policy` does not refuse on it; `exceeded_criteria()`
never lists it. Non-vacuity: the same report still lists the three milling
criteria.

### S4. Every criterion carries its bound and its provenance (the backbone)

**Defect.** Two of the three caps the badge draws against are built in the
GUI (`sim_diagnostics.rs:936-947`): the power cap as `max_power_kw ×
safety_factor` and the deflection cap as `DEFLECTION_SAFE_LD_RATIO = 4.0`
(`:1126`). The second is an L/D RATIO drawn beside a gate that judges
MILLIMETRES against `EXCEEDS_BOUND_MM = 0.200` (`tool_load/deflection.rs:64`).
Different quantity, same row — class 2. The GUI is not an alternate data
model (`rs_cam_viz/CLAUDE.md`).

**Mechanism.** Two fields on `CriterionStatus` (`verdict.rs:583`):

```rust
/// The bound `display_peak` is judged against, in `unit`. `None` when the
/// criterion is unmodelled or the bound does not exist.
pub bound: Option<f64>,
/// Where `bound` came from, typed. The hover formats this; it never
/// retypes a number.
pub bound_source: Option<BoundSource>,
```

and one typed enum:

```rust
pub enum BoundSource {
    /// `MachineProfile::power_at_rpm(rpm) × safety_factor`.
    MachinePowerCurve { rpm: f64, safety_factor: f64 },
    /// The matched vendor row's floor or ceiling; carries `ChipBoundsSource`.
    VendorChipBand(ChipBoundsSource),
    /// `tool_load::deflection::EXCEEDS_BOUND_MM`.
    DeflectionBudget,
    /// `RigidityProfile::doc_roughing_factor` (or `adaptive_doc_factor`) × D.
    /// A rule of thumb with no published source. Does not gate.
    RigidityRuleOfThumb { factor: f64, diameter_mm: f64 },
    /// The drill gates' own bounds.
    DrillEnvelope,
}
impl BoundSource {
    /// The SETTING that moves this bound: the machine, the tool, the
    /// material, the vendor row. The operator's rule 3, and the durable half
    /// of provenance (REVIEW_DESIGN §6.3).
    pub fn setting(&self) -> &'static str;
    /// One clause, formatted from the values it carries at render time.
    /// Never a literal number.
    pub fn clause(&self) -> String;
    /// Whether an exceedance of this bound may refuse an export.
    pub fn gates_export(&self) -> bool;
}
```

Each gate's `as_criterion_status` fills both from the bound it actually
judged. The deflection row then reads mm against 0.200 mm, as the gate does.

`gates_export` lives on the SOURCE, not on the kind, because the reason a
bound may not gate is the bound's provenance, not the quantity. The day the
machine is measured, `RigidityRuleOfThumb` is replaced by a measured variant
and the row gates with no UI change (`PLAN.md` §11).

**Gate.** `enforce_load_policy` (`gcode/mod.rs:667`) reads
`report.exceeded_criteria()` and refuses on any. It gains one filter: an
exceedance whose `bound_source.gates_export()` is `false` is reported but does
not refuse. The refusal message lists what refused; a second line lists what
exceeded without refusing, so the operator sees both.

**Sentry.** `a_criterion_carries_its_own_bound_g_<code>.rs`: every milling
criterion in a simulated report has `bound.is_some()` when modelled; the
deflection bound equals `EXCEEDS_BOUND_MM`; the power bound equals
`power_at_rpm × safety_factor` at the toolpath's RPM; every `clause()` is
non-empty and contains the formatted bound; `setting()` is one of the four
names. A second file, `a_weak_bound_cannot_refuse_an_export_g_<code>.rs`:
an exceeded criterion whose source does not gate exports; an exceeded power
criterion still refuses; the refusal message names both.

**Blast radius.** `CriterionStatus` is built in each gate's
`as_criterion_status` (chipload, power, deflection, three drill gates) and
read by `verdict_badge`, `criterion_detail`, the MCP report and the CLI
report. Constructor sites are compiler-found. `mcp_wire_surface_pin` moves,
additively, with the cause named.

### S3. Depth of cut as a post-simulation criterion, non-gating (PLAN §11 Reading A)

**Mechanism.**

- `CriterionKind::DepthOfCut`, `label() = "depth of cut"`, `unit() = "mm"`.
- `DepthVerdict` in a new `tool_load/depth.rs`, the shape of `ChiploadVerdict`
  (about 100 lines): `Within { peak_mm, bound_mm, population }`, `Exceeds {
  .. }`, `Unmodeled(UnmodeledReason)`; `as_criterion_status` fills `bound` and
  `bound_source: RigidityRuleOfThumb { factor, diameter_mm }`.
- The producer reads `axial_engagement_mm` per cutting sample; statistic
  `PeakHigh` (one excursion is the finding, as for deflection); carries
  `GatePopulation` so zero cutting samples is X-VAC, not a clean pass.
- **The bound is the factor the clamp used**, read from one shared core
  helper that both `clamp_dpp_to_rigidity` and this producer call:
  `adaptive_doc_factor` for the adaptive families, `doc_roughing_factor`
  otherwise, × `tool.diameter`. Not a constant (`REVIEW_DESIGN.md` W2).
- **Finishing passes and drills are `Unmodeled(NotApplicableForOp)`**: no depth
  cap applies to a finishing pass (`clamp_dpp_to_rigidity` is roughing-only),
  and a drill has no radial engagement. The row paints `—` with the reason.
- Before a simulation the depth stays a rationale entry
  (`RationaleReason::RigidityFactor`). No pre-simulation row.

**Sentry.** `the_depth_that_cut_is_a_measured_load_g_<code>.rs`: a simulated
roughing toolpath yields a `DepthOfCut` row with `bound == factor × D` and a
`display_peak` equal to the trace's peak `axial_engagement_mm`; a finishing
toolpath yields `Unmodeled(NotApplicableForOp)`; a hand-set DPP above the cap
yields `Exceeds` AND the export is not refused (the S4 filter, exercised on
the row it exists for); a trace with no cutting samples is vacuous.

**Blast radius.** The `CriterionKind` matches again (S1 already opened them);
one new gate module; `ToolpathLoadVerdict` gains a field; the report summary
counts (`within` / `exceeds` / `fully_unmodeled`) include the new row — check
`ToolLoadReportSummary`'s tests and the Readiness pills, which V3 replaces.

---

## 2. Viz — in order, after the properties wave lands

### V1. The face loses the `≈`; the hover keeps the reason

`verdict_badge` (`sim_diagnostics.rs:1140`): the `Approximate` arms stop
adding `\u{2248}` and stop using `WARNING_MILD`; colour follows `state` alone.
`verdict_tooltip` keeps `Approximate(why)` and prints *which input is
approximate*. `Confidence`'s doc (`verdict.rs:198-202`) is rewritten: the
ruling of 2026-09-18, and why — the behaviour carries the meaning (a weak
bound cannot gate, S4), so the face does not have to. `criterion_detail`
(`sim_op_list.rs`) follows the same rule.

**Sentry.** Extend the chip-verdict file or add one: no painted text on the
badge strip contains `≈`; the hover for an `Approximate` status contains the
reason string.

### V2. The row reads its own bound; the caption carries the setting

`verdict_badge` computes the percent from `status.bound`, not from a `cap`
argument; the three cap bindings in `draw_toolpath_section` and
`DEFLECTION_SAFE_LD_RATIO` are deleted. The row caption prints
`bound_source.setting()` (W1); the population prints on the row (W4); the
statistic prints through `ObservedStatistic::label` (S2 of the review). The
hover prints `bound_source.clause()` — formatted from values, never a literal
(`REVIEW_DESIGN.md` §6.3).

**Sentry.** Rendered, not source-scanned (the corridor sentry is the model):
the deflection row's percent equals `peak / EXCEEDS_BOUND_MM`; no row's text
contains `L/D`; every modelled row's hover contains its `setting()`.

### V3. Readiness answers its own question

The `"Tool load"` `check_row` and its three `CountPill::verdict` calls
(`readiness_panel.rs:196-221`) are replaced by the per-limit rows —
`draw_tool_load_badges` extended to take the Readiness geometry (one 560-point
column). This satisfies the rule that a new panel replaces something. The
triage headline stays above the rows (review S4).

**Sentry.** The Readiness page paints one row per `CriterionKind` for the
selected toolpath and no `within / exceeds / unmodeled` pill; the existing
Readiness sentries in `rs_cam_viz/tests/` keep passing.

### V4. The power bar returns, as a 0-to-limit bar

`ui/feeds/compare.rs`: one row, the same idiom as the chip verdict row,
reading `feeds::power_at_operating_point(final operation)` (S2) — so the bar
shows power at the depth that will be cut — against `available_kw`. Hover:
`BoundSource::MachinePowerCurve` clause and setting. A refusal paints the
clause, never a zero.

**The sentry `no_power_gauge_returns_to_the_card_g_chipverdict` is REPLACED,
not deleted** (`SURVEY_UI.md` "What must NOT happen next"). Its successor
tests the reason: across the shipped presets × species × diameters the bar's
utilisation is not uniformly low — at least one shipped recipe reads over
50 % (the review's own stated condition: "a quarter of recipes pass half
scale"), and the median is under the peak. A ban whose reason expired becomes
a test of the reason. The `≈`-free and one-verdict-row arms of that file stay.

### V5. LOOK

Run the GUI on the Wanaka project. Compare the predicted rows on the Feeds
tab with the measured rows after a simulation. Decide with the operator
whether anything else is needed — the distribution (PLAN 4d) is the
interesting part and the least valuable, and it stays "probably not yet".

---

## 3. Sequence and hand-offs

```
T-15 (running)        pass 10; power re-evaluated at the final state
  └─ S2               make that evaluation public: power_at_operating_point
S1                    gantry push: an absent limit is visibly absent
S4                    bound + BoundSource on CriterionStatus; gate filter
S3                    depth of cut, post-sim, non-gating
── wait for gen_sim_rest_ux W2 ──
V1  V2  V3  V4        one agent, one file-ownership list, after a fresh
                      git status on every viz file it will touch
V5                    LOOK, with the operator
```

Each step: one Opus agent, design decided in the brief, sentry red-first, no
commit, orchestrator reviews the diff by path and commits. Register entries
for anything found on the way.

---

## 4. What this plan refuses to do

- No histogram, no bins, no second copy of the trace (`PLAN.md` 4d) until V5
  says the comparison leaves a question unanswered.
- No number typed into a hover string. Every figure is formatted from the
  value it describes at render time.
- No `planning/…` path in an operator-facing string. Register items only.
- No GUI-owned limit. Every bound comes from the gate that judged it.
- No gate on `0.20 × D` while that number has no source.
