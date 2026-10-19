# Map: core-session

Scope: `crates/rs_cam_core/src/{session,selection,settings,budget,util}/` and the root files `lib.rs ids.rs geo.rs measurement.rs interrupt.rs toolpath.rs mesh.rs polygon.rs`. Baseline HEAD 99b6fb41.
Paths are relative to `crates/rs_cam_core/src/` unless they start with `crates/`.
In flux: a path tagged (flux) is in `instruments/in_flux_paths.txt`. Line numbers in those files can move. Flux files in scope: `selection/rule.rs`, `session/{command,mod,load_report,multitool,project_file,save,selection_status}.rs`, `session/compute/{generation,tests}.rs`, `session/mutation/{config,entities,toolpath,tests}.rs`. No file in `settings/`, `budget/`, `util/` or the root files is in flux.
Counts come from `rg` and are approximate. Total: about 46,000 lines (session 29.5k, selection 3.3k, settings 1.8k, budget 1.7k, util 0.07k, root 7.7k). Inline `#[cfg(test)]` modules are large: `session/mod.rs` tests start at :1985, `polygon.rs` at :1976, `toolpath.rs` at :897, `mesh.rs` at :854, `geo.rs` at :415, `measurement.rs` at :816.

## 1. Purpose and file map

Purpose. `session/` holds `ProjectSession`, the one owner of project state (models, tools, setups, toolpath configs, cached results, the stored simulation). Every surface (GUI, MCP, CLI) mutates it through `ProjectSession::apply(Command)` and reads it through `query` and typed getters. The folder also holds the invalidation and freshness rules, the project file loader and saver, and the generation, simulation and export doors. `selection/` holds the pickable items of a model and the op selection. `settings/` and `budget/` hold the app settings file and the memory budget. The root files are the crate spine: geometry primitives, the toolpath IR, ids, cancellation and measurement vocabulary.

### session/ (29.5k)
| file | responsibility | lines |
|---|---|---|
| `session/mod.rs` (flux) | facade and re-exports; `SessionError` :141; `LoadedModel` :379; datum types `Corner/XYDatum/ZDatum` :579-720; `SetupData` :741; `ToolpathConfig` :807 (`models_read` :862, `generation_inputs_signature` :911); `SimulationOptions` :1052; `SimulationDropCause` :1185; `ProjectSession` struct :1308; getters; inline tests from :1985 | 2638 |
| `session/command.rs` (flux) | X-macro registry `for_each_command!` :122 (78 rows: 69 Command, 5 Query, 4 Job); `CommandKind/Reach/Surfaces` :848-880; `Effects` :1117; ~60 `*Args` structs; macro generators :2041, :2224; `apply` :2339; `query` :2652; `start` :2746; `try_with_effects` :2784; `with_effects` :2812 | 2849 |
| `session/mutation.rs` | declares `mutation/`; `drop_stock_change_dependents`, `drop_obstacle_dependents`, `obstacle_effects` | 164 |
| `session/mutation/toolpath.rs` (flux) | toolpath CRUD; the invalidation core: `invalidate_result_chain` :237, `invalidate_output_dependents_of_set` :302, `walk_output_dependents` :331, `insert_result` :755, `drop_simulation` :816, `drop_result` :836, `bump_all_revisions` :847 | 941 |
| `session/mutation/config.rs` (flux) | dressup, heights, boundary, rest analysis, stock, post, machine, tool replace; `drop_results_for_model` :402 | 765 |
| `session/mutation/entities.rs` (flux) | add/remove model, tool, setup; datum, pause message, setup models; obstacles | 623 |
| `session/mutation/stock_change.rs` | S2 stock change CRUD | 200 |
| `session/mutation/tests.rs` (flux) | mutation tests | 1258 |
| `session/compute.rs` | job runner `execute_job` :518, `execute_generation` :1233, strategy advisor `execute_recommend_clearing_strategy` :1505, feed modulation `modulate_annotated_against_trace` :1775, sim candidate isolation :2209, `GenObserver` :390, `generate_all` :2589 | 2645 |
| `session/compute/generation.rs` (flux) | `resolve_generation_inputs`, boundary clip, seed stock | 1120 |
| `session/compute/simulation.rs` | `run_simulation`, `run_simulation_memoized`, `modulate_simulation_trace` | 825 |
| `session/compute/diagnostics.rs` | `diagnostics`, `collision_check`, `simulation_triage` :723 | 1682 |
| `session/compute/export.rs` | `export_gcode*` | 469 |
| `session/compute/params.rs` | `set_toolpath_param_impl` :146, `set_tool_param_impl` :531 (string-keyed) | 696 |
| `session/compute/tests.rs` (flux) | compute tests | 2740 |
| `session/project_file.rs` (flux) | serde sections `Project*Section`, `RETIRED_KEYS` :85, `check_retired_keys` :230, `ProjectLoadWarning` :282, string-token parsers :784-900, model loading | 1486 |
| `session/save.rs` (flux) | session to TOML, temp-file write | 880 |
| `session/builder.rs` | `ProjectSessionBuilder` for fixtures (chaining pins ids; allocating `add_*`) | 286 |
| `session/dependencies.rs` | `edges` :112, `state` :219, `primary_edges` :261: the one dependency rule | 294 |
| `session/generation_plan.rs` | `plan` :144: order of generation steps | 255 |
| `session/rest_stock.rs` | `SimulationResolution` :51, `RESOLUTION_FLOOR_MM` :62, rest identity (`SourceStock`), `snapshot_is_current` :233 | 698 |
| `session/rest_fresh.rs` | rest-vs-fresh twin comparison records (no entry in the folder CLAUDE.md) | 115 |
| `session/result_provenance.rs` | `FeedStage`, `ResultProvenance`, `store_simulation` :141 | 182 |
| `session/load_report.rs` (flux) | `LoadReportMemo` (identity-keyed memo), `LoadReportStamp` | 564 |
| `session/selection_status.rs` (flux) | `selection_status` :226 memo, `pick_edit` :448, `list_model_items` :471, `hole_selection` :376 | 603 |
| `session/multitool.rs` (flux) | multitool finishing planner, tier map preview, `PlannerOrigin` use | 1760 |
| `session/cycle_time.rs` | `toolpath_cycle_time`, `CycleTime`, `MissingInputs` | 608 |
| `session/pins.rs`, `pin_report.rs` | pin inputs, pin set report | 578, 237 |
| `session/shoe.rs`, `stock_change_report.rs`, `reach.rs`, `eval_context.rs` | shoe plan, S4 rows, reach map input, `SetupEvalContext` | 277, 264, 179, 247 |
| `session/diagnostics_types.rs` | `ProjectDiagnostics`, `Verdict*` (hand `Serialize` for 2 types :403, :414) | 418 |

### selection/ (3.3k)
| file | responsibility | lines |
|---|---|---|
| `selection/mod.rs` | facade, re-exports | 32 |
| `selection/item.rs` | `SourceRef`, `ItemKind`, `Shape`, `Item::new` :237 (only ctor), `ItemSet` :317, `ItemError` :188 (thiserror) | 687 |
| `selection/key.rs` | `MatchKey`, `KEY_MATCH_EPS_MM` :24 | 167 |
| `selection/rule.rs` (flux) | `Selection` (private fields, no `Default`) :178, `Rule`, `Filter`, `KindFilter`, `SelectionEdit`, `apply` :354 | 505 |
| `selection/resolve.rs` | `resolve` :350 (pure), `ResolveScope`, `MatchedBy`, `to_setup_frame` :205, `face_in_setup_frame` :274 | 475 |
| `selection/view.rs` | `view` :189, `OpItems`, `Regions/Curves/Holes` newtypes | 370 |
| `selection/refusal.rs` | `SelectionRefusal`, `RefusalCause` :149, `SelectionNote`, `ItemRef` | 290 |
| `selection/stock.rs`, `tests.rs` | `stock_items` :13; lib sentries | 44, 694 |

### settings/ (1.8k), budget/ (1.7k), util/ (0.07k)
| file | responsibility | lines |
|---|---|---|
| `settings/mod.rs` | `AppSettings` and sections, `SettingToken` trait :78 and macro, `SectionReader` :385, `parse_lenient` :543, `load` :691, `merged_text` :833, `save_to` :960 | 986 |
| `settings/paths.rs` | `*_from(env)` resolvers, `install_paths` process cell `static PATHS: RwLock` :174 | 376 |
| `budget/mod.rs` | `MemoryBudget`, `MemoryLimit`, `StopReason`, `parse_byte_size` :230, `format_bytes` :289 | 299 |
| `budget/guard.rs` | `UsageProbe` trait :60, `BudgetGuard` :78 (implements `CancelCheck` :314), `BudgetWatch` | 413 |
| `budget/estimate.rs` | held-result formula, preflight, `largest_cell_that_fits` | 583 |
| `budget/grid.rs`, `settings.rs` | `GridCapRole`, `grid_cells` :110; `[memory] limit` text | 162, 102 |
| `util/panic_message.rs`, `build_info.rs` | `panic_payload_message`; build constants from `build.rs` env | 36, 23 |

### Root files (7.7k)
| file | responsibility | lines |
|---|---|---|
| `lib.rs` | module list; layer doc; re-exports `ToolpathId` | 65 |
| `ids.rs` | `ToolpathId` (serde transparent), `ModelId`, `SetupId`, `ObstacleId`, `StockChangeId`; all `pub usize` tuple fields | 49 |
| `interrupt.rs` | `CancelCheck` trait, `Cancelled`, `NeverCancel`, `FlagCancel`, `run_uncancellable`, `check_cancel` | 83 |
| `geo.rs` | `P2/P3/V2/V3` aliases of nalgebra, `BoundingBox3`, `Triangle`, polyline helpers, segment distance | 665 (prod to :414) |
| `mesh.rs` | `TriangleMesh`, `SpatialIndex`, `MeshError` (thiserror), `ray_pick_triangle`, test mesh makers :795, :842 in prod code | 1078 (prod to :853) |
| `polygon.rs` | `Polygon2`, offset via `cavalier_contours` with panic containment, `FlattenPolicy`, containment, `geo` crate boolean glue, segment predicates | 3010 (prod to :1975) |
| `toolpath.rs` | `Move/MoveType/MoveIntent/Toolpath` IR; also path simplification and grid raster generators :613, :779 | 1616 (prod to :896) |
| `measurement.rs` | `MeasurementDomain/Stage/Provenance`, `ProjectedXyAreaMm2`, `SurfaceAreaMm2`, `swept_footprint_area` :687 | 1098 (prod to :815) |

## 2. Public entry points and external callers

Counts: lines in other crates that name the module path (`rg "rs_cam_core::<mod>"`).

| entry | external callers | sample path:line |
|---|---|---|
| `session::*` (ProjectSession and types) | viz 491, cli 31, mcp 8 | `crates/rs_cam_viz/src/controller/events/mod.rs:66`, `crates/rs_cam_cli/src/project.rs:656` |
| `ProjectSession::apply(Command)` | core 409 `.apply(` hits, viz 187, cli 4 | `crates/rs_cam_viz/src/controller/events/mod.rs:101`, `crates/rs_cam_cli/src/project.rs:309` |
| `ProjectSession::load` | cli 4 sites | `crates/rs_cam_cli/src/project.rs:656`, `crates/rs_cam_cli/src/rough_score.rs:180` |
| `ProjectSessionBuilder` | viz tests, core tests | `crates/rs_cam_viz/src/compute/worker/test_fixture.rs:237` |
| `execute_job` / `JobHandle` / `ProjectSession::start` | viz compute lane | `crates/rs_cam_viz/src/compute/worker.rs` |
| `generate_toolpath`, `generate_all`, `run_simulation`, `collision_check`, `export_gcode` (pub, outside `apply`) | viz: 10, 47, 7, 30, 7; cli: 10, 0, 9, 16, 8 | counts by `rg` of the method name |
| `CommandId::ALL`, `wire_name()`, `surfaces()` | viz registry and pins | `crates/rs_cam_viz/src/ui_command.rs:1036`, `crates/rs_cam_viz/tests/command_registry_surfaces.rs:51` |
| `selection::*` | viz 80, cli 1 | `crates/rs_cam_viz/src/ui_command.rs:54` (viz own `Selection` type, a different thing) |
| `settings::*` | viz 37, cli 7 (viz wraps it as `io::app_settings`) | `crates/rs_cam_viz/src/bin/main.rs:18`, `crates/rs_cam_cli/src/memory_budget.rs:64` |
| `budget::*` (`BudgetGuard`, `MemoryBudget`) | viz 53, cli 9 | `crates/rs_cam_viz/src/compute/worker.rs:492`, `crates/rs_cam_cli/src/project.rs:397` |
| `ids::*` | viz 39, cli 1 | `crates/rs_cam_viz/src/state/job.rs:13` (re-export) |
| `util::*` | viz 6, cli 3, mcp 3 | build info and panic message |
| spine `geo/polygon/mesh/toolpath` | viz 245, cli 18 (lines) | pervasive |
| inbound from other core folders | `crate::session` named in 50 files: `compute` 3, `gcode`, `io`, `diagnostics`, `feeds`, `tool_load`, `maps`, `stock`, `trace`, `machine`, `dressup`, `export`, `metrology` | see section 3 |

## 3. Dependencies and layering

`session/` imports (lines naming `crate::<mod>`): compute 385, machine 69, stock 60, feeds 58, trace 55, ids 47, maps 36, selection 34, gcode 34, polygon 29, geo 27, dressup 23, diagnostics 21. Session is the top of the core layer stack by design (`lib.rs` layer doc :3-17 does not list it).

| finding | path:line | note |
|---|---|---|
| Folder cycle session and compute | `session/compute.rs`, `compute/simulate.rs:180`, `compute/obstacle.rs:326`, `compute/catalog.rs` | `compute` takes `session::SetupData`, `LoadedModel`; session takes `compute::*` (385 lines) |
| Folder cycle session and io | `io/mod.rs:25` imports `session::{LoadedGeometry, LoadedModel, SessionError}`; `session/mod.rs:411` calls `io::model_from_geometry` | model record lives in session, loaders in io |
| Lower layers read the whole session | `gcode/mod.rs:22` (`ProjectSession`), `gcode/mod.rs:353`, `tool_load/mod.rs:162`, `export/gcode_validator.rs:721`, `stock/sim_triage.rs`, `feeds/suggest.rs`, `maps/geom_cache.rs`, `dressup/feed_modulation.rs`, `machine/kinematic_utilization.rs`, `trace/narrate.rs:199` | the lib.rs layer list puts export/gcode last, yet they import the top-level aggregate |
| `selection` imports `session` and `compute` | `selection/resolve.rs:17` (`session::LoadedModel`), `selection/resolve.rs:14` (`compute::transform::SetupTransformInfo`), `selection/view.rs:28`, `selection/refusal.rs:11` (`compute::catalog::ItemUse`); reverse: `compute/config.rs:449`, `compute/catalog.rs:987` use `selection::Selection` | selection is in the CLAUDE.md table beside geometry, but it depends upward |
| `settings` and `budget` cycle | `settings/mod.rs:68-69` imports `budget::settings`, `budget::{MemoryBudget, MemoryLimit}`; `budget/settings.rs:94` calls `crate::settings::parse` (test module only) | production edge is one way (settings to budget). The test edge is a cycle |
| `budget` imports `compute` and `stock` | `budget/estimate.rs:40-44` (`compute::simulate::SimulationRequest`, `stock::dexel::DexelRay`, `toolpath::{Move, Toolpath}`) | a leaf-looking folder depends on the simulation request type |
| Spine imports up the stack | `toolpath.rs:7-8` imports `geometry::region_set::RegionSet`, `surface::dropcutter::DropCutterGrid`; `polygon.rs:632` imports `util::panic_message` (fine, util is a leaf) | `toolpath.rs` holds grid raster generators :613, :779 that need those types. Doc links only: `geo.rs:315-316` names `ops`, `finish`; `measurement.rs:217,266` names `finish` |
| `ids.rs` split | `ToolId` is in `compute/tool_config.rs:8`, not `ids.rs` | one id family in two folders |

## 4. Traits

Defined here:

| trait | path:line | implementors | note |
|---|---|---|---|
| `CancelCheck` | `interrupt.rs:5` | blanket `impl<F: Fn()->bool>` :9; `NeverCancel` :40; `FlagCancel` :57; `BudgetGuard` `budget/guard.rs:314` | 55 core files take `&dyn CancelCheck`; session takes `&AtomicBool` (29 sites in `session/`) |
| `UsageProbe` | `budget/guard.rs:60` | `ProcessRss` :71; test `Reading` :357 | seam for tests |
| `SettingToken` | `settings/mod.rs:78` | three enums by macro :105 (`ToolpathColourDefault`, `LogLevel`, `PresentModeSetting`) | token and label per variant |

Not defined here (checked): no `OpParams` or `ParamEnum` use in the session folder. Param writes are string-keyed (`session/compute/params.rs:146`) and call `OperationConfig` accessors. The `for_each_command!` X-macro replaces a trait: the registry holds rows, not a `Command` trait.
Error impls: `SessionError` and `SelectionRefusal` use hand `Display` plus `impl std::error::Error` (`session/mod.rs:266,333`, `selection/refusal.rs:277,290`). `ItemError` (`selection/item.rs:188`), `ByteSizeError` (`budget/mod.rs:206`) and `MeshError` (`mesh.rs:10`) use `thiserror`. Two styles in one area.
Other manual impls: `Default` for `XYDatum` :632, `SimulationOptions` :1161, `MultitoolPlanSpec` `session/multitool.rs:221`, `ProjectStockConfig`; `Clone` that returns empty for `LoadReportMemo` `session/load_report.rs:188` and `SelectionStatusMemo` `session/selection_status.rs:599`; `Serialize` by hand for `VerdictSeverity`, `VerdictKind` `session/diagnostics_types.rs:403,414`.

## 5. Recurring patterns

| concern | pattern used here | example path:line | enforcement | deviations inside the area |
|---|---|---|---|---|
| errors and refusals | One `SessionError` enum, hand `Display`; refusals are typed where they matter (`StaleCompletion`, `StaleSimulation`, `RetiredKeys`, `StockChangeRefused`, `Selection`); `Result<_, SessionError>` everywhere in session | `session/mod.rs:141` | TYPES (variants) / DOC (the "no string match" intent) | String-carrying catch-alls: `InvalidParam(String)` 73 uses, `OperationFailed(String)` 42, `MissingGeometry(String)` 17, `Export(String)`, `UnsupportedSetup(String)`, `TomlParse(String)`; 59 `SessionError::X(format!(..))` sites in `session/`. `settings/mod.rs:680,833,960` return `Result<_, String>`. `thiserror` in `item.rs:188`, `budget/mod.rs:206`, `mesh.rs:10` but not in `SessionError`. `session/save.rs` has 69 `unwrap/expect/panic` hits, almost all in its test module (:287 onward) |
| state mutation | Every write goes `ProjectSession::apply(Command)`; `apply` is an exhaustive `match` over a generated enum; setters are `pub(crate)`; `Effects` is `#[must_use]` and built only in `try_with_effects`/`with_effects` | `session/command.rs:2339`, `:2784`, `:1117` | TYPES for `pub(crate)`, exhaustive match, `#[must_use]`; SENTRY for "every setter has a row" (`tests/setters_have_rows_wp15a.rs`) and "no outside caller" (`tests/setters_are_crate_private_wp15b.rs`, `tests/hatches_are_crate_private_wp7.rs`) | Pub `&mut self` doors outside `apply`: `generate_toolpath` `session/compute.rs:2554`, `generate_all` :2589, `run_simulation` `session/compute/simulation.rs:72`, `run_simulation_memoized` :249, `modulate_simulation_trace` :411, `plan_multitool_finishing` `session/multitool.rs:338`, `LoadedModel::adopt_geometry` `session/mod.rs:494`. `compute/validate.rs:164` `apply_stale_default_fix` is `pub`, writes configs through `toolpath_configs_mut()` `session/mod.rs:1809` and calls no `drop_result` (callers: tests only). `toolpath_configs_mut` is `pub(crate)` not test-gated. `ProjectSessionBuilder::add_*` `session/builder.rs:186-221` bypass `Effects` by design |
| parameters and config | Op params live in `compute::catalog::OperationConfig`; session writes one param by name with a JSON value, range from `check_param_range`; `SimulationOptions` struct with many bool flags | `session/compute/params.rs:146`, `session/mod.rs:1052` | CONVENTION (names are string arms) with SENTRY for registry parity (`tests/suggest_refuses_what_the_registry_refuses_fm4.rs`) | `set_toolpath_param_impl` hand-matches names `feed_rate`, `plunge_rate`, `stepover`, `depth_per_pass`, `spindle_rpm` :180-260 before the generic arm. `set_tool_param_impl` `params.rs:531-590` has no range check and casts `v as u32` for `flute_count` :545. `SimulationOptions` carries four bools (`auto_resolution`, `use_predicted_feed_in_gates`, `adaptive_feed_modulation`, plus the f64 `modulation_feed_scale`) `session/mod.rs:1052-1160` |
| units and coordinate frames | Unit in the name (`_mm`, `_deg`, `_mm2`); bare `f64`; setup frame vs stock/world frame stated in docs; one frame door `to_setup_frame`/`face_in_setup_frame`; area newtypes with private field | `selection/resolve.rs:205,274`, `measurement.rs:457-463` | DOC / CONVENTION; TYPES only for `ProjectedXyAreaMm2`, `SurfaceAreaMm2` (private field) | `Toolpath`/`Move.target` are bare `P3` with no frame tag `toolpath.rs:126`. `ArcCW{i,j}` carry no unit/plane tag :30-35. `ProjectStockConfig` and tool sections are bare `f64` `session/project_file.rs:466-620`. `MoveType::feed_rate: f64` has no unit newtype. Inverse frame helper `inverse_transform_point_from_setup` `session/mod.rs:1887` is a second door beside `selection/resolve.rs:274` |
| identifiers | Newtypes in `ids.rs`; used at API edges (`ToolpathId` 130 session hits, `SetupId` in `generation_plan.rs:113`); the registry carries raw `usize` index in most `*Args` | `ids.rs:20`, `session/generation_plan.rs:113` | TYPES for `ToolpathId` and `ObstacleId`/`StockChangeId`; CONVENTION elsewhere | `LoadedModel.id: usize` `session/mod.rs:380`; `SetupData.id: usize` :742; `ToolpathConfig.tool_id/model_id: usize` :813-814; `ToolId` outside `ids.rs` (`compute/tool_config.rs:8`); `find_tool_by_raw_id`/`find_model_by_raw_id` `mod.rs:1836,1844`; `results: HashMap<usize,_>`, `toolpath_revision: HashMap<usize,u64>`, `SetupData.toolpath_indices: Vec<usize>` `mod.rs:1308-1330` are keyed by index although `ids.rs:1-8` says id and index diverge; `next_*_id: usize` counters; every id has `pub` inner field so `SetupId(setup.id)` wraps ad hoc (viz 8 sites); `ModelId`/`SetupId` have no `Display`/`Ord` |
| background work, progress, cancel | Job = capture (`start`) / run free fn / adopt command; cancel by `&AtomicBool`; `GenObserver` + `ToolpathPhaseSink` for phase text; `BudgetGuard` sets the same flag | `session/command.rs:2746`, `session/compute.rs:390`, `budget/guard.rs:78` | TYPES (exhaustive `Job` match; `JobHandle` must cover each row) | Two cancel idioms: `&AtomicBool` 29 sites in session vs `&dyn CancelCheck` in 55 core files; `FlagCancel` adapter `interrupt.rs:57`. Phase text is a `&str` (`GenObserver::set_phase` `session/compute.rs:455`). `Effects`-free reads (`run_simulation`) block the caller |
| caching, freshness, invalidation | Four mechanisms: (1) revision counters `toolpath_revision` + `next_revision`, bumped only in `drop_result`; (2) `simulation_epoch` bumped only in `drop_simulation`; (3) value/`Weak`-identity memos (`LoadReportMemo`, `SelectionStatusMemo`); (4) `SourceStock` identity for rest (`rest_stock.rs`) plus `RestFreshRecord{revision}`. Edges come from one function `dependencies::edges` | `session/mutation/toolpath.rs:836,816`, `session/dependencies.rs:112`, `session/load_report.rs:1-40` | SENTRY (`stale_set_has_one_answer_wp28`, `dependency_edges_are_the_walker_dep1`, `mutation_paths_invalidate_alike_p0`, `adopt_result_rejects_stale_completion`, `a_late_simulation_does_not_refill_the_core_d7`); TYPES only for `pub(crate)` of `drop_*` | `ToolpathConfig::generation_inputs_signature` builds a `String` from `serde_json` + `{:?}` with `unwrap_or_default()` `session/mod.rs:911-925`: a serialise failure gives an empty piece, and a new field is not forced into it (no destructure). `adopt_geometry` uses a destructure guard :494 (TYPES) but `models_read` :862 does not. Writers must pick `invalidate_*` vs `walk_*` by hand (`AdoptResult` `command.rs:2339+` uses the walk) |
| diagnostics, warnings, messages | Load warnings are a typed enum `ProjectLoadWarning` + `tracing::warn!`; operator text built in `Display`; refusals carry `fix()` text | `session/project_file.rs:282`, `selection/refusal.rs:149,203` | TYPES (enum) / CONVENTION (text) | Unknown-token policy splits: warn for tool type, face_up, z_rotation `project_file.rs:784-848`; silent default for `parse_tool_material` :882, `parse_cut_direction` :890, `Corner::from_key` `mod.rs:613`, `XYDatum::from_key` :662, `ZDatum::from_key` :705 (`.parse().unwrap_or(0.0)` :709). `settings` returns `Vec<String>` warnings `settings/mod.rs:543` (not typed) |
| serialization and wire types | `serde` with `default = "fn"` (24), `rename_all` (10), `transparent` (4), one `deny_unknown_fields`; project TOML has retired-key refusal table; tokens are strings (`to_key/from_key`); MCP wire names are literals in the registry | `session/project_file.rs:85`, `ids.rs:19`, `session/command.rs:122` | SENTRY (registry names: `command_registry_completeness`; wire pin `crates/rs_cam_viz/tests/mcp_wire_surface_pin.rs:271`; `project_tool_section_type_key_shape_frozen` `project_file.rs:1459`) | `ModelId`, `SetupId` lack `#[serde(transparent)]` `ids.rs:33,37` (others have it). Project sections hold enum-like fields as `String` (`tool_type`, `face_up`, `cut_direction`, `tool_material`) `project_file.rs:533-690`. `save.rs:14-50` repeats the key tables that `project_file.rs` parses (two directions by hand). `RETIRED_KEYS` is private `project_file.rs:85`; the CLAUDE.md names it with `pub` path |
| numeric tolerances, epsilons | Named `const` with `_MM` suffix for domain tolerances; ad hoc literals in algorithm code | `selection/key.rs:24`, `budget/grid.rs:35`, `session/rest_stock.rs:62` | CONVENTION | `CLAMP_EPS = 0.001` twice `toolpath.rs:628,801`; `1e-9` literals `toolpath.rs` (38 hits), `polygon.rs` (32), `geo.rs` (22); `ORIENT_EPS` `polygon.rs:1868`, `PLINE_POS_EQUAL_EPS` :657, `geo.rs:193` local `EPS`; `KEY_MATCH_EPS_MM` and `MIN_CELL_MM` both `1e-6` with separate names; `selection/item.rs:27` `CIRCLE_ARC_STEP_DEG`; `selection/item.rs:501` `1e-9` inline |
| geometry primitives | nalgebra aliases `P2/P3/V2/V3`, `Polygon2{outer, holes}`, `BoundingBox3`; `[f64;4]` bbox in items; `geo` crate and `cavalier_contours` behind `polygon.rs` | `geo.rs:7-16`, `polygon.rs:113` | CONVENTION | `ItemSet::bbox` returns `[f64;4]` `selection/item.rs:476` not a bbox type. Point-to-segment distance exists 5 times: `geo.rs:381,393`, `polygon.rs:1946`, `dressup/link.rs:49`, `compute/alignment_pins/walls.rs:181`, `compute/execute/standing_material.rs:764`. `path_length` `selection/key.rs:126` repeats `geo.rs:288`. `area_centroid` `key.rs:100` beside `Polygon2::signed_area` `polygon.rs:221` |
| test style and sentries | Folder CLAUDE.md lists named sentries in `crates/rs_cam_core/tests/`; names are sentences with a package tag (`..._p2_5`); inline `mod tests` for small units; fixtures through `ProjectSessionBuilder` | `tests/command_registry_completeness.rs`, `session/builder.rs:1` | SENTRY | Test-only API in prod files: `mesh.rs:795,842` `make_test_*` are `pub` outside `cfg(test)`. `session/mod.rs` holds ~650 lines of tests inline. Production `allow(clippy::indexing_slicing)` without `// SAFETY:` (root rule): `toolpath.rs` 9 of 9, `mesh.rs` 9 of 9, `polygon.rs` about 14 of 25 (comment says "bounded indexing in algorithmic code"); `toolpath.rs:600` puts `#[allow]` before the doc comment |
| naming and module layout | One folder per layer, facade `mod.rs` with `pub use`; `*_impl` raw half + door wrapper; `Args`/`Answer`/`Handle` suffix; package tags (`P2.5`, `WP15a`) in comments | `session/mod.rs:19-95`, `session/mutation/entities.rs:36,47` | CONVENTION | `compute.rs` + `compute/` and `mutation.rs` + `mutation/` (file beside folder). `session/compute.rs` is a 2.6k grab bag (see 8). Doc comments cite programme tags that name deleted `planning/` paths |
| how a feature is registered | Add a row to `for_each_command!`; the macro builds enums, `CommandId`, wire name, surfaces; the compiler forces the `apply`/`query`/`start` arm and the `Surfaces` fields | `session/command.rs:122` | TYPES (match arms, struct literal) + SENTRY (wire names unique, `Skip` reason non-empty) | Surfaces `Reach::Skip(&'static str)` reasons are free text, checked by source scan in viz tests. A new `ProjectFile` key needs `project_file.rs`, `save.rs` by hand with no compiler link |

## 6. Extension points

| to add | files touched | compiler forces | not forced (sentry or none) |
|---|---|---|---|
| a Command/Query/Job row | `session/command.rs`: row in `for_each_command!` :122, `*Args`/answer struct, arm in `apply` :2339 (or `query` :2652 / `start` :2746); the `pub(crate)` impl in `session/mutation/*.rs`; `session/mod.rs` re-export list :40-95; viz `mcp` tool + `rs_cam_mcp` wire type if reached by MCP; viz `ui_command.rs` consumes `CommandId::ALL` | `apply` match is exhaustive; `Surfaces` literal needs all three fields; `JobHandle` needs a variant (matched in `start`) | the re-export in `mod.rs` (not forced, only `pub` use); MCP tool for a row marked `Reached` (viz sentry `command_registry_surfaces`, `mcp_wire_surface_pin`); the invalidation choice (`invalidate_*` vs `walk_*`) is by hand, sentry `mutation_paths_invalidate_alike_p0` |
| a `ToolpathConfig` field | `session/mod.rs:807` struct; `project_file.rs` `ProjectToolpathSection` :689 + `toolpath_config_from_section` :993; `save.rs`; `generation_inputs_signature` :911 if it changes output; `models_read` :862 if it reads a model; `builder.rs`; `ReplaceToolpathConfig` projection in viz | struct literal sites (many fixtures, `command.rs`, `tests`) | signature, save and load are by hand; no destructure guard |
| an `OperationConfig` param | `compute/` (derive/catalog, outside this area); session side is string-keyed: `params.rs:146` generic arm reads the registry | none in session | `check_param_range`, `unknown_param_error` use the registry; hand arms at `params.rs:180-260` |
| a tool param | `compute/tool_config.rs`; `params.rs:531` arm; `project_file.rs:533` section + `tool_from_project_section` :851; `save.rs` | none | all by hand |
| a `SessionError` variant | `session/mod.rs:141`, `Display` :266 | `Display` match is exhaustive; other crates that match | `From` impls, MCP/GUI message mapping |
| a retired project key | `RETIRED_KEYS` `project_file.rs:85` (or `RETIRED_VALUES` :183) | none | CLAUDE.md names the rule only (DOC) |
| a setting key | `settings/mod.rs` section struct + `Default` + `parse_lenient` :543 + `merged_text` :833; `settings/tests.rs` round trip | none | reader and writer must list the key twice; sentry `settings::tests` round trip |
| a path/env resolver | `settings/paths.rs` `*_from(env)` + wrapper | none | CONVENTION "pure `_from` form" |
| a `SimulationDropCause` | `session/mod.rs:1185` variant, `COUNT` :1206, `ALL` :1209, `slot` :1230, `short_label` :1219 | `slot` match is exhaustive; `ALL` length is `COUNT` (array type) | `COUNT` equals variant count is only checked by array length |
| a dependency edge kind | `session/dependencies.rs` `EdgeKind` :70; walker `mutation/toolpath.rs:331` matches `EdgeKind` | exhaustive match in the walker | `generation_plan` agreement: sentry `generation_plan_is_the_edge_walk_w0b` |
| a selection filter/rule | `selection/rule.rs` `Filter`/`KindFilter`; `resolve.rs:350`; `view.rs:189`; `refusal.rs` cause + `fix()` :203 | `RefusalCause` matches; `Selection` private fields | `selection_refusals_name_the_item_p2_4` (one arm per cause) |
| an item kind | `selection/item.rs` `ItemKind` :48, `Shape` :74; `view.rs:37,60` `serves`; `refusal.rs:33` `kind_noun`, :129 `note_fix`; session `selection_status.rs` | exhaustive matches | io doors that create items (`every_import_door_gives_items_p2_1` SENTRY) |
| a budget grid cap | `budget/grid.rs:51` `GridCapRole` | `GridCapRole` match | caps pinned by a sentry (`memory_budget_core_b1_b2_b3`) |
| a datum variant | `session/mod.rs:625-720` enum + `to_key` + `from_key` + `label`; persistence in `project_file.rs:1053`, `save.rs` | `label`/`to_key` matches | `from_key` has a `_ =>` default so a new variant is silently unparsed |

## 7. CLAUDE.md drift

| statement | where | reality |
|---|---|---|
| "`walk_output_dependents` is pure" | `session/CLAUDE.md` Invariants | `session/mutation/toolpath.rs:331` takes `&mut self` and calls `drop_result` (:~395). It touches no simulation, but it is not pure |
| file map lists the session files | `session/CLAUDE.md` Files | `session/rest_fresh.rs` (115 lines) and `session/compute/params.rs` (696) are not named. `compute/tests.rs` and `mutation/tests.rs` are not named |
| "`dependencies::edges` (rules) ... no surface re-derives either" | `session/CLAUDE.md` | not checked in code beyond `edges` callers; `generation_plan` and the walker both call it (consistent) |
| "`outlines_pre_selection` is a test reference only" | `selection/CLAUDE.md` Invariants | `selection/item.rs:506` (`ItemSet::scaled`) and :545 (`with_circles`) read it in production. `selection/item.rs:580` says "P2.3 deletes this": still present. `LoadedModel::outlines_pre_selection` `session/mod.rs:557` is the reference-only wrapper; `io/svg_input.rs:312` and `io/dxf_input.rs:875` sit in `#[cfg(test)]` (:296, :859) |
| `RETIRED_KEYS` is `project_file::RETIRED_KEYS` | `session/CLAUDE.md` | the const is private (`project_file.rs:85`); the folder doc `use` path suggests a public item |
| "`setups_mut` stays `#[cfg(test)]`" | `session/CLAUDE.md` | true (`session/mod.rs:1826`). `toolpath_configs_mut` :1809 and `find_toolpath_config_by_id_mut` :1756 are `pub(crate)`, not test-gated, and have production callers (`compute/validate.rs:168`, `mutation/config.rs:263`) |
| doc cites `session/mutation.rs:1339` for `bump_all_revisions` | `session/command.rs` `Effects.stale` doc :1124-1128 | `mutation.rs` has 164 lines. The function is at `session/mutation/toolpath.rs:847` |
| "`mod.rs:1-2` ... a future MCP server" | `session/mod.rs:1-2` | the MCP server exists; stale module doc |
| `session/mod.rs:1790-1810` docs name `rename_setup()`, `edit_toolpath_selection()`, `add_obstacle()` as methods | `session/mod.rs` `*_mut` docs | those names are not `pub` methods; rows are `SetSetupName`, `EditSelection`, `AddObstacle` |
| `util/` row "—" in the core CLAUDE.md folder table | `crates/rs_cam_core/CLAUDE.md` | `util/` has no folder file. Consistent, but the core file says "Most folders carry their own" |
| settings and budget sentry names | `settings/CLAUDE.md`, `budget/CLAUDE.md` | all named paths exist (`settings::`, `budget::settings`, `memory_budget_core_b1_b2_b3`, `rest_stock::tests`) |
| every sentry named in `session/CLAUDE.md` and `selection/CLAUDE.md` | both | each name has a file in `crates/rs_cam_core/tests/` (checked by `ls`): no missing sentry found |
| "Every production `allow(...)` carries a `// SAFETY:` line" | root `CLAUDE.md` | not true for `toolpath.rs`, `mesh.rs`, `polygon.rs` (see section 5, test row) |
| `lib.rs` layer doc :3-17 | `lib.rs` | lists layers 1-6 but not `session`, `selection`, `settings`, `budget`, `diagnostics`, `metrology`; session sits above all of them |

## 8. Navigability

| item | path | note |
|---|---|---|
| over ~1500 lines (prod) | `session/command.rs` 2849 (about 2000 of macro rows) (flux); `session/compute.rs` 2645; `session/mod.rs` 2638 (about 1980 prod) (flux); `polygon.rs` 3010 (prod 1975); `session/compute/diagnostics.rs` 1682; `session/multitool.rs` 1760 (flux); `toolpath.rs` 1616 (prod 896); `session/project_file.rs` 1486 (flux) | test files `session/compute/tests.rs` 2740 and `mutation/tests.rs` 1258 are large too |
| mixed responsibilities | `session/compute.rs`: job runner, strategy advisor (:1302-1700), feed modulation (:1745-2095), simulation candidate runs (:2145-2270), `ProjectSession` impl (:2276+) | the advisor and the modulation code have no session dependency beyond inputs |
| mixed responsibilities | `session/mod.rs`: error type, `LoadedModel`, datum types, `SetupData`, `ToolpathConfig`, `SimulationOptions`, drop-cause set, session struct, 650 lines of tests | datum types and `SimulationOptions` could sit in their own files |
| mixed responsibilities | `polygon.rs`: data type, offset engine, panic containment, flatten policy, containment tree, `geo` crate conversion, segment predicates | |
| misplaced | `toolpath.rs:453-895`: simplification and grid-to-raster generators in the IR file | belongs with `surface`/`ops` (the file imports `surface::dropcutter`) |
| misplaced | `mesh.rs:795,842` `make_test_hemisphere`, `make_test_flat` are `pub` | test helpers in prod |
| misplaced | `ToolId` `compute/tool_config.rs:8` | beside `ids.rs` siblings |
| misplaced | datum types and `PlannerOrigin` in `session/mod.rs` | project vocabulary in the facade |
| misplaced | `compute/validate.rs:164` `apply_stale_default_fix(&mut ProjectSession)` | a session mutation outside `session/` |
| misplaced | `selection/resolve.rs` `LoadedModel` dependency | resolver takes a session type |
| misleading names | `session/mutation.rs` (164 lines of helpers) vs folder `mutation/`; `find_*_by_raw_id` (raw means the `usize` of a typed id); `util/` ("no layer owns") holds build info and a panic helper; `settings/` (app file) vs `session/` project file; `selection::view` returns op items, not a GUI view; `Selection` in viz is a different type | |
| two `ProjectSession` doors for reads | typed getters (`list_toolpaths`, `get_result`) vs `query(Query)` (5 rows) | only 5 reads go through the registry |

## 9. Candidate debt leads (UNVERIFIED)

1. `session/mod.rs:911` `generation_inputs_signature` builds a string from `serde_json::to_string(..).unwrap_or_default()` and `{:?}`. A serialise error compares as equal; a new `ToolpathConfig` field is not forced in. Use a destructure or a typed hash.
2. `session/mod.rs:380,742,813,814` model, setup, tool ids are bare `usize`. `ids.rs` has `ModelId`, `SetupId`. `find_*_by_raw_id` `mod.rs:1836,1844` exists to bridge.
3. `session/mod.rs:1308-1330` results and revisions are keyed by toolpath index. `bump_all_revisions` `mutation/toolpath.rs:847` exists because an index shift breaks them. A `ToolpathId` key would remove that class.
4. `session/mod.rs:613,662,705` and `project_file.rs:882,890` fall back to a default on an unknown token with no warning, while `project_file.rs:784-848` warns. `ZDatum::from_key` `:709` maps a bad offset to `0.0` (a safety-relevant datum).
5. `session/compute/params.rs:531-590` tool params: no range check, `v as u32` cast at :545, string arms. `set_toolpath_param_impl` :146 mixes hand arms with a registry arm.
6. `compute/validate.rs:164` `apply_stale_default_fix` is a public session mutation with no `drop_result` and no `Command` row; only tests call it.
7. `session/mod.rs:1809` `toolpath_configs_mut` is `pub(crate)` and not test-gated. The WP7 sentry names it, but the type system allows any core module to write configs without invalidation.
8. `session/compute.rs`, `generate_all` :2589, `generate_toolpath` :2554, `run_simulation` and others are pub `&mut self` mutators outside `apply`. The sentry exempts "six compute doors" by name only.
9. `ids.rs:33,37` `ModelId`, `SetupId` lack `#[serde(transparent)]`, `Display`, `Ord`; all five ids have `pub` inner fields.
10. `selection/item.rs:506,545` production code reads `outlines_pre_selection`, which the docs say is reference-only and "P2.3 deletes". Dead-intent code with live callers.
11. `selection/key.rs:126` `path_length` duplicates `geo.rs:288`. Point-to-segment distance has 5 copies (section 5, geometry row).
12. `toolpath.rs:628,801` `CLAMP_EPS` defined twice; the two raster generators :613, :779 look like near-duplicates (`merge_segments_skip_clamped` :873 is shared).
13. `toolpath.rs`, `mesh.rs`, `polygon.rs`: about 30 production `allow(clippy::indexing_slicing)` lines lack `// SAFETY:` (root rule). `toolpath.rs:600` puts the attribute above the doc comment.
14. `mesh.rs:795,842` `make_test_*` helpers are `pub` in production code.
15. `settings/mod.rs:680,833,960` return `Result<_, String>`; warnings are `Vec<String>` `:543`. Other folders use typed errors.
16. `session/mod.rs:141` `SessionError` catch-all string variants (`InvalidParam` 73, `OperationFailed` 42, `MissingGeometry` 17) carry operator text built by `format!` at 59 sites. The `GeneratedEmpty` doc :186-200 shows the project already wants typed variants.
17. `session/mod.rs:266` `Display` is hand-written for 22 variants while `ItemError`, `ByteSizeError`, `MeshError` use `thiserror`.
18. `session/command.rs:122` surfaces `Reach::Skip(&'static str)` reasons: roughly 120 free-text strings. Verified by source scan (viz), not by type.
19. `session/save.rs:14-50` and `project_file.rs:784-900` hold the token tables for tool type, material, cut direction in two directions by hand. A shared enum-token trait (like `settings::SettingToken` :78) would unify them.
20. `session/compute.rs:1302-1700` strategy advisor and `:1745-2095` feed modulation live in the session folder with no session state; candidates to move.
21. `session/mod.rs:2638` plus `session/compute/tests.rs:2740`: large inline and sibling test files; `mod.rs` tests (:1985-2638) test `ProjectSession` basics beside the facade.
22. `budget/estimate.rs:40` `budget` imports `compute::simulate::SimulationRequest`; `settings`/`budget` cycle in tests `budget/settings.rs:94`.
23. `lib.rs:3-17` layer doc omits `session`, `selection`, `settings`, `budget`, `diagnostics`, `metrology`; the real layering has cycles (section 3).
24. `session/command.rs:1124-1128` doc points to `session/mutation.rs:1339` (file has 164 lines); `session/mod.rs:1-2` says "future MCP server".
25. `session/mod.rs:1186-1275` `SimulationDropCause` / `SimulationDropCauses` is a hand-rolled bit set with `COUNT`, `ALL`, `slot`; `bitflags` or an enum-set type could replace it.
26. `session/mod.rs:1052-1160` `SimulationOptions` has several bool flags plus f64 scale and `auto_resolution`; `resolution: f64` beside `auto_resolution: bool` allows a contradictory pair.
27. Global state: `settings/paths.rs:174` `static PATHS: RwLock<Option<PathSettings>>` set by `install_paths` :179 and read by every library resolver; a hidden input to `tool_library_dir` :206.
28. `interrupt.rs:71` `run_uncancellable` holds the workspace's one `expect`; the 26 wrappers it replaced are gone, but 29 `&AtomicBool` sites in `session/` still bypass the `CancelCheck` trait.
