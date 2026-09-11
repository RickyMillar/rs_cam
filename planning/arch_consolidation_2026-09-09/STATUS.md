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
| P0-D1 **DONE (a) (`281c4ae7`; sentry `09263c04`)** | **Coolant: which export door is right?** | `ToolpathConfig::coolant` is a per-toolpath core field (`session/mod.rs:769`), round-trips both project loaders, and the emitter carries an A3 fix so a coolant change between same-tool phases is not suppressed. The GUI door (`viz/io/export.rs:340`) and the CLI job door (`cli/src/main.rs:547,613`) honour it. The core door hardcodes `CoolantMode::Off` (`core/gcode/mod.rs:367`), so `rs_cam_cli project --emit-gcode` and `rs_cam_cli run` drop the setting silently. **No GUI control and no MCP setter writes the field**; the export wizard's tally points at an inspector control that does not exist. | (a) core honours `tc.coolant`; the Phase 0 test inverts into an equality. (b) declare the field dead and delete it with the test. Opposite Phase 1 work; not decided here. |
| P0-D2 | **N7: retime on an unguarded kinematics fallback** | See row N7. | (a) guard with `is_some()` so `kinematics: None` keeps live-sim runtime byte-identical, per `machine.rs:141-146`. (b) accept the fallback and change the readiness label rule. |
| P0-D3 | **N10: `radial_finish` unranged divisors** | See row N10. | (a) add `ParamRange::greater_than(0.0)` rows (a threshold; operator's call). (b) leave and record. |

## Operator rulings — 2026-09-11

- **Q5 (test-fixture door) — ANSWERED 2026-09-11, "take the recommendation":** a `pub ProjectSessionBuilder` for setup-time construction plus `apply` for the live-mutation test sites (about 117 sites over three crates). WP7 is no longer blocked; it still requires WP5, WP6 and WP6b.
- **No large test gates** (2026-09-11): verifiers run sentries, targeted suites, the small crates and lint only.


- **Branch.** Work on `master` directly. `ui-fix-2026-09-09` was fast-forwarded
  into master at `8f14e02e`. No pull requests. This is a personal project.
- **Architecture.** `RULING_ONE_COMMAND_SURFACE_DRAFT.md` is ADOPTED as Phase
  0's architecture ruling: four kinds (`Command` / `Query` / `Job` /
  `UiCommand`), unconstructible resolved inputs, the six mutation hatches
  `pub(crate)`, `macro_rules!` registry with a completeness sentry, CLI surfaces
  declared per row and never generated. First step: `SetToolpathParam` alone.
- **P0-D1 coolant → (a).** The core export door honours `ToolpathConfig::coolant`.
  The Phase 0 parity test inverts to equality.
- **N7 → (a).** The modulation retime guards on `machine.kinematics.is_some()`;
  a machine with no kinematics block keeps the plain simulated runtime.
- **N10 → add rows.** `angular_step` and `point_spacing` get
  `ParamRange::greater_than(0.0)` in the `peck_depth` shape. Operator-authored
  threshold.

## Implementation plan — 2026-09-11

`IMPLEMENTATION_PLAN.md` (`fe080c6d`) sequences the adopted ruling over the nine
phases: 15 work packages, dependency order forced by types, per-package call-site
inventory, sentries, deletions, rollback, risk register, per-phase definition of
done, tracker protocol. Reviewed adversarially (round 1 REJECT, ten blockers,
all applied; round 2 ACCEPT WITH CHANGES, applied). **Nothing is implemented.**
Q5 (which test-fixture door) blocks WP7; WP1 (`SetToolpathParam` through `apply`)
can start. Next: operator reads the plan; compact; start WP1 in a gap.

## Command surface — work packages (2026-09-11)

Per `IMPLEMENTATION_PLAN.md` §8. One row per package. States: `TODO` /
`IN PROGRESS (who)` / `BLOCKED (on what)` / `DONE (fix hash; sentry hash)`.
Never delete a row; mark it `DROPPED (why)`.

| WP | Subject | State | Evidence |
|---|---|---|---|
| WP1 | Row 1: `SetToolpathParam` through `apply` | DONE (`90c8e90e`; sentry `255f6fe3`) | Sentries `command_registry_completeness` (8), `command_registry_surfaces` (3), `mutation_paths_invalidate_alike_p0::n15_apply_reports_the_set_the_setter_dropped` (8). All three were compile-fail red on the sentry commit. Suites: core 3777 passed / 1 red by design (`f036b::modulation_raises_cutting_chipload_toward_band`) / 271 ignored; viz 621; CLI 32; MCP 29; core `--lib session::` 154. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings` both clean. No heavy gate: WP1 is not a phase end. |
| WP2a | MCP wire compatibility pin (shape, not only name) | DONE (`ecee1ee5`; sentry `8f4faf43`) | Sentry `crates/rs_cam_viz/tests/mcp_wire_surface_pin.rs`, two arms: `wire_surface_matches_the_checked_in_snapshot` and `every_mcp_reached_command_row_is_on_the_wire`. Both were red on the sentry commit, because the snapshot did not exist. The snapshot `crates/rs_cam_viz/tests/snapshots/mcp_wire_surface.json` pins **78** tools, one row per tool: the wire name and the `inputSchema`. `cargo test -p rs_cam_viz -p rs_cam_cli --test mcp_wire_surface_pin` also passes; that build turns the `preserve_order` feature of `serde_json` on, which `cargo tree -e features` shows is absent from the `-p rs_cam_viz` build, so the key canonicalisation holds on both stores. Suites: viz 623 passed / 0 failed. `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings` and `cargo fmt --all -- --check` both clean. No heavy gate: WP2a is not a phase end. |
| WP3 | `Effects` everywhere; `AdoptResult` | DONE (`25f37b74`; sentry `0d22acaf`) | Sentry `crates/rs_cam_core/tests/adopt_result_rejects_stale_completion.rs`, five arms: `a_completion_for_a_superseded_parameter_set_is_refused`, `an_edit_to_another_toolpath_does_not_refuse_this_completion`, `an_absent_index_is_refused_as_absent`, `every_producer_reports_the_dropped_set`, `the_enable_toggle_excludes_its_own_index`. The sentry commit alone was compile-fail red, 14 errors: `AdoptResultArgs`, `Command::AdoptResult` and `SessionError::StaleCompletion` did not exist, and the producers returned `()`. Green on the fix: 5 passed / 0 failed. Two in-crate viz sentries in `crates/rs_cam_viz/src/controller/tests.rs` prove the drain hands the stamp through: `drain_refuses_a_completion_whose_revision_moved` and `drain_adopts_a_completion_whose_revision_is_current`. The two P0 contract tests `the_setter_the_inspector_door_and_the_replacement_agree` and `a_dropped_result_and_a_bumped_revision_are_the_same_event` pass with their assertions unchanged; only the arm helpers, the `.revision` read and the `AdoptResult` door moved in that file. `remove_toolpath`'s MCP arm keeps an empty stale list by ruling (`bump_all_revisions` re-keys, it does not drop). Suites: core `--test adopt_result_rejects_stale_completion` 5, `--test mutation_paths_invalidate_alike_p0` 8, `--test command_registry_completeness` 8, `--test toolpath_rebind_g_mcprebind` 9, `--test set_param_refuses_absent_field_n5` 5, `--test export_honors_coolant_p0d1` 2, `--test model_path_round_trip_g_modelrelink` 6; core `--lib session::` 154; viz 625 (was 623, plus the two new sentries); CLI 32; MCP 29 — all 0 failed. The verifier added one change beyond the writers' commits: `let _ =` at `crates/rs_cam_core/tests/command_registry_completeness.rs:395`, because `Effects` is `#[must_use]` and the writers' scan did not cover the core test targets. `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings` and `cargo fmt --all -- --check` both clean. No whole-core suite and no heavy gate: operator ruling 2026-09-11. |
| WP4 | MCP Mutations section onto `McpRequestKind::Core` | IN PROGRESS (Fable lane, 2026-09-11; core writer; plan §15) | — |
| WP5 | GUI inspector toolpath door | DONE (`caf8fbe6`; sentry `973834f7`) | Sentry `crates/rs_cam_core/tests/replace_toolpath_config_gates_on_the_signature.rs`, four arms: `a_replacement_outside_the_signature_lands_and_drops_nothing`, `a_replacement_that_moves_the_signature_drops_the_chain`, `the_command_writes_the_three_unsupplied_fields_verbatim`, `the_row_declares_its_surfaces`. The viz half is two in-crate tests in `crates/rs_cam_viz/src/controller/tests.rs`: `the_projection_keeps_the_three_fields_the_entry_cannot_supply_wp5` and `an_open_panel_that_edits_nothing_drops_no_result_wp5`. The sentry commit alone was compile-fail red on both targets: core read 10 errors (`E0432` on `ReplaceToolpathConfigArgs`, `E0599` on `Command::ReplaceToolpathConfig`, on `CommandId::ReplaceToolpathConfig`, on `ToolpathConfig::generation_inputs_signature` and on `ToolpathConfig::clone`), and `rs_cam_viz --lib` read one (`E0425: cannot find function project_entry_onto in module crate::ui::properties`). The signature moved to core as `ToolpathConfig::generation_inputs_signature` with nine fields in — operation, dressups, heights, boundary, rest analysis, tool id, model id, stock source, face selection — and the viz copy is deleted; the inspector applies a projection each frame through `Command::ReplaceToolpathConfig` and stamps `Effects.stale`; the stale-stamp ordering defect the writer found is fixed in the same commit (`write_entry_runtime_to_gui` runs BEFORE the config write-back, because it copies the pre-draw `entry.stale_since` and the reverse order erased the fresh stamp). The dead `ProjectSession::replace_toolpath_config` is deleted. P0 flip: arm 4 of `mutation_paths_invalidate_alike_p0.rs` moved onto the command and arm 3 stays on the raw `invalidate_toolpath_inputs` door, so `the_setter_the_inspector_door_and_the_replacement_agree` keeps its independent comparison (plan §4 WP5 non-vacuity trap). Suites: core `--test replace_toolpath_config_gates_on_the_signature` 4, `--test mutation_paths_invalidate_alike_p0` 8, `--test command_registry_completeness` 10, `--test restore_snapshot_invalidates_like_the_setter_n14` 4; core `--lib session::` 156 (was 155: one gate test replaced by two); viz 628 over 40 binaries (was 626, plus the two new in-crate tests), of which `feeds_apply_drops_result_n13` 5, `command_registry_surfaces` 3, `mcp_wire_surface_pin` 2, `boundary_controls_always_visible_g_boundaryinherit` 3, `freshness_surfaces_g_freshrender` 6, `model_refresh_invalidates_results_g_rescalestale` 4; CLI 32; MCP 29 — all 0 failed. The verifier folded two changes into the fix commit: `cargo fmt --all` rewrapped the import block in `crates/rs_cam_core/tests/mutation_paths_invalidate_alike_p0.rs:115`, and the `project_entry_onto` doc at `crates/rs_cam_viz/src/ui/properties/mod.rs:3706` named the retained boundary-inherit field by its identifier, which the G-BOUNDARYINHERIT source sentry refuses anywhere under `src/ui`; the doc names it in prose now and states the correct reason the core field survives (project-file compatibility, not emitted geometry). `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings` and `cargo fmt --all -- --check` both clean, both re-run after that edit. No whole-core suite and no heavy gate: operator ruling 2026-09-11. |
| WP6 | egui scratch-copy pattern, twelve draw sites | TODO | — |
| WP6b | The 27 non-egui viz sites and the 9 CLI sites | TODO | — |
| WP7 | The eleven hatches go `pub(crate)` | TODO (Q5 answered 2026-09-11: builder + `apply`; requires WP5, WP6, WP6b) | — |
| WP7a | `ProjectSessionBuilder` + the 63 SETUP test sites (plan §20) | DONE (`084b9b50`; sentry `76dbfb41`) | Sentry `crates/rs_cam_core/tests/session_builder_preserves_ids.rs`, seven arms: `builder_keeps_tool_ids_and_order`, `builder_keeps_model_ids_and_order`, `builder_keeps_toolpath_order_and_bindings`, `builder_replaces_the_seeded_setup_then_appends`, `builder_leaves_the_stock_alone`, `builder_stores_the_result_and_no_revision`, `builder_raises_the_id_counters_above_every_supplied_id`. The sentry commit alone was compile-fail red, one error: `E0432: no ProjectSessionBuilder in session`. Green on the fix: 7 passed / 0 failed. 51 sites migrated of the 63 the scout classified SETUP — 27 in `crates/rs_cam_viz/tests` (16 files), 18 in the viz in-crate test modules (7 files), 6 in `crates/rs_cam_core/tests` (5 files); the verifier counted the hatch half of that as 38 `*_mut()` call lines removed (22 viz tests / 13 viz in-crate / 3 core tests). Twelve SETUP sites stay for WP7: eleven `wizard_mut` arranges in `wizard_e2e.rs` and one boundary arrange in viz `controller/tests.rs`. Six `models_mut().push` sites in `ui/properties/operations/mod.rs` `#[cfg(test)]` are SETUP-shaped and remain for WP7. Suites: core `--test session_builder_preserves_ids` 7; the five migrated core binaries in one run — `claims_reference_cascade_am6` 8, `export_datum_setup_frame` 5, `lateral_setup_end_to_end` 4, `setup_datum_round_trip_p2` 5, `simulation_issue_channel_m1` 2 passed / 1 ignored (`rivers_b4_probe_project_curve_on_remaining_stock`, expensive); viz 625 (40 binaries); CLI 32; MCP 29 — all 0 failed. The verifier added `let _ =` at eight sites in three test files, because the session setters return `#[must_use]` `Effects` and the raw field writes they replaced returned nothing: viz `controller/tests.rs:2581,2642,2757,2927,3118`, core `tests/simulation_issue_channel_m1.rs:450`, core `tests/claims_reference_cascade_am6.rs:463,647`. `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings` and `cargo fmt --all -- --check` both clean. No whole-core suite and no heavy gate: operator ruling 2026-09-11. |
| WP8 | N14 and N6: undo, optimizer, drill picks | DONE (`e079e192`; sentry `5c32ced7`) | Sentry `crates/rs_cam_core/tests/restore_snapshot_invalidates_like_the_setter_n14.rs`, four arms: `the_restore_drops_the_set_the_setter_drops_n14`, `a_byte_identical_restore_still_drops_the_chain_f2_5`, `the_payload_writes_the_feeds_provenance`, `a_drill_pick_drops_the_chain_n6`. The viz half is one in-crate test, `an_undo_stales_the_downstream_row_and_restores_the_provenance_n14` in `crates/rs_cam_viz/src/controller/tests.rs`. The sentry commit alone was compile-fail red on three targets, two errors each: `--test restore_snapshot_invalidates_like_the_setter_n14` and `--test mutation_paths_invalidate_alike_p0` read `E0432` on `RestoreToolpathSnapshotArgs` and `E0599` on `Command::RestoreToolpathSnapshot`; `rs_cam_viz --lib` read `E0559` twice on `UndoAction::ToolpathParamChange`'s absent `old_feeds_provenance` and `new_feeds_provenance`. P0 flip: arm 2 of `mutation_paths_invalidate_alike_p0.rs` moved onto the new command (the old door is `pub(crate)` now, so the arm could not flip in place), `n14_undo_and_the_setter_invalidate_alike` and `n6_a_drill_pick_invalidates_the_chain` both read `{0, 1}`, and the pin-hole sibling stays green unchanged. The optimizer keeps `pub(crate) apply_toolpath_param_snapshot_narrow` on its two call sites; `the_narrow_path_leaves_the_neighbour_result_cached` in `session/mutation.rs` pins that the neighbour's cached result survives it. N14 and N6 closed; `set_feeds_provenance` kept pub with zero production callers. Suites: core `--test restore_snapshot_invalidates_like_the_setter_n14` 4, `--test mutation_paths_invalidate_alike_p0` 8, `--test command_registry_completeness` 10, `--test adopt_result_rejects_stale_completion` 5, `--test drill_picks_resolve_to_targets_g_drillpickstale` 6, `--test toolpath_rebind_g_mcprebind` 9; core `--lib session::` 155; core `--lib tool_load::optimize::` 223; viz 626 (40 binaries, was 625); CLI 32; MCP 29 — all 0 failed. Both cherry-picks auto-merged; WP7a did not touch `mutation_paths_invalidate_alike_p0.rs`. The verifier changed nothing beyond the writer's two commits. `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings` and `cargo fmt --all -- --check` both clean. No whole-core suite and no heavy gate: operator ruling 2026-09-11. |
| WP9 | `Query` first row: cycle time | DONE (`5173c923`; sentry `720626b2`) | Sentry `crates/rs_cam_core/tests/query_cycle_time_one_answer.rs`, six arms: `machine_model_basis_matches_the_oracle`, `simulated_no_accel_basis_matches_the_oracle`, `cutting_only_basis_matches_the_oracle`, `no_evidence_matches_the_oracle_none`, `a_query_for_a_missing_toolpath_refuses`, `the_registry_carries_the_cycle_time_row_as_a_query`. The file copies the pre-fix `readiness::toolpath_cycle_time` in as a frozen oracle and asserts the `Query` answers the same `CycleTime`. `command_registry_completeness` gains two arms, `a_constructed_query_answers_its_identifier` and `query_answer_has_one_variant_per_query_row`. Both binaries were compile-fail red on the sentry commit alone, 5 errors each: `Query`, `QueryAnswer`, `ToolpathCycleTimeArgs`, `ToolpathCycleTimeAnswer`, core `CycleTime` / `CycleTimeBasis` and `ProjectSession::query` did not exist. The payload carries the GUI's cut trace (the GUI does not use the session simulation slot), so the `MachineModel` and `SimulatedNoAccel` bases survive on a real project. The `Query` row declares `mcp: Skip` and the wire snapshot did not move. Suites: core `--test query_cycle_time_one_answer` 6, `--test command_registry_completeness` 10 (was 8), `--test mutation_paths_invalidate_alike_p0` 8, `--test adopt_result_rejects_stale_completion` 5, `--test drill_runtime_survives_retime_n2` 5 (the plan's P0 flip, green byte-for-byte); core `--lib session::` 154; viz 625, of which `cycle_time_basis_g_timeest` 12, `command_registry_surfaces` 3 and `mcp_wire_surface_pin` 2; CLI 32; MCP 29 — all 0 failed. The verifier changed nothing beyond the writer's two commits. `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings` and `cargo fmt --all -- --check` both clean. No whole-core suite and no heavy gate: operator ruling 2026-09-11. |
| WP10 | `Job` first row: generate one toolpath | DONE (`3ff3cffa`; sentry `b952e6ee`) | Sentry `crates/rs_cam_core/tests/job_three_steps_equal_generate_toolpath.rs`, four arms plus a control: (a) `the_three_job_steps_generate_what_generate_toolpath_generates` — the three steps cache the move count, the FNV-1a fingerprint over `Debug` of the moves, the span count and `spans_valid` that `generate_toolpath` caches, with a non-vacuity guard on an empty move list; (b) `an_edit_after_start_refuses_the_handles_result` — an edit between step (i) and step (iii) moves the revision, so the adopt refuses with `SessionError::StaleCompletion` and inserts nothing; (c) `execute_job_holds_no_session` — `execute_job` coerces to a plain function pointer, and the arm drops the session and generates from the handle alone; (d) `the_generate_toolpath_row_is_a_job` — the row declares `CommandKind::Job` and the wire name the MCP tool carries; control `generate_toolpath_repeats_its_own_answer`. The sentry commit alone was compile-fail red, 7 errors: `E0432` on `GenerateToolpathArgs`, `GenerateToolpathHandle`, `Job`, `JobHandle` and `execute_job`, `E0599` three times on `ProjectSession::start` and three times on `CommandId::GenerateToolpath`. Green on the fix: 5 passed / 0 failed. **Core-only, per plan §16 addendum**: `start` / `execute_job` / `AdoptResult` land, `generate_toolpath` runs the three steps inline, and the viz worker and drain switch is WP11b, which also closes N12. Two deliberate deviations from §16's ruling 2 stand: `start` takes the cancel flag (the resolver walks a full-grid tier map for a `PlannedTierRegions` boundary, so a flag made inside `start` would make Cancel a lie), and `start` does NOT bump the revision — it calls `self.results.remove`, the call the old head made, and `drop_result` stays the door that bumps. The CLI needed no change: all six sites call `generate_toolpath`, whose signature did not move. Suites: core `--test job_three_steps_equal_generate_toolpath` 5, `--test command_registry_completeness` 10 (`DECLARED_ROWS` derives from the registry list, so no count needed a bump), `--test adopt_result_rejects_stale_completion` 5, `--test mutation_paths_invalidate_alike_p0` 8, `--test set_param_refuses_absent_field_n5` 5; the generation-heavy trio `--test disconnected_finish_retract_structure_p0` 1, `--test drill_runtime_survives_retime_n2` 5, `--test export_honors_coolant_p0d1` 2; core `--lib session::` 156; viz 628 over 40 binaries; CLI 32; MCP 29 — all 0 failed. Both cherry-picks needed a resolution, because the writer based on `a012d10f` and master since gained WP5. `session/command.rs`: the module doc keeps both paragraphs, the `use super::` list is the union (`GenerateToolpathHandle` and `ToolpathConfig` both), the row list keeps WP5's `ReplaceToolpathConfig` row and puts the `Job` row last, and both payload structs survive (`ReplaceToolpathConfigArgs` and `GenerateToolpathArgs`). `session/mod.rs`: both re-export lists are the union; WP10's `pub use compute::{..}` block is a superset of master's, so nothing was dropped. The verifier changed nothing beyond the writer's two commits: `cargo fmt --all -- --check` was clean on both, so neither of the writer's two flagged moves was needed, and clippy reported zero warnings on the first run. `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings` and `cargo fmt --all -- --check` both clean. No whole-core suite and no heavy gate: operator ruling 2026-09-11. |
| WP11a | Publish `ResolvedGenInputs` with private fields | DONE (`a9ef73d6`; sentry `181f5c13`) | Sentry `crates/rs_cam_core/tests/resolved_gen_inputs_has_one_producer.rs`, three arms: `the_bundle_is_public_and_its_fields_are_private`, `one_public_function_produces_the_bundle`, `the_bundle_is_nameable_from_another_crate`. The sentry commit alone was compile-fail red (`E0412: cannot find type ResolvedGenInputs in module rs_cam_core::session`, two sites); the two scan arms are red by inspection, because the build stops before a scan runs. Green on the fix: 3 passed / 0 failed. Suites: core `--test command_registry_completeness` 8, core `--test mutation_paths_invalidate_alike_p0` 8, viz 623, CLI 32, MCP 29 — all 0 failed. `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests -- -D warnings` and `cargo fmt --all -- --check` both clean. No whole-core suite and no heavy gate: operator ruling 2026-09-11. |
| WP11b | Narrow the executor; close N12 | TODO | — |
| WP12 | Delete `ComputeRequest`'s mirrored fields | TODO | — |
| WP13 | `UiCommand` split and the cross-surface sentry | IN PROGRESS (classification table DRAFT 2026-09-11; no variant moved) | `WP13_CLASSIFICATION_DRAFT.md`: McpRequestKind 76 rows, AppEvent 133 rows, 9 open calls, 9 plan mismatches |

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
| **N6** | `set_drill_selected_holes` is a second narrow mutation path the audit did not name, sitting beside a wide one. Closed by WP8 (`e079e192`). | `core/session/mutation.rs:944` vs `:911` | Small; folds into Phase 1A. |
| **N7 — DONE (`c6af7f4b`; sentry `c1191963`)** | **The modulation retime integrates on an unguarded kinematics fallback.** `apply_adaptive_feed_modulation` takes `self.machine.effective_kinematics()` with no `is_some()` guard, and that falls back to `generic_wood_router`. With modulation ON (the default) and `kinematics: None` the retime overwrites `trace.summary.total_runtime_s` and stamps `runtime_by_intent = Some(..)`, so `readiness.rs` labels the toolpath `MachineModel` on a machine that has no kinematics block. `machine.rs:141-146` says `None` must keep live-sim runtime byte-identical. `f036b.rs:281-284` still claims an `is_some()` guard that no longer exists. Found by the N2 scout 2026-09-10 (read, not run). Behaviour decision, not a fold bug: needs an operator ruling before a fix. | `core/session/compute.rs:3055`, `machine.rs:147-150`, `viz/ui/readiness.rs:507-509` | Small once ruled. |
| **N8** | **The core/MCP diagnostics route projects heights and drops every pin.** `ResolvedHeights::from_context` writes `top_z = stock_top_z`, `bottom_z = stock_bottom_z`, `feed_z = stock_top_z`, `retract_z = clearance_z = safe_z`, so on that route `geom.bottom_above_top_z`, `geom.feed_z_below_top_z` and `geom.clearance_z_below_retract_z` can never fire and `geom.depth_beyond_stock` misses a pinned Top Z (J8.1). The GUI's `diagnostics_heights` resolves the real `HeightsConfig`. Root cause is a missed call: `tc.heights` is in scope at the call site. Found by the N4 scout 2026-09-10 (read, not run). Fixed inside the N4 work package. | `core/diagnostics/adapters/from_static_checks.rs:78-90`, `core/session/compute.rs:4265` | Small; in N4. |
| **N9 — DONE (`52fdd7dd`)** | **The ribbon and the session route disagree on preconditions and model refs by design.** `collect_diagnostics` passes `preconditions: None` and `model_refs: None`; `diagnose_toolpath_with_trace` passes `Some(..)`. A Rest with no prior op exposes it. The N4 sentry excludes this family and says so. Found by the N4 scout 2026-09-10. | `viz/ui/properties/operations/mod.rs:2281,2285` vs `core/session/compute.rs:4291-4292` | Small; viz holds the session and can pass both. |
| **N12** | **Seven generation-input divergences between the GUI worker door and the session door** (Finding 1, made concrete). Both doors call `execute_operation_annotated_with_regions`, but everything before it is written twice. (1) `ToolpathConfig::face_selection` is read by the GUI only: `controller/events/compute.rs:354-372` derives polygons from the picked faces and `:536-542` anchors an auto `top_z` to the face; core's `resolve_generation_inputs` never reads the field. (2) A `BoundarySource::FaceSelection` boundary confines to the face on the GUI door (`worker/execute/mod.rs:770-775`) and to the stock rectangle on the session door (`session/compute.rs:2084-2115`). (3) Feed-optimisation stock: viz builds it (`worker/helpers.rs:41-52`), core passes `None` (`session/compute.rs:1712`); live on a fresh-stock 2D Pocket with the shipped default `feed_optimization: true`. (4) Dressup stock-top frame: core `emission_stock_bbox.max.z`, viz `req.heights.top_z`; diverge on a pinned Top Z. (5) Entry-probe index: core reuses the memoised index, viz builds a fresh one and gates on `entry_style`. (6) Four `HeightContext` builders; the two generation ones have no polygon fallback. (7) The single-polygon clip is open-coded in viz. Found by the Phase 0 generation-parity scout 2026-09-11 (read, not run). Phase 3 owns the fix; Phase 0 pins the shape. | `viz/controller/events/compute.rs:181-760`, `viz/compute/worker/execute/mod.rs:43-237,760-908`, `core/session/compute.rs:1090-1993` | Phase 3. |
| **N13 — DONE (`4fe9972a`; sentry `b7503d8a`)** | **The feeds Apply funnel invalidates nothing.** `controller/events/mod.rs:936-949` (GUI Apply all, project rollup, MCP `apply_feeds`) writes `tc.operation` through `toolpath_configs_mut()` with no `drop_result`, no `invalidate_result_chain`, no `invalidate_toolpath_inputs`, and `session.simulation` is not cleared. `state/freshness.rs:79-97` derives freshness from `core_has_result` and ignores `stale_since`, so the op reads `Current` and export (`io/export.rs:281`) emits the PRE-apply geometry and feed words. The 500 ms auto-regen sweep masks it only on the 12 op specs with `default_auto_regen: true`. Same class as the pre-G-FRESHSTATE inspector defect. Found by the Phase 0 mutation-paths scout 2026-09-11 (read, not run). Wrong-feed export class: fix now. | `viz/controller/events/mod.rs:936-953`, `viz/state/freshness.rs:79-97` | Small. |
| **N14** | **Undo/redo and optimizer apply invalidate one index; the setter and the inspector invalidate the chain.** `apply_toolpath_param_snapshot` (`core/session/mutation.rs:1248`) ends at `drop_result` + `simulation = None`, so an undone or optimizer-applied edit leaves downstream `FromRemainingStock` results standing. `set_toolpath_param` (`:236-238`) and `invalidate_toolpath_inputs` (`:1385-1391`) walk the chain. Undo carries no `feeds_provenance` (`viz/state/history.rs:29-37`). `AUDIT.md:68`'s claim that `replace_toolpath_config` is narrow is STALE: it is wide since R0.1 §4.3 (`mutation.rs:1222`). Found 2026-09-11 (read, not run). Phase 1A with N6. Closed by WP8 (`e079e192`). | `core/session/mutation.rs:1248` vs `:236,1385` | Phase 1A. |
| **N15** | **A third staleness model: `compute_stale_set(ToolpathParamChanged)` returns exactly one index** (`core/session/compute.rs:53-58`), no chain walk, so the MCP reply's `stale_toolpaths` under-reports what core dropped, and on N13's path it is the only signal. `compute_stale_set_for_toolpath_param_returns_single_toolpath` (`compute.rs:5355`) PINS the narrow answer; Phase 1A must change that test, not only the code. Found 2026-09-11. Phase 1A. **WP1 (`90c8e90e`) renamed the sentry arm to `n15_apply_reports_the_set_the_setter_dropped`, which compares `apply(..)?.stale` against the dropped set, and deleted the vacuous unit test `compute_stale_set_for_toolpath_param_returns_single_toolpath`; the narrow `compute_stale_set` answer stays pinned under `n15_compute_stale_set_still_reports_one_index_pinned_divergence`.** | `core/session/compute.rs:53-58,5355` | Phase 1A. |
| **N10 — DONE (`3f8d97bb`; sentry `1634bb13`)** | **`radial_finish` divides by two unranged params.** `angular_step` and `point_spacing` are `ParamDef::required(.., "f64")` with no `ParamRange`; `0.0` saturates `num_spokes` to `usize::MAX` and overflows `Vec::with_capacity`; a negative value gives an empty toolpath with no error. Adding a range row is a numeric threshold, so it needs an operator ruling; the precedent shape is `peck_depth`'s `required_ranged(.., ParamRange::greater_than(0.0), ..)`. Found by the N5 scout 2026-09-10. Phase 4B. | `core/radial_finish.rs:96,108`, `catalog.rs:1933-1940` | Small once ruled. |
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

- 2026-09-11: core-vs-GUI export parity is now pinned by
  `crates/rs_cam_viz/tests/export_parity_core_vs_gui_p0.rs` (`1e497aa6`):
  RPM, tool changes and datums byte-identical; coolant divergence pinned
  pending P0-D1.
- 2026-09-11: the four param write paths are pinned by
  `crates/rs_cam_core/tests/mutation_paths_invalidate_alike_p0.rs`
  (`720c95e7`); N6, N14 and N15 divergences asserted with Phase 1A flip
  notes.
- 2026-09-11: the GUI worker door and the session door are pinned by
  `compute/worker/gen_parity_p0_tests.rs` (`41c3b6cd`): one geometry with
  feed-opt off; the feed-opt-stock divergence (N12 item 3) asserted with a
  Phase 3 flip note; N12 items 1, 2 and 4-7 unpinned.
- 2026-09-11: a disconnected Scallop finish is pinned by
  `crates/rs_cam_core/tests/disconnected_finish_retract_structure_p0.rs`
  (`3fa62094`): every inter-island transition retracts (hookup cap 3.0
  against a 17 mm gap), descents tag `EntryPlunge`, and no fed move leaves
  the islands.

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
