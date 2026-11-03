# Plan: contracts the compiler holds (feeds, tools, entries, findings)

Status: RULED 2026-11-03 (section 0). Progress and open decisions: `STATUS.md`. Phase hand-offs: `phases/P*.md`. K2 is now the envelope design and needs one more look (the
UX sketch in K2b). Owner: the operator. Lead: this session.

## 0. Operator rulings (2026-11-03)

| # | Ruling | Effect on the plan |
|---|---|---|
| Q1 | At the feed cap, compare "lower rpm, same chip" with "same rpm, smaller chip" by power and ship the one inside the curve (option c). | K7 power check picks the arm; replaces R4 Q10 at the cap. |
| Q2 | Folded into Q4: the envelope gives a geometry proposal with its basis when the operator has not stated a value. | K2b |
| Q3 | A score tie between vendor rows takes the lower chip; the card shows both rows. | K2 row choice |
| Q4 | Suggest is a **space of safe choices**, not one number. The UI shows the space and where the operator's value sits. | K2 becomes K2b `SafeEnvelope`; Suggest never writes geometry by itself. |
| Q5 | No feedback loop. Suggest stays fast; the simulation verifies. | K7 keeps one engagement definition; the sim result is shown against the envelope, it does not change Suggest. |
| Q6 | **The program never refuses an operator; every finding is a warning** (confirmed 2026-11-03: "people might want to generate shit g-code just for aircut tests or similar to test the machine"). This includes the refusals that exist today: height order, cuts into the model, G-CENTRECUT, load exceeded. | K6 `Consequence` has no blocking variant. Generation and export always produce output and carry the warnings with it. The compiler still refuses *code* (that is the plan's purpose); it never refuses a *job*. |
| Q7 | Do not stress the display of estimated facts. The **lineage** of a toolpath must show clearly which facts it used and how each is known. | K3 keeps `ToolFact`; K5 outcomes for Estimated facts warn; new K8 `Lineage`. |
| Q8 | A straight plunge for Project Curve now; a ramp that follows the projection later if it is cheap. | K4 `StraightPlunge` now, `Planner` on-curve ramp later. |
| Q10 | A missing input (unset depth or floor, no chip data, no tool, inch units, a zero feed) stops generation or emission with a reason. Only findings warn. | K6 `EmitError` and NotReady reasons stay outside the `Consequence` column. |
| Q9 | Bare numbers in old project files: fail to load. Fast iteration; this is not a production build. | K3 loader refuses; the planning fixtures are migrated by a script in the same commit. |

Evidence: `planning/feeds_blind_review_2026-11-03/` (the review, `rootcause/INDEX.md` and six
reports) and four research maps in `research/` (A op catalog, B feeds pipeline, C tool facts and
feasibility, D IR, entries and findings). Every file:line below comes from those maps.

## 1. The problem in one paragraph

The review found 24 dangerous field values on 7 of 19 toolpaths (ops 1, 2, 3, 5, 6, 7, 14; after
the corrections of 2026-11-03). They do not come from 24 bugs.
They come from four places where the code lets a missing fact turn into a number without a
decision:

1. **Suggest does not receive the cut.** All 25 registry rows hold `FeedsHints::NONE`; 18 ops send
   no hint; no op sends a radial width; `FeedsInput` reads `None` as "auto-calculate" and fills it
   from 31 default sources (B 1.8). The apply step cannot tell an operator value from a default,
   so `ApplyScope::Both` writes the default over the operator (B 7.1).
2. **A missing tool fact reads as the optimistic fact.** `non_centre_cutting: None` means "centre
   cutting"; the loader fills flute length 25, stick-out 45, flutes 2 with no warning; no field
   records "measured" or "estimated" (C 1.3, 1.4).
3. **An op can skip the entry decision.** `strip_all` removes the entry and states no plunge rule;
   the pipeline does not read `DressupPolicy`; the dressup plunge feed is a hidden
   0.5 x first feed (D 2.2, 2.4).
4. **A finding has no stated consequence.** Export reads no `Diagnostic`; collisions, entry load and
   link-cuts-stock block nothing in core; each surface decides; three doc comments claim blocks
   that the code does not do (D 3.2, 3.3).

## 2. The principle

> A decision that affects the cut is a **type**, not a default. The compiler refuses code that
> skips the decision. A test only pins the answer that the type forces.

The codebase already does this in places, and the plan copies those places:

| Existing pattern | Where (A 3.1, C 3) | Copied by |
|---|---|---|
| Exhaustive match, no wildcard ("a new op fails to compile until it decides") | `OperationConfig::cutting_levels`, `catalog.rs:1940-1944` | K1, K3, K4 |
| Full row literals, no `Default` on the row type | `Capabilities`, `capabilities.rs:3-4` | K1, K3 |
| Required macro clauses (F163) and derive refusals (F185) with `compile_fail` doctests | `operation_configs.rs:3538`, `rs_cam_derive/src/lib.rs:353` | K1 |
| Unit newtypes that refuse a mix | `feeds/quantities.rs:15-17` | K2 |
| Typestate (sealed) | `trace/transform_provenance.rs:68-83` | K2 |
| One fact, one predicate, one refusal per stage (G-CENTRECUT) | C 3.5 | K5 |
| A consequence that is one typed function | `BoundSource::gates_export`, `ExportRefusal::override_needed` | K4 |
| One mutation door | `ProjectSession::apply(Command)` (WP7) | all |

## 3. The contracts

Seven contracts, K1 to K7. Each has: the type, what the compiler refuses, the sentry, and the
root causes it closes (section 5).

### K1. Every op states its cut: `CutSpec`

Replace `FeedsHints` (and the two `Option<f64>` inputs of `FeedsInput`) with one required value
that each op builds from its own config and heights.

```rust
/// CONTRACT K1. Built only by `OperationConfig::cut_spec` (exhaustive, no wildcard).
pub struct CutSpec {
    pub radial: Radial,
    pub axial: Axial,
    pub total: TotalDepth,
    pub first_pass: FirstPass,      // what the first pass meets
}
pub enum Radial {
    Width(Stated<Mm>),              // Pocket/Face/Zigzag/Adaptive stepover
    Slot,                           // Profile, Trace, Project Curve (every side), Bore: ae = D
    Groove,                         // V ops: width follows depth (no tool angle: config only)
    Scallop(Stated<Mm>),            // finishing: width from the scallop target
    AngularStep(Stated<f64>),       // Radial Finish, degrees
    SurfaceFollow,                  // Waterline, Ramp Finish, single-pass Pencil
    Axial,                          // Drill, Pin Drill: no radial cut
}
pub enum Axial {
    PerPass(Stated<Mm>),            // depth_per_pass the operator set
    ZStep(Stated<Mm>),
    OnePass(Option<Stated<Mm>>),    // Project Curve depth, Inlay pocket depth; None = S11 unset
    Peck(Stated<Mm>),
    HelixPitch(Stated<Mm>),         // Bore
    ChamferWidth(Stated<Mm>),       // Chamfer
    Surface { max: Option<Stated<Mm>> },
}
pub enum TotalDepth { Known(Stated<Mm>), FromHeights, Unset, None(&'static str) } // None needs a reason
pub enum FirstPass { Slot, NotSlot } // P4: from the config; FreshStock/RestStock wait for the stock source
```

- `Stated<T>` holds the operator's value and marks it "operator". Only the op's own config can
  make one. Suggest cannot build a `Stated` (private constructor in `compute/`).
- **Refused by the compiler:** a new `OperationConfig` variant without a `cut_spec` arm; an arm
  that returns a default (no `Default` on `CutSpec`, `Radial`, `Axial`); `TotalDepth::None` with no
  reason string.
- **Row rule:** each registry row states `cut_rule: CutRule` (`RadialRule`, `AxialRule`,
  `calc_reads`); `CutRule` has no `Default`, so a new `REG_*` literal does not compile without
  it. The derive hook (`#[derive(OpParams)]` emits a `HasCutSpec` bound) is dropped (P4 decision
  D4): the exhaustive `cut_spec` match and the full row literal hold the rule.
- **P4 corrections (lead decisions D1-D3, 2026-11-03):** `FirstPass` is `Slot | NotSlot` until
  Suggest receives the stock source; `Radial::AngularStep`, `Axial::HelixPitch`,
  `Axial::ChamferWidth`, `TotalDepth::Unset` and `Axial::OnePass(Option<..>)` were added so that
  each operator field reaches the spec; `Radial::Groove` holds no tool angle; Profile, Trace and
  Project Curve on every side, and Bore, are `Slot`.
- **Sentry:** walk `OperationType::ALL`; for each op, a config with a known stepover and depth
  must give a `CutSpec` that carries those numbers (not a frozen table of answers: the test
  checks that the op's own field reaches the spec). This replaces
  `operation_feeds_hints_is_an_explicit_per_op_decision`, which today freezes `NONE`.

### K2. Every shipped number names its source: `Sourced<T>` and `Recommendation<Complete>`

```rust
/// CONTRACT K2. No public constructor for `value` alone.
pub struct Sourced<T> { value: T, basis: Basis }
pub enum Basis {
    Operator,                        // from a `Stated` in the CutSpec
    Printed(RowRef),                 // a vendor row cell
    Claim(ClaimRef),                 // G1/G2/G3/G6/G10 on a RowRef
    Rule(&'static FormulaConstant),  // a named rule; Cited/Unsourced is visible (F201)
    Derived { law: &'static str, from: Vec<Basis> },
}
pub struct Recommendation<S: Completeness> { /* Sourced fields */ }
// S = Complete | Partial. `apply` accepts only Recommendation<Complete>.
```

- The function that owns a rule is the only builder of its `Sourced` value (as `resolve_plunge`
  already owns the plunge). A family default can become a shipped number only through a
  `Rule(&FormulaConstant)`, so the card shows it as a rule, Cited or Unsourced.
- **Geometry ownership.** Suggest never replaces a `Basis::Operator` field with a lower basis. It
  may only lower it when a sourced cap binds (flute guard, deflection, power), and the new value
  carries `Derived { law: "<cap>", from: [Operator, ...] }`. Raising an operator depth (the
  14.05 mm case) does not compile: the cap API returns `min(stated, cap)` only.
- `Complete` needs every written field to have a basis that is not `Rule` with an Unsourced
  constant, OR an explicit operator acceptance recorded in the command. A `Partial`
  recommendation shows on the card and cannot be applied.
- A unit newtype per field (`Rpm`, `FeedMmMin`, `PlungeMmMin`, `ChipMm`) so a plunge cannot take a
  side feed from another row: the G10 plunge takes `Sourced<FeedMmMin>` and checks that its
  `RowRef` is the claim's row (report 02 c).
- **Sentry:** a `compile_fail` doctest for each refusal (raise an operator depth; build a
  `Sourced` without a basis; apply a `Partial`). The FM1 matrix re-blesses in the commit that
  moves numbers (B 5, FM1 rule).

### K2b. Suggest returns a space of safe choices: `SafeEnvelope` (ruling Q4)

Suggest stops being "one number that overwrites the op". It returns the region of the
(depth, width) plane where the cut is safe for this tool, machine and material, with the feed and
rpm at each point, and one proposed point. Today only an axial envelope exists, and only for some
ops (`feeds/profile.rs:56`, `ConstraintEnvelopes { axial }`); K2b generalises it.

```rust
/// CONTRACT K2b. Built only by `feeds::envelope::for_cut` from a CutSpec (K1).
pub struct SafeEnvelope {
    pub axes: EnvelopeAxes,                 // which of ap / ae the op can vary (from the CutSpec)
    pub boundary: Vec<Limit>,               // each limit is a curve with its law and source
    pub speeds: SpeedsMap,                  // (ap, ae) -> Sourced<Rpm>, Sourced<FeedMmMin>, plunge
    pub proposal: Sourced<CutPoint>,        // one point, with the reason it was chosen
    pub operator: Option<Placement>,        // where the operator's stated point sits
}
pub struct Limit { kind: LimitKind, law: &'static str, basis: Basis } // power, flute, deflection,
                                                                     // chip band, rubbing floor, slot
pub enum Placement { Inside { margin: f64 }, Outside { binding: LimitKind } }
```

- **What Suggest writes.** "Apply" writes the speeds (rpm, feed, plunge, ramp feed) for the
  geometry the operator keeps. It writes depth or stepover only when the operator picks a point
  (the proposal or another point inside). The command records the pick. There is no silent
  geometry overwrite; that closes the 14.05 mm depth and the face stepover by construction.
- **When the operator has not stated a value** (Q2): the proposal is that value, with its basis
  (a sourced claim, or a named rule shown as Unsourced). The op is not ready to generate until the
  value is set, by the operator or by accepting the proposal (the S11 "unset refuses" rule stays).
- **An operator point outside the envelope** is a warning that names the binding limit (Q6: no
  block). The card shows it; the lineage records it.
- **UI sketch (to agree before Phase 5).** One small plot per op: depth on one axis, width on the
  other, the safe region drawn by its edge lines, not shaded (ruling G-CHARTLINES, 2026-09-23: feeds charts draw lines, not fill), the binding limit named on each edge, a dot for the operator, a
  ring for the proposal; feed and rpm read out for the point under the pointer. Ops with one free
  axis (Trace depth, Drill peck) show a bar, not a plane. The MCP tool returns the same struct as
  JSON, so an agent reads the same space. A text-light design, per the operator's UI rule.
- **Why it is not too complicated.** The calculator already computes each limit (flute guard,
  deflection, power, chip band, rubbing floor); today it collapses them into one point and throws
  the rest away. The envelope keeps them. The cost is a sample grid per op (a few dozen calculator
  calls, all closed-form; no generation), which stays fast (Q5).
- **The type rule.** `SafeEnvelope` has no public constructor; `apply` takes a `CutPoint` that
  came from an envelope (`EnvelopePick<'env>`), so a caller cannot apply speeds for a point the
  envelope did not compute.

### K3. Every tool fact states how it is known: `ToolFact<T>`

```rust
/// CONTRACT K3. Fields are private; readers choose an accessor.
pub enum ToolFact<T> {
    Stated { value: T },                   // the operator stated this value (measured or read)
    Catalogue { value: T, source: String },
    Estimated { value: T, basis: String },
    Unknown,
}
impl<T> ToolFact<T> {
    pub fn known(&self) -> Option<&T>;               // Stated, Catalogue
    pub fn planning(&self) -> Option<(&T, Confidence)>; // any value, with its confidence
}
pub enum CentreCutting { Yes, No(NonCentreCuttingGeometry) }
```

- Applies to: `cutting_length`, `stickout`, `flute_count`, `shank_diameter`, taper, a new V tip
  flat, and `centre_cutting: ToolFact<CentreCutting>`. `Unknown` replaces "`None` = centre
  cutting".
- The project TOML reads `stickout = { measured = 30.0 }` or `{ estimated = 25.0, basis = ".." }`.
  A bare number refuses to load (ruling Q9) with a message that names the key and the two forms.
  A script migrates the fixtures in `planning/` in the same commit. An absent key is `Unknown`, not
  25 / 45 / 2 (C 7.1).
- **Refused by the compiler:** a read of a fact as a bare number (the field is private);
  a planner that uses `planning()` must carry the `Confidence` into its output or drop it on
  purpose with a named function.
- Blast radius: 605 references in 185 files read `.stickout`, `.cutting_length` or `.flute_count`;
  135 references in 51 files read `non_centre_cutting`. Most are test builders. Section 7 phases
  this with a builder and accessor methods first.

### K4. Every op states how it enters material: `EntryContract`

```rust
/// CONTRACT K4. A required registry field; no Default.
pub enum EntryContract {
    Dressup { allowed: EntrySet, default: RoleDefault },
    Planner { kinds: EntrySet },                     // 3D Rough, Bore, Pencil crease
    DescentIsCut,                                    // Drill, Pin Drill
    StraightPlunge { reason: &'static str, feed: PlungeFeedRule },
}
pub enum PlungeFeedRule { OpPlungeRate, AxialChip(ClaimRef), InAirThenAxialChip }
```

- Replaces `DressupPolicy`, `EntryStylePolicy` and the 3D Rough entry field as the one source. The
  pipeline reads the contract (today it does not read the policy, D 2.2).
- `strip_all` cannot leave an op with no entry: Project Curve must state `StraightPlunge
  { reason, feed }` (today's behaviour, now visible and graded) or a `Planner` on-curve ramp.
- A `StraightPlunge` op with a tool whose `centre_cutting` is `No` or `Unknown` generates and
  carries the K5 `PlungeKnown` or `CentreCut` warning (ruling Q6: no refusal).
- The hidden `entry_plunge_rate` (0.5 x first feed, `dressup_apply.rs:711-722`) becomes a stated
  `PlungeFeedRule`.
- **IR:** add `MoveIntent::EntryApproach` (feed through air above the entry) so the air lead is
  not `EntryPlunge` (D 1.4). Every `match` on `MoveIntent` without a wildcard then fails until it
  decides, which is the wanted effect (96 files mention `EntryPlunge`). Add one `EntryRecord`
  per entry in `Produced`; readers (`rough-score`, triage, modulator) read the record.

### K5. Every physical rule has the G-CENTRECUT shape: `PhysicalRule`

```rust
/// CONTRACT K5. One row per rule in `physical_rules!` (X-macro); each row names every stage.
physical_rules! {
    CentreCut   { fact: centre_cutting, plan: Clamp, gen: Warn, sim: Measure, export: Warn },
    ReachFlute  { fact: cutting_length, plan: None,  gen: Warn, sim: Measure, export: Warn },
    ReachHolder { fact: stickout,       plan: None,  gen: Warn, sim: Measure, export: Warn },
    PlungeKnown { fact: centre_cutting, plan: None,  gen: Warn, sim: None,    export: Warn }, // no limit is known to clamp to
    VTipFlat    { fact: v_tip,          plan: Model, gen: Warn, sim: None,    export: Warn },
}
```

- One feasibility stage after `run_stages` (`session/compute.rs:738`) reads the emitted toolpath,
  the tool facts and the stock box, and gives a typed `FeasibilityReport`. It needs no mesh, so
  op 3 (hole 30 mm, flute 25 mm, DXF model) is caught (C 4.1).
- The outcome (ruling Q6, Q7): every rule **warns**; none blocks generation or export. The
  stage values are `Clamp`, `Model`, `Measure`, `Warn` or `None`; no `Refuse` value exists, so a
  refusal cannot be written. The warning text names the fact and how it is known ("flute 25 mm,
  Estimated: hole 30 mm"). The plan-stage clamp stays: the planner still avoids the danger where it
  can (the G-CENTRECUT ramp at the limit angle); only the outcome where it cannot changes from a
  refusal to a warning. The rule needs every stage, because the warning must be the same on every
  surface and in the lineage.
- **Refused by the compiler:** a rule row that omits a stage (macro arity); a stage marked
  `None` without a reason.
- **Sentry:** per rule, a test that is red with the defect injected (the G-CENTRECUT pair is the
  template).

### K6. Every finding states its consequence: `Consequence` on `diag_ids!`

```rust
diag_ids! {
    project.entry_load      => Consequence::Warn(Danger::Machine),
    project.link_cuts_stock => Consequence::Warn(Danger::Part),
    project.air_cut         => Consequence::Info,
    ...
}
/// No blocking variant (ruling Q6): the program never refuses a job.
pub enum Consequence { Warn(Danger), Info }
pub enum Danger { Machine, Tool, Part }
```

- A row without a consequence does not compile. A finding without an id does not compile, so each
  of today's refusals gets an id (G-CENTRECUT descent has none, D 2.3).
- Generation and `export_program` never refuse on a finding. They return the output plus the
  `Warn` findings. Every surface (GUI, MCP, CLI) shows the same list; today only the GUI asks on
  collisions (D 3.2). `Severity` stays for display.
- Today's refusals become warnings in Phase 2: height order, cuts into the model, G-CENTRECUT
  (generation in `JobPhases::run_stages` and export), load exceeded (its override goes; nothing is
  left to override). The GUI "Export Anyway" dialog becomes a plain warning list. A
  `Danger::Machine` warning may keep one confirm click in the GUI; it never stops MCP or CLI.
- Lead proposal: the exported G-code header lists each warning as a comment, so the file states
  what was knowingly accepted (an air-cut test stays self-describing).
- **Sentry:** a project with each of today's refusal cases exports a program on every surface,
  and the export result names each finding id.

### K7. One engagement definition, before and after the simulation

- One `Engagement { ap, psi, d_eng, section }` with one constructor from a planned `CutSpec` and
  one from a sim sample (arc). Both feed `PowerTerms::of` and `lateral_cutting_force`. Today the
  gate, Suggest and the modulator use three different widths (B 4.2). This follows the operator
  rule "one load definition codebase-wide".
- Pre-sim power evaluates the `CutSpec` envelope, not one point: nominal, slot (when
  `FirstPass::Slot` or `Radial::Slot`), and the planner cap (adaptive trochoid cap). It ships the
  maximum with its case name. Op 1 would show the near-slot case the 1.18 kW peak came from.
- Ruling Q5: no feedback loop. The simulation verifies: its measured engagement and power are
  placed on the K2b envelope plot (a sim point outside the envelope is a warning that names the
  limit). Suggest never reads the simulation. The profile boundary (`feeds/profile.rs:15-21`)
  stays.
- Ruling Q1: at the cutting-feed cap, the power check compares the two arms ("lower rpm, same
  chip" and "same rpm, smaller chip") on the power curve and ships the arm inside it; the basis
  names the arm. This replaces R4 Q10 only at the cap.

### K8. Every toolpath carries its lineage: `Lineage` (ruling Q7)

- One record per generated toolpath: the `CutSpec` (K1), the envelope pick and its basis (K2b),
  each tool fact used with its `ToolFact` state (K3), the entry contract and the entries made (K4),
  the K5 rule results and the K6 findings. It is built from the `Sourced` bases, so it needs no
  second bookkeeping.
- It is shown in the toolpath details and returned by MCP and the CLI. It is not a banner; the
  estimated facts are visible where a person looks for "how was this made" (Q7).
- **Refused by the compiler:** `ToolpathComputeResult` cannot be built without a `Lineage` (a
  required field, no `Default`). Not `Produced`: it is the kernel side channel and derives
  `Default` (lead decision 2026-11-03, `phases/P5b.md` D3).

## 4. How an agent sees the contracts

- `crates/rs_cam_core/CONTRACTS.md` (40 lines or fewer, the folder-file style): one line per
  contract K1-K8, the type, the file, the sentry. Each folder `CLAUDE.md` that a contract touches
  names it in its "invariants" lines.
- Each contract type's doc comment starts with `CONTRACT Kn.` and states what the compiler refuses.
- Each refusal has a `compile_fail` doctest beside the type (the F206 style), so the docs show the
  rejected code.
- Workspace lint: deny `clippy::wildcard_enum_match_arm` (not set today, A 3.1). Where a wildcard
  is needed, `#[expect(.., reason = "..")]` states why (W16 holds this).

## 5. Root cause to contract

| Root cause (`rootcause/INDEX.md`) | Ops | Closed by |
|---|---|---|
| 3D Rough depth 14.05 (no depth hint, rigidity floor, dial) | 1, 14 | K1 Axial::PerPass + K2b (no silent geometry write) |
| Face stepover 5.49 (no stepover hint) | 0, 12 | K1 Radial::Width + K2 |
| Slot unseen, finish row, chip 0.382 | 5, 18 | K1 Radial::Slot; K2 row choice must match the CutSpec class; a tie takes the lower chip and shows both rows (Q3) |
| rpm lowered at the feed cap, power falls | 1, 5, 14 | ruling Q1; K7 power on the envelope |
| plunge = side feed / z from another row | 5 | K2 RowRef check on the G10 plunge |
| no power for slot ops | 5, 18 | K1 + K7 |
| sim peak never returns | 1 | K7 + ruling Q5 |
| Trace / Project Curve depth unseen, 14 slit passes | 9, 10, 13 | K1 Axial::OnePass/PerPass + K2 |
| R1 depth from 2 x tip, not the operator | 16 | K1 + K2 |
| long-tool factor never lowers the feed | 15-17 | K2 (the dial becomes a `Derived` cap with its law shown); ruling B7 stays open |
| Amana tapered cell contradicts itself | 17 | data fix (Phase 0) |
| Project Curve strips entries, 168 straight plunges | 5 | K4 StraightPlunge + K5 PlungeKnown |
| rest entry 14.5 mm, links 13 mm | 2 | K6 (one shared warning on every surface, Q6); the planner fix is separate work (needs sim) |
| non-centre-cutting plunges in 3D Rough | 1 | K5 CentreCut plan stage for cut and link descents |
| hole deeper than flute / holder | 3 | K5 ReachFlute, ReachHolder |
| unknown centre cutting plunges silently | 6, 7 | K3 Unknown + K5 PlungeKnown |
| V tip not modelled | 9 | K3 v_tip fact + K5 VTipFlat |
| dwell has no effect in a peck cycle (G82 emits it; peck and chip-break cycles do not) | 4 | Phase 0 (D2 in `phases/P0.md`) |
| entry counter miscounts | (tooling) | K4 EntryApproach + EntryRecord; Phase 0 counter fix |
| 31 default fills | all | K1 + K2: each fill becomes a `Rule` basis or disappears |

## 6. What the contracts do not do

- They do not choose numbers. A ruling or a sourced claim still chooses each number; the types
  only stop a number from shipping without a source.
- They do not fix the op 2 rest planner (the 14.5 mm entry). K6 makes the existing finding the same
  warning on every surface; the planner fix needs a simulation and is separate work.
- They do not remove the operator's freedom: a stated value always wins over a default; an
  override is a named, recorded command.

## 7. Migration order

Each phase is one package with its own commit(s). "Same numbers" phases must keep the FM1 matrix,
the op goldens and the G-code byte-identical; their sentry is that nothing moved.

| Phase | Work | Numbers move? | Gate |
|---|---|---|---|
| 0 | Local fixes: `rough-score` entry counter; `suggest` CLI prints the peck and passes the power fallback; `derive_wanaka21.py:594` uses the flute; drill dwell (P0 D2); Amana tapered v8 cell; parse warnings for unknown tool material / cut direction; `stickout >= cutting_length` validate rule | small, named | focused tests |
| 1 | Lint: deny `wildcard_enum_match_arm`; fix or `#[expect]` each hit (`apply_drill_defaults` `_ => {}` first) | no | clippy |
| 2 | K6 column: commit A today's behaviour, commit B every finding refusal becomes a warning (Q6), the GUI "Export Anyway" step becomes one confirm on `Danger::Machine`; emit errors (inch units, bad feed, compensation) stay as `EmitError`; warnings in the G-code header | yes: projects that refused now export (G-code of non-refused projects byte-identical apart from the header) | sentry + goldens |
| 3 | K4 `EntryContract` with today's behaviour; pipeline reads it; `EntryApproach` intent; `EntryRecord` | no (IR tags change; G-code same) | goldens, G-code byte-identical |
| 4 | K1 `CutSpec` built per op; Suggest reads it but each op keeps its present basis | no | FM1 identical |
| 5 | K2 `Sourced` bases; K2b `SafeEnvelope` (core + MCP JSON); Apply writes speeds only, geometry only on a pick; Q3 lower-chip tie | yes: every silent overwrite of an operator value stops | FM1 re-bless, cells named |
| 5b | K2b envelope plot in the GUI (agree the sketch first) and K8 `Lineage` view | no | viz tests |
| 6 | K3 `ToolFact` with accessors, then private fields; TOML provenance (bare numbers fail, Q9; fixtures migrated by script); Unknown centre cutting | no for stated tools; new warnings for Unknown | loader tests |
| 7 | K5 feasibility stage and the five rules (warnings, Q6) | warnings added | per-rule red/green sentries |
| 8 | Q1 cap policy (power picks the arm; after P9 changes 1-2); Q8 Project Curve `StraightPlunge` | yes (Q1) | FM1 re-bless |
| 9 | K7 one engagement; sim points placed on the envelope (no feedback, Q5) | yes | sim CSV re-bless (heavy-tests: ask first) |

Phases 1-4 touch `feeds/`, `compute/catalog`, `dressup/`, `diagnostics/`. The perf session
(`planning/perf_sweep_2026-11-03/PLAN.md`) edits `mesh.rs`, `dropcutter.rs`, `ops/project_curve.rs`
and `finish/scallop/ring_generation.rs`. Phase 3 does not edit `ops/project_curve.rs` (the op emits through `toolpath.rs`), but it must wait for
that session to commit.

## 8. Decisions for the operator (ruled 2026-11-03: see section 0; the table below is the record of the options)

| # | Question | Options | Lead's recommendation |
|---|---|---|---|
| Q1 | R4 Q10 at the default 6000 cap: Suggest lowers the rpm and keeps the chip, so the power on a constant-torque spindle falls (op 1: 1.01 kW at 16219) | (a) keep Q10; (b) at the cap, keep the rpm and lower the chip when the power check binds; (c) compare both by power and ship the one inside the curve | (c) |
| Q2 | May the family default table (`mod.rs:2978-3083`) ship a geometry number? | (a) yes as `Rule`; (b) preview only, `Partial` until the operator states the value | (b) for geometry; feeds keep their claims |
| Q3 | A score tie between vendor rows | (a) id order (today); (b) refuse and name both rows; (c) take the lower chip | (c) with both rows on the card |
| Q4 | `ApplyScope::Both` on an operator depth or stepover | (a) overwrite (today); (b) cap only (lower, never raise); (c) never touch geometry, speeds only | (b) |
| Q5 | Post-sim power returns to Suggest (profile boundary, `profile.rs:15-21`) | (a) no; (b) yes as `Basis::Simulated`, a second Suggest stage | (b), after K7 |
| Q6 | Export consequence per finding: `project.entry_load` Critical, `project.link_cuts_stock`, rapid / holder / obstacle collisions, machine-safety `Error` | per row: BlockExport with or without an override, or Warn | BlockExport with a named override for each; collisions without an override |
| Q7 | An Estimated tool fact that a rule needs | (a) refuse export until measured; (b) refuse export unless a named waiver; (c) warn only | (b) |
| Q8 | Project Curve entry | (a) `StraightPlunge` with the G6 axial-chip feed, refused for non-centre or unknown tools; (b) a new on-curve ramp planner | (a) now, (b) later |
| Q9 | Bare numbers in old project files | (a) Estimated; (b) Catalogue; (c) refuse to load | (a) |

## 9. Cost and risk

- Phases 0-4 change no number and are mostly type work: one agent each, the lead reviews by path.
- Phase 5 is the largest behaviour change: every op where Suggest overwrote operator geometry
  moves. The FM1 matrix re-blesses with the moved cells named.
- Phase 6 has the widest edit (605 references); a builder for test tools keeps the diff mechanical.
- Each phase ends with: the folder sentries, the focused crate tests, clippy. The core full gate
  and any test over three minutes need the operator's go-ahead (root `CLAUDE.md`).

## 10. Terms

| Term | Meaning | Source |
|---|---|---|
| FM1 | The feeds matrix golden: 1000 cells that must equal `tests/fixtures/feeds_matrix_2026-09-23/matrix_2026-09-23.csv`; a feeds change re-blesses it and names the moved cells. | `crates/rs_cam_core/src/feeds/CLAUDE.md` |
| F201 | Each feeds formula constant is marked `Cited` or `Unsourced`; the unsourced count is pinned. | `planning/tech_debt_2026-10-19/F201_UNSOURCED_FEEDS_CONSTANTS.md` |
| F206 | Derive refusals proven by `compile_fail` doctests (`--doc op_params_refusals`). | `crates/rs_cam_core/src/compute/CLAUDE.md` |
| W16 | A lint exception is `#[expect(lint, reason = "..")]`; `#[allow]` is denied. | root `CLAUDE.md` |
| R4 Q10 | At a feed ceiling Suggest lowers the rpm and keeps the chip. Ruling Q1 replaces it at the cap. | `planning/extrapolation_2026-09-24/RAMP_PLAN.md` |
| B7 | The open op-architecture ruling on the long-tool factor. | `planning/op_architecture_2026-10-14/PLAN.md` |
| G-CENTRECUT | The model rule shape: tool fact → predicate → plan clamp → generation outcome → sim measure → export outcome → sentry. Its outcomes were refusals; ruling Q6 makes them warnings. | `crates/rs_cam_core/src/tool/CLAUDE.md`, `compute/CLAUDE.md` |
| G1 … G10 | Claim rules in the feeds pipeline: G2 Janka hardness basis, G3 family rules, G6 drill rules and the ramp axial chip, G10 plunge fraction. | `crates/rs_cam_core/src/feeds/CLAUDE.md` |
| Op n | 0-based toolpath index in the nz-wanaka-21 project of the blind review. | `planning/feeds_blind_review_2026-11-03/briefs/INDEX.md` |
