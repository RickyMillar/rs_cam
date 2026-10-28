# F. The selection contract and the surface parity audit

Design only. No product file changed. No cargo command ran. Code read at
HEAD b5aa2a45 (2026-10-28). Abbreviations: `V/` = `crates/rs_cam_viz/src/`,
`C/` = `crates/rs_cam_core/src/`, `M/` = `crates/rs_cam_mcp/src/`,
`L/` = `crates/rs_cam_cli/src/`. Code blocks are design. Nobody compiled them.

The operator's ask: "make sure the interface is well designed for the rest
of the app (what is selected.. etc) and the gui/mcp/cli and that all have
good parity." This file gives the contract that designs A-E plug into, and
the audit of the surfaces today.

## 1. Inventory: every notion of "selected"

| # | Notion | Type and place | Kind | Writers | Readers |
|---|---|---|---|---|---|
| N1 | Tree focus | `Selection` enum, `V/state/selection.rs:8-23`; the field `AppState.selection` is `pub` (`V/state/mod.rs:151`, also `split_mut` `:237`) | view | `UiCommand::Select` → `handle_select` (`V/controller/events/model.rs:18-45`); about 40 direct `state.selection =` writes (`rg -c`): `V/app/viewport.rs` 5, `V/app/mcp/commands.rs` 6, `V/app/mcp/view.rs` 2, `V/app/mcp.rs:1001`, `V/app/mcp/generation.rs:380`, `V/app/mcp/commands/selection.rs:1400`, `V/app/mcp/commands/stock_changes.rs` 2, `V/controller/events/*.rs` 19, `V/app.rs:381` | properties panel `V/ui/properties/mod.rs:165-178`; `vector_source_toolpath` `V/state/viewport.rs:360-375`; keys `V/app/input.rs:439-460`; MCP echo `V/app/mcp/view.rs:688-714` |
| N2 | Op selection (project) | core `Selection { rules, picks, excludes }`, `C/selection/rule.rs:351-362`; edits `SelectionEdit` `:371-391`; target `SelectionTarget { Op, Boundary }` `C/session/selection_status.rs:90-97`; `for_face_pick` `:103-108` | project | `Command::EditSelection` (`C/session/command.rs:645-651`, `:2846-2848`), and `SetBoundaryConfig` for the L-8 switch (`ClickEdit::BoundaryToSelection`, `selection_status.rs:188-203`) | `selection_status` → `SelectionReport` (`selection_status.rs:267-340`, memo) |
| N3 | Pick mode | `SelectionUi.mode: Option<ViewportMode { Pick, Tabs }>`, `V/state/pick.rs:203-232` (L-3: one mode at a time) | view | panel buttons `V/ui/properties/selection_section.rs:200`, `V/ui/properties/geometry_tab.rs:256`; pick bar `V/ui/pick_bar.rs:233`; Escape `V/app/input.rs:429-433`; `keep_only` `V/ui/properties/mod.rs:159`, `:1677` | `active_pick`, `pick_target` (`V/app/viewport.rs:573-578`) |
| N4 | Row hover | `SelectionUi.row_hover: Option<MatchKey>`, `V/state/pick.rs:228` | view | `V/ui/properties/selection_section.rs:721`; reset `V/ui/properties/geometry_tab.rs:50` | `V/app/viewport.rs:679` |
| N5 | Item hover | `HoverDetector { last }`, `V/state/selection_highlight.rs:132-151`, on `RsCamApp` (`V/app.rs:101`) | view | `V/app/viewport.rs:680` | `V/app/gpu_upload.rs:154` |
| N6 | Face hover | `last_hover_face: Option<FaceGroupId>`, `V/app.rs:82` | view | `V/app/viewport.rs:725-768` | `V/app/gpu_upload.rs:55-57` |
| N7 | Simulation focus | `simulation.focused_toolpath()` | view | the sim op list (`V/ui/sim_op_list.rs:400` sends `UiCommand::Select`) and playback | `vector_source_toolpath`; `V/ui/sim_timeline.rs:500`; `V/ui/sim_diagnostics.rs:1679` |
| N8 | Frame flash | `ViewportState.selection_flash`, `V/state/viewport.rs:215-220` | view | `frame_selection_items` `V/app.rs:454-472` | the drawing upload |
| N9 | Camera | `OrbitCamera` on `RsCamApp.camera`, `V/render/camera.rs:20-29` | view | `UiCommand::SetViewPreset`, `ToggleProjection`, `ResetView` (`V/app/input.rs:274-303`); `fit_camera_to_bbox` `V/app.rs:440` | renderer, picking |

### 1.1 Findings

- **F-1. `Selection::Face` and `Selection::Faces` are dead.** No production
  code constructs them. `rg 'Selection::Faces?\s*\('` without match arms
  gives only `V/controller/workflow_tests.rs:1284`, `:1309`. Each other
  hit is a match arm: `V/app/gpu_upload.rs:48-49`, `V/ui/setup_panel.rs:136`,
  `V/controller/events/toolpath.rs:63-81`, `V/controller/events/model.rs:879`,
  `V/ui/properties/mod.rs:173-176`, `:1557-1616`. A face click in Toolpaths
  sends `TogglePick` (`V/app/viewport.rs:563-607`). Other workspaces drop it
  (`:605`). The WP23 rule in `V/ui_command.rs:41-45` applies: "A dispatch
  arm is not a caller." Delete the variants and their arms.
- **F-2. REPORT A.1 SEL-3 is stale.** It says that outside pick mode
  shift-click accumulates `Selection::Faces`. `handle_viewport_click`
  (`V/app/viewport.rs:356-371`) passes `shift` only to the pick mode
  (`PickClick::Shift`). SEL-12 ("the tree face selection has no undo") is
  therefore moot.
- **F-3. The tree focus has two write paths.** `handle_select` sets
  `pending_upload` on a change (`model.rs:31-43`; the comment records the
  stale height-plane bug that this fixes). The direct writes skip that
  rule. MCP `set_ui_view toolpath_index` (`V/app/mcp/view.rs:488`) is one
  of them. Whether a later upload covers each direct write is NOT MEASURED.
- **F-4. Two answers to "which toolpath is lit".** The drawing highlight
  reads `highlight_toolpath` → `vector_source_toolpath` (in Simulation, the
  sim focus). The face highlight `selected_face_ids` reads the tree
  `Selection::Toolpath` (`V/app/gpu_upload.rs:30-46`). They differ in the
  Simulation workspace.
- **F-5. Three hover slots** (N4, N5, N6) feed two buffers. D's per-face
  state buffer needs one hover answer.
- **F-6. Pick mode has no command row.** Panels write `SelectionUi`
  directly. It is view state, so that is legal. MCP can neither read nor
  set it. Section 5 rules that MCP needs no write door for it.
- **F-7. MCP has no read door for the view focus.** `project_summary`
  (`V/app/mcp/project.rs:20`) reports no focus, workspace or pick mode. Only
  the echo of the write tool `set_ui_view` gives a 5-arm `selection` kind
  (`view.rs:708-714`, `"other"` for Model, Setup, Obstacle). `select` takes
  Machine and Stock only (`view.rs:530-545`).
- **F-8. Undo of a selection edit is coalesced.** `apply_selection_edit`
  (`V/controller/events/mod.rs:109-143`) pushes no step. The edit rides the
  inspector snapshot (`V/ui/properties/mod.rs:1623-1633`), which flushes
  when the operator leaves the toolpath. So ten clicks are one step today,
  together with every panel edit since the snapshot. Tab edits push one
  step per act (`mod.rs:255-300`). An MCP edit enters undo only if the
  inspector took a snapshot of that toolpath first: NOT MEASURED.
- **F-9. `SelectionEditParam` converts in one direction only**
  (`M/server.rs:2084-2095`). The F166 rule in `crates/rs_cam_mcp/CLAUDE.md`
  asks for an exhaustive `match` in both directions. Today a new core
  variant (E's `AddPicks`) compiles with no wire variant.
- **F-10. The CLI job has no boundary selection.** The job key `selection`
  writes `SelectionTarget::Op` only (`L/job.rs:1294-1301`).
- **F-11. A boundary click and MCP differ by design.** A GUI click on a
  boundary whose source is not `Selection` sends one `SetBoundaryConfig`
  (`V/controller/events/mod.rs:165-190`). MCP `edit_toolpath_selection
  target=boundary` refuses on that boundary. The agent calls
  `set_boundary_config` first. The project state is the same after both.

## 2. The surfaces and the parity matrix

The CLI subcommands are `job`, `run`, `sweep`, `project`, `smoke`,
`nc-time`, `rough-score`, `version` (`L/main.rs:151-508`). No `selection`
subcommand exists. A job `[[operation]]` takes the key `selection` in the
project-file shape (`L/job.rs:237-243`, `JOB_KEYS` `:258-271`) as one
`SelectionEdit::Replace`. `project` reads a project file, which stores
each op selection. The CLI reads `selection_status` only in a test
(`L/job.rs:1489`); the registry row says `cli: Skip` (`C/session/command.rs:581-589`).

| Action | GUI door | MCP door | CLI door | Project file | Undo |
|---|---|---|---|---|---|
| Select a tree item | `UiCommand::Select` (`V/ui/setup_panel.rs:113-321`, `V/ui/toolpath_panel.rs:615`); viewport direct writes (`V/app/viewport.rs:509-549`) | `set_ui_view {toolpath_index, select}` (`V/app/mcp/view.rs:472-545`): toolpath, machine, stock only | not applicable: no tree | not stored | none (view) |
| Add a pick | pick-mode shift-click → `AppEvent::AddPick` (`V/ui/pick_bar.rs:78`) → `click_edit` → `EditSelection` | `edit_toolpath_selection {add_pick}` | `[[operation.selection.picks]]` (Replace) | `operation.params.selection.picks` | coalesced snapshot (F-8) |
| Toggle a pick (click) | `AppEvent::TogglePick` (`V/app/viewport.rs:553-561`, `:588-596`) → `click_edit` | GAP by design: the agent reads `selection_status.selected` and sends add or remove | Replace | as above | as above |
| Remove a pick | toggle on a held item | `edit_toolpath_selection {remove_pick}` | Replace | `excludes` or fewer `picks` | as above |
| Batch picks | GAP (E: `AddPicks`) | GAP (E) | already: a list of `picks` | already | — |
| Add a layer rule | pick-mode right-click → `PickLayer` → `layer_rule_edit` (`V/controller/events/mod.rs:580-605`) | `edit_toolpath_selection {add_rule}` | `[[operation.selection.rules]]` (`crates/rs_cam_cli/tests/a_job_layer_rule_cuts_only_its_layer_p2_7.rs:127-129`) | `rules` | as above |
| Rule from a picked item ("Rule…") | Geometry tab offers (`rule_offers`, `V/ui/properties/selection_section.rs:22`) | `add_rule` with the same rule | `rules` | `rules` | as above |
| Select similar | GAP (E) | `add_rule` (E: same rule value) | `rules` | `rules` | — |
| Reset to default | Reset button → `SelectionEdit::Reset` (`selection_section.rs:220`) | `edit_toolpath_selection "reset"` | omit the key | key absent = op default | as above |
| Clear (empty) | GAP | `{replace: {}}` = `Selection::empty()` (`C/selection/rule.rs:476`) | `selection = {}` | empty table | — |
| Pick mode on or off | panel Pick button; Escape | not applicable: the agent names keys | not applicable | not stored | none |
| Frame items | "Show me" → `AppEvent::FrameSelectionItems` → `frame_selection_items` (`V/app/input.rs:223-228`) | `set_ui_view {frame_items}` → same method (`V/app/mcp/view.rs:647-663`) | not applicable: no view | not stored | none |
| Camera preset | keys 1-4 (`V/app/input.rs:485-510`), menu, dock → `UiCommand::SetViewPreset` | GAP (row says `Skip`, `V/ui_command.rs:390-395`) | not applicable | not stored | none |
| Projection | `UiCommand::ToggleProjection` | GAP | not applicable | not stored | none |
| Fit / reset view | `UiCommand::ResetView` | GAP | not applicable | not stored | none |
| Camera pose | orbit, pan, zoom (`V/app/viewport.rs:848-881`) | GAP | not applicable | not stored | none |
| Model visibility | eye → `UiCommand::SetModelVisibility` | `set_ui_view {model_visibility}` (`view.rs:620-640`) | not applicable | not stored | none |
| Box select | GAP (B) | covered by batch picks | covered by `picks` | — | — |
| Hover | pointer | not applicable: no pointer | not applicable | — | — |
| Read the op selection | Geometry tab via the `SelectionStatus` query (`V/ui/properties/mod.rs:442-457`) | `selection_status` (`V/app/mcp/commands/selection.rs:355-361`) | refusal text on the run | — | — |
| Read the view focus | on screen | GAP (F-7) | not applicable | — | — |

### 2.1 The parity sentries that exist, and their pattern

| Sentry | Pattern |
|---|---|
| `V/tests/command_surface_completeness.rs` | Census over both registries: a `gui: Reached` row has a constructor in a production view file; an `mcp: Skip` view row has no tool; no wire name twice. |
| `V/tests/mcp_core_arm_describes_every_row.rs` | Builds one `CoreRequest` per row from `CoreRequest::all_defaults()` (no typed list) and scans the describe step. Records that `RsCamApp` cannot be built in a test (`:12-17`). |
| `V/tests/mcp_authoring_surface.rs` | Reads the schema and description text from the real `ToolRouter`. |
| `V/tests/mcp_wire_surface_pin.rs` | Snapshot `tests/snapshots/mcp_wire_surface.json` of every tool's `inputSchema`; `set_ui_view` is in it. |
| `V/tests/every_add_door_takes_one_model_and_tool_q7.rs` | Same fixture, two doors, same stored op; a source scan pins the arms that need `RsCamApp`. |
| `V/tests/export_parity_core_vs_gui_p0.rs` | Same project, core and GUI export, byte compare. |
| `mcp_and_gui_give_one_*` lib tests (`V/app/mcp/commands/selection.rs:1349`, `:1713`, `:1844`; `floor.rs:160`) | Two project copies; one takes the GUI edit (`pick_edit`, `layer_rule_edit`), one takes the wire JSON (`wire_edit`); assert equal stored selection and report (`selection.rs:960-1000`). |

New actions take the last pattern for project state and the census for
view rows.

## 3. How "what is selected" reaches the rest of the app

| Consumer | Reads | Door |
|---|---|---|
| Properties panel | N1 | `match state.selection` (`V/ui/properties/mod.rs:165-178`) |
| Geometry tab | N2 report | `SelectionStatus` query (`mod.rs:442-457`) |
| Drawing highlight (L-9) | N2 report of the drawn toolpath | `highlight_report` (`V/state/selection_highlight.rs:41-48`); upload on a new `Arc` (`ReportDetector`, `:88-121`) |
| Face highlight | N2 `selected` face keys | `selected_face_ids` (`V/app/gpu_upload.rs:28-51`), see F-4 |
| Simulation workspace | N7 | `focused_toolpath()` |
| Status bar | nothing | `V/ui/status_bar.rs` reads no selection (`rg` empty) |
| MCP `selection_status` | N2 report | `selection_report_json` (`V/app/mcp/commands/selection.rs:410-412`) |
| MCP `project_summary` | nothing of N1, N3 | F-7 |
| MCP `get_notifications` | the toast stack | a selection edit toasts only a refusal or the Q2 floor note (`V/controller/events/mod.rs:126-140`) |

**Notification ruling (recommended).** A selection change does not toast
and does not notify an agent. UX_REVIEW §9 Q5 recommends (B): "a failed or
stale state shows its word on the row; no toast for a state". A selection
is a state. An agent reads state through a read door (section 6), not
through the toast stack.

## 4. One selection contract

### 4.1 View state against project state

| State | Owner | The one read door | The one write door |
|---|---|---|---|
| Op selection (rules, picks, excludes; Op and Boundary) | core `ProjectSession` | `session.selection_status(tc, target)` → `Arc<SelectionReport>` (the GUI panel through `Query::SelectionStatus`) | `Command::EditSelection` (and the L-8 `SetBoundaryConfig`) |
| Tree focus | `AppState` | `AppState::focus()` (new) | `AppController::set_focus()` (new) = `handle_select` |
| Highlighted toolpath | derived | `selection_highlight::highlight_toolpath` | none (derived from focus and sim focus) |
| Pick mode, row hover, list filter | `GuiState.selection_ui` | `SelectionUi` getters | `SelectionUi` setters (view only) |
| Hover | `RsCamApp` | `HoverTarget` (new, one slot) | the viewport frame |
| Camera | A's `state/camera` module | A's pose reader | one `RsCamApp::apply_camera` (4.4) |

The rule for every sibling: a gesture that changes what an op cuts ends in
`Command::EditSelection`. A gesture that changes what the user looks at
writes view state through one named door. No third notion of "selected"
enters the code.

### 4.2 Retire `Selection::Face` and `Selection::Faces`

Delete both variants and every arm in F-1. Delete `draw_face_selection`
and `draw_faces_selection` (`V/ui/properties/mod.rs:1557-1616`) and the two
W10 tests that build the variant (`workflow_tests.rs:1279-1325`). Keep the
model-removal arm for `Selection::Model`. Q7 (`toolpath.rs:63-81`) then
takes faces from the op selection only. `selected_face_ids` becomes:

```rust
// V/app/gpu_upload.rs — one door with the drawing highlight (F-4).
fn selected_face_ids(&self) -> Vec<FaceGroupId> {
    crate::state::selection_highlight::highlight_report(self.controller.state())
        .map(|(_, _, report)| report.selected.iter()
            .filter(|k| k.kind == ItemKind::Face)
            .filter_map(|k| k.brep)
            .map(FaceGroupId)
            .collect())
        .unwrap_or_default()
}
```

### 4.3 One door for the tree focus (compiler-enforced)

The operator prefers a compiler rule to a sentry (memory
"type-system-enforced patterns"). Make the field private:

```rust
// V/state/mod.rs
pub struct AppState {
    selection: Selection,          // was `pub`
    /* … */
}
impl AppState {
    pub fn focus(&self) -> &Selection { &self.selection }
    /// The view door. `pending` reports a change for the upload rule.
    pub(crate) fn set_focus(&mut self, next: Selection) -> FocusChange {
        let old_setup = self.setup_of_focus();
        let changed = self.selection != next;
        self.selection = next;
        FocusChange { changed, setup_changed: old_setup != self.setup_of_focus() }
    }
}
#[must_use]
pub struct FocusChange { pub changed: bool, pub setup_changed: bool }

// V/controller/events/model.rs — handle_select becomes the one caller
// that turns FocusChange into `pending_upload`.
pub(crate) fn set_focus(&mut self, next: Selection) {
    if self.state.set_focus(next).changed { self.pending_upload = true; }
}

impl Selection {
    /// The one wire token: the MCP echo, `get_ui_view` and the status bar.
    pub fn wire_kind(&self) -> &'static str { match self {
        Selection::None => "none", Selection::Stock => "stock",
        Selection::PostProcessor => "post", Selection::Machine => "machine",
        Selection::Model(_) => "model", Selection::Tool(_) => "tool",
        Selection::Setup(_) => "setup", Selection::Obstacle(..) => "obstacle",
        Selection::Toolpath(_) => "toolpath",
    } }
}
```

`SplitMut.selection` (`V/state/mod.rs:237`) becomes `&Selection`. Every
direct write in N1 moves to `AppController::set_focus`. The compiler finds
each one.

### 4.4 The viewport facade: a pure function that returns intents

B's input state machine must not write state. The repo already has the
shape: `pick_bar::click_event(..) -> AppEvent` (`V/ui/pick_bar.rs:54-83`).
`RsCamApp` cannot be built in a test (`mcp_core_arm_describes_every_row.rs:12-17`),
so a pure function is the only form that a test can drive.

```rust
// V/interaction/intent.rs (new). Pure: no &mut, no egui context.
pub enum ViewportGesture {
    Click { hit: Option<PickHit>, mods: Mods, button: Button },
    BoxRelease { keys: Vec<MatchKey>, mods: Mods },   // B supplies the keys
    Key { key: ViewKey, mods: Mods },                  // Ctrl+A, F, Escape, 1-4
}
pub struct ViewContext<'a> {
    pub workspace: Workspace,
    pub focus: &'a Selection,
    pub selection_ui: &'a SelectionUi,
    pub session: &'a ProjectSession,
}
/// The one translation from a gesture to the events the controller runs.
pub fn gesture_events(g: &ViewportGesture, cx: &ViewContext<'_>) -> Vec<AppEvent>;
```

The output types are the existing doors:

- A tree hit gives `AppEvent::Ui(UiCommand::Select(..))`. It never writes
  `state.selection`.
- An item or face hit in Toolpaths gives `TogglePick`, `AddPick` or
  `PickLayer` (as today).
- A box release gives one `AppEvent::EditToolpathSelection { edit:
  SelectionEdit::AddPicks(keys) }` (E's variant).
- A camera key gives `UiCommand::SetViewPreset`, `ToggleProjection`,
  `ResetView`, or A's new rows.
- A pick-mode key (Escape) gives a new `UiCommand::EndViewportMode`
  (`gui: Reached`, `mcp: Skip("the agent names keys; it has no pick mode")`,
  `cli: Skip`). This retires the direct write at `V/app/input.rs:432`.

MCP calls the same doors, not `gesture_events`: it has no pointer. Parity
is at the event and command level.

### 4.5 One hover slot

```rust
// V/state/selection_highlight.rs — replaces N4/N5/N6 as the upload input.
#[derive(Debug, Clone, PartialEq)]
pub enum HoverTarget {
    Item(MatchKey),                  // a drawing item, a STEP edge, an STL face key
    Face(ModelId, FaceGroupId),      // a STEP face outside pick mode
}
pub struct HoverDetector { last: Option<HoverTarget> }
```

`row_hover` stays as the Geometry tab's input. The viewport resolves pick
hover, then `row_hover`, then the face hover into one `HoverTarget` per
frame. D's state buffer reads `HoverDetector::hovered()` only.

## 5. Parity rules for the new actions

| Action | GUI door | MCP door (tool, param) | CLI door | Undo | Notification | Sentry |
|---|---|---|---|---|---|---|
| Box select (B) | `gesture_events(BoxRelease)` → `EditSelection(AddPicks)`; Ctrl → `RemovePicks` | `edit_toolpath_selection {add_picks: [key…]}`, `{remove_picks: […]}` | `[[operation.selection.picks]]` (exists) | one act (Q-2) | none | `mcp_and_gui_give_one_box_select_v9` |
| Select similar (E) | context menu → core builds the rule → `EditSelection(AddRule)` | `edit_toolpath_selection {add_rule: <rule>}` | `[[operation.selection.rules]]` | one act | none | `mcp_and_gui_give_one_similar_rule_v11` |
| Ctrl+A in pick mode (E) | `AddRule(Rule { source: ToolpathModel, filter: Filter { kinds: Only(effective_kinds) } })` | the same `add_rule` | the same rule in TOML | one act | none | arm of the `_v11` test |
| Clear | Geometry tab "Clear" → `Replace(Selection::empty())` | `{replace: {}}` (exists) | `selection = {}` | one act | none | arm of the `_v9` test |
| Camera preset, projection (A) | `UiCommand::SetViewPreset`, `ToggleProjection` → `RsCamApp::apply_camera` | `set_ui_view {camera: {preset, projection}}` → the same method | not applicable: the CLI draws no view | none | none | `one_view_proj_by_key_and_by_mcp_v4` |
| Fit (A) | key F, dock → `apply_camera(Fit(..))` | `set_ui_view {camera: {fit: "model" \| "selection"}}` | not applicable | none | none | same test, fit arm |
| Camera pose (A) | orbit, pan, zoom | `set_ui_view {camera: {yaw, pitch, distance, target}}`; the reply echoes the pose | not applicable | none | none | same test, pose arm |
| Frame items | exists | exists | not applicable | none | none | existing |
| Hover (B, D) | `HoverTarget` | not applicable: no pointer | not applicable | none | none | D's zero-rebuild stats test |
| Read the view focus | screen | `get_ui_view` (new `UiQuery`): focus kind and id, workspace, pick mode, camera pose | not applicable | — | — | census + `_v4` reads it |
| Boundary selection | Geometry tab boundary rows | `target: "boundary"` (exists) | `[operation.boundary_selection]` (Q-3) | one act | none | CLI twin of `a_job_layer_rule…_p2_7` |

**Rulings in this table.**

- The CLI needs no camera, hover or pick-mode door. These are view state.
  The CLI renders no view, and `command_surface_completeness` property 2
  already forces `cli: Skip` on every view row.
- The CLI needs no new key for batch picks or rules. The job key
  `selection` is a whole `Selection`, and `picks` is a list:

```toml
[[operation]]
type = "pocket"
input = "part.dxf"
tool = "flat_3mm"

[[operation.selection.rules]]
source = "toolpath_model"
filter = { layers = ["CUT"], kinds = { only = ["circle"] } }

[[operation.selection.picks]]   # one table per item; a box gives N tables
source = "toolpath_model"
kind = "loop"
layer = "OUTLINE"
anchor = [12.5, 40.0]
measure = 182.3

[[operation.selection.picks]]
source = "toolpath_model"
kind = "face"
anchor = [30.0, 20.0]
measure = 600.0
brep = 7
z = -6.0
```

  The fields come from `MatchKey` (`C/selection/key.rs:42-63`). The
  `kinds` spelling follows `KindFilter`'s serde form; E must confirm it in
  its round-trip test.
- MCP camera fields go on `set_ui_view`, not a new tool. That is the
  convention of `Select`, `SwitchWorkspace` and `SetModelVisibility`
  (`V/ui_command.rs:384-433`). The view rows keep
  `mcp: Skip("set_ui_view carries camera instead")`, so property 4 of the
  census still holds.
- `apply_camera` is one `RsCamApp` method with two callers, as
  `frame_selection_items` is (`V/app.rs:454`; callers `V/app/input.rs:225`,
  `V/app/mcp/view.rs:656`). MCP applies it synchronously after
  `toolpath_index`, for the reason in `view.rs:455-463`. MCP writes the
  pose at once with no tween (REPORT D.1 step 6).

```rust
// V/app.rs — the one camera door (A owns the body).
pub(crate) enum CameraRequest {
    Preset(ViewPreset),
    Projection(ProjectionMode),
    ToggleProjection,
    Fit(FitTarget),               // Model | Selection | Bounds(BoundingBox3)
    Pose(CameraPose),             // yaw, pitch, distance, target
}
pub(crate) fn apply_camera(&mut self, req: CameraRequest, animate: bool) -> CameraPose;
```

## 6. The "what is selected" read API

**Verdict: `SelectionReport` already is the shared summary.** It holds
`counts` by kind, `faces_by_facing`, `foreign`, `selected`, `item_states`,
`refusal` and `lost` (`C/session/selection_status.rs:267-340`). Every
reader takes it from the one memo: the Geometry tab (through the query
row), the drawing highlight, the face ids, MCP `selection_status`, and the
CLI test. Do not add a `SelectionSummary` type. Two gaps stay:

1. **The view focus has no summary.** Add one view type and one `UiQuery`
   row. The status bar, the `set_ui_view` echo and `get_ui_view` read it.

```rust
// V/state/selection.rs
#[derive(Debug, Clone, serde::Serialize)]
pub struct FocusSummary {
    pub kind: &'static str,                 // Selection::wire_kind
    pub id: Option<String>,                 // the item id, when one exists
    pub toolpath_index: Option<usize>,
    pub workspace: &'static str,            // workspace_key
    pub pick: Option<PickSummary>,          // toolpath_index, target, kinds on
    pub highlighted_toolpath: Option<usize>,// highlight_toolpath (L-9)
}
pub fn focus_summary(state: &AppState) -> FocusSummary;
```

2. **The one-line count text.** If the Geometry tab builds its count
   phrase ("14 loops, 2 circles") in viz, move that function beside
   `SelectionReport` and let the status bar call it. Do not write a second
   phrase builder. Verify the present site before the move.

## 7. Tests to add

Names follow the repo form (claim, then package tag).

| Test | Kind | Asserts |
|---|---|---|
| `mcp_and_gui_give_one_box_select_v9` | viz lib, `V/app/mcp/commands/selection.rs` | Two copies of one project. GUI: `gesture_events(BoxRelease{keys})` gives one `EditToolpathSelection(AddPicks)`; apply it. MCP: `wire_edit({"add_picks": keys})`. Equal stored `Selection`, equal `selection_status` report. Arms: on the default (O-PICK replaces once), on a rule (picks join), `remove_picks`, `{replace:{}}`. |
| `mcp_and_gui_give_one_similar_rule_v11` | viz lib | Core builds the rule from a picked key. GUI and wire `add_rule` store the same rule. Ctrl+A arm. |
| `a_gesture_gives_events_not_writes_v9` | viz lib | `gesture_events` for a click on a tree hit gives `UiCommand::Select`; on an item gives `TogglePick`; on nothing gives `Select(None)`. A source scan: `V/app/viewport.rs` holds no `.selection =` (non-vacuity anchor: the scan finds `gesture_events(`). |
| `one_view_proj_by_key_and_by_mcp_v4` | viz lib | `apply_camera(Preset(p), animate=false)` from the key path and from the `set_ui_view` param conversion give equal `view_proj` for each preset, both projections, and a fit. The MCP pose echo round-trips. |
| `a_job_with_batch_picks_replays_one_selection_v9` | CLI `L/tests/` | A job TOML with two `picks` and one rule. `project` save, then load: the stored `Selection` equals the job's; `selection_status` equals the MCP twin built with `add_picks`. |
| `the_focus_has_one_door_v0` | compile rule | No test: the private field is the rule. One `#[test]` in `V/state` proves `set_focus` reports `changed` and `setup_changed`. |
| census arms | `command_surface_completeness.rs` | Each new view row (`EndViewportMode`, A's camera rows, `get_ui_view`) is in the census. A new property: every `mcp: Skip` reason that names `set_ui_view` names a field that `SetUiViewParam` has (read the schema off the router). |
| wire | `mcp_wire_surface_pin.rs`, `an_unknown_mcp_field_is_refused_f26` | Re-bless the snapshot once per wire change. |

## 8. Commit sequence and invariants

Each commit lands with its sentry. Agents implement; the lead commits.

1. **F1 Retire `Selection::Face/Faces`** (F-1, F-4). Delete the variants,
   arms, two panels and two tests; `selected_face_ids` reads
   `highlight_report`. Break: none on any wire or file (view state only).
2. **F2 The focus door** (F-3). Private field, `focus()`, `set_focus()`,
   `wire_kind()`. Every direct write moves. Break: none outside viz.
3. **F3 `get_ui_view` and `FocusSummary`** (F-7). New `UiQuery` row and
   MCP tool; `set_ui_view` echo reads `FocusSummary`. Break: the echo key
   `selection` gains the tokens `model`, `setup`, `obstacle`, `post`,
   `none` in place of `"other"`. State it in the commit (ruling 2026-09-16).
4. **F4 The gesture facade** (with B). `interaction/intent.rs`,
   `EndViewportMode` row, `HoverTarget`. Break: none on the wire.
5. **E's `AddPicks`/`RemovePicks`** and the wire twin, with the reverse
   conversion of F-9. Break: the MCP schema of `edit_toolpath_selection`
   grows; re-bless the snapshot.
6. **A's camera door** and the `set_ui_view camera` field. Break: the
   schema of `set_ui_view` grows; re-bless.
7. **F5 CLI boundary selection** (if Q-3 is yes). Break: a new job key.

Invariants kept: every op-selection change goes through
`ProjectSession::apply(Command::EditSelection)`; `SelectionReport` stays
the one summary; core stays free of view types; L-3 (one viewport mode),
L-9 (only the drawn toolpath lights) and O-PICK (in `Selection::apply`,
`C/selection/rule.rs`) hold on every surface.

## 9. Open questions and requirements

### 9.1 Open questions for the lead

- **Q-1. Delete `Selection::Face/Faces` (F-1)?** Recommendation: yes.
  No producer exists. The tree shows the op's faces through the Geometry
  tab.
- **Q-2. One undo step per selection act?** Today the edits coalesce into
  the inspector snapshot (F-8). Recommendation: give `EditSelection` the
  `EditTabs` treatment (`V/controller/events/mod.rs:196-300`): one step
  per act. Then "a box is one undo step" is true by the command, and the
  undo of an MCP edit stops depending on what the inspector showed.
- **Q-3. A CLI job key for the boundary selection (F-10)?**
  Recommendation: yes, `[operation.boundary_selection]` through
  `EditSelection { target: Boundary }`. The registry row already says
  `cli: Reached` for `EditSelection`.
- **Q-4. A toggle edit on the wire?** Recommendation: no. Core says
  "No toggle edit exists here" (`C/selection/rule.rs:366-368`): a toggle
  needs the resolved state. The agent reads `selected` first.
- **Q-5. MCP door for pick mode?** Recommendation: no write door. Report
  it in `get_ui_view` only, so an agent can see what the operator does.
- **Q-6. Notify agents on a selection change?** Recommendation: no
  (section 3, UX_REVIEW §9 Q5 (B)). `get_ui_view` is the read door.

### 9.2 Requirements for the sibling designs (one line each)

- **All:** A gesture that changes what an op cuts ends in `Command::EditSelection`; nothing else writes the op selection.
- **All:** Do not add a fourth "selected" or "hovered" slot; use `AppState::focus()`, `SelectionReport`, `HoverTarget`.
- **All:** A new view row goes in `for_each_ui_command!` with `cli: Skip`, and the census passes.
- **A:** Put camera wire fields on `set_ui_view` as one `camera` object; keep the view rows `mcp: Skip("set_ui_view carries camera instead")`.
- **A:** One `RsCamApp::apply_camera(CameraRequest, animate)` serves the key, the dock and MCP; MCP passes `animate = false`.
- **A:** The `set_ui_view` reply echoes the pose; the wire enums take `#[schemars(inline)]` and `deny_unknown_fields`; add a `build_info().features` token.
- **A:** Pin `one_view_proj_by_key_and_by_mcp_v4`.
- **B:** The input state machine calls a pure `gesture_events(..) -> Vec<AppEvent>`; `V/app/viewport.rs` holds no `.selection =` write.
- **B:** A box release sends one `EditToolpathSelection(AddPicks)`, never N `AddPick` events.
- **B:** A tree hit sends `UiCommand::Select`; Escape sends `UiCommand::EndViewportMode`.
- **B:** The modifier rule and the button scheme appear in the shortcuts window.
- **C:** The BVH is core geometry with no view type; the pick result type stays `PickHit`.
- **C:** One pick per frame feeds both the hover and the tooltip.
- **D:** The face state buffer reads `highlight_report` (L-9) and `HoverDetector::hovered()` only, never `state.selection` directly.
- **D:** The state colours stay `colors.rs` tokens (`the_selection_colours_live_in_colors_p4_4`).
- **E:** Add `AddPicks`/`RemovePicks` to `SelectionEdit` with O-PICK applied once per batch in `Selection::apply`.
- **E:** Give `SelectionEditParam` an exhaustive conversion in both directions (F166, F-9).
- **E:** Build select-similar and Ctrl+A as `AddRule` values from core; add no `SelectAll` variant.
- **E:** A new `Rule` kind is a project-file key and a job key; state the break; add the `mcp_and_gui_give_one_*_v11` test and a CLI round-trip.
