# Map: viz-ui (`crates/rs_cam_viz/src/ui/`)

Baseline HEAD 99b6fb41. Read-only review. Paths are relative to `crates/rs_cam_viz/` unless they start with `crates/`, `planning/` or `tests/` (the `tests/` dir is `crates/rs_cam_viz/tests/`).
"In flux" = listed in `planning/tech_debt_2026-10-19/instruments/in_flux_paths.txt`. In-flux files of this area: `src/ui/mod.rs`, `src/ui/properties/mod.rs`, `src/ui/properties/panel_apply.rs`, `src/ui/properties/tab_badges.rs`, `src/ui/properties/tests.rs`, `src/ui/properties/operations/mod.rs`, `src/ui/properties/operations/validate.rs`. Neighbours also in flux: `src/app/viewport.rs`, `src/app/gpu_upload.rs`, `src/controller/events/{mod,pins,toolpath,undo}.rs`, `src/state/history.rs`, `src/state/toolpath/entry.rs`, `src/interaction/picking.rs`.
Line counts include inline `#[cfg(test)]` tails. Facts come from `rg` and file reads; the SocratiCode graph was not used. Counts marked "about" come from `rg` patterns and are not exact.

## 1. Purpose and file map

`ui/` holds every egui surface of the desktop app: the chrome (menu, workspace bar, status bar), the workspace rails (Setup, Toolpaths, Simulation, Readiness), the inspector (`properties/`), the Feeds Explore window (`feeds/`), the viewport dock and legend (`viewport_overlay.rs`, `overlays/`), the modals and wizards, and the shared component kit (`components/`) with its design tokens (`tokens.rs`). Each panel is an immediate-mode `draw(ui, state, events)` function. A panel reads `&AppState` and pushes `AppEvent` values onto a `Vec`. The controller drains the events after the pass. The inspector is the exception: it applies `ProjectSession::apply(Command)` itself through "panel doors" (section 5, row "State mutation"). `ui/mod.rs` also defines the `AppEvent` enum (about 83 variants, `src/ui/mod.rs:80`) that the controller, the MCP layer and `state/` import, so the folder is also the event vocabulary of the whole app.
Total about 52 700 lines of Rust in about 108 `.rs` files, plus 7 `CLAUDE.md` files. `#![deny(clippy::indexing_slicing)]` at `src/ui/mod.rs:1` repeats a workspace lint (`Cargo.toml:42`).

### Root files

| File | Responsibility | Lines |
|---|---|---|
| `mod.rs` | `pub mod` list (33 mods), `AppEvent` enum, `model_import_event`, re-export of `MODEL_FILE_EXTENSIONS`. IN FLUX | 377 |
| `automation.rs` | per-frame widget snapshot in egui temp data, keyed by `&'static str` ids (`record`, `begin_frame`, `snapshot`) | 50 |
| `tokens.rs` | every colour, space, radius, text size, `Style` install, `as_unit_rgb` | 814 |
| `theme.rs` | legacy re-export of `tokens` (22 `pub` items); doc says "do not add a name" | 78 |
| `menu_bar.rs` | File/View/Toolpath/Simulation menus; 4 `rfd` file dialogs | 284 |
| `workspace_bar.rs` | workspace switcher strip, job cancel button | 264 |
| `status_bar.rs` | status values, lane chips, load-warning chip | 332 |
| `setup_panel.rs` | Setup rail: resources, setups, stock-change list | 560 |
| `toolpath_panel.rs` | Toolpath rail: cards, gutter connector (dependency edges), drag reorder, add menus | 1626 |
| `toolpath_row_controls.rs` | eye / cut / rapid toggles shared by sim op list and inspector | 108 |
| `sim_diagnostics.rs` | Simulation inspector: verdict line, `TabStrip` (Metrics, Warnings, Navigate, Project), cut-metric cards, limit rows, spans, traces | 2841 |
| `sim_op_list.rs` | Simulation left rail: op rows, run buttons | 1402 |
| `sim_timeline.rs` | bottom transport, time axis, boundaries, tool-load markers, `estimate_times` | 1514 |
| `sim_debug.rs` | trace-availability badge (`draw_trace_badge`), JSON formatter | 196 |
| `sim_trace_modal.rs` | one cut-metric trace, large | 180 |
| `readiness.rs` | readiness checks, cycle-time helpers, `simulation_request_is_buildable`, holder-clearance verdicts, two extension traits | 636 |
| `readiness_panel.rs` | Readiness workspace page, project feeds rollup (`egui_plot`) | 953 |
| `preflight.rs` | export pre-flight modal, `simulation_detail` | 693 |
| `export_wizard.rs` | multi-step G-code export wizard; calls `io::export` 8 times | 1227 |
| `pin_wizard.rs` | two-step alignment-pin wizard, `PinWizardState` (held in `state/panels.rs:183`) | 632 |
| `multitool_planner.rs` | multi-tool finishing planner dialog | 1013 |
| `preferences.rs` | File > Preferences window and `PreferencesState` (held in `state/mod.rs:198`) | 1342 |
| `viewport_overlay.rs` | viewport dock (popovers per `DockSection`) | 846 |
| `tool_library_modal.rs`, `machine_library_modal.rs` | per-user library modals | 563, 260 |
| `generation_resolution_modal.rs`, `setup_deletion_modal.rs`, `shortcuts_window.rs` | small modals | 45, 36, 81 |
| `shoe_list.rs` | H2 run sheet (tool and shoe changes) | 101 |

### `components/` (shared kit, about 5 600 lines; imports no `crate::state`, `AppEvent` or `crate::controller` outside comments, `rg` finds none)

| File | Responsibility | Lines |
|---|---|---|
| `mod.rs` | facade, re-exports | 66 |
| `section.rs` | `UiExt` (`named_section`, `disclosure`, `param_grid`), `SummaryCard` | 98 |
| `card.rs`, `notice.rs` | `SectionHeader`, `Card`, `scrim`; `Banner`, `NoticeStack`, `NotMeasured`, `EmptyState`, `Notice` | 189, 556 |
| `data_table.rs` | `DataTable`, `DataRow`, `Details`, `Source` (IC2 tabular rule) | 680 |
| `value_row.rs`, `kv_row.rs`, `choice_row.rs` | `ValueRow` (DragValue + range + tooltip), `KeyValueRow`, `ChoiceRow` | 339, 214, 143 |
| `chip.rs`, `pill.rs`, `precedence.rs`, `provenance.rs`, `freshness.rs`, `suggest.rs`, `compare.rs` | `Role`, `StatusChip`, `CountPill`, `PrecedenceField`, `ProvKind`, `Freshness`, `SuggestButton`, `CompareRow` | 157, 199, 138, 173, 72, 109, 156 |
| `button.rs`, `tab_strip.rs`, `line.rs`, `text.rs`, `motion.rs`, `focus_ring.rs` | `Button` (+`progress`), `TabStrip`, `OneLine`, text styles, easing, `FocusRing` plugin | 244, 164, 68, 151, 83, 93 |
| `histogram.rs`, `sparkline.rs` | charts (`DistributionChart`, `Sparkline`, `ChartFlip`, `TraceOpen`) | 738, 1084 |
| `format.rs` | only `slugify` (filename component); no number formatting | 46 |

### `feeds/` (4 525 lines)

| File | Responsibility | Lines |
|---|---|---|
| `mod.rs` | map doc + `pub use window::draw`; submodules are `pub(crate)` | 55 |
| `window.rs` | Explore window frame | 108 |
| `explore.rs` | nomogram, charts, drag-to-explore | 1439 |
| `compare.rs` | comparison card, Apply-all, power/force rows | 1177 |
| `why.rs` | per-row reasons; `Placement`, `suggest_rows`, `row_basis_rows` | 1211 |
| `shared.rs` | chart helpers used by window, inspector, readiness | 535 |

### `overlays/` (4 460 lines)

| File | Responsibility | Lines |
|---|---|---|
| `mod.rs` | 3 mods | 17 |
| `registry.rs` | `ROWS: &[OverlayRow]` (`:497`), `row(id)`, `dock_section`, `set_overlay`, `apply_overlays` (MCP), `effective_default`, `switch_workspace`, `PREFERENCE_OVERLAYS`, `Legend`, rest-grid helpers | 1899 |
| `live.rs` | per-row Computing / Stale / Failed from live fields | 256 |
| `panel.rs` | catalogue window, `RowState` (7 arms), `settle_pending_show`, Compute rest confirm | 910 |
| `legend_rail.rs` | bottom-right legend; `mirrored` module of render colour literals | 1469 |

### `properties/` (inspector, about 16 000 lines)

| File | Responsibility | Lines |
|---|---|---|
| `mod.rs` (IN FLUX) | `draw` dispatch on `Selection`, `PanelEdit`, `ToolpathTab` (+`key`/`parse` for MCP), snapshot and inputs builders, `build_entry_from_session_and_gui`, `project_entry_onto`, `write_entry_config_to_session`, `apply_auto_enable`, 9 `draw_*_selection` fns | 1619 |
| `toolpath_panel.rs` | `draw_toolpath_panel`: header, tab strip, ribbon, dispatch to tab bodies; per-frame feeds dry run | 1186 |
| `panel_apply.rs` (IN FLUX) | panel doors: `apply_panel_command`, `commit_tool_draft`, `apply_stock_draft`, `apply_setup_draft`, `apply_obstacle_draft`, `apply_head`, machine flushes | 548 |
| `tab_badges.rs` (IN FLUX), `pills.rs`, `feeds_speeds.rs`, `linking_dressup.rs` | tab badges and footer rows; Suggest pills (`PillSuggestions`); Feeds tab; `Param`/`dv`/`dv_pill`/`advanced` row helpers + Linking/Dressup tabs | 618, 554, 924, 670 |
| `tool.rs`, `stock.rs`, `stock_changes.rs`, `setup.rs`, `obstacles.rs`, `post.rs`, `head.rs`, `shoe_row.rs`, `machine_panel.rs`, `model_sim_panels.rs` | one editor per entity | 554, 386, 754, 297, 508, 89, 447, 124, 652, 381 |
| `tests.rs` (IN FLUX) | render-through-harness unit tests | 413 |
| `pins/` (`mod`, `status`, `edit`, `drills`, `plan_view`, `flip_view`) | pin block, pure set edits, drill rows, plan and flip drawings | 471, 143, 280, 197, 513, 297 |
| `operations/mod.rs` (IN FLUX) | re-exports, shared editor helpers, tests | 749 |
| `operations/registry.rs` | `OP_UI_ROWS: &[OpUiRow]` (`:449`), `OpDrawCtx`, `OpDiagram`, one `ed_*` adapter per op via the `editor!` macro (`:188`), validation arms | 600 |
| `operations/{boundary_2d,drill,engrave,finishing,project,surface_3d}.rs` | per-family param editors | 602, 310, 96, 211, 155, 1019 |
| `operations/{shape_diagrams,height_diagram}.rs` | stepover and per-op diagrams; heights diagram with drag | 1200, 611 |
| `operations/validate.rs` (IN FLUX) | `validate_toolpath*` (returns `Vec<String>`), `depth_beyond_stock`, `profile_through_cut`, `collect_diagnostics` | 458 |

## 2. Public entry points and external callers

| Entry point | External callers (path:line) | Count |
|---|---|---|
| `ui::AppEvent` | `src/controller/events/mod.rs:176` (the `match` has no `_ =>` arm), `src/app/input.rs` (46 refs), `src/app/mcp/{view,simulation,commands}.rs`, `src/controller/tests/*`, 12 files under `tests/` | `rg -o`: 87 `ui::AppEvent` paths outside `ui/` |
| `ui::properties::draw` | `src/app.rs:453,582`; `src/app/mcp/commands/stock_changes.rs:634`; `src/controller/tests/mod.rs:341` | 4 |
| Per-surface `draw` | `src/app.rs`: `setup_panel` 447, `status_bar` 467/509/596, `readiness_panel` 535, `toolpath_panel` 571, `sim_timeline` 639, `sim_op_list` 651, `sim_diagnostics` 670, `menu_bar` 982, `workspace_bar` 994, `preflight` 1017, `pin_wizard` 1025, `export_wizard` 1031, `multitool_planner` 1041, `feeds` 1047, `tool_library_modal` 1053, `setup_deletion_modal` 1060, `generation_resolution_modal` 1068, `machine_library_modal` 1082, `preferences` 1090, `sim_trace_modal` 1098, `shortcuts_window` 1112; `src/app/viewport.rs:276` (`viewport_overlay::draw`), `:275` (`overlays::panel::draw_catalogue`) | 28 |
| `ui::overlays::registry` (`switch_workspace`, `apply_overlays`, `row`, `set_overlay`, `selected_area_regions`, `rest_grid_info`, `apply_workspace_defaults`) | `src/app.rs:381`, `src/app/input.rs:345,563`, `src/app/mcp/view.rs:469,559,631`, `src/app/viewport.rs:255,463,775`, `src/app/gpu_upload.rs:1384`, `src/controller/events/mod.rs:487,489`, `src/state/app_settings.rs:79` | 14 |
| `ui::readiness` (`estimate_total_time`, `format_cycle_time`, `holder_clearance_*`, `CycleTimeBasis`) | `src/app/export.rs:297,332`, `src/io/setup_sheet.rs:57`, `src/controller/holder_clearance_*.rs` | 4 non-test files |
| `ui::properties::{ToolpathValidationContext, validate_toolpath_config}` | `src/controller/events/compute.rs:231,233` | 2 |
| `ui::properties::pins::{KeyedPinInputs, keyed_pin_plan}` | `src/controller/events/model.rs:11` | 1 |
| `ui::properties::ToolpathTab` (`key`, `parse`, `ALL`) | `src/state/runtime.rs:293-300` (field type), MCP `set_ui_view` | 2 |
| `ui::pin_wizard::PinWizardState` | `src/state/panels.rs:183`, `src/app/mcp/view.rs:586`, `src/controller/tests/undo.rs:1211` | 3 |
| `ui::preferences::PreferencesState` | `src/state/mod.rs:198,535`; `AppEvent::ApplyPreferences` reads it | 2 |
| `ui::tokens`, `ui::theme` | `src/render/mod.rs:1071`, `src/state/pin_view.rs:82-101`, `src/app/viewport.rs` (6 `theme::`); 53 `tokens` and 6 `theme` paths outside `ui/` | about 65 |
| `ui::components::{Notice, NoticeStack, Role, Button, text, format::slugify}` | `src/app.rs:39,1176,1188`, `src/app/export.rs:2` | 4 |
| `ui::automation` | `src/app.rs:897,1177`, `src/controller/tests/mod.rs:324,337` | 4 |

Tests: 115 files in `tests/`; 24 of them import `rs_cam_viz::ui::properties`, 20 `ui::tokens`, 13 `ui::overlays`, 12 `ui::components`, 12 `ui::AppEvent`. Because `pub mod ui` is crate-public, every `pub` item in `ui/` is test API surface.

## 3. Dependencies and layering

`ui/` imports (path mention counts, `rg -o`): `crate::ui` 546, `crate::state` 322, `crate::ui_command` 24, `crate::render` 16, `crate::io` 10, `crate::controller` 9, `crate::compute` 3. From core: `feeds` 117, `session` 106, `compute` 75, `tool_load` 37, `machine` 36, `diagnostics` 35, `material` 19, `stock` 16, `selection` 14, `maps` 14, `io` 12, then `trace`, `surface`, `geo`, `gcode`. External crates: `egui` 0.36 (`Cargo.toml:16`), `egui_plot` 0.37 (3 files: `feeds/explore.rs:50`, `feeds/shared.rs:29`, `readiness_panel.rs:11`), `rfd` (7 sites), `serde_json` (`sim_debug.rs:131`). No `rs_cam_mcp`, `rs_cam_derive` or `rs_cam_cli`. No threads, channels or `Mutex` in `ui/` (`rg` finds none).

Layering inversions (each is a two-way import with the neighbouring layer):

| Inversion | Evidence |
|---|---|
| Event vocabulary lives in the UI layer; the controller, MCP bridge and input handler import it | `src/ui/mod.rs:80` defines `AppEvent`; `src/controller/events/mod.rs`, `src/app/input.rs`, `src/app/mcp/*` import it |
| `controller` calls UI-folder logic | `src/controller/events/compute.rs:231-233` (`validate_toolpath_config`), `src/controller/events/model.rs:11` (`keyed_pin_plan`), `src/controller/holder_clearance_*` (`ui::readiness::holder_clearance_check`) |
| `io` calls UI-folder logic | `src/io/setup_sheet.rs:57` uses `ui::readiness::{self, CycleTimeBasis}` |
| `app` calls UI-folder logic | `src/app/export.rs:297,332` (`estimate_total_time`, `format_cycle_time`), `:2` (`components::format::slugify`) |
| `state/` holds UI types | `src/state/panels.rs:176,183` (`PartList`, `PinWizardState`), `src/state/mod.rs:198,535` (`PreferencesState`), `src/state/runtime.rs:293-300` (`ToolpathTab`), `src/state/app_settings.rs:79` (calls `ui::overlays::registry::apply_workspace_defaults`), `src/state/pin_view.rs:82-101` (`ui::tokens` colours) |
| `render` reads UI tokens; UI reads render | `src/render/mod.rs:1071` uses `ui::tokens`; `ui` uses `render::{camera,colors,toolpath_render,sim_render}` at `menu_bar.rs:253`, `overlays/panel.rs:21`, `overlays/legend_rail.rs:34,1089,1110,1118,1284`, `sim_timeline.rs:5`, `toolpath_panel.rs:7`, `properties/tool.rs:426`, `properties/model_sim_panels.rs:285` |
| UI reaches into `compute::worker` | `properties/tool.rs:407` (`compute::worker::helpers::build_cutter`) |
| UI calls `controller` | `generation_resolution_modal.rs:12` (`PlanResolutionConfirm`), `properties/pins/mod.rs:96-101` (`controller::pin_drill_blocker`, `PinDrillBlocker`) |
| UI does file IO in the draw pass | `properties/machine_panel.rs:450-454` (`rfd` + `std::fs::read_to_string`), `menu_bar.rs:40-70`, `setup_panel.rs:185`, `model_sim_panels.rs:72` (blocking dialogs) |
| UI computes domain results in the draw pass | `properties/toolpath_panel.rs:511-521` (`feeds::suggest::feeds_result_for_operation` every frame) and `:538` (`PillSuggestions::new`, a dry run of the apply funnel every frame); `sim_timeline.rs:1251` (`estimate_times` loops all boundaries every frame) |

## 4. Traits

| Trait | Defined | Implementors | Note |
|---|---|---|---|
| `UiExt` | `components/section.rs:21` | `egui::Ui` (`:44`) | layout helpers |
| `CycleTimeBasisExt` | `readiness.rs:368` | `rs_cam_core::session::CycleTimeBasis` (`:385`) | extension trait because the type is foreign (orphan rule); `remedy()` strings name GUI menu paths |
| `MissingInputExt` | `readiness.rs:414` | `rs_cam_core::session::MissingInput` (`:419`) | same reason |

Traits from elsewhere that `ui/` implements: `egui::Widget` for `ProvenanceBadge`, `Button`, `KeyValueRow`, `NotMeasured`, `CountPill`, `StatusChip`, `ChartFlip`, `TraceOpen`, `OneLine`, `SuggestButton`, `ChoiceRow` (11 impls); `egui::Plugin` for `FocusRing` (`components/focus_ring.rs:45`). Other components (`Card`, `SectionHeader`, `ValueRow`, `DataTable`, `Details`, `TabStrip`, `Banner`, `Sparkline`, `DistributionChart`, `PrecedenceField`, `Freshness`, `SummaryCard`, `CompareRow`) expose an inherent `fn show(self, ui)` with different return types (`bool`, `ValueRowOutcome`, `DataTableResponse`, `()`); `rg` finds 15 such `show` methods in 12 files. The `components/mod.rs:18-22` doc says "leaf widgets impl `egui::Widget`", so the convention is only half applied. There is no panel trait: panels are free functions `(ui, &mut AppState, &mut Vec<AppEvent>)` or a variant. No core trait (`OpParams`, generator traits) is implemented or called in `ui/`; operation data is read through `OperationConfig` enum matches and the core registry (`op.registry_entry().params.defs`, `OperationConfig::param_range_for_type`).

## 5. Recurring-pattern table

| Concern | Pattern used here | Example path:line | Grade | Deviations inside the area |
|---|---|---|---|---|
| Errors and refusals | Mixed. Core refusals arrive as `Err` from `session.apply`. Panels log (`tracing::warn!`) and drop. Local validators return `Vec<String>` or `Result<_, String>`. One real error enum in `ui/`: `PinPlacementError`. No `anyhow` or `thiserror`. Production `unwrap`/`expect`/`panic`/indexing are denied by workspace lint (`Cargo.toml:37-42`) | `properties/panel_apply.rs:239-241` (refusal goes to the log only); `properties/operations/validate.rs:151` (`Vec<String>`); `properties/pins/mod.rs:254` (`PinPlacementError`) | LINT for panics; NONE for refusal shape | `properties/mod.rs:834` (`.ok()?` drops the refusal), `:1609` (`if let Ok`, no `Err` arm); `properties/panel_apply.rs:320,375` (`Result<(), String>`); `properties/pins/edit.rs:66` (`Result<_, String>`); `preferences.rs:278,290,398,411,505` (`Result<_, String>`); `properties/operations/registry.rs:134` (`unreachable!` in the production lookup `row`) |
| State mutation | Intent route: a draw pushes `AppEvent` (about 255 emit sites); the controller applies `Command` through `ProjectSession::apply`. Inspector route: panels edit a CLONE ("draft") and apply one `Command` themselves through `panel_apply.rs` (the "panel door"), then raise `PanelSideEffects` flags (`upload`, `pin_drill_sync`, `invalidate_simulation`, `state/mod.rs:228`) for the frame loop. View-only writes go through `AppEvent::Ui(UiCommand)` (57 rows in `for_each_ui_command!`) | `properties/panel_apply.rs:227` (`apply_panel_command`); `ui/mod.rs:80` | SENTRY: `tests/egui_draw_sites_write_through_commands_wp6.rs`, `tests/production_writes_go_through_apply_wp15a.rs` (source scans; blind to a receiver not named `session`), `tests/command_surface_completeness.rs`. TYPES for the controller `match` | The post-apply epilogue (`stamp_stale`, `mark_edited`, `invalidate_simulation`) is written at `panel_apply.rs:130-139`, `:339-345`, `:384-390`, `properties/mod.rs:834-846`, `:1609-1618`, while the helper `apply_panel_command` (`:227`) does only the first and last step; `properties/panel_apply.rs:49-70` pushes an undo step into `state.history` by hand beside the command; `properties/mod.rs:823` writes the whole `ToolpathConfig` every frame the panel is open and relies on a core signature gate; `GuiState`/`state.panels.*` fields are written directly in draw code (`machine_panel.rs:455-460`) |
| Parameters and config | Operation params: `Param{op,name,label}` (`linking_dressup.rs:28`); `dv`/`dv_pill` take the range from `OperationConfig::param_range_for_type` and the tooltip from the core registry row. Dressup: `DressupConfig::FIELD_DEFS`. Entities that are not operations (tool, stock, head, machine, shoe, obstacle) use literal ranges in `ValueRow::new(..., lo..=hi)` | `properties/linking_dressup.rs:45-70` | SENTRY: `tests/the_help_key_is_the_param_name_ui04.rs` (arm 5 checks every `dv` key has a range). Not TYPES: `name` is `&'static str` | About 83 literal range sites outside `components/`: `properties/tool.rs:215,242,360`, `properties/stock.rs:151-159`, `properties/head.rs:175,283-286,371-372`, `properties/shoe_row.rs:102`, `properties/machine_panel.rs:362`, `properties/model_sim_panels.rs:167`, `properties/toolpath_panel.rs:999-1022`, `properties/tool.rs:235` (`.range(1..=8)` flutes); a wrong `name` on a path the scan does not read gives an unbounded row with no tooltip |
| Units and frames | `f64` mm everywhere; the unit sits in the identifier (`_mm`, about 255 idents; `_deg`; `turn_rad`) and in the `suffix` string of `ValueRow` (" mm", " deg", " mm/s²"). Drawing uses `f32` egui points (about 383 `f32` mentions). The inspector reads setup-local Z through `HeightContext` (comment at `properties/mod.rs:569`); pin views take the frame from core `PinSetupMap`. Doc rule: mm 2 decimals, coordinates 3, mm/tooth 4 (`components/CLAUDE.md`) | `properties/mod.rs:569`; `properties/tool.rs:215` | CONVENTION (no unit newtype in `ui/`) | `(f64, f64)` tuples for 2D points and ranges: `properties/pins/flip_view.rs:67-70,109,123`, `pins/edit.rs:140`, `pins/plan_view.rs:97`, `stock_changes.rs:104,178` (`stock_z`); degrees in tool fields (`tool.rs:242`) but radians in `flip_view.rs:73`; `feeds_speeds.rs:794` casts `f64` to `f32` for a ramp drawing; about 216 inline `format!("{:.N}")` precision sites against no shared number formatter |
| Identifiers | Core newtypes `ToolpathId`, `SetupId`, `ModelId`, `ObstacleId` (`rs_cam_core/src/ids.rs`), `ToolId` (`rs_cam_core/src/compute/tool_config.rs:8`, not in `ids.rs`). Inside `ui/`, ids come from `state::job::*` / `state::toolpath::ToolpathId` re-exports. Widgets key on `egui::Id`; overlays, automation hooks and params key on `&'static str` | `ui/mod.rs:5`; `overlays/registry.rs:197` (`id: &'static str`) | TYPES for entity ids; CONVENTION for string keys | Raw `usize` ids: `ui/mod.rs:256` (`setup_id: usize`), `:143` (`PinPartModels(Option<Vec<usize>>)`), `:151` (`use_tools: Vec<(f64, usize)>`: a pin DIAMETER as an f64 key), `properties/pins/mod.rs:136,81` (`id: usize`, `tool: Option<(usize,String)>`), `toolpath_panel.rs:58` (`tool_id: usize`), `sim_diagnostics.rs:2252-2269` (`sid: usize`), `properties/mod.rs:284-285,473,479,583,1160,1209` (`ToolId(tc.tool_id)`, `ModelId(m.id)` wrap raw ints at each use); three import paths for one type (`state::toolpath::ToolpathId`, `rs_cam_core::ToolpathId`, `rs_cam_core::ids::ToolpathId`) |
| Background work, progress, cancellation | UI owns no work. It reads `LaneSnapshot`/`LaneState` (`status_bar.rs:28`), pushes `AppEvent::{GenerateAll, CancelGeneration, RunSimulation, PreviewMultitoolPlan}`, and shows progress through `Button::progress(f32)` (`components/button.rs:146`). A compute-and-show overlay waits in a pending slot and settles in `settle_pending_show` (`overlays/panel.rs:605`) | `workspace_bar.rs:92` (cancel); `overlays/panel.rs:605` | DOC + SENTRY (`the_dot_becomes_a_ring_in_flight_g_ring`) | `properties/machine_panel.rs:450-454` and the other `rfd` sites block the UI thread; `request_repaint` is called in only 3 files (`pins/flip_view.rs`, `multitool_planner.rs`, `toolpath_panel.rs`) |
| Caching, freshness, invalidation | Freshness is derived each frame from core results: `FreshnessState` (`state/freshness.rs:26`), `components::Freshness` (one cue), `stamp_stale(state, &effects.stale)` after each command. Overlay `RowState` is derived per frame (`overlays/panel.rs:70`). Drafts live in `state/` (`history.tool_draft`, `panels.grbl.parsed()` parses on change). egui temp memory is allowed for drag and tab state only | `overlays/panel.rs:81-110`; `properties/panel_apply.rs:133` | SENTRY: `tests/panel_drafts_leave_egui_memory_ui09.rs` (names the permitted `insert_temp` sites), `freshness_surfaces_g_freshrender.rs` | Active inspector tab sits in egui temp memory keyed by `ui.id().with("tp_tab").with(entry.id.0)` (`properties/toolpath_panel.rs:288-295`) while `state/runtime.rs:293` holds `pending_toolpath_tab`; `properties/toolpath_panel.rs:511-538` and `sim_timeline.rs:1251` recompute per frame with no cache key; `export_wizard.rs:866` comment accepts a re-export per frame |
| Diagnostics, warnings, messages | One component per message kind: `Notice`/`NoticeStack`, `Banner`, `StatusChip` with `Role` (Ok, Caution, Danger, ...), `NotMeasured` (abstain), `DataRow::note`. Findings come from core (`diagnostics::adapters`, `ToolLoadVerdict`). UI strings are written inline in draw code. A finding has one owner row (the footer) | `components/notice.rs`; `properties/operations/validate.rs:231` | SENTRY: `ui_string_hygiene` (cargo-fmt indentation defect only), `the_inspector_text_is_bounded_ic1`. Text content: NONE | Validation messages are plain `String` (`validate_toolpath -> Vec<String>`, `registry.rs:411-440`), so a caller cannot match on a kind; GUI menu paths are quoted in strings (`readiness.rs:385-440`, "Machine properties > Kinematics > Import GRBL $$") and rot if a menu is renamed; refusals at `panel_apply.rs:239` reach the log only |
| Serialization and wire types | None in `ui/`. Exception: `sim_debug.rs:131` formats `serde_json::Value`. Overlay row ids are the MCP wire names (`overlays/registry.rs:197-199`). `ToolpathTab::key/parse` is the MCP tab vocabulary (`properties/mod.rs:190-230`). `UiCommand` rows carry wire names (`src/ui_command.rs:348`) | `properties/mod.rs:190` | SENTRY: `tests/overlays_registry.rs`, `tests/mcp_wire_surface_pin.rs`, `tests/command_registry_surfaces.rs` | `parse` accepts a second spelling `"feeds_speeds"` (`properties/mod.rs:227`) that `key()` never emits; `PREFERENCE_OVERLAYS` (`registry.rs:1624`) repeats overlay ids as strings |
| Numeric tolerances | Few named constants: `DEPTH_BEYOND_STOCK_EPSILON_MM` aliasing core (`validate.rs:237`), `MODEL_FLAT_EPS_MM = 1e-6` (`height_diagram.rs:131`), `UNITY_TOLERANCE = 1e-3` (`feeds/why.rs:64`), `TIE_EPS_PX` (`height_diagram.rs:32`), pin constants from core (`pins/edit.rs:23-27`) | `properties/operations/validate.rs:237` | CONVENTION | Inline literals: `properties/post.rs:47` (`+ 1e-6`), `overlays/registry.rs:372,382` (`threshold + 1e-6`), `sim_trace_modal.rs:149` (`> 1e-6`); `height_diagram.rs:37` and `pins/plan_view.rs:31` define `*_STEPS_PER_MM = 10.0` twice |
| Geometry primitives | Drawing: `egui::{Pos2, Vec2, Rect}` (`f32`). Data: core `P2`/`P3`/`Polygon2`/`BoundingBox3` (about 10 mentions) and `rs_cam_core::geo`. Pin and flip views use bare `(f64,f64)` | `properties/pins/plan_view.rs`; `properties/mod.rs:60-67` | CONVENTION | `(f64, f64)` tuples in `pins/` and `stock_changes.rs` (see Units row); `toolpath_panel.rs:1429,1450` return `Vec<(f32,f32)>` ranges as tuples |
| Test style and sentries | Three layers: (1) integration sentries in `tests/` named `<claim>_<ticket>.rs` (115 files), many are source scans with a non-vacuity arm and a ratchet budget (`LITERAL_BUDGET = 0` at `panels_read_the_token_module_up1.rs:55`; `RAW_DRAG_VALUE_ALLOWANCE`, `HAND_ROLLED_EMPHASIS` at `component_contracts_up2.rs:880,692`); (2) behavioural headless egui renders (`properties/tests.rs`, `controller/tests/mod.rs:324-360`); (3) about 83 unit tests in 24 `ui/` files. Test modules carry `#[allow(unwrap_used, expect_used, panic, indexing_slicing)]` | `tests/panels_read_the_token_module_up1.rs:1` | SENTRY | The colour sentry counts only `Color32::from_rgb(` (`:122`), so `from_rgba_unmultiplied(100,180,255,..)` at `toolpath_panel.rs:591`, `shape_diagrams.rs:198,322,589,837`, `properties/tool.rs:546`, `sim_timeline.rs:734`, and `Color32::WHITE` at `sim_timeline.rs:591,602,1360` and `model_sim_panels.rs:294` pass; 35 raw `FontId::proportional(<n>)` in `shape_diagrams.rs` (26) and `feeds_speeds.rs` (9); a heuristic scan finds 20 `allow(...)` lines without a `// SAFETY:` within 4 lines (some are test-module allows): `sim_op_list.rs:716,1343`, `preflight.rs:374`, `sim_diagnostics.rs:2384,2755`, `sim_timeline.rs:402,694,810,812`, `multitool_planner.rs:874`, `status_bar.rs:294`, `components/format.rs:23`, `components/notice.rs:435`, `feeds/compare.rs:157`, `feeds/explore.rs:480,1207`, `properties/feeds_speeds.rs:20,164`, `properties/mod.rs:170`, `properties/operations/mod.rs:122` |
| Naming and module layout | One file per surface; `draw` is the entry name; `draw_<thing>` for sub-panels; tickets in file or test names (`g_*`, `ic1`, `dc5`, `up2`, `wp27`). Folder `CLAUDE.md` for 6 of 7 folders (`properties/operations/` has none) | `ui/CLAUDE.md` | CONVENTION | Two `toolpath_panel.rs` (`ui/` rail and `properties/` inspector host); two `compare.rs` (`feeds/`, `components/`); `properties/pills.rs` vs `components/pill.rs`; `pub mod` in most folders but `pub(crate) mod` in `feeds/mod.rs`; `components/format.rs` holds a filename slug only |
| How a feature is registered | Four registries: (a) `AppEvent` variant (compiler-forced match), (b) `for_each_ui_command!` row in `src/ui_command.rs:348`, (c) `OP_UI_ROWS` row per `OperationType` (`properties/operations/registry.rs:449`), (d) `overlays::registry::ROWS` row (`:497`). Dock placement by a `match` on the id string (`dock_section`, `registry.rs:1462`) and by `PATHS_PLACED` strings (`viewport_overlay.rs:688`) | see section 6 | (a) TYPES; (b) SENTRY + TYPES; (c) SENTRY (`operations_registry_ui05`); (d) SENTRY (`overlays_registry`, `the_viewport_dock_reaches_every_option_g_vpdock`) | `viewport_overlay.rs:707,763` and `controller/events/mod.rs:487` do `registry::row("literal")`; a mistyped id returns `None` and the control is silently not drawn (`if let Some(row)`); `app/input.rs:585-586` binds keys to row ids as strings |

## 6. Extension points

| New item | Files and arms touched | Compiler forces | Held only by sentry, doc or luck |
|---|---|---|---|
| New `AppEvent` | variant in `src/ui/mod.rs:80`; emit site in a `draw`; arm in `src/controller/events/mod.rs:176` (and the sub-handlers it calls) | the controller arm (no `_ =>`) | MCP or CLI parity (`command_surface_completeness.rs`) when the event wraps a `Command` |
| New view command or query (camera, modal, playback) | row in `for_each_ui_command!` (`src/ui_command.rs:348`) with payload type and `Surfaces`; handler in `controller/events`; constructor in a draw file; MCP arm if `mcp: Reached` | match arms generated from the macro | `gui: Reached` needs a production constructor (`command_surface_completeness.rs`); a dead row must be deleted by hand |
| New operation (UI side; core adds the `for_each_op!` row first) | `OP_UI_ROWS` row + `ed_*` adapter via `editor!` + a `draw_<op>_params` fn in `properties/operations/<family>.rs` + re-export in `operations/mod.rs`; optional `dia_*` diagram in `shape_diagrams.rs`; optional `val_*` arm in `registry.rs`; `StepoverPattern::from_operation` arm (`shape_diagrams.rs:54` has `_ => None`); height-diagram per-op logic (`height_diagram.rs:90,170,440,545` have `_` arms); `Drill \| AlignmentPinDrill` hard-coded at `feeds_speeds.rs:95` and `properties/mod.rs:1038`. The add menus read `OperationType::ALL_2D/ALL_3D` and `op.spec()` (`toolpath_panel.rs:1153-1190`), so they need no edit | nothing in `ui/`; the `editor!` macro (`registry.rs:188`) swallows a variant mismatch | `operations_registry_ui05` (row exists, diagram reason non-empty); the `Stepover` diagram row draws nothing when `from_operation` returns `None` and no sentry says so; the `_` arms in `height_diagram.rs` |
| New operation parameter | schema row in core (`OpParams`); `dv(ui, p(OperationType::X, "name", "Label"), &mut cfg.field, " mm", speed)` in the family editor; range and help come from the schema | none (name is a string) | `the_help_key_is_the_param_name_ui04` (resolves every `dv` key against the registry) |
| New tool, stock, head, machine or shoe field | field in core record; row with literal `ValueRow::new(.., lo..=hi)` in the entity editor; draft apply in `panel_apply.rs`; setup sheet, project IO and initialisers per the `rs_cam_viz/CLAUDE.md` "audit" rule | struct literals of the record (partly) | range agrees with core validation: DOC only |
| New overlay | `OverlayRow` in `ROWS` (`registry.rs:497`): id, group, surface, mechanism, flag path string, `get`/`set` fns, precondition, `default_for`, hover; state field in `state/overlays.rs` or `state/viewport.rs`; `UploadTime` rows name a trigger; legend arm in `legend_rail.rs` and `registry::Legend`; optional `dock_section` arm; optional `PREFERENCE_OVERLAYS` string; optional `PATHS_PLACED` and popover id lists (`viewport_overlay.rs:688,729`); optional key in `app/input.rs:585` | nothing (`flag` is a path string) | `overlays_registry.rs`, `the_dock_states_read_live_state_g_vpstate.rs`, `the_viewport_dock_reaches_every_option_g_vpdock.rs` |
| New inspector tab for a toolpath | variant in `ToolpathTab` (`properties/mod.rs:178`) + `ALL` + `label` + `key` + `parse`; arm in `properties/toolpath_panel.rs:300` (`match active_tab`); badge in `tab_badges.rs`; MCP `set_ui_view` list reads `ALL` | `match` arms in `label`/`key`/`active_tab` | `ALL` array (hand-written, not derived from the enum); `parse` string arms |
| New Simulation inspector tab | `SimInspectorTab` in `src/state/simulation.rs:1314` (`ALL: [Self; 4]`), `TabStrip::tab(...)` in `sim_diagnostics.rs:82-101` | `match` in `label` | `ALL` length; `the_sim_inspector_is_tabbed_ic3` |
| New component | file in `components/`, `pub mod` + `pub use` in `components/mod.rs`; reads `tokens`; named in `components/CLAUDE.md`; sentry `component_contracts_up2` | none | the state-free rule (no scan found); `Widget` vs `show` shape |
| New colour or text style | token in `tokens.rs` | none | `panels_read_the_token_module_up1` (only `from_rgb(`) |
| New wizard step or modal | draw fn in `ui/`, state in `state/panels.rs` or `state/wizard.rs`, call in `src/app.rs:1017-1112` (about 22 hand-ordered calls), open/close `UiCommand` rows | none | modal draw order (no written rule) |

## 7. CLAUDE.md drift

| Statement | Reality | Where |
|---|---|---|
| `ui/CLAUDE.md:3` "`ui::mod` runs once per frame from `../app.rs`" | `ui/mod.rs` has no `draw` function; `src/app.rs` calls about 22 per-surface `draw` fns itself (`app.rs:447-1112`) | `src/ui/CLAUDE.md:3` |
| `ui/CLAUDE.md:29` "Every Run Simulation affordance uses `simulation_request_is_buildable`" | `readiness_panel.rs:118` and `sim_op_list.rs:50` call it; `menu_bar.rs:231` pushes `AppEvent::RunSimulation` from an always-enabled "Run Simulation" button with no check; the only test that names the predicate is `src/controller/tests/simulation_state.rs` | `src/ui/menu_bar.rs:231` |
| `ui/CLAUDE.md:8` "`theme.rs` re-exports it" (`tokens`) | true, but more than 100 call sites still import `theme::` (`sim_op_list.rs` 31, `sim_diagnostics.rs` 21, `readiness_panel.rs` 19, `preflight.rs` 13, `tool_library_modal.rs` 10, `app/viewport.rs` 6) | `src/ui/theme.rs:1-10` |
| `ui/CLAUDE.md` "The crate is on egui 0.36 with `egui_plot` 0.37" | matches `Cargo.toml:16,24`; `src/lib.rs:20` doc still cites `egui-0.34.3 src/context.rs` | `src/lib.rs:20` |
| `components/CLAUDE.md` file list ends "`button.rs`, `motion.rs`, `text.rs`, etc." | omits `format.rs` (only `slugify`, filename logic) and `focus_ring.rs` | `src/ui/components/CLAUDE.md` |
| `components/mod.rs:18-22` "leaf widgets impl `egui::Widget`" and `:23` "see `planning/ui_audit/ARCHITECTURE.md`" | 15 `show` methods use the inherent form; the planning file no longer exists | `src/ui/components/mod.rs:18,23` |
| `components/CLAUDE.md` "Units: mm 2 decimals, coordinates 3, mm/tooth 4, mm/min 0" | no helper or sentry; about 216 inline `format!("{:.N}")` | `src/ui/components/CLAUDE.md` |
| `feeds/mod.rs:13-14` table says `compare` and `why` are drawn by `ui/properties/mod.rs` | callers are `properties/feeds_speeds.rs` and `readiness_panel.rs` | `src/ui/feeds/mod.rs:13` |
| `feeds/CLAUDE.md` `mod.rs` "the facade: the Explore window AND the inspector card" | `mod.rs` is a map plus `pub use window::draw`; the inspector card is `compare.rs`/`why.rs` | `src/ui/feeds/CLAUDE.md` |
| `feeds/CLAUDE.md` "Do not" ends with a fragment ("An input can lose focus mid-typing; a wrong-looking value can be that bug."); `properties/CLAUDE.md` has the fragment "Helix r: None = rule." | unfinished or context-free sentences | `src/ui/feeds/CLAUDE.md`, `src/ui/properties/CLAUDE.md` (Invariants, first bullet) |
| `properties/CLAUDE.md` "A panel door marks the project edited on success; `app.rs` guards on it" | `apply_panel_command` (`panel_apply.rs:227`) does not mark edited; each caller does (12 `mark_edited` calls in `panel_apply.rs`, 3 in `mod.rs`, 1 in `machine_panel.rs`) | `src/ui/properties/panel_apply.rs:227` |
| `properties/CLAUDE.md` "Do not draw a raw egui widget where `ui/components/` has one" | outside `components/`: about 104 `ui.button`/`Button::new`, 50 `ComboBox`, 52 `checkbox`, 27 `DragValue::new`, 12 `collapsing`/`CollapsingHeader`, 74 `ui.separator()`. Only `DragValue` and emphasis are ratcheted by a sentry (`component_contracts_up2.rs`) | `src/ui/properties/CLAUDE.md` (Invariants) |
| `properties/CLAUDE.md` "Every edit writes through the core command path" | draft commit also pushes a GUI-side `UndoAction::ToolChange` into `state.history` (`panel_apply.rs:49`) beside the command | `src/ui/properties/panel_apply.rs:49` |
| `properties/pins/CLAUDE.md` "`PinDrillState` (one `PinDrillRow` per pin size)" | `PinDrillState` is an enum `NoPins \| Sizes(Vec<PinDrillRow>)` | `src/ui/properties/pins/mod.rs:67` |
| `ui/mod.rs:49-60` "do not reintroduce a field-grained apply event without re-reading `planning/review_2026-08-08/APPLY_CONTRACT_CENSUS.md` §3.4" | file is deleted (also `XVAC_CENSUS.md`, `ORCHESTRATION_LOG.md`, `ui_audit/ARCHITECTURE.md`, `unified_finish_planner_design.md`); the safety reason is inline, the sentry `apply_contract_a3` exists | `src/ui/mod.rs:49-60` |
| `rs_cam_viz/CLAUDE.md` "GUI state is not an alternate data model" | the inspector copies a `ToolpathEntry` from session + GUI every frame, edits it, and projects it back (`properties/mod.rs:678,746,823`) | `src/ui/properties/mod.rs:678` |

All 24 integration sentry names listed in the `ui/` `CLAUDE.md` files exist in `tests/`. The two `--lib` filters `a_setup_map_op_draws_no_boundary_controls_d3` and `pin_edit_one_door_p5` exist (`src/ui/properties/tests.rs`, `src/controller/tests/undo.rs`). Every file in the `CLAUDE.md` file maps exists. `legend_rail::mirrored` is a `pub mod` (`legend_rail.rs:1246`), not a function.

## 8. Navigability

| Item | Detail |
|---|---|
| Files over about 1500 lines | `sim_diagnostics.rs` 2841 (cut-metric cards, limit rows, verdict logic `limit_rows`/`is_advisory`, spans, trace disclosures, Warnings tab); `overlays/registry.rs` 1899 (data table + lookup + MCP apply + workspace defaults + legend choice + rest-grid helpers); `toolpath_panel.rs` 1626 (cards, menus, connector geometry, drag/drop); `properties/mod.rs` 1619 (IN FLUX; dispatcher + entry projection + inputs builders + 9 selection draws); `sim_timeline.rs` 1514 |
| Near the limit | `legend_rail.rs` 1469, `feeds/explore.rs` 1439, `sim_op_list.rs` 1402, `preferences.rs` 1342, `export_wizard.rs` 1227, `feeds/why.rs` 1211, `shape_diagrams.rs` 1200, `properties/toolpath_panel.rs` 1186, `feeds/compare.rs` 1177, `components/sparkline.rs` 1084 |
| Logic that is not UI, in `ui/` | `readiness.rs` (cycle time, holder-clearance verdict, buildable predicate) is read by `io`, `app`, `controller`; `properties/operations/validate.rs` is called by the controller; `properties/pins/mod.rs::keyed_pin_plan` is called by the controller; `properties/mod.rs` entry projection (`project_entry_onto`) |
| State types in the wrong layer | `PinWizardState`, `PreferencesState`, `ToolpathTab`, `PartList`, `AppEvent` belong with `state/` or an events module |
| Misleading names | `properties/toolpath_panel.rs` (tab host) vs `ui/toolpath_panel.rs` (rail); `feeds/compare.rs` vs `components/compare.rs`; `components/format.rs` (no formatting); `theme.rs` (old name of `tokens`); `operations/mod.rs` (helpers + tests; the dispatcher is `registry.rs`); `feeds/` (Explore window AND inspector card); `properties/linking_dressup.rs` (also holds the row helpers `dv`, `p`, `Param`, `advanced` every editor uses) |
| Layout | `src/app.rs` and `src/app/` coexist; `ui/mod.rs:49-60` holds a 12-line comment about a deleted enum |
| Public surface | `pub mod ui` plus `pub` items make internals test API; `feeds` submodules are `pub(crate)` while siblings are `pub` |

## 9. Candidate debt leads (UNVERIFIED)

1. `src/ui/properties/panel_apply.rs:130-139,339-345,384-390` and `src/ui/properties/mod.rs:834-846,1609-1618`: the apply-and-stamp epilogue is written five times beside `apply_panel_command` (`:227`), which covers only part of it. A new door can skip `invalidate_simulation`.
2. `src/ui/properties/panel_apply.rs:239-241`: a refused panel edit is logged with `tracing::warn!` and the operator sees nothing. `src/ui/properties/mod.rs:834` (`.ok()?`) and `:1609` (`if let Ok`) drop the refusal without a log.
3. `src/ui/properties/mod.rs:823-856`: the whole `ToolpathConfig` is replaced on every frame the panel is open. Correctness rests on core's `generation_inputs_signature` gate. IN FLUX.
4. `src/ui/properties/mod.rs:678-800` (`ToolpathEntry` projection): a second, GUI-side copy of the toolpath record. A new `ToolpathConfig` field that `project_entry_onto` does not copy is lost on the next frame.
5. `src/ui/properties/toolpath_panel.rs:511-538`: `feeds_result_for_operation` and a `PillSuggestions` dry run of the apply funnel run on every frame with no cache key.
6. `src/ui/sim_timeline.rs:1251`: `estimate_times` loops every boundary and recomputes cycle time on every frame.
7. `src/ui/properties/machine_panel.rs:450-454`, `menu_bar.rs:40-70`, `setup_panel.rs:185`, `properties/model_sim_panels.rs:72`: blocking file dialog and `std::fs` read inside a draw pass.
8. `src/ui/overlays/registry.rs:197-226`: `flag: Option<&'static str>` is a field path as a string; `dock_section` (`:1462`) matches ids as strings; `PREFERENCE_OVERLAYS` (`:1624`) and `viewport_overlay.rs:688-735` repeat the ids. A typo compiles and the control disappears (`if let Some(row) = registry::row(..)`).
9. `src/ui/properties/operations/registry.rs:188` (`editor!` macro) and `:134` (`unreachable!` in `row`): the adapter ignores a non-matching variant; the lookup panics if a row is missing. Only `operations_registry_ui05` holds the table against `OperationType::ALL`.
10. `src/ui/properties/operations/shape_diagrams.rs:54`: `StepoverPattern::from_operation` ends `_ => None`, and `OpDiagram::Stepover` rows draw nothing in that case. The registry doc says the silent arm was removed.
11. `src/ui/properties/operations/height_diagram.rs:80-95,160-172,430-442,540-548`: line index `0..4` as `usize` with a `_ =>` arm for the bottom line; an enum would be total.
12. `src/ui/properties/feeds_speeds.rs:95` and `src/ui/properties/mod.rs:1038`: `Drill | AlignmentPinDrill` is hard-coded where a core capability flag may answer (`honors_pinned_bottom_z` shows the pattern at `operations/mod.rs:252`).
13. `src/ui/mod.rs:143,151,256`: `PinPartModels(Option<Vec<usize>>)`, `PinWizardApply { add_drills: Vec<f64>, use_tools: Vec<(f64, usize)> }`, `WizardSetSetupPauseMessage { setup_id: usize }`: raw ids and a float diameter as a key, beside the newtypes `ModelId`, `ToolId`, `SetupId`.
14. `src/ui/properties/pins/mod.rs:41-48`: `tool: Option<(usize, String)>`, `candidates: Vec<(usize, String)>`: tuple ids instead of `ToolId`.
15. `src/ui/properties/mod.rs:170`: `#[allow(clippy::too_many_arguments)]` sits above the `ToolpathTab` enum (a stray attribute) without a SAFETY line. IN FLUX.
16. About 20 `allow(...)` lines without `// SAFETY:` (list in section 5, test-style row; some may be test modules). The root rule requires one for production code.
17. `tests/panels_read_the_token_module_up1.rs:122`: the colour budget matches only `Color32::from_rgb(`; `from_rgba_*` and `Color32::WHITE` slip through (sites in section 5). The 35 raw `FontId::proportional(<n>)` in `shape_diagrams.rs` and `feeds_speeds.rs` bypass `tokens`.
18. `src/ui/theme.rs` (78 lines, 22 `pub` items) has more than 100 live call sites in 8 files, while its doc says it "shrinks".
19. `src/ui/sim_diagnostics.rs:2059` and `src/ui/components/data_table.rs:422`: two private `capitalised` fns. `properties/pins/mod.rs:191` `wrapped` and `properties/tab_badges.rs:123` `wrapped_small_label` are two wrap helpers.
20. `src/ui/properties/{tool,stock,head,shoe_row,machine_panel}.rs`: about 83 literal `lo..=hi` ranges that duplicate, and may disagree with, core validation. Operation params already use the schema range.
21. `src/ui/readiness.rs:385-440`: GUI path strings inside remedy text ("Machine properties > Kinematics > Import GRBL $$", "Simulation > Run Simulation") are not tied to the menu labels.
22. `src/ui/properties/mod.rs:227`: `ToolpathTab::parse` accepts `"feeds_speeds"` that `key()` never returns.
23. `src/ui/properties/toolpath_panel.rs:288-295`: active tab in egui temp memory keyed by the string `"tp_tab"`, while `state/runtime.rs:293` carries `pending_toolpath_tab`. Two stores for one fact.
24. `src/ui/menu_bar.rs:231`: the Simulation menu "Run Simulation" has no `simulation_request_is_buildable` gate (see section 7).
25. `src/ui/mod.rs:23-60`: a tombstone comment for a deleted enum points to a deleted planning file. The sentry `apply_contract_a3::per_field_apply_affordance_no_longer_exists` is the real guard.
26. `src/ui/components/format.rs`: a filename slug in a "formatting" module, used by `src/app/export.rs:2`; about 216 inline `{:.N}` formats duplicate the unit rules in `components/CLAUDE.md`.
27. `src/ui/properties/panel_apply.rs:49-70`: undo for a tool edit is a GUI-side `UndoAction::ToolChange{old,new}` in `state/history.rs` (IN FLUX), separate from the `Command` that changes the session. Two undo records for one edit.
28. `src/ui/automation.rs`: `record(ui, &'static str, ...)` ids are free strings; ids that tests read (`status_lane_*`, `viewport_catalogue`, `overlay_cancel_all`) have no registry in this folder.
29. Component convention split (`Widget` vs `show`): callers cannot tell which form a component takes (section 4).
30. `src/ui/properties/linking_dressup.rs:45-52` (`schema_range`): a param with no declared range gets `NEG_INFINITY..=INFINITY`, so a `dv` row for an unknown name is an unbounded widget; only the `ui04` source scan prevents it.
