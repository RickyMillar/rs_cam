# P29-3 design: `Cut`, `cuts()` and the exemption list

Written 2026-11-11 by the implementer of P29-0 to P29-2. HEAD `447cf7d8b`. No product code
is edited for this note. It stands alone: a new lead and a new implementer can work from it.

Status: a proposal. It waits for the lead's answers to section 9.

## 0. Where this step sits

The programme is in `planning/feeds_contract_2026-11-03/phases/P29_FINDINGS.md` (design in
section 4, the change list in section 6). The evidence is in:

- `phases/P28_limit_audit.md`: 34 exemptions from a limit, by op family, role or span.
- `phases/P29_parts/A_cuts_and_limits.md`: what exists, the planned-cut table of the 25 ops,
  the limit list, and 12 more exemptions (M1 to M12).
- `phases/P29_parts/B_metrics_and_display.md`: the metrics and the display surfaces.
- `phases/P29-0_probes.md`: the measured numbers on the two wanaka projects.
- `phases/P29-2_DESIGN.md`: `LimitId` and `Check` (commits `8c8671f07`, `447cf7d8b`).

Done before this step (all byte-identical, FM1 unchanged):

| Step | Commit | What it gave |
|---|---|---|
| P29-1 | `2685c84f6` | `crates/rs_cam_core/src/feeds/cut_load.rs`: `CutLoad::of(&Engagement, OperatingPoint, tool, material)` with lazy accessors `power_kw`, `force`, `deflection_mm`, `chip`; `OperatingPoint` with one named constructor for each door |
| P29-2a | `8c8671f07` | `crates/rs_cam_core/src/limits/`: `LimitId` (29), `Check`, `CheckState`, the mappings from the five old limit lists, the projections |
| P29-2b | `447cf7d8b` | `checks` keys beside the old fields in MCP `get_tool_load_report`, `get_safe_envelope`, `get_suggest_rationale` |

The goal of P29-3 (`P29_FINDINGS.md` row P29-3): the type `Cut`, the required method `cuts()`,
and ONE list in code of every exemption. The output stays byte-identical. Every exemption of
today becomes a visible entry. No limit changes its scope in this step.

Paths below are under `crates/rs_cam_core/src/` unless they start with `crates/` or `planning/`.

## 1. The `Cut` type

New file `limits/cut.rs` (the folder that holds `Check`; P29 design 4.5 puts `check_all(&Cut,
..)` there). The folder rule of `limits/CLAUDE.md` invariant 1 stays: no operation type, family
or pass role in the folder. So `Cut` holds no such field, and `cuts()` (which reads the op)
lives in `compute/catalog/`, not in `limits/`.

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cut {
    pub kind: CutKind,
    pub engagement: CutEngagement,
    pub point: Option<OperatingPoint>,   // feeds::cut_load::OperatingPoint; None: the feed is not known yet
    pub source: CutSource,
    pub facts: CutFacts,
}
```

Every part is `Copy`. `Engagement` is `Copy` (6 `f64`, an `EngagedSection` of at most 5 `f64`,
an `Option<f64>`), `OperatingPoint` is `Copy` (4 words). The whole `Cut` is about 200 bytes on
the stack and holds no heap value, so a per-sample `Cut` allocates nothing.

### 1.1 `CutKind`

```rust
pub enum CutKind {
    Body,                       // the pass the operator states
    FirstPass(FirstPassKind),   // Slot | FirstRow | PlannerCap: a wider cut the plan makes first or holds as a cap
    SpringPass,                 // a repeat at the final depth
    Entry(EntryKind),           // compute::catalog::EntryKind: Plunge | Ramp | Helix | PeckLadder
    EntryApproach,              // a feed through air above an entry
    Cleanup(CleanupKind),       // WaterlineSweep | ResidueMop | ContourResidue
    CappedStep(FeedCapSource),  // adaptive3d::FeedCapSource, 6 kinds
    SlotRing,                   // the P23d first ring of a contour Pocket (a capped move with no FeedCap item)
    LeadIn, LeadOut,
    Link,
    Drill,
}
```

`EntryKind` and `FeedCapSource` are existing enums of the planners; `limits/` may read them
(they name a cut, not an operation). If the lead wants `limits/` free of every `compute::`
import, `EntryKind` moves to `limits/cut.rs` and `compute/catalog/entry_contract.rs` re-exports
it (question Q1).

### 1.2 `CutEngagement`

```rust
pub enum CutEngagement {
    Side(Engagement),                       // feeds::engagement::Engagement, as today
    AxialPlunge { diameter_mm: f64, core_diameter_mm: f64 },   // today PowerModelInputs::axial_plunge, flat tools only
    NoModel(NoEngagementModel),             // Ramp | Helix | PlungeOfShapedTip | DepthAfterSimulation | NoStatedDepth | NoStatedWidth
}
```

P29-3 adds NO new model. A ramp, a helix and a plunge of a ball, a bull nose, a V-bit or a
taper are `NoModel` (the inclined engagement is P29-8). A cut of an op that states no depth or
no width is `NoModel(NoStatedDepth | NoStatedWidth)` (P29-9 gives it a value). `CutLoad` then
returns `NoNumber`; nothing prints a zero.

### 1.3 `CutSource`

```rust
pub enum CutSource {
    Planned,
    Measured { sample_index: u32, move_index: u32, depth: SampleDepth },
}
pub enum SampleDepth { RemovedLayer, EngagedEdge }   // axial_doc_mm | axial_engagement_mm
```

`SampleDepth` names the M4 item (one sample, two depths) as a typed value. Both doors stay in
this step: `Engagement::from_sample` builds `RemovedLayer`, `tool_load::deflection::
sample_engagement` builds `EngagedEdge`. A probe decides later whether one depth is enough.

### 1.4 `CutFacts`: the geometry facts that replace the labels

```rust
pub struct CutFacts {
    pub thins_chip: RadialThinning,       // feeds::geometry::RadialThinning: Applied | FinishPass | DrillCycle
    pub offset_ring: bool,                // the corner-coverage rule applies
    pub leave_after_mm: Option<f64>,      // the radial stock this cut leaves for a later pass
    pub depth_rule: DepthLimit,           // ForceBudget | Factor | None(NoDepthLimit)
}
pub enum NoDepthLimit { DrillCycle, FinishPass }
```

In THIS step each fact is filled from today's label, at one place (`cuts()`), with today's
value:

| Fact | Today's source | Today's readers that then read the fact |
|---|---|---|
| `thins_chip` | `RadialThinning::of(family, role)` (`feeds/geometry.rs:59`) | Step 5 (`feeds/calculator/feed.rs:101`), `feeds/efficiency.rs:209`, `session/compute.rs:2240`, `session/compute/toolpath_diagnostics.rs`, `tool_load/chipload.rs:606` and `distribution.rs:779` (`of_lut_role`) |
| `offset_ring` | `family == Pocket && role == Roughing && Radial::Width(_)` (`feeds/envelope.rs:2461`, `feeds/suggest.rs:751`) | `axes_plan` (`computed_width`), `geometric_limits` (`:2516`), `rigidity_limit` (`:3237`) |
| `leave_after_mm` | `OperationConfig::radial_stock_to_leave_mm()` with `pass_role` (`tool_load/deflection.rs:138`) | `deflection::exceeds_limit` |
| `depth_rule` | `RigidityProfile::depth_cap_rule(family, role)` (`machine/mod.rs:293`) | the 6 readers of section 5.4 |

`depth_rule` is the typed fact that replaces the `law` text read in
`limits::DepthRule::of_limit` (`limits/mod.rs`; it compares `Limit::law` with
`feeds::envelope::RIGIDITY_FACTOR_LAW`). `rigidity_limit` (`feeds/envelope.rs:3168`) stores
the rule on the `Limit` it builds, in a new field `Limit::depth_rule: Option<limits::DepthRule>`
that is `#[serde(skip)]` (the `Limit` JSON is on the wire and must not move). `DepthRule::
of_limit` then reads the field. The law text stays as the printed law only.

A fact holds the label's value in this step, so it is NOT yet geometry. The exemption list
(section 4) says so for each one. P29-4 to P29-7 change the values.

## 2. `OperationConfig::cuts()`

New file `compute/catalog/cuts.rs`:

```rust
pub struct CutContext<'a> { pub tool: &'a ToolConfig, pub dressups: Option<&'a DressupConfig> }
pub struct CutList { cuts: SmallList<DeclaredCut> }          // a fixed array of at most 8, no heap
pub struct DeclaredCut { pub kind: CutKind, pub width: DeclaredWidth, pub depth: DeclaredDepth, pub facts: CutFacts }
impl OperationConfig { pub fn cuts(&self, ctx: &CutContext<'_>) -> CutList }
```

One exhaustive `match self` with 25 arms and no wildcard (the workspace denies
`clippy::wildcard_enum_match_arm`). It stands in the place of `cut_spec()` + `FirstPass` +
`planner_width()` + the entry `if let` of `feeds::ramp::entry_geometry`. In this step those
functions stay, and a sentry holds `cuts()` equal to them (section 6, commit A). A
`DeclaredCut` holds the stated numbers; it becomes a `Cut` when a caller gives the tool and
the operating point (`DeclaredCut::at(&self, tool, point) -> Cut`).

`DeclaredWidth`: `Stated(mm) | Slot | PassCeiling { stepover } | CapMult { stepover, mult } |
Scallop(mm) | None(NoStatedWidth)`. `DeclaredDepth`: `Stated(mm) | None(NoStatedDepth |
DepthAfterSimulation)`.

### 2.1 The cuts each op declares in THIS step

"Today's cuts and today's scope only": a cut is in the list when a pre-simulation reader reads
it today (`CutSpec`, `FirstPass`, `PlannerWidth`, `UnliftedFeed::of_slot_ring`, the entry
contract). A cut that the generator makes and no reader reads today is NOT declared in this
step; it goes on the exemption list (section 4) as "undeclared", so it is visible. Source: the
table of `phases/P29_parts/A_cuts_and_limits.md` section 2.

`Entry` rows: each op declares the entry kinds of its `EntryContract`
(`compute/catalog/registry.rs`), with `CutEngagement::NoModel`. The entry geometry (angle,
radius, pitch) comes from `feeds::ramp::entry_geometry` where it exists today.

| # | Op | `Body` (width, depth) | Other declared cuts today | Entry kinds (contract) | Facts today (`thins`, `ring`, `depth_rule`) |
|---|---|---|---|---|---|
| 1 | Face | stepover, depth per pass | - | Plunge, Ramp, Helix (dressup) | Applied, ring, ForceBudget |
| 2 | Pocket | stepover, depth per pass | `FirstPass(Slot)`; `SlotRing` (contour pattern only) | Plunge, Ramp, Helix | Applied, ring, ForceBudget |
| 3 | Profile | slot, depth per pass | `FirstPass(Slot)` | Plunge, Ramp, Helix | Applied, no, ForceBudget |
| 4 | 2D Adaptive | stepover, depth per pass | `FirstPass(PlannerCap)` at `c D`; `FirstPass(Slot)` when `slot_clearing` | Plunge, Ramp, Helix (default Helix) | Applied, no, Factor |
| 5 | VCarve | none, `max_depth` when set | - | Plunge | FinishPass, no, None(FinishPass) |
| 6 | Inlay pocket | none, pocket depth | - | Plunge | FinishPass, no, None(FinishPass) |
| 7 | Inlay plug | none, depth per pass | - | Plunge | FinishPass, no, None(FinishPass) |
| 8 | Zigzag | stepover, depth per pass | `FirstPass(Slot)` | Plunge, Ramp, Helix | Applied, ring, ForceBudget |
| 9 | Trace | slot, depth per pass | `FirstPass(Slot)` | Plunge | FinishPass, no, None(FinishPass) |
| 10 | Drill | no radial, no depth (a peck) | `Drill` (the axial plunge power case) | none (the descent is the cut) | DrillCycle, no, None(DrillCycle) |
| 11 | Bore | slot, none (a helix pitch) | `FirstPass(Slot)` | none | Applied, no, ForceBudget |
| 12 | Chamfer | none, none (a chamfer width) | - | Plunge, Ramp, Helix | FinishPass, no, None(FinishPass) |
| 13 | Parallel finish | scallop or stepover, none | - | Plunge | FinishPass, no, None(FinishPass) |
| 14 | 3D Adaptive | stepover, depth per pass | `FirstPass(PlannerCap)`: `c D` (geodesic) or `CapMult` (contour parallel) | Plunge, PeckLadder, Ramp, Helix (planner) | Applied, no, Factor |
| 15 | Waterline | none, Z step | - | Plunge, Ramp, Helix | Applied (SemiFinish thins), no, None(FinishPass) |
| 16 | Pencil | offset stepover or none, none | - | Plunge, Ramp, Helix; own Ramp | FinishPass, no, None(FinishPass) |
| 17 | Scallop | scallop, none | - | Plunge, Ramp, Helix | FinishPass, no, None(FinishPass) |
| 18 | Unified Finish | scallop, Z step | - | Plunge (planner) | FinishPass, no, None(FinishPass) |
| 19 | Steep/Shallow | stepover, Z step | - | Plunge, Ramp, Helix | FinishPass, no, None(FinishPass) |
| 20 | Ramp Finish | none, max step-down when set | - | Plunge, Ramp, Helix | FinishPass, no, None(FinishPass) |
| 21 | Spiral Finish | stepover, none | - | Plunge, Ramp, Helix | FinishPass, no, None(FinishPass) |
| 22 | Radial Finish | none (an angular step), none | - | Plunge, Ramp, Helix | FinishPass, no, None(FinishPass) |
| 23 | Horizontal Finish | stepover, none | - | Plunge, Ramp, Helix | FinishPass, no, None(FinishPass) |
| 24 | Project Curve | slot, `DepthAfterSimulation` | `FirstPass(Slot)` | Plunge | FinishPass, no, None(FinishPass) |
| 25 | Pin Drill | no radial, no depth (a peck) | `Drill` | none | DrillCycle, no, None(DrillCycle) |

Notes that the implementer must hold:

- The `ring` fact is today's test of `axes_plan` (`feeds/envelope.rs:2461`): the Pocket
  family, Roughing, and `Radial::Width(_)`. It is a family test, not geometry: Face and Zigzag
  are "ring" by it, and Bore (the Pocket family, `Radial::Slot`) is "no". `cuts()` must
  reproduce that test exactly in this step.
- Row 15: Waterline is SemiFinish; `RadialThinning::of` gives `Applied` for it
  (`feeds/geometry.rs:62`), and `depth_cap_rule` gives none.
- Rows 4 and 14 declare the planner cap because `PowerCases` reads it today. The capped steps
  (`CutKind::CappedStep`) are NOT declared in this step.
- Spring passes, cleanup, slot lines of row 14, the Pocket Zigzag pattern, the Bore wall
  finish: not declared; on the exemption list.

## 3. `Cut::of_sample`

`Cut::of_sample(sample, &SampleContext) -> Option<Cut>`; `None` for an air sample.
`SampleContext` is built once per toolpath (the same lifetime as `tool_load::
ToolpathLoadContext`) and holds: the built tool, the span lookup, a `Vec<Option<
FeedCapSource>>` by move index, a bit set of capped moves, the `CutFacts` of the op.

The simulation sample must NOT grow: its size is pinned (G-SIMMEM, `stock/simulation_cut.rs`
near line 862: "a sample does not carry it (its size is pinned)"). So the kind comes from side
tables, by `sample.move_index`:

| `CutKind` | Source today | Missing today |
|---|---|---|
| `Entry(Plunge | Ramp | Helix)`, `EntryApproach` | `sample.source_intent` (`MoveIntent::{EntryPlunge, EntryRamp, EntryHelix, EntryApproach}`); the triage reads it the same way (`stock/sim_triage.rs:1402`) | `PeckLadder` has no intent: it is a run of `EntryPlunge` with retracts; `compute::catalog::entry_records` joins the run after generation. `of_sample` gives `Plunge`. |
| `LeadIn`, `LeadOut`, `Link`, `Drill` | `sample.source_intent` | A `Linking` move inside an `Entry` span (95 to 596 samples per 3D rough, `phases/P29-0_probes.md` section 7.2): the intent says `Link`, the span says entry. Rule: the intent wins; the list names the case. |
| `Cleanup(WaterlineSweep)` | `SpanKind::WaterlineCleanup` in `sample.span_path` (the span lookup) | The span covers every move from the cleanup marker to the next depth pass (`compute/spans.rs:588-605`), links included. Spans can be invalid after a transform (`annotated.spans_valid == false`): then the kind is `Body` and the list names the gap. |
| `Cleanup(ResidueMop | ContourResidue)` | none | No marker: the residue cleanup of a geodesic slice and of 2D Adaptive is emitted as `ClearingCut` inside the depth pass (`adaptive3d/clearing.rs:3020`, `adaptive/path.rs:1874`). Not told apart in this step. |
| `CappedStep(source)` | The `FeedCap` items of the semantic trace (`trace::semantic_trace`, `SemanticKey::CapSource`, `FeedCapSource::from_key`) by move range, with `Move::feed_cap` on the move | The semantic trace is optional on a result. Without it a capped move is `CappedStep` with no source; add `FeedCapSource`-less variant or fall back to `Body` (Q4). |
| `SlotRing` | A capped `ClearingCut` move with no `FeedCap` item (the Pocket contour first ring, `ops/pocket.rs:431`) | Inferred, not stated. P29-3 commit C adds a `FeedCapSource`-like marker for it, or leaves the inference (Q4). |
| `SpringPass`, `FirstPass(..)` | none | No marker on a move. A first row or a planner-cap step is a `Body` sample. |
| `Body` | `MoveIntent::{ClearingCut, FinishingCut}` with none of the above | `MoveIntent::Unknown` (an untagged generator): `Body`, and the list names it. |

The kind then stands in the place of span ancestry for the scope question. In THIS step the
gates keep `is_steady_state_for_gate`; a sentry holds the two equal:

```
is_steady_state_for_gate(sample, lookup)  ==  Cut::of_sample(sample, ctx).kind.is_judged_today()
```

where `is_judged_today()` is one `match` in the exemption table. A mismatch found while
writing that sentry is a finding, and goes to the lead before any change (the P29-0 probe
found two candidates: the `Linking` samples inside entry spans, and 273 `EntryPlunge` samples
of Profile after tabs that sit in no span).

Per-sample cost: one array index for the cap table, one span walk that the gates already do
(`span_path` is 2 to 4 ids), one `match`. No allocation. The gates build the engagement they
build today; `Cut::of_sample` adds the kind and the facts around it.

## 4. The exemption list

New file `limits/exemptions.rs`: one `static EXEMPTIONS: [Exemption; N]`.

```rust
pub struct Exemption {
    pub id: &'static str,            // "P28-01", "M1", "S-NoBound"
    pub limit: &'static [LimitId],   // which limits
    pub who: Who,                    // Facts(..) | Kind(CutKind class) | Undeclared(&'static str) | Reader(&'static str)
    pub class: ExemptionClass,       // Physical(Geometry) | NeedsRuling | Ruled { by, date, text }
    pub where_: &'static [&'static str],   // file paths of the scope reads (no line numbers)
    pub closes_in: &'static str,     // "P29-4", "P29-6", ...
}
```

`Physical` takes `limits::Geometry` (the closed list of P29-2: `NoRadialEngagement`,
`NotAnOffsetRing`, `NoVerticalFeed`) or it is `NeedsRuling`. There is no third way to write
"kept" or "deferred". `Ruled` quotes the operator ruling and its date.

### 4.1 The rows (the first version of the table)

Class: P = physical, R = needs a ruling, O = has an operator ruling.

| Id | Limit | Who is exempt today | Class | Closes in |
|---|---|---|---|---|
| P28-01 | ForceBudget | `depth_rule == Factor` (the Adaptive family) | R | P29-4 |
| P28-02 | ForceBudget | `depth_rule == None(FinishPass)`: wall and surface finishes | O (R2, 2026-09-23) | stays, as a ruling |
| P28-03 | ForceBudget | slot and groove cuts filed `FinishPass` (Trace, VCarve, the Inlay ops, Chamfer, Project Curve) | R | P29-5 |
| P28-04 | ForceBudget | `None(DrillCycle)` | P (NoRadialEngagement) | - |
| P28-05 | width rule (most removal) | not `offset_ring`, Adaptive | R | P29-4 / a width ruling |
| P28-06 | width rule | finish cuts with a stepover | R | later |
| P28-07 | CornerCoverage | not `offset_ring` | P (NotAnOffsetRing) | - |
| P28-08 | a width ceiling of an adaptive cut | Adaptive | R (no source) | ruling R2 of `P29_FINDINGS.md` section 7 |
| P28-09 | MoveBudget | ops with no move model, no stepover | R | later |
| P28-10 | SpindlePower (as a limit) | `Drill` | R (no end-cut force source) | P29-8 |
| P28-11 | every pre-sim limit that reads a depth | `DepthAfterSimulation` (Project Curve) | R | P29-9 |
| P28-12 | SpindlePower, the slot case | undeclared first slots (3D Adaptive sweep, Face region, 2D Adaptive without slot clearing) | R | P29-5 / P29-7 |
| P28-13 | SpindlePower, the planner width | `CapMult` (contour parallel 3D rough) | O (Q2: known unsourced) | later |
| P28-14 | pass ceiling | contour parallel 3D rough | R | later |
| P28-15 | ToolDeflection before the simulation | `FinishPass` and SemiFinish cuts | R | P29-7 |
| P28-16 | ToolDeflection, two limit values | a rough cut: 200 um before, the leave after | R | P29-7 |
| P28-17 | ToolDeflection | `Drill` | P (NoRadialEngagement) | - |
| P28-18 | chip-thinning lift | `thins_chip == FinishPass` | O (2026-11-05) | stays |
| P28-19 | axial thinning of a ball | ball, tapered ball | R | later |
| P28-20 | ChipMax, ChipMin | `Drill` | P (NoRadialEngagement) | - |
| P28-21 | every gate | `Entry(_)` samples | R | P29-6 |
| P28-22 | every gate | `Cleanup(WaterlineSweep)` samples | R | P29-6 |
| P28-23 | every gate | `LeadOut`, `Link`, dogbone samples | R (the reading is of neighbour stock: a measurement fact, not geometry of the cut) | P29-6 |
| P28-24 | VendorDepthTable | a peck | P (NoRadialEngagement) | - |
| P28-25 | VendorAxial | ops with `AxialEnvelope::NotApplied` | R | later |
| P28-26 | CuttingFeedCap arms (Step 7) | `Drill` | P (NoRadialEngagement; the drill takes Step 9c) | - |
| P28-27 | tip plunge cap | flat, bull, V-bit | R (no source) | P29-8 |
| P28-28 | plunge-entry stability | every family but Adaptive | R | P29-8 |
| P28-29 | modulator load caps | `LeadIn`, `LeadOut` moves | R | P29-6 |
| P28-30 | modulator load caps | rapid, retract, `Drill`, `Entry(Plunge)`, `EntryApproach` | R for the plunge (no model), P for the rest | P29-8 |
| P28-31 | the default cut | per (family, role) table | R (no source) | P27 |
| P28-32 | the depth the calculator reads | helix pitch, chamfer width, peck | R | P29-9 |
| P28-33 | ToolDeflection, chip at a V tip | V-bit | R | later |
| P28-34 | ForceBudget, other readers | as P28-01 to 04 (modulator, card, depth gate) | follows the four | P29-4 |
| M1 | ChipMax, ChipMin | samples below 95 % of the steady feed | R | P29-6 |
| M2 | the gates against the modulator | entry ramp and helix: capped by the modulator, judged by no gate | R | P29-6 |
| M3 | every gate | entry samples go to `entry_spike`, not to the verdict | follows P28-21 | P29-6 |
| M4 | SpindlePower against ToolDeflection | two depths for one sample | R (a probe) | P29-6 |
| M5 | ForceBudget (depth gate) | every sample but the peak-depth one | R | P29-6 |
| M6 | ChipMax, ChipMin | the vendor row is chosen by the op kind (3D Adaptive, Project Curve) | R | P25 / P27 |
| M7 | width rule | Adaptive excluded by name on an axial bar | follows P28-05 | P29-4 |
| M8 | triage readers | the span predicate with no span lookup drops entries | R | P29-6 |
| M9 | ZAxisRate in the RPM choice | not a drill | P (NoVerticalFeed for a side cut's RPM; its plunge reads the rate) | - |
| M10 | modulator planner floor | by strategy and move intent | R | later |
| M11 | SpindlePower slot case | wrong `FirstPass` filings (Face region, 3D sweep, Bore depth, VCarve stepover, Inlay one pass) | R | P29-5 |
| M12 | ForceBudget, ToolDeflection | `FirstPass` and planner-cap cuts (only power reads them) | R | P29-7 |
| S-NoBound | ForceBudget | `NoNumberReason::NoBound` (the depth row of a finish) | = P28-02 | - |
| S-NotJudgedHere | every envelope limit | a limit on a fixed axis; a per-point limit; the drawn move-budget curve (`NoNumberReason::NotJudgedHere`) | R | P29-5 / P29-10 |
| U-* | every limit | one row per undeclared cut of section 2: spring passes, residue cleanup, Pocket Zigzag pattern, Bore wall finish, tab re-plunge, row connectors, capped steps, perimeter sweep | R | P29-5 to P29-8 |

Count for the first table: 34 + 12 + 2 + about 8 undeclared rows = about 56. Physical: 8
(P28-04, 07, 17, 20, 24, 26, M9, and the non-plunge half of P28-30). Operator-ruled: 3
(P28-02, P28-13, P28-18). The rest need a ruling. The implementer fixes the exact count when
the table is code, and the sentry pins it.

P28 classes "physical" that this table does NOT accept as physical: P28-02 (a ruling, not
geometry), P28-23 (a measurement fact). The lead rules whether a measurement fact gets its own
class (Q5).

### 4.2 How the sentry holds "the list only shrinks"

`crates/rs_cam_core/tests/the_exemption_list_only_shrinks_p29_3.rs`:

1. The count of rows with `NeedsRuling` is pinned to a number in the test (`MAX_NEEDS_RULING`).
   The test fails when the count is ABOVE the pin, and also when it is BELOW it (so a closed
   exemption lowers the pin in the same commit; the pin is always exact).
2. Every `Physical` row names a `Geometry` variant (the type holds this).
3. Every `Ruled` row has a non-empty quote and a date.
4. Every id is unique, and every id of `phases/P28_limit_audit.md` (34) and of
   `phases/P29_parts/A_cuts_and_limits.md` section 3.3 (M1 to M12) is in the table. The two id
   lists are constants in the test, not a read of `planning/` (a test that runs reads no
   `planning/` path; `crates/rs_cam_core/tests/CLAUDE.md`).
5. Source scan, the link between the table and the code: each scope read in product code
   carries a marker comment `// EXEMPT(<id>)` on its line or the line above, and the test
   asserts (a) every `where_` file of a row holds a marker with that id, (b) every marker id is
   in the table. A new scope read without a row then needs a new marker, and a new marker needs
   a new row, which moves the pin. This is the weak point of the design: a scope read with no
   marker is not found by this scan. Section 7 names the stronger scan that P29-last adds.

## 5. Which sites move in this step

Rule of the step: a site MOVES when it can read the new type with the same bytes. A site
WAITS when the move changes scope or needs a ruling.

### 5.1 The 24 `Engagement` constructor sites

Counted in `phases/P29_parts/A_cuts_and_limits.md` section 1.2 (`from_cut` 10, `from_tool_cut`
9, `from_sample` 5). After P29-1 they build the engagement and pass it to `CutLoad`.

| Sites | This step |
|---|---|
| The 5 `from_sample` sites (`tool_load/power.rs`, `display.rs`, `depth.rs`, `dressup/feed_modulation.rs`, `compute/simulate.rs`) and the 6 `deflection::sample_engagement` sites | WAIT for commit C of this step only as a sentry (`Cut::of_sample` beside them, held equal). They read `Cut` in P29-6, when the gates judge by kind. |
| The planned sites of the calculator and the envelope (`feeds/mod.rs` Step 6, `feeds/operating_point.rs` x2, `feeds/calculator/close.rs`) | MOVE in commit B: they take their widths from `cuts()` in place of `PowerCases::of_operation`. Byte risk: the order of the cases and the tie rule of `PowerCases::worst` (a later case wins a tie). `cuts()` must give the cases in that order. |
| `feeds/force_budget.rs` (2), `feeds/predict.rs`, `feeds/cutter_constraints.rs` (2), `feeds/efficiency.rs`, `compute/execute/finish_raster.rs`, `tool/mod.rs` (a test) | STAY: they hold a tool and a cut point, no operation; a `Cut` adds nothing until `check_all` (P29-4). |
| `dressup/feed_modulation.rs` (`ModulationCutter::engagement`) | WAITS for P29-6. |

### 5.2 The 19 `cut_spec()` sites

`feeds/operating_point.rs` (5), `trace/lineage.rs` (3), `feeds/envelope.rs` (2),
`feeds/suggest/apply.rs` (2), `crates/rs_cam_viz/src/ui/feeds/envelope.rs` (2), and one each in
`feeds/suggest.rs`, `feeds/predict.rs`, `feeds/force_budget.rs`, `feeds/engagement.rs`,
`session/compute/toolpath_diagnostics.rs`.

| Sites | This step |
|---|---|
| `feeds/engagement.rs` (`PowerCases::of_operation`) | MOVES: `PowerCases` is built from `cuts()` (commit B). `FirstPass` and `planner_width()` keep one reader each until then, and none after. |
| The 7 `depth_after_simulation()` reads (`feeds/operating_point.rs` x3, `feeds/predict.rs`, `feeds/envelope.rs`, `feeds/calculator/*` through `FeedsInput`) | MOVE to `DeclaredDepth::None(DepthAfterSimulation)` in commit B only if the lead wants it now; else WAIT for P29-9 (Q6). No byte risk: it is one bool. |
| `feeds/envelope.rs` `axes_plan` | WAITS. It maps `Radial` and `Axial` to the envelope axes; `cuts()` does not replace that. Only its `computed_width` line moves (to `facts.offset_ring`). |
| `trace/lineage.rs` (3), viz (2), `feeds/suggest/apply.rs` (2), `feeds/suggest.rs`, `feeds/force_budget.rs`, `toolpath_diagnostics.rs` | STAY: they read the operator's stated values, which `CutSpec` keeps (`P29_FINDINGS.md` 4.1: "`CutSpec` stays as the operator's statement of the body pass"). |

### 5.3 The span filter sites

`rg` at `447cf7d8b` gives 17 call sites of the three predicates outside `tool_load/
locality.rs`: `is_steady_state_for_gate` in `tool_load/depth.rs:188`, `power.rs:261`,
`cut_force.rs:127`, `distribution.rs:745, 782`, `chipload.rs:458`, `mod.rs:302`,
`compute/simulate.rs:1116`, `stock/sim_triage.rs:944, 1713`; `is_phantom_transit` and
`is_configured_entry` in `power.rs:418, 421`, `deflection.rs:381, 384`, `chipload.rs:750, 753`,
`session/compute.rs:2271`. (Part A counted 10 for the first predicate.)

ALL WAIT for P29-6. This step adds `Cut::of_sample` and the equality sentry of section 3 only.
Each site gets its `// EXEMPT(P28-21)`, `(P28-22)`, `(P28-23)` or `(M8)` marker.

### 5.4 The 6 `depth_cap_rule` readers

`tool_load/depth.rs:240`, `tool_load/cut_force.rs:150`, `feeds/suggest/invariants.rs:225`,
`feeds/envelope.rs:3177`, `feeds/force_budget.rs:508`, `session/compute.rs:2316`.

MOVE in commit B: each reads `facts.depth_rule` of the op's `Body` cut in place of
`depth_cap_rule(family, role)`. `depth_cap_rule` stays as the ONE filler of the fact (inside
`cuts()`), with the marker `// EXEMPT(P28-01, P28-02, P28-04)`. Byte risk: none in arithmetic;
`force_budget::rigidity_depth_cap` takes `(family, pass_role)` in its signature today and is
called by tests (`crates/rs_cam_core/tests/feeds_matrix_instrument_fm1.rs:696`,
`the_depth_that_cut_is_a_measured_load_g_s3depth.rs`, `finish_depth_is_reported_not_capped_
fm6.rs`). The signature change touches those tests (a compile fix, no value).

### 5.5 The straight plunge and `efficiency.rs` onto `CutLoad`

Left out of P29-1 (`feeds/cut_load.rs` module doc):

- The straight plunge: `PowerModelInputs::axial_plunge` at `feeds/mod.rs` (`axial_power_terms`)
  and `feeds/operating_point.rs` (`resolve_axial_point`). With `CutEngagement::AxialPlunge`,
  `CutLoad` gains `of_cut(&Cut, tool, line)`, and its `power_terms` matches the engagement:
  `Side` as today, `AxialPlunge` through `PowerModelInputs::axial_plunge`, `NoModel` gives
  `NoNumber::NotModelled`. Force, deflection and chip of an axial plunge are `NoNumber`. MOVES
  in commit A. Byte risk: the flute count is `f64::from(u32)` on both paths today; keep it.
- `feeds/efficiency.rs:209` (the lift) and `:296` (`specific_energy`): it reads the NOMINAL
  diameter, not the engaged diameter, and builds no `Engagement`. Two ways: (a) build the
  engagement of the stated cut (`EngagedDiameter::of_hint`) and read `CutLoad::thinning_lift`;
  on a flat tool the two diameters are equal, on a ball they differ, so (a) MOVES NUMBERS on
  ball and tapered tools; (b) leave it and list it. Proposal: (b) in this step, with an
  exemption row `U-efficiency`; it joins in P29-11 (`P29_FINDINGS.md` row P29-11, "efficiency
  through the engagement"). Q7.

## 6. The commits

Each commit is byte-identical: FM1 unchanged, no pin moved, every existing sentry green.

Measured gate times on this machine (debug build, `scripts/cargo_lane.sh`, `-j 2`), from the
P29-1 and P29-2 runs:

| Gate | Command | Time |
|---|---|---|
| G1 | `cargo fmt --all -- --check` | seconds |
| G2 | one new core sentry, `cargo test -p rs_cam_core -j 2 --test <name>` | 27 to 30 s build, under 1 s run |
| G3 | 8 to 12 named core sentries in one command | 5 to 30 s when built |
| G4 | `cargo test -p rs_cam_core -q --lib -j 2` | 1 min 51 s to 2 min 12 s. `feeds::envelope::tests::the_envelope_meets_its_time_budget` goes red under this loaded run (a known timing trap); run it alone (119 to 130 ms against a 300 ms budget) |
| G5 | FM1: `cargo test -p rs_cam_core -j 2 --features heavy-tests --test feeds_matrix_instrument_fm1` | 1 min 10 s to 1 min 15 s |
| G6 | full clippy (the command of the root `CLAUDE.md`) | 2 min 50 s to 3 min 03 s |
| G7 | `cargo check -p rs_cam_core --all-targets -j 2` | 1 min 42 s |
| G8 | viz: `cargo test -p rs_cam_viz -j 2 --lib` | 50 s when built (706 tests) |
| G9 | viz: three named tests (`mcp_wire_surface_pin`, `the_limit_rows_read_their_own_bound_g_ownbound`, `ribbon_and_mcp_diagnostic_ids_n4`) | 54 s |

Rules that held in P29-1 and P29-2 and hold here: every cargo command through `timeout <s>
scripts/cargo_lane.sh <args> -j 2`, in the foreground; no workspace-wide `cargo test`; no core
full gate; no viz test while `rs_cam_gui` runs; a red test or a moved pin stops the work and
goes to the lead with no edit and no bless; the implementer does not commit.

### Commit A: the types and the list (no reader)

- `limits/cut.rs`: `Cut`, `CutKind`, `CutEngagement`, `CutSource`, `CutFacts`.
- `limits/exemptions.rs`: the table. The `// EXEMPT(id)` markers in product code (comments
  only).
- `feeds/cut_load.rs`: `CutLoad::of_cut`; the straight plunge through it (5.5).
- `feeds/envelope.rs`: `Limit::depth_rule` (`#[serde(skip)]`); `limits::DepthRule::of_limit`
  reads it.
- Sentries: `the_exemption_list_only_shrinks_p29_3` (section 4.2);
  `one_cut_gives_one_load_on_every_door_p29_1` gains the axial plunge door;
  `every_limit_is_a_limit_id_p29_2` keeps its claims (the `DepthRule` claim now reads the field).
- Gates: G1, G2 (two sentries), G3 (`pre_sim_power_ships_the_worst_case_k7`,
  `power_reads_the_true_engaged_section_p20d`, `a_non_centre_plunge_cuts_the_ring_p22_0c6`,
  `the_envelope_keeps_each_limit_k2b`, `folder_maps_name_real_files_f140`), G4, G5, G6. About
  9 minutes.

### Commit B: `cuts()` and the planned readers

- `compute/catalog/cuts.rs`: `cuts()`, `CutList`, `DeclaredCut`.
- `PowerCases` is built from `cuts()`; the 6 `depth_cap_rule` readers, `computed_width`,
  `RadialThinning::of` and `exceeds_limit` read the facts (5.1, 5.2, 5.4, 1.4).
- Sentry `every_op_declares_its_cuts_p29_3`: for each of the 25 ops (the fixtures of
  `crates/rs_cam_core/tests/common/op_fixtures.rs`, and each `ClearingStrategy`, `slot_clearing`
  on and off, Pocket contour and zigzag): (1) the widths of `cuts()` equal `PowerCases::
  of_cut(cut_spec, planner_width).widths(ae, D)` in order; (2) the facts equal the four label
  functions of section 1.4; (3) the entry kinds equal `EntryContract::record_kinds()`; (4) a
  source scan: `FirstPass`, `planner_width(` and `depth_cap_rule(` have no reader outside
  `compute/catalog/cuts.rs` and their definitions.
- Gates: G1, G2, G3 (add `every_op_states_its_cut_k1`, `every_op_states_its_entry_contract_k4`,
  `the_lift_reads_the_pass_ceiling_width_p23a`, `the_roughing_depth_meets_the_force_budget_p13c`,
  `finish_depth_is_reported_not_capped_fm6`, `the_depth_that_cut_is_a_measured_load_g_s3depth`,
  `the_modulator_reads_the_depth_gate_force_budget_rule_p16`), G4, G5, G6, G8, G9. About 12
  minutes. FM1 is the main proof here.

### Commit C: `Cut::of_sample` and the kind sentry

- `limits/cut.rs`: `Cut::of_sample`, `SampleContext`; `CutKind::is_judged_today`.
- Sentry `a_sample_states_its_cut_kind_p29_3`: on generated and simulated fixtures (a Pocket
  with a ramp entry, a 3D Adaptive geodesic slice with capped steps and a waterline cleanup, a
  Profile with tabs and leads) the equality of section 3 holds for every sample, and each
  `CutKind` that the fixture should hold has at least one sample (non-vacuity).
- Gates: G1, G2, G3 (`gate_population_vacuity_xvac`, `arcfit_gate_population_d4`,
  `a_move_holds_its_worst_sample_p20g`), G4, G6. The fixture simulation time is not measured;
  estimate under 1 minute for small fixtures. No wanaka run.
- If the equality does not hold on a class of samples (section 3 names two candidates): stop,
  report, and the lead rules whether `is_judged_today` gets a named arm for the class.

A two-commit split is possible: A + B together, then C. Three is safer: commit B is the only
one that touches the calculator path.

## 7. Byte-identity risks

1. `PowerCases` from `cuts()` (commit B): the case order and the tie rule. `PowerCases::worst`
   gives a tie to the LATER case (`feeds/engagement.rs`, `p >= best.1`). FM1 holds
   `power_case` per cell; it is the proof.
2. `RadialThinning` for SemiFinish (Waterline): `of(family, role)` gives `Applied`,
   `of_lut_role` gives `Applied` too; one fact, but two constructors exist. Keep both results.
3. `offset_ring` must equal `computed_width` exactly, with its `Radial::Width(_)` test
   (Bore is false; a Pocket with an unset stepover is true).
4. `leave_after_mm`: `exceeds_limit` has four cases (`FinishTarget`, `NoLeaveField`,
   `StockToLeave`, `ZeroLeave`) and prints a different clause for each. The fact must keep the
   four, so it is not a plain `Option<f64>`: use `Leave { FinishPass, NoField, Zero, Mm(f64) }`.
5. `Limit::depth_rule` must not reach the JSON (`#[serde(skip)]`); `Limit` derives `PartialEq`,
   and viz tests compare limits: a new field takes part in the comparison. Check
   `crates/rs_cam_viz/tests/every_limit_is_drawn_and_named_p5d.rs` and
   `the_envelope_plot_shows_only_what_limits_the_cut_p5c.rs` for struct literals of `Limit`.
6. `rigidity_depth_cap` changes its signature (5.4): three test files call it.
7. The `// EXEMPT` markers are comments. A source-scanning test that counts lines or matches
   text near a marked line can move: `one_cut_gives_one_load_on_every_door_p29_1` (it strips
   comments, safe), `every_limit_is_a_limit_id_p29_2` (strips comments, safe), the F140 folder
   map test (reads `CLAUDE.md` files; `limits/CLAUDE.md` and `compute/CLAUDE.md` need the new
   files in their maps), `every_feeds_formula_constant_states_its_source_f201` (no new constant
   is planned; if `CutKind` needs one, it needs an F201 row).
8. `limits/` must not import `compute::`. `cuts()` is in `compute/catalog/`, and `CutKind`
   wants `EntryKind` and `FeedCapSource`. The P29-2 sentry scans `src/limits/` for
   `compute::`; `adaptive3d::FeedCapSource` passes, `compute::catalog::EntryKind` does not (Q1).

The stronger exemption scan (not in this step): P29-last deletes the scope reads; then a scan
for `OperationFamily`, `PassRole`, `OperationType` and `SpanKind` in `feeds/`, `tool_load/` and
`dressup/feed_modulation.rs` holds "no limit reads the op type" with no marker comment.

## 8. Open items carried from P29-1 and P29-2

- M4: two depths for one sample (`axial_doc_mm`, `axial_engagement_mm`).
- The operating point differs by door (the flute count and the chip rule): on a sample with 3
  flutes against a tool with 2, power x 1.31, force and deflection x 0.84, chip x 0.67
  (`crates/rs_cam_core/tests/one_cut_gives_one_load_on_every_door_p29_1.rs`, claim C).
- `tool_load/depth.rs` and the mean chip of `feeds/operating_point.rs` build a chip outside
  `OperatingPoint`.
- `Check::bound_detail` is transitional (it goes at P29-10).
- `Basis::setting` is exact only for a basis from `limits::basis_of_bound`.
- Nine value-and-bound records have no projection to `Check` (`EntrySpike`,
  `EntryLoadObservation`, `PlungeStressWarning`, `CutForceReading`, `CutEfficiency`,
  `RuleResult`, `PowerFigure`, `DeflectionPrediction`, `MeasuredPower`).
- A tool description of the MCP server can change with no pin (the wire snapshot holds names
  and input schemas only).

## 9. Questions for the lead

- Q1. `CutKind` reads `compute::catalog::EntryKind`. Move `EntryKind` into `limits/cut.rs` and
  re-export it from `compute/catalog/entry_contract.rs` (my choice: `limits/` stays free of
  `compute::`), or allow the one import?
- Q2. `CutFacts` holds label values in this step (`thins_chip` from the role, `offset_ring`
  from the family). Is that acceptable as "the structure with today's behaviour", with each
  one on the exemption list? The other way is to wait with the facts until P29-4 to P29-7.
- Q3. Undeclared cuts (spring passes, residue cleanup, the Bore wall finish, tab re-plunges,
  row connectors, capped steps, the perimeter sweep): one exemption row each (my choice), or
  one row for all?
- Q4. `SlotRing` and a capped step with no semantic trace are inferred today. Add a planner
  marker in commit C (a product change in `ops/pocket.rs`, byte risk on the trace), or keep the
  inference and list it (my choice for this step)?
- Q5. P28-23 (lead-out, link bridge, dogbone samples read neighbour stock): a measurement
  fact, not geometry of the cut. A fourth class `MeasurementArtifact`, or `NeedsRuling`?
- Q6. The seven `depth_after_simulation()` reads: move to `DeclaredDepth` in commit B, or wait
  for P29-9?
- Q7. `feeds/efficiency.rs`: leave on the nominal diameter and list it (my choice), or move it
  to the engaged diameter now (numbers move on ball and tapered tools)?
- Q8. The marker comment `// EXEMPT(id)` as the link between the table and the code: accept the
  weak point of section 4.2 for this step?
- Q9. Three commits (A, B, C) or two (A + B, C)?
- Q10. The `Linking` samples inside entry spans and the Profile `EntryPlunge` samples in no
  span (section 3): if the kind and the span filter disagree there, which one states today's
  behaviour in `is_judged_today`? (The span filter is what the gates run, so my choice is: the
  filter, with a named arm.)

## 10. Lead rulings (2026-11-11, before the hand-over)

The design is approved with these answers. No code exists for P29-3 yet.

- Q1. Move `EntryKind` into `limits/cut.rs` and re-export it. `limits/` imports no `compute::`.
- Q2. Yes. The facts hold today's label values in this step, filled at one place. Each one is
  a row of the exemption list. P29-4 to P29-7 replace each label value with geometry.
- Q3. One exemption row for each undeclared cut. A shared row hides which cut a later step
  closes.
- Q4. Keep the inference in this step and list it. A planner marker is a product change; it
  belongs to P29-6, with a prediction of the trace bytes that move.
- Q5. `NeedsRuling`. The class list stays at three. The operator decides at P29-6 how a sample
  that reads neighbour stock is judged; the lead does not close it as a fact of measurement.
- Q6. Wait for P29-9.
- Q7. Leave `feeds/efficiency.rs` on the nominal diameter and list it. It moves at P29-11.
- Q8. Accepted for this step. P29-last deletes the scattered reads, and its search guard
  replaces the marker comment.
- Q9. Three commits (A, B, C). Each is byte-identical and has its own gates.
- Q10. The span filter states today's behaviour, in a named arm of `is_judged_today`. Both
  mismatches (the `Linking` samples inside entry spans, the Profile `EntryPlunge` samples in no
  span) are rows of the exemption list.

One more rule for the implementer: the count of "needs a ruling" rows is pinned by the sentry.
A step that lowers it names the operator ruling (R1 to R6, or a new one) in the commit text.
