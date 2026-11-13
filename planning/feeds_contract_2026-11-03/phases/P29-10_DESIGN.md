# P29-10 design: one record on every surface

Written on 2026-11-13 by a design agent for the lead. Documents only: no product code changed, no
cargo command ran (another agent built in the tree). HEAD `0038733a5`. Every `path:line` is at
that commit (`git grep -n <pattern> 0038733a5 -- crates/`), with the symbol name as the stable
pointer.

Change list row (`P29_FINDINGS.md` section 6): "Display: every surface reads `Check`. Planned and
sample points share the chart. The force field draws for every op. Viz and MCP stop assembling
values." Acceptance: "one record on every surface". Display only: no cutting number moves.

## 0. The answer in short

1. Core gets ONE row type, `limits::CheckRow`. A row holds a `Check` and its subject: a declared
   cut, a cut group, the stated point, the worst measured point or the RPM choice. It also holds
   its caveats, its exemption row, its tone and its text. Three projections build the rows, each
   one exhaustive. Every surface reads the rows. No surface computes a margin, a percent, a worst
   row or a state word.
2. The compiler holds the projection. A new `LimitId`, `CutGroup`, reason, caveat or gate field
   does not compile until the projection answers it. A source scan holds the viz side: no viz
   match on a limit or state enum, except the one `Tone` to `Role` match.
3. The Suggest card stays quiet. The face gets at most ONE new status row: the worst row that
   exceeds, with its caveat in words. Every row sits in one closed `Details`, "Limits · n".
4. The planned rows describe the PROPOSAL (what ⚡ writes), not the cut that the op holds now. On
   the operator file, idx 1 holds 10 x 5.5 mm, 4.11 times the budget (`P29_FINDINGS.md` section
   11), and its proposal is inside. So the drawer has two parts, "As the op stands" and "If you
   apply ⚡" (operator question O6).
5. Planned cuts and simulation samples are marks on one width x depth plane. The force field moves
   to core and draws for every op with a force line. Where the force budget does not judge the
   cut, the budget line is faint and says so.
6. The wire has one key, `limit_rows`. The three P29-2b `checks` keys, `envelope.cut_checks` and
   `Check::bound_detail` go. Section 3.9 names each break.
7. Eight commits, 10a to 10h. 10a to 10g move no cutting number and no check state. 10h adds a
   judgement (the deflection of a fixed depth axis), so the lead decides it (question L3).

## 1. Scope

### 1.1 The sources

`P29_FINDINGS.md` sections 4, 6, 7, 10; `P29_parts/B_metrics_and_display.md` section 5 (19
surfaces, 28 record types); `P29-6_DESIGN.md` section 13 (F6); `P29-7_DESIGN.md` sections 16.8 and
16.9; `P29-8_DESIGN.md` sections 4.4 and 13; `P29-9_DESIGN.md` sections 11, 15 and 17;
`NEXT_SESSION.md` (top section); the "Open" lists of commits `eb6a4a4a7` (7c), `d7e6527b8` (8d),
`82ba2805f` (9c/9d), `01a2ff1cc`, `e6b587a2e` and `0038733a5`; the folder files `limits/`,
`feeds/`, `tool_load/`, `stock/`, viz `ui/`, `ui/feeds/`, `ui/properties/`, `ui/components/`,
`app/mcp/`, and the crate files of `rs_cam_mcp` and `rs_cam_cli`.

### 1.2 A verdict for each logged item

| # | Item | Verified at | Verdict | Commit |
|---|---|---|---|---|
| 1 | The chip rows of `cut_checks` reach no card | `ui/feeds/compare.rs` and `why.rs` read no `cut_checks`; `rg cut_checks crates/rs_cam_viz/src` gives only test hits and the MCP description. The two cases (zigzag Pocket first slot `ChipMax` 0.1704 against 0.127 mm; geodesic rough body `ChipMin` 0.1229 against a point band 0.127 mm) come from the add door on the default machine (`P29-7_DESIGN.md` 16.9), not from a repo file | In: the card drawer lists every planned row; the face shows the worst one that exceeds | 10f |
| 2 | A `MayOccur` note fills the card header | `envelope.rs:2964` `may_occur_notes` pushes a sentence into `notes`; viz `ui/feeds/envelope.rs:1036` prints `notes.first()` through `force_budget::note_face` (`feeds/force_budget.rs:625`). `note_face` has a short face for two note kinds only (the computed-width assumptions, the force-budget floor) and returns every other note whole. The notes are pushed in this order (`envelope.rs:1654` to `:1694`): the no-depth reason clause, the `MayOccur` sentences, the move-budget notes (`move_budget_not_met_note`, `:4348`), the width assumptions, the floor note, the trap-2 conflict (`MOVE_BUDGET_CONFLICT`, `:4337`). So the no-depth clause, a move-budget note and the trap-2 note fill the header the same way | In, by structure. (a) The `MayOccur` fact is on the row already (`CutCheck::may_occur_row`): the sentence leaves `notes`, the row carries the caveat. (b) `notes: Vec<String>` becomes `notes: Vec<EnvelopeNote>`, a closed enum (`NoDepth(reason)`, `MoveBudgetNotMet { ceiling_mm }`, `ComputedWidth`, `ForceBudgetFloor`, `MoveBudgetConflict`), with `face()` and `text()` in one match each. A new note kind does not compile without a face | 10b |
| 3 | The depth card shows one bound | No depth card exists at HEAD (`CUT_METRIC_ORDER`, `state/simulation.rs:200`, has four metrics). The one bound is on the depth row of "Other limits": since 6c it prints the sample with the largest depth over its own cap (`tool_load/CLAUDE.md`, sentry `the_depth_that_cut_is_a_measured_load_g_s3depth`), and its bound is that sample's cap. `metric_distribution(DepthOfCut)` still bins with one ceiling, and no surface reads it | In, as the F6 merge (P29-6 F6): the depth predicate and the force reading are one predicate. One `ForceBudget` row per group; its value and bound are the force in N against the budget (one bound for every sample); the depth over its own cap goes to the detail. The depth histogram ceiling goes. Operator question O3 | 10a, 10e |
| 4 | `exceeds_breakdown` names no group | `tool_load/verdict.rs:883` `ExceedsEntry` has `gate` and `side`, no `group`; `ExceededCriterion` carries `group` (`verdict.rs:782`) | In: `ExceedsEntry` gains `group` (wire addition) | 10c |
| 5 | The triage `DedupKey` merges a headline and a group finding of one id | `stock/sim_triage.rs:142` `DedupKey` has no group; the class-B loop (`sim_triage.rs:560`) keys a tool-load diagnostic on `(id, toolpath_id)`; `from_tool_load.rs:98` `group_diagnostics` reuses the headline id and only changes the message | In: `DedupKey` gains `group: Option<CutGroup>`; a group finding keeps its own entry | 10c |
| 6 | The `NotJudged` group is visible only in MCP | `GroupCell::NotJudged` (`tool_load/groups.rs:137`) reaches MCP through `load_report.per_toolpath[].groups`; the GUI reads `criteria()`, which drops it (ruling Q5). `criteria()` also holds the S6 own-material rows (`verdict.rs:693` to `:696`) and the drill rows (`:675` to `:685`) | In: the projection destructures the whole `ToolpathLoadVerdict` and reads every `GroupVerdicts` cell, the own-material verdicts and the drill gates, not `criteria()`; the inspector gets a closed "Cut groups · n" drawer | 10a, 10e |
| 7 | `GroupVerdicts::headline_only` is a second constructor for tests | `verdict.rs:407`; every caller is test code (core unit tests, 10 core integration tests, viz `app/mcp/tests.rs` and the test module of `ui/sim_diagnostics.rs`); viz and CLI reach `test-support` through their dev-dependencies (`rs_cam_viz/Cargo.toml:47`, `rs_cam_cli/Cargo.toml:28`) | In: it moves behind `#[cfg(any(test, feature = "test-support"))]` and takes a name that says so. No product door can call it | 10c |
| 8 | `M4` (two sample depths) | Power and chip read `axial_doc_mm` (`feeds/engagement.rs:243`); depth, deflection and force read `axial_engagement_mm` (`tool_load/depth.rs:259`, `deflection.rs:189`, `cut_force.rs:145`) | Split. Display part in: each group row states the depth it read (caveat `SampleDepth`). The numeric part (one depth) moves gate values, so it is not display: re-file to P29-11 with a probe (question L4) | 10a, 10e |
| 9 | Entry value rows, lower-bound rows, `NoNumber` rows with reasons, the earlier leave on the power row only | `CutCheck::own_depth`, `entry_no_number` (`envelope.rs:1105`); `NoNumberReason` (`limits/check.rs`); `no_edge_reason` (`envelope.rs:2610`); 9c commit message "the earlier leave reaches only the power row" | In: each fact becomes a typed caveat with one core clause, and a margin face with a floor of "< 0.1 %" (section 3.3). Nothing refuses. The deflection row of a fixed depth axis is 10h (question L3) | 10a, 10f, 10h |
| 10 | Where viz and MCP assemble values | Section 2.3: 27 sites | In: each site is deleted or reads a row; the chart geometry sites stay and are named | 10d to 10g |
| 11 (new) | The lineage reads no `Check` | `trace/lineage.rs:92` `MeasuredLineage` holds `binding: LimitKind` and `judged: JudgedValue` (the worst sample placed on the envelope); the panel `ui/properties/lineage.rs` and MCP `get_toolpath_lineage` print them | In: `MeasuredLineage` holds one `CheckRow` (`Subject::MeasuredPoint`, built by `of_judged_value` and the binding). Wire: `measured.judged` and `measured.binding` become `measured.row` (break) | 10e |

### 1.3 Exemption rows and open items that name P29-10

| Row or item | Where | Verdict |
|---|---|---|
| `M4` (`closes_in` "P29-10 (one depth predicate, P29-6 F6)") | `limits/exemptions.rs` | Narrow: the display states the depth of each row. Re-file the numeric part: `closes_in` "P29-11 (a probe: the share of samples where the two depths differ)". Text only; no pin moves |
| `S-NotJudgedHere` (`closes_in` "P29-10") | `limits/exemptions.rs`; markers `feeds/envelope.rs` (`no_edge_reason`), `limits/project.rs` (`of_envelope`) | Two arms. The arm "the deflection on a fixed depth axis" closes in 10h (a value check at the fixed depth). The arm of the stated point (`of_envelope`: a curve or a per-point edge with no margin at the point) stays unless the implementer measures no instance on the four files (section 9, item 9). The lead rules (L3) |
| `Check::bound_detail` ("It goes at P29-10") | `limits/check.rs:184`, `limits/CLAUDE.md` open items | Goes in 10a: the projection writes `BoundSource::clause` into the row detail, and no reader needs the numbers |
| P29-6 F6 ("P29-10 can merge the two rows") | `P29-6_DESIGN.md` section 13 | In (item 3); measure the claim first (section 9, item 4) |
| `NEXT_SESSION.md` line 56: the GUI envelope passes no dressups | `0038733a5` | Closed: `FeedsInputs::context` builds the one context with the toolpath's dressups; viz test `the_gui_reads_the_session_suggest_door_p29_9` compares `cut_checks` with `session.safe_envelope` |

### 1.4 Out of scope

- No cutting number moves: no feed, speed, depth, width, proposal, verdict state or G-code byte. A
  commit that moves one stops the work.
- The histograms, the over-time series, the heat map, the limit curves, the speeds grid and the
  nomogram walls stay (`P29_FINDINGS.md` 4.4: built FROM checks or from a law over a grid, not
  replaced by one record).
- `CriterionStatus`, `Placement` and `JudgedValue` stay. The export gate and the load policy read
  them. Their deletion is P29-last (question L5).
- The diagnostics adapter (`diagnostics/adapters/from_tool_load.rs`) keeps its messages; only the
  group key changes (question L9).

## 2. What exists today

### 2.1 The records

| Record | Built at | Holds | Readers |
|---|---|---|---|
| `limits::Check` | `limits/check.rs:158`; projections `limits/project.rs` | limit, value, bound, unit, side, margin, state, basis, `bound_detail`, binding, population | MCP only |
| `feeds::envelope::CutCheck` | `envelope.rs:1105`, filled by `read_declared_cuts` (`:2480`) | cut, `may_occur_row`, `own_depth`, `entry_no_number`, `check`; one per declared cut and planned limit, at the proposal | MCP `get_safe_envelope` (inside `envelope`), CLI `suggest` (inside `envelope`), the viz parity test |
| `SafeEnvelope::notes` | `envelope.rs:1655`, `:1694` (`may_occur_notes`, `:2964`), `:4473`, `:4526` | strings | viz card header, MCP, CLI |
| `CriterionStatus` | `tool_load/verdict.rs:1345`; `criteria()` | kind, group, state, peak, bound, bound source, population, exceeded | every sim surface, Readiness, the export gate, MCP `checks` through `of_criterion` |
| `GroupVerdicts` / `GroupCell` | `verdict.rs:390`, `groups.rs:137` | one cell per gate and group: `Headline`, `NoSamples`, `Judged`, `NotJudged` | MCP (`load_report` serialised), the diagnostics adapter (Exceeds only) |
| `CutForceReading` | `tool_load/cut_force.rs` | peak N, budget N or none, population | the force card, MCP |
| `RpmChoice` | `feeds/mod.rs` | rpm, binding, limits | card hover, MCP `basis.rpm`, MCP `checks` |
| `MeasuredLineage` | `trace/lineage.rs:92` | `binding: LimitKind`, `judged: JudgedValue` | lineage panel, MCP `get_toolpath_lineage` |
| `SuggestWarning::OutsideEnvelope` | `feeds/suggest.rs` | binding, margin | card face line, diagnostic `project.outside_envelope` |

### 2.2 The surfaces

"Reads" says whether the surface reads a `Check` or assembles from raw fields.

| Surface | File and line | Reads | Assembles |
|---|---|---|---|
| SAFE CUT header | `ui/feeds/envelope.rs:1014` `header`, `:1036` | `SafeEnvelope::notes` | picks the first note |
| ⚡ hover | `ui/feeds/envelope.rs:968` `apply_hover` | `proposal_binding`, `Limit::binding_cut`, `Placement` | formats the binding sentence |
| Envelope plot | `ui/feeds/envelope.rs` (`EnvelopeScene`, `draw_plane`, `draw_bar`) | `boundary`, `proposal`, `stated`, `operator` | limit names (`limit_name`), marks |
| Nomogram | `ui/feeds/explore.rs` | `FeedsExplain`, `CutEfficiency` | chip lines `feed / (rpm x flutes)` (`:247`, `:356`, `:767`, `:815`): chart geometry |
| Suggest card face and `Details` | `ui/feeds/compare.rs:124` `build_card`, `why.rs` | `PowerReading`, `CutEfficiency`, `SuggestWarning`, `FeedsWarning` | power share, headroom phrase, chip verdict, chip floor test, outside-envelope percent |
| Pills | `ui/properties/pills.rs` | `FieldApplyPreviews` | nothing: a pill shows a value, not a limit. No change |
| Lineage panel | `ui/properties/lineage.rs` | `Lineage` (`measured.binding`, `measured.judged`) | formats the judged value |
| Sim verdict line, "Other limits" | `ui/sim_diagnostics.rs:1146` `limit_rows`, `:1189` `draw_limit_rows` | `criteria()` | percent, burn test, colour, status word, tooltip percent |
| Sim cut-metric cards | `ui/sim_diagnostics.rs:1958`, `:2136`; `state/simulation.rs:347` | `MetricDistribution`, `LimitRow` | glyph, peak percent, rank, titles (`CutMetricSpec::of`, `:1627`) |
| Sim force readings | `ui/sim_diagnostics.rs:2027` `force_readings` | `CutPoint3` | min and max of depth and width |
| Sim width x depth chart | `ui/sim_cut_chart.rs:657`; `state/simulation/force_field.rs` | `SafeEnvelope`, `CutPoint3`, `budget_n`, `ForceField` | over-budget test (`:423`), the planned dot at `env.stated` (`:668`, `:876`), the field |
| Sim op list flags | `ui/sim_op_list.rs:1133` | `criteria()` | short labels (`:1267`), detail sentence (`:1280`) |
| Warnings tab, Readiness cards and chips, pre-flight, export wizard | `ui/sim_diagnostics.rs:573`, `ui/readiness*.rs`, `ui/preflight.rs` | `SimulationTriage` findings (`Diagnostic`) | nothing; the messages come from core |
| Readiness tool load | `ui/readiness_panel.rs:315` `worst_limit_rows`, `:355` `row_outranks` | `criteria()` per toolpath | worst row by `peak / bound` |
| MCP `get_safe_envelope` | `app/mcp/generation.rs:1072` | `SafeEnvelope` (with `cut_checks`), `of_envelope` (`checks`, `:1128`) | caps the grid |
| MCP `get_tool_load_report` | `app/mcp/generation.rs:358` | `ToolLoadReport`, summary, `of_criterion` (`checks`, `:408`, built at `:1221`), `per_depth_pass` | adds `group` to each check by hand (`:1236`) |
| MCP `get_suggest_rationale` | `app/mcp/generation.rs:251`, `suggest_basis_json` `:98` | `CutterOpProfile`, `of_rpm_choice` (`checks`, `:298`) | the `basis` object field by field |
| MCP `get_toolpath_diagnostics`, `get_project_diagnostics` | `app/mcp/diagnostics.rs` | `Diagnostic` | utilisation percents (`:236` to `:251`, a kinematics reading, not a limit) |
| MCP `get_toolpath_lineage` | `state/freshness.rs` `toolpath_lineage_reply` | `Lineage::to_json` | nothing |
| CLI `suggest` | `rs_cam_cli/src/suggest.rs:153` | `SafeEnvelope` JSON (with `cut_checks`) | no `checks` key: a parity gap with MCP |
| CLI `project` | `rs_cam_cli/src/project.rs:216` `ToolpathSummaryEntry`, `:707` load report | `ProjectVerdict`, findings, `LoadPolicyOutcome` advisories | no per-toolpath limit record at all: a parity gap |

### 2.3 The assembly sites (item 10)

Each site decides WHAT to show from raw numbers, so two surfaces can differ for one state.

| # | File:line @0038733a5 | Symbol | What it assembles | After P29-10 |
|---|---|---|---|---|
| 1 | `ui/sim_diagnostics.rs:1234` | `pct_of_bound` | peak / bound x 100 | reads `CheckRow::face` |
| 2 | `ui/sim_diagnostics.rs:1439` | `verdict_tooltip` | a second percent | reads `CheckRow::detail` |
| 3 | `ui/sim_diagnostics.rs:1217` | `burn_floor` | the chip floor from `BoundSource` (repeats `of_criterion`) | deleted: the `ChipMin` row holds the floor |
| 4 | `ui/sim_diagnostics.rs:1146` | `limit_rows` | `burn_risk` from the HEADLINE chip verdict, applied to a row by its group | deleted: a `ChipMin` row is its own row |
| 5 | `ui/sim_diagnostics.rs:1293` | `verdict_face` | colour and text from `LoadState` and percent | reads `Tone` and `face` |
| 6 | `ui/sim_diagnostics.rs:1394` | `limit_status` | role from `gates_export` | reads `Tone` |
| 7 | `ui/sim_diagnostics.rs:1351` | `limit_data_row_with` | the "depth is reported" sentence | reads the `NoNumber(NoBound)` clause |
| 8 | `ui/sim_diagnostics.rs:2311` | `status_glyph` | advisory colour | reads `Tone`; the out-of-band share stays (a histogram fact) |
| 9 | `ui/sim_diagnostics.rs:2351` | `peak_text` | percent of bound, of floor, of budget | reads the row of its limit |
| 10 | `ui/sim_diagnostics.rs:1627` | `CutMetricSpec::of` | titles, display units, scale (a wildcard arm over `CriterionKind`) | reads `LimitId::text()` |
| 11 | `ui/sim_diagnostics.rs:1788` | `card_rank` | order by `CriterionKind` | order of the rows |
| 12 | `ui/sim_diagnostics.rs:2027` | `force_readings` | min and max of sample depth and width | stays: a range of samples, not a limit (named) |
| 13 | `ui/readiness_panel.rs:355` | `row_outranks` | worst row by peak / bound | reads `CheckRow::margin` through one core rank (`CheckRow::worse_than`) |
| 14 | `ui/sim_op_list.rs:1267` | `criterion_short_label` | a second vocabulary ("advance/t", "defl") | reads `LimitId::text().short` |
| 15 | `ui/sim_op_list.rs:1280` | `criterion_detail` | a state sentence | reads `detail` |
| 16 | `ui/sim_cut_chart.rs:423` | `over_budget_dots` | sample force > budget | reads the sample state from core (`CutPoint3::over_budget`) |
| 17 | `ui/sim_cut_chart.rs:648` | `dot_hover` | force against budget text | reads core text |
| 18 | `state/simulation.rs:449` | `build_cut_metric_set` | `fz = steady_feed / (rpm x flutes)` by hand | core builds the field through `AdvancePerToothMm` |
| 19 | `state/simulation.rs:418` | `build_cut_metric_set` | the field only under `budget_line` | core field for every op (section 3.8) |
| 20 | `state/simulation/force_field.rs:51`, `:93` | `ForceField::build` | a core law on a grid, in viz | moves to core |
| 21 | `ui/feeds/compare.rs:954`, `:1031` | `power_row`, `power_face` | required / available | reads the body `SpindlePower` row if equal (section 9, item 5) |
| 22 | `ui/feeds/compare.rs:692` | `headroom_phrase` | deflection headroom percent | stays in P29-10 (a `CutEfficiency` reading, P29-11) |
| 23 | `ui/feeds/compare.rs:987` | `POWER_CAUTION_SHARE` 0.7 | a viz threshold that makes a problem row | goes: the face shows a power row only when the row exceeds or a caveat holds. The constant has no source (F201 class). Operator question O1 |
| 24 | `ui/feeds/why.rs:746` | `chipload_min_row` | a second `ChipMin` judgement, only for `PassRole::Finish` | deleted: the body `ChipMin` row of every role |
| 25 | `ui/feeds/why.rs:1158` | `suggest_line` (`OutsideEnvelope`) | `-margin x 100` | reads the stated-point row |
| 26 | `ui/feeds/envelope.rs:1036` | `header` | the first note | reads the notes that remain (10b) |
| 27 | `app/mcp/generation.rs:1221`, `:1261`, `:1274` | `load_report_checks_json`, `envelope_checks_json`, `rpm_checks_json` | three hand-built `checks` keys; a `group` key added by hand | deleted: one `limit_rows` door |

Named and kept (chart geometry or not a limit): `ui/feeds/shared.rs:173` (`feed / (rpm x flutes)`,
the nomogram input), `ui/feeds/explore.rs` chip lines, `force_readings` (#12), `headroom_phrase`
(#22), MCP `per_depth_pass` (`app/mcp/simulation.rs:1017`), `suggest_basis_json` (a basis record,
not a limit). Core sites outside viz and MCP: `diagnostics/adapters/from_tool_load.rs` formats kW
and um from verdicts (`:616`, `:703`); `gcode` load-policy advisories format from
`ExceededCriterion`. Question L9.

### 2.4 Per-surface matches that can leave a limit out

`CutMetricSpec::of` (wildcard arm `Criterion(kind)`), `card_rank`, `criterion_short_label`,
`limit_status`, `status_glyph`, `row_outranks`, `pct_of_bound`, `over_budget_dots`, `why::suggest_line`
(per `SuggestWarning`), `limit_name` in `ui/feeds/envelope.rs` (per `LimitKind`). A new limit
compiles today and shows on no viz surface, or shows with a wrong word. That is the failure the
operator ruled out on 2026-11-12 ("blatantly obvious ... across all movement").

## 3. The design

### 3.1 One row type

New file `limits/row.rs`. It imports nothing from `compute/` or `session/` (folder invariant 1).

```rust
/// THE display row (P29-10): one check and what a reader needs to show it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckRow {
    pub subject: Subject,
    pub check: Check,
    /// Closed list; empty for a plain row. Order: the order of `Caveat::ALL`.
    pub caveats: Vec<Caveat>,
    /// The `EXEMPTIONS` row that holds a `NotJudged`, `MayOccur`, `NoBound` or
    /// `NotJudgedHere` row. `None` for every other row.
    pub exemption: Option<&'static str>,
    pub tone: Tone,
    /// One line, at most 60 characters (IC1): "Entry ramp · force <p> % of budget".
    pub face: String,
    /// The hover: value, bound, unit, basis clause, caveat clauses, population, and the
    /// operating point the row read (depth, width, rpm, feed or chip).
    pub detail: String,
}

pub enum Subject {
    /// A declared cut before the simulation, at the proposal depth and width (P29-7 7c).
    /// A body, slot or cleanup row reads the proposal speeds (P29-9 ruling, 2026-11-13).
    /// An entry row reads the stored entry feed over the stored speed (P29-8 8d), so its
    /// `detail` names that feed. `at`: the point on the width x depth plane, `None` for
    /// an axial cut (a plunge) or a cut with no width.
    Planned { cut: CutKind, occurs: Occurs, at: Option<PlanePoint> },
    /// The operator's stated point against one limit of the boundary (`of_envelope`):
    /// the cut as the op stands.
    Stated,
    /// The samples of one cut group after the simulation. `material`: `None` for the
    /// stock population, the material label for an S6 own-material verdict.
    Group { group: CutGroup, headline: bool, samples: usize, material: Option<String> },
    /// The worst sample placed on the envelope (`MeasuredLineage`, K7).
    MeasuredPoint,
    /// The RPM choice.
    Rpm,
}

/// `compute::catalog::Presence` seen from `limits/` (invariant 1): no row id here,
/// the row id is `CheckRow::exemption`.
pub enum Occurs { Always, MayOccur }

pub enum Caveat {
    /// An entry at its own depth: the depth per pass of a ramp (`OwnDepthRule`).
    WorstCase,
    /// The value is a lower bound of the cut (`BodyDepth::AtLeast`, `OnlyALowerBound`).
    LowerBound,
    /// The chip reads a stored whole-mm/min feed (`StoredFeedChip`).
    StoredFeed,
    /// The cut may not occur (`Presence::MayOccur`).
    MayOccur,
    /// The bound does not gate an export (`Basis::gates_export` false).
    Advisory,
    /// The gate stated an empty population (X-VAC).
    NoSamples,
    /// M4: the sample depth the row reads.
    SampleDepth(SampleDepth),
    /// A finish pass judged at the earlier op's leave (`BodyDepth::EarlierLeave`).
    AtEarlierLeave,
}

pub enum Tone { Within, Exceeds, ExceedsAdvisory, NoNumber, NotApplicable }
```

`CutCheck` is deleted. Its four fields map into `Subject::Planned`, `caveats` and `exemption`;
`entry_no_number` becomes the `detail` clause of the `NoNumber` row. `SafeEnvelope::cut_checks:
Vec<CutCheck>` becomes `SafeEnvelope::rows: Vec<CheckRow>` (the planned rows and the stated-point
rows). `Check` stays the inner record unchanged except `bound_detail` (goes).

### 3.2 Three projections, each exhaustive

All in `limits/row.rs`, all pure, all called once per memo build (never per frame):

1. `rows_of_envelope(&SafeEnvelope) -> Vec<CheckRow>`: the envelope writes its planned rows in
   `read_declared_cuts` through `CheckRow::planned(..)` (one constructor), then appends
   `of_envelope(env, stated)` as `Subject::Stated` rows when `stated` is `Some`.
2. `rows_of_verdict(&ToolpathLoadVerdict) -> Vec<CheckRow>`: destructures the whole
   `ToolpathLoadVerdict` with no `..`, as `mark_simulation_stale` does (`verdict.rs`): the headline
   fields, `drill_gates`, `material_split`, `cut_force`, `groups` and the report-only fields (each
   named, with a comment that it gives no row). Inside, it destructures `GroupVerdicts` with no
   `..` (`headline, chipload, power, deflection, depth, cut_force`) and walks `CutGroup::ALL` for
   each gate. Every cell gives one row:
   - `Headline`: the headline field through `of_criterion`.
   - `Judged`: the verdict through `as_criterion_status` then `of_criterion`.
   - `NotJudged { row, reason, samples }`: state `NoNumber(NotJudgedHere)`, `exemption` = the row,
     the reason in `detail`.
   - `NoSamples`: state `NoNumber(NoMaterialRemoved)`.
   - (Overruled by section 14 ruling 1: no merge; `depth` gives the `ForceBudget` row as the gate
     writes it and `cut_force` gives no row.) `depth` and `cut_force` give ONE `ForceBudget` row per group (F6, O3): the force N against the
     budget N is the value and the margin; the depth over its own cap is in `detail`. The STATE is
     the depth gate's state, because the depth gate is the criterion that warns and it applies
     the gate tolerance; the force reading has no state of its own. Section 9 item 4 measures that
     the two agree in state and in sign of the margin; where they differ, the merge stops. Without
     a budget (a finish pass, `budget_line` gives `None`) the row is `NoNumber(NoBound)` with
     `exemption` `S-NoBound`.
   - `material_split`: one row per own material and gate (`Subject::Group { material: Some(..) }`),
     and one `NoNumber(NoForceLine)` row per not-judged material count (S6).
   - `drill_gates` give `Drill` group rows; the gantry-push row gives its known absence.
   - A `NotApplicable` row (a milling gate on a drill cycle) goes only to the "Cut groups" drawer,
     never to "Other limits", the face or Readiness. Today `limit_rows` hides it
     (`ui/sim_diagnostics.rs:1146`), and a drill shows its three drill rows; that stays.
   A new verdict field, gate field, `GroupCell` variant or `CutGroup` does not compile until the
   projection answers it.
3. `rows_of_rpm_choice(&RpmChoice) -> Vec<CheckRow>`: `of_rpm_choice` with `Subject::Rpm`.

The toolpath name joins outside `limits/`: `ProjectSession::limit_rows(index) -> LimitRows`
(`{ envelope: Vec<CheckRow>, verdict: Vec<CheckRow>, rpm: Vec<CheckRow> }`) is the one door for the
GUI memo, MCP and the CLI, as `suggest_slots` is for the context (question L7).

### 3.3 One writer of each text

- `LimitId::text(self) -> LimitText { noun, short, unit, scale }`: one match, no wildcard. It
  replaces `CutMetricSpec::of` titles and scales (deflection in um, scale 1000), `criterion_short_label`
  and the viz `limit_name`. `CriterionKind::label` stays for the old records.
- `NoNumberReason::clause`, `Geometry::clause`, `Caveat::clause`: one match each, no wildcard.
- The margin face, one function `margin_face(&Check) -> String`:
  - a ceiling: "{value / bound} % of {noun}" ("force <p> % of budget"; a ramp at 1.87 times the budget, `P29-8_DESIGN.md` section 13, prints 187 %);
  - a floor that holds: "{value / bound} % of floor"; a floor that fails: "{|margin|} % under floor";
  - the kit units rule allows 0 to 1 decimals for a percent (`ui/components/CLAUDE.md`). A margin
    under 0.1 % prints "< 0.1 %", never "0 %". So a 0.02 % row reads "chip min < 0.1 % under floor ·
    stored feed". No tolerance hides it, and no invented constant judges it.
- The face: "{cut or group label} · {margin face}", then the first caveat word when one holds
  ("worst case", "lower bound", "stored feed", "may not occur", "advisory"). The detail holds every
  clause.

### 3.4 One tone rule

`Tone::of(&Check, &[Caveat])`, one match over `CheckState`:

- `Within` gives `Within`. `NoNumber(_)` gives `NoNumber`. `NotApplicable(_)` gives `NotApplicable`.
- `Exceeds` gives `ExceedsAdvisory` when the basis does not gate an export, OR when any caveat
  among `WorstCase`, `LowerBound`, `StoredFeed`, `MayOccur` holds. Else `Exceeds`.

A caveat is a fact of the record, not a threshold, so the rule holds no constant. K6 holds: no tone
refuses. Viz maps `Tone` to `Role` in ONE match (`ui/components/` `role_of_tone`): `Exceeds` to
`Danger`, `ExceedsAdvisory` to `Caution`, `Within` to `Ok`, `NoNumber` and `NotApplicable` to
`Unknown`. This keeps today's sim rule (`limit_status`, gating = Danger, advisory = Caution).

### 3.5 What a future author does to add a mechanism

1. A new `LimitId`: the compiler stops at `scope`, `planned_limit_of`, `gate_of` (today) and at
   `LimitId::text` (new). Every surface then shows it with its word and unit; no viz edit.
2. A new `CutKind` or `CutGroup`: the compiler stops at `CutKind::label`, `CutKind::group` and the
   `ByGroup` walk of `rows_of_verdict`.
3. A new gate (a field of `GroupVerdicts`): the destructure of `rows_of_verdict` does not compile.
4. A new reason or caveat: its clause match does not compile.
5. The viz side: the scan `no_surface_matches_a_limit_p29_10` (section 6) fails on any viz match on
   `LimitId`, `LimitKind`, `CriterionKind`, `CutGroup`, `CheckState` or `LoadState` outside
   `role_of_tone`. So `limit_name` (`ui/feeds/envelope.rs`, a match on `LimitKind`) must read
   `LimitId::text` (10g), and `CutMetricSpec::of` must go (10e).
6. A new envelope note kind: `EnvelopeNote::face` does not compile (10b).

### 3.6 The Suggest card stays quiet

Today the face shows one status row per problem (`compare.rs:361`, `build_card`): chip, power over 70 %, the
engaged diameter, the calculator warnings, the finish chip floor. The planned rows show nowhere.

After 10f (sketch, rail width):

```
SAFE CUT                         ⚠ <short note face>   ⚡   <- header: EnvelopeNote::face only
[width x depth plot: proposal, stated, planned marks]
...speed rows...
⚠ <cut> · <limit> <margin face> · <caveat>                  <- NEW, only when a row exceeds
▸ Recommended · n                                           <- unchanged
▸ Limits · n · k over                                       <- NEW closed Details
```

The drawer "Limits · n", one `DataTable` in two parts. The first part is the op as it stands (the
`Subject::Stated` rows: the stated point against each limit). The second part is what ⚡ writes
(the `Subject::Planned` rows at the proposal), grouped by cut in declaration order, then the RPM
rows:

```
AS THE OP STANDS
Stated point  force          <margin face>           exceeds · advisory
Stated point  flute guard    <margin face>           within
IF YOU APPLY ⚡ (proposal ap x ae)
Body          force          <margin face>           within
Body          chip max       <margin face>           within
Body          deflection     N/A                     not judged here (S-NotJudgedHere)
Slot ring     force          <margin face>           exceeds · advisory
Entry ramp    force          <margin face>           exceeds · worst case
Entry ramp    chip min       < 0.1 % under floor     exceeds · stored feed
Entry helix   force          <margin face>           within · may not occur
RPM           spindle max    binds                   within
```

(The sketch shows the shape only. No number in it is measured.)

Rules: the face takes ONE row, the worst row that exceeds, from either part, by
`CheckRow::worse_than` (a stated row outranks a planned row: it is the cut of today; then a
non-advisory row outranks an advisory row; then the lower margin). The drawer label counts the
rows and the rows that exceed. A row that is `NoNumber` never reaches the face. The planned part
says "proposal" in its title, so a reader cannot take a planned row for the cut of today.
`chipload_min_row` and the 70 % power share go (sites 23, 24). The `OutsideEnvelope` face line
(site 25) and the ⚡ hover (`apply_hover`, `ui/feeds/envelope.rs:968`) read the stated row and the
binding planned row.

### 3.7 The simulation inspector

- "Other limits" and the cards read `rows_of_verdict` for the headline group, plus every row of
  another group that exceeds (ruling Q5 stays the rule of the default view).
- A new closed `Details` under "Other limits": "Cut groups · n", every group row of every gate,
  `NotJudged` with its exemption row and `NoSamples` included (item 6). It draws nothing while
  closed.
- The verdict line, the op list flags and Readiness read the rows and `CheckRow::worse_than`.
- `ExceedsEntry.group` (item 4); `DedupKey.group` (item 5): a group finding is its own triage entry.
- M4: a group row of power or chip says "removed layer depth"; a row of force or deflection says
  "engaged edge depth" (in `detail`, not on the face).

### 3.8 One plane, and the force field for every op

Both charts are width x depth planes on the kit (`components::chart::PlaneChart`): the envelope
plot (Passes tab, `ui/feeds/envelope.rs`) and the simulation chart (`ui/sim_cut_chart.rs`). Today
the simulation chart draws one planned dot at `env.stated` (`:876`), and the envelope plot draws
the proposal, the stated point and the candidate.

Design:

- Core `feeds::plane` gives the position of each declared cut at a point:
  `PlaneMarks::at(&DeclaredCuts, CutPoint, describes_rows)` (10g). The envelope holds two sets,
  `marks_at_proposal` (the point of the planned rows; each row holds its point, `subject.at`) and
  `marks_at_stated`. The body sits at the point; a slot ring at width D and
  the point depth; a ramp at width D and its own depth. The positions read the declared width and
  depth rules (`DeclaredWidth`, `DeclaredDepth`), so no judgement runs.
- The Passes plot draws the marks at the PROPOSAL, each with the worst tone of its planned rows
  (they describe the proposal). The Simulation chart draws the marks at the STATED point, with no
  tone: the samples are the cut that the op holds, and the planned rows do not describe it when
  the proposal differs. When the stated point equals the proposal, the Simulation marks take the
  tone of the rows. Operator question O5.
- One painter (a kit `Mark` kind "planned cut": a small hollow square, `TEXT_MUTED`, a `CAUTION`
  outline when a row exceeds). No new label on the face: the cut name and its rows are the hover.
  The proposal, stated and candidate marks keep their D3 mark rule.
- So a planned slot ring and its samples sit on one plane, at the same width.
- The force field moves to core (`tool_load::force_field` beside `cut_force`): the field of the
  BODY cut (section 16, R1), built through `ProjectSession::force_plane` once per load report in
  the GUI cut-metric memo (R9), outside the envelope, for every op that has a force line and a
  planned chip (`AdvancePerToothMm` door, site 18). The budget is the machine fact
  `MachineProfile::rigidity.force_budget_n` (always present). The contour levels stay
  `CONTOUR_SHARES` of that budget, traced once with the field (`ForcePlane::levels`). The
  envelope edge is the lowest over every declared cut and stays its own line; where it sits under
  the body contour, the mark of the cut that sets it shows why.
- Where the body cut has the force-budget depth limit (`cut_force::budget_line`,
  `BudgetRole::Limit`), the 100 % line is `CAUTION`. Where it does not (a finish pass,
  `S-NoBound`, `BudgetRole::NoLimitHere`), the 100 % line is `TEXT_FAINT` and its label reads
  "budget, no limit here". Over-budget dots draw only where the budget is a limit
  (`CutPoint3::over_budget`, strict). Operator question O4.
- A drill has no field (no lateral engagement: `Geometry::NoRadialEngagement`). An op with no
  envelope (a refusal) has no plane. A one-axis op (a bar) shows its planned marks on the bar.
- The explainer page and its asserts are in section 8.

### 3.9 The wire, and each break

| Surface | Today | After | Break |
|---|---|---|---|
| MCP `get_safe_envelope` | `envelope.cut_checks`, top-level `checks` (stated point) | `envelope.limit_rows` (planned and stated rows) | `cut_checks` and `checks` deleted |
| MCP `get_tool_load_report` | `checks` (from `criteria()`, `group` added by hand); `load_report.per_toolpath[].groups` | `limit_rows: [{toolpath_id, toolpath_name, rows}]`, every cell; `summary.exceeds_breakdown[].group` | `checks` deleted; `groups` leaves the wire if no reader deserializes it (section 9, item 7) |
| MCP `get_suggest_rationale` | `checks` (RPM) | `limit_rows` (RPM rows) | `checks` deleted |
| Any `Check` on the wire | `bound_detail` | gone; the clause is in the row `detail` | `bound_detail` deleted |
| `envelope.notes` | strings, the `MayOccur` sentence among them | `[{kind, face, text}]` from `EnvelopeNote`; no `MayOccur` kind | shape change; the sentence moves to the row |
| MCP `get_toolpath_lineage` | `measured.binding`, `measured.judged` | `measured.row` | both keys deleted |
| CLI `suggest` | `envelope` (with `cut_checks`) | the same `envelope.limit_rows` and the RPM `limit_rows` | as MCP |
| CLI `project` `summary.json` | no limit record | `per_toolpath[].limit_rows` (group rows) | addition |

`build_info().features` gains one token, `limit_rows` (the rule of `rs_cam_mcp/CLAUDE.md`: a
token per capability). It lands in 10a with the rename of `cut_checks`, so a client never sees the
new key without the token. The tool descriptions in `mcp_server.rs` (the `get_safe_envelope` one
names `cut_checks` and `checks`) change in the commit that changes the key. The MCP wire snapshot
re-blesses in each such commit.

### 3.10 What does not change

The export gate (`criteria()`, `exceeded_criteria`, `warns_on_export`), the load policy text, the
diagnostics messages (except the group key), the histograms and their populations, the envelope
edges, Suggest, the modulator and every gate. FM1 must stay byte-equal in every commit.

## 4. Commits

Each commit: fmt on the files it touched (never `cargo fmt --all`: peers edit the tree), the folder
sentries of the folders it touched, core `--lib`, viz `--lib`, `rs_cam_mcp` and `rs_cam_cli` tests,
full clippy, FM1 byte-equal (`feeds_matrix_instrument_fm1 --features heavy-tests`), op golden (the
known 46 rows only). Every cargo command through `timeout <s> scripts/cargo_lane.sh <args> -j 2`.

| Commit | What | Number moves? | Pinned text that moves | Tests and sentries | Screenshots |
|---|---|---|---|---|---|
| 10a | `limits/row.rs`: `CheckRow`, `Subject`, `Occurs`, `Caveat`, `Tone`, `LimitId::text`, the clause matches, `margin_face`, the three projections, `CheckRow::worse_than`; `CutCheck` becomes the planned `CheckRow`; `Check::bound_detail` goes; `ProjectSession::limit_rows`; feature token `limit_rows` and the `get_safe_envelope` description. No surface reads it yet except MCP and the CLI through the envelope serialisation (the rename) | No | MCP wire snapshot (`cut_checks` to `limit_rows`, `bound_detail`); `mcp_server.rs` description; P29-7, P29-8, P29-9 sentries that read `cut_checks` | New `one_record_on_every_surface_p29_10` (claims 1 to 6, section 6); `every_limit_judges_every_declared_cut_p29_7` reads `rows`; `every_limit_is_a_limit_id_p29_2` (`LimitId::text` names all 28) | none (no surface) |
| 10b | `EnvelopeNote` (closed enum, `face`, `text`); the `MayOccur` sentence leaves `notes`, the row carries `Caveat::MayOccur` and the exemption row; the header reads `EnvelopeNote::face` (site 26); `force_budget::note_face` goes | No | `envelope.notes` shape on MCP and CLI; FM1 has no notes column (check, section 9, item 11) | P29-7 sentry claim on `MayOccur`: the row, not the note | S1 |
| 10c | `ExceedsEntry.group`; `DedupKey.group`; `headline_only` behind `test-support` | No (the triage may show one more entry per group: a display count, section 9, item 6) | MCP snapshot (`exceeds_breakdown[].group`) | `no_removing_sample_is_outside_the_gates_p29_6`; a lib test: a headline and a group finding of one id give two triage entries | S7 |
| 10d | MCP and CLI read `ProjectSession::limit_rows`: `limit_rows` keys, delete the three `checks` keys and their helpers (`generation.rs:1221`, `:1261`, `:1274`); CLI `suggest` RPM rows and `project` `summary.json` rows; tool descriptions | No | MCP snapshot; `mcp_server.rs` descriptions; the three `_p29_2b` lib tests become one parity test | New viz lib test `the_mcp_rows_are_the_session_rows_p29_10`; CLI test: `suggest` and `project` print the same rows as MCP for one fixture | none (wire) |
| 10e | The simulation surfaces read rows: `limit_rows`, `LimitRow`, sites 1 to 11, 13 to 15 go; "Cut groups · n" drawer; Readiness and the op list read rows; no F6 merge (section 14 ruling 1: the depth row stays as the gate writes it, the force reading gives no row); M4 caveat; the lineage `measured.row` (item 11) and the lineage panel | No | viz text sentries (`ui_string_hygiene`, `the_inspector_text_is_bounded_ic1`); `cut_metric_cards_read_the_gate_g_cutcards` | `the_sim_inspector_is_tabbed_ic3`, `the_simulation_page_is_summary_first_dc6`, `cut_metric_cards_read_the_gate_g_cutcards`, `a_job_with_each_finding_exports_and_names_it_on_every_surface_k6`; new scan `no_surface_matches_a_limit_p29_10` | S5, S6, S7 |
| 10f | The Suggest card: the face row and the two-part "Limits · n" drawer; sites 21, 23, 24, 25; `apply_hover` reads the binding planned row; the power row reads the body row if section 9 item 5 shows equality | No | viz card sentries | `every_stage_that_moves_a_number_is_on_the_card_g_visible`, `the_chipload_verdict_is_one_row_g_chipverdict`, `the_card_states_the_width_assumptions_p13c`, `the_inspector_draws_only_kit_rows_w6`, `the_issue_stack_is_capped_w6`; new `the_card_shows_one_planned_row_and_a_drawer_p29_10` | S2, S3, S4 |
| 10g | One plane: core `PlaneMarks`, core force field for every op, `CutPoint3::over_budget`; both charts draw planned marks; sites 16 to 20 go; `limit_name` (`ui/feeds/envelope.rs`) reads `LimitId::text`; the explainer page and its script (section 8) | No | `p17_visual_snapshots` (re-bless, lead ruling 2 of P17); `the_chart_kit_draws_the_envelope_p17` | `the_sim_chart_seeks_on_a_dot_p17`, `every_limit_is_drawn_and_named_p5d`, `the_envelope_plot_marks_the_operator_and_the_proposal_k2b`, `the_cut_force_is_the_deflection_force_p17`; new `a_planned_cut_and_its_samples_share_one_plane_p29_10` | S8, S9, S10 |
| 10h (lead, L3) | The deflection of a fixed depth axis becomes a value check at the fixed depth (the earlier leave of a finish, the lower bound of a Project Curve): `no_edge_reason` loses its `Deflection` arm | No cutting number. Check states move: `NoNumber(NotJudgedHere)` rows on wanaka idx 5, 9, 16 and rivmap idx 2, 3 get a value (`P29-9_DESIGN.md` section 17 lists 3 or 4 rows per op) | exemption pins (section 5); MCP snapshot | `the_exemption_list_only_shrinks_p29_3`; `every_limit_judges_every_declared_cut_p29_7`; new claim: a finish deflection row reads the earlier leave | S4 (idx 16 after) |

Order: 10a, 10b, 10c, 10d (the wire goes first, so MCP proves the rows before a pixel moves), then
10e, 10f, 10g; 10h last. 10e, 10f and 10g each start with the screenshots of today (the "before").

## 5. Exemption rows and pins

Pins at HEAD (`the_exemption_list_only_shrinks_p29_3`): ROWS 38, MAX_NEEDS_RULING 27,
ROWS_WITH_A_FILE 37 (`NEXT_SESSION.md`, P29-9 row).

- `M4`: text only (`closes_in` "P29-11 ..."), in 10a. No pin moves. Its `where_` keeps
  `feeds/engagement.rs` and `tool_load/deflection.rs`.
- `S-NotJudgedHere` in 10h: the deflection arm closes. If the stated-point arm (`limits/project.rs`)
  has no instance on the four files (section 9, item 9), the row closes: ROWS 37, MAX_NEEDS_RULING
  26, ROWS_WITH_A_FILE 36. Else the row narrows to the stated-point arm, `where_` loses
  `feeds/envelope.rs`, and no pin moves.
- No new row. `CheckRow::exemption` reads existing ids; claim 4 of the new sentry holds that each
  id is in `EXEMPTIONS`.

## 6. Tests

New sentry `crates/rs_cam_core/tests/one_record_on_every_surface_p29_10.rs` (red before 10a: it
does not compile; green after). Claims, on the simulated fixtures of
`no_removing_sample_is_outside_the_gates_p29_6` and the planned fixtures of
`every_limit_judges_every_declared_cut_p29_7`:

1. `rows_of_envelope` gives exactly one planned row per declared cut and planned limit, plus one
   stated row per boundary limit when `stated` is `Some`.
2. `rows_of_verdict` gives exactly one row per gate and group (no F6 merge, section 14 ruling 1:
   `depth` is one gate and the force reading `cut_force` gives no row), `NotJudged` and `NoSamples` included, plus one row per own material
   and gate (S6) and one row per drill gate.
3. Every row with a value and a bound has `margin == side.margin(value, bound)`; `Exceeds` iff the
   margin is below `-1e-9` on a planned or stated row; a group row has the gate's own state.
4. Every `exemption` id is a row of `EXEMPTIONS`; every `NotJudged`, `MayOccur`, `NoBound` and
   `NotJudgedHere` row has one.
5. `WorstCase` holds exactly on an entry row with an own depth of a ramp; `LowerBound` exactly on
   `BodyDepth::AtLeast` rows; `StoredFeed` exactly on rows that read a `StoredFeedChip`.
6. Every face is at most 60 characters and no face prints "0 %" for a margin that is not zero.

Viz scan `crates/rs_cam_viz/tests/no_surface_matches_a_limit_p29_10.rs`: no product source under
`crates/rs_cam_viz/src` holds a `match` arm or a `matches!` on `LimitId::`, `CriterionKind::`,
`CutGroup::` or `CheckState::` outside `role_of_tone`; non-vacuity anchor: the scan finds
`role_of_tone` and at least one `CheckRow` reader; a comment never matches (viz test rule).

Other new tests: named in section 4. Tests that go red by design: the three `_p29_2b` lib tests in
`app/mcp/tests.rs` (deleted with their keys), the parity test `the_gui_reads_the_session_suggest_door_p29_9`
(reads `rows`), any test that reads `cut_checks` or `bound_detail` (`rg -n "cut_checks|bound_detail"
crates/` lists them), and the viz text sentries of the card and the inspector.

## 7. Screenshots

Before every shot: build the release GUI from HEAD and check it with `gui_status` (a stale GUI fails
silently). Load the file, never save it (`planning/rivmap_block_job/nz_wanaka_21_operator.toml` is
read-only). Shots through MCP `set_ui_view` and `screenshot_gui`. Each shot has a "before" (HEAD of
the commit's parent) and an "after", judged by the image, critiqued and iterated
(`feedback_ui_judged_by_screenshots`). The histogram card is the style bar.

Op index and kind of the operator file (from the file): 0 face, 1 adaptive3d (geodesic, stated
10 x 5.5 mm), 2 adaptive3d (geodesic rest), 3 alignment_pin_drill, 4 drill, 5 project_curve, 6, 7, 8
pocket, 9 project_curve, 10 trace, 11 pocket ("Face clamp pads" is its name; it is a Pocket), 12 face,
13 trace, 14 and 15 adaptive3d (geodesic), 16 scallop, 17 profile.

| Shot | File, op | Workspace and panel | What it proves |
|---|---|---|---|
| S1 | operator, idx 1 (geodesic rough) | Toolpaths, Passes tab, SAFE CUT header | the header shows a short `EnvelopeNote` face; the `MayOccur` sentence is gone (10b) |
| S1b | operator, idx 1 | Passes tab, drawer open | "As the op stands": the stated 10 x 5.5 mm rows exceed; "If you apply ⚡": the proposal rows (10f, O6) |
| S2 | operator, idx 14 and idx 15 | Passes tab, card face and "Limits" drawer open | one face row: the entry ramp force row with "worst case"; the drawer lists every row by cut (10f) |
| S3 | operator, idx 0 and idx 12 (the Face ops; section 9 item 2 names which entry rows read `ChipMin` `Exceeds` on the stored file) | Passes tab, drawer open | the row reads "< 0.1 % under floor · stored feed", tone Caution, not Danger (10f) |
| S4 | operator, idx 16 (finish) and idx 5 (Project Curve) | Passes tab, drawer open | `NoNumber` rows with their reasons and exemption rows; "lower bound" on idx 5; after 10h, idx 16 shows a deflection value at the earlier leave |
| S5 | operator, after a full simulation, idx 1 and idx 2 | Simulation, Metrics tab, "Other limits" and "Cut groups" open | the cleanup group of idx 1 and the entry group of idx 2 are their own rows; `NotJudged` cells visible (10e) |
| S6 | operator, after a full simulation | Readiness page, tool load block | the worst rows are the rows of the inspector (one rank) (10e) |
| S7 | operator, after a full simulation, idx 2 | Simulation, Warnings tab | the headline finding and the entry-group finding are two entries (10c) |
| S8 | operator, idx 6 (Pocket, slot ring) | Passes tab, envelope plot | the slot ring mark at width D beside the proposal; hover names its rows (10g) |
| S9 | operator, after a full simulation, idx 1 and idx 16 | Simulation, Metrics tab, width x depth chart | on idx 1 the neutral planned marks sit at the stated point, over the samples; on idx 16 the field draws with a faint "budget, no limit here" line (10g, O4, O5) |
| S10 | `planning/fixtures/rivmap100/rivmap100_live_0925.toml`, op "3D Rough ladder 8 > 2" (adaptive3d, `contour_parallel`, `depth_per_pass = 8.0` in the file) | Passes tab, plot and drawer | the perimeter sweep slot mark and its rows; the proposal 2.585 mm (`NEXT_SESSION.md` P29-7 row) stays |

The zigzag Pocket `ChipMax` case of item 1 is not in a repo file. To show it: add a zigzag Pocket
with a 6 mm 2F end mill to `planning/fixtures/rivmap100/rivmap100.toml` through the GUI add door,
shoot, and do not save.

## 8. The explainer page and its asserts

The operator's rule: a model is explained by an interactive HTML page plus a script that asserts
each claim. 10g proposes a new chart, so 10g delivers:

- `planning/feeds_contract_2026-11-03/phases/P29-10_explainer/index.html`: one page. Left: an op
  picker (the ops of the four project files) and the row table (subject, limit, margin face,
  caveats, exemption). Right: the width x depth plane with the planned marks, the samples (a
  thinned set), the force contours at 25, 50, 75 and 100 % of the budget and the envelope edges. A
  click on a row lights its mark; a toggle shows one group of samples at a time.
- `.../P29-10_explainer/data/*.json`: the MCP replies `get_safe_envelope` and
  `get_tool_load_report` of each op, taken from a GUI built from HEAD, plus the field grid that core
  builds (a test-support door writes it).
- `.../P29-10_explainer/assert_claims.py`: reads only `data/`, exits non-zero on the first false
  claim, prints each claim with PASS or FAIL. Claims:
  1. One planned row per declared cut and planned limit; one group row per gate and group.
  2. Every margin equals the side rule of its value and bound.
  3. Each planned mark sits at the `(ae, ap)` of its row.
  4. (Section 16, R3.) A mark takes its tone from its row, never from its position. For a mark
     of a body-kind cut (the field's chip and width model) the force-budget row exceeds exactly
     when the mark is above the 100 % contour, within one grid cell. For every other cut the row
     is the truth and its own bound is on the hover (the idx 1 helix: 2.0 mm against the field's
     1.925 mm at D, row bound 2.345 mm).
  5a. (R2.) In every column the force-budget edge is never above the body 100 % contour by more
     than one cell.
  5b. (R2.) In every column where the body cut sets the edge (core names the cut of each column,
     `force_budget_columns`) the edge equals the body 100 % contour within one cell; a column on
     a geometric cap (flute top, depth floor) is outside 5b. One cell is the cell of the field
     grid in the data file, at the depth top of the GUI chart.
  6. `WorstCase` and `LowerBound` hold exactly on the rows of claim 5 of section 6.
  7. The CLI `suggest` rows equal the MCP rows for the same op (the CLI output is in `data/`).
  8. Every claim the page writes in prose has an assert number beside it on the page.

## 9. To measure by the implementer first

No cargo run was allowed for this note. Measure each item before the commit that needs it:

1. The baseline: the count of planned rows and group rows by state and reason, per op, on both
   wanaka files and rivmap100 (MCP replies, a GUI built from HEAD). It is the "before" of every
   screenshot and the input of claim 1.
2. The Face `ChipMin` entry rows: which ops, how many on each file, and their margins at HEAD
   (`e6b587a2e` says they stay `Exceeds` on the stored files until an apply).
3. The zigzag Pocket `ChipMax` case and the geodesic `ChipMin` case on a GUI add door (section 7).
4. F6: on every simulated op of the four files, in every group, the state of the depth row equals
   the state of the force reading against the budget, and the depth margin and the force margin
   have the same sign. If one differs, the merge stops and the lead rules.
5. The card power bar (`power_at_operating_point` at the applied cut) against the body
   `SpindlePower` planned row (proposal speeds). Equal on every op: the bar reads the row. Else the
   bar stays and the note of the commit names why.
6. Whether a split triage entry (10c) changes an FM1 column (`diagnostic_ids`,
   `diagnostic_severities`) or the export header count. FM1 must stay byte-equal.
7. Who deserializes `ToolpathLoadVerdict` or `GroupVerdicts` (`rg -n "Deserialize|from_str|from_value"`
   near those types) before `groups` leaves the wire.
8. Cost: the build time of `rows_of_verdict` and of the core force field on the shelf fixture and
   on wanaka idx 1 (debug and release); the field for every op runs once per load report, never per
   frame. The load report of the shelf fixture took 373 ms after 6d (`NEXT_SESSION.md`).
9. Whether the stated-point arm of `S-NotJudgedHere` (`limits/project.rs`, `of_envelope`) has an
   instance on the four files (decides the pins of 10h).
10. Text fit: the longest face on the four files at the rail width (340 points), against IC1 (60
    characters) and `ui_string_hygiene`.
11. Whether FM1 holds the text of `envelope.notes` or a diagnostic message (10b, L9).
12. The ops whose envelope is a bar or `None` (no plane): their count, and how the planned marks
    show on a bar.

## 10. Questions

### 10.1 For the operator (each changes what the operator sees by default)

- **O1. The Suggest card face.** Today: one status row per problem, and the planned checks show
  nowhere. Proposal: at most ONE new face row, the worst planned row that exceeds, with its caveat
  in words; every row in a closed "Limits · n · k over" drawer. The power row leaves the face
  unless a row exceeds (today a power share over 70 % puts it on the face; the 70 % has no source).
  Options: (a) the proposal; (b) no face row, only the count on the drawer label; (c) one face row
  for each row that exceeds (the count is not measured; section 9 item 1). **My choice: (a).**

  ```
  ⚠ <cut> · <limit> <margin> · <caveat>       (example: Entry ramp · force <p> % of budget · worst case)
  ▸ Recommended · n
  ▸ Limits · n · k over
  ```

- **O2. A row that exceeds under a caveat.** A 0.02 % chip row from a stored rounded feed, or an
  entry over the budget under the worst-case depth: the same Danger colour as a real problem, or
  Caution with the caveat word? **My choice: Caution with the word ("stored feed", "worst case").**
  The rule needs no threshold: the caveat is a fact of the record.

- **O3. One force row in place of the depth row.** After the simulation, "Other limits" shows a
  depth row (mm against the cap of one sample) and the force card shows force against the budget.
  The two are one test (P29-6 F6). Proposal: one row, "force <p> % of budget"; the depth and its cap
  go to the hover. The Warnings tab still words the finding as a depth finding (`load.depth.exceeds`)
  until P29-last (L9). **My choice: merge.**

- **O4. Force contours on every op.** Today an op under no force limit (a finish pass) draws no
  contour. Proposal: the field draws for every op with a force line; on a cut that the budget does
  not judge, the 100 % line is faint and reads "budget, no limit here", and no over-budget dot
  draws. **My choice: draw it.**

  ```
  depth |  .  .  . 100 % budget, no limit here (faint)
        |      .  .  75 %
        |   ░░▒▒▓▓  samples        □ planned
        +--------------------------- width
  ```

- **O5. Planned marks on both charts.** Each declared cut with a width and a depth (the body, a
  slot ring at full width, a ramp at its own depth) is a small hollow square; the name is on hover
  only. On the Passes plot the marks sit at the proposal and take the colour of their rows. On the
  Simulation chart they sit at the stated point (the cut that the samples show) and have no colour,
  because the rows describe the proposal. **My choice: yes, both charts, as described.**

- **O6. The proposal and the op as it stands.** The planned rows describe what ⚡ writes, not the
  cut that the op holds now. Example: idx 1 holds 10 x 5.5 mm, 4.11 times the budget
  (`P29_FINDINGS.md` section 11, measured before P29-4), and its proposal is inside the budget. A
  drawer of planned rows alone can read "Body · force within" for that op. Proposal: the drawer has
  two parts, and the stated rows rank first for the face row. **My choice: two parts.**

  ```
  ▾ Limits · n · k over
    AS THE OP STANDS
    Stated point  force   <p> % of budget   exceeds · advisory
    IF YOU APPLY ⚡ (proposal ap x ae)
    Body          force   <p> % of budget   within
    Entry ramp    force   <p> % of budget   exceeds · worst case
  ```

### 10.2 For the lead (each with my choice)

- **L1. Planned rows at the proposal or at the stated point?** The P29-9 ruling (2026-11-13) says
  one rule: a body row reads the proposal speeds. An entry row reads the stored entry feed over the
  stored speed (P29-8 8d), so the rule is not one rule today; each row `detail` states its operating
  point. The Simulation chart shows what the op cut (the stored speeds). **My choice: keep the
  ruling, and show both parts (O6).** P29-10 adds no per-cut judgement at the stated point: that
  would be a new judgement, not display. The stated point keeps its per-limit rows
  (`Subject::Stated`, `of_envelope`). A per-cut set at the stated point is a later step if the
  operator asks for it.
- **L2. Does a planned row that exceeds become a `Diagnostic`?** It would add findings to the export
  header and Readiness, and the P29-8 ruling stops the work on a new diagnostic in an FM1 cell.
  **My choice: no.** Card rows only.
- **L3. 10h (`S-NotJudgedHere`, the deflection of a fixed depth axis) in P29-10?** It changes check
  states, not cutting numbers. **My choice: yes, as the last commit**, after the display shows the
  `NoNumber` rows, with the expected rows listed in its commit message; the lead can move it to
  P29-11.
- **L4. M4.** **My choice: display in P29-10, the numeric merge to P29-11 with a probe** (the share
  of removing samples where `axial_doc_mm` and `axial_engagement_mm` differ, by op).
- **L5. Old records.** **My choice:** delete `CutCheck`, `Check::bound_detail`, the three `checks`
  keys and the `MayOccur` note in P29-10; keep `CriterionStatus`, `Placement`, `JudgedValue` and
  `criteria()` (the export gate reads them) to P29-last.
- **L6. Wire shape.** **My choice:** a row is `{subject, check, caveats, exemption, tone, face,
  detail}`, with the `Check` nested, not flattened.
- **L7. Where the toolpath name joins.** **My choice:** `ProjectSession::limit_rows(index)`, the one
  door for the GUI memo, MCP and the CLI.
- **L8. The card power bar.** **My choice:** it reads the body row if section 9 item 5 shows
  equality; else it stays and the commit says why.
- **L9. The diagnostics and load-policy messages** (`from_tool_load.rs`, `gcode` advisories) format
  their own numbers. **My choice:** leave them in P29-10 (they are findings, and FM1 may hold their
  text); file them for P29-last with a re-bless.

## 11. Findings outside the scope

- **F1.** `ui/feeds/compare.rs:987` `POWER_CAUTION_SHARE = 0.7` is a viz constant with no source
  that decides a problem row. O1 removes its reader.
- **F2.** `ui/sim_diagnostics.rs:1146` `limit_rows` sets `burn_risk` from the HEADLINE chip verdict
  and applies it only to the headline group row, so an entry-group `ChipMin` row reads as a ceiling
  percent today. 10e deletes the site.
- **F3.** `ui/sim_op_list.rs:1267` keeps a second vocabulary ("advance/t", "defl") that
  `SURVEY_UI.md` section 4 counted; 10e deletes it.
- **F4.** CLI `project` writes no per-toolpath limit record and CLI `suggest` writes no stated-point
  rows: two parity gaps with MCP (10d closes both).
- **F5.** `ui/feeds/why.rs:746` `chipload_min_row` judges the chip floor only for
  `PassRole::Finish`: a role read on a display surface (P29-last would have found it).
- **F6.** The P29-6 F3 stale doc (`sim_op_list.rs` cites `refuses_export`) is still open.

## 12. Lead rulings


Lead, 2026-11-13. The operator gave the lead the calls of P29 (2026-11-12) and judges UI work by
screenshots. So the lead rules O1 to O6 now, and the operator sees them on the screenshots S1 to
S10; a ruling that the operator overrules is one constant or one drawer.

- **O1: (a).** At most one new face row, and a closed "Limits" drawer. The 70 % power threshold
  goes (F1).
- **O2: Caution with the caveat word.** The caveat is a fact of the record; no threshold.
- **O3: merge, if section 9 item 4 passes.** If a state or a sign differs on any group, the merge
  stops and the lead rules again.
- **O4: draw the field for every op**, with the faint line and its words where the budget is no
  limit.
- **O5: yes, both charts.**
- **O6: two parts.** The stated rows rank first for the face row.
- **L1: keep the rule in 10a to 10h.** But the gap is real: before the simulation only the body
  has rows at the stated point. A first slot or a slot ring at the STATED depth has no row, and
  that is the cut the machine makes. It gets its own step after 10f, "10i: every declared cut is
  judged at the stated point", with a note and predicted rows first (no cutting number moves; new
  rows only). The two-part drawer of 10f is built so that 10i fills its first part.
- **L2: no diagnostic** from a planned row.
- **L3: 10h goes last**, with its expected rows in the commit message.
- **L4: display in P29-10, the numeric merge of M4 in P29-11 with a probe.**
- **L5, L6, L7, L9: as chosen.** `ProjectSession::limit_rows` is the one door.
- **L8: as chosen.** If the two differ, the commit names why.
- **Order and stops.** 10a to 10d first (the wire). Each item of section 9 is measured before the
  commit that needs it. FM1 stays byte-equal and the op golden shows only the known 46 rows in
  every commit; a moved cell stops the work.
- **A known red:** viz `the_envelope_plot_marks_the_operator_and_the_proposal_k2b` (an edge colour
  of `CornerCoverage`, older than P29-9). 10g touches that chart: trace the cause there and fix
  the test or the colour, and say which was wrong.

## 13. Measured before the code

Measured on 2026-11-13 at HEAD `3a351d9a2` by the 10a to 10d implementer. A throwaway core probe
test (deleted before every gate; a copy is outside the repo) loaded each file read-only through
`ProjectSession::load` and read the session doors (`safe_envelope`, `of_envelope` at `stated`,
`tool_load_report`). No GUI ran. No file was saved.

The four files are the files of `P29-0_probes.md` and the P29-7 and P29-9 notes:
`planning/rivmap_block_job/nz_wanaka_21_operator.toml` ("operator"),
`planning/rivmap_block_job/nz_wanaka_21_6mm_rough_work.toml` ("work"),
`planning/fixtures/rivmap100/rivmap100.toml` and
`planning/fixtures/rivmap100/rivmap100_live_0925.toml` ("live").

### 13.1 Item 1: the baseline row counts

Planned rows are `SafeEnvelope::cut_checks`; stated rows are `of_envelope(env, stated)`.

| File | Planned | Exceeds | Within | `NotApplicable` | `NoNumber` (reason: count) | Stated | Stated exceeds |
|---|---|---|---|---|---|---|---|
| operator | 662 | 36 | 365 | 51 | NotModelled 186, NotJudgedHere 18, NoBound 4, OnlyALowerBound 2 | 187 | 4 |
| work | 662 | 31 | 370 | 51 | NotModelled 186, NotJudgedHere 18, NoBound 4, OnlyALowerBound 2 | 185 | 3 |
| rivmap100 | 139 | 12 | 66 | 10 | NotModelled 38, NotJudgedHere 7, NoBound 4, OnlyALowerBound 2 | 38 | 0 |
| live | 139 | 11 | 67 | 10 | NotModelled 38, NotJudgedHere 7, NoBound 4, OnlyALowerBound 2 | 38 | 2 |

Operator file, per op (planned rows, of them exceeds; stated rows, of them exceeds; envelope
axes): idx 0 Face 35/2, 12/0, plane; 1 Adaptive3d 70/5, 14/1, plane; 2 Adaptive3d 70/4, 13/1,
plane; 3 AlignmentPinDrill 10/0, 7/0, axial bar; 4 Drill 10/0, 7/0, axial bar; 5 ProjectCurve
15/0, 8/0, fixed; 6 Pocket 42/4, 11/0, plane; 7 Pocket 42/4, 10/0, plane; 8 Pocket 35/2, 10/0,
plane; 9 ProjectCurve 18/0, 8/0, fixed; 10 Trace 21/0, 9/0, axial bar; 11 Pocket 35/2, 13/0,
plane; 12 Face 35/2, 12/0, plane; 13 Trace 21/0, 9/0, axial bar; 14 Adaptive3d 70/4, 14/1, plane;
15 Adaptive3d 70/4, 13/1, plane; 16 Scallop 28/2, 7/0, radial bar; 17 Profile 35/1, 10/0, axial
bar. The planned exceeds of the operator file by kind: entry `ChipMin` 16, entry `RubbingFloor` 6,
entry `ChipMax` 2 (idx 8), entry `ForceBudget` 4 (ramp, idx 1, 2, 14, 15), entry `SpindlePower` 1
(idx 1 ramp), first slot `ForceBudget` 4 and `ChipMax` 4 (`U-PerimeterSweep`), body `ChipMin` 4
(idx 1, 2, 14, 15, margins -2.2 % to -3.3 %), idx 17 plunge entry `PlungeRate` 1 (1582 against
1347.47 mm/min, as `cc0a3cb74` says).

Group rows after the simulation: the four files were NOT simulated (minutes each, and the lead
forbids the long wanaka runs). The five simulated fixtures of `common::kind_fixtures` give, per
fixture, one cell per gate and group (5 gates of `GroupVerdicts` x 5 groups): every milling
fixture has the Drill cell `NotJudged`; Cleanup is `NoSamples` on the plain Pocket, the Profile
and the 2D Adaptive; Link is `NoSamples` on both Pockets.

### 13.2 Item 2: the entry `ChipMin` rows that exceed

| File | Ops | Rows | Margin |
|---|---|---|---|
| operator | idx 0, 12 (Face) | ramp entry and helix entry, 2 per op | -9.67e-5 (-0.0097 %) |
| work | idx 0, 12 (Face) | ramp entry and helix entry, 2 per op | -1.91e-5 (-0.0019 %) |
| operator | idx 6, 7 (Pocket) | ramp and helix | -75.7 % (chip 0.0133 against 0.0550 mm) |
| operator | idx 11 (Pocket) | ramp and helix | -66.5 % |
| operator | idx 16 (Scallop) | helix | -91.2 % |
| rivmap100, live | idx 0 Face, idx 1 3D Rough, idx 2 Scallop | ramp and helix | -47.5 % to -93.4 % |

The Face rows are the stored-feed rows of `e6b587a2e`: a margin of less than 0.01 %. On the face
"0 %" is wrong for them; section 3.3 prints "< 0.1 %". The other rows are real: a chip far under
the floor.

### 13.3 Item 4 (F6): the depth row and the force reading do not agree in state

Measured on the five fixtures only (the four files were not simulated). Every judged group has
the depth gate `Within` and the force under the budget, with the same sign of margin, EXCEPT one
cell: the 2D Adaptive fixture, Link group: the depth gate is `Reported` (no cap: a
`NoNumber(NoBound)` row) and the force reading has a budget, 1.73 N against 35.32 N (margin
+0.951). So a merged row changes a `NoNumber(NoBound)` row to a `Within` row: a state moves.
No fixture sample exceeds, so the `Exceeds` side is not tested at all.

Under ruling O3 the merge stops. 10a writes the depth row as the gate writes it (`of_criterion`)
and writes no force row: the force reading stays report-only (the gate record `load_report`
keeps it). The lead rules again before 10e.

### 13.4 Item 6: the triage split and FM1

FM1 builds its diagnostic columns from `diagnostics_from_static_checks`,
`diagnostics_from_feeds_result`, `heuristic_hints_from_recommendation` and
`diagnostic_from_suggest_warning` (the planned cells) and from `ProjectSession::diagnose_toolpath`
(the simulated subset). None of them reads `stock::sim_triage` or `DedupKey`. The export header
reads no triage (`session/compute/export.rs`). Reasoned: 10c moves no FM1 cell and no export
header count. The FM1 gate of 10c checks it.

### 13.5 Item 7: who deserializes the load verdict

`rg` over `crates/` finds one deserializer of `ToolLoadReport` (and so of `ToolpathLoadVerdict` and
`GroupVerdicts`): the round-trip unit test of `tool_load/verdict.rs`. `gate_population_vacuity_xvac`
deserializes a `PowerVerdict` only. No product code deserializes them. The design rule ("if no
reader deserializes it") is met by product code but not by test code. 10d keeps `groups` on the
wire, so no `serde` attribute moves; the lead decides if it goes.

### 13.6 Item 9: the stated-point arm of `S-NotJudgedHere` has instances

`of_envelope` at the stated point gives `NoNumber(NotJudgedHere)` 55 times on the operator file, 53
on the work file and 18 on each rivmap file: `ChipMax` on every op with a stated point,
`CuttingFeedCap` on the Face and the Pocket ops, `MoveBudget` on the 3D roughs, `ToolDiameter` and
`EngagementFloor` on the bars, and all 8 limits of a Project Curve. So the row does not close in
10h: it narrows to the stated-point arm, and no pin moves (section 5).

### 13.7 Item 11: FM1 holds no note text

FM1 prints no `SafeEnvelope::notes` (`envelope_columns` prints the binding kind and the proposal
`ap` and `ae`). Its column `entry_notes` is the note list of `SuggestWarning::RampFeed`, not an
envelope note. Its diagnostic columns hold the ids and the severities, not the messages. So 10b
cannot move an FM1 cell.

### 13.8 Item 12: the envelopes with no plane

Operator file: plane 11 ops; axial bar 5 (idx 3, 4, 10, 13, 17); radial bar 1 (idx 16); fixed 2
(idx 5, 9). No op refuses an envelope. The notes on the four files: "Width assumes: chips clear;
tool life not modelled" (50 characters, 14 ops), the no-depth reason (73), the move-budget
conflict (135) and the `MayOccur` first-slot sentence (224 characters, 8 ops).

## 14. Lead rulings after 10a to 10d

Lead, 2026-11-13, on the six questions of the 10a to 10d report (commits `8f35e40d5`, `f6702c771`,
`2853ab8b7`, `ac23e7a4e`, `527839dfa`).

1. **F6: no merge in P29-10.** The depth row stays as the gate writes it. The force reading stays
   report-only and gives no row. The numeric question goes to P29-11 with M4. 10e shows the rows
   as they are.
2. **`Caveat::StoredFeed` holds only on a row whose state the rounding can change**: the
   stored-feed chip interval of the value holds the bound. A chip row at -75 % has no such caveat
   and takes the normal tone. Sentry claim 5 states the exact rule and tests both sides.
3. **A face says what its row compares.** A planned force-budget row and the depth gate row compare
   a depth in mm with a depth in mm, so their face names the depth (and may name the force budget
   as the cause); only a row that compares newtons shows a force share. Core holds the rule
   (`limits::Compared`); the words get their final review on the 10e screenshots.
4. **`groups` stays on the wire** until 10e removes its last reader.
5. **Departures accepted:** `own_depth_mm` (the plane point comes in 10g), `Caveat::MayOccur {
   row }`, `samples: Option`, `LimitText` without unit and scale, four tokens, the door with the
   trace, the `Basis::gates_export` rule (the export gate reads `BoundSource::gates_export`, not
   `Basis`). `Diagnostic.group` in about 100 struct literals is accepted for now.
6. **Every toolpath has an entry** in `get_tool_load_report` `limit_rows`. A toolpath with no
   verdict gets its entry with the no-simulation rows (the rows the door gives with no trace),
   never an absent entry. The CLI `project` rows follow the same rule.

### For the clean-up pass

- A constructor for `Diagnostic`, so that a new field does not touch about 100 struct literals.

## 15. The envelope time budget: cause and fix

The lib test `feeds::envelope::tests::the_envelope_meets_its_time_budget`
failed alone: 342 to 346 ms (six runs, 2026-11-13, operator's machine,
load average 2.3) against `ENVELOPE_TIME_BUDGET_MS` = 300 ms.

### The cause, measured

A temporary probe split one envelope of the budget fixture (a 6 mm end
mill Pocket, operator chip 0.0278 mm). The fixture declares five cuts:
`Body` (`Always`), `SlotRing` (`Always`), `Entry(Ramp)` (`Always`, at its
own depth), `Entry(Helix)` (`MayOccur`, own depth) and `Entry(Plunge)`
(`MayOccur`, axial). Only `Body` and `SlotRing` set a ceiling
(`planned::sets_a_ceiling`).

| Phase | Time | Solves x cuts | Ramp and helix judges |
|---|---|---|---|
| Drawn force-budget edge | 237.5 ms | 115 x 5 | 230 calls, 118.0 ms, one distinct input each |
| Exact ceilings | 45.5 ms | 15 x 5 (force budget) | 30 calls, 15.2 ms |
| Drawn deflection edge | 11.4 ms | 8 x 5 | 16 calls, 5.8 ms |
| Pick deflection edge | 11.5 ms | 8 x 5 | 16 calls, 5.8 ms |

In the drawn force-budget edge, `Body` took 60.7 ms (115 distinct inputs)
and `SlotRing` 58.4 ms (104 distinct inputs).

`budget_solve`, `deflection_solve` and `table_solve` called
`judge_declared_cuts`, which judges every declared cut. Then
`ByCut::lowest_always` drops each cut that does not set a ceiling before it
reads the answer. So each width column solved the ramp and the helix in
full (`force_budget::depth_at_budget`, `cutter_axial_constraints`) on the
same width (D) and the same entry chip, and discarded the answer. That is
about 145 ms in the probe run (356 ms with the probe). The fix removed
about 135 ms of 344 ms, about 40 %.

### The commits

- `d7e6527b8` (P29-8 8d) added the cost. Before 8d an entry had no
  engagement model: the diff removes
  `CutKind::Entry(EntryKind::Ramp) => CutEngagement::NoModel(NoEngagementModel::Ramp)`
  and the same line for `Helix`. So the judge of an entry returned at
  `width_and_chip`. 8d gave each entry a width and a chip, so its judge
  runs the full solve.
- `eb6a4a4a7` (P29-7 7c) adds more cost. It made each edge solve over
  every declared cut. The slot ring solve is measured: 58.4 ms in the
  drawn force-budget edge and 4.3 ms in the exact ceilings. 7c also raised
  `BUDGET_EDGE_SPLIT_LEVELS` from 4 to 12, which adds width columns. The
  share of the split levels is not measured. Other commits changed the
  envelope after P19 too, and their share is not measured. Removal of
  these costs changes numbers, so this fix does not touch them.

### The fix (no value changes)

`planned::lowest_ceiling::<L>` is the ceiling-only form of the driver. It
skips a cut where `sets_a_ceiling` is false before it judges the cut. For
every other cut it calls the same scope and judge step (`judge_one`, which
`judge_declared_cuts` now also calls). It uses the same tie rule
(`keep_lowest`, which `lowest_always` now also calls). Thus it returns
the same answer as `judge_declared_cuts::<L>(..).lowest_always(value)`.
The three edge solves call it. The rows (`ceiling_rows`, `value_rows`)
still read every cut through `judge_declared_cuts`.

### The proof of equality

- A throwaway dump serialized 95 envelopes (JSON and the full `Debug`,
  which also holds `exact`, `pick_limits` and the solved columns). The
  set: the budget fixture, the 3.175 mm hardwood Pocket, every
  `OperationType` on a 6 mm end mill, a 6 mm ball and a 12 mm V-bit
  (75 sections: 45 envelopes and 30 refusals), and all 18 toolpaths of
  `nz_wanaka_21_operator.toml` (all 18 build an envelope; 12 draw a
  `Rigidity` edge, 7 `ApCurve` and 5 `ApCeiling`; 3 are `Unmodelled`). The dump ran
  twice on the unchanged tree, and the two files were byte-equal. The
  dump after the fix was byte-equal to them.
- A new lib test, `the_ceiling_driver_gives_the_lowest_always_answer_p29_10`,
  compares the two drivers for the force budget, the deflection and the
  vendor depth table. It reads both fixtures at 40 widths and 3 chips.
  When the `sets_a_ceiling` skip is removed, the test fails.
- FM1 is byte-equal. The op golden shows only the known 46 rows.

### Timings (the budget test alone, six runs each)

| Tree | ms | Load average | Free memory |
|---|---|---|---|
| Before | 344, 346, 342, 346, 343, 343 | 2.25 to 2.36 | 10 GiB |
| After | 206, 213, 214, 205, 212, 208 | 3.01 | 12 GiB |

Inside the parallel core `--lib` run the test still fails (757 ms, load
average 5.6; 2805 passed, 1 failed). The trap in `feeds/CLAUDE.md` names
this failure, but no parallel run before the fix was measured, so the
trap is the likely cause, not a measured one.

### Open for the lead

- 205 to 214 ms is about 1.4 times under the budget, not the ten times
  that the budget doc states. The rest of the cost comes from 7c (the
  slot ring solve and the split levels). The remaining inputs are almost
  all distinct, so a cache gives little. A further cut changes a number
  (fewer split levels, fewer scan steps in `depth_at_budget`) or the
  budget. Both are rulings for the lead.

## 16. Lead rulings for 10g

Lead, 2026-11-13, on the stop report of the 10g implementer. The probe on
HEAD `a381097c7` found claim 5 of section 8 false as written: the field is
the body cut, and the force-budget edge is the lowest ceiling over every
declared cut (on wanaka idx 1 the planner cap sets the edge at the narrow
widths; at ae 0.445 mm the edge rides the flute top).

- **R1. The field is the force of the BODY cut at the planned chip.** It
  is the field the samples live in, so it predicts the over-budget dots.
  The envelope edge stays its own line. Where the edge sits under the body
  contour, another declared cut sets it, and the planned mark of that cut
  shows it. No field is built from the lowest cut.
- **R2. Claim 5 becomes two claims.** 5a: in every column the force-budget
  edge is never above the body 100 % contour by more than one cell. 5b: in
  every column where the body cut sets the edge (core names the cut of
  each column: `SafeEnvelope::force_budget_columns`, wire
  `force_budget_columns`), the edge equals the body 100 % contour within
  one cell. A column on a geometric cap (`ColumnCeiling::FluteTop` or
  `DepthFloor`) is outside 5b, and the page says so. One cell is the cell
  of the field grid in the data file, which states its depth top (the top
  of the GUI chart).
- **R3. Claim 4 becomes: a mark takes its tone from its ROW**, never from
  its position against the contour. For a mark of a body-kind cut the row
  exceeds exactly when the mark is above the 100 % contour, within one
  cell (asserted). For every other cut (an entry at its own chip, a
  planner cap at its wider width) the row is the truth and its own bound
  is on the hover. The example is the helix mark of idx 1 (2.0 mm against
  the field's 1.925 mm at D, row bound 2.345 mm).
- **R4. Plot labels keep their words.** `LimitText` gains `chart`
  (`ChartName::Line` or `NoLine` with a reason); the viz reads it
  (`LimitKind::chart_name`).
- **R5. One fact, one field.** Every row with an own depth has a plane
  point with that depth, so `subject.own_depth_mm` goes; `subject.at` is
  `{ae_mm, ap_mm, own_depth}` and `CheckRow::own_depth_mm` reads it.
- **R6. k2b: the test is wrong; rewrite it to the truth of today**, and
  close the product gap: the plot shows WHY a dot is outside (the crossed
  edge in view, or the planned mark of the cut that places it outside) and
  the dot's hover names that cut and limit.
- **R7.** Fix the scan of `cut_metric_cards_read_the_gate_g_cutcards` to
  follow the one renderer; keep the claim.
- **R8.** IC1 caps follow the measure. "Max RPM" shares the row of "Max
  feed" by a layout rule with a reason, not by a looser merge constant.
  The rotated axis titles: report only.
- **R9.** The field is built once per load report in the cut-metric memo,
  outside the envelope and never per frame; a test holds that a second
  frame builds no field.

### Found while building (10g)

- **The k2b point.** The fixture Pocket (6 mm end mill, stated chip
  0.0278 mm) places the deep point (ae 5.56, ap 8.62 mm) outside by
  `Rigidity` at margin -0.4185. The drawn curve is the lowest cut at the
  curve chip: the slot ring, 17.24 mm. The placement also judges the
  stated width at the pick chip, `max(0.0278, suggested 0.127)` mm
  (`judged_ceiling`, P29-7 trap 1): the BODY cut there gives 6.08 mm. So
  the drawn edge is the lowest over the declared cuts at its own chip; the
  two ceilings differ by the chip, not by the cut. No edge changed.
  `SafeEnvelope::outside_cut` names the cut (the body); the body mark at
  the proposal is in view and the dot hover reads "Now is outside: Rigidity
  DOC of the body".
- **The width of a full slot sample.** On the simulated contour Pocket
  fixture the widest sample reads 5 mm against the 6 mm slot ring mark (a
  measured arc at the simulation cell reads short). The new sentry holds
  that no sample is wider than the slot ring mark by more than one column.
- **Claim 2 of section 8 is false as written.** On the operator file, 24 entry chip rows (chip
  min, chip max, rubbing floor of a ramp or a helix) hold a margin that is not the side rule of
  their value: P29-9 makes such a row judge the end of its stored whole-mm/min feed interval
  (section 6 claim 3 says "the side rule of the judged value"). The script asserts the section 6
  form (it recovers the stored feed from the two margins: 6447, 6448, 4571 and 240 mm/min, each a
  whole feed) and the page states it. The lead rules the words of claim 2.
- **Departures.** `mark_color` (a planned mark is `CAUTION` when a row of its cut exceeds, else
  `TEXT_MUTED`) is a second tone-to-colour rule beside `role_of_tone`: the plot has no `DANGER`
  (ruling Q6). The neutral marks of the simulation chart are `TEXT_STRONG`, not `TEXT_MUTED`:
  a muted outline reads as one more heat cell (judged on S9). A mark past the view (the idx 6
  helix at its own depth 1.0 mm over a 0.91 mm view) is held on the frame edge, as the dot is;
  its hover gives the true depth. A mark at the operator's dot is left out (the dot covers it).
  No field is built per envelope memo: the Passes plot draws marks, not the field (R9 names an
  envelope memo; the lead confirms).
- **Not seen on a file.** The faint "budget, no limit here" line draws on no op of the four
  files: the Scallop ops reach at most 75 % of the budget under the chart top. The snapshot
  `sim_chart_340_no_limit.png` of `p17_visual_snapshots` draws it.


## 17. Lead rulings after 10g (2026-11-14)

- **Claim 2 of section 8.** The words are the words of section 6 claim 3: each margin equals the
  side rule of the JUDGED value and its bound. An entry chip row judges the end of the interval of
  its stored feed (P29-9), so its margin is not the margin of the printed value.
- **R9.** The Passes plot draws marks and no field, so no field is built for an envelope memo. The
  field is built once for each load report.
- **`mark_color`.** A plot has no danger colour (ruling Q6 of P17), so a mark is caution or muted.
  The rule stays, but it is a second map from a tone to a colour. The clean-up pass puts it beside
  `role_of_tone`, as one function of `Tone` and the surface kind.
- **Open from the lead review of 10g, to close before the operator judges P29-10:**
  1. `assert_claims.py`: a claim with zero cases prints NOT TESTED and fails. Claim 4 has no row
     that exceeds; claim 5a judges 0 columns on idx 0. Add the stated body mark of idx 1 to claim 4.
  2. Nobody opened the explainer page in a browser.
  3. `END_HANG` (2 pt) is fitted to the IC1 counter and moves no visible pixel. It is not a layout
     rule. Derive it, or restore the caps 41 / 38 / 30 with the reason.
  4. The IC1 fixtures paint no face line, so no cap covers the face line of 10f.
  5. The "after" screenshots come from three builds. One build of the final HEAD, one full set, and
     an index from S1 to S10 to the files.
- **Order.** The point-band finding (a vendor row with one printed chip value) is ruled before
  10i, because 10i gives the stated point chip rows, and the face line of the card reads the stated
  rows. The 10h and 10i notes predict the face line of every operator op and the Readiness rows
  before the code.

## 18. Step 10h: predicted rows (before the code)

Baseline: HEAD `702c8e97b` plus `pb.patch` (P29-10i PB). A throwaway probe
(deleted; a copy is in the agent scratchpad `p29_10hi/`) loaded the four
files read-only and wrote every row of `ProjectSession::limit_rows`. A
second throwaway probe judged the deflection of each declared cut at the
fixed depth from the public pieces (the declared cuts of the Suggest
context, `DeflectionPlanned`, the target `deflection_limit_um` of the body
leave, the operator chip of the drawn edge), with no 10h code.

### Section 9 item 9, measured again

`of_envelope` at the stated point gives `NoNumber(NotJudgedHere)` 55 times
on `nz_wanaka_21_operator.toml`, 53 on `nz_wanaka_21.toml` and 18 on each
rivmap file (the same counts as section 13.6): `ChipMax` on every op with a
stated point, `CuttingFeedCap` on the Face, Pocket and 3D Rough ops,
`MoveBudget` on the 3D roughs, `ToolDiameter` and `EngagementFloor` on the
bars, and the limits of a Project Curve (flute guard, cutting length,
vendor axial, force budget, engagement floor, tool diameter, chip max). So
`S-NotJudgedHere` narrows and does not close: its `where_` loses
`src/feeds/envelope.rs` (the one marker there was the deflection arm of
`no_edge_reason`), its `who` names the stated point only, and no pin moves
(ROWS 37, MAX_NEEDS_RULING 26, ROWS_WITH_A_FILE 37).

### The rule of the step

On a fixed depth axis the envelope draws no deflection edge (the boundary
does not change: no `Limit` is added, so `place`, `propose` and the
placement do not move). Each declared cut is a value check at the fixed
depth: the body at the earlier op's leave (9c) or at the lower bound of a
Project Curve (9b), an entry at its own depth, against the root of the one
beam at the operator chip (the chip of the drawn edge on a free axis). The
basis is the basis of the drawn deflection edge. A cut the beam cannot
judge says why: no built tool `NoToolBody`, no force line `NoForceLine`, no
stickout or no depth before the simulation `NotModelled`, a straight plunge
`NotModelled` (no side-cut model). No row is silent.

### Predicted rows (the list for the commit message)

Each row is `NoNumber(NotJudgedHere)` with row `S-NotJudgedHere` today. The
beam reaches the 200 um target at no depth below the flute on any of these
cuts (no root), so a row with a value is `Within` with no bound; on a lower
bound it is `NoNumber(OnlyALowerBound)` with the caveat `lower bound`.

| File | idx (op) | Cut | After 10h |
|---|---|---|---|
| operator, nz_wanaka_21 | 16 (Scallop) | body | `Within`, no bound, depth 0.5 mm (the leave), caveat `at earlier leave` |
| operator, nz_wanaka_21 | 16 | ramp entry | `Within`, no bound, depth 0.5 mm, caveats `may not occur`, `at earlier leave` |
| operator, nz_wanaka_21 | 16 | helix entry | `Within`, no bound, own depth 1.0 mm, caveat `may not occur` |
| operator, nz_wanaka_21 | 16 | plunge entry | `NoNumber(NotModelled)` |
| operator, nz_wanaka_21 | 9 (Project Curve) | body | `NoNumber(OnlyALowerBound)`, depth 2.418 mm, caveat `lower bound` |
| operator, nz_wanaka_21 | 9 | first slot | `NoNumber(OnlyALowerBound)`, depth 2.418 mm, caveat `lower bound` |
| operator, nz_wanaka_21 | 9 | plunge entry | `NoNumber(NotModelled)` |
| operator, nz_wanaka_21 | 5 (Project Curve) | body | `NoNumber(NotModelled)`: the body depth is known after the simulation (`BodyDepth::None(DepthAfterSimulation)`; the setup gives no lower bound) |
| operator, nz_wanaka_21 | 5 | first slot | `NoNumber(NotModelled)`, same reason |
| operator, nz_wanaka_21 | 5 | plunge entry | `NoNumber(NotModelled)` |
| rivmap100, live_0925 | 2 (Scallop) | body, ramp entry, helix entry, plunge entry | as idx 16 (body and ramp at 0.5 mm, helix 1.0 mm, plunge `NotModelled`) |
| rivmap100, live_0925 | 3 (Project Curve) | body, first slot, plunge entry | body and first slot `NoNumber(OnlyALowerBound)` at 1.0 mm; plunge `NotModelled` |

Counts: 34 rows leave `NotJudgedHere` (10 on each wanaka file, 7 on each
rivmap file). On each wanaka file 3 become `Within`, 2 `OnlyALowerBound`
and 5 `NotModelled`; on each rivmap file 3 `Within`, 2 `OnlyALowerBound`
and 2 `NotModelled`. No row exceeds. No other row changes (no state, tone,
caveat or face of another limit). The 34 rows lose the exemption row
`S-NotJudgedHere`; the 8 ramp and helix entry rows of the Scallops keep `U-EntryFallback`
where they hold `may not occur`.

Face line of the Passes card on the 18 operator ops: no change (a planned
row never reaches the face; 10h writes no stated row). Before and after:
idx 1, 2, 14, 15 "stated point · depth 387 % / 74 % / 73 % / 57 % over
force cap" (advisory); every other op no line. Readiness: no change (10h
touches no gate). FM1 and the op golden: no cell and no row (no edge, no
proposal, no feed moves).

### Measured beside predicted (10h)

The same row probe ran on the 10h tree; "measured" is the diff of the PB
dump and the 10h dump.

| Quantity | Predicted | Measured |
|---|---|---|
| Rows that leave `NotJudgedHere` | 34 (10, 10, 7, 7) | 34, the rows of the table above |
| `Within`, no bound (Scallop body, ramp, helix) | 12 | 8 (mismatch: the ramp, below) |
| `NoNumber(OnlyALowerBound)` (Project Curve body and first slot, idx 9 and rivmap idx 3) | 8 | 8 |
| `NoNumber(NotModelled)` (plunge entries; idx 5 body and first slot) | 14 | 18 (the 4 Scallop ramp rows join) |
| Rows that exceed | 0 | 0 |
| Other rows that change | 0 | 0 |
| Face lines of the 18 operator ops | no change | no change |
| Verdict rows (Readiness) | no change | no change |

The mismatch: the ramp entry of a Scallop has no own depth (a Scallop states
no depth per pass), so it sits on the body plane. The prediction probe
judged it as a slot at the leave (`Within`). The first 10h build did the
same, and claim 5 of `every_limit_judges_every_declared_cut_p29_7` failed
(P29-8 8d: a ramp or a helix with no own depth has no engagement; its depth
rows hold no number). The build now gives such a ramp `NotModelled`. The
prediction rule missed the P29-8 rule; the product follows P29-8.

Pins: no pin moves (ROWS 37, MAX_NEEDS_RULING 26, ROWS_WITH_A_FILE 37). The
row `S-NotJudgedHere` narrows: `where_` is `src/limits/project.rs` and
`src/limits/row.rs`.

Finding outside the scope: `nz_wanaka_21*` idx 5 (Project Curve, no lower
bound from the setup) has no force-budget, vendor depth table or power row
at all: `ceiling_rows` returns `None` with no proposal depth, and no scope
reason covers it. 10h keeps that behaviour for those three limits (no
number moves) and gives the deflection rows `NotModelled`.

Explainer data (`P29-10_explainer/data/`): regenerated on the PB + 10h tree,
so this commit also carries the PB rows of the page. The six
`mcp_safe_envelope_operator_*.json` files come from a throwaway probe that
builds the `get_safe_envelope` reply as `mcp_get_safe_envelope` does (the
core serialiser and the grid cap of `rs_cam_mcp::response`) on a read-only
load; against the old files only `limit_rows` differs, and only in the
`ChipMin` rows (the caveat `advisory_floor`; the body tone of idx 1, 2, 14,
15). The two `cli_suggest_*.jsonl` files are the release CLI
(`rs_cam_cli suggest --json`). The field and tool-load files are unchanged
(no simulation). `build_page.py` rebuilt `index.html`; `assert_claims.py`:
11 of 11 PASS.
