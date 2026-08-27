# Feeds, speeds, chipload, and load-feedback architecture review

Date: 2026-08-07  
Revision reviewed: `d8202268719bb05d51abdd48c78f19823d0be71d`  
Scope: every production path that recommends, writes, modifies, simulates,
assesses, optimizes, graphs, narrates, or explains feed/RPM/DOC/WOC.  
Status: **review only — no production code or recommendation number changed.**

## Executive conclusion

The system has a **good layered core**, not a single clean unified system yet.
Its strongest pieces are the shared power and deflection physics, the
stage-labelled post-simulation explanation, per-field provenance, and the
full-simulation optimizer. Those are real architectural improvements, not just
better comments.

However, a user cannot currently treat all visible chipload signals as one
thing. The system still shows three different physical quantities under nearby
or identical "chipload" language:

| Quantity | Meaning | Correct use |
|---|---|---|
| **Commanded advance/tooth** | `programmed feed / (RPM × flutes)` | compare with a vendor chipload band; show the recipe the user set |
| **Achieved advance/tooth** | commanded advance/tooth after predicted machine feed / emitted modulation | compare with the same vendor band; post-simulation gate value |
| **Arc-mean chip thickness** | material-chip geometry at a particular engagement arc | force/engagement analysis only; it cannot be compared directly with the vendor advance band without a separately sourced conversion |

The current viewport heat-map compares the third value with a band for the
first value. This is a real correctness defect on an operator-facing graph,
not merely a colour/wording preference. It directly explains why a recommended
setting can look bad in the graph even when the corrected gate says it is
acceptable.

There is also one stale recommendation mechanism still capable of moving real
settings: `arc_fit_ratio_for_op` predicts the old arc-mean gate metric and
feeds the Suggest recalibration for Adaptive3d and DropCutter. The source
itself records that retiring it would move its solved feeds by 4× and 6.7×,
respectively. That is not safe to casually delete, but it must not continue to
be presented as a current-model recommendation.

## Architecture assessed

```text
Tool / material / machine / operation configuration
                 │
                 ▼
feeds::calculate() ───────────────► FeedsResult (static recipe)
                 │                      │
                 ▼                      ├── Feeds UI / provenance / modal
feeds::suggest::apply_*()               └── Suggest invariant passes
                 │
                 ▼
OperationConfig / generated toolpath IR
                 │
                 ├── optional feed_modulation (per-move emitted feed)
                 └── machine kinematics (predicted achieved feed)
                 │
                 ▼
SimulationCutTrace
                 │
                 ▼
tool_load::{chipload,power,deflection,drill_gates}
                 │
                 ├── diagnostics / narration / preflight
                 ├── viewport / timeline
                 └── full-simulation optimizer
```

This separation is fundamentally sound:

- **Suggest is a pre-simulation projection**, not a claim that it knows the
  final engagement of every move.
- **Tool-load gates are post-simulation assessment** and must be the authority
  when current simulation data exists.
- **Feed modulation is an emitted-toolpath transformation**, not an alternate
  static recommendation.
- **Optimization evaluates generated candidates**, rather than trusting a
  heuristic formula.

The main architectural failure is at the boundaries: values cross layers as
bare `f64`s with an implied measure, and several presentation/action paths make
an independent decision rather than consuming one named operating-point
contract.

## What is architecturally clean

### 1. Power physics has one formula and an explicit axis

`tool_load::power::predicted_power_kw` is the shared formula used by Suggest
and the simulation gate (`crates/rs_cam_core/src/tool_load/power.rs:55-71`,
`feeds/mod.rs` Step 6). The prior safety-factor display mismatch is fixed: the
UI-facing power numerator and denominator now live on the same commanded-feed
axis. The focused parity test passes.

This is the desired pattern: one formula, distinct static-vs-observed inputs,
and an explicit explanation of why they may differ.

### 2. Deflection has a genuine shared physical owner

`feeds::force::lateral_cutting_force` is used by the pre-simulation predictor,
cutter axial constraints, simulation gate, optimizer preflight, and feed
modulation (`crates/rs_cam_core/src/feeds/force.rs`). `effective_feed_for_sample`
is shared by chipload, power and deflection gates (`tool_load/mod.rs:24-61`).

The inputs legitimately differ by stage — static WOC/DOC versus sample arc and
axial engagement — but they converge on one force/cantilever model. This is the
best architectural example in the area.

### 3. Post-simulation chipload diagnostics now preserve stages and units

`feeds::explanation::FeedExplanation` explicitly labels commanded advance,
matched band, achieved-feed ratio, and gate observation
(`crates/rs_cam_core/src/feeds/explanation.rs`). The tool-load adapter surfaces
both the observed result and the commanded-over-band fact
(`diagnostics/adapters/from_tool_load.rs:76-205`).

This fixes the old B3 failure mode where four incompatible numbers appeared
without a relationship. It is also the right user model: **commanded** and
**achieved** may differ because a router cannot reach the programmed feed in
short/corner-heavy moves.

### 4. Provenance is per value, not a badge on a whole recipe

`FeedsProvenance` records source per feed, plunge, RPM, DOC, WOC, and scallop
field (`feeds/provenance.rs`). It distinguishes a vendor RPM plus formula feed,
manual values, optimizer values, and automatic correction. This is a strong
answer to “why did this setting change?” and avoids the old misleading
single-source label.

### 5. Deliberate geometry duplication is guarded

`ToolGeometryHint::engaged_diameter_at_doc` and
`MillingCutter::lookup_diameter_at` duplicate a shape calculation because the
static calculator intentionally does not carry a full cutter. The source says
so and has a cross-shape DOC parity sentry (`feeds/mod.rs:108-127`). This is a
justified, tested duplication — not a concern to eliminate blindly.

## Findings

### [high] The viewport chipload graph compares incompatible units

**Locations**

- `crates/rs_cam_core/src/tool_load/mod.rs:201-223`
- `crates/rs_cam_viz/src/app/gpu_upload.rs:1058-1082`
- `crates/rs_cam_viz/src/render/toolpath_render.rs:720-747`

**Evidence**

`chipload_envelopes_for_session` supplies a vendor band in linear advance per
tooth. `build_chipload_per_move` then takes the maximum
`effective_chip_thickness_mm` for each move. The first module documents this
exactly as a known mismatch and states that it paints false rubbing risk.

**Impact**

This is the most likely explanation for the user observation: a static
recommendation may be fine, and the post-simulation gate may be fine, while the
viewport paints the same path blue/red because it is comparing a thinner
arc-mean chip to an advance-per-tooth band. The colour therefore cannot be used
to validate or reject recommendations.

**Recommended repair**

Create a named per-move display measure, e.g.
`AchievedAdvancePerTooth`, from:

```text
 effective_feed_for_sample(sample, predicted_feeds)
 ----------------------------------------------------
             sample.rpm × sample.flutes
```

Use it exclusively with the vendor envelope in the viewport/timeline. Keep
arc-mean chip thickness available as a distinct engagement/force visualisation;
do not rename it to chipload or compare it to a vendor band.

**Acceptance**

A synthetic two-arc fixture must demonstrate that changing only engagement arc
changes the chip-thickness graph but not the feed-per-tooth band comparison.
Add a screenshot test/read review: gate, colour, and displayed units must agree.
This is display-only, but it is visible behaviour and should ship with a
screenshot rather than as a core cleanup.

---

### [high] Stale arc-fit prediction still changes Suggest recommendations

**Locations**

- `crates/rs_cam_core/src/feeds/predict.rs:633-891`
- `crates/rs_cam_core/src/feeds/suggest.rs:2059-2154`
- `planning/review_2026-08-04/TECH_DEBT_2_CLOSEOUT.md:718`

**Evidence**

`arc_fit_ratio_for_op` states in its own source that it predicts the old
arc-mean chip-thickness metric removed from the gate on 2026-08-06. It is still
called by `recalibrate_feed_for_chipload`; calibrated ratios 0.25
(Adaptive3d) and 0.15 (DropCutter) can raise a feed toward a target calculated
against a quantity that the gate no longer reports.

**Impact**

This is not a graph-only problem. It can make two recommendation families
materially wrong or at least impossible to justify with the current model. The
code records the retirement blast radius: 4× for Adaptive3d and 6.7× for
DropCutter. It also means the rationale text can promise “observed chipload”
without predicting the current observation.

**Recommended repair**

Do not re-fit per-operation constants. The pre-simulation system cannot know
achieved/commanded feed ratio without an emitted path plus machine model. Build
a dedicated evidence package:

1. capture current Suggest → generate → simulate outcomes for at least two
   fixtures per affected family;
2. measure command, achieved feed, gate observation, and any post-sim
   modulation separately;
3. choose explicitly between retiring automatic feed-up, limiting it to a
   clearly marked estimated mode, or moving this action into a simulation-backed
   optimizer;
4. red-first test the approved change and publish before/after recommendations.

Until then, the UI should label this result as a legacy pre-simulation estimate
rather than presenting it as gate-targeted calibration.

---

### [high] The modal and main panel have different safety and apply contracts

**Locations**

- validated main recipe: `crates/rs_cam_core/src/feeds/suggest.rs:637-664`
- infallible modal preview: `crates/rs_cam_core/src/feeds/suggest.rs:671-689`
- modal controls: `crates/rs_cam_viz/src/ui/feeds_modal.rs:381-587`
- modal write paths: `crates/rs_cam_viz/src/controller/events/mod.rs:780-948`
- main panel’s split apply: `crates/rs_cam_viz/src/ui/properties/mod.rs:1724-1930`

**Evidence**

The main panel calls `feeds_result_for_operation`, which validates an
unrunnable tool/operation pair. The modal calls `feeds_explain_for_operation`,
which deliberately runs the infallible calculator as a “would-have-produced”
preview. But the modal exposes per-field Apply and an `Apply all` button.
Those actions can write the preview values directly or invoke the full legacy
apply path.

The main panel correctly separates `Apply recommended speeds` from `Apply cut
geometry`; the modal's `Apply all` explicitly overwrites RPM, feed, plunge,
DOC, and WOC. Thus two user-visible “apply recommendation” paths have different
validation and different cut-geometry effects.

**Impact**

A user can unknowingly change cut geometry via the modal while the primary
panel says speed application does not change geometry. The modal can also offer
and apply an invalid pairing which the primary panel refuses. This is both
architectural duplication and a safety/UX issue.

**Recommended repair**

Make preview and application separate types:

- `FeedsPreview`: may be infallible/read-only and must be visibly marked as an
  unvalidated hypothetical;
- `ApplicableRecommendation`: created only after
  `validate_tool_for_operation` and the canonical invariant funnel succeed;
- all writes (single field, speeds, cut geometry, project batch, modal) route
  through one application API with an explicit `ApplyScope`.

In the short term, remove modal write affordances or route them to the same
speed-only/cut-only calls used by the main panel. Do not preserve a legacy
all-fields write merely for UI muscle memory.

---

### [high] The “chipload” visual vocabulary does not distinguish command, achievement, and geometry

**Locations**

- static card: `crates/rs_cam_viz/src/ui/properties/mod.rs:1724-1930`
- modal’s current/recommended and nomogram verdict: `ui/feeds_modal.rs:381-587`,
  `:1688-1704`, `:2068-2080`, `:2168-2189`
- simulation gate explanation: `feeds/explanation.rs`
- modulation operating point: `properties/mod.rs:1917-1924`

**Evidence**

The main card shows a static recipe. The modal’s nomogram judges
`feed/(rpm×flutes)` against its static row. The simulated gate judges achieved
advance/tooth. The viewport currently judges arc-mean thickness. All are useful
in their own domains, but the visible surfaces do not establish which one is
being shown before applying green/red “within band” language.

**Impact**

Even after the heat-map unit repair, users can reasonably conclude that a
static Suggested value failed because the post-simulation number is lower. In
fact, the machine may be slowing corners or feed modulation may deliberately
be changing per-move feed.

**Recommended repair**

Use one compact operating-point card after simulation:

```text
Commanded:  0.031 mm advance/tooth
Achieved:   0.024 mm advance/tooth  (77%; machine kinematics)
Vendor band: 0.018–0.032 mm advance/tooth
Gate:       Within — peak steady-state 0.028 mm advance/tooth
```

Show arc-mean thickness only under an “engagement/force” label. The pre-sim
card should say `Predicted recipe`, not `Within`/`Break` unless it is explicitly
only evaluating the command-space vendor band.

---

### [medium] Suggest and the gate can still select different LUT rows

**Locations**

- static recipe / modal: `feeds/mod.rs:990-1048`,
  `feeds/explain.rs:167-213`
- gate / optimizer / viewport envelope: `tool_load/chipload.rs:465-484`,
  `tool_load/mod.rs:224-311`
- ledger: close-out `F-LUT2`

**Evidence**

Suggest uses `find_best_row_for_geometry`, which admits RPM-only observations.
The gate, optimizer and viewport use the chipload-envelope resolver, which
requires chipload-bearing rows. This is a legitimate policy distinction only if
it is made explicit; today the recipe can be anchored by one observation while
the assessed band comes from another.

**Impact**

Recommended RPM/chipload/source and the gate/graph band may disagree without a
physical change in the toolpath. It is a direct duplication of row-selection
policy, not merely an implementation detail.

**Recommended repair**

Introduce one resolver with an explicit purpose enum, for example:

```rust
enum LutUse {
    Recipe { allow_rpm_only: bool },
    ChiploadEnvelope,
    DisplaySiblings,
}
```

First run a number-preserving census that records whether the selected row
changes on the current fixture/catalogue set. Then decide whether recipe
selection should be one combined row, a deliberate two-row recipe with two
provenance labels, or envelope-first selection. Do not collapse the paths before
measuring which operator-visible values move.

---

### [medium] The optimizer initially evaluates a different feed world than production simulation

**Locations**

- candidate scoring: `crates/rs_cam_core/src/tool_load/optimize/candidate.rs:374-396`
- production simulation options: `crates/rs_cam_core/src/session/compute.rs:2273-2325`
- reconciliation UI: `crates/rs_cam_viz/src/ui/optimize_project.rs:564-678`

**Evidence**

Candidate scoring deliberately sets both `use_predicted_feed_in_gates` and
`adaptive_feed_modulation` to `false`. Normal simulation can enable either.
Project optimization has a post-apply reconciliation surface, which is good,
but the initial candidate card is not necessarily the operating point that will
be displayed/exported after normal simulation.

**Impact**

This is an intentional isolation boundary, not an accidental duplicate formula.
It is still a user-model gap: “safe/faster candidate” means safe/faster in the
unmodulated commanded-feed candidate model until reconciliation, not necessarily
in the live emitted-feed model.

**Recommended repair**

Keep the isolation for now, but make it explicit in every optimizer result:
`candidate estimate — commanded feed, no feed modulation`. Require/recommend
post-apply regeneration and reconciliation before presenting it as final. Before
attempting any unification, add an end-to-end retarget fixture — close-out
ledger `F-OPT` records that current tests use synthetic verdicts and do not
measure a real post-unit-conversion optimizer outcome.

---

### [medium] Two more old chip-thickness-versus-advance policies remain

**Locations**

- axial DOC chipload floor: `feeds/cutter_constraints.rs:386-462`
- bipolar engagement refusal: `tool_load/chipload.rs:219-286`
- gate’s vestigial sample predicate: `tool_load/chipload.rs:703-725`

**Evidence**

The axial envelope computes an arc-mean chip thickness and compares it with a
vendor `chipload_min_mm_per_tooth`. The bipolar optimizer predicate does the
same intentionally, and its source correctly states that the samples are useful
but the vendor advance band is the wrong yardstick. The main gate still requires
`effective_chip_thickness_mm` merely to preserve its pre-conversion sample
population, even though its observation is now achieved advance/tooth.

**Impact**

The axial-envelope path can influence Suggest’s depth-per-pass decision for
ball/tapered tools using an uncalibrated cross-unit threshold. The bipolar and
validity cases are not simple “replace with fpt” fixes: doing so would remove
the engagement-variation signal. They need a separate physical envelope or an
explicit heuristic contract.

**Recommended repair**

Treat these as a family of **chip-thickness policy** work, not as leftovers of
the corrected fpt gate:

1. decide whether axial minimum-DOC is a sourced physical limit, a heuristic
   advisory, or should be retired;
2. keep the bipolar signal only with a named/sourced chip-thickness envelope or
   a disclosed derived scaling;
3. make the gate-population change its own measured package, because moving the
   predicate changes verdict population as well as arithmetic.

---

### [low] There is still local formula duplication, but it is not the primary problem

Nominal feed-per-tooth division appears in simulation stamping, modal current
values, narration, static predictor, cutter constraints, and gates. Some of
this is legitimate because the values have different fallback/invalid-input
semantics. The problem is not “seven divisions exist”; it is that the output
is often a bare `f64` with no type-level measure or stage.

A generic global helper is worthwhile only once it accepts a named input type
and returns a named measure. Replacing divisions mechanically risks hiding the
actual distinction between commanded, effective, and geometric chip values.

## Recommended target architecture

Do not undertake a mega-rewrite. Preserve the current boundaries, then make the
cross-boundary contracts explicit.

```text
StaticRecipeInputs
   -> RecommendationPlan { command-space values, LUT selection, provenance }
   -> ApplicableRecommendation { validated + invariant-resolved }
   -> OperationConfig
   -> EmittedOperatingPoint { per-move programmed/modulated feed }
   -> SimulatedOperatingPoint { achieved feed, engagement, sample population }
   -> LoadAssessment { band, criteria, explanations }
```

Important domain types should prevent accidental comparison:

```rust
struct AdvancePerToothMm(f64);
struct ArcMeanChipThicknessMm(f64);
struct CommandedFeedMmMin(f64);
struct AchievedFeedMmMin(f64);
struct VendorChiploadBand { min: AdvancePerToothMm, max: AdvancePerToothMm };
```

This is not a units-framework mandate. Small newtypes at the gate/graph/UI
boundaries would have prevented the current heat-map bug and make Rust reject
many accidental cross-domain comparisons.

## Recommended sequence

1. **Graph correctness first (small, visual, no recipe-number change).**
   Replace the heat-map measure with achieved advance/tooth, preserve a separate
   chip-thickness visual, and add an image/synthetic two-arc sentry.
2. **Make UI application one funnel.**
   Remove or reroute modal writes; validate before application; preserve
   speed-only versus cut-geometry actions everywhere.
3. **Resolve stale arc-fit Suggest recalibration.**
   This needs a dedicated before/after evidence package and an operator decision
   because it moves recommended Adaptive3d/DropCutter feeds substantially.
4. **Unify LUT selection policy after a delta census.**
   Use one resolver with declared purpose rather than two accidental entry
   points.
5. **Give optimizer results their simulation assumptions.**
   Then build a real retarget/reconciliation fixture before merging its model
   with live modulation/kinematics.
6. **Open a separate chip-thickness-policy investigation.**
   It owns axial DOC floors, bipolar engagement, and the gate population
   predicate; it must not be folded into a cosmetic graph change.

## Verification performed

Focused tests on the reviewed revision:

```text
cargo test -p rs_cam_core --test feed_explanation_snapshot_b3 -q
  7 passed
cargo test -p rs_cam_core --test chipload_report_wording_t12_t15 -q
  6 passed
cargo test -p rs_cam_core --test power_ceiling_parity_f2 -q
  7 passed
```

Before running Cargo: `free -g` and `pgrep -af "carg[o]"`; no concurrent Cargo
job was present. This review did not run a live GUI/MCP session, so the
heat-map finding is source-proven but should still receive the proposed
screenshot validation before a UX conclusion is promoted.

## Bottom line for the user

When a recommendation and the current chipload graph disagree, do **not**
assume the recommendation is wrong and do **not** assume the graph is right.
Today the viewport graph has a known unit error. Prefer the post-simulation
load verdict plus its stage-labelled diagnostic, while checking whether the
machine achieved materially less feed than commanded. The recommendation is
still only a pre-simulation recipe; the simulated gate is the authority once a
current, measurable simulation exists.
