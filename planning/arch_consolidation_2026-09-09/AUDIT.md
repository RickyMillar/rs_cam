# rs_cam architecture / tech-debt audit — 2026-09-09

> **PROVENANCE.** This document was produced by gpt-5.6-astra in a pi session on
> 2026-09-09 at 21:22 UTC, as a read-only review at commit `5087490f`. It was
> never written to a file. It existed only inside the pi session transcript
> `~/.pi/agent/sessions/--home-ricky-personal_repos-rs_cam--/2026-09-09T07-13-43-916Z_01a08503-f3ec-7cb0-9e06-18f23f467ae9.jsonl`
> until 2026-09-10, when the operator asked for it to be found and recorded.
>
> The text below is VERBATIM. Nothing was edited, summarised or corrected.
> Read `STATUS.md` beside this file before acting on any finding: several are
> partly or wholly closed by work that landed after 2026-09-09.

---


# rs-cam architecture / technical-debt audit

**Read-only review at `5087490f`.** Used SocratiCode and source inspection. No builds, tests, application execution, or file changes.

## Overall assessment

**The biggest debt is duplicated orchestration and ownership—not duplicated algorithms.**

rs-cam already has useful shared primitives. However, callers still independently decide:

- which geometry, stock, and heights to supply;
- which cached result is authoritative;
- what an edit invalidates;
- which summaries to update;
- how to assemble an export.

Consequently, a shared algorithm can be correct while different product paths behave differently.

Below, `core/` means `crates/rs_cam_core/src/`; `viz/` means `crates/rs_cam_viz/src/`. These are prioritized architectural findings, not an exhaustive bug inventory. Runtime consequences were not tested.

---

## Highest-value changes

### 1. [High] Core and GUI still assemble separate generation pipelines

**Evidence:** [core/session/compute.rs:1404](crates/rs_cam_core/src/session/compute.rs#L1404), [viz/compute/worker/execute/mod.rs:603](crates/rs_cam_viz/src/compute/worker/execute/mod.rs#L603), [viz/compute/worker/helpers.rs:24](crates/rs_cam_viz/src/compute/worker/helpers.rs#L24).

Both paths independently coordinate generation, dressups, boundary processing, entry-descent optimization, findings, and result construction.

The underlying dressup pipeline **is already shared**. Its surrounding preparation is not.

A concrete divergence:
- GUI prepares stock for the feed-optimization dressup.
- Session generation passes `None` for that stock.
- The shared dressup implementation requires that stock before executing the feature.

Several boundary-processing blocks also explicitly describe themselves as mirroring the other implementation.

**Recommendation:** Introduce a core-owned, immutable **resolved generation request** and one complete executor. Session and GUI should prepare/submit that request; GUI retains asynchronous scheduling, progress presentation, and artifact handling.

**Benefit:** A new generation-wide behavior gets implemented once, rather than patched into two pipelines.

---

### 2. [High] Invalidation is a convention rather than an enforced mutation contract

**Evidence:** [core/session/mutation.rs:211](crates/rs_cam_core/src/session/mutation.rs#L211), [core/session/mutation.rs:1042](crates/rs_cam_core/src/session/mutation.rs#L1042), [core/session/mod.rs:1491](crates/rs_cam_core/src/session/mod.rs#L1491).

Normal parameter mutation uses `invalidate_result_chain`, including downstream remaining-stock and derived-region consumers.

However:
- `replace_toolpath_config` removes only that operation’s result and simulation.
- `apply_toolpath_param_snapshot` does the same.
- Undo and optimizer application use the snapshot method.
- Public mutable accessors require callers to remember separate invalidation calls.

**Consequence:** The same logical edit has different dependency effects depending on its entry point.

**Recommendation:** One transactional mutation boundary that validates the change and computes its invalidation set. Route ordinary edits, undo/redo, optimizer application, and bulk replacements through it. Restrict raw mutable access afterward.

This is more valuable than merely adding another invalidation call to each current caller.

---

### 3. [High] Computed results have competing owners

**Evidence:** [viz/state/runtime.rs:28](crates/rs_cam_viz/src/state/runtime.rs#L28), [viz/io/export.rs:90](crates/rs_cam_viz/src/io/export.rs#L90), [viz/app/mcp.rs:1288](crates/rs_cam_viz/src/app/mcp.rs#L1288).

Results exist in both `session.results` and `gui.toolpath_rt`.

The split now has behavioral significance:
- Feed modulation updates session results.
- MCP narration reads the worker result from GUI runtime.
- Export prefers session results but falls back to GUI results, including results retained for stale display.

The narration implementation consequently needs special wording explaining that it describes the pre-modulation plan.

**Recommendation:** One authoritative result store, with explicit views such as **planned**, **emitted**, and **stale display snapshot**. Consumers should request the appropriate view rather than choose a storage location.

Keeping an old toolpath visible is legitimate. Making that display cache an implicit export fallback is the architectural problem.

---

### 4. [High] Export shares the emitter but duplicates program assembly

**Evidence:** [core/gcode/mod.rs:300](crates/rs_cam_core/src/gcode/mod.rs#L300), [viz/io/export.rs:118](crates/rs_cam_viz/src/io/export.rs#L118), [viz/io/export.rs:142](crates/rs_cam_viz/src/io/export.rs#L142).

Core and GUI independently select results and build `GcodePhase` records.

There are concrete differences:
- Core phase construction hardcodes `CoolantMode::Off`; GUI uses `tc.coolant`.
- GUI explicitly filters disabled operations; core iterates cached results without that filter. Disabling an operation deliberately retains its own cached result.
- Controller-compensation selection is implemented twice.

**Recommendation:** A core **export-plan builder** owning operation selection, setup grouping, result freshness, tool identity, RPM, coolant, and datum handling. GUI/CLI provide an explicit selection and policy; they do not rebuild phases.

This is distinct from consolidating the post-processors themselves, which are already shared.

---

### 5. [High] Timing corrections update selected summaries instead of one measurement model

**Evidence:** [core/simulation_cut.rs:709](crates/rs_cam_core/src/simulation_cut.rs#L709), [core/compute/simulate.rs:1445](crates/rs_cam_core/src/compute/simulate.rs#L1445), [core/session/compute.rs:3160](crates/rs_cam_core/src/session/compute.rs#L3160).

The rebase fixes toolpath/project cutting-time figures, but explicitly leaves semantic summaries, hotspots, and per-kinematics times on the old clock.

There is another structural inconsistency:
- Initial integration tracks **all** toolpaths, including drills.
- Post-modulation retiming iterates the engagement-summary population instead.

**Consequence:** Changing the timing model requires knowing every downstream aggregate and its population. Drill inclusion has already required special handling in one of these paths.

**Recommendation:** A shared timed-move/sample view with explicit timing provenance, consumed by every aggregation. Where an intentionally preserved commanded-time metric remains, encode that distinction in its type or field name.

Do not silently redefine calibrated engagement metrics while doing this.

---

### 6. [Medium] Model import is duplicated below the point where it should converge

**Evidence:** [core/io.rs:25](crates/rs_cam_core/src/io.rs#L25), [core/session/project_file.rs:569](crates/rs_cam_core/src/session/project_file.rs#L569).

Interactive import and project loading independently dispatch formats and assemble geometry.

Some helpers are shared, but policy still differs:
- Interactive STEP loading applies the requested scale.
- Project STEP loading does not apply its computed scale.
- Winding inspection and model metadata construction follow different paths.
- Extension-to-format recognition is duplicated.

The source already records previous unit/topology divergence in this pair.

**Recommendation:** One loader returning a geometry bundle—mesh, topology, polygons, drill targets, layers, and import diagnostics. Project loading should add path resolution and persisted metadata around that loader.

A straightforward consolidation with a good payoff.

---

## Family-wide contracts that need completing

### 7. [High] Parameter support and parameter validity have multiple authorities

**Evidence:** [core/compute/catalog.rs:590](crates/rs_cam_core/src/compute/catalog.rs#L590), [core/session/compute.rs:278](crates/rs_cam_core/src/session/compute.rs#L278), [viz/ui/properties/operations/finishing.rs:127](crates/rs_cam_viz/src/ui/properties/operations/finishing.rs#L127).

Two concrete examples:

- `OperationParams::set_stepover` defaults to doing nothing. The common setter accepts `"stepover"` without checking whether the operation supports it. RadialFinish inherits that no-op.
- Radial GUI controls constrain angular step and point spacing, but their registry entries publish no ranges; the generator uses these quantities as divisors.

The registry, common setters, GUI ranges, and generator guards are not one contract.

**Recommendation:**
- Unsupported setters return an error, not silent success.
- Share hard numeric validity rules across mutation and execution.
- Keep GUI convenience ranges separate from physical validity.
- Keep raw overrides separate from feeds recommendations, but not exempt from structural validity.

---

### 8. [High] Drilling reconstructs its analytical model after generating motion

**Evidence:** [core/compute/execute.rs:590](crates/rs_cam_core/src/compute/execute.rs#L590), [core/compute/execute.rs:990](crates/rs_cam_core/src/compute/execute.rs#L990), [core/compute/catalog.rs:2106](crates/rs_cam_core/src/compute/catalog.rs#L2106).

The generator creates drilling motion; `build_drill_op_for_config` separately reconstructs holes, depths, cycles, retract height, and tooling for analytical simulation.

Hole-resolution helpers are already shared, but the **resolved drilling plan** is not.

Additionally, the analytical builder assumes a flat profile and envelope diameter while the registry allows any tool.

**Recommendation:** Resolve one validated `DrillPlan`, then derive motion and analytical removal from it. Tool/profile support belongs in that plan’s construction.

This preserves drill-native simulation rather than forcing drilling into the milling engagement model.

---

### 9. [Medium] Shared run-emission behavior is still bypassed for annotations

**Evidence:** [core/toolpath.rs:256](crates/rs_cam_core/src/toolpath.rs#L256), [core/radial_finish.rs:159](crates/rs_cam_core/src/radial_finish.rs#L159), [core/spiral_finish.rs:238](crates/rs_cam_core/src/spiral_finish.rs#L238).

Radial finishing uses the shared rapid–plunge–cut–retract emitter. Spiral finishing manually emits equivalent run structure while attaching ring annotations.

The implementations already differ: spiral’s loop starts the next run with a rapid to its new XY/safe-Z position, without the per-run vertical retract emitted by the shared helper. Its final retract occurs after all runs.

That is a **source-level safety candidate**, not a collision demonstrated by this audit.

**Recommendation:** Extend the shared emitter with annotation hooks or returned move ranges. Keep spiral/radial sampling distinct; share the motion envelope around their runs.

---

### 10. [Medium] Offset failure handling is available, but remains opt-in

**Evidence:** [core/polygon.rs:533](crates/rs_cam_core/src/polygon.rs#L533), [core/rest.rs:96](crates/rs_cam_core/src/rest.rs#L96), [core/inlay.rs:94](crates/rs_cam_core/src/inlay.rs#L94).

`offset_polygon` delegates to the reported implementation and discards the failure channel.

Production callers still use it:
- Rest machining interprets empty reachability as “large tool cannot fit.”
- Inlay uses it when deriving clearing regions.

Thus a library failure and legitimate geometric collapse remain indistinguishable in those callers, despite the shared kernel knowing the difference.

**Recommendation:** Make the structured outcome the default API. Callers explicitly choose how to handle collapse, partial output, and failure. Do not impose one universal failure policy on all algorithms.

---

## Smaller consolidation opportunities

### 11. [Medium] Vendor routing encodes individual incidents rather than explicit machining use

**Evidence:** [core/feeds/vendor_normalize.rs:10](crates/rs_cam_core/src/feeds/vendor_normalize.rs#L10).

`lut_query_for` is correctly shared across consumers. However, flat-tool DropCutter receives a roughing-family substitution, while related operations remain unresolved; the function documents that restricted scope.

**Recommendation:** Represent the resolved machining use and lookup provenance explicitly—operation, cutter, intended role, and substitution rationale. Review coverage across combinations.

**Do not** simply extend the DropCutter substitution to every finishing operation: borrowing a roughing band is a physical-policy decision, not a code-deduplication exercise.

### 12. [Medium] New caches copy the same infrastructure

**Evidence:** [core/reach_map_cache.rs:83](crates/rs_cam_core/src/reach_map_cache.rs#L83), [core/tier_map_cache.rs:130](crates/rs_cam_core/src/tier_map_cache.rs#L130).

These duplicate weak-mesh identity matching, locking, lookup, dead-entry cleanup, bounded eviction, and counters.

**Recommendation:** A small private generic weak-identity memo, keeping domain-specific keys and computation separate.

Preserve the different content-keyed strategy in `finish_surface_cache`; not every cache has the same identity requirements.

---

## What I would preserve

The existing direction is generally good:

- Operation registry and family adapters.
- Shared dressup transformations and provenance reconciliation.
- `RegionSet`, `point_runs`, and shared motion emitters.
- Central LUT routing.
- Typed triage and measurability contracts.
- Separate drill-native metrics.
- Distinct algorithms where their geometry genuinely differs.

I would **not** introduce a giant “generic CAM operation” framework or split files merely because they are large.

## Recommended sequence

1. **Unify mutation/invalidation and authoritative result access.**
2. **Centralize export-plan construction.**
3. **Converge generation orchestration around a core request/executor.**
4. **Consolidate importing and parameter validity.**
5. **Unify timing aggregation and resolved drilling plans.**
6. Finish the smaller emitter, offset-outcome, and cache extractions.

**Bottom line:** The highest-return change is making shared code own the complete contract—not just the calculation inside it. That would reduce the recurring need to fix the same behavior separately for the GUI, CLI, MCP, and individual operation families.