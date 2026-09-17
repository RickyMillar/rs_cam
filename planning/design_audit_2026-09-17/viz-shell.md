# Design and feature-debt audit — viz-shell

Group: `crates/rs_cam_viz/src/controller/`, `state/`, `app/`, `compute/`, `render/`, `io/`, `interaction/`, plus `controller.rs`, `app.rs`, `mcp_server.rs`, `mcp_bridge.rs`, `ui_command.rs`, `host.rs`.

### SHL-01 Post-config mirror needs hand-sync at every write site
- kind: design
- pattern: one concept two representations
- where: `crates/rs_cam_viz/src/state/runtime.rs:271` (`GuiState::post: PostConfig`), `state/runtime.rs:329-349` (`post_from_session`/`post_to_session`), `controller/events/mod.rs:385`, `app/input.rs:115-129`, `app/mcp/commands.rs:2458-2469`, `controller/io.rs:374-381`
- evidence: `rg -n "gui\.post\." crates/rs_cam_viz/src/controller crates/rs_cam_viz/src/app` finds 4 distinct call sites that write `session.post_config()` via `Command::SetPostConfig` and then separately hand-copy one field back onto `state.gui.post` so the mirror does not visibly lag until the next save. `app/mcp/commands.rs:2467` comment: "The viz mirror of the same field. The GUI's Feeds & Speeds modal reads this copy, not the session's." `controller/io.rs:365-372` documents a past bug (WP17) from calling the setter on every save. `state/runtime.rs:389-403` carries a regression test (`every_post_format_survives_the_session_round_trip`) written after a real drift bug (W9/P-1: reload silently reset grblHAL to GRBL).
- proposal: Replace the per-site hand field-copy with one call to `self.state.gui.post = GuiState::post_from_session(session.post_config())` after every successful `Command::SetPostConfig` apply (e.g. fold it into `adopt_post_effects`, which every one of these sites already calls or could call). A field added to `ProjectPostConfig` then can't be forgotten in one of the four manual copy sites.
- breaks: none (internal refactor; `adopt_post_effects` signature already threads `Effects` everywhere it's called)
- effort: S
- risk: low — round-trip test already pins the format edge case; the fix only removes duplicated one-field copies
- sentry: `state/runtime.rs::every_post_format_survives_the_session_round_trip` (existing); add a test that changing `spindle_strategy` through the `SetSpindleStrategy` AppEvent and through the MCP `SetPostConfig` route both leave `gui.post` byte-identical to `post_from_session(session.post_config())`
- owner:

### SHL-02 Adding one MCP command touches five match blocks in one file, plus three more files
- kind: design
- pattern: enum dispatch replicated in N places
- where: `crates/rs_cam_viz/src/app/mcp/commands.rs:218,632,1616,1758,2154` (five separate `CoreRequest::AddToolpath` / `CommandId::AddToolpath` arms), `crates/rs_cam_viz/src/mcp_bridge.rs:670,712` (`CoreRequest` enum variant + `CommandId` mapping), `crates/rs_cam_viz/src/mcp_server.rs:1027-1032` (tool declaration + description), `crates/rs_cam_mcp` (`AddToolpathParam`)
- evidence: `rg -n "AddToolpath" crates/rs_cam_viz/src/app/mcp/commands.rs` returns 8 lines across 5 distinct match statements (a pre-command side-effect match, a command-building match, a notification-message match, a shared-reply `CommandId` OR-list, and a per-`CommandId` reply match). `rg -c "#\[tool\(" crates/rs_cam_viz/src/mcp_server.rs` = 78.
- proposal: This is a lot of ceremony for one command and is probably not shrinkable without a real macro (the four matches encode four genuinely different questions: side effect, command payload, toast text, reply shape). Worth naming as debt rather than fixing blind: a small `for_each_mcp_command!`-style table (mirroring the existing `for_each_command!` / `for_each_ui_command!` pattern the crate already uses for `ui_command.rs`) could collapse the four `commands.rs` matches plus the `mcp_bridge.rs` enum/mapping into one declaration site per tool.
- breaks: none if done as an additive macro; would be a large mechanical rewrite of `commands.rs` if done for real
- effort: L
- risk: medium — `commands.rs` is 2687 lines and load-bearing for every MCP write path; a botched macro could silently drop a match arm
- sentry: `cargo test -p rs_cam_viz -q --test mcp_authoring_surface`, `--test mcp_wire_surface_pin`
- owner:

### SHL-03 `controller/tests.rs` is one 6 736-line file spanning ~11 unrelated eras
- kind: design
- pattern: god function/file with a visible seam
- where: `crates/rs_cam_viz/src/controller/tests.rs` (whole file), included via `crates/rs_cam_viz/src/controller.rs:46` (`mod tests;`)
- evidence: `wc -l controller/tests.rs` = 6736; `grep -c "#\[test\]" controller/tests.rs` = 115; `grep -c "^// ---" controller/tests.rs` = 22 (11 banner-delimited sections, e.g. "Workspace behavior tests (Phase 7)" at :793, "MCP `cancel_generation`" WP23 banner at :1487, "Roadmap F.1 — session.results cache" at :1571, later banners cite "§30 ruling 3" and "§33" operator rulings); `grep -c "^mod " controller/tests.rs` = 0 — no internal module split. `controller.rs` already mounts four *separate* single-theme test files (`holder_clearance_scope_g_holderscope.rs`, `holder_clearance_staleness_g_holderstale.rs`, `results_parity_tests.rs`, `workflow_tests.rs`), so the pattern of "one file per theme" is established elsewhere in the same directory — `tests.rs` is the one file where it was never applied.
- proposal: Split `controller/tests.rs` along its existing banner boundaries into theme files under `controller/tests/` (e.g. `workspace.rs`, `mcp_cancel.rs`, `session_results_cache.rs`, ...), the same way `ui/properties/mod.rs` and `app/mcp.rs` were split in the 2026-09-17 structure programme. Pure move, no behavior change.
- breaks: none (test-only, file layout)
- effort: M
- risk: low — mechanical move; `cargo test -p rs_cam_viz -q` after the split is the whole verification
- sentry: `cargo test -p rs_cam_viz -q` (full pass count must be unchanged: 115 tests)
- owner:

### SHL-04 `drain_compute_results` is a 636-line function with a seam three of its six arms already use
- kind: design
- pattern: god function with a visible seam
- where: `crates/rs_cam_viz/src/controller/events/compute.rs:409-1035` (`drain_compute_results`); `ComputeMessage::Toolpath` arm :412-616 (~205 lines), `ComputeMessage::Simulation` arm :617-927 (~310 lines), `ComputeMessage::Collision` arm :928-1023 (~95 lines) inlined directly in the match; `ComputeMessage::Optimize`/`Reach`/`Job` arms :1025-1034 each a one-line call to `self.handle_optimize_result(*result)` / `handle_reach_map_result` / `handle_job_result`
- evidence: read the whole function; measured with `awk` line-span per top-level `fn` in the file — `drain_compute_results` is the largest at 636 lines, next is `build_mcp_diagnostics` at 188. The `compute/CLAUDE.md` invariant ("A result arrives with the revision it was computed for. The controller rejects a result whose revision no longer matches") is implemented per-arm inline for `Toolpath`/`Simulation`/`Collision` but as a named method for the other three.
- proposal: Extract `adopt_toolpath_result`, `adopt_simulation_result` and `adopt_collision_result` private methods, matching the pattern the `Optimize`/`Reach`/`Job` arms already use. `drain_compute_results` becomes a ~15-line dispatch, and each extracted method is independently readable and testable.
- breaks: none — pure extraction, same borrow shape (`&mut self` throughout)
- effort: M
- risk: low — behavior-preserving refactor; existing sentries cover the outcomes
- sentry: `cargo test -p rs_cam_viz -q --test generate_all_fixpoint_parity`, `cargo test -p rs_cam_viz compute::worker::tests::`, plus the in-crate `controller::tests` suite that exercises `drain_compute_results`
- owner:

### SHL-05 Mesh/enriched-mesh upload keys hand-roll the compare-then-rebuild pattern per call site
- kind: design
- pattern: duplicate helper
- where: `crates/rs_cam_viz/src/app/gpu_upload.rs:270-281` (`mesh_upload_key`/`enriched_upload_key` compare), `:816-914` (`collision_upload_key`), `:1242-1244` (`rest_heatmap_upload_key`), `:1305-1306` (`tier_preview_upload_key`), `:1375-1376` (`reach_overlay_upload_key`), `:1085-1091` (`ToolpathUploadKey` compare via `previous.remove(&tp_id)`)
- evidence: `rg -n "upload_key" crates/rs_cam_viz/src/app/gpu_upload.rs | grep -v toolpath" ` shows six independent `if key_field != Some(&new_key) { rebuild }` blocks, one per resource kind, each written by hand rather than through one generic "memoized-by-key" helper. `render/upload_cache.rs` defines the six key TYPES (`MeshUploadKey`, `EnrichedUploadKey`, `CollisionUploadKey`, `RestHeatmapUploadKey`, `TierPreviewUploadKey`, `ReachOverlayUploadKey`, `ToolpathUploadKey`) but no shared comparison/rebuild function.
- proposal: Add one small generic helper in `upload_cache.rs`, e.g. `fn refresh<K: PartialEq, T>(slot: &mut Option<K>, built: &mut Option<T>, new_key: K, build: impl FnOnce() -> T) -> bool`, and thread the six call sites through it. Lower priority than SHL-01..04: each site's rebuild body differs enough (loops, side lists) that the win is de-duplicating the six near-identical `if key != Some(&new)` guards, not the rebuild bodies themselves.
- breaks: none
- effort: S
- risk: low — mechanical; `render_pipelines_headless_g_pipesmoke` and the reach/viewport sentries already cover each cache's behavior
- sentry: `cargo test -p rs_cam_viz -q --test render_pipelines_headless_g_pipesmoke`, `--test reach_overlay_p5`
- owner:

### SHL-06 The five `spawn_*_lane` worker functions repeat the same ~20-line dequeue block verbatim
- kind: design
- pattern: duplicate helper
- where: `crates/rs_cam_viz/src/compute/worker.rs:892` (`spawn_toolpath_lane`), `:1015` (`spawn_analysis_lane`), `:1140` (`spawn_optimize_lane`), `:1236` (`spawn_job_lane`), `:1368` (`spawn_reach_lane`)
- evidence: read all five function bodies. Each opens with the identical block: lock `lane.inner`, `while inner.queue.is_empty() { ... check shutdown ... reset state/current_job/current_phase/started_at/active_toolpath_id/active_toolpath_index/active_cancel to idle/None ... lane.wake.wait(inner) }`, then a second `if lane.shutdown... return`, then `#[allow(clippy::expect_used)] let request = inner.queue.pop_front().expect("queue checked")`, `lane.cancel.store(false, ...)`, `inner.state = LaneState::Running`, `inner.current_job = Some(<per_lane>_job_label(&request))`, `inner.current_phase = None`, `inner.started_at = Some(Instant::now())` — byte-for-byte identical across `spawn_toolpath_lane:899-916`, `spawn_analysis_lane:1023-1040`, `spawn_optimize_lane:1148-1165`, `spawn_job_lane:1244-1261` (the `job_lane` comment at :1266 even documents a deliberate DIFFERENCE — `active_toolpath_id` intentionally not set — showing the boilerplate is copy-pasted per lane and each copy's small deviation has to be remembered by hand). `awk` line-span measurement: the five functions are 125/123/117/96/93 lines respectively (554 total); the shared prologue is ~20 of each.
- proposal: Extract the dequeue-and-mark-running prologue into one generic helper, e.g. `fn dequeue_running<T>(lane: &LaneQueue<T>, label: impl FnOnce(&T) -> String, on_pop: impl FnOnce(&mut LaneQueue<T>, &T)) -> Option<T>` (or a small trait per request kind for the per-lane field resets), and have each `spawn_*_lane` call it before its own execute-and-send body. This also makes the deliberate `active_toolpath_id` omission in `spawn_job_lane` an explicit parameter instead of a fact an engineer must notice while reading five near-identical functions.
- breaks: none — internal to `compute::worker`, no public signature changes
- effort: M
- risk: medium — this is the lane loop that every generation, simulation, optimize and job request rides; a refactor needs the full `-p rs_cam_viz` compute suite green before and after
- sentry: `cargo test -p rs_cam_viz compute::worker::tests::`, `cargo test -p rs_cam_viz -q --test generate_all_fixpoint_parity`, `--test optimize_runs_on_the_job_lane_wp14b`
- owner:

## Top three

1. **SHL-01** (post-config mirror hand-sync) — smallest effort (S), fixes a class of bug that already bit the project once (W9/P-1 grblHAL regression) and has already recurred as a pattern at 4 sites; the fix is a one-line change per call site plus folding into `adopt_post_effects`.
2. **SHL-06** (five duplicated lane-spawn prologues) — medium effort, but the payoff is real: today a bug fix or a new lane-state field (there have been several — `active_toolpath_id`, `active_cancel`, `current_phase`) has to be applied identically five times by hand, and the `spawn_job_lane` comment shows an engineer already had to explain, in prose, why one copy quietly differs from the other four.
3. **SHL-04** (`drain_compute_results` god function) — medium effort, low risk, and the seam is already proven: three of the six `ComputeMessage` arms already delegate to a named method, so extracting the other three is finishing a pattern already half-applied, not inventing one.

## Checked and clear

- Toolpath GPU upload is properly content-addressed through `render/upload_cache.rs::ToolpathUploadKey` (`app/gpu_upload.rs:1074-1089`); cheap resources (stock wireframe, axes, fixtures) are deliberately rebuilt unconditionally per a documented cost/benefit note at `app/gpu_upload.rs:150-157`, and `upload_gpu_data` itself only runs when `take_pending_upload()` is true (`app.rs:804-806`) — not every frame. Not a finding.
- `ComputeMessage` result adoption is one dispatch loop (`controller/events/compute.rs:409-1035`); no per-kind duplicate adoption path exists elsewhere in the group. (The loop's *body* is a god function — see SHL-04 — but the "one path" invariant itself holds.)
- Stale-stamping is one door: `state::stale::stamp_stale`, called from both `apply_controller_command` and `apply_quietly` (`controller/events/mod.rs:66,99`) and from the MCP surface's `core_stale` (`app/mcp/commands.rs:1675-1678`). No narrower recomputation found.
- Metric-capture staleness (`state/simulation/playback_state.rs:160`) is derived from the capture revision, not a mutable stored bool, matching `state/CLAUDE.md`'s invariant.
- `host.rs`'s winit/Wayland wrapper is a well-measured, dated (2026-08-12) design with a named limitation on record, not a design smell.
- The `mcp` Cargo feature's ~54 scattered `cfg(feature = "mcp")` sites looked like dead configuration surface (nothing in the workspace builds `rs_cam_viz` with `--no-default-features`), but `planning/arch_consolidation_2026-09-09/STATUS.md` (WP25/28/29/30) shows `cargo clippy -p rs_cam_viz --no-default-features --all-targets -- -D warnings` is an active, currently-green gate. Not a finding.
- `MultitoolPlannerState` (`state/multitool_planner.rs:125`) mirrors core's `MultitoolPlanSpec` field-by-field with doc comments saying so, but the translation is one-way only (`to_spec()`, called from 2 sites in `controller/events/planner.rs`) and the state is never persisted or reloaded, so it cannot drift the way `GuiState::post` did (SHL-01). Not a finding.
- MCP `export_gcode`'s `accept_previous_geometry` deliberately does NOT read the GUI's `gui.stale_export` toggle (`app/mcp.rs:848-853`, G-STALEXPORT 2026-09-10) — an intentional safety separation (an automation client must not inherit a human's left-on checkbox), not a missed sync.
- `controller/events/simulation.rs:266-269`'s hardcoded `use_predicted_feed_in_gates: false` is a documented, tracked placeholder (labelled "F-036 territory"), not a silent wrong default.
- `planning/structure_2026-09-17/evidence_round2/dead_pub_surface.md`'s viz-shell rows are partly stale: `SimulationRuntimeHotspot`, `ActiveCutSample`, `semantic_runtime_metrics`, `current_cut_sample`, `runtime_hotspots` no longer exist at all in `state/simulation.rs` (now 1014 lines; the doc cites lines 1350-1413, past EOF) — the file was refactored since the evidence ran. `GenerateAllScope`, `OptimizeStageRow` and `ToolpathMoveVisibility` are all live and used from `ui/` call sites outside their defining file (`rg` confirms 3 hits each, not 1) — the doc's single-file appearance was checking too narrow a search. Do not re-cite this doc's viz-shell rows without re-verifying.

## Add-a-thing count

Adding one MCP command/tool (e.g. `add_toolpath`) that also needs no new GUI entry point touches, inside viz-shell alone:

- `crates/rs_cam_viz/src/mcp_server.rs` — the `#[tool(...)]` method: name, description prose, `Parameters<Param>` extraction, `send_request`/`cheap_read` call.
- `crates/rs_cam_viz/src/mcp_bridge.rs` — a new `CoreRequest` enum variant, plus its `CommandId` mapping arm.
- `crates/rs_cam_viz/src/app/mcp/commands.rs` — up to five separate match arms across separate match statements: a pre-effect hook (workspace switch / highlight), the `Command`-building arm, the notification-message arm, and either the shared `CommandId` OR-list or a dedicated per-`CommandId` reply arm (see SHL-02 for the exact `AddToolpath` line list: `:218,632,1616,1758,2154`).

Outside viz-shell but on the same add-a-tool path (named for completeness, not this group's to fix):

- `crates/rs_cam_mcp/src/server.rs` — the `<Name>Param` struct (cli-mcp group).
- `crates/rs_cam_core/src/session/command.rs`'s `for_each_command!` row and the `ProjectSession::apply` arm (core-session group).
- `crates/rs_cam_viz/src/ui/mod.rs`'s `AppEvent` variant, if the GUI should also reach the command directly (viz-ui group) — `controller/events/mod.rs`'s dispatch arm for that event is inside viz-shell.
- `crates/rs_cam_viz/tests/mcp_wire_surface_pin.rs` snapshot re-bless and `crates/rs_cam_core/tests/command_registry_completeness.rs`.
