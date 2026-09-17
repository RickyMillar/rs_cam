# Lane C verification — findings 5, 8, 9

**Method.** I READ the tree at `62665c62` (98 commits after the audit's
`5087490f`). I ran no `cargo` command. Nothing below is an execution result.
Every claim carries a `path:line` from today's tree.

**Contract reminder.** `None` means NOT MEASURED, never clean.

## Verdicts

| # | Subject | Verdict |
|---|---|---|
| 5 | Timing corrections update selected summaries | **PARTLY CLOSED** — and the fix opened a NEW defect the audit did not have (preconditions in §5) |
| 8 | Drilling reconstructs its analytical model | **STILL TRUE**, but the audit's verb is wrong |
| 9 | Shared run emission bypassed for annotations | **STILL TRUE**, and wider than the audit says |

---

## Finding 5 [High] — Timing corrections update selected summaries

**Verdict: PARTLY CLOSED.**

### Lead with this: the retime path drops a drill's runtime, today, by default

This is the most important thing in this lane. The audit named the shape
("initial integration tracks all toolpaths … post-modulation retiming
iterates the engagement-summary population"). I read the code and the shape
has a live consequence the audit did not state.

**Symptom 1 — G-DRILLTIME is fixed on the integrator path and NOT on the
retime path.**

The integrator walks every toolpath:

- `crates/rs_cam_core/src/compute/simulate.rs:1464` iterates
  `request.groups` and inserts a breakdown for every entry.
- `crates/rs_cam_core/src/compute/simulate.rs:1501` writes
  `trace.toolpath_runtimes` for all of them.
- `crates/rs_cam_core/src/compute/simulate.rs:1523-1533` folds the project
  total over the INTEGRATED set, then adds any summary the integrator missed.

The retime does not:

- `crates/rs_cam_core/src/session/compute.rs:3183` resets
  `let mut project_total = 0.0;`.
- `crates/rs_cam_core/src/session/compute.rs:3199`
  `for tp_summary in &mut trace.toolpath_summaries {` — the engagement
  population only.
- `crates/rs_cam_core/src/session/compute.rs:3257`
  `trace.summary.total_runtime_s = project_total;`.

A drill toolpath has no `toolpath_summaries` row. The analytic branch at
`crates/rs_cam_core/src/compute/simulate.rs:1058-1081` applies
`group_stock.apply_drill_op(...)` and emits `DrillSample`s. It produces no
`SimulationCutSample`. The summary list is built from samples only
(`crates/rs_cam_core/src/simulation_cut.rs:1121`). The code says so itself at
`crates/rs_cam_core/src/compute/simulate.rs:1489-1495`.

So under modulation the drill's integrated seconds leave
`trace.summary.total_runtime_s` and `trace.summary.runtime_by_intent`. That is
the exact defect `drill_cycle_time_integration_g_drilltime.rs` closed on the
other path.

**Symptom 2 — `toolpath_runtimes` is never rewritten, so two published slots
disagree.**

`crates/rs_cam_core/src/compute/simulate.rs:1501` is the only writer that
POPULATES `trace.toolpath_runtimes` (I grepped `toolpath_runtimes` across
`crates/`; `simulation_cut.rs:979` and `simulation_cut.rs:1181` write
`Vec::new()`). The retime block rewrites
`toolpath_summaries[].total_runtime_s` and `runtime_by_intent`, and leaves
`toolpath_runtimes` at its pre-modulation value.

`crates/rs_cam_viz/src/ui/readiness.rs:499` reads `toolpath_runtimes` FIRST:

```
if let Some(rt) = trace.and_then(|t| t.toolpath_runtimes.iter().find(|r| r.toolpath_id == id)) {
    return CycleTime::of(rt.breakdown.total_s, CycleTimeBasis::MachineModel);
}
```

That function is the single funnel — *"Every operator-facing surface routes
through here"* (`crates/rs_cam_viz/src/ui/readiness.rs:469-470`) — and the
project fold over it feeds *"the readiness panel, the pre-flight gate, the
export wizard's Save step and the printed setup sheet"*
(`crates/rs_cam_viz/src/ui/readiness.rs:530-533`). So after a modulated
simulation those four surfaces read the PLANNED clock while
`ProjectDiagnostics.total_runtime_s` reads the EMITTED clock.

**Symptom 3 — the G-AIRDENOM sentry cannot see symptom 1 or 2.**

`crates/rs_cam_core/tests/air_cut_one_time_base_g_airdenom.rs` asserts
`cutting + rapid == total` over the summary population. Both symptoms live
outside that population. Neither `drill_cycle_time_integration_g_drilltime.rs`
nor `crates/rs_cam_viz/tests/cycle_time_basis_g_timeest.rs` mentions
modulation (I grepped `modulat` in both; no match).

**Symptom 4 — the two writers read two different kinematics sources.**

The integrator runs only when the machine profile carries an EXPLICIT
kinematics block. `session/compute.rs:938` builds the request field as
`kinematics: self.machine.kinematics.map(|kin| ...)` — the raw `Option`, not
the fallback. The GUI worker does the same at
`crates/rs_cam_viz/src/compute/worker/execute/mod.rs:377-383`, whose comment
states *"When kinematics is `None` (the default for every shipped preset),
this stays byte-identical to pre-F-034"*. The gate is
`crates/rs_cam_core/src/compute/simulate.rs:1386`
`if let Some(ctx) = request.kinematics`.

The retime runs on every profile. `session/compute.rs:3055` uses
`self.machine.effective_kinematics()` — the generic-wood-router fallback,
which never returns `None`.

So on a shipped preset the retime overwrites every summary's
`total_runtime_s` with a fallback-kinematics integration and stamps
`runtime_by_intent: Some(b)` (`session/compute.rs:3216`). `toolpath_runtimes`
stays empty, readiness falls to the summary arm
(`crates/rs_cam_viz/src/ui/readiness.rs:502-515`), and that arm reports basis
`MachineModel` on the strength of a kinematics block the operator never set.
I state this as read, not as a defect ruling: the retime's own comment
(`session/compute.rs:3052-3054`) says the fallback is deliberate for
modulation. The point for the refactor is that TWO writers of one field
disagree about which source is authoritative.

### Preconditions, stated exactly

| Symptom | Fires when |
|---|---|
| 1 — drill dropped from the project total | modulation runs AND the profile carries explicit `kinematics`. Without the integrator the drill was never in the project total either (it has no samples), so the retime does not make it worse. |
| 2 — `toolpath_runtimes` stale | modulation runs AND the profile carries explicit `kinematics`. With `kinematics: None` the list is empty and readiness never reads it. |
| 4 — two kinematics sources | modulation runs. Visible on EVERY profile, and most visible on a preset. |

**Modulation itself is on by default.** `SimulationOptions::default()` sets
`adaptive_feed_modulation: true` at
`crates/rs_cam_core/src/session/mod.rs:1002` (operator ruling J-3,
2026-08-13). Its own precondition is one milling operation with a chipload
band: `crates/rs_cam_core/src/session/compute.rs:3056-3059` returns early when
`envelopes.is_empty()`. A drill-only project is therefore unaffected. A mixed
project is not.

**No sentry covers the retime path with a drill in the fixture.**
`crates/rs_cam_core/tests/adaptive_feed_modulation_pipeline_f036b.rs` is the
sentry that DOES exercise the retime. I grepped `drill` in it: the only hits
are `drill_targets: std::sync::Arc::new(Vec::new())` at line 134 and a comment
at line 522. Its fixture holds no drill operation.

I did not measure the size of any error. That needs a run.

### The rest of the finding: what is closed and what stands

**Closed.** The per-toolpath and per-project cutting slices ARE on one clock.
`rebase_cutting_times` (`crates/rs_cam_core/src/simulation_cut.rs:763`) moves
each cutting sample by its own `commanded ÷ modulated` ratio, then applies one
acceleration factor. The caller
(`crates/rs_cam_core/src/session/compute.rs:3232-3255`) writes
`cutting_runtime_s`, `air_cut_time_s`, `low_engagement_time_s`,
`rapid_runtime_s` and `average_mrr_mm3_s`. The function's own doc states the
invariant: *"The result satisfies `cutting_runtime_s + rapid_runtime_s ==
breakdown.total_s` exactly"* (`simulation_cut.rs:739-741`).

**Still open, exactly as `CLAUDE.md` records.** The source names the excluded
populations at `crates/rs_cam_core/src/simulation_cut.rs:744-751`:

> `average_engagement` stays the time-weighted mean over the COMMANDED
> clock. … `KinematicsSummary::cutting_runtime_s`, the
> `SimulationSemanticCutSummary` rows and `SimulationCutHotspot` keep the
> naive clock too, so the per-class times no longer sum to the toolpath's.

The `AirCutRatios` trait is implemented for five types
(`crates/rs_cam_core/src/simulation_cut.rs:678-685`):
`SimulationCutSummary`, `SimulationToolpathCutSummary`,
`SimulationSemanticCutSummary`, `SimulationCutHotspot`,
`SummaryAccumulator`. Two of those five carry a mixed base and the trait
cannot tell a caller which. The doc warns in prose
(`crates/rs_cam_core/src/simulation_cut.rs:646-650`) and nothing enforces it.

### Boundary, stated exactly

| Quantity | Clock after modulation |
|---|---|
| `SimulationToolpathCutSummary::total_runtime_s` | integrator, modulated |
| `SimulationToolpathCutSummary::{cutting,rapid,air_cut,low_engagement}` | integrator, modulated (rebased) |
| `SimulationToolpathCutSummary::average_mrr_mm3_s` | integrator, modulated |
| `SimulationCutSummary` (project) | integrator, modulated — **minus every drill** |
| `SimulationToolpathCutSummary::average_engagement` | commanded, naive (deliberate) |
| `KinematicsSummary::cutting_runtime_s` | commanded, naive |
| `SimulationSemanticCutSummary` rows | commanded, naive |
| `SimulationCutHotspot` | commanded, naive |
| `ToolpathKinematicRuntime::breakdown` | integrator, **pre-modulation** (empty when the profile has no explicit kinematics) |

### Is the recommendation still the right shape?

**Yes, with one correction: name the four vocabularies that already exist and
collapse them, do not add a fifth.**

The tree already carries four partial provenance markers:

- `FeedsProvenance::{Planned, Emitted}` (planned versus emitted feeds)
- `CycleTimeBasis::{MachineModel, SimulatedNoAccel, CuttingOnly}`
  (`crates/rs_cam_viz/src/ui/readiness.rs`)
- `ToolpathKinematicRuntime` (`crates/rs_cam_core/src/simulation_cut.rs:825`)
- `RebasedCuttingTimes` (`crates/rs_cam_core/src/simulation_cut.rs:691`)

The audit asks for "a shared timed-move/sample view with explicit timing
provenance". That is right. It also asks to "encode that distinction in its
type or field name" for the deliberately preserved commanded-time metric.
That has NOT been done: `average_engagement` is a bare `f64` with a doc
comment. The finding's second half is untouched.

Add one requirement the audit did not state: **the population must be part of
the view, not a field the caller picks.** Both symptoms above come from two
sites choosing two different populations for the same fold.

### Size estimate — counted

- **Crates: 4.** `rs_cam_core`, `rs_cam_viz`, `rs_cam_cli`, `rs_cam_mcp`.
- **Writers of the timing model: 2 + 1 helper.**
  `compute/simulate.rs:1445` (`apply_kinematics_cycle_time`),
  `session/compute.rs:3196-3266` (the retime block),
  `simulation_cut.rs:763` (`rebase_cutting_times`).
- **Aggregate types on the wire: 5** (`AirCutRatios` impl list), of which
  **2** are mixed base, plus `KinematicsSummary` and
  `ToolpathKinematicRuntime`.
- **Core source files reading a runtime field: 15.**
  `machine.rs`, `machine_kinematics.rs`, `simulation_cut.rs`, `narrate.rs`,
  `sim_measurability.rs`, `sim_triage.rs`, `session/mod.rs`,
  `session/compute.rs`, `compute/simulate.rs`, `dexel_stock/stamping.rs`,
  `tool_load/{chipload,deflection,power}.rs`,
  `tool_load/optimize/{context,mod}.rs`.
- **Consumer files outside core: 9**, 39 reference lines
  (`rs_cam_viz/src/ui/readiness.rs`, `rs_cam_viz/src/ui/sim_diagnostics.rs`,
  `rs_cam_viz/src/state/simulation.rs`,
  `rs_cam_viz/src/controller/events/compute.rs`,
  `rs_cam_viz/src/app/mcp.rs`, `rs_cam_viz/src/compute/worker/tests.rs`,
  `rs_cam_cli/src/main.rs`, `rs_cam_cli/src/project.rs`,
  `rs_cam_mcp/src/response.rs`).

The two symptoms above are each a small change. The one-timing-view refactor
is not.

---

## Finding 8 [High] — Drilling reconstructs its analytical model

**Verdict: STILL TRUE. The audit's verb is wrong and the correction matters.**

### The audit's verb is wrong

The audit says drilling "reconstructs its analytical model **after generating
motion**". That reads as "derives the model from the emitted moves". It does
not. `build_drill_op_for_config`
(`crates/rs_cam_core/src/compute/execute.rs:613`) reads the CONFIG. It never
touches the toolpath. Both functions derive from the same config, in
parallel.

This matters for scoping. A refactor written against the audit's wording would
look for a motion-to-model inversion. There is none to remove.

### What IS still duplicated

Holes and the cycle expansion are already shared:

- `drill_holes_for_config` (`crates/rs_cam_core/src/compute/execute.rs:1122`)
  serves `generate_drill` at line 1160 and `build_drill_op_for_config` at
  line 631.
- `pin_holes_in_emission_frame`
  (`crates/rs_cam_core/src/compute/execute.rs:932`) serves
  `generate_alignment_pin_drill` at line 1201 and
  `build_drill_op_for_config` at line 668. Its doc says why:
  *"the ONE place the pin-drill's two differently-framed hole sources are
  reconciled, shared by the generator and by `build_drill_op_for_config` so
  the two cannot drift"* (`execute.rs:908-910`).
- `drill::fed_descents` (`crates/rs_cam_core/src/drill.rs:166`) expands the
  cycle once. The emitter uses it at `drill.rs:213` and `drill.rs:247`. The
  metric uses it at `crates/rs_cam_core/src/drill_metrics.rs:279`.

**Five expressions are still written twice.** For `OperationConfig::Drill`:

| Quantity | Generator | Analytical builder |
|---|---|---|
| `top_z` | `execute.rs:1164` `ctx.stock_bbox.max.z` | `execute.rs:637` `stock_bbox.max.z` |
| depth / `bottom_z` | `execute.rs:1163` `cfg.depth` | `execute.rs:638` `top_z - cfg.depth` |
| cycle | `execute.rs:1161` `cfg.cycle.to_core(cfg)` | `execute.rs:652` `cfg.cycle.to_core(cfg)` |
| retract | `execute.rs:1168` `effective_safe_z(cfg.retract_z, ...)` | `execute.rs:660` `effective_safe_z(cfg.retract_z, top_z)` |
| feed | `execute.rs:1166` `op.feed_rate()` | `execute.rs:653` `cfg.feed_rate` |

I read `OperationConfig::feed_rate` at
`crates/rs_cam_core/src/compute/catalog.rs:994`. It dispatches to
`as_params().feed_rate()`, which for `DrillConfig` returns the same field. The
two expressions agree today. They are two expressions.

The `AlignmentPinDrill` arm repeats the same five, with depth written as
`stock_z + cfg.spoilboard_penetration` in the generator
(`execute.rs:1207-1208`) and as `min.z - cfg.spoilboard_penetration` in the
builder (`execute.rs:674`). Same number, two derivations.

### The silent-refusal point the audit did not name

`build_drill_op_for_config` calls `drill_holes_for_config(...).ok()?` at
`crates/rs_cam_core/src/compute/execute.rs:631`, and
`pin_holes_in_emission_frame(...).ok()?` at `execute.rs:669`. A builder
refusal becomes `None`, and the caller then stores `OpData::Toolpath` instead
of `OpData::DrillOp`:

- `crates/rs_cam_core/src/session/compute.rs:1925-1928`
- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:1049-1060`

The result is a drill toolpath with motion and no analytical model. It would
then take the stamping branch, produce engagement samples, and publish no
`drill_summaries` row and no drill gates. The generator refuses on the same
inputs, so today this is unreachable — held by convention, not by type. That
is precisely what a `DrillPlan` would make impossible.

### F4.8, read before judging

F4.8 changed hole PICK resolution, not the plan split.
`drill_holes_for_config` (`execute.rs:1122-1144`) now refuses on a stale pick
through `stale_drill_picks_refusal` (`execute.rs:1127-1133`), and the same
refusal is in `pin_holes_in_emission_frame` (`execute.rs:945-947`). The
centroid fallback is removed, not gated (`execute.rs:1106-1113`). Because both
sides call the same helper, F4.8 landed on both sides at once. It made the
shared half larger. It did not touch the duplicated half.

### The tool-profile claim is CONFIRMED

`tool_profile: ToolProfile::Flat` is hardcoded in both arms
(`crates/rs_cam_core/src/compute/execute.rs:650` and `execute.rs:687`), and
`tool_diameter_mm = tool_def.radius() * 2.0` at `execute.rs:624` is the
ENVELOPE diameter. The registry entry allows any tool:
`REG_DRILL` sets `tool_constraints: ToolConstraintsDef::ANY_TOOL` at
`crates/rs_cam_core/src/compute/catalog.rs:2190`. This is the audit's
`catalog.rs:2106` anchor at today's line. `CLAUDE.md` already ledgers the
consequence to the radius programme (R-12): all three drill gates divide by
the envelope radius, so a tapered ball reads `Within` on an overloaded cutter.

### The R-plane sentry is sound, and it does not cover this

`crates/rs_cam_core/tests/drill_fed_descents_motion.rs` compares the STORED
toolpath's fed descents against `fed_descents`. Its own header states why
(`lines 26-38`): `DrillToolpathSummary::feed_time_s` reads the config, so
asserting against `drill_summaries` would be vacuous. The sentry pins the
cycle expansion. It says nothing about the other four duplicated expressions,
and nothing about the `.ok()?` refusal.

### Is the recommendation still the right shape?

**Yes.** "Resolve one validated `DrillPlan`, then derive motion and analytical
removal from it" is correct, and F4.8 makes it cheaper, not more expensive.
Two amendments:

1. Correct the wording. The plan replaces a PARALLEL derivation, not a
   post-hoc reconstruction.
2. Make the plan's construction fallible ONCE. The `.ok()?` at
   `execute.rs:631` and `execute.rs:669` must become a refusal that both
   consumers inherit, not a `None` one consumer can swallow.

Tool and profile belong in the plan, as the audit says. That work is already
ledgered as R-12; do not double-count it.

### Size estimate — counted

- **Crates: 2.** `rs_cam_core` and `rs_cam_viz`.
- **Call sites of `build_drill_op_for_config`: 2.**
  `crates/rs_cam_core/src/session/compute.rs:1914`,
  `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:1049`.
- **Functions to merge: 3.** `build_drill_op_for_config` (`execute.rs:613`),
  `generate_drill` (`execute.rs:1147`),
  `generate_alignment_pin_drill` (`execute.rs:1186`).
- **Shared helpers already in place: 3.** `drill_holes_for_config`,
  `pin_holes_in_emission_frame`, `drill::fed_descents`.
- **Sentries that must keep passing: 4.**
  `drill_fed_descents_motion.rs`, `drill_op_step3.rs`,
  `drill_material_plumbing_f016.rs`,
  `drill_picks_resolve_to_targets_g_drillpickstale.rs`.

Small. One crate pair, two call sites.

---

## Finding 9 [Medium] — Shared run emission bypassed for annotations

**Verdict: STILL TRUE, and wider than the audit says.**

### The audit's three claims, checked

1. *"Radial finishing uses the shared emitter."* TRUE.
   `crates/rs_cam_core/src/radial_finish.rs:160` calls
   `tp.emit_path_segment_with_intent(...)` per run, then
   `tp.final_retract(params.safe_z)` at line 170.
2. *"Spiral finishing manually emits equivalent run structure while
   attaching ring annotations."* TRUE.
   `crates/rs_cam_core/src/spiral_finish.rs:248-298` is the loop. It emits
   `rapid_to_with_intent(..., Linking)` at line 254,
   `feed_to_with_intent(..., EntryPlunge)` at line 259, then body feeds at
   line 292. The reason is the annotation: it captures
   `rapid_move_index = tp.moves.len().saturating_sub(1)` at line 258 and
   pushes a `SpiralFinishRuntimeEvent::Ring` at that index (lines 263-273).
   The shared emitter returns no move range, so the index cannot be captured
   through it.
3. *"Spiral's loop starts the next run with a rapid to its new XY/safe-Z
   position, without the per-run vertical retract. Its final retract occurs
   after all runs."* TRUE. There is no `MoveIntent::Retract` anywhere in
   `spiral_finish.rs` except `final_retract` at line 299 (I grepped
   `MoveIntent::Retract` in that file; no other match).

The shared emitter's contract is at
`crates/rs_cam_core/src/toolpath.rs:265-273`: rapid to `(path[0].xy, safe_z)`,
plunge, feed, then `rapid_to_with_intent(P3::new(last.x, last.y, safe_z),
MoveIntent::Retract)`. Spiral has the first three and not the fourth.

### What the audit missed: adaptive 2D has the same omission

`crates/rs_cam_core/src/adaptive/path.rs:1516-1526` — the
`AdaptiveSegment::Rapid(entry)` arm emits
`rapid_to_with_intent(..., Linking)` to the new XY at `safe_z`, then the
plunge. There is one retract in the whole function, at
`adaptive/path.rs:1594-1598`, after every segment. The motive is the same:
the `AdaptiveSegment::Marker` arm at line 1511 captures `tp.moves.len()` for
an `AdaptiveRuntimeAnnotation`.

Caveat for the scoper: adaptive 2D's segment model has a `Link` variant that
stays down deliberately (`adaptive/path.rs:1527-1533`). Its run envelope is
a different contract from the finish emitter's. Call it the same source-level
shape, not the same bug.

### Full census — six hand-rolled run emitters

I listed every core file that emits `MoveIntent::EntryPlunge`, then removed
the consumers (`compute/spans.rs`, `dressup.rs`, `entry_audit.rs`,
`feed_modulation.rs`, `feedopt.rs`, `sim_triage.rs`, `boundary.rs`,
`surface_link.rs`, `unified_finish.rs`). Six generators emit their own run
structure.

**Group A — no per-run vertical retract (the safety-relevant shape):**

| File | Emission | Annotation motive |
|---|---|---|
| `crates/rs_cam_core/src/spiral_finish.rs:248-299` | one final retract | `SpiralFinishRuntimeEvent::Ring` at a captured move index |
| `crates/rs_cam_core/src/adaptive/path.rs:1516-1598` | one final retract | `AdaptiveRuntimeAnnotation` at a captured move index |

**Group B — retract per run, hand-rolled for a stated reason:**

| File | Retract site | Reason |
|---|---|---|
| `crates/rs_cam_core/src/scallop.rs:2536-2539` and `2640-2643` | per run, at the anchor | helical link between rings + `ScallopRuntimeAnnotation` |
| `crates/rs_cam_core/src/pencil.rs:1874-1878` | per run, at `prev_end` | stock-aware surface links + `PencilRuntimeAnnotation` |
| `crates/rs_cam_core/src/adaptive3d/path.rs:1272-1280` (`lift_to_safe_z`, 6 call sites at 1372, 1380, 1402, 1500, 1531, 1562) | per transition | peck-plunge ladder + `Adaptive3dRuntimeAnnotation` |

**Group C — pure duplication, no annotation:**

| File | Note |
|---|---|
| `crates/rs_cam_core/src/steep_shallow.rs:473-489` and `497-505` | emits the full shared envelope by hand, including the per-run `Retract`. This is NOT an annotation bypass. File it as a plain call-site conversion. |

### Severity framing — the diagonal rapid is masked by default

`tsp::rebuild_group` (`crates/rs_cam_core/src/tsp.rs:466-524`) discards the
framing rapids and regenerates them as a VERTICAL `Retract` followed by a
horizontal `Linking` traverse — the `idx > 0` arm at `tsp.rs:497-506`, and the
`idx == 0` arm at `tsp.rs:481-492` which exists for exactly this case
(*"Without this retract, the next single rapid would go diagonally … slicing
through stock"*). `DressupConfig` defaults `optimize_rapid_order: true`
(`crates/rs_cam_core/src/compute/config.rs:2010`).

So on default dressups the diagonal rapid in the spiral and adaptive-2D IR
does not reach G-code. It is masked, not absent. With the dressup off, the IR
ships as written. I did not run either arm.

### Is the recommendation still the right shape?

**Yes.** "Extend the shared emitter with annotation hooks or returned move
ranges" is exactly what the five Group-A/B generators need — every one of them
hand-rolls in order to capture a move index. Three amendments:

1. **A returned move range is the right primitive, not a callback.** All five
   annotation sites want an index, and three of them (`scallop`, `pencil`,
   `adaptive3d`) want the index of a move the emitter has not written yet.
2. **Group C is a separate, trivial work item.** `steep_shallow` needs no
   hook. Do not bundle it and inflate the phase.
3. **Group A is the only part with a safety argument, and it is
   conditional.** State the TSP masking in the phase, or the phase will be
   scoped as a safety fix and measured as one.

### Size estimate — counted

- **Crates: 1.** `rs_cam_core` only. No `rs_cam_viz` or `rs_cam_cli` file
  emits a run.
- **Shared emitter methods: 3.** `emit_path_segment` (`toolpath.rs:225`),
  `emit_path_segment_with_intent` (`toolpath.rs:256`),
  `emit_closed_contour_with_intent` (`toolpath.rs:298`).
- **Existing shared-emitter call sites (regression surface): 17** across 13
  files — `ramp_finish.rs:840`, `radial_finish.rs:160`,
  `profile.rs:189`, `steep_shallow.rs:342,350`,
  `project_curve.rs:373,387`, `zigzag.rs:243`, `pocket.rs:449`,
  `waterline.rs:214,243,254`, `inlay.rs:185,376`, `trace.rs:173`,
  `rest.rs:207`, `vcarve.rs:183,248`, `horizontal_finish.rs:285`.
- **Generators to convert: 6** (2 in Group A, 3 in Group B, 1 in Group C).
- **Runtime annotation types to route through the hook: 5.**
  `SpiralFinishRuntimeAnnotation`, `AdaptiveRuntimeAnnotation`,
  `Adaptive3dRuntimeAnnotation`, `ScallopRuntimeAnnotation`,
  `PencilRuntimeAnnotation`.

---

## Found while verifying

1. **The post-modulation retime drops every drill toolpath from the project
   total.** `crates/rs_cam_core/src/session/compute.rs:3183-3257`. G-DRILLTIME
   was closed on `apply_kinematics_cycle_time` and never on this path.
   Precondition: modulation on (the default) AND an explicit machine
   kinematics block. No sentry covers it. See Finding 5, Symptom 1.

2. **`SimulationCutTrace::toolpath_runtimes` has one populating writer and is
   stale after modulation.** `crates/rs_cam_core/src/compute/simulate.rs:1501`.
   `crates/rs_cam_viz/src/ui/readiness.rs:499` reads it first, for the four
   operator surfaces. Those surfaces report the planned clock while the
   diagnostics report the emitted clock. Same precondition as symptom 1. See
   Finding 5, Symptom 2.

2b. **The integrator and the retime read two different kinematics sources.**
   `session/compute.rs:938` uses raw `self.machine.kinematics`;
   `session/compute.rs:3055` uses `effective_kinematics()`. On a shipped
   preset the retime therefore integrates with a fallback block and the
   readiness basis reads `MachineModel`. See Finding 5, Symptom 4.

3. **Adaptive 2D shares spiral's missing per-run retract, for the same
   annotation reason.** `crates/rs_cam_core/src/adaptive/path.rs:1516-1598`.
   The audit named spiral only. See Finding 9.

4. **`build_drill_op_for_config` can refuse silently.** `.ok()?` at
   `crates/rs_cam_core/src/compute/execute.rs:631` and `execute.rs:669`. A
   builder refusal becomes `OpData::Toolpath` — a drill toolpath with motion,
   no analytical model, no drill gates. Unreachable today only because the
   generator refuses on the same inputs. Convention, not type. See Finding 8.

5. **`steep_shallow` hand-rolls the shared emitter for no stated reason.**
   `crates/rs_cam_core/src/steep_shallow.rs:473-505`. It reproduces the
   envelope exactly, including the retract. It is duplication with no
   annotation motive, so it does not belong in the same work item as the
   annotation bypasses.

6. **The audit's own anchors barely drifted for two of the three findings.**
   `compute/simulate.rs:1445`, `toolpath.rs:256` and `radial_finish.rs:159`
   are the same lines today. Only the drill and spiral anchors moved. The
   98 commits did not touch these three areas much — which is itself evidence
   that the findings stand.

## What I did NOT verify

- No magnitude for any of the three Finding 5 symptoms. That needs a
  simulation run, and I ran no `cargo`.
- Whether the diagonal rapid in spiral or adaptive 2D reaches a real `.nc`
  file with the TSP dressup disabled. I read the code path only.
- Whether `rapid_collision_count` reports the Group-A diagonal rapids. The
  live-replay checker should see them for stamped ops, but I did not run it.
