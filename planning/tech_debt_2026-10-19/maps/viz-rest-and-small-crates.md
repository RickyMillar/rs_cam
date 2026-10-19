# Map: viz-rest-and-small-crates

Evidence baseline: HEAD 99b6fb41. Read-only review. Paths marked **(in flux)** are in
`instruments/in_flux_paths.txt`. All cites are `path:line` from the working tree at read time.
Line counts are approximate. `V` = `crates/rs_cam_viz/src`.

Scope: `V/{state,compute,render,io,interaction,bin}/`, the viz root files, and the crates
`rs_cam_cli`, `rs_cam_mcp`, `rs_cam_mcp_proxy`, `rs_cam_derive`. Not mapped here (other maps):
`V/app/`, `V/controller/`, `V/ui/`. They appear only where this area calls into them.

## 1. Purpose and file map

**Purpose.** This area is the non-widget half of the desktop application plus the three small
satellite crates. `state/` holds GUI view state and derives freshness from the core answer.
`compute/` runs the four background lanes (Toolpath, Analysis, Reach, Job) and carries the
memory-budget ledger. `render/` holds the wgpu pipelines and the GPU upload keys. `io/` wraps
core import and export for the GUI and MCP. The root files host the winit event loop, the
embedded MCP server, the MCP-to-GUI bridge and the view command registry. `rs_cam_cli` is the
batch binary. `rs_cam_mcp` holds MCP parameter structs (and some logic). `rs_cam_mcp_proxy` is a
std-only stdio supervisor. `rs_cam_derive` is a proc-macro crate used only by core.

### 1a. viz `state/` (GUI state; CLAUDE.md: `V/state/CLAUDE.md`)

| File | Responsibility | Lines |
|---|---|---|
| `state/mod.rs` | `AppState`, `Workspace` (+`ALL`/`order`), `JobRun`, `PanelSideEffects`, modal state, `unsaved_project_summary`, `close_modals_for_exclusivity` | 549 |
| `state/freshness.rs` | `FreshnessState` (per toolpath) and `SimFreshness` (simulation); derived, never stored | 303 |
| `state/stale.rs` | `stamp_stale`: the one place `stale_since` is stamped from `Effects::stale` | 61 |
| `state/runtime.rs` | `GuiState`, `ToolpathRuntime`, `ReachOverlayState`, `StaleResultPolicy`, `ToolLoadOverrides` | 439 |
| `state/job.rs` | re-exports of core ids/configs, `SetupFrame` (wrapper over core `SetupTransformInfo`), display helpers | 688 (half tests) |
| `state/toolpath.rs` + `toolpath/{catalog,configs,support}.rs` | pure re-export facades of `rs_cam_core::compute::*` | 27+7+12+9 |
| `state/toolpath/entry.rs` **(in flux)** | `ToolpathEntry`, `ToolpathEntryInit`, `ToolpathResult` | 381 |
| `state/simulation.rs` | `SimulationState`, cut-metric cards, caches, playback, checks, checkpoints | 1564 |
| `state/simulation/{playback_state,issue_triage,semantic_trace}.rs` | playback state, bounded triage, semantic index | 436/524/362 |
| `state/simulation/tests.rs` **(in flux)** | tests of the above | 1053 |
| `state/viewport.rs` | `ViewportState`, `ToolpathDrawFilter`, `toolpaths_to_draw` (WP27 draw rule) | 336 |
| `state/overlays.rs` | `OverlayPanelState`, `DockSection` (viewport dock state) | 182 |
| `state/selection.rs` | `Selection` enum | 23 |
| `state/history.rs` **(in flux)** | `UndoAction`, `UndoHistory` | 396 |
| `state/panels.rs` | `PanelDrafts`: typed draft state of panels (UI-09) | 237 |
| `state/wizard.rs` | `WizardState`, `OutputLayout`, `ControllerOption` (export wizard) | 190 |
| `state/multitool_planner.rs` | planner dialog state | 559 |
| `state/app_settings.rs` | applies `AppSettings` to `AppState` | 170 |
| `state/pin_view.rs` | pin display frame, pin colours (C8) | 111 |
| `state/shoe_plan.rs` | memo of the core shoe plan, run sheet text (H2) | 250 |

### 1b. viz `compute/`, `render/`, `io/`, `interaction/`, `bin/`

| File | Responsibility | Lines |
|---|---|---|
| `compute/mod.rs` | `ComputeBackend`, `LaneControl`, `MemoryStatusSource` traits; `ComputeLane`, `LaneSnapshot`, `ComputeError`, `ComputeMessage`, `JobRequestId`, `ToolpathSubmitOutcome` | 621 |
| `compute/worker.rs` | request/result structs, `ArtifactPolicy`, `JobBudget`, `BudgetLedger`, `LaneQueue`, `ThreadedComputeBackend`, four `spawn_*_lane` fns | 1904 |
| `compute/worker/execute/mod.rs` | `run_compute*`, `run_simulation_with_phase`, `build_playback_data`, cut-trace file writers | 641 |
| `compute/worker/helpers.rs` | collision-check and trace artefact builders | 144 |
| `compute/worker/{tests,test_fixture,gen_parity_p0_tests}.rs` | tests; the last two **(in flux)** | 3475/336/438 |
| `render/mod.rs` | `RenderResources`, `ViewportCallback` (`egui_wgpu::CallbackTrait`), all WGSL shader strings | 1610 |
| `render/toolpath_render.rs` | toolpath line pipeline, entry preview, chipload colours | 1230 |
| `render/sim_render.rs` | simulated stock, tool ghost, `head_draw_parts` | 1144 |
| `render/upload_cache.rs` | content-addressed upload keys, `ArcId`, `refresh_key` | 625 |
| `render/{mesh,stock,fixture,grid}_render.rs`, `height_planes.rs` | other pipelines | 340/310/221/165/113 |
| `render/camera.rs`, `colors.rs`, `gpu_safety.rs` | camera; colour constants; buffer-size guard | 281/83/48 |
| `io/export.rs` | G-code export doors (single, per setup, combined, single toolpath), refusals, safety pass | 1036 |
| `io/setup_sheet.rs` | HTML setup sheet | 450 |
| `io/import.rs` | thin wrappers over `rs_cam_core::io::load_model_file`, map to `VizError` | 58 |
| `io/app_settings.rs` | re-export of core settings loader | 17 |
| `interaction/picking.rs` **(in flux)** | `pick`, `PickHit`, `PickContext`, pin/hole/toolpath picks | 426 |
| `bin/main.rs` | `rs_cam_gui` entry: tracing setup, panic hook, Wayland warning | 191 |

### 1c. viz root files

| File | Responsibility | Lines |
|---|---|---|
| `lib.rs` | `run()` builds the winit loop; `GuiWaker`; module list; `#![deny(unwrap_used, panic, indexing_slicing)]` | 228 |
| `host.rs` | `RsCamHost`/`RsCamAppProxy`: owns the winit `ApplicationHandler`, drains MCP from `about_to_wait` | 327 |
| `mcp_server.rs` | `EmbeddedCamServer`: 92 `#[tool]` methods, each sends an `McpRequestKind` and awaits JSON | 1868 |
| `mcp_bridge.rs` | `McpRequestKind`, `CoreRequest` (macro `declare_core_requests!`), `FrameLoopBeat`, `McpReadCache`, response builders | 2057 |
| `mcp_lifecycle.rs` | `McpExitSignal`, watchdog, completion guard | 357 |
| `ui_command.rs` | view registry `for_each_ui_command!` (64 rows), `UiCommand`/`UiQuery`/`UiCommandId`/`SurfaceId` | 1067 |
| `present_mode.rs` | present-mode decision, negotiated-mode capture layer | 591 |
| `error.rs` | `VizError` | 61 |
| `app.rs`, `controller.rs` | shells owned by other maps | 1464 / 746 |

### 1d. Small crates

| Path | Responsibility | Lines |
|---|---|---|
| `rs_cam_cli/src/main.rs` | clap `Cli`/`Commands` (Version, Job, Run, Sweep, Project, Smoke, NcTime, RoughScore), tracing init | 730 |
| `rs_cam_cli/src/command.rs` | `apply_command`: thin door to `ProjectSession::apply` | 70 |
| `rs_cam_cli/src/job.rs` **(in flux)** | TOML job file parse and execute | 1586 |
| `rs_cam_cli/src/project.rs` | GUI project file diagnostic executor, `run_generation_plan` | 1622 |
| `rs_cam_cli/src/smoke.rs` **(in flux)** | smoke baseline runner and diff | 1488 |
| `rs_cam_cli/src/run.rs` **(in flux)** | generic `run <op>` through the registry | 322 |
| `rs_cam_cli/src/rough_score.rs` **(in flux)** | `rough-score` JSON scorer | 779 |
| `rs_cam_cli/src/sweep.rs`, `nc_replay.rs`, `memory_budget.rs` | param sweep; G-code replay; `--memory-limit` | 670/544/108 |
| `rs_cam_cli/examples/{migrate_ap_rule,apply_suggest_save}.rs` | one-off tools | 297/159 |
| `rs_cam_mcp/src/server.rs` | 72 param structs, 12 wire enums, plus parse/build helpers | 2857 |
| `rs_cam_mcp/src/response.rs` | response caps (`MAX_RESPONSE_BYTES`, `DEFAULT_MAX_*`), `ResponseBudget`, `CappedArray` | 822 |
| `rs_cam_mcp_proxy/src/proxy.rs` | routing, handshake replay, child lifecycle | 1408 |
| `rs_cam_mcp_proxy/src/{procinfo,config,tools,log,main}.rs`, `bin/rs_cam_mcp_fake_child.rs` | `/proc` reads and signals; args; tool schemas; log; entry; test fixture | 306/112/80/63/36/186 |
| `rs_cam_derive/src/lib.rs` | `#[derive(OpParams)]`, `#[derive(ParamEnum)]` | 433 |

## 2. Public entry points and external callers

| Entry point | Defined | Callers outside the area (path:line, counts) |
|---|---|---|
| `AppState` | `V/state/mod.rs:133` | 36 files in `V/{ui,app,controller}`; 24 integration tests |
| `stamp_stale` | `V/state/stale.rs:46` | 13 prod sites in controller/app, e.g. `V/controller/events/compute.rs:617`, `V/app/mcp/commands.rs:1882` |
| `freshness`, `freshness_at`, `simulation_freshness` | `V/state/freshness.rs:~85-250` | 20 files (ui/app/controller), 7 tests; `FreshnessState` in 15 src files, `SimFreshness` in 12 |
| `ComputeBackend` trait | `V/compute/mod.rs:478` | `AppController<B: ComputeBackend>` at `V/controller.rs:120`; 31 impls (see section 4) |
| `ThreadedComputeBackend` | `V/compute/worker.rs:1033` | `V/app.rs:237`, `V/controller.rs:247` only |
| `GenerationControl`, `MemoryControl` | `V/compute/mod.rs:131,241` | `V/mcp_server.rs:15` (MCP thread), tests/mcp_escape_hatches.rs |
| `io::export::*` | `V/io/export.rs` | `V/app/export.rs:17,31,140,176`; `V/app/mcp.rs:883,897,934,950,961`; 11 tests |
| `io::setup_sheet::generate_setup_sheet_from_session` | `V/io/setup_sheet.rs:69` | `V/controller/io.rs:547` (1 caller) |
| `io::import::import_*` | `V/io/import.rs:27-58` | `V/controller/io.rs:90,112,127,142,182,238` |
| `interaction::picking::pick` | `V/interaction/picking.rs:76` | `V/app/viewport.rs:35` (`PickContext` at :21, :323) |
| `render::{RenderResources, ViewportCallback}` | `V/render/mod.rs:124,846` | `V/app/viewport.rs:1,465`; `V/app/gpu_upload.rs:6`; `V/app/simulation.rs:3` |
| `render::upload_cache::refresh_key` | `V/render/upload_cache.rs:303` | `V/app/gpu_upload.rs:278,280,692,1270,1334,1382,1439` (the GPU upload pass lives in `app/`, not `render/`) |
| `UiCommand`, `UiQuery`, `SurfaceId` | `V/ui_command.rs` | 56 uses in `V/controller/events/mod.rs`; 28 in `V/app/input.rs`; 22 in `V/app/mcp.rs`; 18 in `V/mcp_server.rs` |
| `McpRequestKind`, `CoreRequest` | `V/mcp_bridge.rs:500`, `:~735` | built by `V/mcp_server.rs`, consumed by `V/app/mcp.rs`, `V/app/mcp/commands.rs` |
| `rs_cam_viz::run`, `io::app_settings::load` | `V/lib.rs:48`, `V/io/app_settings.rs:14` | `V/bin/main.rs:21,28,52` |
| `rs_cam_mcp::server::*` params | `rs_cam_mcp/src/server.rs` | `V/mcp_server.rs:23-39`, `V/mcp_bridge.rs:16`, `V/app/mcp/**` |
| `rs_cam_mcp::response::*` caps | `rs_cam_mcp/src/response.rs` | `V/mcp_bridge.rs:11` (`CutTraceCaps`), `V/ui_command.rs` (`GetCutTraceArgs.caps`) |
| `rs_cam_derive::{OpParams, ParamEnum}` | `rs_cam_derive/src/lib.rs:45,54` | 33 derive uses, all in `rs_cam_core/src/compute/operation_configs.rs`, `catalog/schema.rs`, `catalog/registry.rs`, `catalog/tests.rs` |
| `rs_cam_cli` | binary only (`main.rs`) | no `[lib]`; `command.rs:7-8` says an integration test cannot reach it |
| `rs_cam_mcp_proxy` | binary `rs_cam_mcp_proxy`; `proxy::run` | started by `.mcp.json` (`rs-cam-capped`); no Rust caller |

## 3. Dependencies and layering

Imports of `crate::<folder>` per folder (rg count, non-test and test lines mixed):

| From | Imports |
|---|---|
| `state/` | `state` 22, `ui` 17, `compute` 4, `render` 1 |
| `compute/` | `compute` 20, `state` 26, `io` 4, `controller` 3 (tests only), `ui` 1 (tests only) |
| `render/` | `state` 7, `ui` 2, `render` 1 |
| `io/` | `error` 24, `state` 14, `ui` 1 |
| `interaction/` | `state` 7, `render` 1 |

Crate edges: `rs_cam_viz -> rs_cam_core` (+`rs_cam_mcp` optional, `rmcp`, `tokio`). `rs_cam_cli -> rs_cam_core`
only (`rs_cam_cli/Cargo.toml`). `rs_cam_mcp -> rs_cam_core, rmcp`. `rs_cam_mcp_proxy -> serde_json` only
(`rs_cam_mcp_proxy/Cargo.toml`). `rs_cam_derive -> syn/quote/proc-macro2`; the generated code names
`crate::compute::catalog::…`, so it works only inside core (`rs_cam_derive/src/lib.rs:10-12`).

**Layering inversions (lower layer imports upper):**

| Site | Inversion |
|---|---|
| `V/state/mod.rs:198,535` | `AppState.preferences: Option<crate::ui::preferences::PreferencesState>` (state holds a ui type and builds it) |
| `V/state/panels.rs:176,183` | `PanelDrafts` holds `ui::properties::head::PartList` and `ui::pin_wizard::PinWizardState` |
| `V/state/runtime.rs:300` | `GuiState.pending_toolpath_tab` holds `ui::properties::ToolpathTab` |
| `V/state/app_settings.rs:79` | calls `crate::ui::overlays::registry::apply_workspace_defaults` |
| `V/state/pin_view.rs:82-84,101` | state colours come from `ui::tokens` (and `egui::Color32` in state) |
| `V/io/setup_sheet.rs:57` | `io` imports `ui::readiness::{self, CycleTimeBasis}` |
| `V/render/mod.rs:1071` | `render` reads `ui::tokens::SURFACE_SUNKEN` for a clear colour |
| `V/render/upload_cache.rs:~57` | upload keys import `state::viewport::{SpanKindFilter, ToolpathColorMode}` (render depends on state; expected, but the key is a copy of state) |
| `V/ui_command.rs:50-54` | root registry imports `render::camera::ViewPreset`, `state::*` |
| `V/state/runtime.rs:283` | `#[cfg(feature = "mcp")] mcp_highlights` lives in GUI state: MCP concern inside state |
| `V/mcp_bridge.rs:17-18` | bridge imports `crate::state::Workspace`, `ui_command` (acceptable), but `V/ui_command.rs:293` gates a field on `feature = "mcp"` so the view registry is feature-dependent |

`compute/` contains no `ui` import outside tests, and `render/` does not import `controller` or `app`.
Core independence is held: `rs_cam_core` does not import viz (Cargo manifests).

## 4. Traits

| Trait | Defined | Implementors | Notes |
|---|---|---|---|
| `ComputeBackend` | `V/compute/mod.rs:478` | prod: `ThreadedComputeBackend` (`V/compute/worker.rs:1187`). Test doubles: 30, e.g. `V/controller/tests/mod.rs:127,572,632,928`, `V/controller/workflow_tests.rs:47`, `V/controller/results_parity_tests.rs:49`, `V/compute/worker/gen_parity_p0_tests.rs:102`, `V/app/mcp/tests.rs:844`, and 17 in `V/../tests/*.rs` (18 types named `Silent*/Idle*/Inert*/Scripted*Backend`) | Six defaulted methods are silent no-ops (`submit_job` :492, `submit_reach_map` :500, `clear_sim_prefix_cache` :518, `set_memory_budget` :536, `memory_reserved_bytes` :545, `preflight_simulation` :570). `lane_snapshots` returns `[LaneSnapshot; 4]` (:590): a new `ComputeLane` variant does not force an edit there or in `cancel_all` (:577). |
| `LaneControl` | `V/compute/mod.rs:136` | `LaneQueue<ComputeRequest>` (`worker.rs:975`), `DetachedLane` (`mod.rs:145`), 5 test stubs in `tests/mcp_escape_hatches.rs:76,233,346,593,709` | |
| `MemoryStatusSource` | `V/compute/mod.rs:218` | `LedgerMemoryStatus` (`worker.rs:509`), `DetachedMemory` (`mod.rs:226`), `FixedBudget` test (`tests/mcp_escape_hatches.rs:749`) | |
| `rs_cam_core::budget::UsageProbe` (from core) | core | `SharedProbe` (`worker.rs:527`), `FixedProbe`/`RaisableProbe` tests (`worker/tests.rs:2798,2853`) | |
| `rs_cam_core::trace::debug_trace::ToolpathPhaseSink` (core) | core | `ToolpathPhaseTracker` (`worker.rs:1027`) | |
| `egui_wgpu::CallbackTrait` (ext) | egui_wgpu | `ViewportCallback` (`render/mod.rs:928`) | `prepare` uses 3 `unwrap()` under `allow` with `// SAFETY:` (`render/mod.rs:938,1047,1050`) |
| `eframe::App` (ext) | eframe | `RsCamApp` (`V/app.rs:849`), `RsCamAppProxy` (`V/host.rs:105`) | |
| `winit::ApplicationHandler` (ext) | winit | `RsCamHost` (`V/host.rs:225`) | |
| `rmcp::ServerHandler` (ext) | rmcp | `EmbeddedCamServer` (`V/mcp_server.rs:1860`) | tool list built by `#[tool_router]` macro |
| `tracing_subscriber::Layer` (ext) | ext | `NegotiationCapture` (`V/present_mode.rs:382`) | |
| `ComputeError` | enum, not trait | `From<String>`, `From<StopReason>`, `From<SimulationError>`, `From<OperationError>` | `Message(String)` is the catch-all arm |

Defined elsewhere in viz and not mapped here: `CycleTimeBasisExt`, `MissingInputExt` (`V/ui/readiness.rs:368,414`), `UiExt` (`V/ui/components/section.rs:21`).
`rs_cam_cli`, `rs_cam_mcp`, `rs_cam_mcp_proxy`, `rs_cam_derive` define no traits.
Traits deliberately not used: no `Serialize`-based wire types in viz root (replies are `String` JSON built by hand); no `OpParams` use in viz (it reads core's schema); CLI declares no trait and no `[lib]`.

## 5. Recurring-pattern table

Grades: TYPES | LINT | SENTRY | DOC | CONVENTION | NONE.

| Concern | Pattern used here | Example | Grade | Deviations inside the area |
|---|---|---|---|---|
| Errors and refusals (viz) | `VizError` (thiserror) at the io/controller layer; `ComputeError {Cancelled, OverBudget, Message(String)}` for lanes; refusal sentences as `String` built by helper fns | `V/error.rs:8`, `V/compute/mod.rs:270`, `V/compute/mod.rs:297,314` | TYPES for the enums; DOC for the message wording | `VizError::{StlImport,SvgImport,DxfImport,StepImport,ProjectSave}` are never constructed (rg: 0 uses outside `error.rs`); `io/import.rs:15` maps every core error to `VizError::Other(String)`, while `From<SessionError>` maps to `ProjectLoad` (`error.rs:57`). `VizError::Other` doc says "incremental migration". `V/mcp_server.rs:402` formats errors as `{"error": String}`. `ComputeError::Message(String)` absorbs `OperationError` variants other than 2 (`compute/mod.rs:445`). |
| Errors (CLI) | `anyhow::Result` everywhere, `bail!`/`with_context`; clap parse errors | `rs_cam_cli/src/run.rs:271-317` | CONVENTION | `smoke.rs:1074` returns `Option`; `memory_budget.rs:35` returns `Result<_, String>` |
| Errors (mcp, proxy) | `Result<_, String>` for parse/build helpers; proxy returns `Result<_, String>` and JSON `Value` errors | `rs_cam_mcp/src/server.rs:1659,1674,1753,1837`; `rs_cam_mcp_proxy/src/config.rs:27`, `procinfo.rs:131,158` | NONE (stringly) | `rs_cam_mcp_proxy/src/proxy.rs:1064` returns `Result<Value, Value>` |
| Panics and unwrap | `#![deny(clippy::unwrap_used, panic, indexing_slicing)]` on `lib.rs` and CLI `main.rs`; module-level `deny(indexing_slicing)` on `compute/`, `io/`, `interaction/`, `app.rs`, `controller.rs`; tests use per-module `#[allow]` | `V/lib.rs:1`, `V/compute/mod.rs:1`, `rs_cam_cli/src/main.rs:1` | LINT | `render/`, `state/` and `mcp_*` carry no module deny (only the crate-level deny from `lib.rs:1` applies for unwrap/panic; indexing is allowed with reason comments). `render/mod.rs:938,1047,1050,1392` and `compute/worker.rs:922` use `allow(unwrap/expect)` with `// SAFETY:`. Many `allow(indexing_slicing)` carry a trailing reason but no `// SAFETY:` line (listed in `instruments/allows_without_safety.md`, e.g. `render/sim_render.rs:81,163,256,408`, `render/mesh_render.rs:66,151,222,259`, `render/toolpath_render.rs:126,153,450,604,849,1006`). |
| State mutation | All project writes go through `ProjectSession::apply(Command)`. Core `*_mut` hatches are `pub(crate)`. Draw sites never mutate core directly. | `rs_cam_core/src/session/mod.rs:1756,1809,1827` (crate-private); `V/state/stale.rs:46`; `rs_cam_cli/src/command.rs:31` | TYPES (visibility) + SENTRY (source scan) | The scan only sees receivers spelled `session.` (`tests/production_writes_go_through_apply_wp15a.rs:42-46`). Direct `.apply(` bypasses the CLI door at `rs_cam_cli/src/project.rs:309` and `smoke.rs:751`. `GuiState.post: PostConfig` is a manual MIRROR of `session.post_config()` (clone synced by hand: `V/controller/io.rs:366-394,433`, `V/controller/events/undo.rs:20-21,92-93`, `V/app/mcp/commands.rs:520`); export reads the mirror (`V/io/export.rs:604,632,642`). |
| Freshness and invalidation | Freshness is DERIVED on every read from the core result cache plus lane status; never stored. `stale_since` only drives the 500 ms auto-regen debounce. | `V/state/freshness.rs:77-95,247`, `V/state/mod.rs:450` | TYPES (enums, exhaustive `label()`) + SENTRY (`freshness_surfaces_g_freshrender`, `effects_are_stamped_wp19`) | `GuiState::toolpath_rt_or_default` creates a row with `auto_regen = true` (`V/state/runtime.rs:353-356`), but `stamp_stale` doc says a hardcoded `true` would queue 3D passes (`V/state/stale.rs:40-45`); the former is called at `V/controller/events/compute.rs:410` to set `stale_since`. `edit_counter` still drives generate-all and `submitted_edit_counter` (`V/controller/events/compute.rs:1712,1742`, `V/controller/events/simulation.rs:441`) while `state/CLAUDE.md` says "never the counter". |
| Result revision and epoch stamps | Toolpath result carries the generation-input revision; simulation and collision carry a simulation epoch; controller rejects mismatches. | `V/compute/worker.rs:188-195` | DOC + SENTRY (`compute::worker::tests::`) | `ComputeResult.revision: Option<u64>` where `None` means "not stamped" and is adopted at current revision (`worker.rs:189-195`): the type allows the unstamped path in production. |
| Parameters and config | Operation params come from core's `OpParams` derive (33 uses, all in core). Viz re-exports core config types without owning any. MCP sets params by `(index, "name", serde_json::Value)`. | `V/state/toolpath/configs.rs:1-12`, `rs_cam_derive/src/lib.rs:45`; `rs_cam_cli/src/command.rs:46-52` | TYPES (derive; compiler refuses a field without `ParamType`) | `SetToolpathParamArgs.param: String` is the entry for every write (CLI `job.rs`, `run.rs`, `smoke.rs`; MCP `SetToolpathParamInput`). CLI guesses JSON type from text (`rs_cam_cli/src/job.rs:1272 param_value_from_str`). |
| Units and frames | Bare `f64` millimetres for CAM values; `f32`/nalgebra for GPU; frames named by doc: world, stock-relative, setup-local, display. `SetupFrame` wraps core `SetupTransformInfo`. | `V/state/job.rs:45-160`, `V/compute/worker.rs:247-286` (`PlaybackToolpath.frame: Option<SetupTransformInfo>`), `V/state/pin_view.rs:57` | CONVENTION | No newtype for mm, radians, frame. `render/camera.rs`, `render/*` convert `f64 -> f32` by casts. Display-vs-emission shifts via `SetupFrame::emission_to_display_shift` (`state/job.rs:119`), `pin_display_point` (`state/pin_view.rs:57`), `hole_display_point` (`interaction/picking.rs:290`): three helpers for one idea. `PIN_MARK_LIFT_MM=0.1` (`pin_view.rs:21`) and `HOLE_MARK_LIFT_MM=0.05` (`picking.rs:277`) are two separate lifts. |
| Identifiers | Core newtypes `ToolpathId`, `ModelId`, `SetupId`, `ToolId`, `ObstacleId` (`rs_cam_core::ids`), re-exported in `state/job.rs:13`. Indexes are bare `usize`. | `V/state/job.rs:13`, `V/compute/worker.rs:98-110` | TYPES for ids; NONE for index vs id | `LaneSnapshot.active_toolpath_id: Option<usize>` (`V/compute/mod.rs:71`) and `active_toolpath_index: Option<usize>` (:79): the field named id is untyped. `io/import.rs:27-58` takes `id: usize`; callers pass `model_id.0` (`V/controller/io.rs:182,238`). `GetCutTraceArgs.toolpath_id: Option<usize>` (`ui_command.rs:293`) and `McpRequestKind::* { index: usize }` mix id and index in one type. `ComputeRequest` carries both `handle` index and `VizExtras.toolpath_id` (`worker.rs:86-110`). In `rs_cam_mcp/src/server.rs` 43 fields are `*_index: usize`. |
| Background work, progress, cancellation | Four lanes, each a worker thread + `LaneQueue`. Cancel = per-submit `Arc<AtomicBool>`; budget stop = `BudgetGuard` sets the same flag, lane maps to `ComputeError::OverBudget`. Phase string via `ToolpathPhaseSink`. | `V/compute/worker.rs:1469,1582,1715,1813` (`spawn_*_lane`), `:841` (`LaneQueue`), `:581` (`BudgetLedger`) | DOC + SENTRY | Submit rules differ per lane (toolpath: resubmit cancels and requeues; analysis: latest wins; job: FIFO; reach: own lane) and live only in doc comments (`compute/mod.rs:24-52`). MCP progress is a separate channel `ProgressUpdate` (`mcp_bridge.rs:482`). |
| Memory budget | `JobBudget` shares one cell across lanes; `BudgetLedger` admits heavy jobs by estimate. | `V/compute/worker.rs:416-500,581-725` | SENTRY (`compute::worker::tests::`) | `settings_file_text()` / `cell_text_up` in `compute/mod.rs:345-358` build user sentences with hand-coded `PER_MM = 1000.0` rounding. |
| Caching | Content-addressed upload keys (`*UploadKey`), `Weak`-pinned `ArcId`, `refresh_key`; `ShoePlanMemo` keyed by hash; cut-metric and triage caches keyed by weak pointers. | `V/render/upload_cache.rs:73,303`, `V/state/shoe_plan.rs:77`, `V/state/simulation.rs:148,491,538` | SENTRY (unit tests in `upload_cache.rs:354+`) + DOC | Each key must list every input by hand ("adding a new input to a builder means adding it here", `upload_cache.rs` module doc); no compiler link between a builder's reads and its key. `refresh_key` is called only from `app/gpu_upload.rs`; `ToolpathRenderData.upload_key` is a field set in `toolpath_render.rs:383,574,730`. |
| Diagnostics and user messages | Messages are `String` built at the point of refusal; severities via `controller::Severity`; MCP errors `{"error": …}`. Cut-trace figures use `Option` so null = NOT MEASURED. | `V/compute/mod.rs:297-343`, `V/io/export.rs:60-190` | DOC | Same fact is phrased in several places: export refusals (`io/export.rs:441,505`), blocking message vs `blocking_refusal_text`, `unsaved_project_summary` (`state/mod.rs:100`) has an embedded run of spaces in a string literal (`state/mod.rs:107`). |
| Serialization and wire types | `rs_cam_mcp` structs: `serde::Deserialize + schemars::JsonSchema + Default`; replies are `String` JSON (`json_str`, `format_result`). View registry payloads are plain Rust structs, not serde. | `rs_cam_mcp/src/server.rs:25-34`, `V/mcp_server.rs:402`, `V/ui_command.rs:69-330` | DOC (CLAUDE.md "fixed token set takes an enum") + SENTRY (`tests/mcp_wire_surface_pin.rs`, `mcp_core_arm_describes_every_row.rs`) | Enum rule broken by fixed-token `String` fields: `face_up` (`server.rs:33`), `operation_type` (:199, :466, :646), `kind` (:603), `tool_type` (:668), boundary `source` (:832), spindle `strategy` (:1218), `scope` (:1242). `SetUiViewArgs.workspace/properties_tab/select/modal: Option<String>` (`ui_command.rs:180-186`) while `Workspace::ALL` exists. `McpRequestKind::ApplyFeeds{scope:String}`, `GetOperationSchema{operation_type:String}` (`mcp_bridge.rs:~520,~540`). 13 `schemars(inline)` uses vs 12 enums in `server.rs`. |
| Wire caps | Named constants in `response.rs`; `ResponseBudget` charges bytes | `rs_cam_mcp/src/response.rs:50-113` | DOC (CLAUDE.md "never a number in a handler") | `V/mcp_server.rs:61` `BUSY_READ_DEADLINE = 750 ms` and `mcp_bridge.rs:85 PARKED_FRAME_LOOP = 2 s` are deadlines, not caps; no sentry found for numeric literals in handlers. |
| Tolerances and epsilons | Local literals `1e-6`, `1e-9`, `1e-10`, `f64::EPSILON`; named consts only in a few places. | `V/io/import.rs:19` (`1e-9`), `V/render/toolpath_render.rs:50,194,883,955`, `V/render/fixture_render.rs:113`, `V/interaction/picking.rs:412` (`eps = 1e-4`), `V/render/camera.rs:121,195`; CLI `rs_cam_cli/src/rough_score.rs:53,56` (`TIME_EPS_S`, `TOP_Z_EPS_MM`), `smoke.rs:372` (`NUMERIC_DRIFT_REL_TOL=0.05`) | NONE | No shared tolerance module in viz; pick thresholds are named (`picking.rs:14,17`). |
| Geometry primitives | Core `P2/P3/V3/BoundingBox3/Polygon2`; render uses `[f32;3]` arrays and `nalgebra::{Point3,Matrix4}<f32>`; picking mixes both. | `V/interaction/picking.rs:6`, `V/render/camera.rs:20-70` | CONVENTION | `PickHit::StockFace{face_normal:[f32;3]}` (`picking.rs:30`); `determine_face_normal` returns `[f32;3]` (`:411`). |
| Colours | `render/colors.rs` holds `[f32;3]` constants; UI colours are `ui::tokens` `Color32`. | `V/render/colors.rs:9-83`, `V/state/pin_view.rs:80-101` | DOC (`render/CLAUDE.md` "a colour belongs in colors.rs") | Literal colours in pipelines: `render/toolpath_render.rs:228-230,471,628,760-779,822,839,1008`, `render/mesh_render.rs:226`. Two palettes (`colors.rs` and `ui/tokens`). `render/colors.rs:44 COLLISION_POINT` flagged dead in `instruments/dead_pub_surface.md:15` though CLAUDE.md names it. |
| Test style and sentries | Unit tests inline; one integration file per gate with a gate-id suffix (`_g_xxx`, `_wpNN`, `_p2_6`, `_up3`); source-scanning sentries with non-vacuity anchors; 31 hand-written `ComputeBackend` doubles. | `V/../tests/*.rs` (112 files), `tests/production_writes_go_through_apply_wp15a.rs:1-50` | SENTRY | No shared test backend crate/module: `SilentBackend` re-declared in 12+ test files (e.g. `tests/apply_contract_a3.rs:57`, `tests/feeds_apply_drops_result_n13.rs:75`, `tests/a_click_picks_the_drawn_item_p2_6.rs:71`). `rs_cam_derive` has no tests and no trybuild. `rs_cam_mcp` has no `tests/` dir. `rs_cam_cli` cannot be tested from outside (`command.rs:8`). |
| Naming and layout | One concept per file; `mod.rs` facades; `*_tests.rs` or `tests/` sub-modules; CLAUDE.md per folder. | `V/state/toolpath.rs:1-27` | CONVENTION | `state/toolpath/{catalog,configs,support}.rs` are 7-12 line re-export files; `state/job.rs` mixes re-exports, `SetupFrame`, mesh helpers and 300 lines of tests; `compute/worker.rs` holds requests, budget ledger, queue, backend and four lane bodies (1904 lines). |
| Registering a new feature | Registry macros plus exhaustive matches: core `for_each_command!`, view `for_each_ui_command!`, `declare_core_requests!`, `Workspace::order()`, `UiCommandId`. | `V/ui_command.rs:340-840`, `V/mcp_bridge.rs:735-900`, `V/state/mod.rs:46-55` | TYPES (exhaustive match, generated enums) + SENTRY (`command_surface_completeness.rs`, `mcp_core_arm_describes_every_row.rs`) | `Workspace::ALL: [Workspace;4]` (`state/mod.rs:32`) is a hand list backed by `order()` + sentry. `close_modals_for_exclusivity` (`state/mod.rs:479`) lists modals by hand; each modal is a separate `bool`/`Option` field, so a new modal is not forced into the list. `McpReadKind` + `McpReadSnapshot` (`mcp_bridge.rs:33-70`) pair by hand. |

## 6. Extension points

| New item | Files touched | What the compiler forces |
|---|---|---|
| New core command row (MCP tool that mutates) | core `session/command.rs` (`for_each_command!` row) and handler; `rs_cam_mcp/src/server.rs` (param struct); `V/mcp_bridge.rs` (`declare_core_requests!` row with workspace); `V/mcp_server.rs` (`#[tool]` method); `V/app/mcp/commands.rs` (`core_command_for` arm, describe arm, toast/refusal); `FEATURE_CATALOG.md` | Describe step matches on `CoreRequest::id()` (`mcp_bridge.rs:~750`), so a missing arm does not compile. `#[tool]` registration is NOT forced: sentries `command_surface_completeness.rs`, `mcp_core_arm_describes_every_row.rs`, `mcp_wire_surface_pin.rs` catch it. |
| New view command or view read | `V/ui_command.rs` (row + payload type); handler arm in `V/controller/events/mod.rs` (UiCommand) or `V/app/mcp.rs` `ui_query` (UiQuery); a GUI constructor in `V/ui/**` if `gui: Reached`; MCP `#[tool]` if `mcp: Reached` | Generated `UiCommand`/`UiQuery` enums make the dispatch match non-exhaustive until added. A `Reached` row with no constructor fails only the census sentry (`command_surface_completeness.rs`). |
| New operation type | core `compute/catalog/registry.rs`, `operation_configs.rs` (`#[derive(OpParams)]`), generation code; CLI picks it up automatically (`rs_cam_cli/src/run.rs:271` iterates `OperationType::ALL`); `V/ui/properties/operations/*`; MCP `parse_operation_type` (`rs_cam_mcp/src/server.rs:1659`); `FEATURE_CATALOG.md` | `OperationType` matches in core are exhaustive. In this area: `state/toolpath/configs.rs:1-12` and `catalog.rs:1-7` hand-list re-exports (a new config type is not exported until added; compile error only if used by name). `smoke.rs:1074` and `run.rs:271` iterate `ALL`, so no edit. |
| New op parameter | the config struct field in core (`#[param(...)]` attribute); nothing in viz/CLI/MCP | The derive writes the schema; CLI `job` refuses keys not in the schema (`rs_cam_cli/CLAUDE.md` C2). A field without `skip`/`driver_set` must implement `ParamType` (compiler). |
| New compute lane | `V/compute/mod.rs` (`ComputeLane` variant, `LaneState`), `V/compute/worker.rs` (new `spawn_*_lane`, `ThreadedComputeBackend` fields, `submit_*`, `cancel_lane`, `lane_snapshot`, `drain_results`), `ComputeMessage` variant, `V/controller/events/*` drain arm | `match` on `ComputeLane` and `ComputeMessage` are exhaustive. NOT forced: `ComputeBackend::cancel_all` (`mod.rs:577`) and `lane_snapshots -> [_;4]` (`mod.rs:590`) are hand lists; defaulted `submit_*` hide a missing override. |
| New toolpath-adjacent GUI state field | `V/state/runtime.rs` or `state/mod.rs`; audit setup sheet (`io/setup_sheet.rs`), project IO (`controller/io.rs`), controller fixtures, MCP and UI initialisers (CLAUDE.md `rs_cam_viz/CLAUDE.md` GUI contracts) | Struct literal initialisers force an edit where `AppState`/`GuiState` are built with all fields (`GuiState::new` at `runtime.rs:322`). Many tests use `..Default`; not forced. |
| New freshness state | `V/state/freshness.rs` enum + `label()`; `SimFreshness::is_stale`; every `match` in ui/app | `label()` and `is_stale` use exhaustive match or `matches!`; `matches!` hides the new arm (`freshness.rs:~185`). |
| New workspace | `V/state/mod.rs` `Workspace` variant, `ALL`, `order`, `label`, `hint`; `V/app/mcp` key parse | `order`/`label`/`hint` exhaustive (compiler). `ALL` length is a hand-written `[_;4]` (compiler error on count mismatch only if the array literal is not edited). |
| New render pipeline or GPU resource | `V/render/<x>_render.rs`; `RenderResources` field and shader const in `render/mod.rs`; upload key in `render/upload_cache.rs`; upload call in `V/app/gpu_upload.rs`; colour in `render/colors.rs` | Nothing forces the key to list its inputs. `render_pipelines_headless_g_pipesmoke` smoke-tests construction. |
| New CLI subcommand | `rs_cam_cli/src/main.rs` (`Commands` variant + match arm), new module, apply writes via `command::apply_command` | Match on `Commands` exhaustive. The `apply_command` door is not forced (two direct `.apply(`). |
| New export door | `V/io/export.rs` fn; caller in `V/app/export.rs` / `V/app/mcp.rs`; parity test `export_parity_core_vs_gui_p0` | Not forced; parity is a sentry. |
| New wire enum token | `rs_cam_mcp/src/server.rs` enum with `#[schemars(inline)]` and `From` to core type; `build_info().features` token (`server.rs:1970`) | `From` match exhaustive; the features token and `schemars(inline)` are CONVENTION (pin test `build_info_published_capability_flags_frozen`, `server.rs:2143`). |
| New proxy tool | `rs_cam_mcp_proxy/src/tools.rs` (const + `is_proxy_tool` + `definitions`), `proxy.rs:1033 run_proxy_tool` | `is_proxy_tool` and `definitions()` are separate lists; not forced. |

## 7. CLAUDE.md drift

| Statement | Reality |
|---|---|
| `render/CLAUDE.md`: "A colour belongs in `colors.rs`. Do not write a literal colour in a pipeline." | Literals exist: `render/toolpath_render.rs:228-230,471,628,760-779,822,839,1008`, `render/mesh_render.rs:226`. |
| `render/CLAUDE.md`: "Every upload goes through `upload_cache.rs` ... `refresh_key`" | `refresh_key` is only called from `V/app/gpu_upload.rs`, not from `render/`; the upload pass lives in `app/`. The file map omits that fact. `COLLISION_POINT` is named in the doc and reported unused by `instruments/dead_pub_surface.md:15`. |
| `render/CLAUDE.md`: "`mod.rs` — the pipeline facade" | `render/mod.rs` is 1610 lines: `RenderResources::new` (:263-776), `ViewportCallback::prepare` (:929-1384) and four WGSL shader strings, not a facade. |
| `compute/CLAUDE.md`: "`worker.rs` and `worker/` — the worker loop, the per-operation execute helpers" | `worker/execute/mod.rs` holds only `run_compute*`, playback data and trace writers; per-operation helpers were removed ("Dispatch now goes through `rs_cam_core::session::execute_job`", `worker/tests.rs` header, `worker.rs` top comments). |
| `compute/CLAUDE.md` invariant: toolpath result "arrives with the revision it was computed for" | `ComputeResult.revision` is `Option<u64>`; `None` is adopted at the current revision (`worker.rs:189-195`). |
| `state/CLAUDE.md` invariant: "`simulation_is_stale` is the ONE door ... Never the counter." | `submitted_edit_counter` is still written (`V/controller/events/simulation.rs:441`) and `edit_counter` still guards generate-all plans (`V/controller/events/compute.rs:1712,1742`). They are not freshness reads, but the file map lists no explanation. |
| `state/CLAUDE.md` invariant: "A panel draft with CONTENT lives here ... A view toggle ... may stay in egui memory" | Sentry `panel_drafts_leave_egui_memory_ui09` exists. `state/` still holds `ui` types (`panels.rs:176,183`), so the dependency direction contradicts the "view-side state, no ui" intent (no doc states the rule). |
| `state/CLAUDE.md` file list omits `state/toolpath/entry.rs` role detail and `state/stale.rs` callers | Minor: file map correct (all listed files exist). No missing file found in `state/`. |
| `rs_cam_viz/CLAUDE.md`: `io/app_settings.rs` "only re-exports core's settings loader and writer" | True (`io/app_settings.rs:9-17`). |
| `rs_cam_viz/CLAUDE.md` table has no row for root files (`host.rs`, `mcp_*`, `ui_command.rs`, `present_mode.rs`, `error.rs`) | No CLAUDE.md covers the 8,000 root lines. The `mcp_server.rs`/`mcp_bridge.rs`/`ui_command.rs` contracts live only in module docs. |
| `rs_cam_cli/CLAUDE.md`: "A machine profile comes from `io::machine_library` ... Do not hardcode a preset in a subcommand." | `rs_cam_cli/src/nc_replay.rs:51-60` `builtin_machine()` hardcodes `MachineKinematics::shapeoko_xxl_ricky_tuned()` with feed caps `4000.0`/`10_000.0` when `--machine` is absent. |
| `rs_cam_cli/CLAUDE.md`: "Route mutations ... through the same `ProjectSession` command doors" | `command.rs` says "the three CLI call sites share one entry point" but two sites call `session.apply` directly (`project.rs:309`, `smoke.rs:751`). The doc `command.rs:26` also says "all fifteen CLI sites"; 25 `apply_command(` hits exist (rg count: job 7, run 6, smoke 5, rough_score 4, project 3). |
| `rs_cam_cli/CLAUDE.md`: "One vocabulary per concept ... tool type is `ToolType`'s own serde token" | `run.rs:300-317` uses `ToolType::parse_lenient`; the vocabulary it accepts was not compared with the serde tokens. Unverified. |
| `rs_cam_cli` default path `planning/toolpath_acceptance/cases_agent_smoke.csv` (`main.rs:311`) | The file still exists (`planning/toolpath_acceptance/`). Root `CLAUDE.md` says some `planning/…` paths were deleted 2026-09-17; no break here. |
| `rs_cam_mcp/CLAUDE.md`: "A value with a fixed set of legal tokens takes an enum" | Violated at `rs_cam_mcp/src/server.rs:33,199,466,603,646,668,832,1218,1242` (see section 5). |
| `rs_cam_mcp/CLAUDE.md`: "Do not put controller, GUI state or `ProjectSession` mutation logic here" | `server.rs:1753 resolve_material`, `:1837 build_tool_config`, `:1659/1674 parse_*`, `:1970 build_info` are logic (parsing, tool building, capability list), not wire types. Crate `description` in `Cargo.toml` and the `server.rs` header both say only structs and "small helpers". |
| `rs_cam_mcp/CLAUDE.md`: "A cap ... is named here, in `response.rs`" | Caps are in `response.rs:50-113`. Not checked: literal numbers in `V/app/mcp/**` handlers (other map). |
| `rs_cam_mcp_proxy/CLAUDE.md`: file table and invariants | Match the tree and `Cargo.toml` (`serde_json` only). `Proxy` is `pub` but flagged unused outside the crate (`instruments/dead_pub_surface.md:120`). Binary is Unix-only (`proxy.rs:19 std::os::unix`) with no `cfg(unix)`. |
| `V/lib.rs:222-229` test `mcp_mode_fails_before_event_loop_creation` | Calls `super::run(true)` (1 argument) but `run` takes `(mcp_mode, settings)` (`lib.rs:48`). The module compiles only under `cfg(all(test, not(feature = "mcp")))`, so a default test build never compiles it. |
| `rs_cam_derive/src/lib.rs` header says "The macros do not panic" | True (no `unwrap`/`panic!` in the file). It has no tests (no `#[test]`, no `tests/`). |

## 8. Navigability

| Item | Note |
|---|---|
| Files over 1500 lines in scope | `compute/worker/tests.rs` 3475; `mcp_bridge.rs` 2057; `compute/worker.rs` 1904; `mcp_server.rs` 1868; `rs_cam_mcp/src/server.rs` 2857; `render/mod.rs` 1610; `rs_cam_cli/src/project.rs` 1622; `rs_cam_cli/src/job.rs` 1586; `rs_cam_cli/src/smoke.rs` 1488; `state/simulation.rs` 1564; `rs_cam_mcp_proxy/src/proxy.rs` 1408 (near). |
| Mixed responsibilities | `compute/worker.rs`: wire structs, `ArtifactPolicy`, memory ledger, lane queue, four lane bodies. `mcp_bridge.rs`: frame-loop beat counters, read cache, request enum, response builders, tier-map rendering (`render_tier_map_preview` :1324). `state/simulation.rs`: cut-metric cards, caches, playback, checks, checkpoints. `rs_cam_mcp/src/server.rs`: param structs + parsing + tool building + `build_info`. `rs_cam_cli/src/project.rs`: JSON records, generation-plan walk, feeds apply, report printing. `render/mod.rs`: resources, callback, shaders. |
| Misleading names | `rs_cam_mcp` is "shared param types" but also helper logic. `state/toolpath/*` are core re-exports, not toolpath state. `io/import.rs` takes `id: usize` and `core_err_to_viz` returns `VizError::Other`. `rs_cam_viz::io::app_settings` is a pure core re-export. `ToolpathEntry` in `state/toolpath/entry.rs` vs `ToolpathRuntime` in `runtime.rs` vs core `ToolpathConfig`: three toolpath row types in GUI code. |
| Wrong folder | `state/pin_view.rs` (colours, `egui::Color32`, display points) and `state/shoe_plan.rs` (text formatting) are view helpers in `state/`. `interaction/picking.rs:277-302` holds display helpers (`hole_display_point`, `toolpath_setup_frame`) that sit next to `state/pin_view.rs:57`. `V/app/gpu_upload.rs` (1602 lines) is the upload pass but the keys are in `render/`. `mcp_highlights` is MCP-only state inside `GuiState`. `state/shoe_plan.rs::run_lines` and `io/setup_sheet.rs` share H2 text. |
| Root files without CLAUDE.md | `lib.rs`, `host.rs`, `mcp_server.rs`, `mcp_bridge.rs`, `mcp_lifecycle.rs`, `ui_command.rs`, `present_mode.rs`, `error.rs` (about 8,000 lines) have no folder CLAUDE.md. |
| Subtree docs only in prose | `compute/mod.rs:24-52` lane submit rules; `mcp_bridge.rs:116-165` frame-loop liveness; `lib.rs:56-170` event-loop comments (about 120 lines of comments in `run`). |

## 9. Candidate debt leads (UNVERIFIED)

| Lead | Where |
|---|---|
| `VizError` typed variants `StlImport/SvgImport/DxfImport/StepImport/ProjectSave` and their `From` impls are never constructed; `user_message` arms dead | `V/error.rs:11-25,38-45`; `V/io/import.rs:15-17` maps to `Other` |
| `toolpath_rt_or_default` hard-codes `auto_regen = true`, contradicting `stamp_stale` doc (12 of 24 ops declare false) | `V/state/runtime.rs:353-356`; `V/controller/events/compute.rs:410`; `V/state/stale.rs:40-45` |
| `GuiState.post` is a hand-synced mirror of `session.post_config()`; export reads the mirror, MCP and undo sync by clone | `V/state/runtime.rs:344`; `V/controller/io.rs:366-394`; `V/io/export.rs:604` |
| `V/lib.rs` test calls `run(true)` with the wrong arity (run takes 2 args); it compiles only with `--no-default-features`, so it is never built by default | `V/lib.rs:222-229` |
| `LaneSnapshot.active_toolpath_id: Option<usize>` should be `ToolpathId`; `ComputeResult.revision: Option<u64>` allows unstamped prod path | `V/compute/mod.rs:71`; `V/compute/worker.rs:189` |
| `ComputeBackend` has 6 silent no-op defaults and `lane_snapshots -> [_;4]`; a new lane compiles with `cancel_all` missing it | `V/compute/mod.rs:492,500,518,536,545,570,577,590` |
| 31 hand-written `ComputeBackend` test doubles, 18 near-identical `Silent/Idle/Inert` structs | `V/controller/tests/mod.rs:127`, `tests/apply_contract_a3.rs:57`, `tests/feeds_apply_drops_result_n13.rs:75` and 16 more |
| Fixed-token `String` fields in `rs_cam_mcp` contradict its own CLAUDE.md | `rs_cam_mcp/src/server.rs:33,199,466,603,646,668,832,1218,1242`; `V/mcp_bridge.rs:~520,~540` (`ApplyFeeds.scope`, `GetOperationSchema.operation_type`) |
| `SetUiViewArgs` carries `workspace`, `properties_tab`, `select`, `modal` as `Option<String>` | `V/ui_command.rs:177-186` |
| Three duplicate operation-token parsers: `run.rs::parse_op_type` (Result), `smoke.rs::parse_op_type` (Option), `rs_cam_mcp::parse_operation_type`; all scan `OperationType::ALL` for `kind_str()` | `rs_cam_cli/src/run.rs:271`, `smoke.rs:1074`, `rs_cam_mcp/src/server.rs:1659` |
| Direct `session.apply(` in CLI bypasses `apply_command`, against `command.rs` doc | `rs_cam_cli/src/project.rs:309`, `smoke.rs:751` |
| `nc_replay::builtin_machine` hardcodes a personal machine preset and feed caps | `rs_cam_cli/src/nc_replay.rs:51-60` |
| `rs_cam_mcp/src/server.rs` holds logic (`resolve_material`, `build_tool_config`, `build_info`) in a "types" crate that depends on `rs_cam_core` | `rs_cam_mcp/src/server.rs:1659,1674,1753,1837,1970` |
| Literal colours in pipelines against `render/CLAUDE.md`; second palette in `ui::tokens` | `V/render/toolpath_render.rs:228-230,471,628,760-779,822,839,1008`; `V/render/mesh_render.rs:226` |
| Upload keys must be kept in step with builder inputs by hand | `V/render/upload_cache.rs` module doc; `V/app/gpu_upload.rs:278-1439` |
| Layering inversion `state -> ui` (preferences, pin wizard, head part list, toolpath tab, overlay registry, tokens) | `V/state/mod.rs:198`, `panels.rs:176,183`, `runtime.rs:300`, `app_settings.rs:79`, `pin_view.rs:82` |
| Modal exclusivity is a hand list; each modal is its own `bool`/`Option` | `V/state/mod.rs:479-497` |
| `mcp_highlights: HashMap<String, Instant>` with composed string keys (`"toolpath_{id}_{param}"`) | `V/state/runtime.rs:284`; `V/app/mcp/commands.rs:290`; `V/app/mcp.rs:123` |
| Wire reply bodies are hand-built `String` JSON everywhere (92 `#[tool]` methods return `String`) | `V/mcp_server.rs:402` and all `#[tool]` fns |
| `McpRequestKind` mixes id and index as `usize` (`index`, `toolpath_id: Option<usize>`) | `V/mcp_bridge.rs:500-680`; `V/ui_command.rs:293` |
| `rs_cam_derive` has no tests (no unit, no trybuild) for 433 lines of macro code with refusal paths | `rs_cam_derive/src/lib.rs` |
| Proxy uses stringly state (`state() -> &'static str`), `send_signal(pid, &str)` via subprocess, `stop_reason: Option<String>`; no `cfg(unix)` gate | `rs_cam_mcp_proxy/src/proxy.rs:107-114,63-85`; `procinfo.rs:131` |
| `unsaved_project_summary` string literal contains a run of spaces from a line continuation | `V/state/mod.rs:106-108` |
| Epsilons are local literals (`1e-6`, `1e-9`, `1e-10`, `1e-4`) with no shared constant | `V/render/toolpath_render.rs:50,883,955`; `V/interaction/picking.rs:412`; `V/io/import.rs:19`; `V/render/camera.rs:121,195` |
| `ComputeError::Message(String)` collapses all other `OperationError` variants to text | `V/compute/mod.rs:440-453` |
| `render/mod.rs` `prepare` is a 450-line function inside a trait impl with 3 `allow(unwrap_used)` | `V/render/mod.rs:929-1384` |
| CLI `job.rs::param_value_from_str` guesses JSON type from user text | `rs_cam_cli/src/job.rs:1272` |
| `smoke.rs` default path points at `planning/…` (planning purge risk) | `rs_cam_cli/src/main.rs:311` |
