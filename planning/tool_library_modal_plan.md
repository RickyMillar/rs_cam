# Tool Library modal — implementation plan

Goal: a dedicated **Tool Library** management screen in the GUI
(`rs_cam_viz`) to browse the per-user tool catalogs
(`~/.config/rs_cam/tools/*.toml`), preview a tool, add one to the open
project, and delete tools — without walking the `Add Tool ▸ From library`
submenu. Build **Phase 1** in one pass; Phase 2 listed for later.

## Architectural constraints (verified)

- **Modal `draw` gets `&AppState` (immutable)** — `feeds_modal::draw(ctx,
  state: &AppState, events: &mut Vec<AppEvent>)` (`ui/feeds_modal.rs:30`)
  and `controller.rs:117 state_ref_and_events_mut() -> (&AppState, &mut
  Vec<AppEvent>)`. So the modal **reads** state and routes **every**
  change through `AppEvent`s. Ephemeral view state (selected catalog,
  selected tool index, filter text) uses **egui temp memory**
  (`ui.data_mut`/`ui.data`), exactly like the machine/tool "Save to
  library" buffers already do.
- **No per-frame disk I/O**: the controller loads a catalog snapshot into
  the modal state when it opens (and after any mutation). The modal
  renders from that snapshot.
- **Cannot visually verify egui headless** — get it compiling + clippy
  clean + controller tests green; user confirms layout after a rebuild
  (`cargo build --release -p rs_cam_viz --bin rs_cam_gui`, relaunch).
- Zero-warning clippy (16 deny lints). Pre-existing untracked
  `feeds/explain.rs` + `feeds_modal.rs` stay UNSTAGED. The GUI saves/loads
  via core `ProjectSession` (unified state).

## Reusable pieces (verified)

- `draw_tool_preview(ui, tool: &ToolConfig)` — `ui/properties/tool.rs:255`,
  **private** → make `pub(crate)`. Uses
  `crate::compute::worker::helpers::build_cutter(tool)` (`tool.rs:268`),
  in-crate, fine to call from the modal.
- `ToolConfig::summary()` and `ToolType::label()` — `compute/tool_config.rs`
  (~240 / 27) for row/detail labels.
- Angle fields: `included_angle` (V-bit, full) vs `taper_half_angle`
  (tapered ball, per-side) — show the relevant one per `tool_type`.
- `tool_library` API (`rs_cam_core/src/tool_library.rs`): `list_libraries()`,
  `load_library(name) -> Result<ToolCatalog>`, `save_library(name,
  &ToolCatalog)`, `ToolCatalog { tools: Vec<ToolConfig> }`. Name validation
  via `path_in`/`name_is_valid` (reuse for new fns).
- Two-pane reference: `ui/feeds_modal.rs:132+` uses `egui::SidePanel::left(..)
  .show_inside(ui, ..)` + `egui::CentralPanel::default().show_inside(..)`.
  Selectable list: `ui/project_tree.rs:98+` `selectable_label` + `.context_menu`.
- egui_extras / TableBuilder is **NOT** a dep — use `ScrollArea` +
  `selectable_label` rows with fixed-width `ui.horizontal` columns.

## Phase 1 — browse + preview + add-to-project + delete-tool

### Core (`crates/rs_cam_core/src/tool_library.rs`)
Add (reuse `path_in`, mirror existing `_from`/`_library` pairing + error style):
```rust
pub fn remove_tool_at_in(dir,&name,index) -> Result<ToolCatalog,Err> // load, remove, save, return new
pub fn remove_tool_at(name,index) -> Result<ToolCatalog,Err>         // resolved-dir wrapper
```
(Delete-whole-catalog + rename are Phase 2: `delete_from/delete_library`,
`rename_in/rename_library`.) Add unit tests (dir-parameterized, like the
existing ones) for remove_tool_at_in: removes the right index, persists.

### State (`crates/rs_cam_viz/src/state/mod.rs`)
- Define near the other modal states:
  ```rust
  pub struct ToolLibraryModalState {
      /// Cached snapshot loaded when the modal opens / after mutations.
      pub catalogs: Vec<(String, rs_cam_core::tool_library::ToolCatalog)>,
  }
  ```
- Add field on `AppState` (after `feeds_modal`, line 83):
  `pub tool_library_modal: Option<ToolLibraryModalState>,`
- Init `None` in `AppState::new()` (line ~208 area).

### Events (`crates/rs_cam_viz/src/ui/mod.rs`, enum at line 47)
```rust
OpenToolLibrary,
CloseToolLibrary,
DeleteLibraryTool { catalog: String, index: usize },
// AddToolFromLibrary(Box<ToolConfig>) already exists — reuse for "Add to project".
```
Add `pub mod tool_library_modal;` to the module list (~line 7).

### Dispatch + handlers (`controller/events/mod.rs` match ~line 15; impl in `controller/events/model.rs`)
- `OpenToolLibrary => self.open_tool_library()` — load snapshot:
  `let catalogs = tool_library::list_libraries().iter().filter_map(|n|
  tool_library::load_library(n).ok().map(|c|(n.clone(),c))).collect();
  self.state.tool_library_modal = Some(ToolLibraryModalState{catalogs});`
- `CloseToolLibrary => self.state.tool_library_modal = None;`
- `DeleteLibraryTool{catalog,index} => self.delete_library_tool(&catalog,index)`
  — `tool_library::remove_tool_at(&catalog,index)`; on Ok refresh the
  snapshot (re-run the open-load); log + ignore Err (or toast).
- `AddToolFromLibrary` already dispatched → `handle_add_tool_from_library`
  (`model.rs:58`) which `session.add_tool` + selects + `mark_edited`.

### Preview helper (`ui/properties/tool.rs:255`)
`fn draw_tool_preview` → `pub(crate) fn draw_tool_preview`.

### Open trigger (`crates/rs_cam_viz/src/ui/menu_bar.rs`)
In the File menu (or a top-level button) push `AppEvent::OpenToolLibrary`
(`ui.close_menu()` after). Optionally also a "Manage…" small_button in the
project-tree Tool Library section (`project_tree.rs:98`).

### Modal (`crates/rs_cam_viz/src/ui/tool_library_modal.rs`, NEW)
```rust
pub fn draw(ctx: &egui::Context, state: &AppState, events: &mut Vec<AppEvent>) {
    let Some(modal) = state.tool_library_modal.as_ref() else { return };
    let mut still_open = true;
    egui::Window::new("Tool Library").collapsible(false).resizable(true)
        .anchor(egui::Align2::CENTER_CENTER,[0.0,0.0])
        .default_width(820.0).default_height(520.0)
        .open(&mut still_open)
        .show(ctx, |ui| draw_content(ui, modal, events));
    if !still_open { events.push(AppEvent::CloseToolLibrary); }
}
```
`draw_content`: read selection from egui temp memory —
`sel_cat: Option<String>` (id "toollib_cat"), `sel_idx: Option<usize>`
(id "toollib_idx"), `filter: String` (id "toollib_filter").
- **Left `SidePanel`**: a `ScrollArea` listing `modal.catalogs` names via
  `selectable_label(sel_cat==name, format!("{name} ({} tools)", n))`; on
  click set sel_cat in temp memory, clear sel_idx. Below it, a filter
  `TextEdit`, then the selected catalog's tools as selectable rows:
  `ui.horizontal` columns [name (min 160) | type label (90) | ⌀ (70) |
  flutes (40) | angle]; angle column shows `included_angle`° for v_bit
  else `taper_half_angle`° for tapered else "—". Filter by name substring.
  Clicking a row sets sel_idx.
- **Right `CentralPanel`**: if a tool selected → `draw_tool_preview(ui,
  tool)` + a read-only `egui::Grid` of its fields (name/type/Ø/cutting
  length/flutes/relevant angle/shank/material/vendor). Action buttons:
  - **Add to project** → `events.push(AppEvent::AddToolFromLibrary(
    Box::new(tool.clone())))`.
  - **Delete from catalog** (confirm via a second click / are-you-sure
    using temp-memory bool) → `events.push(AppEvent::DeleteLibraryTool{
    catalog: cat.clone(), index})`; clear sel_idx.

### Render wiring (`crates/rs_cam_viz/src/app.rs`, after feeds modal ~line 540)
```rust
if self.controller.state().tool_library_modal.is_some() {
    let (state, events) = self.controller.state_ref_and_events_mut();
    crate::ui::tool_library_modal::draw(ctx, state, events);
}
```

### Verify
- `cargo build -p rs_cam_viz` (debug) + `cargo clippy -p rs_cam_core -p
  rs_cam_viz --tests` zero warnings.
- `cargo test -p rs_cam_core --lib tool_library` (new remove test).
- `cargo test -p rs_cam_viz --lib controller::tests` green.
- Tell user to rebuild release + relaunch to view; layout unverified by me.

## Phase 2 (later, not this pass)
Rename tool, move tool between catalogs, edit-in-place, new/delete/rename
whole catalog (core: `delete_from/delete_library`, `rename_in/rename_library`),
search across all catalogs, dedupe button.

## Commit shape
1. core: `remove_tool_at` + tests.
2. viz: state field + events + handlers + preview pub + menu trigger +
   modal file + app.rs wiring. (One viz commit; keep feeds files unstaged.)
