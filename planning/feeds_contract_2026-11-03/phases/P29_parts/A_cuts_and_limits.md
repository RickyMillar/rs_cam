# P29 part A: the cuts and the limits (items 1, 2, 3 of the investigation plan)

Read-only survey, 2026-11-11, HEAD `484542abe`. No product code, no cargo run.
Every path is relative to `crates/rs_cam_core/src/` unless it starts with `crates/`.
Line numbers are the lines read on that commit.

Marks used in the tables:

- (R): read in the code of the generator or the limit.
- (D): from the module doc, the folder `CLAUDE.md`, the registry row or the config struct. The
  generator body was not read line by line.

Caller counts come from `rg` over `crates/*/src`, without test files and comment lines.

---

## 1. What exists toward the target

### 1.1 Answer to the key question

No. There is no ONE function that takes an engagement and returns force, power, deflection and
chip. There is one engagement TYPE and four model functions that read it. Each caller assembles
the four by hand, with its own constructor, its own feed and its own scope filter.

| Quantity | The one model function | Inputs | Line |
|---|---|---|---|
| Power (kW) | `PowerTerms::of(PowerModelInputs).kw_at_feed(feed)` (`predicted_power_kw`) | engagement section, force line, rpm, Z, feed | `tool_load/power.rs:113, 135, 175` |
| Force (N), mean and peak | `Engagement::edge_force(line, fz, Z) -> EdgeForce` | engagement (ap, psi, d_eng, helix), force line, fz, Z | `feeds/engagement.rs:348` |
| Tip deflection (mm) | `predict::tip_deflection_from_engagement(tool, material, engagement, fz)` | engagement, tool body (stickout, E), material, fz | `feeds/predict.rs:624` |
| Peak chip (mm) | `RadialThinning::lift(ae, d_eng)`; post-sim `display::peak_chip_per_tooth` | ae, d_eng, the thinning rule (role), advance | `feeds/geometry.rs:85`, `tool_load/display.rs:83` |

The inverse functions (a limit as a solved value) are separate again:
`Engagement::fz_cap_for_peak_force` (`feeds/engagement.rs:393`), `PowerTerms::feed_for_kw`
(`tool_load/power.rs:150`), `force_budget::depth_at_budget` (`feeds/force_budget.rs:351`),
`cutter_constraints::cutter_axial_constraints` (`feeds/cutter_constraints.rs:208`).

The wrappers that call the model functions, and how they differ:

| Wrapper | Line | Engagement built by | Width | Feed | Scope read |
|---|---|---|---|---|---|
| Calculator Step 6 `worst_case_power` | `feeds/calculator/power.rs` (3 calls) | `from_cut` | max over `PowerCases` | the raw feed of Step 5 | `is_axial_plunge()` = family Drill (`:112`) |
| `power_at_operating_point` / `power_at_shipped_point` | `feeds/operating_point.rs:240, 370` | `from_cut` (`:290`) | max over `PowerCases::of_operation` (`:307`) | op feed, or the steady feed | `depth_after_simulation` (`:262`) |
| `force_at_operating_point` / `force_at_shipped_point` | `feeds/operating_point.rs:344, 399` | `resolve_operating_point` | nominal | MEAN chip | `depth_after_simulation` (`:353, 407`) |
| `force_budget::peak_force_n` | `feeds/force_budget.rs:329` | `from_tool_cut` | stated | fz argument | none |
| `predict_peak_deflection_um` | `feeds/predict.rs:301` | (internal) | stepover, else a family default (`:373`) | op feed, rpm fallback 18000 (`:291`) | Drill (`:313`), `depth_after_simulation` (`:350`), Adaptive (`:373`) |
| `cutter_axial_constraints` | `feeds/cutter_constraints.rs:208` | `from_tool_cut` (2 calls) | argument | fz argument | none |
| `sample_power_kw` / `gate_sample_power` | `tool_load/power.rs:198, 265` | `from_sample` (depth `axial_doc_mm`) | measured arc | `effective_feed_for_sample` | drill kind, span predicate (`:273-278`) |
| `sample_tip_deflection_mm` | `tool_load/deflection.rs:198` | `deflection::sample_engagement` (depth `axial_engagement_mm`) | measured arc | `deflection_feed_per_tooth_mm` | none (the caller filters) |
| `cut_force::sample_peak_force_n` | `tool_load/cut_force.rs:68` | `deflection::sample_engagement` | measured arc | fz argument | none (the caller filters) |
| Modulator `constrained_max_move` | `dressup/feed_modulation.rs:719` | `ModulationCutter::engagement` (`from_cut`, `:326`) and `SampleEngagement::of_sample` (`:263`) | mean or per sample | solved | move intent skip (`:680-702`), `ForceBudgetScope` (`:442`) |

### 1.2 The engagement and the section

| Item | Holds | Constructed by | Read by | Gap to the target |
|---|---|---|---|---|
| `Engagement` (`feeds/engagement.rs:166`) | `ap_mm`, `ae_mm`, `psi_rad`, `d_eng_mm`, `section: EngagedSection`, `helix_deg: Option`. Private fields; a literal outside the module does not compile (`:34-47`). | `from_cut` (`:181`): 10 call sites in 7 files. `from_tool_cut` (`:211`): 9 in 6 files. `from_sample` (`:227`): 5 in 5 files (`tool_load/power.rs`, `display.rs`, `depth.rs`, `dressup/feed_modulation.rs`, `compute/simulate.rs`). | `power_inputs` 9 sites; `edge_force` 7 sites outside the file; `fz_cap_for_peak_force` 3 sites (`tool_load/deflection.rs`, `feeds/efficiency.rs`, `dressup/feed_modulation.rs`). | It is geometry only. It holds no feed, no rpm, no flute count, no tool body, no material, no kind (body, slot, entry, cleanup), no source (planned or measured), no time weight. It has no axial (plunge) member: a straight plunge is `PowerModelInputs::axial_plunge` (`:742`), which is not an `Engagement`. |
| `EngagedDiameter` (`feeds/engagement.rs:97`) | `d_eng`, `nominal`. Private. | `of_hint` (`:110`), `of_tool` (`:130`). | Every `Engagement` constructor. | None for this purpose. One open residual: the V-bit hint adds the flat tip and the cutter does not (`:88-91`). |
| `EngagedSection` (`feeds/section.rs:54`) | `Rectangle {ap, ae, d_eng, psi}`, `VGroove {..}`, `AxialCircle {diameter, core}`. | `side_cut` (`:174`), `axial_plunge` (`:213`, flat tool only; every other tip is `TipShapeNotModelled`, `:223-226`). | `PowerTerms::of` only (area and edge term). | `SectionKind::gates()` (`:141`) says an axial circle never moves a value. A ramp or a helix has no section: it is not a side cut and not a straight plunge. A ball or a taper in a plunge has no section. |
| `PowerModelInputs` (`feeds/engagement.rs:721`) | line, section, rpm, Z. Private. | `Engagement::power_inputs` (`:313`), `axial_plunge` (`:742`). | `PowerTerms::of`. | Already the "engagement plus rpm plus Z" record for power only. Force and deflection take `fz` and `Z` as loose arguments. |
| Second sample door: `deflection::sample_engagement` (`tool_load/deflection.rs:169`) | Returns an `Engagement` through `from_tool_cut`. | 6 call sites: `deflection.rs` (2), `cut_force.rs` (2), `distribution.rs`, `feed_modulation.rs`. | Deflection gate, force card, force-budget cap of the modulator. | It reads `axial_engagement_mm`. `Engagement::from_sample` reads `axial_doc_mm` (`feeds/engagement.rs:235`). The doc says so (`deflection.rs:165-167`). One sample gives two engagements: one for power and depth, one for deflection and force. `SampleEngagement` holds both (`dressup/feed_modulation.rs:245-253`). |

### 1.3 The cut declaration (`compute/catalog/`)

| Item | Holds | Constructed by | Read by | Gap |
|---|---|---|---|---|
| `CutSpec` (`cut_spec.rs:65`) | `radial: Radial` (7 variants, `:78`), `axial: Axial` (8 variants, `:115`), `total: TotalDepth`, `first_pass: FirstPass`. No `Default`. | Only `OperationConfig::cut_spec` (`compute/catalog.rs:1157-1341`), an exhaustive match of 25 arms. | 19 call sites in 10 files: `feeds/operating_point.rs` (5), `trace/lineage.rs` (3), `feeds/envelope.rs` (2), `feeds/suggest/apply.rs` (2), `rs_cam_viz/src/ui/feeds/envelope.rs` (2), and one each in `feeds/suggest.rs`, `predict.rs`, `force_budget.rs`, `engagement.rs`, `session/compute/toolpath_diagnostics.rs`. | It is ONE cut per op: the body pass as the operator states it. It holds no list. It has no entry, no cleanup, no link, no finish step-down. `calc_inputs` (`:335`) drops a peck, a helix pitch, a chamfer width and a surface offset (`:343-346`), and a groove, a scallop, an angular step and a surface width (`:358-362`): the calculator then reads a family default. |
| `FirstPass` (`cut_spec.rs:255`) | `Slot` or `NotSlot`. | The same match. | ONE reader: `PowerCases::of_cut` (`feeds/engagement.rs:868-871`). | It adds a power case only. Force budget, deflection and the feed do not read it. The exception is the Pocket slot ring (below). |
| `PlannerWidth` (`cut_spec.rs:200`) | `None`, `PassCeiling`, `CapMult(f64)`. | `OperationConfig::planner_width` (`compute/catalog.rs:1359-1396`), a match on the op kind and on `ClearingStrategy`. | ONE reader: `PowerCases::of_operation` (`feeds/engagement.rs:876`). | The first planner declaration to the feeds side. `CapMult` is unsourced and serves the power case only (`cut_spec.rs:207-210`). |
| `PowerCases` / `PowerCase` (`feeds/engagement.rs:853, 816`) | `slot: bool`, `planner: PlannerWidth`. Cases: `Nominal`, `PlannerCap`, `Slot`, `AxialPlunge`. | `of_cut`, `of_operation`, `NOMINAL`: 8 call sites in 8 files. | `widths`, `worst` (power); `full_feed_width` (the lift): 7 call sites in 7 files. | The nearest thing to "the list of cuts an op declares". It is a list of WIDTHS at one depth, for power and for the lift only. |
| `UnliftedFeed::of_slot_ring` (`feeds/geometry.rs:151`) | The op feed over its lift, for the first ring. | Matches `OperationConfig::Pocket` with the contour pattern only (`:160-187`). | `compute/execute/clearing_2d.rs:219`, `feeds/profile.rs` (the card). | The only place where a declared non-body cut reaches the emitted feed. One op, one pattern. |
| `OpRegistryEntry` (`compute/catalog/schema.rs:640`) | `op_type`, `spec` (label, families, `feeds_family`, `feeds_pass_role`), `params`, `tool_constraints`, `lead_link_policy`, `entry: EntryContract`, `geometry`, `policy`, `capabilities`, `generate`. | 25 statics `REG_*` in `compute/catalog/registry.rs:80-1620`. No row uses `..`. | `OperationType::registry_entry` (`compute/catalog.rs:629`), an exhaustive match. | A new op does not compile without a row. The row holds a cut RULE (`CutRule`, `cut_spec.rs:265`), not the cuts. The compile-time net exists; the content is one cut. |
| `Capabilities` (`capabilities.rs:45`) | About 25 fields. Relevant: `cut_rule`, `move_count: MoveCountModel` (`:622`), `axial_envelope: AxialEnvelope` (`:672`), `adaptive_woc`, `planner_uniform_engagement`, `entry_probe`, `lut_route`. | The rows. | `OperationConfig::capabilities()`. `axial_envelope` has ONE reader (`feeds/suggest/axial_envelope.rs:59`). | `axial_envelope`, `move_count`, `adaptive_woc` are limit scopes stored as row flags. They are the per-op scope decisions that P29 point 1 removes. |
| `EntryContract` (`entry_contract.rs:29`) | `Dressup {allowed, default, none_is, own}`, `Planner {kinds}`, `DescentIsCut`, `StraightPlunge {reason, feed}`. | The rows (`registry.rs`, 25 uses). | `compute/config.rs` (23), `compute/execute/dressup_apply.rs` (5), `trace/lineage.rs` (4), `compute/feasibility.rs` (2), `compute/execute/volumetric.rs` (1). | It states which KINDS of entry an op can make. It holds no geometry (angle, radius, pitch) and no engagement. No feeds or load code reads it for a load. |
| `EntryRecord` (`entry_contract.rs:273`) | `kind`, `approach_from`, `from`, `to`, `feed_mm_min`. | Only `entry_records` (`:363`), from the emitted moves, after generation. | ONE call site outside the file (`session/compute.rs`). "No stage that writes moves reads a record" (`:14-16`). | This is already "one object per entry cut", built from the moves. It has no engagement and no limit reads it. |
| Entry geometry: `feeds::ramp::entry_geometry` (`feeds/ramp.rs:332`) | `Entry {geometry: Ramp{angle} / Helix{radius, pitch}, source, clearance}`. | From `DressupConfig`, or from `Adaptive3dConfig` for the 3D rough (`:340`). | The ramp feed (`resolve_ramp_feed`, `:691`), the entry notes (`:937`). | Two sources of entry geometry (`EntrySource::Dressup`, `EntrySource::Adaptive3d`, `:277`). The 2D Adaptive helix radius is decided a third time by the planner bore (`adaptive/opening.rs:24-45`), with the shared function `dressup::contained_helix_radius`. |
| `every_op_conforms_w4` | An integration test, `crates/rs_cam_core/tests/every_op_conforms_w4.rs`. | - | - | NOT READ in full. The folder file says check 6 is a known red (`compute/CLAUDE.md:39`). The sentries that hold the cut are `every_op_states_its_cut_k1` and `every_op_states_its_entry_contract_k4` (`compute/CLAUDE.md:28`). |

### 1.4 Span kinds, move intents, the feed cap

| Item | Holds | Written by | Read by | Gap |
|---|---|---|---|---|
| `MoveIntent` (`toolpath.rs:56`) | 13 variants: `Drilling`, `EntryPlunge`, `EntryApproach`, `ClearingCut`, `FinishingCut`, `EntryHelix`, `EntryRamp`, `Linking`, `Retract`, `LeadIn`, `LeadOut`, `ObstacleClearance`, `Unknown`. | Each generator and dressup, per move. | The modulator skip (`dressup/feed_modulation.rs:680-702`), the span builder (`compute/spans.rs:331-339`), `entry_records`, the triage (`stock/sim_triage.rs:1402-1405, 1649`). | The intent is the only per-move statement of what a cut is. It has two cut kinds (`ClearingCut`, `FinishingCut`). It has no slot ring, no cleanup, no perimeter sweep, no spring pass. Profile, Trace, VCarve and Inlay emit `FinishingCut`; Profile is filed Roughing (section 2). |
| `SpanKind` (`trace/toolpath_spans.rs:184`) | 10 variants: `Operation`, `DepthPass`, `Region`, `Entry`, `LeadOut`, `LinkBridge`, `DressupArtifact`, `GeometryRefit`, `RapidOrderBarrier`, `WaterlineCleanup`. | `compute/spans.rs` (from intents `:331-339`, from 3D rough events `:586-605, 644-656`), `dressup/mod.rs` (`:620, 1671, 1674, 1843, 2045, 2486`). | `tool_load/locality.rs`. | `LeadIn` moves get `SpanKind::Entry` (`compute/spans.rs:338`). A `WaterlineCleanup` span is every move from a cleanup marker to the next depth-pass start (`compute/spans.rs:588-605`). |
| `is_steady_state_for_gate` (`tool_load/locality.rs:156`) | `!is_phantom_transit && !is_configured_entry`. Phantom: `WaterlineCleanup`, `LinkBridge`, `LeadOut`, `DressupArtifact`, or `in_transit_span` with no `Entry` ancestor (`:198-214`). | - | 10 call sites outside the file: `tool_load/chipload.rs`, `power.rs`, `depth.rs`, `cut_force.rs`, `mod.rs`, `distribution.rs` (2), `stock/sim_triage.rs` (2, both with `None` for the span lookup, `:944, 1713`), `compute/simulate.rs:1116`. | The one span filter of the post-sim side. P29 point 3 says "no span filter". With `None` for the lookup, every in-transit sample is phantom (`:219`), so the two triage readers also drop entries. |
| `Move::feed_cap` (`toolpath.rs:143`) | `Option<f64>`: the highest feed a move may carry. | `ops/pocket.rs:431` (the slot ring), `adaptive3d/path.rs:2718` (from `FeedCapStart` markers), `adaptive/geodesic/slice.rs:190` (`feed_caps`). | `dressup/feed_modulation.rs:1553`, `dressup/feedopt.rs:262`, `dressup/condition.rs:109-116`, arc fit, TSP, `finish/surface_link.rs:627, 641`, `stock/simulation_cut.rs:875-890` (a capped-move record). | A planner-to-feed-stage channel. It carries a feed, not a cut. The feeds side (Suggest, the envelope, the cards) does not read it. |
| `FeedCapSource` (`adaptive3d/mod.rs:510`) | 6 variants: `PlannedTurn`, `Capped2d`, `DepartureWindow`, `CappedSplit3d`, `LevelContour`, `StockRead3d`. | The geodesic plan and the 3D lift (`adaptive3d/clearing.rs:3608-3984`, `level_contour.rs:611`). | The semantic trace (`trace/semantic_trace.rs:44, 1214`). | It names WHY a move is capped. Each variant is a cut over the pass ceiling, up to a slot. These are real declared cut kinds; they live in the 3D rough only and reach no limit. |

### 1.5 The verdict and limit records

There are six record shapes for "a value against a limit".

| # | Shape | Line | Fields | Used by |
|---|---|---|---|---|
| 1 | `Limit` + `Edge` + `LimitKind` (pre-sim envelope) | `feeds/envelope.rs:379, 302, 236` | `kind` (14 kinds), `law: &str`, `basis: Basis`, `edge` (`ApCeiling`, `AeCeiling`, `ApFloor`, `AeFloor`, curves, `PerPoint`, `Unmodelled{reason}`). `Limit::margin(p)` (`:467`). | `SafeEnvelope`; `rs_cam_viz/src/ui/feeds/envelope.rs` (32 `LimitKind::` uses). |
| 2 | `Placement` + `JudgedValue` + `MeasuredTool` | `feeds/envelope.rs:573, 556, 520` | The stated or measured point against the limits. | `SafeEnvelope::place` (`:1496`), `compute::simulate::envelope_placement`. |
| 3 | `ChiploadVerdict`, `PowerVerdict`, `DeflectionVerdict`, `DepthVerdict` | `tool_load/verdict.rs:1786, 1912, 2115, 2242` | Four enums, each `Within / Exceeds / Unmodeled`, each with its own fields. Depth adds `Reported` (`:2265`). | `ToolpathLoadVerdict` (`:302`), `OwnMaterialVerdict` (`tool_load/mod.rs:693`). |
| 4 | `CriterionStatus` (the shared projection of shape 3) | `tool_load/verdict.rs:1223` | `kind: CriterionKind` (8 kinds, `:967`), `state`, `confidence`, `unmodeled_reason`, `sample_range`, `population`, `display_peak`, `unit`, `bound`, `bound_source: BoundSource` (7 variants, `:1048`), `exceeded`. | `gcode/mod.rs` (8), `rs_cam_viz/src/ui/sim_diagnostics.rs` (4), `sim_op_list.rs` (2), `tool_load/distribution.rs` (3). |
| 5 | `BindingConstraint` (the modulator) | `tool_load/verdict.rs:32` | 9 variants: `ChiploadMax`, `DeflectionMax`, `PowerMax`, `MachineMaxFeed`, `KinematicReach`, `ChiploadMin`, `PlungeRate`, `FeedCap`, `ForceBudget`. | `dressup/feed_modulation.rs` (35), `session/compute.rs` (8), `feeds/cutter_constraints.rs` (8). |
| 6 | `RpmLimit` / `RpmLimitValue` | `feeds/calculator/spindle.rs` (18 uses, one file) | The named RPM limits of Step 5c. | `FeedsResult::rpm_choice`. |

Other records of the same idea: `CutForceReading` (`tool_load/cut_force.rs:159`, report-only, not
a criterion), `EntrySpike` (`tool_load/verdict.rs:1665`), `EntryLoadObservation`
(`stock/sim_triage.rs`), `AxialBindingConstraint` (`feeds/cutter_constraints.rs:153`),
`PowerFigure` (`feeds/operating_point.rs:136`), `RigidityDepthCap` (`machine/mod.rs`),
`FeedsWarning`, `SuggestWarning` (`feeds/suggest.rs:269`), `DrillGateOutcome`
(`tool_load/drill_gates.rs:24`).

Gap to P29 point 5: `CriterionStatus` is the nearest shape to "value, limit, margin, binding,
basis". It holds `display_peak`, `bound`, `bound_source`, `state`, `population`. It has no margin
field and no "binding" field. `BoundSource` (post-sim) and `Basis` (`feeds/sourced.rs`, pre-sim)
are two different provenance types for the same idea. `LimitKind` (14) and `CriterionKind` (8)
are two lists of limits with no shared member type: power is `LimitKind::Power` and
`CriterionKind::Power`; the force budget is `LimitKind::Rigidity`, `CriterionKind::DepthOfCut`,
`BindingConstraint::ForceBudget` and `CutForceReading`.

`UnmodeledReason::NotApplicableForOp(String)` (`tool_load/verdict.rs:210`) carries free text.
`UnmodeledReason::NotImplemented(String)` (`:199`) exists; the depth gate uses it for "no machine
profile" (`tool_load/depth.rs:152-156`).

### 1.6 The limit sources

| Item | Line | Signature (inputs) | Reads op type? |
|---|---|---|---|
| `SafeEnvelope::for_cut` | `feeds/envelope.rs:1122` | `EnvelopeInput {operation, tool, material, machine, context, ..}` | Yes: `axes_plan` (`:2375`), `removal_rate` (`:1287-1292`), each `*_limit` builder. 7 call sites: `feeds/suggest/apply.rs`, `feeds/profile.rs`, `session/mod.rs`, `compute/geometry_default.rs`, `rs_cam_viz/src/ui/properties/pills.rs`, `rs_cam_viz/src/state/runtime.rs`. |
| `RpmLimit` | `feeds/calculator/spindle.rs` | `CalcState` | Family Drill for the Z rate (`:162`); `is_axial_plunge` for the power limit (`:225`). |
| `force_budget::rigidity_depth_cap` | `feeds/force_budget.rs:497` | `(machine, family, pass_role, tool, material, ae, fz, depth)` | Yes, by argument, through `depth_cap_rule` (`:508`). |
| `RigidityProfile::depth_cap_rule` | `machine/mod.rs:293` | `(family, pass_role) -> Option<DepthCapRule>` | It IS the scope rule (`:299-304`). 6 product readers: `tool_load/depth.rs:240`, `tool_load/cut_force.rs:150`, `feeds/suggest/invariants.rs:225`, `feeds/envelope.rs:3177`, `feeds/force_budget.rs:508`, `session/compute.rs:2316`. |
| `cutter_axial_constraints` | `feeds/cutter_constraints.rs:208` | tool, material, vendor row, ae, fz, targets | No. A pure function of the cut. Called from `feeds/envelope.rs` (deflection edge) and `feeds/suggest/axial_envelope.rs` (4). |
| `depth_at_budget` | `feeds/force_budget.rs:351` | `(tool, material, ae, fz, budget_n) -> BudgetDepth` | No. Pure. |
| The modulator constraint set | `dressup/feed_modulation.rs:491` `ModulationContext` | rpm, Z, `cutting_ceiling`, `chipload_band`, `radial_thinning`, kinematics, `deflection_inputs`, `power_inputs`, `force_budget_inputs`, `sample_engagements`, `plunge_rate_mm_min` | The solver is op-blind. The SESSION fills it by op: `ForceBudgetScope::of(depth_cap_rule(family, role), ..)` (`session/compute.rs:2314-2317, 2360-2361`), `RadialThinning::of(family, role)` (`:2231-2234`), `planner_uniform_engagement` (`:2082`). |

The modulator computes seven candidate caps per move (`dressup/feed_modulation.rs:719-760`):
chip max, deflection, power, force budget, machine max feed, kinematic reach, then the chip-min
floor; after them the plunge-rate guard and `Move::feed_cap`. The deflection bound is always
`EXCEEDS_BOUND_MM` (200 um) (`session/compute.rs:2345`), for a rough pass too.

### 1.7 The pre-simulation path against the post-simulation path

Pre-sim (Suggest, the cards, the envelope):

1. `feeds::suggest_for_operation` (`feeds/suggest.rs:702`) builds `FeedsInput` through
   `feeds_input_for_operation` (`:770`). `FeedsInput` holds `operation: OperationFamily`,
   `pass_role`, `operation_kind`, ap, ae, the tool hint, the machine, the material.
2. `calculator::calculate_with` (`feeds/calculator.rs:249`) runs the steps: rpm, chip, engagement,
   flute guard, engagement floor, shank, feed, long tool, Step 5c RPM choice, Step 6 power ladder,
   Step 7 feed ceiling, Step 8 plunge, 9b rubbing floor, 9c drill.
3. `SafeEnvelope::for_cut` (`feeds/envelope.rs:1122`) adds the geometric limits, power, the vendor
   depth table, deflection, rigidity, the move budget and the per-point limits, and picks the
   proposal.
4. The cards read `CutterOpProfile::shipped_power` (`feeds/profile.rs:270`) =
   `power_at_shipped_point`, `cut_efficiency` (`feeds/efficiency.rs:164`),
   `predict_peak_deflection_um`.

Post-sim (the gates, per sample):

1. `tool_load::evaluate_toolpath` (`tool_load/mod.rs:636`): 2 call sites (itself and
   `session/compute/measured.rs`). It runs `chipload::evaluate`, `power::evaluate`,
   `deflection::evaluate`, `depth::evaluate`, `cut_force::evaluate`, and the drill gates.
2. `compute::simulate::envelope_placement` places the measured population on the pre-sim envelope
   (`compute/simulate.rs:1116`).
3. `stock::sim_triage` adds the entry load, the link load and the plunge-class findings.

Where the two paths share a function:

- `PowerTerms::of` / `kw_at_feed` (power).
- `Engagement::edge_force` (force) and `tip_deflection_from_engagement` (deflection).
- `RadialThinning::lift` (chip).
- `depth_cap_rule` and `rigidity_depth_cap` (the depth cap and the force budget).
- `Material::force_line`; the vendor lookup route `feeds::vendor_normalize::lut_query_for`
  (`tool_load/chipload.rs:155`).

Where they fork:

| Fork | Pre-sim | Post-sim |
|---|---|---|
| Engagement constructor | `from_cut`, `from_tool_cut` | `from_sample` (power, depth), `deflection::sample_engagement` (deflection, force) |
| Width | The stated width, then the worst of `PowerCases` (power only) | The measured arc, `ae = r (1 - cos psi)` |
| Feed | The op feed or the calculator feed; the steady feed on the card (P20e) | `effective_feed_for_sample` (`tool_load/mod.rs:76`): the modulated feed when the trace holds one |
| Which cuts | ONE cut: the `CutSpec` body, at up to 3 widths for power | Every sample that passes the span filter (and, for chip, the feed filter) |
| Deflection limit | 200 um (`DEFLECTION_BACKOFF_TARGET_UM`), roughing only (`feeds/envelope.rs:2890`) | 200 um, or the radial stock to leave of a rough pass (`tool_load/deflection.rs:138-151`) |
| Scope decision | `CutSpec` shape, `AxesPlan`, (family, role), row flags | `is_drill_kinematics()`, the span predicate, (family, role) through the registry spec |
| Record shape | `Limit` / `Edge` / `Placement`, `FeedsWarning` | Four verdict enums, `CriterionStatus` |

The modulator is a third path. It sits after the simulation, reads the gates' sample builders,
and uses a third record (`BindingConstraint`) and a population of its own (section 3.3, item M2).

---

## 2. The planned-cut table

25 operation kinds (`compute/catalog/registry.rs`). "Declared" means: reaches the feeds side
before the simulation, through `CutSpec`, `FirstPass`, `PowerCases`, `PlannerWidth`,
`UnliftedFeed::of_slot_ring`, or the plunge and ramp rules. Every op declares its entry KINDS
through `EntryContract`; no op declares an entry ENGAGEMENT, so "entry load" is in the
undeclared column of every milling row and is not repeated in full.

Shared facts for every milling op:

- The plunge feed and the ramp feed go through `VerticalRateLimit` (`feeds/vertical.rs:149`) and
  `resolve_ramp_feed` (`feeds/ramp.rs:691`). These bound a RATE (axial chip, Z axis rate). They
  bound no force, power or deflection.
- A dressup entry reads its geometry from `DressupConfig` (ramp angle, helix radius, pitch;
  `feeds/ramp.rs:318-327`). The role sets the default style: Roughing gives Ramp, SemiFinish and
  Finish give None (`entry_contract.rs:196-198`).
- A link is a `Linking` move. It gets `SpanKind::LinkBridge` (`compute/spans.rs:331`) and no gate
  reads it. `project.link_cuts_stock` reports it (`stock/sim_triage.rs:1649`).

| # | Op kind | Family, role as filed (`registry.rs`) | Cuts the generator makes | Declared today | Undeclared |
|---|---|---|---|---|---|
| 1 | Face | Pocket, Roughing (`:90-91`) | (D) Raster rows at the stepover, one ladder of levels over `depth` (`ops/face.rs:1-5`, `face_levels`). Row connectors. One entry per level. On a picked region the tool stays inside the outline "as in a pocket" (the doc of `generate_face`, `compute/execute/clearing_2d.rs:736-754`). | `Width(stepover)`, `PerPass`, `Known(depth)`, `FirstPass::NotSlot` (`compute/catalog.rs:1166-1171`). | The first row of a region Face (a slot; `NotSlot` is filed). Row connectors. |
| 2 | Pocket | Pocket, Roughing (`:144-145`) | (R) Contour pattern: offset rings from the compensated boundary inward; each ring is its own closed pass with a rapid, a plunge and a retract (`ops/pocket.rs:397-427`). The first ring of each piece is a full slot at the wall, fed at the unlifted feed with `feed_cap` (`:414-434`). Spring passes at the final Z (`finishing_passes`, `compute/catalog.rs:2300`). Zigzag pattern: the Zigzag kernel (`clearing_2d.rs:230-240`). | `Width`, `PerPass`, `FromHeights`, `FirstPass::Slot` (`:1172-1177`). The slot ring feed (`clearing_2d.rs:219-227`). | One plunge per ring. The Zigzag pattern under the same `CutSpec` (no slot ring feed: `feeds/geometry.rs:160-163`). Spring passes. The force and the deflection of the slot ring (only its power and its feed are declared). |
| 3 | Profile | Contour, Roughing (`:200-201`) | (R) One ring per level on the compensated offset: a full slot in fresh stock at every level. Tabs: a lift and a re-plunge at each tab (`clearing_2d.rs:698-738`, `dressup/tabs.rs`, 4 `EntryPlunge` writes). Spring passes (`catalog.rs:2303`). Open path on side `On`. Body intent `FinishingCut` (`ops/profile.rs`, 2 writes). | `Radial::Slot`, `PerPass`, `FromHeights`, `FirstPass::Slot` (`:1180-1185`). | The re-plunge after each tab. Spring passes. FLAG: the body is tagged `FinishingCut` on an op filed Roughing. |
| 4 | 2D Adaptive | Adaptive, Roughing (`:256-257`) | (R) Per level, per inset piece: adaptive passes held to the pass ceiling `c D` (`adaptive/CLAUDE.md` invariants). Optional slot lines: a sparse zigzag of full slots (`adaptive/path.rs:1019-1050`). Entry: dressup, default Helix (`registry.rs:266-271`), a helix bore with a flat lap (`adaptive/opening.rs:24-45`). "Exempt: opt-in slot lines, the steps out of a straight plunge" (`adaptive/CLAUDE.md`). Cleanup by `cleanup_strategy`: residue mop or contour-parallel residue (`adaptive/path.rs:1874, 2023`). Keep-down links inside the pass load. | `Width`, `PerPass`, `FirstPass::Slot` only when `slot_clearing` (`catalog.rs:1186-1195`). `PlannerWidth::PassCeiling` (`:1371`). | The helix bore. The steps out of a straight plunge (over the ceiling). The cleanup pass. The force budget and the width rule do not read this op at all (P28 rows 1, 5). |
| 5 | VCarve | Trace, Finish (`:320-321`) | (D) Scan-line zigzag rows across the outline at `stepover`; the depth at each point is the distance to the outline over `tan(half angle)`, capped by `max_depth` (`ops/vcarve.rs:1-11`). Straight plunge only (`registry.rs:336-339`). | `Radial::Groove`, `Surface{max_depth}`, `TotalDepth::None`, `NotSlot` (`:1196-1203`). | The row stepover (a config field, `VCarveConfig::stepover`; `Groove` gives the calculator no width). The first row (a full V slot at the full depth). FLAG: filed Finish; the body removes the whole V volume. |
| 6 | Inlay pocket | Trace, Finish (`:386-387`) | (D) The V walls only, in ONE pass to `pocket_depth` (`ops/inlay.rs:1-8`). The kernel also reads `floor_overlap` and a ring pitch from `floor_ridge` (`ops/inlay.rs:345-346`); those rings were not read. Straight plunge. | `Groove`, `OnePass(pocket_depth)`, `NotSlot` (`:1204-1209`). | The wall pass is a full V slot at the full pocket depth. The ring pitch. FLAG: filed Finish. |
| 7 | Inlay plug | Trace, Finish (`:450-451`) | (D) The mirrored plug surface `u(e) = S + e / tan a`, levels at `depth_per_pass`, an `outer_margin` cleared around the plug, a `stepover` field (`ops/inlay.rs:27-31`, `InlayPlugConfig`). NOT READ in the kernel. | `Groove`, `PerPass`, `NotSlot` (`:1210-1215`). | The area clearing around the plug with a V-bit (a rough cut by its work). FLAG: filed Finish. |
| 8 | Zigzag | Pocket, Roughing (`:514-515`) | (D) A wall inset and scan rows at the stepover, per level (`clearing_2d.rs:352-359`, `ops/zigzag.rs:1-5`). Row connectors. | `Width`, `PerPass`, `FromHeights`, `FirstPass::Slot` (`:1216-1221`). | Row connectors. No unlifted feed on the first row (the P23d rule is Pocket contour only). |
| 9 | Trace | Trace, Finish (`:570-571`) | (D) The path at each level of `depth_per_pass` to `depth`, on the line or beside it (`ops/trace_path.rs`). Every level is a full slot. Straight plunge at the op plunge rate (`registry.rs:578-581`). | `Radial::Slot`, `PerPass`, `Known/Unset`, `FirstPass::Slot` (`:1223-1228`). | Nothing in the body. FLAG: a full slot at a stated depth is filed Finish, so it reads no force budget and no pre-sim deflection (P28 row 3). |
| 10 | Drill | Drill, Roughing (`:631-632`) | (D) `Drilling` moves, the peck cycle (`ops/drill.rs`). The descent is the cut (`registry.rs:639`). | `Radial::Axial`, `Peck`, `FromHeights` (`:1229-1234`). Power case `AxialPlunge` (advisory). | None found. |
| 11 | Bore | Pocket, Roughing (`:707-708`) | (D) A feed out to `r - a` above the stock; a helix down in whole turns of `depth / n`; one floor turn; with `finish_allowance > 0` a feed out to `r`, a floor turn and a helix UP the wall; `ClearingCut` and `FinishingCut` intents, never `EntryHelix` (`ops/bore.rs:1-29`). | `Radial::Slot`, `HelixPitch`, `FromHeights`, `FirstPass::Slot` (`:1236-1241`). | The depth: `calc_inputs` gives no depth for a helix pitch (`cut_spec.rs:343-346`), so the calculator sizes the cut at the family default depth. The wall finish pass (width = the allowance, depth = the whole hole). |
| 12 | Chamfer | Trace, Finish (`:778-779`) | (D) Tip rings at one Z from the tool angle and `chamfer_width` (`ops/chamfer.rs:1-5`). Any dressup entry. | `Groove`, `ChamferWidth`, `TotalDepth::None`, `NotSlot` (`:1242-1247`). | The engaged cone section (width and depth both follow `chamfer_width`; the calculator reads neither). |
| 13 | Parallel finish (Drop Cutter) | Parallel, Finish (`:835-836`) | (D) Raster rows on the drop-cutter surface. Straight plunge only (`registry.rs:846-849`). | `Scallop` or `Width(stepover)`, `Surface{None}`, `NotSlot` (`:1248-1256`). | The depth: the stock the rough left. No field states it. |
| 14 | 3D Adaptive | Adaptive, Roughing (`:896-897`) | (R, partly D) Per Z level and region. Geodesic: a 2D geodesic plan per slice piece, draped; every fed step held to `c D`; a step over `c` up to a slot is cut at a capped feed, over a slot it is split (`adaptive3d/CLAUDE.md` P22f-2a; `FeedCapSource`, `adaptive3d/mod.rs:510-528`). Opening: the grown outline, else a full-slot perimeter sweep (`adaptive3d/clearing.rs:3045-3106`). Contour Parallel: rings with no width bound. Entries by the planner: plunge, peck ladder, ramp, helix (`registry.rs:909-916`; `adaptive3d/path.rs:177, 2322`), geometry in `Adaptive3dConfig`. A waterline cleanup after EVERY level: steep contours over 30 degrees (`adaptive3d/path.rs:1630-1645`, `clearing.rs:2027-2100`). The slice cleanup (residue mop) inside the level (`clearing.rs:3020-3040`). Stay-down links. Optional "Contour each level". | `Width`, `PerPass`, `FromHeights`, `FirstPass::NotSlot` (`:1257-1262`). `PlannerWidth::PassCeiling` (geodesic) or `CapMult` (contour parallel) (`:1361-1370`). `AxialEnvelope::Roughing3d` (`registry.rs:933`). | The perimeter sweep (a full slot; `NotSlot` is filed). The capped steps up to a slot. The waterline cleanup (the whole level step on a steep wall). The slice cleanup. Four entry kinds and the departure out of a plunge (`FeedCapSource::DepartureWindow`). The force budget and the width rule (P28 rows 1, 5). |
| 15 | Waterline | Contour, SemiFinish (`:963-964`) | (D) Constant-Z rings; the helical chain descends along the lower ring (`ops/CLAUDE.md`, `ops/waterline.rs`: 1 `EntryPlunge`, 4 `FinishingCut`, 1 `Linking`). | `SurfaceFollow`, `ZStep`, `FromHeights`, `NotSlot` (`:1263-1268`). | The radial width: the stock the rough left on the wall. The helical descent along the ring. |
| 16 | Pencil | Trace, Finish (`:1017-1018`) | (D) Crease chains; offset passes at `offset_stepover` when `num_offset_passes > 1`; its own crease ramp on rest stock (`registry.rs:1033`; `finish/pencil/emission.rs`: `EntryRamp`, `EntryApproach`, `EntryPlunge`). | `Width(offset_stepover)` or `SurfaceFollow`, `Surface{None}`, `NotSlot` (`:1271-1280`). | The first crease pass (a ball in a corner: engagement on both sides). The crease ramp. |
| 17 | Scallop | Scallop, Finish (`:1081-1082`) | (D) Rings or the iso-field at the scallop stepover. | `Scallop(height)`, `Surface{None}` (`:1281-1286`). | The depth (stock to leave). |
| 18 | Unified Finish | Scallop, Finish (`:1143-1144`) | (D) Bands: raster, waterline at `z_step`, pencil claims; planner entry, plunge only (`registry.rs:1163-1165`). `MoveCountModel::NotModelled` (`:1178`). | `Scallop(height)`, `ZStep`, `FromHeights` (`:1287-1292`). | Each band is a different cut (raster on shallow, waterline on steep); one `CutSpec` covers all. |
| 19 | Steep/Shallow | Contour, Finish (`:1215-1216`) | (D) Waterline on steep, raster on shallow. | `Width(stepover)`, `ZStep` (`:1293-1298`). | The same as row 18. |
| 20 | Ramp Finish | Parallel, Finish (`:1272-1273`) | (D) A ramping contour descent (`ramp_descent: true`, `registry.rs:1300`). | `SurfaceFollow`, `Surface{max_stepdown}` (`:1299-1306`). `AxialEnvelope::NotApplied` (`:1296`). | The radial width. |
| 21 | Spiral Finish | Scallop, Finish (`:1326-1327`) | (D) A spiral at the stepover. | `Width(stepover)`, `Surface{None}` (`:1307-1312`). | The depth. |
| 22 | Radial Finish | Parallel, Finish (`:1387-1388`) | (D) Spokes at `angular_step`. | `AngularStep`, `Surface{None}` (`:1313-1318`). | The width: `calc_inputs` gives none for an angular step (`cut_spec.rs:360`); the real width grows with the radius. |
| 23 | Horizontal Finish | Parallel, Finish (`:1441-1442`) | (D) Flat areas at the stepover. | `Width(stepover)`, `Surface{None}` (`:1319-1324`). | The depth. |
| 24 | Project Curve | Trace, Finish (`:1502-1503`) | (D) The curve at a signed offset below the model surface; chains linked (`ops/project_curve.rs:1-13`, `compute/execute/project_curve_chaining.rs`). Straight plunge only. | `Radial::Slot`, `SurfaceOffset`, `TotalDepth::None`, `FirstPass::Slot` (`:1328-1333`). | The depth into the stock: no pre-sim figure of any kind (P28 row 11). FLAG: a full slot filed Finish. |
| 25 | Pin Drill | Drill, Roughing (`:1565-1566`) | (D) As Drill, into the spoilboard. | `Radial::Axial`, `Peck`, `TotalDepth::None` (`:1334-1339`). | None found. |

Filing flags, in one list:

1. Trace, VCarve, Inlay pocket, Inlay plug, Chamfer and Project Curve are filed (Trace, Finish).
   Trace and Project Curve state `Radial::Slot` and `FirstPass::Slot`. P28 is correct.
2. Profile is filed (Contour, Roughing) and emits `FinishingCut` moves.
3. Face states `FirstPass::NotSlot`; a region Face cuts inside an outline.
4. 3D Adaptive states `FirstPass::NotSlot`; its opening can be a full-slot perimeter sweep.
5. Bore is filed (Pocket, Roughing) with `Radial::Slot`; the calculator reads no depth for it.
6. Pocket with the Zigzag pattern keeps the `CutSpec` of the contour pattern.

---

## 3. The limit list

### 3.1 Every limit, as a signature

"Op read" = every place where the limit or its caller reads the op family, the pass role, the
strategy, the op kind or the span kind.

| # | Limit or check | Side | Function | Inputs | Op read | Today / what it needs |
|---|---|---|---|---|---|---|
| L1 | Spindle power, the cut | pre | Step 6 ladder, `feeds/calculator/power.rs:60` | ap, ae, d_eng, rpm, feed, line, `PowerCases` | `is_axial_plunge()` (`:112`); `power_force_line()` = `depth_after_simulation` (`feeds/mod.rs:588-601`); the cases come from `FirstPass` and `PlannerWidth`. | Needs the LIST of cuts (it has widths only). N/A for an axial plunge only by a missing end-cut force source, not by geometry. |
| L2 | Spindle power, the envelope edge | pre | `power_limit`, `feeds/envelope.rs:2642` | grid (ap, ae), chip | `plan.axes.ap_free()` (`:2649`); `!f.section.gates()` (`:876`). | Pure function of the cut once the cut has a depth. |
| L3 | Power at rpm (RPM choice) | pre | `power_limit_rpm`, `feeds/calculator/spindle.rs:223` | chip per rev, rpm range | `is_axial_plunge()` (`:225`). | As L1. |
| L4 | Spindle power, the gate | post | `power::evaluate`, `tool_load/power.rs:286`; `gate_sample_power` (`:265`) | sample engagement, feed, rpm, line, machine curve | `is_drill_kinematics()` (`:273, 315`); span predicate (`:278, 435, 438`). | Pure per sample. The span filter is the only scope. |
| L5 | Force budget as a depth | pre | `rigidity_limit`, `feeds/envelope.rs:3168`; `rigidity_cap_mm`, `feeds/suggest/invariants.rs:214` | ae, fz, budget, tool, material | `depth_cap_rule(family, role)` (`:3177`, `invariants.rs:225`); `Roughing` only (`:3178`, `invariants.rs:221`). | `depth_at_budget` is pure. Needs nothing new on the cut. The scope is the whole exemption. |
| L6 | Force budget as a depth | post | `depth::evaluate`, `tool_load/depth.rs:111` | peak sample: ae, fz, ap | `is_drill_kinematics()` (`:131`); `depth_cap_rule(spec.feeds_family, spec.feeds_pass_role)` (`:240, 279-282`); span predicate (`:186`). | Judges ONE sample (the peak depth), not every sample. Non-gating (`BoundSource::MachineForceBudget`). |
| L7 | Force budget as a feed | post | modulator, `ForceBudgetScope::of`, `dressup/feed_modulation.rs:447` | engagement, line, budget | `depth_cap_rule(family, role)` at `session/compute.rs:2314-2317`. | Pure in the solver. |
| L8 | Cut force reading | post | `cut_force::evaluate`, `tool_load/cut_force.rs:178`; `budget_line` (`:145`) | sample engagement, fz | `is_drill_kinematics()` (`:180`); `depth_cap_rule` (`:147-150`); span predicate (`:124`). | Report-only. Pure per sample. |
| L9 | Tool deflection | pre | `deflection_limit`, `feeds/envelope.rs:2882` | ae, fz, tool, material | `ap_free()` and `Roughing` only (`:2890`). | `cutter_axial_constraints` is pure. A finish cut needs a depth on the cut (today `Surface{None}`). |
| L10 | Tool deflection, the predictor | pre | `predict_peak_deflection_um`, `feeds/predict.rs:301` | op, tool, material, machine | Drill (`:313`); `depth_after_simulation` (`:350`); Adaptive default width (`:373`). | Needs a depth and a width on the cut. |
| L11 | Tool deflection, the gate | post | `deflection::evaluate`, `tool_load/deflection.rs:248` | sample engagement, fz, tool, material | `is_drill_kinematics()` (`:273`); `exceeds_limit(pass_role, leave)` (`:138-151, 268`); span predicate (`:371, 374`). | Pure per sample. The LIMIT reads the role (two limits for one cut, P28 row 16). |
| L12 | Chip maximum (band) | pre | Step 5 feed; `per_point_limits`, `feeds/envelope.rs:3529` | chip, band, lift | `RadialThinning::of(family, role)` (`feeds/geometry.rs:59-66`); Drill in Step 7 (`feeds/calculator/feed.rs:232`). | Pure once the cut says whether it thins (today a role rule). |
| L13 | Chip maximum and minimum, the gate | post | `chipload::evaluate`, `tool_load/chipload.rs:360` | sample advance, measured arc, band | `is_drill_kinematics()` (`:389`); `lut_query_for(operation_kind, ..)` (`:155`); `RadialThinning::of_lut_role(pass_role)` (`:606`); span predicate (`:458, 750, 753`); THE FEED FILTER: only samples at 95 % or more of the steady feed (`:197, 225-245, 418`). | Needs the cut kind on the sample in place of the feed filter and the role. |
| L14 | Rubbing floor (chip minimum) | pre | Step 9b, `feeds/calculator/close.rs`; `per_point_limits` (`envelope.rs:3542-3565`) | chip, band minimum | None found. | Pure. It warns, never lifts. |
| L15 | Vendor depth table (1, 2, 3 x D) | pre | `vendor_depth_limit`, `feeds/envelope.rs:2753` | ae, fz, band maximum, D | `ap_free()` and `!plan.peck` (`:2761`). | Pure function of the cut. N/A for a peck by geometry. |
| L16 | Depth beyond the table (above 3 x D) | pre | `geometry::depth_beyond_published_table`, `feeds/geometry.rs:349`; finding `feeds.depth_beyond_published_table` | ap / D | NOT READ at the caller. | Pure by signature. |
| L17 | Vendor axial bound | pre | `geometric_limits`, `feeds/envelope.rs:2501-2513`; `axial_envelope_for_operation`, `feeds/suggest/axial_envelope.rs:34` | vendor row, tool, material, ae | `capabilities().axial_envelope` (`axial_envelope.rs:59`); 5 classes (`capabilities.rs:672-685`). | The row flag is the scope. The bound itself reads the row and the tool. |
| L18 | Flute guard, cutting length | pre | Step 4, `feeds/calculator/cut.rs:86`; `geometric_limits` (`envelope.rs:2478-2498`) | ap, flute length | `depth_after_simulation` (`cut.rs:100`). | Pure once the cut has a depth. |
| L19 | Flute reach on the live stock | post | `compute::simulate::flute_reach`; findings `tool.flute_overreach`, `tool.reach_past_flute` | sample depth, tool body | NOT READ. P28 says the zone checks hold no op exemption. | Not verified here. |
| L20 | Width rule (most removal inside the limits) | pre | `propose_removal_rate`, `feeds/envelope.rs:2113` | every limit above, at each width | `computed_width` = `family == Pocket && role == Roughing` (`:2461-2464`); `removal_rate` also for a non-Adaptive roughing `AxialBar` (`:1287-1292`: Profile, Bore); `has_computed_width` (`feeds/suggest.rs:751-754`); `feeds/mod.rs:2720`. | Needs a width ceiling for a cut that is not an offset ring (no source in the repo, P26 section 3). |
| L21 | Corner coverage (0.8536 D) | pre | `geometric_limits`, `feeds/envelope.rs:2516-2533` | D | `plan.computed_width` (`:2516`). | Physically N/A outside offset rings. Needs "offset ring" on the cut, not the family. |
| L22 | Pass ceiling `c D` | planner | `PlannerWidth::pass_ceiling_mm`, `cut_spec.rs:221` | stepover, D | `planner_width()` match on op kind and `ClearingStrategy` (`compute/catalog.rs:1359-1396`). | A planner fact, already a function of the stepover and D. |
| L23 | Move budget (width floor) | pre | `runtime_limit`, `feeds/envelope.rs:3378` | model box, stepover, depth | `ae_free()` and `stepover().is_some()` (`:3385`); `capabilities().move_count` (`:3397`; `feeds/predict.rs:733-813`). | Not a load limit. A runtime limit of the plan. |
| L24 | Tool diameter, engagement floors | pre | `geometric_limits` (`:2535-2552`), `diameter_edge` (`:2561`) | D, d_eng(ap) | None. | Pure. |
| L25 | Machine cutting-feed cap | pre | Step 7, `feeds/calculator/feed.rs:229-233` | feed, machine ceiling | Family Drill (`:232`). | Pure for a side cut. A drill takes Step 9c. |
| L26 | Machine cutting-feed cap, kinematic reach | post | modulator caps 5 and 6 (`dressup/feed_modulation.rs`) | feed, kinematics | Move intent skip (`:680-702`). | Pure per move. |
| L27 | Plunge rate (vertical rate limit) | pre | `VerticalRateLimit::bound`, `feeds/vertical.rs:196`; `plunge_basis`, `feeds/plunge.rs:424` | candidate, rpm, axial chip, Z axis rate | Family Drill (`feeds/plunge.rs:430`). | A rate limit only. Needs an axial engagement to become a load limit. |
| L28 | Ramp feed | pre | `resolve_ramp_feed`, `feeds/ramp.rs:691` | feed, chip, rpm, Z, tan(theta) | `OperationConfig::Adaptive3d` (`:340, 985`); Drill (`:393`); home row (Pocket, Roughing) (`:422-423`); `Roughing` (`:947`). | A rate limit only. |
| L29 | Tip plunge cap (150 mm/min per mm of tip) | pre | `safe_plunge_cap_mm_min`, `tool_load/plunge_stress.rs:45`; `check_plunge_stress` (`:64`) | tool shape, D, plunge rate | Tool shape only (`:30-41`). | Pure function of the tool. Unsourced. |
| L30 | Plunge-entry stability (depth over 0.5 D) | pre | `check_plunge_entry_stability`, `feeds/suggest/adaptive_entry.rs:110` | depth per pass, D, entry style | `Roughing` and family Adaptive (`:117-118`); `OperationConfig::Adaptive3d` (`:634`). | Needs the entry as a cut. |
| L31 | Geometric plunge guard | post | modulator, `BindingConstraint::PlungeRate` (`tool_load/verdict.rs:65-80`) | move vector, op plunge rate | None (it reads the move vector). | Pure per move. |
| L32 | Entry load (peak bite over 2 x the median body bite) | post | `stock/sim_triage.rs:1293-1410` | entry samples, body samples | `source_intent` entry intents (`:1402-1405`). | A relative rule. It needs the entry as a cut with an engagement to become a limit. |
| L33 | Link cuts stock | post | `stock/sim_triage.rs:1649` | link samples | `source_intent == Linking`. | The same. |
| L34 | Core and centre cutting | pre and post | `compute/feasibility.rs` (K5 `physical_rules!`), `compute::simulate::core_contact`; findings `safety.non_centre_cutting_plunge`, `safety.helix_inside_core` | tool body core, descent slope | `EntryContract::` (2 reads in `feasibility.rs`). | NOT READ in detail. The rules live on the tool body (`tool::body::CoreZone`). |
| L35 | Z axis rate on the RPM choice | pre | `feeds/calculator/spindle.rs:159-170` | Z rate, chip per rev | Family Drill (`:162`). | Physically for a vertical feed only. |
| L36 | Drill gates (chip welding, peck, plunge feed) | post | `drill_gates::evaluate`, `tool_load/drill_gates.rs:151` | `DrillOp`, summary | `drill_op` is `Some` for the two drill kinds (`tool_load/mod.rs:454-461`). | Their own geometry. |
| L37 | Peck rule | pre | `drill_peck_rule`, `feeds/envelope.rs:2320` | hole, tool | Exhaustive match on the op kind (`:2327-2363`). | Drill geometry. |
| L38 | Default cut when the operator states none | pre | `operation_default_profile`, `feeds/mod.rs:2306-2412`; `:2793-2830, 2910-2957` | family, role | The whole table is (family, role). | Not a limit: a default. It decides what the limits then judge. |
| L39 | Envelope placement of the simulation | post | `compute::simulate::envelope_placement`, `compute/simulate.rs:1116` | gate population, pre-sim envelope | Span predicate (`:1116`). | Follows L4. |

### 3.2 The 34 rows of P28, checked at HEAD

"OK" = the read is in the code at HEAD at the cited place (a shift of a few lines is not listed
as an error).

| Row | Check | Note |
|---|---|---|
| 1 | OK | `machine/mod.rs:301`. |
| 2 | OK | `machine/mod.rs:303`. The `Reported` arm for "no rule" is below `tool_load/depth.rs:289`; the cited `:288` is the end of the cap call. |
| 3 | OK | Registry lines `:570, 320, 386, 450, 778, 1502` hold `feeds_family: Trace`; the roles at `:571, 321, 387, 451, 779, 1503` are `Finish`. |
| 4 | OK | `machine/mod.rs:300`. |
| 5 | OK | `feeds/envelope.rs:2461-2464, 1287-1292`; `feeds/suggest.rs:751-754`. |
| 6 | OK | Same lines. |
| 7 | OK | `feeds/envelope.rs:2516`. |
| 8 | OK | `feeds/mod.rs:2793`; `compute/validate.rs:283`. The validate check also needs a flat tool (`:280`). |
| 9 | OK | `feeds/envelope.rs:3385`; `capabilities.rs:622-628`. |
| 10 | OK | `feeds/calculator/power.rs:110-112`, `spindle.rs:225`, `close.rs:356-358`. |
| 11 | OK | `cut_spec.rs:165`; `cut.rs:68, 100`; `envelope.rs:1279`; `operating_point.rs:262`. Also `:353, 407`, `predict.rs:350`, `calculator/speed.rs:71`, `close.rs:402`. |
| 12 | OK | `compute/catalog.rs:1170` (Face), `:1190-1194` (2D Adaptive), `:1261` (3D Adaptive). |
| 13 | OK | `compute/catalog.rs:1365-1369`. |
| 14 | OK | Same arm; `PlannerWidth::CapMult` gives no full-feed width (`cut_spec.rs:235`). |
| 15 | OK | `feeds/envelope.rs:2890`. |
| 16 | OK | `tool_load/deflection.rs:138-151`; `session/compute.rs:2345`. |
| 17 | OK | `tool_load/deflection.rs:273`; `feeds/predict.rs:313`. |
| 18 | OK | `feeds/geometry.rs:62`; `feeds/calculator/feed.rs:88`. |
| 19 | OK | `feeds/calculator/feed.rs:96-97`. |
| 20 | OK | `tool_load/chipload.rs:389`. |
| 21 | OK, incomplete | `tool_load/locality.rs:156-161`. See M1 and M2 below. |
| 22 | OK | `tool_load/locality.rs:124-129, 208`. The span covers every move from the cleanup marker to the next depth-pass start (`compute/spans.rs:588-605`). The cleanup runs after every level (`adaptive3d/path.rs:1630-1645`). |
| 23 | OK, incomplete | `tool_load/locality.rs:194-214`. `LeadIn` moves are `SpanKind::Entry` (`compute/spans.rs:338`), so a lead-in is a "configured entry", not a phantom. |
| 24 | OK | `feeds/envelope.rs:2761`. |
| 25 | OK | `feeds/envelope.rs:2501-2513`; `capabilities.rs:672-675`. |
| 26 | OK | `feeds/calculator/feed.rs:232`. |
| 27 | OK | `tool_load/plunge_stress.rs:30-48`. |
| 28 | OK | `feeds/suggest/adaptive_entry.rs:117-118`. |
| 29 | OK | `dressup/feed_modulation.rs:680-702`. |
| 30 | OK | Same lines. The skip list holds `EntryPlunge` and `EntryApproach`. It does NOT hold `EntryRamp` or `EntryHelix`. |
| 31 | OK | `feeds/mod.rs:2308-2409`. |
| 32 | OK | `cut_spec.rs:343-346`. |
| 33 | Partly | `feeds/predict.rs:182` holds the caveat type. `tool_load/chipload.rs:885` at HEAD is a general refusal (no rpm x flute divisor), not a V-bit rule. The V-bit finding id is `tool.v_tip_unmodelled`. Not traced further. |
| 34 | OK | `dressup/feed_modulation.rs:442-466`; `tool_load/cut_force.rs:145-154`; `tool_load/depth.rs:240`. |

No P28 row was found wrong. Rows 21, 23 and 33 need the notes above.

### 3.3 What P28 missed

| # | Finding | Where | Class (P28 terms) |
|---|---|---|---|
| M1 | The chip gate has a FEED filter beside the span filter. It reads only samples whose feed is at least 95 % of the op's steady feed. A Pocket slot ring at the unlifted feed (P23d), a planner-capped step (`Move::feed_cap`), a lead at its own feed and a ramp at the ramp feed are below it when their feed is under 0.95 x the steady feed. The filter reads `s.feed_rate_mm_min`, the feed of the simulation run. Power, deflection, depth and force have no such filter. | `tool_load/chipload.rs:197, 225-245, 414-418` | SCOPE. Not verified by a run: which share of a real op falls out. |
| M2 | The modulator population is not the gate population. The gates drop entry samples and phantom samples. The modulator drops phantom samples only: "configured-entry samples bind" (ruling 3). So `EntryRamp` and `EntryHelix` moves take the power, deflection and force-budget caps in the modulator, and no gate judges them. A phantom move (waterline cleanup) is still modulated, on its MEAN engagement. | `session/compute.rs:2262-2281`; `dressup/feed_modulation.rs:277-290, 680-702` | SCOPE |
| M3 | The gates route entry samples to an `entry_spike` advisory on the verdict (`Within` only). P28 row 21 names only `project.entry_load`. | `tool_load/chipload.rs:753, 1016-1037`; `power.rs:438, 551`; `deflection.rs:374, 468`; `verdict.rs:1665` | - |
| M4 | One sample has two engagements: `axial_doc_mm` for power and depth, `axial_engagement_mm` for deflection and force. | `feeds/engagement.rs:235`; `tool_load/deflection.rs:165-181` | UNKNOWN (stated in a doc, no reason given) |
| M5 | The depth gate judges ONE sample (the peak depth) against the force budget at that sample's width and chip. A shallower sample with a wider arc is not judged by it. The force card reads every sample but is report-only. | `tool_load/depth.rs:172-190, 222-288`; `tool_load/cut_force.rs:178` | SCOPE |
| M6 | The vendor chip band is chosen by the op kind: 3D Adaptive reads the Pocket rows, Project Curve reads a row by tool shape. | `feeds/vendor_normalize.rs:85-119`; `tool_load/chipload.rs:155` | SOURCED RULE or UNKNOWN (not traced) |
| M7 | The removal-rate proposal excludes the Adaptive family by name even on an `AxialBar`. | `feeds/envelope.rs:1291` | SCOPE (part of row 5) |
| M8 | The two triage readers call the span predicate with no span lookup, so every in-transit sample is dropped, entries included. | `stock/sim_triage.rs:944, 1713`; `tool_load/locality.rs:219` | UNKNOWN |
| M9 | The Z axis rate enters the RPM choice for the Drill family only. | `feeds/calculator/spindle.rs:162` | PHYSICAL (a side op takes it at the plunge and the ramp) |
| M10 | The modulator's planner-engagement floor reads the op strategy (`planner_uniform_engagement`) and the move intent (`ClearingCut`, `FinishingCut` only). | `session/compute.rs:2082-2092, 2183-2197` | SCOPE |
| M11 | The `FirstPass` and `Radial` filings in section 2 (Face region, 3D Adaptive sweep, Bore depth, VCarve stepover, Inlay one pass) remove a cut from the power case set. P28 row 12 covers the power case only for three ops. | `compute/catalog.rs:1166-1262` | UNKNOWN |
| M12 | The pre-sim deflection limit and the force budget read no `FirstPass` and no `PlannerWidth`. Only power sizes the slot or the planner cap. A Pocket slot ring is sized for power and for feed, not for force or deflection. | `feeds/engagement.rs:868-871` (one reader); `feeds/envelope.rs:2882, 3168` | SCOPE |

### 3.4 Sorted by "what the limit needs"

Pure function of (cut, tool, stock, machine) today, behind a scope read that is the only
op-type dependence: L2, L4, L5, L7, L8, L9, L11 (the value), L14, L15, L18, L24, L26, L29, L31.

Needs a new field on the cut:

| Field | Limits that need it |
|---|---|
| A depth where the op states none (`Surface{None}`, `SurfaceOffset`, `HelixPitch`, `ChamferWidth`) | L1, L5, L9, L10, L18 for the finishes, Project Curve, Bore, Chamfer |
| A width where the op states none (`Groove`, `SurfaceFollow`, `AngularStep`) | L1, L5, L9, L12 |
| The cut kind (body, slot or first pass, entry, cleanup, sweep, spring pass, link) | L13 (in place of the feed filter), every gate (in place of the span filter), L32, L33 |
| Whether the cut thins its chip (in place of the role) | L12, L13 |
| The deflection limit basis (stock to leave after this cut) | L9, L11 (one limit, in place of the role) |
| "Offset ring" (in place of the Pocket family) | L21 |
| An axial or inclined engagement (plunge, ramp, helix) | L27, L28, L30, L32: no section exists for a ramp or a helix, and none for a plunge of a ball, a bull nose, a V-bit or a taper (`feeds/section.rs:213-227`) |
| A width ceiling for a non-ring rough cut | L20 (no printed source, P26 section 3) |

Physically not applicable by geometry: L21 outside offset rings; L15 and L18 for a peck; the
radial limits (L9, L11, L12 thinning, L20) for a straight plunge (`Radial::Axial`); L35 for a
cut with no vertical feed.

---

## 4. The 5 facts that most constrain a design

1. The engagement type exists and is closed (`feeds/engagement.rs:166`, private fields, 24
   constructor call sites). It is geometry only. Feed, rpm, flute count, tool body and material
   travel beside it as loose arguments, so each of about 10 wrappers assembles force, power,
   deflection and chip by hand.
2. An op declares ONE cut (`CutSpec`, built in one exhaustive match,
   `compute/catalog.rs:1157`). The only extra cuts that reach the feeds side are widths for the
   power case (`FirstPass`, `PlannerWidth`: one reader each) and the Pocket slot ring feed (one
   op, one pattern). The compile-time net for "every op states it" already exists three times:
   the registry row, `CutRule`, `EntryContract`.
3. The planners already know their extra cuts and name them: `FeedCapSource` (6 kinds),
   `EntryRecord` (4 kinds), the `WaterlineCleanup` marker, the slot-first ring index, the 2D
   bore. None of these reaches a limit. They reach feed stages (`Move::feed_cap`) or the trace.
4. Before the simulation, many cuts have no number to judge: 17 of 25 ops give the calculator no
   depth, no width, or neither (`cut_spec.rs:335-371` applied to the arms of
   `compute/catalog.rs:1166-1339`; the two drills are in the 17), and no engaged section exists for a
   ramp, a helix, or a plunge of any tool but a flat end mill (`feeds/section.rs:213-227`). "Every
   cut through every limit" needs these models, or a "no number" basis on the record.
5. After the simulation, a sample has no cut kind. The scope comes from span ancestry
   (`tool_load/locality.rs:156`), from the op kind (`is_drill_kinematics`), from (family, role)
   through the registry spec, and for the chip gate from the feed (95 % rule). There are six
   record shapes for a value against a limit, and two provenance types (`Basis`, `BoundSource`).

## 5. What surprised me

- The chip gate's 95 % feed filter (M1). It is a second scope rule that no P28 row names, and
  the P23d slot ring feed can put the slot ring outside the chip gate.
- The modulator limits `EntryRamp` and `EntryHelix` moves by power, deflection and force budget,
  and no gate then judges those moves (M2). The cap and the check have different populations.
- One sample builds two engagements with two depths (M4).
- The 3D rough's waterline cleanup runs after every level, and its span runs to the next
  depth-pass start. Every move in it is outside every gate.
- A Trace or a Project Curve states `Radial::Slot` and `FirstPass::Slot` in its `CutSpec`, and
  the same registry row files it as Finish. The cut and the label disagree inside one row.
- Bore states a slot, and the calculator sizes it at the family default depth, because a helix
  pitch is "not a side-cut depth" (`cut_spec.rs:326-330`).
- `EntryRecord` is already one object per entry cut, built for every op after generation. Only
  one call site reads it.

## 6. Not read, or not verified

- The finish generators (rows 13, 15 to 23 of section 2), Inlay plug, and the Zigzag, Face and
  Trace kernels: read from the module doc, the folder file, the registry and the config only.
- `every_op_conforms_w4`: not read.
- The zone checks (core, shank, holder, shoe, flute reach): not read; P28's statement stands
  unchecked.
- M1: the share of samples the feed filter removes on a real op needs a probe.
- No cargo command ran. Every claim is from the source text at `484542abe`.
