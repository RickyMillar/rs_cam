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
