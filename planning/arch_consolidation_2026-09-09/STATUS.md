# Architecture consolidation — status tracker

**Read this before `PLAN.md`.** The audit was a read-only review at `627ef997`
on 2026-09-09. It says of itself: *"Runtime consequences were not tested."*
**98 commits landed between that commit and this tracker.**

On 2026-09-10 all twelve findings were verified against the current tree by
four read-only lanes. Their full reports are in `verify/LANE_A..D.md`, each
claim carrying a `path:line` at today's tree. **No lane ran a test.** Every
verdict below is from reading, and the reports say so; treat "STILL TRUE" as
"the code still has this shape", not "the failure was reproduced".

> **Protocol.** The orchestrator updates THIS file only. `AUDIT.md` and
> `PLAN.md` stay verbatim as produced. States: `TODO` / `IN PROGRESS (who)` /
> `PARTLY CLOSED` / `BLOCKED (on what)` / `DONE (commit)`. Never delete a row;
> mark it `DROPPED (why)`.

---

## Execution checkpoint — 2026-09-10, N1 DONE

- **N3: DONE (`069a2314`).** Already fixed by G-STEPUNITS before this
  execution. The project loader applies `EnrichedMesh::apply_uniform_scale`;
  `tests/step_project_load.rs::both_doors_apply_a_step_models_declared_units`
  pins both import doors. This phase did not reimplement it. The default
  heavy gate does not enable `step`, so its zero-test STEP binaries are not
  a rerun of that sentry.
- **N1: DONE (`d4e1154b`; sentry `2687b82b`).** Reproduced through core
  checked export before the fix: a disabled operation's label was emitted
  from its retained result. Core
  now filters `!tc.enabled` before result lookup and datum transformation.
  `tests/export_disabled_cached_n1.rs` asserts disabled motion, tool, RPM and
  pre/post snippets are absent, an enabled control remains, and re-enabling
  restores the cached operation without regeneration. Cache retention and
  enabled missing/stale-result policy are unchanged. Both CLI callers use
  this corrected core door; the cached-disabled reproduction is a core test,
  not a separate CLI-command reproduction.
- **Verification:** focused N1 1 passed; core G-code 35; datum 5; GUI export
  6; CLI 31. `cargo fmt --all -- --check` and workspace all-target Clippy
  with `rs_cam_core/heavy-tests` / `-D warnings` passed. The full core heavy
  gate completed in 2718 s: **3783 passed, 1 failed, 288 ignored**, across
  301 result targets (299 integration binaries plus library and doctests).
  This is **not a green full gate**. Its sole failure is the established
  F-036b `modulation_raises_cutting_chipload_toward_band`: **0.0214** against
  **[0.0320, 0.0550]**, exactly the baseline documented in
  `../ui_fix_2026-09-09/reports/J2.md` (lines 303–367) and its sibling
  `HANDOVER.md` (lines 74–86).
  The all-F-word median includes G-RAMPCONTAIN's extra entry segments; the
  cutting-only population is 0.0550. No assertion was weakened or ignored.
  Local logs: `/tmp/rs_cam_n1/`, `/tmp/rs_cam_n1_clippy.log`,
  `/tmp/rs_cam_n1_core_heavy.log` (matching `.status` files); these are local
  execution artifacts, not repository fixtures.
- **Next:** N2. The operator gave standing approval on 2026-09-10 ("I approve
  it all … assume approval … dont worry about me for the gates"), so the
  per-item human gate is lifted. The tests, the review step and the stop
  conditions all stand unchanged. N4 and N5 remain open; N6 stays with
  Phase 1A. No architecture phase is claimed complete. `PLAN.md` and
  `AUDIT.md` remain verbatim.

## Execution checkpoint — 2026-09-10, N5 DONE

- **N5: DONE (`1e4373aa`; sentry `3b8cd298`).** `OperationParams::set_stepover`
  and `set_depth_per_pass` are optional trait methods with empty default
  bodies. An operation with no such field inherited a setter that discarded
  the value, and the named arm of `ProjectSession::set_toolpath_param` then
  stamped `Manual` provenance on the absent field, ran
  `invalidate_result_chain` and reported `Ok(())`. Both setters now report
  whether they wrote. The arm refuses on `false`, before the stamp and
  before the invalidation, with the generic serde arm's own wording from one
  shared `unknown_param_error`. The DR-LIVE range gate moves into
  `check_param_range`, which every numeric route calls. The reject
  populations are **11** operations for `stepover` and **14** for
  `depth_per_pass`; the sentry asserts both counts and both directions, so a
  fix that refused every write would not pass. The three alias setters
  (Pencil `offset_stepover`, Waterline `z_step`, RampFinish `max_stepdown`)
  keep working. `rs_cam_cli run --set` turns a silent success on an
  unsupported field into an error, by intent.
- **Verification:** sentry RED pre-fix at `2 passed; 3 failed` (a runtime
  red), GREEN post-fix at `5 passed; 0 failed`, and GREEN again after the
  format. Core `--lib session` 157; `toolpath_rebind_g_mcprebind` 9;
  `rs_cam_cli` 31. The core dev loop (no `heavy-tests`) ran 290 result
  targets: **3741 passed, 1 failed, 271 ignored**. Its sole failure is the
  established F-036b `modulation_raises_cutting_chipload_toward_band` at
  **0.0214** against **[0.0320, 0.0550]** — the baseline in
  `../ui_fix_2026-09-09/reports/J2.md`. `cargo fmt --all -- --check` and
  workspace all-target Clippy with `rs_cam_core/heavy-tests` / `-D warnings`
  both pass. The format moved the sentry file only, so it folded into the
  sentry commit. `rs_cam_viz` tests compiled under Clippy; they were not
  executed. Local log: `/tmp/n5_core.log` — a local artifact, not a
  repository fixture.

## Execution checkpoint — 2026-09-10, N4 DONE

- **N4: DONE (`44c68add`; sentry `6d1d2026`).** `gui_and_mcp_diagnostic_ids_match`
  hand-rebuilt the session's input and compared the result with itself, so it
  could not bite. The core diagnose route built its `ResolvedHeights` from
  defaults instead of the toolpath's own `heights`, so a pinned Top Z and a
  retract pinned below the feed plane reached the GUI ribbon and never
  reached the MCP route. `ResolvedHeights::from_heights` is the one
  resolver; the diagnose route passes `&tc.heights`, and the GUI's
  `diagnostics_heights` delegates to it. The tautology test is deleted and
  the sentry replaces it: it drives the real MCP entry point and the ribbon
  on one session and asserts the two id sets agree.
- **Verification:** sentry RED pre-fix at `1 passed; 2 failed` (a runtime
  red; arm 1, the auto-heights baseline, was already green), GREEN post-fix
  at `3 passed; 0 failed`. `depth_beyond_stock_core_g_depthstockcore` 8;
  `inert_claims_dial_f4` 5; `unified_finish_dropped_band_finding_d1` 3. The
  whole `rs_cam_viz` suite: **606 passed, 0 failed**, across 36 result
  targets — the deleted test's module still compiles and carries no unused
  import. The core dev loop is unchanged from the N5 checkpoint at **3741
  passed, 1 failed, 271 ignored** across 290 result targets, its sole
  failure the established F-036b baseline. `cargo fmt --all -- --check`
  needed no change on either commit, and workspace all-target Clippy with
  `rs_cam_core/heavy-tests` / `-D warnings` passes.

## Execution checkpoint — 2026-09-10, N2 DONE

- **N2: DONE (`736a2959`; sentry `8634acf1`).** The cycle-time integrator
  published `toolpath_runtimes` for every toolpath and folded the project
  total over the engagement summary list, which carries no drill row. The
  modulation re-time then rebuilt only part of that, so the two published
  runtimes disagreed and the project total dropped the drill.
  `simulation_cut::publish_cycle_times` is now the one publisher: the
  integrator tail and the re-time both call it, so every runtime reaches
  every surface on one clock. `session::compute::reintegrate_toolpath`
  builds the map the re-time hands it.
- **Verification:** sentry RED pre-fix at `3 passed; 2 failed` (a runtime
  red). Its three fixture and non-vacuity tests were green pre-fix, so the
  fixture is not vacuous. GREEN post-fix at `5 passed; 0 failed`.
  `drill_cycle_time_integration_g_drilltime` 4;
  `air_cut_one_time_base_g_airdenom` 6; viz `cycle_time_basis_g_timeest`
  12. `adaptive_feed_modulation_pipeline_f036b` reads **9 passed, 1
  failed** — its one failure is the established red-by-design
  `modulation_raises_cutting_chipload_toward_band` at **0.0214** against
  **[0.0320, 0.0550]**, the same number as before this fix. No other
  expected number moved. The core dev loop: **3746 passed, 1 failed, 271
  ignored** across 291 result targets (the five new sentry tests are the
  whole delta from the N4 checkpoint). `rs_cam_viz` 606; `rs_cam_cli` 31.
  `cargo fmt --all -- --check` needed no change on either commit, and
  workspace all-target Clippy with `rs_cam_core/heavy-tests` /
  `-D warnings` passes.

## Execution checkpoint — 2026-09-10 late, N1–N5 DONE

- **N1** `d4e1154b` (sentry `2687b82b`), **N5** `1e4373aa` (sentry `3b8cd298`),
  **N4** `44c68add` (sentry `6d1d2026`), **N2** `736a2959` (sentry `8634acf1`).
  Each sentry ran RED alone and GREEN after its fix; the red output is in
  each fix commit's body. N3 was already closed by G-STEPUNITS.
- Full core heavy gate after all four: **3793 passed, 1 failed, 288 ignored**
  across 303 result targets. The one failure is the F-036b instrument, red by
  design. Not a green full gate. Viz suite 606/0, CLI 31/0. fmt and clippy
  (heavy-tests, `-D warnings`) clean. Log `/tmp/rs_cam_gate_n245.log`.
- **Open before the phases:** N6 (folds into Phase 1A), N7 (needs an operator
  ruling), N9 (small), N10 (needs an operator ruling), N11 (Phase 4B). N8 was
  closed inside N4.
- **The N1 checkpoint above says "N4 and N5 remain open". That was true when
  written and is superseded by this checkpoint.**
- The three landing commits carry a `docs(status): Nx DONE` row edit each.
- Process note for the next verifier: never `git reset --hard` in this tree.
  It drops the foreign `.mcp.json` edit. The N5 verifier did that once and
  restored the file byte-identical (blob `e74e2a8c`).

## Execution checkpoint — N9 DONE (`52fdd7dd`)

Standing operator approval covers commits and advancing items; tests, review,
resource limits and unresolved-policy stop conditions remain binding. N1–N5
were already landed and were not repeated. N9 now snapshots canonical session
precondition and model-reference contexts alongside the GUI's temporary entry;
the ribbon requires both contexts. The extended N4 sentry consumes that same
production snapshot, not a hand-rebuilt parallel input path. Missing Rest
predecessor and dangling-model cases reproduced at **4 passed / 2 failed**
before the fix; all **6 passed** after it, including valid controls. GUI
load-verdict, feeds, generation-stat and height inputs are preserved.

Review ACCEPT. Full viz **609/0**, MCP **29/0**, format and heavy-enabled
workspace Clippy pass. Full core heavy: **3793 passed, 1 failed, 288 ignored**,
303 result targets; only F-036b at **0.0214** vs **[0.0320, 0.0550]**, exactly
the preceding baseline. This is not a green full gate. No live GUI/MCP check
was run. Local evidence: `/tmp/rs_cam_n9_red.log`, `/tmp/rs_cam_n9_retry.log`,
`/tmp/rs_cam_n9_gates/summary.json` and sibling gate logs. An attributable type
import compilation error in the first implementation was fixed in one retry;
the final sentry also guards actual production snapshot assembly.

`.mcp.json` remains byte-identical (Git blob
`e74e2a8ce2f72ac0ed51f3f7b95638d4133de980`); raw file SHA-1 is a different hash
algorithm input and must not be compared to the Git blob hash.
Next is Phase 0's remaining executable evidence. N7 and N10 remain explicitly
decision-blocked; N6 and N11 retain their phase assignments.

## Phase 0 policy decisions — pending operator rulings (2026-09-11)

| # | Question | Facts (read, not run) | Options |
|---|---|---|---|
| P0-D1 | **Coolant: which export door is right?** | `ToolpathConfig::coolant` is a per-toolpath core field (`session/mod.rs:769`), round-trips both project loaders, and the emitter carries an A3 fix so a coolant change between same-tool phases is not suppressed. The GUI door (`viz/io/export.rs:340`) and the CLI job door (`cli/src/main.rs:547,613`) honour it. The core door hardcodes `CoolantMode::Off` (`core/gcode/mod.rs:367`), so `rs_cam_cli project --emit-gcode` and `rs_cam_cli run` drop the setting silently. **No GUI control and no MCP setter writes the field**; the export wizard's tally points at an inspector control that does not exist. | (a) core honours `tc.coolant`; the Phase 0 test inverts into an equality. (b) declare the field dead and delete it with the test. Opposite Phase 1 work; not decided here. |
| P0-D2 | **N7: retime on an unguarded kinematics fallback** | See row N7. | (a) guard with `is_some()` so `kinematics: None` keeps live-sim runtime byte-identical, per `machine.rs:141-146`. (b) accept the fallback and change the readiness label rule. |
| P0-D3 | **N10: `radial_finish` unranged divisors** | See row N10. | (a) add `ParamRange::greater_than(0.0)` rows (a threshold; operator's call). (b) leave and record. |

## Do these before the phases. They are defects, not refactors.

The rows below retain the original audit evidence. N1/N3's current execution
states above supersede their historical descriptions.

| ID | What | Evidence | Size |
|---|---|---|---|
| **N3 — DONE (`069a2314`)** | **Historical: STEP unit scale is DROPPED on the project loader.** `rs_cam_cli run --units inches part.step` cuts geometry **25.4x wrong**. Two lanes confirmed independently. `project_file.rs:617` computes the scale; the STL, DXF and SVG arms pass it; the STEP arm (`:665-678`) never reads it. Lane B read `truck-stepio` 0.3.0 and found its READER performs **no unit conversion at all**, so `ModelUnits` is the only unit conversion a STEP file gets in this product. Not a double-scale: the interactive door is correct and the project door is wrong. Reachable on three doors; the GUI alone cannot create such a record (`import_step_path` hardcodes 1.0, `rescale_model` refuses STEP). | `core/session/project_file.rs:665-678` vs `core/io.rs:31,108` | **ONE LINE**, plus a STEP row in `model_units_survive_reload_g_unitsreload.rs`, which today has ZERO `step` hits. G-UNITSRELOAD closed the third divergence in this loader pair and left the fourth. |
| **N1 — DONE (`d4e1154b`; sentry `2687b82b`)** | **Historical: CLI export may emit a DISABLED operation's toolpath.** `set_toolpath_enabled` calls `invalidate_output_dependents`, not `invalidate_result_chain`, so a disabled operation keeps its cached result on purpose. Core's export phase collect discards the `ToolpathConfig` and never reads `enabled`. The GUI and MCP routes filter; `rs_cam_cli project --emit-gcode` and `rs_cam_cli run` appear not to. **THIS IS A READ, NOT A REPRODUCTION** — reproduce before believing it. No test pins either behaviour. | `core/session/mutation.rs:339`, `core/gcode/mod.rs:328` | Small, once reproduced. Wrong-cut class if real. |
| **N2 — DONE (`736a2959`; sentry `8634acf1`)** | **G-DRILLTIME is closed on one path and live on another.** `apply_kinematics_cycle_time` integrates every toolpath including drills. The post-modulation retime (`session/compute.rs:3183-3257`) resets `project_total` to zero and folds over `trace.toolpath_summaries` — the engagement population, which a drill never joins. It never rewrites `trace.toolpath_runtimes`, and that is what `readiness::toolpath_cycle_time` reads FIRST for the readiness panel, pre-flight gate, export wizard and setup sheet. Needs a configured machine kinematics block. No sentry covers it: the one test exercising the retime has no drill in its fixture. **`CLAUDE.md`'s G-DRILLTIME entry is therefore now half-true and must be amended when this is fixed.** | `core/session/compute.rs:3183-3257` vs `core/compute/simulate.rs:1464-1533` | Medium. Two writers of one field. |
| **N4 — DONE (`44c68add`; sentry `6d1d2026`)** | **A parity guard that cannot bite.** `gui_and_mcp_diagnostic_ids_match` hand-rebuilds the session's inputs, and its "GUI arm" calls `ResolvedHeights::from_context` — the MCP-route call. **It compares MCP against a replica of MCP.** Recorded earlier as J8.4 "a mirror that became a parallel copy"; it is worse than that. | `viz/ui/properties/operations/mod.rs:2657` | Small. Rewrite or delete. |
| **N5 — DONE (`1e4373aa`; sentry `3b8cd298`)** | **`set_toolpath_param` accepts and silently discards a write on many operations.** The named `"stepover"` arm returns `Ok(())` on every operation whose config omits the setter — **11 operations**. `depth_per_pass` the same on **14**. The six named arms also bypass the DR-LIVE `ParamRange` gate entirely, which lives only in the generic `_` arm. `angular_step` / `point_spacing` have no registry range and are divisors. | `core/session/compute.rs:509`, `radial_finish.rs:96,108` | Part of Phase 4B, but the silent-success half is a defect now. |
| **N6** | `set_drill_selected_holes` is a second narrow mutation path the audit did not name, sitting beside a wide one. | `core/session/mutation.rs:944` vs `:911` | Small; folds into Phase 1A. |
| **N7** | **The modulation retime integrates on an unguarded kinematics fallback.** `apply_adaptive_feed_modulation` takes `self.machine.effective_kinematics()` with no `is_some()` guard, and that falls back to `generic_wood_router`. With modulation ON (the default) and `kinematics: None` the retime overwrites `trace.summary.total_runtime_s` and stamps `runtime_by_intent = Some(..)`, so `readiness.rs` labels the toolpath `MachineModel` on a machine that has no kinematics block. `machine.rs:141-146` says `None` must keep live-sim runtime byte-identical. `f036b.rs:281-284` still claims an `is_some()` guard that no longer exists. Found by the N2 scout 2026-09-10 (read, not run). Behaviour decision, not a fold bug: needs an operator ruling before a fix. | `core/session/compute.rs:3055`, `machine.rs:147-150`, `viz/ui/readiness.rs:507-509` | Small once ruled. |
| **N8** | **The core/MCP diagnostics route projects heights and drops every pin.** `ResolvedHeights::from_context` writes `top_z = stock_top_z`, `bottom_z = stock_bottom_z`, `feed_z = stock_top_z`, `retract_z = clearance_z = safe_z`, so on that route `geom.bottom_above_top_z`, `geom.feed_z_below_top_z` and `geom.clearance_z_below_retract_z` can never fire and `geom.depth_beyond_stock` misses a pinned Top Z (J8.1). The GUI's `diagnostics_heights` resolves the real `HeightsConfig`. Root cause is a missed call: `tc.heights` is in scope at the call site. Found by the N4 scout 2026-09-10 (read, not run). Fixed inside the N4 work package. | `core/diagnostics/adapters/from_static_checks.rs:78-90`, `core/session/compute.rs:4265` | Small; in N4. |
| **N9 — DONE (`52fdd7dd`)** | **The ribbon and the session route disagree on preconditions and model refs by design.** `collect_diagnostics` passes `preconditions: None` and `model_refs: None`; `diagnose_toolpath_with_trace` passes `Some(..)`. A Rest with no prior op exposes it. The N4 sentry excludes this family and says so. Found by the N4 scout 2026-09-10. | `viz/ui/properties/operations/mod.rs:2281,2285` vs `core/session/compute.rs:4291-4292` | Small; viz holds the session and can pass both. |
| **N12** | **Seven generation-input divergences between the GUI worker door and the session door** (Finding 1, made concrete). Both doors call `execute_operation_annotated_with_regions`, but everything before it is written twice. (1) `ToolpathConfig::face_selection` is read by the GUI only: `controller/events/compute.rs:354-372` derives polygons from the picked faces and `:536-542` anchors an auto `top_z` to the face; core's `resolve_generation_inputs` never reads the field. (2) A `BoundarySource::FaceSelection` boundary confines to the face on the GUI door (`worker/execute/mod.rs:770-775`) and to the stock rectangle on the session door (`session/compute.rs:2084-2115`). (3) Feed-optimisation stock: viz builds it (`worker/helpers.rs:41-52`), core passes `None` (`session/compute.rs:1712`); live on a fresh-stock 2D Pocket with the shipped default `feed_optimization: true`. (4) Dressup stock-top frame: core `emission_stock_bbox.max.z`, viz `req.heights.top_z`; diverge on a pinned Top Z. (5) Entry-probe index: core reuses the memoised index, viz builds a fresh one and gates on `entry_style`. (6) Four `HeightContext` builders; the two generation ones have no polygon fallback. (7) The single-polygon clip is open-coded in viz. Found by the Phase 0 generation-parity scout 2026-09-11 (read, not run). Phase 3 owns the fix; Phase 0 pins the shape. | `viz/controller/events/compute.rs:181-760`, `viz/compute/worker/execute/mod.rs:43-237,760-908`, `core/session/compute.rs:1090-1993` | Phase 3. |
| **N13** | **The feeds Apply funnel invalidates nothing.** `controller/events/mod.rs:936-949` (GUI Apply all, project rollup, MCP `apply_feeds`) writes `tc.operation` through `toolpath_configs_mut()` with no `drop_result`, no `invalidate_result_chain`, no `invalidate_toolpath_inputs`, and `session.simulation` is not cleared. `state/freshness.rs:79-97` derives freshness from `core_has_result` and ignores `stale_since`, so the op reads `Current` and export (`io/export.rs:281`) emits the PRE-apply geometry and feed words. The 500 ms auto-regen sweep masks it only on the 12 op specs with `default_auto_regen: true`. Same class as the pre-G-FRESHSTATE inspector defect. Found by the Phase 0 mutation-paths scout 2026-09-11 (read, not run). Wrong-feed export class: fix now. | `viz/controller/events/mod.rs:936-953`, `viz/state/freshness.rs:79-97` | Small. |
| **N14** | **Undo/redo and optimizer apply invalidate one index; the setter and the inspector invalidate the chain.** `apply_toolpath_param_snapshot` (`core/session/mutation.rs:1248`) ends at `drop_result` + `simulation = None`, so an undone or optimizer-applied edit leaves downstream `FromRemainingStock` results standing. `set_toolpath_param` (`:236-238`) and `invalidate_toolpath_inputs` (`:1385-1391`) walk the chain. Undo carries no `feeds_provenance` (`viz/state/history.rs:29-37`). `AUDIT.md:68`'s claim that `replace_toolpath_config` is narrow is STALE: it is wide since R0.1 §4.3 (`mutation.rs:1222`). Found 2026-09-11 (read, not run). Phase 1A with N6. | `core/session/mutation.rs:1248` vs `:236,1385` | Phase 1A. |
| **N15** | **A third staleness model: `compute_stale_set(ToolpathParamChanged)` returns exactly one index** (`core/session/compute.rs:53-58`), no chain walk, so the MCP reply's `stale_toolpaths` under-reports what core dropped, and on N13's path it is the only signal. `compute_stale_set_for_toolpath_param_returns_single_toolpath` (`compute.rs:5355`) PINS the narrow answer; Phase 1A must change that test, not only the code. Found 2026-09-11. Phase 1A. | `core/session/compute.rs:53-58,5355` | Phase 1A. |
| **N10** | **`radial_finish` divides by two unranged params.** `angular_step` and `point_spacing` are `ParamDef::required(.., "f64")` with no `ParamRange`; `0.0` saturates `num_spokes` to `usize::MAX` and overflows `Vec::with_capacity`; a negative value gives an empty toolpath with no error. Adding a range row is a numeric threshold, so it needs an operator ruling; the precedent shape is `peck_depth`'s `required_ranged(.., ParamRange::greater_than(0.0), ..)`. Found by the N5 scout 2026-09-10. Phase 4B. | `core/radial_finish.rs:96,108`, `catalog.rs:1933-1940` | Small once ruled. |
| **N11** | **The Apply/pill funnel discards a Stepover apply on a no-field op by the same mechanism as N5.** `suggest.rs:1084-1085` calls the defaulted `set_stepover` and does not read whether it wrote. Found by the N5 scout 2026-09-10. Phase 4B. | `core/feeds/suggest.rs:1084-1085` | Small. |

---

## Verified finding verdicts

| # | Finding | Audit rating | **Verified verdict** | Correction to the audit |
|---|---|---|---|---|
| 1 | Core and GUI assemble separate generation pipelines | High | **STILL TRUE** | Exactly TWO sites. **MCP is not a third** — it pushes the GUI's own `AppEvent::GenerateToolpath`. 14 comments in the viz worker describe themselves as mirroring `session/compute.rs`. |
| 2 | Invalidation is a convention, not an enforced contract | High | **PARTLY CLOSED** | Half the first bullet is DEAD: `replace_toolpath_config` now propagates AND has **zero callers**. `apply_toolpath_param_snapshot` is still narrow (6 production sites). Raw mutable accessors unchanged (**73 non-test sites, counted**). F2.5's `invalidate_tool` bypass is fixed viz-side; undo's toolpath-param arm still carries it. **A CONSTRAINT THE PLAN MUST CARRY:** `optimize/candidate.rs:428-430` documents a live DEPENDENCE on the narrowness. The transaction cannot be "always invalidate the chain". |
| 3 | Computed results have competing owners | High | **PARTLY CLOSED** | The export fallback the audit called the architectural problem is now guarded by `StaleResultPolicy`, default `Refuse` (G-STALEXPORT, `66d2c232`). Two stores remain but the split is now DELIBERATE (G-LATERESULT, `9544ca75`). |
| 4 | Export duplicates program assembly | High | **STILL TRUE** | **THREE sites, not two.** The CLI job-file pipeline is a third. All three stated differences hold. |
| 5 | Timing corrections update selected summaries | High | **PARTLY CLOSED** | G-AIRDENOM closed the per-toolpath half. Four summaries stay mixed-base, exactly as `CLAUDE.md` records. **FOUR partial provenance vocabularies already exist** (`FeedsProvenance`, `CycleTimeBasis`, `ToolpathKinematicRuntime`, `RebasedCuttingTimes`) — collapse them, do not add a fifth. |
| 6 | Model import duplicated below convergence | Medium | **STILL TRUE — and it hides N3** | Extension recognition duplicated **four** ways. `winding_report` is never measured on a project load. There is a **third** import door (`LoadedModel::from_file`) and a fourth legacy dispatch in `viz/io/project.rs`. F4.4 unified a DIFFERENT pair (the three in-place refresh doors). |
| 7 | Parameter support and validity have multiple authorities | High | **STILL TRUE and wider** | See N5. G-SCHEMAENUM closed advertised-subset-of-accepted only; the reverse direction and "the string is consulted at validation" are both open. |
| 8 | Drilling reconstructs its analytical model after generating motion | High | **STILL TRUE — but the audit's VERB is wrong** | It does NOT derive from emitted motion. `build_drill_op_for_config` derives from the CONFIG, in parallel with `generate_drill`. Holes and cycle expansion are already shared. **Five expressions written twice.** Size: 2 crates, 2 call sites, 3 functions. **Re-rate this: it is Small, not High.** |
| 9 | Shared run emission bypassed for annotations | Medium | **STILL TRUE and wider** | **SIX** hand-rolled emitters. The audit missed `adaptive/path.rs:1516-1598`, same omission and same motive. `steep_shallow.rs:473-505` duplicates with no annotation motive and is a separate item. Severity is masked, not absent: `tsp::rebuild_group` regenerates framing rapids and `optimize_rapid_order` defaults true. |
| 10 | Offset failure handling remains opt-in | Medium | **STILL TRUE** | **13 production sites** on the dropping door across 7 files; 8 already migrated. `adaptive/path.rs` alone holds 5. **Correction the plan must carry:** two of three failure classes are `debug_assert!`s in the dependency, so a RELEASE build cannot fully separate failure from collapse even after every caller migrates. The Policy row overstates what the door can deliver. |
| 11 | Vendor routing encodes incidents | Medium | **STILL TRUE** | `lut_query_for` is 30 lines: three special cases plus a pass-through. Its own doc names five finish ops with the same hole and says they are not routed. `LookupQuery` carries no provenance. The five named holes have **no sentry at all**. |
| 12 | New caches copy the same infrastructure | Medium | **STILL TRUE and UNDERCOUNTED** | **THREE** weak-identity memos, not two. `geom_cache.rs` is the same pattern and the OLDEST. ~250 non-blank lines of memo infrastructure across three files. The generic **must carry `peek`**, and `geom_cache`'s one-entry-three-slots shape does not fit a one-key-one-value generic. |

---

## Phase status, after verification

| Phase | Subject | Status | Note |
|---|---|---|---|
| 0 | Contracts and executable evidence | **ROUGHLY HALF PAID** | Of eight characterization requirements: 1 well covered, 3 substantially covered, 3 where adjacent tests exist but do not assert the stated contract, 1 barely started. See below. |
| 1A | One mutation contract | TODO | Finding 2. Carry the `optimize/candidate.rs` constraint. Fold in N6. |
| 1B | One artifact owner | TODO | Finding 3, de-scoped: the export fallback is already guarded. |
| 1C | Migrate consumers | TODO | |
| 2 | Centralize export planning | TODO | Finding 4, **three** sites. Settle N1 first. |
| 3 | Unify the full generation pipeline | TODO | Finding 1. The largest phase. Needs Phase 1. |
| 4A | Canonical model import | TODO | Finding 6. **Fix N3 first and separately** — it is one line and it is a live wrong-size defect. Four extension dispatches, four doors. |
| 4B | Canonical parameter contracts | TODO | Finding 7 / N5. |
| 5A | Resolve drilling once | TODO | **Re-rated to Small.** Finding 8's verb is wrong. |
| 5B | Shared annotated run emission | TODO | Finding 9, six emitters. |
| 5C | Structured offset outcomes | TODO | Finding 10, 13 sites. Carry the release-build caveat. |
| 6A | One timing view | TODO | Finding 5. Fix N2 first or fold it in. |
| 6B | Explicit vendor lookup use | TODO | Finding 11. |
| 7 | Cache mechanics | TODO | Finding 12, three files. |
| 8 | Whole-system acceptance | TODO | |

### Phase 0 — what is already paid

Roughly 30 sentries were added by the UI programme and the core track in the
two days before this tracker, on top of the F-024..F-040 regression net and
the `_litmatrix_*` feeds suite. The full heavy gate is green at **299
binaries, 3773 passed**, with `adaptive_feed_modulation_pipeline_f036b` red by
design.

The largest genuine gap is **core-versus-GUI export parity**: exactly one test
crosses the doors (`g_modexport`) for one property. Coolant, per-operation
RPM, tool changes and datums have none.
**Correction 2026-09-11 (read, not run):** `g_modexport` calls only the GUI
door (`export_gcode_from_session_with_policy`); no viz test calls
`export_gcode_checked`. Before Phase 0's parity test, NO test compared the two
doors on one session. The registry-driven-coverage ask is
mostly paid — nine binaries already drive off `for_each_op!`.

**Ten source-level string-pin sentries exist.** Three name their own weakness
in their headers; seven do not. A source-level pin is weaker than a test that
runs the path, and Phase 0's job is evidence a refactor cannot silently break.
Treat those seven as gaps, not coverage. N4 is the worst case of the class.

---

## What earlier work already closed, per finding

**Finding 2.** G-FRESHSTATE made stock, tool and model edits invalidate on
both routes and put `tool_id`/`model_id` into `generation_inputs_signature`.
F2.12 gave the holder-clearance verdict the edit-counter stamp every sibling
row had. F2.13 made it declare how much of the job it examined. F4.7 made
`rescale_model` invalidate. `replace_toolpath_config` propagates and is dead.

**Finding 3.** G-STALEXPORT guarded the fallback; G-LATERESULT made the split
deliberate.

**Finding 5.** G-AIRDENOM rebased cutting times per sample.

**Finding 6.** F4.4 unified the three in-place refresh doors into
`LoadedModel::adopt_geometry` with an exhaustive destructure. **Different pair
from the one the audit names.**

**Finding 7.** G-SCHEMAENUM fixed six advertised enum values no config could
hold and added a generic round-trip sentry.

---

## Provenance, and one prior mistake to avoid

The pi command `/techdebt-orchestrate` pointed at
`planning/architectural_refactor_2026-06-06_v2.md` until 2026-09-10. **That
plan is COMPLETE** — every work item T0..T16 landed 2026-06-07, verified by
content and not by its own tracker. It is also a CORE refactor, not a GUI one.
An account pointed at it would have found nothing to do. The command now
points here.

`AUDIT.md` and `PLAN.md` were produced by gpt-5.6-astra on 2026-09-09, lived
only inside a pi session transcript, and were recovered on 2026-09-10.

## Related open items

`planning/ui_fix_2026-09-09/PLAN.md` §11 carries twelve follow-ons opened on
2026-09-10, several inside these phases: F2.14 (widen the holder check),
F4.10 / F4.11 / F4.12 (drill pick coverage), J8.1 (MCP drops a pinned Top Z),
J8.4 (now N4). Read that list before opening work packages here.
