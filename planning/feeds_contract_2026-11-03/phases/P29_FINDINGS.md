# P29 findings: one cut object, every limit and metric a function of it

Written by the lead on 2026-11-11. Investigation only: no product code, no cargo run. HEAD
`484542abe`. The two surveys hold the evidence with `file:line`:

- `P29_parts/A_cuts_and_limits.md` (what exists, the cut list of 25 ops, 39 limits, 12 gaps
  that P28 missed).
- `P29_parts/B_metrics_and_display.md` (metrics beyond limits, 19 display surfaces, 28 verdict
  and finding types).

This note is the lead's reading of them: the design, what moves, the ordered change list, and
the rulings the operator must give. Nothing here is decided until the operator approves it.

## 1. The answer in short

1. The fix is feasible, and most of the parts exist. It is a join of existing types, not a
   new system beside them.
2. The cause is wider than P28 found. An op declares ONE cut (the body pass). Every other
   cut it makes (slot ring, entry, cleanup, capped step, spring pass) reaches no limit, or
   one limit by a special channel.
3. No single function gives the loads of a cut. One engagement type exists; about 10
   wrappers assemble force, power, deflection and chip by hand, each with its own width,
   feed and scope rule.
4. The display has the same shape of problem: 6 record shapes for "a value against a
   limit", 3 lists of limit names, 5 basis vocabularies, 4 forms of "which limit binds".
5. The first three steps change no number. The first behaviour step is the force budget on
   the Adaptive family. It needs two rulings (section 6).

## 2. What exists, and its gap

| Part | State | Gap |
|---|---|---|
| `Engagement` (closed type, 24 constructor sites) | Geometry of a side cut | No feed, rpm, tool, material, cut kind or source. No form for a ramp, a helix, or a plunge of a tool that is not flat. |
| `CutSpec` (one exhaustive match, 25 arms, no default) | The body pass as stated | One cut, not a list. 17 of 25 ops give the calculator no depth, no width, or neither. |
| `FirstPass`, `PlannerWidth`, `PowerCases` | Extra widths | Read by power only (one reader each). Force and deflection do not read them. |
| `EntryContract`, `EntryRecord` | Every op states its entry kind; one record per entry after generation | No engagement. No limit reads them. |
| `FeedCapSource`, cleanup marker, slot-first ring | The planners name their extra cuts | They reach the feed stage or the trace, never a limit. |
| `depth_at_budget`, `cutter_axial_constraints`, `PowerTerms`, `edge_force` | Pure functions of the cut | Each sits behind a scope read on (family, role). |
| `CriterionStatus` (post-sim) | Kind, value, bound, bound source, state, population | No margin, no binding. |
| `Limit` + `JudgedValue` + `Placement` (pre-sim) | Limit, basis, value, margin, binding | No state, no population. `placement_of` and `placement_of_measured` already place a planned point and a sample by the same limits. |
| Compile-time net | Registry row, `CutRule`, `EntryContract` all refuse a new op without a statement | The statement is one cut and a kind of entry. |

## 3. What the surveys add to P28

P28's 34 rows hold at HEAD (none wrong). New, by weight:

1. **The chip gate has a feed filter** (M1). It judges only samples at 95 % or more of the
   steady feed. A slot ring at its lower feed (P23d) or a capped step can fall out of the
   gate. The share on a real op is not measured.
2. **The cap and the check see different moves** (M2). The modulator caps ramp and helix
   entries by power, deflection and force budget. No gate then judges those moves.
3. **Force and deflection never see the slot** (M12). A Pocket slot ring is sized for power
   and feed only.
4. **The depth gate judges one sample** (M5): the deepest. A shallower, wider sample is not
   judged against the force budget.
5. **One sample builds two engagements** with two depths (M4): one for power and depth, one
   for deflection and force.
6. **Filing errors** beyond the six slot ops: Profile is Roughing and emits finish moves; a
   region Face and the 3D perimeter sweep are slots filed `NotSlot`; Bore states a slot and
   the calculator reads the family default depth.
7. **Entries have three rule sets.** Ramp angle 3, 10 and 12 degrees; helix pitch 1 and
   2 mm; plunge feed by two formulas. The dressup plunge does not read the op plunge rate
   that Suggest computes. None has a source. Suggest sees only the 3D Adaptive entry.
8. **A drill is outside the sample model**: its own samples, its own outcome type, no
   body-zone walk in the simulation (not verified against the mesh holder check).
9. **Display**: the iso-force field draws only for ops under the force budget rule, so an
   adaptive op shows no contours. Viz and MCP assemble several values themselves from core
   laws. No surface shows the cusp height of a stated stepover.

## 4. The design

Five types. Each joins or extends a type that exists.

### 4.1 `Cut`: the one object

A `Cut` is one engagement of the tool with the stock, with all a limit needs:

- `kind`: `Body`, `FirstPass` (slot or first row), `SpringPass`, `Entry(Plunge | Ramp | Helix)`,
  `Cleanup`, `CappedStep(FeedCapSource)`, `Link`, `Drill`.
- `engagement`: the existing `Engagement`, extended with an inclined form (ramp, helix: a slot
  at a slope) and the axial form for every tip. A form with no model returns "no number",
  never a silent skip.
- `point`: feed, rpm, flute count (the operating point; today loose arguments).
- `source`: `Planned` (from the op) or `Measured(sample)` (from the simulation).
- Geometry facts that replace the labels: `thins_chip`, `offset_ring`, `leave_after_mm`.
  These replace the reads of (family, role).

`CutSpec` stays as the operator's statement of the body pass. `Cut` is built from it.

### 4.2 `cuts()`: the declaration

`OperationConfig::cuts(&self, ctx) -> CutList`: one exhaustive match, no default, in the place
of today's `cut_spec` + `FirstPass` + `planner_width` + the entry `if let`. A new op does not
compile without it. A sentry asserts, per op fixture, that every `kind` the emitted moves
carry is in the declared list (the planner cannot emit a cut that the op did not declare).

After the simulation the same type comes from each sample: `Cut::of_sample`. The sample takes
its `kind` from the move intent and the planner markers, in place of span ancestry.

### 4.3 `CutLoad`: one function

`CutLoad::of(&Cut, tool, material) -> {chip, force (mean, peak), power, deflection}`. The
four model functions stay; the 10 wrappers become callers. The two depths of M4 become two
named fields of the sample cut with a stated reason, or one depth (a probe decides).

### 4.4 `Check`: one record

`Check { limit: LimitId, value, bound, margin, state, basis, binding, population }`.

- `LimitId` is one list in the place of `LimitKind` (14), `CriterionKind` (8),
  `BindingConstraint` (9) and `RpmLimit`.
- `basis` is the existing `Basis`, extended with what `BoundSource` holds (`gates_export`,
  `setting`). A measured value states `Measured`.
- `state`: `Within`, `Exceeds`, `NoNumber(reason)`, `NotApplicable(reason)`.
  `NotApplicable` takes a closed list of geometry reasons only (`NoRadialEngagement`,
  `NotAnOffsetRing`, `NoVerticalFeed`). It has no member for "deferred" or "kept".
- `CriterionStatus` and `Placement` become projections of it, then go.

### 4.5 `limits::check_all`: one door

`check_all(&Cut, tool, material, machine) -> Vec<Check>`: every limit, for every cut. No
argument holds the op type, so no limit can branch on it. Readers:

- Suggest and the envelope: the largest cut where every declared cut passes.
- The gates: every sample that removes material. The result groups by `kind` (body, entry,
  cleanup), so an entry reading is visible and separate, not hidden and not mixed.
- The modulator: the same checks solved for the feed.
- Every surface: cards, charts, lineage, MCP, CLI read `Check`. A planned cut and a sample
  plot as the same kind of point.

What `Check` does not replace: histograms, time series, limit curves and the force field.
They are built FROM per-sample checks or from `check_all` over a grid. The per-sample path
must stay cheap (tens of thousands of samples): the sample cut is a small `Copy` value.

## 5. What moves when every cut reads every limit

Estimates by reading. P29-0 measures them before any behaviour step.

| Change | Wanaka, operator file (18 mm roughs) | Wanaka, 6 mm working file | FM1 | Weight |
|---|---|---|---|---|
| Force budget on Adaptive | idx 1 at 5.5 x 10 mm is about 4 x over; idx 14 over (P26) | idx 1: depth 6.0 to about 3.85 mm at 1.2 mm width; idx 14 (3 mm) no change; idx 2 not measured | up to 48 cells (P26) | Largest |
| Slot and groove ops read the budget and pre-sim deflection | idx 5, 9, 10, 13 (small and V tools in a full slot): size not measured | the same | Trace, VCarve, Inlay, Chamfer, Project Curve rows | Unknown; probe |
| Gates judge entries and cleanup | idx 2 holds an entry-load critical today; idx 1 helix entries; cleanup after every level of idx 1, 2, 14 | the same ops | every 3D rough cell with a simulation | High: new findings, no new number in Suggest |
| Chip gate without the 95 % filter | slot rings of idx 6, 7, 8, 11; capped steps of the 3D roughs | the same | Pocket cells | Risk: slow moves read as "under the band" |
| Force and deflection read the slot ring | Pockets idx 6, 7, 8, 11: the depth can fall | the same | Pocket, Profile, Zigzag cells | Medium |
| One deflection limit; finish deflection pre-sim | idx 16 and the slot ops get a pre-sim value | the same | finish cells | Low on wanaka |
| Depth gate on every sample | more "exceeds" rows on wide shallow samples | the same | simulation cells | Low to medium |

Limits whose own basis is weak. Making them universal spreads the weakness:

- **The adaptive width ceiling**: no printed source (P26). Without it and without the corner
  rule, the removal-rate rule picks a slot. It needs a ruling before P29-4.
- **The force budget (35.3 N)** is the largest printed hobby slot force. It is a machine
  class figure, not a tool figure. On an adaptive cut it halves the depth of the 6 mm rough.
  The operator must accept that result or question the budget.
- **Entry cuts have no force model** for a ramp, a helix or a plunge of a ball or V tool. A
  first model (a slot at a slope) is geometry, but it is new and unchecked against a source.
- **The entry rule values** (3, 10, 12 degrees; pitch; plunge feed) have no source.
- **The deflection limit**: 200 um, 50 um, or the stock to leave: three values today.

## 6. The change list

Order: structure first with no number moved, then behaviour by safety of the operator's cut.
Each behaviour step states predictions before the run, has one sentry, and gets a wanaka
read and an FM1 measurement. Rulings marked R are the operator's.

| Step | What | Moves a number? | Sentry | Ruling |
|---|---|---|---|---|
| P29-0 | Measure: the share of samples the 95 % filter drops; the force of the wanaka slot and groove ops; idx 2; the loads of entry and cleanup samples on wanaka. Temporary probes. | No | - | - |
| P29-1 | `CutLoad`: one load function; the 10 wrappers call it. M4 named. | No (byte-identical) | one cut gives one force, power, deflection and chip on every door | - |
| P29-2 | `Check`, `LimitId`, one basis. `CriterionStatus` and `Placement` project to it. MCP gains the record beside the old fields. | No | every limit of every surface is a `LimitId` | - |
| P29-3 | `Cut` and `cuts()`: required method. Filled with today's cuts and today's scope. Every exemption of P28 and M1 to M12 becomes a visible entry in one list, marked "needs a ruling" unless physical. | No | the exemption list only shrinks; emitted kinds are declared kinds | - |
| P29-4 | The Adaptive family reads the force budget (pre-sim depth, post-sim gate, modulator, force card, chart contours). | Yes: idx 1, 2, 14, 15; up to 48 FM1 cells | an adaptive rough meets the budget | R1, R2 |
| P29-5 | Limits read the cut, not the label: slot and groove ops (Trace, VCarve, Inlay, Chamfer, Project Curve) read the budget and the pre-sim deflection. Filing flags fixed (Profile intent, Face region, sweep, Bore depth). | Yes | a full slot reads the same limits under every op | R3 |
| P29-6 | The sample holds its cut kind. The gates judge entries, cleanup and capped steps, grouped by kind. The 95 % feed filter goes. The depth gate judges every sample. | Yes: findings, verdicts | no sample that removes material is outside the gates | R4 |
| P29-7 | Force and deflection read every declared cut (slot ring, planner cap). One deflection limit for one cut. Finish deflection before the simulation. | Yes: Pocket and Profile depths | Suggest's cut passes the gates on every declared cut | R5 |
| P29-8 | Entries as declared cuts before the simulation: the inclined engagement; one entry rule set for both emitters; the dressup plunge reads the op plunge rate. | Yes: entry feeds and geometry | one entry rule, two emitters | R6 |
| P29-9 | A depth or width where the op states none (finish: the stock the earlier op leaves; Project Curve: the offset as a bound). Else `NoNumber` on the record. | Yes: new pre-sim values | no op has a silent missing value | - |
| P29-10 | Display: every surface reads `Check`. Planned and sample points share the chart. The force field draws for every op. Viz and MCP stop assembling values. | Display only | one record on every surface | - |
| P29-11 | Other metrics on the cut: efficiency through the engagement, one rubbing rule, cusp height of a stated stepover, one reached-feed measure, the per-op `Capabilities` metric flags. | Small | - | later |
| P29-last | Delete the scattered scope reads; a search guard keeps them out. | No | the guard | - |

Before the operator's cut: P29-0 to P29-6. The rest is logged and follows. P29-11 does not
block anything.

Size: P29-1 to P29-3 are wide refactors (24 engagement sites, 19 `cut_spec` sites, 10 span
filter sites, 6 `depth_cap_rule` readers) with byte-identical output as the proof. One
implementer, one step at a time.

## 7. Rulings for the operator

- **R1. The adaptive depth rule.** The force budget replaces `2 x D` for adaptive roughs. On
  the 6 mm rough the depth at 1.2 mm width falls from 6.0 to about 3.85 mm. Accept, or
  review the budget first.
- **R2. The adaptive width.** No source prints one. Options: (a) keep the stated width and
  only cap the depth; (b) 0.5 D, where chip thinning ends (a geometric fact); (c) the machine
  factor (0.2 D, no source). The lead recommends (a) now and (b) as the proposal when the
  width is unset.
- **R3. Slot and groove ops read the roughing limits.** A full slot is judged as a full slot,
  whatever the op is called. The finish label keeps only what is about finish quality.
- **R4. Entries and cleanup in the gates.** They warn like any cut, shown as their own group.
  The relative "entry load" rule goes when the absolute one exists.
- **R5. One deflection limit.** Proposal: the stock the cut leaves, with 200 um as the ceiling;
  the same value before and after the simulation.
- **R6. One entry rule set.** Which ramp angle, pitch and plunge feed stand for both
  emitters. No source exists; the operator's own practice is the best input.

## 8. Fit with the other designs

- **P26** (no width rule for adaptive) becomes R1 and R2 of P29-4. It is not a phase of its own.
- **P25** (row lookup as categories) runs beside P29. The vendor row is the input that gives
  a cut its chip; the route by op kind (`LutRoute`, M6) becomes a route by the cut and the
  tool category. P25-0 can run at any time.
- **P24b** (one force line) is unchanged: `CutLoad` holds the one line.
- **P27** (feeds from fundamentals) comes AFTER P29-3. Its model then enters as more limits
  on the cut (tool body stress, edge strength, chip room) that return the same `Check`, and
  it cannot grow a second pipeline. The brief needs one added paragraph: "limits are
  functions of a `Cut` and return a `Check`; add limits there". If the dedicated session
  starts before P29-3 lands, it works on the model and the evidence only, with no product
  wiring.

## 9. Not verified

- The finish generators and several 2D kernels were read from their module docs, not line by
  line (marked D in part A).
- The zone checks (core, shank, holder, flute reach) were not re-read; the drill's missing
  body-zone walk needs a check against the mesh holder check.
- Every figure of section 5 outside P26 is an estimate. P29-0 measures them.

## 10. Operator answers (2026-11-11)

- **R1: accepted.** "35.3 budget.. 1.2mm width at 4mm seems around ok. I imagine it would do ok
  at 6 but it could be right that it's pushing the machine hard!" The budget stands; the
  adaptive depth reads it.
- **R2: open, the operator asked how the width is set.** "I thought this was just calculated on
  the fly by the calcs trying to hold the load?" The lead's answer: the planner holds the
  engagement at a target that comes from the stated stepover; it does not choose the stepover.
  Lead default until the operator objects: Suggest keeps a stated stepover and caps the depth;
  with no stepover it proposes the best pair inside the limits with a width ceiling of 0.5 D.
- **R3: accepted.** "yeah, slot is a slot! it can vary in z depth due to rest and similar
  though". Before the simulation the limit reads the stated depth (the worst case); after it,
  each sample is judged at its real depth.
- **R4: accepted.** Entries and cleanup are judged like any cut, as their own group.
- **R5: the operator has no view.** Lead default: a rough pass may bend up to the stock it
  leaves, never above 200 um; the same value before and after the simulation. Flagged as a
  lead decision; the operator can change it at P29-7.
- **R6: the operator asked what the question is.** Explained in the reply; it waits for P29-8.

The operator has not yet said "approved" for the design and the order as a whole.
