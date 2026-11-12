# P29-7 design: every limit before the simulation judges every declared cut

Written 2026-11-12 on HEAD `8413af726`. No product code changed. The lead reviews this note,
commits it with the two prediction files, and then starts an implementer. The result of each
commit is judged against section 10. A mismatch stops the work.

Files of this step:

- this note;
- `P29-7_predictions_fm1.csv`: the FM1 cell values that move (P29-6 columns). 32 rows: 11
  `proposal_ap_mm` cells (probe), 11 `binding_limit` cells (reasoned), 10 `proposal_ae_mm` cells
  that the removal-rate pick can move (not predicted; the implementer reports them).
- `P29-7_predictions.csv`: table `W` (both wanaka files, every declared cut of every op, read-only
  loads), table `S` (the FM1 cells that move, with the cut that binds), table `H` (hand checks).

Order assumed: P29-5b (option A), P29-5c, then this step. The probe ran on the tree after 5b and
5c. Its wanaka table was run with the 5b option B filing (the slot on every 3D rough), so it holds
both answers: column `proposal_pred_A` drops the geodesic slot, column `proposal_pred_B` keeps it.

Rulings in force: R1 (the force budget, 35.3224 N; tuning parked by the operator, 2026-11-12:
"rabbit hole", tune later; no new value is proposed here), R3 (a slot is a slot), R4 (every cut
group is judged), R5 (lead default, no operator view: "a rough pass may bend up to the stock it
leaves, never above 200 um; the same value before and after the simulation"). Every finding warns;
no job is refused. No legacy: a rule, field or type that no reader reads after the step is deleted.

The operator's intent (2026-11-12), the acceptance of the design: "I want to leave the code in a
place where its blatently obvious to anyone in the future that adding a mechanism like force neds
to happen acreoss all movement", and "Solid codebase with well-engrained patterns". P29-6 gave the
post-simulation side that shape (one scope table, one driver, a fixed record). P29-7 gives the
pre-simulation side the same shape, on the same scope table.

## 1. Scope

### 1.1 The sources

| Source | What it puts in P29-7 |
|---|---|
| Change list | "Force and deflection read every declared cut (slot ring, planner cap). One deflection limit for one cut. Finish deflection before the simulation. Moves Pocket and Profile depths." Acceptance: "Suggest's cut passes the gates on every declared cut." Ruling R5. |
| Rows whose `closes_in` names P29-7 (13 at HEAD, 14 after 5b option A) | `P28-15`, `P28-16`, `M4`, `M12`, `F-LeaveAfter`, `F-ThinsChip` and `F-OffsetRing` ("P29-4 to P29-7"), `U-SpringPass`, `U-ResidueCleanup`, `U-PocketZigzag`, `U-BoreWallFinish`, `U-RowConnector`, `U-CappedStep`; `U-PerimeterSweep` after 5b option A |
| Hand-over, P29-3 and P29-6 | the readers that WAIT for body facts in `FeedsInput`: Step 5 of the calculator (`feeds/calculator/feed.rs:102`, `RadialThinning::of(input.operation, input.pass_role)`), 83 `FeedsInput {` literal sites |
| Lead | the two traps of P29-4: `propose` reads the stated chip while the card judges at `max(stated, suggested)`; `propose` checks no Runtime width floor |

### 1.2 A verdict for each row

| Row | Verdict | Why |
|---|---|---|
| `M12` (force and deflection never see the slot ring, the first slot, the planner cap) | close, 7c | the planned driver judges every declared cut |
| `P28-15` (no pre-sim deflection on a finish cut) | close, 7d | the deflection limit runs for every role where the depth axis is free |
| `P28-16` (deflection: 200 um before, the leave after) | close, 7d | R5: one value, `min(leave, 200 um)`, on the envelope, the gate and the modulator |
| `F-LeaveAfter` (the leave from the role) | close, 7d | the leave comes from the op's radial leave field only (`radial_stock_to_leave_mm`); the role is not read (2.7) |
| `U-SpringPass`, `U-ResidueCleanup`, `U-PocketZigzag`, `U-BoreWallFinish`, `U-RowConnector`, `U-CappedStep` | close, 7b | each becomes a declared cut with its feed rule; where no pre-sim number exists, the record says why (`NoNumber`) |
| `U-PerimeterSweep` (option A of 5b: the geodesic residual sweep) | close, 7b | declared with `Presence::MayOccur`: the limits answer it as a warning, not a ceiling (Q1) |
| `M4` (one sample, two depths) | re-file: "P29-10 (one depth predicate, P29-6 F6)" | a post-simulation question; no pre-sim reader has two depths. Text only |
| `F-ThinsChip` | narrow, 7a: Step 5 reads the fact (`body_facts.thins_chip`), so `feed.rs` leaves the row; the filler still reads the role (`P28-18`, ruled: a finish pass does not thin). `closes_in` "later (no geometry rule for a finish pass; P28-18 is a ruling)" | |
| `F-OffsetRing` | re-file: "P29-9" | the offset-ring fact is the Pocket-family width rule; no P29-7 reader changes it |

### 1.3 The cut that P29-7 adds a number to, and the cuts it cannot

Before the simulation each declared cut has a width rule (`DeclaredWidth`) and, after 7b, a feed
rule. A limit answers a cut when both give a number:

| Declared cut | Width | Feed (7b) | Force and deflection before the simulation |
|---|---|---|---|
| `Body` | the stated stepover, or D for a slot or groove (5c) | the op feed | judged (today) |
| `FirstPass(PlannerCap)` | `c D` of the stepover (geodesic, 2D Adaptive) or `min(m ae, D)` (ContourParallel, `trochoid_cap_mult`) | the op feed: the planner holds every fed step up to this width at the full feed | judged (new) |
| `FirstPass(Slot)` | D | the op feed | judged (new) |
| `SlotRing` (contour Pocket) | D | unlifted (`UnliftedFeed::of_slot_ring`, P23d) | judged (new) |
| `SpringPass`, `Cleanup(_)`, `CappedStep(_)`, Bore wall finish | no stated width | op feed, or capped by the planner | `NoNumber(NoStatedWidth)` or `NoNumber(PlannerCappedFeed)`; the gates judge them after the simulation (P29-6) |
| `Link` (row connectors of Face and Zigzag) | the stepover | the op feed | judged (new; never binds below the body: same width) |
| `Entry(_)` | no model | entry rules | `NoNumber` (P29-8) |
| `FirstPass(Slot)` with `Presence::MayOccur` (geodesic residual sweep) | D | the op feed | a warning row, not a ceiling (Q1) |

So "Suggest's cut passes the gates on every declared cut" holds for every cut with a number. For
the cuts with `NoNumber` the acceptance is weaker: they are named on the card and judged after the
simulation. The design says so on the record; it does not hide them.

## 2. The design: the pre-simulation side mirrors P29-6

### 2.1 One scope table for both sides

`limits::scope(LimitId, CutGroup)` stays the one table. A declared cut reaches it through
`CutKind::group()`, as a sample does. No second table, no "planned" column: the physics is the
same before and after the simulation (a slot is judged by force whether planned or measured). The
difference before the simulation is the engagement (an entry has no model yet), and that is a
fact of the cut (`CutEngagement::NoModel`), not a scope.

One change of the answer type: `Scope::NotASampleLimit` becomes
`Scope::NotACutLimit(PlanReason)`, with a closed list of reasons. The old name was true only for
the post-simulation side, and a bare "not a cut limit" would hide a scope decision: some envelope
limits that read only the body cut today DO depend on the cut. The 23 limits that answer
`NotASampleLimit` today, split by one test (does the bound change with the width or the chip of
the cut?):

| Limit | Depends on the cut's width or chip? | Answer after 7c |
|---|---|---|
| `VendorDepthTable` | yes (the chip share `fz sin psi / band max`: a slot has `sin psi = 1`) | a cut limit (Q8). My choice: it joins the planned driver in its own commit 7c2, after a prediction run of its own (no number of this note covers it). Until 7c2 it reads the body cut only, as today |
| `RubbingFloor` | yes (the chip) | a cut limit: it joins `ChipMin` as a value check per declared cut (no ceiling, no proposal moves) |
| `FluteGuard`, `CuttingLength`, `VendorAxial` | no: the depth alone | `NotACutLimit(PlanReason::DepthOnly)`: every declared cut of one depth gives the same answer |
| `CornerCoverage`, `MoveBudget`, `EngagementFloor`, `ToolDiameter` | no: a rule of the body plan | `NotACutLimit(PlanReason::PlanLevel)` |
| `CuttingFeedCap`, `PlannerFeedCap`, `KinematicReach`, `PlungeRate`, `SpindleMaxRpm`, `ZAxisRate`, `OperatorRpmCeiling`, `ChartRpm`, `VendorRpmMax`, `GantryPush` | no: the machine, the RPM choice or the feed | `NotACutLimit(PlanReason::MachineOrRpm)` |
| `DrillChipWelding`, `DrillPeckAdequacy`, `DrillPlungeFeed` | no: the drill cycle | `NotACutLimit(PlanReason::DrillGate)` |

`PlanReason` is a closed enum; a new limit must pick one reason or answer per group, and the
compiler asks.

### 2.2 The declared cut carries its feed rule and its presence (7b)

`compute/catalog/cuts.rs`:

```rust
/// The feed at which the emitter cuts a declared cut.
pub enum DeclaredFeed {
    /// The op feed (the emitter writes `feed_rate`).
    OpFeed,
    /// The op feed over the lift of the body (P23d: the slot ring).
    Unlifted,
    /// A feed the planner caps on its own stock read; no pre-sim number.
    PlannerCapped,
}

/// Whether the plan always cuts a declared cut.
pub enum Presence {
    /// Every level cuts it.
    Always,
    /// The plan cuts it only where a generation fact asks (the geodesic
    /// residual sweep); the limits answer it as a warning.
    MayOccur,
}
```

`DeclaredCut` gains `feed` and `presence`. `Declare::cut` takes both; every arm of `cuts()` states
them. One match on `CutKind` with no wildcard gives the default feed of each kind, so a new kind
does not compile until it states its feed.

The cut list after 7b, per op (new cuts in bold):

| Op | Declared cuts |
|---|---|
| Pocket, contour | Body, SlotRing (unlifted), **SpringPass**, entries. The duplicate `FirstPass(Slot)` of a contour Pocket goes: the ring IS the first slot, and it runs unlifted. `PowerCases` reads `SlotRing` as the slot case (byte-identical power) |
| Pocket, zigzag | Body, FirstPass(Slot) (op feed), **Link** (row connectors), **SpringPass**, entries |
| Profile | Body (slot), FirstPass(Slot), **SpringPass**, entries |
| Zigzag | Body, FirstPass(Slot), **Link**, entries |
| Face | Body, FirstPass(Slot) on a region (5b), **Link**, entries |
| Adaptive (2D) | Body, FirstPass(PlannerCap), FirstPass(Slot) with slot clearing, **Cleanup(ResidueMop or ContourResidue)** by `cleanup_strategy`, **CappedStep(DepartureWindow)** (planner capped), entries |
| Adaptive3d | Body, FirstPass(PlannerCap), FirstPass(Slot) (ContourParallel: Always; geodesic: **MayOccur**), **Cleanup(WaterlineSweep)**, **Cleanup(ResidueMop)** (geodesic), **CappedStep(StockRead3d)** (planner capped), entries (4) |
| Bore | Body (slot, pitch depth after 5b), FirstPass(Slot), **Bore wall finish** (width = the allowance, no stated depth), entries |

`MAX_DECLARED_CUTS` rises from 8 to 12 (Adaptive3d geodesic: 11). `CutList::push` drops a cut
silently above the bound today; 7b changes it to fill a fixed array whose length the sentry
`every_op_declares_its_cuts_p29_3` checks for every op fixture, and `push` returns a `Result` that
`cuts()` handles with a named fallback (no panic in product code).

The Bore wall finish needs a kind. My choice: a new `CutKind::WallFinish` in the `Body` group
(the compiler then asks every match on `CutKind` for it, including `group()`, `CutKind::of_sample`
and the cut engagement). The other choice reuses `SpringPass` with a widened doc.

### 2.3 One function per limit, one driver over the declared cuts (7c)

New file `feeds/planned.rs` (it cannot live in `limits/`, which imports nothing from `compute/`;
`feeds` already reads `compute`).

```rust
/// One declared cut at one body column of the envelope plane.
pub struct PlannedCut {
    pub kind: CutKind,
    pub group: CutGroup,
    pub width_mm: Result<f64, NoEngagementModel>,
    pub chip_mm: Result<f64, PlannedNoNumber>,
    pub presence: Presence,
}

/// A limit before the simulation: one function over one planned cut.
pub trait PlannedLimit {
    const LIMIT: LimitId;
    fn depth_ceiling(cut: &PlannedCut, env: &PlanEnv<'_>) -> CeilingAnswer;
}

pub enum CeilingAnswer {
    Depth(f64),
    NoRootBelowFlute,
    NoNumber(NoNumberReason),
}

/// THE planned driver: for each declared cut and each planned limit, the
/// scope cell; a judged cell calls the limit; the result is one record per
/// limit with one cell per declared cut (`ByCut`, private fields, one
/// constructor that calls the closure once for each declared cut).
pub fn judge_declared_cuts(cuts: &CutList, column: BodyColumn, env: &PlanEnv<'_>)
    -> PlannedJudgement;
```

The driver:

1. builds the `PlannedCut` of each declared cut at the body column `(ae, chip)`: the width by
   the cut's `DeclaredWidth` (`Stated` -> `ae`; `Slot` -> D; `PassCeiling` -> `c(ae) D`;
   `CapMult` -> `min(m ae, D)`; `Scallop`, `None` -> `NoModel`), the chip by the cut's
   `DeclaredFeed` (`OpFeed` -> the body chip; `Unlifted` -> the body chip over the body lift;
   `PlannerCapped` -> `NoNumber`);
2. asks `limits::scope(L::LIMIT, cut.group)` for each planned limit;
3. calls `L::depth_ceiling` on each judged cell; a `NotJudged` cell keeps its row id and clause;
4. returns one cell per cut per limit. No limit decides a scope, and no limit walks the cut list.

The planned limits of 7c: `ForceBudgetPlanned` (`force_budget::depth_at_budget` at the cut width
and chip, through the machine's `depth_cap_of(depth_rule)`), `DeflectionPlanned`
(`cutter_axial_constraints` at the cut width and chip, at the R5 limit of 2.7). `SpindlePower`
joins in 7c with the same rule as `PowerCases` today (every declared cut at the op feed, so the
figure is byte-identical) and `PowerCases` is deleted (Q5 decides the unlifted ring power).
`ChipMax` and `ChipMin` join as value checks (the planned peak chip of each cut against the
band); they write `Check` rows and no ceiling.

The envelope (`feeds/envelope.rs`) then draws ONE edge per limit kind: at each width column, the
lowest `Depth` over the declared cuts with `Presence::Always`. The `Limit` gains
`binding_cut: CutKind` at the proposal (the card prints "Rigidity, slot ring"). A cut with
`Presence::MayOccur` adds a note and a `Check` row with its own value, not an edge. The
`rigidity_limit` and `deflection_limit` functions become callers of the driver; their scope reads
(`EXEMPT(M12)` at `envelope.rs:3220`, `EXEMPT(P28-15, M12)` at `:2932`, `EXEMPT(M12)` at
`cuts.rs:389`) go.

The removal-rate pick (`propose_removal_rate`) and `propose` read the same edges, so the proposal
of a roughing op is the most removal inside every declared cut.

### 2.4 What a future author does to add a limit before the simulation

1. Add a `LimitId` member. The compiler stops at three matches with no wildcard:
   - `limits::scope` (each `CutGroup` cell: `Judged`, `NotJudged` with its row, or
     `NotACutLimit`);
   - `feeds::planned::planned_limit_of(LimitId)` (the planned evaluator, or `None` for a limit
     whose scope is `NotACutLimit`);
   - `tool_load::groups::gate_of(LimitId)` (the gate; 7c turns today's hand list `GATE_LIMITS` into
     this match, so the post-simulation side is compiler-held too).
2. Write the planned evaluator: one `impl PlannedLimit` with `depth_ceiling` over one cut.
3. Write the gate: one `impl MetricEvaluator` with `judge` over one group's samples (P29-6).

The compiler forces step 1 and the signatures of steps 2 and 3. A sentry
(`tests/every_limit_judges_every_declared_cut_p29_7.rs`) holds what the compiler cannot: a limit
whose scope judges a group has both an evaluator and a gate, and the two evaluators read one
`CutLoad` (non-vacuity: at least the force budget and the deflection are found).

The note goes into `limits/CLAUDE.md` "How to add a limit" in 7c, in place of today's three steps.

### 2.5 `FeedsInput`: one constructor (7a)

Today `FeedsInput` has 25 public fields and 83 literal sites: 1 product door
(`suggest::feeds_input_for_operation`, `feeds/suggest.rs:775`), 41 lib-test sites (39 in
`feeds/tests.rs`, 1 in `feeds/explain_payload.rs`, 1 in `feeds/vendor_normalize.rs`) and 41 sites
in 28 integration test files. 73 sites state `operation_kind: None`, 77 state
`PowerCases::NOMINAL`, 32 hand-set a depth, 17 a width, 27 a finish role.

The rule, as in P29-6 6a: **the facts of the calculator input come from the operation, and only
from it.**

1. `FeedsInput::of_operation(operation, tool, material, machine, lut, spindle_strategy)` is the
   one constructor (the body of `feeds_input_for_operation`, which is deleted; its callers move).
   It fills `body_facts: CutFacts` from `operation.cuts().body_facts()`, the power cases (after 7c:
   the declared cuts), the row key (family, role, kind) and the hints.
2. `FeedsInput` moves into its own module `feeds/input.rs` and gains one private zero-size field
   `door: Door`, where `Door` is a private type of that module. Every literal outside that module
   fails (E0451), `feeds/tests.rs` included, and so does a struct update from a constructed base
   (`FeedsInput { x, ..base }` needs the private field too). The other fields stay readable as
   `pub(in crate::feeds)` (no accessor churn in the calculator). A test that needs a changed
   input calls one of a few named `with_*` methods of `input.rs` that state why they exist (for
   example the setup context of a long-tool derate test), or states the op that gives the input.
3. Step 5 (`feeds/calculator/feed.rs:102`) reads `input.body_facts.thins_chip` in place of
   `RadialThinning::of(input.operation, input.pass_role)`. `label_facts` fills `thins_chip` from
   the same two inputs, so the read is byte-identical. `operation` and `pass_role` stay as the
   vendor row key (a row key, not a scope, as `ToolpathLoadContext::chip_row_key`).
4. A test that needs a depth or a width states it on the op (`set_depth_per_pass`,
   `set_stepover`). A test that wants "no op" states the op it means: a (Pocket, Roughing) fixture
   with no slot case is a stock Face; a finish fixture is a Scallop or a Waterline.

Is 7a byte-identical? In the product, yes (one door, the same values). In the tests, no: 26
Pocket-family literals state `PowerCases::NOMINAL`, and a Pocket op declares a slot case; 21
Adaptive-family literals state no planner width, and an Adaptive op holds the pass ceiling (the
lift moves). The implementer moves each to the op the test means (a stock Face for a plain
Pocket-family cut) and reports the count of test expectations that still move; each is a red to
name, not to bless.

### 2.6 The two traps of the proposal

**Trap 1: two chips.** The card (`placement_of`, `judged_ceiling`) judges the force-budget depth at
`max(stated chip, suggested chip)` (`BudgetChip::Pick`); `propose` reads the curve chip (the stated
chip). Lead proposal: `propose` reads the pick chip. Effect, computed from the probe: 0 FM1 cells
(no FM1 cell states a chip, so the two chips are one); operator file 0 ops (the stated chip is
above the suggested chip on idx 1, 2, 14, 15); work file idx 1, 2, 15 by 0.0001 mm (the suggested
chip is about 1e-5 mm above the stated chip: 3.9543 -> 3.9542, 3.6696 -> 3.6695, 3.7837 ->
3.7836, the judged ceilings the probe printed). The rivmap 3D Rough of the hand-over (4.217 x
2.303 mm, white oak) moves from 4.217 to 3.27 mm (hand-over number, not re-run). My choice: fix it
in 7c, because the planned driver reads one chip per cut and the pick writes the suggested speeds.

**Trap 2: no Runtime floor in `propose`.** At a stated width (R2: Suggest keeps a stated
stepover) the only lever is the depth, and the force budget caps the depth. When the move-budget
floor needs a deeper cut than the budget allows, no point meets both. My choice: no number moves;
the card and the record state the conflict ("the move budget is not met at the force-budget
depth"), as `propose` cannot raise a capped depth (K2) and must not widen a stated width (R2).
P29-7 lowers the 3D rough depths further (section 10.3), so the Runtime margin gets worse
(probe on HEAD, no product edit: `SafeEnvelope::placement_of` at today's proposal and at the
option A proposal, the same width):

| File | idx | Today | After 7c (option A) |
|---|---|---|---|
| operator | 2 | Outside Runtime -0.184 | Outside Runtime -0.219 |
| work | 1 | Outside Runtime -0.282 | Outside Runtime -0.319 |
| work | 2 | Outside Runtime -0.167 | Outside Runtime -0.203 |
| operator | 1, 14, 15; work 15 | Inside (margin 0.000; work 15 Outside Rigidity by 2e-5) | Inside (0.018, 0.021, 0.049, 0.049) |

The "today" values equal the P29-4 hand-over numbers. With the lead's fix as I propose it (warn,
do not move), these numbers stand and the card names the conflict.

### 2.7 One deflection limit (R5) and finish deflection before the simulation (7d)

The limit of every cut: `min(leave, 200 um)`, where `leave` is the op's radial stock to leave when
above 0 (`OperationConfig::radial_stock_to_leave_mm`), else none (then 200 um). The role is not
read. The four `Leave` arms collapse to two: `Leave::Mm(s)` and `Leave::None` (the finish target).
Today only Bore has a radial leave (`finish_allowance`); every other op has `None`
(`catalog.rs:1532`).

Readers that change:

- `tool_load::deflection::exceeds_limit`: `Leave::Mm(s)` gives `min(s, 0.2)` (today `s`, even above
  0.2 mm). `DeflectionBasis::StockToLeave` keeps the value it uses.
- `feeds/envelope.rs` `deflection_limit`: the target is the same value (today 200 um always); the
  role guard goes (P28-15), so a finish cut with a free depth axis (Trace, VCarve with a max depth,
  Inlay, InlayPlug, Waterline, Unified Finish, Steep/Shallow, Ramp Finish) gets a deflection edge or
  an `Unmodelled` "no deflection limit below the flute" entry.
- `session/compute.rs:2339` (the modulator's deflection cap): the same value (today 200 um always).
- Suggest's backoff (`DEFLECTION_BACKOFF_TARGET_UM`) and the F082 compliance door: the same value.

Modulator reach (the operator wants the modulation redesigned in its own step): 7d tightens the
modulator's deflection cap only on a Bore whose allowance is under 0.2 mm, and loosens nothing.
No FM1 or wanaka op has such a Bore. This widens the post-simulation feed change for that case
only; I name it here so the lead can hold it (Q4).

## 3. Commits

Four commits. Each passes the gate set alone. 7a and 7b move no product number.

| Commit | Content | Moves | Rows |
|---|---|---|---|
| 7a | `FeedsInput::of_operation`, the private door, `body_facts`, Step 5 reads the fact (2.5) | product: nothing (FM1 byte-equal); tests: the literal sites | `F-ThinsChip` narrows |
| 7b | `DeclaredFeed`, `Presence`, the new declared cuts, `WallFinish`, the contour Pocket dedupe, `PowerCases` reads `SlotRing`, `MAX_DECLARED_CUTS` 12 (2.2) | nothing (FM1 byte-equal; no reader reads a new cut yet) | the six `U-*` rows of 1.2 and `U-PerimeterSweep` |
| 7c | `feeds/planned.rs`, the driver, the envelope edges per limit over the declared cuts, `binding_cut`, `gate_of`, `NotACutLimit`, `PowerCases` deleted, trap 1 (2.3, 2.4, 2.6) | FM1 matrix: 11 `proposal_ap_mm` cells and their `binding_limit` (and possibly 10 `proposal_ae_mm` of the removal-rate pick); wanaka: the 3D roughs (section 10.3) | `M12` |
| 7c2 | `VendorDepthTable` joins the driver (Q8) | not predicted here: its own prediction run comes first | none |
| 7d | R5 and the finish deflection edge (2.7) | FM1: 0 cells (probe: no cell has a deflection root below the flute at 200 um, and no cell states a leave); wanaka: 0 ops | `P28-15`, `P28-16`, `F-LeaveAfter` |

Gates of every commit: fmt check; the named sentries `the_exemption_list_only_shrinks_p29_3`,
`every_limit_is_a_limit_id_p29_2`, `every_op_declares_its_cuts_p29_3`,
`a_sample_states_its_cut_kind_p29_3`, `no_removing_sample_is_outside_the_gates_p29_6`,
`a_slot_reads_the_force_budget_under_every_op_p29_5`, `the_roughing_depth_meets_the_force_budget_p13c`,
`the_pocket_proposal_is_the_removal_rate_maximum_p13c`, `the_add_door_stepover_is_the_proposal_p13c`,
`a_pocket_feeds_its_slot_first_ring_unlifted_p23d`, `the_lift_reads_the_pass_ceiling_width_p23a`,
`pre_sim_power_ships_the_worst_case_k7`, `the_envelope_keeps_each_limit_k2b`,
`suggest_never_raises_a_stated_depth_k2`, `an_envelope_edge_does_not_move_with_the_stated_cut_p19`,
`the_deflection_edge_is_a_solved_depth_p11a`, `the_envelope_states_its_deflection_abstention_t4`,
`a_rough_pass_deflects_up_to_its_stock_to_leave_p17`, `the_cut_force_is_the_deflection_force_p17`,
`the_session_modulator_binds_on_the_force_budget_p16`, `one_cut_gives_one_load_on_every_door_p29_1`,
the new `every_limit_judges_every_declared_cut_p29_7` (7c); `cargo test -p rs_cam_core -q --lib`;
FM1 (`--features heavy-tests`); full clippy; viz `--lib` and the viz tests
`the_envelope_plot_shows_only_what_limits_the_cut_p5c`, `every_limit_is_drawn_and_named_p5d`,
`mcp_wire_surface_pin`, `p17_visual_snapshots` (no viz test while `pgrep -ax rs_cam_gui` prints).

New sentry, 7c: `tests/every_limit_judges_every_declared_cut_p29_7.rs`. Claims, over every op
fixture (`fixture_op` of every `OperationType`) and the FM1 tool builder:

1. For every declared cut and every planned limit, the driver returns exactly one cell, and the
   cell's state follows `limits::scope(limit, cut.kind.group())` (no cut outside).
2. The envelope edge of each limit at the proposal width is the minimum over the `Always` cuts
   of their `Depth` cells (the edge holds every declared cut).
3. Suggest's proposal sits at or under every judged `Depth` cell of every `Always` cut (the
   acceptance of the change list).
4. Non-vacuity: a contour Pocket binds on `SlotRing` (FM1 EndMill 3.175 mm hardwood), a 3D rough
   on `FirstPass(PlannerCap)` (wanaka-like 6 mm geodesic op), and one cut of each `NoNumber`
   reason exists.

Inject the defect once (drop the `SlotRing` cell from the minimum) and confirm red.

## 4. Exemption rows and pins

Pins after 5b (option A) and 5c: ROWS 48, PHYSICAL 8, RULED 3, MAX_NEEDS_RULING 37,
ROWS_WITH_A_FILE 41.

| After | Rows closed | `ROWS` | `MAX_NEEDS_RULING` | `ROWS_WITH_A_FILE` | `CLOSED` gains |
|---|---|---|---|---|---|
| 7a | none | 48 | 37 | 41 | - |
| 7b | `U-SpringPass`, `U-ResidueCleanup`, `U-PocketZigzag`, `U-BoreWallFinish`, `U-RowConnector`, `U-CappedStep` (no file each), `U-PerimeterSweep` (one file after 5b A) | 41 | 30 | 40 | none (the `U-*` rows were never limit rows; the list of `CLOSED` keeps limit rows; the lead may add them) |
| 7c | `M12` | 40 | 29 | 39 | `("M12", "P29-7")` |
| 7d | `P28-15`, `P28-16`, `F-LeaveAfter` | 37 | 26 | 36 | `("P28-15", "P29-7")`, `("P28-16", "P29-7")`, `("F-LeaveAfter", "P29-7")` |

`PHYSICAL` 8 and `RULED` 3 stay. Text-only changes: `M4` (`closes_in` "P29-10"), `F-ThinsChip`
(7a: `where_` loses `src/feeds/calculator/feed.rs`; `closes_in` "later"), `F-OffsetRing`
(`closes_in` "P29-9").

Markers that go: `EXEMPT(M12)` at `cuts.rs:389` and `envelope.rs:3220`; `EXEMPT(P28-15, M12)` at
`envelope.rs:2932`; `EXEMPT(P28-16)` at `session/compute.rs:2339`; `EXEMPT(P28-16, F-LeaveAfter)`
at `deflection.rs:143` and `:281`; `EXEMPT(F-LeaveAfter)` at `cuts.rs:425`; the `F-ThinsChip` id
in `calculator/feed.rs:101` (the marker keeps `P28-18, P28-19`).

## 5. Tests that go red

Found by `rg` for `FeedsInput {`, `PowerCases`, `NotASampleLimit`, `exceeds_limit`,
`Leave::FinishPass`, `Leave::NoField`, `Leave::Zero`, `DEFLECTION_BACKOFF_TARGET_UM`,
`MAX_DECLARED_CUTS`, `GATE_LIMITS`, and by the probe. Not run: no test ran on a P29-7 edit. A red
outside this list in the gate set is a mismatch; a red inside it with a different reason is also a
mismatch.

**7a (the constructor):**

- Compile: the 41 integration literal sites (`rg -l "FeedsInput \{" crates/rs_cam_core/tests`:
  28 files, among them `wanaka_defaults_validation`, `tapered_width_model_parity_c3`,
  `rubbing_floor_envelope_band_p1`, `a_vbit_row_is_read_at_its_printed_key_b4`,
  `power_ceiling_parity_f2`, `lut_resolver_purposes_a7`, `a_feed_lift_caps_at_the_cutting_ceiling_g_t18`,
  `_litmatrix_rubbing_floor_clamp`, `the_power_ladder_pulls_the_right_lever_g_ladder`,
  `chipload_boundary_g_chip_ulp`, `literature_matrix/shim.rs`); the 41 lib-test sites (39 in
  `feeds/tests.rs`, `explain_payload.rs`, `vendor_normalize.rs`).
- Behaviour, where a literal moves to a real op: the Pocket-family literals with
  `PowerCases::NOMINAL` (26) and the Adaptive-family literals with no planner width (21), unless
  the implementer picks the op that keeps the meaning (2.5 item 4). The count is a report item.

**7b:** `tests/every_op_declares_its_cuts_p29_3.rs` (claim 1: the written-out `want` list gains
the new kinds and loses the contour Pocket `FirstPass(Slot)`; the `MAX_DECLARED_CUTS` bound);
`tests/a_sample_states_its_cut_kind_p29_3.rs` and `no_removing_sample_is_outside_the_gates_p29_6`
(claim 2 "emitted kinds are declared kinds" can only get greener: more kinds are declared; a red
here is a mismatch); every exhaustive match on `CutKind` in core and viz (compile, `WallFinish`).

**7c:**

- FM1 matrix golden (section 10.1).
- `tests/pre_sim_power_ships_the_worst_case_k7.rs`: `PowerCases` is deleted; the claims move to
  the driver (the same figures).
- `tests/the_pocket_proposal_is_the_removal_rate_maximum_p13c.rs`: if it pins a Pocket proposal of
  a cell where the slot ring binds (FM1 3.175 mm hardwood cells), the pinned depth moves; new
  claim: the maximum inside every declared cut.
- `tests/the_roughing_depth_meets_the_force_budget_p13c.rs`, `the_adaptive_*` P29-4 tests that
  pin a 3D rough depth at the body width: the depth moves to the planner-cap depth (for example
  the wanaka-like 6 mm geodesic fixture, 1.2 mm stepover: 3.9543 -> 3.7499 at the work file's
  chip).
- `tests/every_limit_is_a_limit_id_p29_2.rs` and `the_exemption_list_only_shrinks_p29_3.rs`:
  `NotASampleLimit` -> `NotACutLimit` (compile); the pins.
- `tool_load::groups::GATE_LIMITS` readers (the P29-6 sentry): move to `gate_of`.
- viz: the envelope plot and the card captions that name the binding limit (they gain the cut
  name); `mcp_wire_surface_pin` (the `get_safe_envelope` reply gains `binding_cut` and the
  per-cut `checks`: a stated wire change).

**7d:** `tests/a_rough_pass_deflects_up_to_its_stock_to_leave_p17.rs` (a leave above 0.2 mm now
gives 0.2 mm; new claim: `min(leave, 0.2)`); `tool_load/deflection.rs` lib tests of
`exceeds_limit` (the four arms become two); `the_envelope_states_its_deflection_abstention_t4` and
`the_deflection_edge_is_a_solved_depth_p11a` if they hold a finish fixture with a free depth axis
(a new entry appears); `rs_cam_viz` tests that pin "Finish target" deflection captions.

## 6. Docs to change

- `FEATURE_CATALOG.md`: the envelope paragraph (every limit reads every declared cut; the card
  names the cut that binds); the deflection paragraph (one limit, `min(leave, 200 um)`, before and
  after the simulation, every role); the Suggest paragraph (the proposal is inside every declared
  cut; a cut with no pre-sim number is named).
- `crates/rs_cam_core/src/limits/CLAUDE.md`: "How to add a limit" (section 2.4); `NotACutLimit`;
  the open item on the WAIT readers closes.
- `crates/rs_cam_core/src/feeds/CLAUDE.md`: `planned.rs` in the file map; the K7 line
  ("Pre-sim power is the maximum over the `PowerCases`") becomes the driver; `FeedsInput` door.
- `crates/rs_cam_core/src/tool_load/CLAUDE.md`: the P17 line (Suggest, the envelope and F082
  keep the finish target) becomes R5; `gate_of`.
- `crates/rs_cam_core/src/compute/CLAUDE.md`: `cuts.rs` holds the feed rule and the presence.
- `P29_FINDINGS.md` section 6, the P29-7 row: the row verdicts of 1.2.

## 7. Risks and questions for the lead

Most important first. Each question has my choice.

**Q1. The geodesic residual sweep (5b option A).** My choice: declared with `Presence::MayOccur`;
the limits answer it as a warning row with its own number at the proposal depth, not a ceiling.
The probe gives that number for each wanaka 3D rough (table `W`, cut `FirstPass(Slot)`): for
example operator idx 2 at the proposal depth is over the budget (the slot root is 2.7306 mm, the
proposal 3.4391 mm). The other choice (5b option B, `Always`) lowers every geodesic rough to the
full-slot depth (operator idx 2: 3.5932 -> 2.7306 mm; column `proposal_pred_B`).

**Q2. The planner cap binds the 3D roughs.** The planner holds every fed step up to `c D` at the
full feed, so the force at `c D` is a real steady cut. 7c lowers the wanaka 3D rough proposals by
2 to 6 % (operator idx 1, 2, 14, 15; work idx 1, 2, 15). My choice: accept; this is what the
change list asks ("planner cap"). Every number depends on the 35.3 N budget.

**Q3. The removal-rate pick can move the width.** A slot-ring cap is the same at every body width,
so the most-removal pick of a Pocket or Zigzag may choose a wider width once the depth is capped.
The probe predicts the depth at today's width, not the new width. My choice: accept and have the
implementer report the 10 `proposal_ae_mm` cells; a width move inside the corner-coverage
ceiling is the rule doing its job.

**Q4. 7d widens the modulator for a Bore with a small allowance.** My choice: keep it in 7d; it
tightens only, and only on that op. The other choice leaves the modulator at 200 um until the
modulation design step (then `P28-16` narrows to the modulator and stays).

**Q5. The power of the slot ring.** The ring runs unlifted (P23d), so its power is lower than
`PowerCases` says (it sizes the slot at the op feed). My choice: 7c keeps today's figure
(byte-identical) and names the gap; a later commit gives power the feed rule and moves the FM1
Pocket `power_kw` cells (not predicted).

**Q6. Trap 1 (pick chip) in 7c.** My choice: yes (2.6). It moves 0 FM1 cells, 0 operator-file ops,
three work-file proposals by 0.0001 mm.

**Q7. `WallFinish` or reuse `SpringPass`.** My choice: a new kind (the compiler then asks every
match); the cost is one arm per match.

**Q8. The vendor depth table on the declared cuts.** Its bound depends on the chip share of the
cut, and a slot has the largest share, so on a slot, a ring or a planner cap the table can give a
lower ceiling than on the body. No FM1 cell binds `VendorDepthTable` today (binding counts: none
930, `CornerCoverage` 66, `VendorAxial` 4). My choice: a separate commit 7c2 that puts it in the
driver, with its own prediction run before the commit (the cells that can move: every roughing
cell with a slot, a ring or a planner cap and a matched row with a chip band). The other choice is
a `NotJudged` cell with a new exemption row; that grows the list, against the sentry's "only
shrinks", so it needs the lead's word.

Risks:

- 7a touches 82 test literals. Some tests built a calculator input that no op gives on purpose
  (for example a (Pocket, Roughing) input with no slot). Each moves to the op it means; a test
  whose claim needs an input that no op gives is a test of a state the product cannot reach, and
  is deleted with a stated reason.
- The driver runs per width column of the envelope (`ENVELOPE_GRID_N` columns) and per declared
  cut: up to 12 cuts x 2 solved limits x the columns. The envelope has a time budget sentry
  (`the_envelope_meets_its_time_budget`, a known timing trap). The implementer reports the
  envelope time of the FM1 Adaptive3d cell before and after.
- The force budget is probably low for a partial-width cut (hand-over, P29-4 findings). Every
  depth that 7c lowers is a budget value. A later tune moves them all with no code change.

## 8. The operator's concern: what the simulation shows after modulation

P29-7 changes no post-simulation number on FM1 or wanaka, and it does not widen the modulator's
reach except the Bore case of Q4. The proposals it lowers make the simulated cut lighter when the
operator applies them, so fewer moves cross a modulator cap. No modulation redesign is in this
step.

## 9. Budget, placement, and export

- Every 7c depth on FM1 and wanaka depends on the budget value (35.3224 N): the slot-ring depths,
  the zigzag first-row depths, the planner-cap depths, the CP slot depths. A different budget
  moves each of them; the probe and the hand checks state the budget they used.
- The card placement after 7c reads the same edges as the proposal (one chip, trap 1), so the card
  and the proposal agree on every declared cut.
- Export: no new finding refuses (K6). A planned `Check` row that exceeds is a warning on the card
  and on the export report, never a refusal.

## 10. Predictions

Computed on HEAD `8413af726` plus the 5b edit (option B filing, so the wanaka table holds both
options) and the 5c edit, with a throwaway probe test and no P29-7 product edit (a full prototype
of the driver is too large for a throwaway edit). The probe builds every declared cut of each op
from `OperationConfig::cuts()`, takes its width by the rule of 2.3, its chip by the rule of 2.2
(the body chip: the stated chip on wanaka, the envelope grid chip at the proposal width on FM1;
the slot ring: the body chip times the unlifted feed over the op feed), and solves
`force_budget::depth_at_budget` and `cutter_axial_constraints` at the R5 target. The predicted
proposal is the minimum of today's proposal and every non-body ceiling (a contour Pocket reads its
`SlotRing`, not the duplicate `FirstPass(Slot)`). Every file was restored from a byte copy
(`sha256sum -c` OK, touched); the probe file was deleted; `git status --short crates/` is clean;
the FM1 fixtures hash as before.

Calibration: the probe's "today" proposal equals the FM1 `proposal_ap_mm` of every non-refused
cell it read on the same tree (0 mismatches, 430 cells). No FM1 cell binds `Rigidity` today, so
the body ceiling is calibrated on wanaka: the probe's body ceiling at the stated chip equals the
`Rigidity`-bound proposal of operator idx 1 (2.0924), idx 2 (3.5932), idx 14 (2.4547), idx 15
(3.6983) and work idx 1 (3.9543 against 3.9543, the pick chip 1e-5 mm higher).

Which numbers come from the probe: every `proposal_ap_mm` of 10.1, every ceiling of table `W`
and table `S`, the hand checks. Which come from reasoning: the `binding_limit` and
`proposal_ae_mm` cells, 7a, 7b and 7d moving nothing, the test list, the trap 1 and trap 2
effects except the work-file values the probe printed.

The measurement the implementer reports before the lead commits 7c: the FM1 matrix diff (it must
equal the 11 `proposal_ap_mm` cells, except the conditional cell below; the 10 `proposal_ae_mm`
and the 11 `binding_limit` cells are reported and judged; the Zigzag TaperedBallNose 6 mm
hardwood depth is conditional: it is solved at the lifted chip of today's width 1.0527 mm, and if
the pick widens, the lift falls and the ceiling rises towards the unlifted value 4.0493 mm); the wanaka proposals of both files (read-only loads, they must equal
column `proposal_pred_A`); the envelope time of one FM1 Adaptive3d cell.

### 10.1 FM1

Matrix file, 7c: 11 rows.

| Op | Tool, D, material | Today | Predicted | Binding cut | Width, chip |
|---|---|---|---|---|---|
| Pocket | EndMill 3.175 hardwood | 6.9850 | 4.9846 | SlotRing | 3.175, 0.0889 |
| Zigzag | EndMill 3.175 hardwood | 6.9850 | 4.9846 | FirstPass(Slot) | 3.175, 0.0889 |
| Pocket | BullNose 3.175 hardwood | 6.9850 | 4.9846 | SlotRing | 3.175, 0.0889 |
| Zigzag | BullNose 3.175 hardwood | 6.9850 | 4.9846 | FirstPass(Slot) | 3.175, 0.0889 |
| Pocket | BallNose 3.175 hardwood | 4.4723 | 4.4691 | SlotRing | 3.175, 0.1016 |
| Zigzag | BallNose 3.175 hardwood | 4.4723 | 4.4691 | FirstPass(Slot) | 3.175, 0.1016 |
| Pocket | TaperedBallNose 3.175 hardwood | 4.2518 | 4.1827 | SlotRing | 3.175, 0.1016 |
| Zigzag | TaperedBallNose 3.175 hardwood | 4.2518 | 4.1827 | FirstPass(Slot) | 3.175, 0.1016 |
| Pocket | TaperedBallNose 6 hardwood | 14.3398 | 4.0493 | SlotRing (unlifted) | 6.0, 0.0955 |
| Zigzag | TaperedBallNose 6 hardwood | 14.3398 | 3.0853 | FirstPass(Slot) (op feed) | 6.0, 0.1472 |
| Adaptive3d | TaperedBallNose 6 hardwood | 3.0000 | 2.0957 | FirstPass(Slot) (ContourParallel) | 6.0, 0.2500 |

Why so few: at the FM1 add-door widths (about 0.73 to 0.85 D for the Pocket family) the peak
force of the body cut equals the slot's (the flat stretch of the force over the width), so the
slot ring and the body give the same ceiling on most cells (for example EndMill 6 mm hardwood:
3.4056 both). Profile, Trace and Bore: the body is the slot already. Face (stock): no slot.
Adaptive (2D) and every other Adaptive3d cell: the stated depth is under every ceiling. No FM1
cell has a deflection root below the flute at 200 um (probe), so 7d moves 0 cells.

`binding_limit`: `Rigidity` where it is empty today (9 rows); "Rigidity or CornerCoverage" on the
two removal-rate rows where `CornerCoverage` binds the width today (Pocket and Zigzag, BallNose
3.175 mm hardwood; not predicted).
`proposal_ae_mm`: the 10 Pocket and Zigzag rows may move (Q3).

Simulation file: 0 cells (reasoned: the FM1 simulation ops keep the add-door geometry, the
apply writes speeds only; no sim op states a leave, so 7d moves no gate bound; the FM1
`deflection_cols` print the state, the peak and an unmodelled reason, not the basis clause, so
the collapse of the `Leave` arms moves no FM1 text. The card and export text of the deflection
basis do change: "finish target", "no leave field" and "zero leave" become one clause, and the viz
tests that pin those captions go red by design, 7d).

### 10.2 Suggest on the default machine (table `S`)

The 11 FM1 cells above are the default-machine Suggest cells that move; table `S` holds each with
its binding cut, width and chip.

### 10.3 The two wanaka files (table `W`)

Every declared cut of every op, both files. Option A (recommended for 5b): no geodesic slot.

| File | idx | Op (tool) | Proposal today | 7c, option A | 7c, option B | Cut that binds (A) |
|---|---|---|---|---|---|---|
| operator | 1 | 3D rough (18 mm) | 2.0924 x 5.5 | 2.0552 | 1.9220 | PlannerCap, 5.9913 mm |
| operator | 2 | 3D rest (6 mm) | 3.5932 x 1.5 | 3.4391 | 2.7306 | PlannerCap, 1.6380 mm |
| operator | 14 | 3D rough (18 mm) | 2.4547 x 3.6 | 2.4021 | 1.9970 | PlannerCap, 3.9395 mm |
| operator | 15 | 3D rough (6 mm) | 3.6983 x 1.382 | 3.5172 | 2.6807 | PlannerCap, 1.5104 mm |
| work | 1 | 3D rough (6 mm) | 3.9543 x 1.2 | 3.7499 | 2.6467 | PlannerCap, 1.3132 mm |
| work | 2 | 3D rest (6 mm) | 3.6696 x 1.5 | 3.5119 | 2.7849 | PlannerCap, 1.6380 mm |
| work | 14 | 3D rough (6 mm) | 3.0000 x 1.2 (stated) | 3.0000 | 2.6467 | none (3.7499 > 3.0) |
| work | 15 | 3D rough (6 mm) | 3.7837 x 1.382 | 3.5989 | 2.7364 | PlannerCap, 1.5104 mm |

Every other op of both files: no move. The Pockets with the 2 mm tool (idx 6, 7, 8) have no
force-budget root below the flute on any cut; the 18 mm clamp-pad Pocket (idx 11) states 0.5 mm
under its slot-ring ceiling 2.3452 mm; the Profile (idx 17) is a slot already (2.6974 mm > 1.25
mm stated); the Trace V-bit ops (idx 10, 13) stay under 4.5107 mm; the Faces use the stock
selection; the Project Curve ops propose no depth; the Scallop has no free depth axis. 7d: no op
states a radial leave, and no cut has a deflection root below the flute at 200 um.

Before a simulation the card of each 3D rough names the planner cap. After a simulation: the gates
judged the `c D` samples already (P29-6), so the post-simulation numbers do not move until the
operator applies the new depth.

Amendment (2026-11-12): section 15.3 holds the run of the prototype on HEAD `3e63a0902`. The
option A column above holds to 0.0001 mm; table `W` of `P29-7_predictions.csv` is replaced.

### 10.4 Hand checks (table `H`)

The peak force at the predicted ceiling, at the cut width and chip, against 35.3224 N:

| Family | Cell | Ceiling (mm) | Peak (N) | Peak 0.01 mm deeper (N) |
|---|---|---|---|---|
| Pocket slot ring | EndMill 3.175 hardwood, 3.175 mm, 0.0889 mm | 4.9846 | 35.3153 | 35.3579 |
| Pocket slot ring (unlifted) | TaperedBall 6 hardwood, 6 mm, 0.0955 mm | 4.0493 | 35.3122 | 35.3941 |
| Zigzag first row | TaperedBall 6 hardwood, 6 mm, 0.1472 mm | 3.0853 | 35.3146 | 35.4244 |
| 3D rough slot (CP) | TaperedBall 6 hardwood, 6 mm, 0.25 mm | 2.0957 | 35.2800 | 35.4449 |

The planner-cap ceilings of table `W` are direct `depth_at_budget` roots at the printed width and
chip (the same solver; no separate hand check).

### 10.5 What stays byte-identical

- 7a, 7b: every product output.
- 7c: every op whose declared cuts give no ceiling below its body ceiling (989 of the 1000 FM1
  matrix rows; every power, speeds, warning and cap column); every toolpath and G-code from a
  generation with no apply; every post-simulation number.
- 7d: every FM1 and wanaka output (no leave, no deflection root).

## 11. What changes for the operator's cut

Before a simulation, Suggest lowers the depth of each wanaka 3D rough a little, because the
planner may cut steps up to its pass ceiling (a bit wider than the stated stepover) at the full
feed, and those steps reach the force budget first:

- operator file: the 18 mm back rough 2.09 -> 2.06 mm, the 6 mm back rest 3.59 -> 3.44 mm, the
  18 mm front rough 2.45 -> 2.40 mm, the 6 mm front rough 3.70 -> 3.52 mm;
- work file: the 6 mm back rough 3.95 -> 3.75 mm, the back rest 3.67 -> 3.51 mm, the front rough
  3.78 -> 3.60 mm; the 3 mm front rough keeps 3.0 mm.

Nothing changes until the operator applies the proposal. After a simulation nothing changes either:
the gates already judged those steps. Every number depends on the 35.3 N budget, which the
operator parked for a later tune. If the lead takes option B of 5b, the roughs drop to the
full-slot depth (2.0 to 2.8 mm) instead.

## 12. Findings outside the scope

- **F1.** `CutList::push` drops a cut silently above `MAX_DECLARED_CUTS` (`cuts.rs:213-217`); 7b
  needs 11 cuts on a geodesic 3D rough.
- **F2.** A contour Pocket declares the first ring twice (`FirstPass(Slot)` from its cut spec and
  `SlotRing`), at two feeds. `PowerCases` reads only the first, at the op feed.
- **F3.** The envelope grid chip of a V-bit at `MIN_AP_MM` is tiny (FM1 Pocket V-bit 6.35 mm
  softwood: 0.0042 mm), so the force-budget columns of a V-bit Pocket read almost no force. The
  envelope rule (the grid chip at the shallowest sample) is old; for a cone the chip at the
  shallowest depth is not the chip of the cut.
- **F4.** The deflection solver finds no root below the flute on every FM1 cell and every wanaka
  op at 200 um. The deflection edge is drawn nowhere on these; whether the stickouts of the FM1
  tool builder (cutting length + 8 mm) are representative was not checked.
- **F5.** The finish ops hold `stock_to_leave` fields that are not a radial leave
  (`catalog.rs:1525-1531`). R5 reads only the radial leave, so a semi-finish that leaves stock for
  a final pass keeps 200 um.

## 13. Lead rulings (2026-11-12)

These rulings change parts of sections 1 to 4. Where they differ, the rulings win.

- **Q1: `Presence::MayOccur` is an exemption, and the compiler or a sentry must hold it.** A
  declared cut with `MayOccur` escapes the ceiling, so it gets the `NotJudged` treatment:
  `Presence::MayOccur` carries the id of an `EXEMPTIONS` row, and the sentry that holds the
  `NotJudged` links holds this link too. Row `U-PerimeterSweep` does NOT close in 7b. The sweep is
  a full slot at the lifted feed that no limit caps before the simulation. The row closes when the
  planner feeds the sweep unlifted (5b F1) or when the sweep is a ceiling.
- **Q2: accepted.** The planner cap is a ceiling.
- **Q3: not accepted as written.** 21 of the 32 FM1 cells have no prediction, and every Zigzag
  `FirstPass(Slot)` depth is solved at the lift of today's width. Before the 7c code starts, a
  probe run on the tree after 5b, 5c, 7a and 7b applies the new ceilings inside the removal-rate
  pick and prints `proposal_ae_mm`, `proposal_ap_mm` and `binding_limit` of every row that moves.
  The lead commits that amendment first.
- **Q4: held.** The modulator keeps 200 um. 7d changes the gate, the envelope, Suggest's backoff
  and the F082 door, not `session/compute.rs`. Row `P28-16` narrows to the modulator and stays
  open until the modulation design step ("the simulation shows the feeds that the export
  writes"). This is the same hold as P29-6 6e.
- **Q5, Q6, Q7: the note's choice.** For Q6 the amendment run adds a probe row for the rivmap 3D
  Rough (`planning/fixtures/`, rivmap100), with the planner cap, and a list of the tests that pin
  that depth.
- **Q8: no interim state.** `VendorDepthTable` depends on the cut, so it joins the driver in 7c.
  There is no commit 7c2. The amendment run predicts its cells. `NotACutLimit` is never its
  answer.

Pins with these rulings (after 5b option A and 5c: 48 / 37 / 41):

| After | Rows closed | `ROWS` | `MAX_NEEDS_RULING` | `ROWS_WITH_A_FILE` |
|---|---|---|---|---|
| 7b | the six `U-*` rows with no file | 42 | 31 | 41 |
| 7c | `M12` | 41 | 30 | 40 |
| 7d | `P28-15`, `F-LeaveAfter` | 39 | 28 | 38 |

The implementer recounts each pin from the array; the array wins over this table.

## 14. Result of 7a and 7b

Written 2026-11-12 by the implementer, on HEAD `267dee41c`. Two patches, in order:
`step_7a.patch`, `step_7b.patch`. No number of the product moved: FM1 is byte-equal after each
commit with no re-bless, and the op golden diff equals `opgolden_known_46_rows.txt` after each.

### 14.1 Commit 7a: one constructor of `FeedsInput`

- `FeedsInput` moved to `feeds/input.rs`. It has one constructor, `FeedsInput::of_operation`
  (the body of the deleted `suggest::feeds_input_for_operation`), and a private zero-size
  field `door: Door`. The other fields are `pub(in crate::feeds)`. A new field `body_facts`
  holds `operation.cuts().body_facts()`; Step 5 (`calculator/feed.rs`) reads
  `input.body_facts.thins_chip`. The marker there keeps `P28-18, P28-19`.
- Compile-fail proof (throwaway, deleted): a struct update in an integration test fails with
  E0451 on every field; a struct update inside `feeds/tests.rs` fails with E0451 on `door`.
  `rg -n "FeedsInput \{" crates/` finds no site (the struct line in `input.rs` reads
  `FeedsInput<'a> {`).
- The named `with_*` methods: `with_no_vendor_table` (the formula path; no product door reaches
  it) and `with_feed_max_rate` (the op feed cap of P21c; no product door states it, Q11). Read
  accessors where a reader outside `feeds` exists: `material`, `machine`, `tool_diameter`,
  `flute_count`, `cut_body`, `tool_geometry`, `tool_series`, `vendor_lut`.
- Deleted: `suggest::feeds_input_for_operation`, `PowerCases::NOMINAL` (no reader).
- Row `F-ThinsChip`: `where_` loses `src/feeds/calculator/feed.rs`; `closes_in` "later (no
  geometry rule for a finish pass; P28-18 is a ruling)".
- Sites moved: the 82 literals (39 `feeds/tests.rs`, 1 `explain_payload.rs`, 1
  `vendor_normalize.rs`, 40 in 27 core integration files, 1 viz test) and every caller of the
  old door (8 product files with `suggest.rs`, 29 test files with one viz test). New helper `tests/common/feeds_input.rs`
  (`op_of_row_key`, `op_of_row_key_with_depth`, `op_at_depth`, `op_with`, `tool_of_hint`).
  A (Pocket, Roughing) literal became a stock Face; a slot became a Pocket; a (Contour, Finish)
  cut became a Steep/Shallow; a (Scallop, Finish) cut with a depth became a Unified Finish; a
  (Parallel, Finish) cut with a depth became a Ramp Finish.
- Expectations that moved: 2, both in `_litmatrix_scallop_refuses_flat`. A Scallop op refuses a
  bull nose by its registry tool rule (old: `Ok`), and its refusal of a flat end mill names the
  op and its tools (old: the scallop-height text). Not blessed: the bull test is deleted (no
  Scallop-family op takes a bull nose), and the message claim moved to the drop cutter case,
  where the scallop rule is the rule that refuses (same text as before).
- The implementer's own deletions and narrowings (the limit was five deletions; one test was
  deleted), each with a proposal the lead can reverse:
  1. `scallop_accepts_bull_endmill` deleted; the flat-message claim moved to the drop cutter.
  2. `chipload_bounds_skip_doc_derating_for_drill_ops` lost its depth half (renamed
     `a_drill_cycle_reads_the_point_of_its_side_row`): no op gives a drill depth.
  3. `test_roughing_deeper_than_finishing` keeps one of seven families (Contour: Profile against
     Steep/Shallow); no op gives the other six pairs.
  4. `long_tool_derate_is_shown_ld1` lost its "no stickout" row: a tool states one.
  5. `lut_resolver_census_a6`: the pre-a4 arm (no op kind) is a Pencil, which routes as stated.
- Inputs that changed in tests that still pass:
  - The row-key sweeps skip a key that no op gives: `rubbing_floor_envelope_band_p1` keeps 7 of
    14 keys per tool and species; `lut_resolver_purposes_a7` keeps 5 of 10 (180 of 360 cells).
  - `a_tapered_row_is_read_at_the_tip_a1`: the kind is a Ramp Finish (was DropCutter), so that
    the input holds the depth.
  - `wanaka_defaults_validation`: Adaptive3d and Adaptive (was the Adaptive family), and
    ProjectCurve and DropCutter (was the Trace and Parallel families).
  - `a_vbit_row_is_read_at_its_printed_key_b4` (Trace): the tool tip is 0 (was 0.127 in the
    literal; the gate side already read 0), and the width is D (a Trace is a slot; was 0.3 D).
  - Depths that no op holds are gone: the drill depth of `g_t18`, the depth of
    `test_scallop_stepover_used_for_ball_nose`. Widths or scallop targets that no op of the key
    holds with a depth are gone: a7 purposes (Trace, Roughing is now an InlayPlug at
    (Trace, Finish)), the relief probe, c3, and the finish rows of the diameter-scaling
    instrument (its rows still print the width label that the calculator no longer reads).
  - The tool facts that the door reads: every test tool states its shank (the old input often
    had none; the lookup read the diameter, so the shank is the diameter where it was none),
    a stick-out of 4 x D (the long-tool share is 1, as with no overhang), and its fact kinds.
- Known red `_litmatrix_rubbing_floor_clamp::ipe_pocket_ships_its_computed_feed_above_the_floor_after_r4`:
  it fails with the text that `KNOWN_REDS_TRIAGE.md` B8 records (left
  `amana-flat-hardwood-pocket-6000-2f-spektra`, right `amana-compression-wood-pocket-6350-2f`).
  The red does not change. Not run at HEAD.

### 14.2 The readers of the cut list (checked before 7b)

`body_facts()` (first cut, the body stays first), `declares(Drill)`, `declares(SlotRing)` and
`of_kind(Body)` do not change. `PowerCases::of_cuts` would change twice, so 7b changed it: it
reads `SlotRing` as the slot case (the contour Pocket dedupe), and it skips a `MayOccur` cut
(the geodesic sweep). No product code iterates the whole list.

### 14.3 Commit 7b: the generated cuts are declared

- `DeclaredFeed` (`OpFeed`, `Unlifted`, `PlannerCapped`, and two not in 2.2: `EntryRule`,
  `DrillCycle`, because an entry and a drill cycle are not cut at the op feed) with one match
  `DeclaredFeed::of_kind` (no wildcard). Departure from 2.2: `Declare::cut` takes the presence
  only; the feed comes from `DeclaredFeed::of_kind`, so no arm can state a second feed for a
  kind. `PowerCases::of_cuts` now reads `Presence`. `Presence::{Always, MayOccur(MayOccur { row,
  reason }})`. `CutKind::WallFinish` (group `Body`; `CutKind::of_sample` does not emit it).
- `MAX_DECLARED_CUTS` 12. `CutList::push` returns `Result<(), CutListFull>`; `cuts()` keeps the
  first dropped kind (`CutList::dropped`), and the sentry holds `None` for every config.
- The lists are design 2.2 (the sentry writes them out for each op at its default, each 3D
  strategy, each 2D cleanup strategy and slot clearing, Pocket contour and zigzag with and
  without a spring pass, Profile with spring passes, Face on the stock and on a drawing region,
  Bore with and without an allowance), with the conditions of the generators: a spring pass only with
  `finishing_passes > 0`, a Bore wall finish only with `finish_allowance > 0`, the 2D Adaptive
  cleanup by `cleanup_strategy`. A geodesic 3D Rough declares 10 cuts (the note said 11).
- Ruling Q1: the geodesic `FirstPass(Slot)` is `MayOccur` with row `U-PerimeterSweep` and a
  marker in `cuts.rs`. New claim 7 of `the_exemption_list_only_shrinks_p29_3` holds the link;
  an injected bad row id goes red. An injected missing wall finish turns
  `every_op_declares_its_cuts_p29_3` red.
- Two cuts stay outside the list: the tab re-plunge of a Profile (`U-TabReplunge`, P29-8), and
  the capped-step sources other than the one each op declares (the module doc of `cuts.rs` and
  `compute/CLAUDE.md` say so).
- Rows closed: `U-SpringPass`, `U-ResidueCleanup`, `U-PocketZigzag`, `U-BoreWallFinish`,
  `U-RowConnector`, `U-CappedStep`. Pins from the array: ROWS 42, MAX_NEEDS_RULING 31,
  ROWS_WITH_A_FILE 41, PHYSICAL 8, RULED 3.

### 14.4 Findings outside the scope

- The constructor always writes `operation_kind: Some`: the `None` arms of the readers are dead.
  `chip.rs` formats the kind with `{:?}`, so a type change moves a warning text.
- The `NoLut` path of the calculator has no product reach; `OperationFamily::Face` has no op.
- The fields stay writable inside `crate::feeds`: `envelope.rs` writes the grid point
  (`axial_depth_mm`, `radial_width_mm`, `target_scallop_mm`) after the door.
- One capped-step kind per op is declared (2.2), but the geodesic planner emits more sources
  (`PlannedTurn`, `Capped2d`, `DepartureWindow`, `CappedSplit3d`, `StockRead3d`,
  `LevelContour`), and a ContourParallel 3D Rough may emit no `StockRead3d`. Not checked.
- The row `U-PerimeterSweep` keeps `Who::Undeclared`, but the cut is now declared.
- `U-CappedStep` closes on one declared source per op (2D Adaptive `DepartureWindow`, 3D Rough
  `StockRead3d`); whether each op emits that source was not checked against the kind fixtures.

### 14.5 Placement and what was not run

Section 14 is in `step_7b.patch` and reports both commits. Not run: any HEAD baseline; the 55
edited test binaries of 7a on the 7b tree (only the 7b sentries, the op golden, the lib, FM1 and
clippy ran after 7b); the edited tests marked `#[ignore]` (instruments); the whole viz crate (only
`--lib` and the five named viz tests); the cli tests (no cli code reads `FeedsInput` or `CutKind`).

## 15. Prediction amendment (2026-11-12, HEAD `3e63a0902`)

Written after the lead rulings of section 13, on the tree after 5b (option A), 5c, 7a and 7b. It
replaces the predictions of section 10 where they differ. `P29-7_predictions_fm1.csv` and
`P29-7_predictions.csv` are replaced (the old trap tables are folded into table `W`).

### 15.1 Method

A throwaway prototype edit of `feeds/envelope.rs` only (restored from a byte copy, `sha256sum -c`
OK, touched). An environment variable picks one of four modes in one build:

| Mode | What it does |
|---|---|
| off | HEAD behaviour, byte for byte |
| pick | only trap 1: the stated-width exact ceiling also takes the pick chip (`ExactRole::Pick` in the `Stated` phase), so `propose` reads `min(curve chip, pick chip)` as the card does |
| novt | pick, plus the force budget and the deflection read every declared cut with `Presence::Always` and a number |
| all | novt, plus the vendor depth table on every such cut (Q8) |

The rule of a declared cut at a body column `(ae, fz)`, where `fz` is the chip the limit reads
there (the operator chip, the grid chip of the column, or the pick chip of the exact solve):

- width: `Stated` gives `ae`; `Slot` gives D; `PassCeiling` gives `c(ae) D`; `CapMult` gives
  `min(m ae, D)`; `Scallop` and `None` give no number (skipped);
- chip: `OpFeed` gives `fz`; `Unlifted` gives `fz` over the body lift (`RadialThinning::lift` at
  `PowerCases::full_feed_width(ae)` on the engaged diameter at the depth per pass, the rule of
  `UnliftedFeed::of_slot_ring`); `PlannerCapped`, `EntryRule`, `DrillCycle` give no number;
- `Presence::MayOccur` gives no ceiling (ruling Q1);
- the body keeps its own width (`budget_width` for the force budget; the declared slot width for
  the deflection and the table when the body declares `Slot`).

Each edge builder takes the minimum over the cuts per width column: `rigidity_limit` (the grid
columns and the refinement), `deflection_limit`, `vendor_depth_limit` (its "binds" flag follows
the minimum), and the exact solvers `budget_depth_at`, `deflection_ceiling_at`,
`table_ceiling_at`. So `propose`, the removal-rate pick (`deepest_at` through
`exact_ap_ceiling_at`, and the coarse rank through the drawn edges) and the card placement all
read the minimum. Power keeps `PowerCases` (Q5). The deflection target stays 200 um (R5 is 7d).
The prototype has not the final shape of the driver (2.3); it proves the numbers.

### 15.2 Calibration and FM1

- Calibration: the mode "off" FM1 walk (matrix and simulation file) equals the committed fixtures
  byte for byte (both tests pass, no update variable, walk copies in the scratchpad).
- Mode "all": 10 matrix rows, 104 cells (`P29-7_predictions_fm1.csv`). Per column:
  `proposal_ap_mm` 10, `depth_per_pass_mm` 10, `r_axial_depth_mm` 10, `force_n` 10, `power_kw` 10,
  `proposal_ae_mm` 8, `stepover_mm` 8, `r_radial_width_mm` 8, `cap_mm` 8, `binding_limit` 8,
  `rpm` 4, `feed_mm_min` 4, `ramp_feed_mm_min` 4, `chip_regime` 2. The recipe columns move because
  the add door writes the envelope pick (K2b).
- Simulation file: 0 cells (byte-equal in both modes).
- Mode "novt" gives the same 104 cells: the vendor depth table moves no FM1 cell (15.5).

| Op | Tool, D, hardwood | Today (ap x ae, binding) | After (ap x ae, binding) |
|---|---|---|---|
| Pocket, Zigzag | EndMill 3.175 | 6.9850 x 2.3210, none | 4.9846 x 2.7100, CornerCoverage |
| Pocket, Zigzag | BullNose 3.175 | 6.9850 x 2.3210, none | 4.9846 x 2.7100, CornerCoverage |
| Pocket, Zigzag | BallNose 3.175 | 4.4723 x 2.7100, CornerCoverage | 4.4691 x 2.7100, CornerCoverage |
| Pocket, Zigzag | TaperedBallNose 3.175 | 4.2518 x 2.7075, none | 4.1827 x 2.7100, CornerCoverage |
| Pocket, Zigzag | TaperedBallNose 6 | 14.3398 x 1.0527, none | 3.0853 x 5.1213, CornerCoverage |

The removal-rate pick widens to the corner-coverage width once the slot caps the depth, as Q3
expected. At 5.1213 mm on a 6 mm tool the lift is 1, so the contour Pocket ring (unlifted) and
the Zigzag first row (op feed) give the same 3.0853 mm.

Earlier predictions that changed, and why:

- Pocket TaperedBallNose 6 mm: 4.0493 (the ring at the unlifted chip of today's narrow width)
  becomes 3.0853 at the wider pick, where the ring is not lifted.
- Adaptive3d TaperedBallNose 6 mm: 2.0957 becomes no move (3.0000 stays). The first probe read
  the grid chip of the nearest column (0.25 mm at 0.02 mm width); the envelope reads the chip at
  the stated width 0.432 mm (the column chip there is 0.147 mm), and the slot root at that chip
  is above 3.0 mm.
- The binding of the five removal-rate pairs is `CornerCoverage` (the pick sits on the width
  ceiling and on a depth ceiling; the first limit is the width).
- Every other first prediction stands: the depth of the 3.175 mm cells, and no move on Profile,
  Trace, Bore, Face, 2D Adaptive and the other 3D Adaptive cells.

### 15.3 The wanaka files (table `W`)

Read-only loads of both files; each op's proposal `ap x ae`, binding and placement in modes off,
pick, novt and all. Moves (every other op of both files: no move in any mode):

| File | idx | Today | Pick only | 7c (all) | Binding | Cut that binds (stated chip) | Move |
|---|---|---|---|---|---|---|---|
| operator | 1 | 2.0924 x 5.5 | 2.0924 | 2.0552 x 5.5 | Rigidity | PlannerCap, 5.9913 mm | -1.8 % |
| operator | 2 | 3.5932 x 1.5 | 3.5932 | 3.4391 x 1.5 | Rigidity | PlannerCap, 1.6380 mm | -4.3 % |
| operator | 14 | 2.4547 x 3.6 | 2.4547 | 2.4021 x 3.6 | Rigidity | PlannerCap, 3.9395 mm | -2.1 % |
| operator | 15 | 3.6983 x 1.382 | 3.6983 | 3.5172 x 1.382 | Rigidity | PlannerCap, 1.5104 mm | -4.9 % |
| work | 1 | 3.9543 x 1.2 | 3.9542 | 3.7498 x 1.2 | Rigidity | PlannerCap, 1.3132 mm | -5.2 % |
| work | 2 | 3.6696 x 1.5 | 3.6695 | 3.5118 x 1.5 | Rigidity | PlannerCap, 1.6380 mm | -4.3 % |
| work | 15 | 3.7837 x 1.382 | 3.7836 | 3.5988 x 1.382 | Rigidity | PlannerCap, 1.5104 mm | -4.9 % |
| work | 14 | 3.0 x 1.2 (stated) | 3.0 | 3.0 x 1.2 | none | (3.7499 > 3.0) | 0 |

The option A column of section 10.3 holds: every value equals the run within 0.0001 mm (the work
file's pick chip is about 1e-5 mm above its stated chip). No width moves. The operator file moves
only on the 3D roughs idx 1, 2, 14, 15; the largest move on any wanaka op is 5.2 %. Runtime
placement at the proposal: operator idx 2 -0.184 -> -0.219, work idx 1 -0.282 -> -0.319, work
idx 2 -0.167 -> -0.203 (the section 2.6 values). None of the lead's stop conditions is met.

### 15.4 rivmap100 (table `R`)

`planning/fixtures/rivmap100/rivmap100_live_0925.toml` (the copy most roughing measurements
use), 3D Rough (toolpath 1: 6 mm flat, ContourParallel, white oak, stated 8.0 x 1.2 mm, stated
chip 0.0667 mm):

| Mode | Proposal | Binding |
|---|---|---|
| today | 6.0000 x 1.2 | VendorAxial (placement at the proposal: Outside Rigidity -0.558) |
| pick only | 3.8518 x 1.2 | Rigidity |
| 7c (all) | 2.5851 x 1.2 (-56.9 %) | Rigidity: the ContourParallel perimeter sweep (`FirstPass(Slot)`, `Always` under ContourParallel) at the pick chip |

`planning/fixtures/rivmap100/rivmap100.toml` (stated 2.0 x 1.2): no move (the slot root at the
stated chip is 4.3358 mm). The test fixture
`crates/rs_cam_core/tests/fixtures/rivmap100_rough_2026-11-04_7effc804/rivmap100.toml` (the same
3D Rough), at the points its viz tests set:

| Point (ap, ae) | Today | Pick only | 7c (all) |
|---|---|---|---|
| as saved (2.0, 1.2) | 2.0000, none | 2.0000 | 2.0000 (placement margin 0.481 -> 0.226) |
| (4.217, 2.303), the review point | 4.2170, none (Outside Rigidity -0.289) | 3.2723 | 2.9502, Rigidity |
| (8.0, 2.303) | 4.9402, Rigidity | 3.2723 | 2.9502, Rigidity |

The pick-only value 3.2723 is the hand-over's 3.27 mm. The 7c value adds the sweep slot.

The rivmap live copy moves by more than 10 %; it is not a wanaka file, so the stop rule does not
name it, but it is the roughing benchmark and the lead should see it: most of the move is trap 1
(6.0 -> 3.85 mm), and the rest is the ContourParallel sweep (3.85 -> 2.59 mm).

Tests that read that 3D Rough (`rg -l rivmap100` with `proposal`, `safe_envelope` or a depth),
each at risk of a pinned number or a pinned binding: viz
`the_envelope_plot_shows_only_what_limits_the_cut_p5c` (points None and (4.217, 2.303)),
`every_limit_is_drawn_and_named_p5d` (points None and (8.0, 2.303); line 466 pins
`scene.binding == VendorAxial` at the saved point), `p17_visual_snapshots` (the envelope images),
`the_chart_kit_draws_the_envelope_p17` and `the_sim_chart_seeks_on_a_dot_p17` (the proposal
point), `the_step_records_reach_every_surface_p13b` and core
`the_step_records_are_stored_and_bounded_p13b` (they apply the proposal),
`the_nomogram_readout_abstains_g_hoverbound`. Not run.

### 15.5 The vendor depth table alone (Q8)

Modes "novt" and "all" give the same proposal on every FM1 cell, every wanaka op and both rivmap
files. The table edge moves in on each 3D rough (the slot share `fz sin(psi) / band max` is the
largest at a slot), but it binds no proposal: on these cells the force budget is lower. So the
vendor depth table moves 0 FM1 cells and 0 ops by itself; it moves drawn edges and the `Check`
rows only.

### 15.6 Envelope time

FM1 Adaptive3d EndMill 6 mm hardwood, `SafeEnvelope::for_cut`, mean of 10 in a test build after
one warm-up: off 173 and 178 ms; pick 180 ms; novt 197 and 198 ms; all 156 and 161 ms. The runs
shared the machine with other jobs, so the spread (156 to 198 ms) is noise of the same size as
any effect. No growth beyond about 15 % is visible; the implementer measures it again on the
final driver.

### 15.7 What is still reasoned

- The final driver shape (2.3) gives the same numbers as the prototype: reasoned (the same
  minimum over the same cuts and chips).
- The test list of 15.4 and of section 5: not run.
- 7d (R5, the finish deflection edge): unchanged from section 10 (0 FM1 cells, 0 wanaka ops).
- The binding cut named in table `W` is the cut with the lowest force-budget root at the stated
  chip; the run proves the proposal and the binding limit, not the cut name at the pick chip.

## 16. Result of 7c and 7d

Written 2026-11-12 by the implementer, on HEAD `f27ff3444`. Two patches, in order:
`step_7c.patch`, `step_7d.patch`. The first report of 7c stopped on seven reds; the lead ruled on
each (2026-11-12), and 16.5 gives what was done.

### 16.1 Commit 7c: the shape

- `limits::scope` is the one table of both sides. `Scope::NotASampleLimit` became
  `Scope::NotACutLimit(PlanReason)` (`DepthOnly`, `PlanLevel`, `MachineOrRpm`, `DrillGate`).
  `VendorDepthTable` and `RubbingFloor` are limits of each cut (ruling Q8): judged on the body,
  entry, cleanup and link groups; the drill group is not judged, with the row `P28-20`, which now
  also exempts these two limits (its count and the pins do not change). `ScallopDepth` (not in the
  table of 2.1) answers `PlanLevel`.
- New `feeds/planned.rs`: `DeclaredCuts` (the cut list of an op on a tool, built once per
  envelope), `PlannedCut`, `BodyColumn`, the trait `PlannedLimit` (`judge` over one cut, with its
  own `Env` and `Answer`), the one driver `judge_declared_cuts`, the record `ByCut` (a private fixed
  array, one constructor `from_fn` that calls its closure once per declared cut),
  `ByCut::lowest_always` (the lowest value over the judged `Always` cuts; a tie keeps the first cut,
  the body). The planned limits: `ForceBudgetPlanned`, `DeflectionPlanned`,
  `VendorDepthTablePlanned`, `SpindlePowerPlanned`, `ChipMaxPlanned`, `ChipMinPlanned`,
  `RubbingFloorPlanned`.
- The one exhaustive door (lead ruling on the hand list): `planned_limit_of(LimitId) -> PlaneRole`,
  one match with no wildcard: `DepthCeiling(CeilingEdge)` (`VendorDepthTable`, `Power`,
  `Deflection`, `ForceBudget`, declared in the draw order of the boundary), `ValueCheck(ValueCheck)`
  (`ChipMax`, `ChipMin`, `RubbingFloor`) or `NotACutLimit(PlanReason)`. The envelope reads every
  limit through one loop over `LimitId::ALL` (`envelope::plane_roles`); the drawn edges, the
  pick-chip edges, the exact solves, the exact ceilings at the stated and proposal widths, the
  binding cuts and the per-cut checks each dispatch through one match on `CeilingEdge` or
  `ValueCheck` with no wildcard. FM1 and the probe are byte-equal before and after this change (the
  boundary keeps its order). `tool_load::groups::gate_of(LimitId)` is a match with no wildcard;
  `GATE_LIMITS` is gone (`groups::Gate` lists the gates).
- The width and chip rules are those of the prototype (`am/envelope_proto.rs`), which made the
  predictions: the body reads the column width, or D when it declares the slot width; another cut
  reads `Stated` as the column, `Slot` as D, `PassCeiling` as `c(ae) D`, `CapMult` as
  `min(m ae, D)`; a cut that states no depth of the body's depth axis (the Bore `WallFinish`) has
  no number (`NoStatedDepth`); the unlifted chip is the body chip over the lift at the full-feed
  width, on the engaged diameter at the stated depth per pass (`UnliftedFeed::of_slot_ring`).
- `PowerCases` is deleted. The power fold is `planned::worst_power` (today's rule, ruling Q5: each
  cut at the op feed, the slot ring too; a tie goes to the more specific case,
  `PowerCase::tie_rank`). The lift width is `CutList::full_feed_width`. FM1 was byte-equal after
  this stage alone.
- The envelope: the force budget, the deflection and the table read the driver (`budget_solve`,
  `deflection_solve`, `table_solve`); `budget_width` is gone. `BUDGET_EDGE_SPLIT_LEVELS` rose from
  4 to 12 passes (they stop when no segment splits; red 6 of 16.5).
- The chips (trap 1): the force budget at the stated and the proposal width reads the curve chip
  and the pick chip (`max(stated, suggested)`), the lower ceiling holds, so `propose` reads the chip
  that the card judges at. The deflection and the vendor table in `propose` read the curve chip
  (the operator's chip, else the suggested chip at the shallowest depth); in the removal-rate pick
  they read the pick chip (`exact_ap_ceiling_at`). The power edge reads the operator's chip, else
  the full suggested chip of the grid. The chip rows of the checks read the commanded chip of the
  proposal.
- Trap 2: `MOVE_BUDGET_CONFLICT`, a note when the force budget sets the depth and the move budget
  is not met there; no number moves.
- `Limit::binding_cut`: the cut that sets each depth ceiling at the proposal width (the lowest
  ceiling over the `Always` cuts; the power: the cut of the largest power). An op with no proposed
  depth (Project Curve) names no binding cut. `SafeEnvelope::cut_checks`: one `Check` per declared
  cut and planned limit at the proposal (the depth ceilings against the proposal depth, the power of
  each cut against the rated power, the chip band and the rubbing floor against the planned peak
  chip). A `MayOccur` cut gives its row and a note, never a ceiling (ruling Q1).
- The ⚡ hover names the limit and the cut ("The proposal stands on the Rigidity DOC, slot ring").
- The chart label kit: a flat line whose two end places are taken offers places ON the line
  (`labels::on_places`), before the places beside it (red 7).
- Markers gone: `EXEMPT(M12)` in `cuts.rs` and `envelope.rs`; `EXEMPT(P28-15, M12)` became
  `EXEMPT(P28-15)`. Row `M12` closed (`CLOSED` gains `("M12", "P29-7")`). Text only: `M4`
  `closes_in` "P29-10 (one depth predicate, P29-6 F6)", `F-OffsetRing` `closes_in` "P29-9".
  Pins from the array: ROWS 41, MAX_NEEDS_RULING 30, ROWS_WITH_A_FILE 40.
- New sentry `every_limit_judges_every_declared_cut_p29_7`, four claims. Claim 1: one cell per
  cut and planned limit, the scope decides it; a limit that the scope judges on a group has a
  planned role that names it and a gate OR a named exemption row for the side after the
  simulation; a `NotACutLimit` role states the reason of the scope on every group; `gate_of`
  equals the gates whose `LIMITS` name the limit. Claim 2: each drawn force-budget and deflection
  column is the lowest over the `Always` cuts (the test takes the minimum itself), and every
  `CeilingEdge` draws an edge on a config (the slender end mill of the p11a sentry joined the
  configs: no FM1 tool has a deflection root below the flute). Claim 3: the proposal is inside
  the ceiling of every `Always` cut. Claim 4: the slot ring binds a contour Pocket, the planner cap
  a geodesic rough, and each no-number reason occurs. Each claim went red on its injected defect
  and green after the restore (run on the first form of the claims).

### 16.2 The predictions

- FM1: exactly the 104 cells of `P29-7_predictions_fm1.csv` in 10 matrix rows, no other cell. Per
  column: `proposal_ap_mm` 10, `depth_per_pass_mm` 10, `r_axial_depth_mm` 10, `force_n` 10,
  `power_kw` 10, `proposal_ae_mm` 8, `stepover_mm` 8, `r_radial_width_mm` 8, `cap_mm` 8,
  `binding_limit` 8, `rpm` 4, `feed_mm_min` 4, `ramp_feed_mm_min` 4, `chip_regime` 2. The
  simulation file is byte-equal. The matrix file is byte-equal to the prototype walk
  (`am/all`). Re-blessed on that exact match; byte-equal again after the red 6 fix and after the
  role door.
- Tables `W` and `R`: all 47 rows equal the `proposal_7c` column to 0.0001 mm and the
  `binding_7c` column (also after the red 6 fix and the role door). The binding cut: the planner
  cap on every wanaka 3D rough (as table `W` names it at the stated chip); the first slot on the
  rivmap live copy; the planner cap at the two test-fixture points (4.217, 2.303) and
  (8.0, 2.303), where 15.4 names the sweep slot. There the ContourParallel planner cap is
  `min(1.6 x 2.303, 6) = 3.685` mm (`trochoid_cap_mult` 1.6, read from the fixture). The edge keeps
  the first cut on a tie (strict `<`), so the first-slot root is not below the planner-cap root at
  that chip: equal on the plateau of the peak force, or higher. The two roots were not printed
  apart.
- Table `V`: with the table limited to the body cut (the HEAD rule), the FM1 walk, the simulation
  file and every proposal and binding of the probe are the same as 7c.
- The work and operator files gain the trap 2 note on operator idx 2, work idx 1 and 2, and the
  `MayOccur` note on each geodesic 3D rough.

### 16.3 Envelope time

FM1 Adaptive3d EndMill 6 mm hardwood, `SafeEnvelope::for_cut`, test build, one warm-up, 9 runs
each: HEAD median 176.2 ms; 7c median 168.3 ms (two runs of 9: 168.31, 168.37), before the red 6
fix and the role door. No growth. The lib timing test (one Pocket plane envelope with a model box)
measured 180 ms and 190 ms alone after 7c, under its 300 ms budget.

### 16.4 The wire

`get_safe_envelope`: `envelope.binding_cut`, `binding_cut` on each `envelope.boundary` row,
`envelope.cut_checks` (key name `cut_checks`: the reply has a top-level `checks` since P29-2b). The
feature token `envelope_declared_cuts` records the capability. The tool description names the keys.

### 16.5 The reds and the rulings

1. `the_deflection_edge_is_a_solved_depth_p11a`: re-claimed. At each curve point the body alone
   is not deeper than its own root (the op predictor), and the cut that binds the column (the
   lowest root over the `Always` cuts, at its own width and chip) deflects at most the target at
   the edge and more one convergence step deeper (the forward model
   `tip_deflection_from_engagement`). The slot ring binds a column; at the proposal width the
   binding cut equals `Limit::binding_cut`.
2. `the_envelope_keeps_each_limit_k2b` vendor axial: re-claimed. The cell is a geodesic 3D Rough
   (no full slot that the plan always cuts), where `VendorAxial` still binds the stated 9 mm. New
   assertion: the same cell as a ContourParallel rough binds Rigidity on the first slot (3.85 mm;
   the old value 6.0 mm VendorAxial).
3. `the_roughing_depth_meets_the_force_budget_p13c` Pocket edge: re-claimed. The body is at most
   the budget at the edge; the cut that binds the column is at most the budget at the edge and
   over it 0.01 mm deeper; the slot ring binds a column.
4. `g_s3depth::the_suggest_clamp_still_fires_at_the_helper_s_own_cap`: re-claimed with no wider
   tolerance. For each declared cut with a number (the body and the slot ring of the contour
   Pocket) the planned ceiling and `rigidity_depth_cap` of a sample of that width and chip are one
   number, bit for bit; the envelope ceiling is the lowest of them, bit for bit.
5. `the_envelope_finding_reads_the_gate_feed_p20i` (two tests): re-claimed (lead ruling). On the
   test's contour Pocket the slot power case (op feed, ruling Q5) held the drawn Power edge flat at
   2.598 mm at every width at the old case value 0.25 kW, and the force budget of the declared cuts
   is under it at every width (1.471 to 2.194 mm), so no width had Power as the lowest ceiling. The
   test machine's rated power is now DERIVED in the test (`rated_kw`): one half
   (`RATED_SHARE`) of the power of the stated cut at its force-budget depth at the full width (the
   lowest force-budget depth over the widths), on the same machine. Power is then the lowest
   ceiling by construction; the three claims stand unchanged and pass.
6. The drawn force-budget edge 3 % over the solved minimum: fixed. The split passes stopped after
   4 levels while segments still stood over the minimum (the kink where the body and the slot
   ring cross, and the steep stretch of the ring at a narrow width); they now run until no segment
   splits, up to 12 (the cap of the table edge). Worst after: 0.05 % (ae 0.0752 mm, 5.7062 drawn,
   5.7033 solved). FM1 byte-equal to the blessed 7c fixture; the probe equal.
7. p5d label 7 px off its line: the placement follows its own line now (`on_places`). Both end
   places of the vendor DOC line are taken on this fixture: at the right end the vendor DOC line
   (6 mm) and the Rigidity line (4.34 mm) are about 10 px apart and the two chips do not fit; the
   left end of the Rigidity line is crossed by the move-budget curve, and the left end of the
   vendor DOC line is held by the move-budget label. No order of the labels puts both at an end (a
   layout that preferred more labels at their ends found no gain and was removed). The label now
   touches its line; the test's end clause for the vendor DOC takes the rule of the move-budget
   label (at an end, or on its line inside its span). This is a re-claim of a clause; the lead can
   reverse it. The fixture change "the view top is the hard top" stands by geometry: the depth
   table edge is flat at 17.40 mm (2.9 x D), under the hard top of 20 mm, and 1.3 x 17.40 = 22.6
   mm is over it, so the range reaches the hard top.

`the_simulation_is_placed_not_fed_back_k7::a_tapered_ball_wider_than_its_tip_is_judged_at_its_engaged_diameter`,
red on HEAD (`non-vacuity: EnvelopePlacement { placed: 0, .. }`): commit `6409d36c6` (P29-6 6d)
added to `compute::simulate::envelope_placement` the filter `judged_group(s, ..) !=
Some(CutGroup::Body) => continue`; `judged_group` is `None` when the sample removes no material
(`removed_volume_est_mm3 > 0.0` is false), and the test's sample takes
`SimulationCutSample::test_fixture()` with `removed_volume_est_mm3: 0.0`. 6d did not edit the test
(its last change is `5c6ae274b`, 6a). The 7b gates did not run it (14.5 lists them). Fixed in
its own patch `step_k7fix.patch` (lead ruling): the sample states `removed_volume_est_mm3: 1.0`, so
it is a body sample; the claim and the non-vacuity check stand. It applies before or after 7c and
7d (checked in five orders on a scratch index). The other tests that build
`SimulationCutSample::test_fixture()` samples for a group read (`envelope_placement`,
`judged_group`, `populations`, `judge_by_group`, `evaluate_toolpath`, `headline`): 16 files, among
them ones no gate list runs (`histogram_population_is_the_gate_population_g_cuthist`,
`ceiling_advisory_and_clamp_record_a7`, `chipload_report_wording_t12_t15`,
`heatmap_two_arc_divergence_a1`, `feed_explanation_record_t1`, `a_tapered_row_is_read_at_the_tip_a1`):
all 16 pass on the final tree.

### 16.6 Re-claims of sections 5 and 15.4

- `every_limit_is_drawn_and_named_p5d` line 466: `scene.binding == Rigidity` (predicted); the
  body and muted lines swap; the drawn kinds gain `VendorDepthTable`; the view top is the hard top
  (by geometry, 16.5 item 7); the end clause of the vendor DOC label (16.5 item 7).
- `the_envelope_plot_shows_only_what_limits_the_cut_p5c`: the fixture point binds `Rigidity` (the
  margin 0.481 -> 0.226 of 15.4); the drawn kinds gain `VendorDepthTable`; the amber candidate
  crosses `VendorAxial` and `Rigidity`.
- `the_sim_chart_seeks_on_a_dot_p17`: the off-scale rows gain the depth table.
- `every_op_declares_its_cuts_p29_3` claim 1 reads `worst_power` and `power_case_width`;
  `the_exemption_list_only_shrinks_p29_3` reads `Gate::ALL` and `NotACutLimit`; the doc of
  `the_power_curve_picks_the_arm_at_the_feed_cap_q1` names `PowerCase::Slot`.

### 16.7 Where the final shape differs from section 2

- The trait has an associated `Env` and `Answer` per limit; the driver is generic.
- Grooves (accepted by the lead): on a groove body (VCarve, Inlay, InlayPlug) every planned limit
  reads the declared slot width D; the P29-5c ruling Q1 stands for the radial axis and the speeds
  grid only. The P29-5c table `S` probe (60 degree V-bit 6.35 and 12.7 mm, softwood, hardwood,
  MDF; VCarve, Inlay, InlayPlug at 0.5 D, 1 D, 2 D; Chamfer and Trace) against
  `P29-5c_predictions.csv`: 60 rows, 11 differ (the MDF rows are refused in both).
  - Inlay and InlayPlug, 6.35 mm, softwood, stated 12.7 mm: the proposal 9.6183 -> 6.3500 mm,
    the binding Rigidity -> VendorDepthTable (the table at 1 x D: the slot share
    `fz sin(psi) / band max` with `sin psi = 1`), the placement Outside Rigidity -0.320 ->
    Outside VendorDepthTable -1.000.
  - The same two ops at 3.175 and 6.35 mm: the placement Inside 0.670 -> 0.500 and 0.340 ->
    0.000 (the table ceiling 6.35 mm).
  - Chamfer: the placement 0.934 -> 0.843 (6.35 mm, softwood and hardwood, stated 3.175), 0.967 ->
    0.921 (12.7 mm, softwood and hardwood) and 0.967 -> 0.953 (12.7 mm, MDF). The cause is not
    separated.
  - VCarve and Trace: no difference.
- `gate_of` is empty for `VendorDepthTable` and `RubbingFloor` (accepted with the row `P28-20`);
  the sentry says exactly: a gate OR a named exemption row.

### 16.8 What a future author must do by hand

- A new `LimitId` does not compile until `limits::scope`, `planned_limit_of` and `gate_of` state
  it. A new role (`CeilingEdge` or `ValueCheck` variant) does not compile until each envelope
  match states it (the edge builder, the pick edge, the exact solve, the exact ceilings, the rows,
  the binding rule, the unit). By hand: the `PlannedLimit` impl itself and its solver; a new
  `Gate` variant and a field of `GroupVerdicts`.
- The `MayOccur` note and the trap 2 note are text on `notes`; no surface test pins them.

### 16.9 Findings outside the scope

- Chip rows of `cut_checks` (left for P29-10, display): 6 mm 2F end mill, hardwood, add door, the
  default machine. The first slot of a zigzag Pocket at a 1.0 mm stepover, fed at the op feed under
  the lifted body, gives ChipMax exceeds: 0.1704 against 0.127 mm. The body of a geodesic 3D Rough
  (stepover 1.2 mm, stated depth 6 mm) gives ChipMin exceeds: 0.1229 against a point band of
  0.127 mm (the feed is sized at the planner-cap width). No card shows these rows.
- The `MayOccur` note is the first note of a geodesic 3D Rough, and the card header prints the
  first note in full when it has no short face (`force_budget::note_face`). Not a small change in
  the notes path (the header shows the first note); logged for P29-10.
- No repo machine and op has Power as the lowest depth ceiling on a slot-declaring rough: the force
  budget of the declared cuts sits under the power ceiling at every width (16.5 item 5: 2.598 mm
  against 1.471 to 2.194 mm on the p20i contour Pocket at 0.25 kW; no repo machine is rated under
  0.71 kW).

### 16.10 The gates of 7c (the final 7c tree)

- fmt check: clean. Full clippy (`--workspace --all-targets`, heavy-tests, research,
  test-support, `-D warnings`): clean.
- `cargo test -p rs_cam_core -q --lib`: 2796 passed, 1 failed
  (`the_envelope_meets_its_time_budget` under the loaded run; alone it passes, 198 ms).
- FM1: 2 passed (the matrix and the simulation file equal the 7c fixture).
- The named sentries of section 3, the new sentry, `g_s3depth`, `p20i`,
  `the_step_records_are_stored_and_bounded_p13b`: 25 binaries pass; `p20i` failed its two tests
  until the derived rated power (16.5 item 5), and passes after it (run on the final tree).
- The op golden on the final tree: its diff equals the known 46 rows. The re-injection of one
  defect into each reworded claim (16.5 items 1 to 4, sentry claims 1 and 2) turned each red; the
  sources were restored byte for byte.
- Viz `--lib`: 706 passed. Viz tests p5c, p5d, `mcp_wire_surface_pin`, `p17_visual_snapshots`
  (3 ignored), the chart kit, the sim chart, the step records, the hover bound, the chart lines:
  all pass.
- `rs_cam_mcp`: 39 passed. `rs_cam_cli`: 13 binaries, all pass.
- The op golden (run on the first 7c tree): its diff equals the known 46 rows.

### 16.11 Not run

`wanaka_suggest_integration`; the core full gate; the whole viz crate; the instruments marked
`#[ignore]` (`p17_visual_snapshots` runs 3 ignored tests).
