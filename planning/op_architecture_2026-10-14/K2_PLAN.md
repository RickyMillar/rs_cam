# K2 plan: the obstacle panel, the obstacle renderer, the overlay merge, the K1 leftovers

Branch `op-arch`. Read-only design, 2026-10-16. One editor run. All line
numbers are at HEAD `ba705014`.

Cut order if the run is short: D13 (mesh exclusion, items 21 filter) and
D12 (the plane line) are the optional tail. Everything else is the
operator's ask.

## 0. What is there today (the facts the plan builds on)

- Core model: `crates/rs_cam_core/src/compute/obstacle.rs`.
  `ObstacleGeometry::{Box, OutlineExtrude, Model}` (:86-115),
  `Obstacle { id, name, enabled, clearance, geometry }` (:170),
  `Obstacle::new_default_box` (:205), `next_obstacle_id` (:233),
  `validate_obstacle` (:291), `model_geometry_facts` (:325),
  `ObstacleSet::resolve` (:466), `ResolvedObstacle` (:372),
  `ObstacleSet::plane_z` (:577). Core commands `AddObstacle`,
  `ReplaceObstacle` (wire `edit_obstacle`), `RemoveObstacle`
  (`session/command.rs:578-594`, args :1722-1750).
- GUI list: `ui/properties/setup.rs:296-323`. The list has a name row and
  "+ Add Obstacle". REMOVE is only in the right-click context menu
  (:311-317). That is the "no way in gui to remove" defect: the control
  exists, but the operator cannot find it.
- GUI editor: `setup.rs:331-401` (`draw_obstacle_properties`). It edits a
  box only. An outline or a model obstacle shows "Edit it through the MCP
  edit_obstacle tool" (:376). The clearance is a raw `egui::DragValue`.
- GUI apply: `ui/properties/mod.rs:1191-1219` (`draw_obstacle_selection`)
  keeps `history.obstacle_draft` (`state/history.rs:103`) and calls
  `apply_obstacle_draft` (`ui/properties/panel_apply.rs:309-325`) on a
  click commit. A refusal goes to `tracing::warn` only
  (`panel_apply.rs:242-245`), and the draft drops. So a switch to a model
  kind with no valid model snaps back with no message.
- Controller doors: `handle_add_obstacle` (`controller/events/model.rs:535`)
  and `handle_remove_obstacle` (:563). Dispatch at
  `controller/events/mod.rs:209-212`. Events `AppEvent::AddObstacle`,
  `AppEvent::RemoveObstacle` at `ui/mod.rs:155-156`.
- Undo: `UndoAction` (`state/history.rs:19-53`) has no obstacle arm. An
  obstacle edit is not undoable today.
- Renderer: `app/gpu_upload.rs:600-662` draws box obstacles only, through
  `state::job::obstacle_box` / `obstacle_clearance_box` (`state/job.rs:255-275`),
  with literal colours (:652-659). `render/fixture_render.rs` builds only
  wire boxes plus extra lines.
- Picking: `interaction/picking.rs:123-153` picks box obstacles of EVERY
  setup, but the renderer draws only the active setup.
- Overlays: rows `fixtures` (`ui/overlays/registry.rs:607-626`) and the dead
  `keep_outs` (:627-645). `PREFERENCE_OVERLAYS` lists both (:1644-1645).
- MCP: `obstacle_json` (`app/mcp/commands/obstacles.rs:26-36`) writes
  `"kind": "outline extrude"` (a label with a space) AND
  `geometry.kind: "outline_extrude"` (the serde tag). No reader uses the
  top-level key (`rg` found none).
- Pin walls: `ProjectSession::pin_obstacles` (`session/pins.rs:124`) and
  `pin_obstacle_findings` (:209) each call `ObstacleSet::resolve` for every
  setup. The pin panel calls them each frame
  (`ui/properties/mod.rs:936` `pin_set_report_for`, `pin_report.rs:182`).
  `resolve` rasterises a mesh silhouette on each call (`obstacle.rs:515`).

## 1. Decisions (each one has a reason)

| # | Decision | Reason |
|---|---|---|
| D1 | The obstacle editor becomes an Apply / Cancel editor with a refusal banner, as the S4 stock-change editor (`ui/properties/stock_changes.rs:406-496`). The draft moves from `history.obstacle_draft` to `state.panels.obstacle_editor: Option<ObstacleEditor>`. | A kind switch to `OutlineExtrude` or `Model` is not valid until the operator picks a model. A click-commit editor applies an invalid draft and drops it. The `state/CLAUDE.md` rule: a draft with content lives in `state`, not in egui memory. |
| D2 | Keep `Selection::Obstacle`. The editor draws on the `Selection::Obstacle` page (the inspector), as today. It does not draw a second time under the setup list. | Picking, the highlight, `setup_panel.rs:524`, `pin_view.rs:35` and `state/mod.rs:420` read the selection. One editor host only (ui/CLAUDE.md: no duplicate route). |
| D3 | Keep `Command::ReplaceObstacle(` constructed in `ui/properties/panel_apply.rs` (`apply_obstacle_draft`). Keep `AppEvent::AddObstacle(SetupId)` and `AppEvent::RemoveObstacle`. | The sentry `tests/egui_draw_sites_write_through_commands_wp6.rs` (`ADOPTED_ROWS` "edit_obstacle", :334-347) needs that constructor under `src/ui/`. `controller/holder_clearance_staleness_g_holderstale.rs:159,288,298` drives these names. |
| D4 | `apply_obstacle_draft` returns `Result<(), String>` (the core words). The draw site writes the error into `ObstacleEditor.refusal` and keeps the editor open. It also pushes a notification through the existing panel side-effect path, or only the banner (see risk R6). | The S4 refusal contract (`crud.rs` `a_refusal_reaches_the_editor_in_the_core_words_s4`). |
| D5 | Add is one click: `AppEvent::AddObstacle(setup)` adds the default box (`Obstacle::new_default_box`), selects it, and opens the editor on it. The operator then switches the kind and applies. | The box is always valid, so the add never refuses. The kind switch then goes through the refusal path of D1/D4. No second add door. |
| D6 | REMOVE is a visible control in two places: a "Remove" quiet button on each list row, and a "Remove" button in the editor. Both push the existing `AppEvent::RemoveObstacle`. Keep the context-menu entry. | The operator report. One event, so one door. |
| D7 | The enable checkbox moves onto each list row. A new event `AppEvent::SetObstacleEnabled(SetupId, ObstacleId, bool)` makes the controller apply `ReplaceObstacle` with the STORED record and the new flag (the S4 `SetEnabled` shape, `controller/events/model.rs:619-633`). | One-click toggle with no editor. The stored record cannot carry an unapplied draft. |
| D8 | Undo: add `UndoAction::ObstacleChange { setup_id: SetupId, obstacle_id: ObstacleId, old: Option<Obstacle>, new: Option<Obstacle> }`. `None` means "absent". Add = (None, Some), remove = (Some, None), edit and toggle = (Some, Some). | One variant covers the three commands. |
| D9 | Item 4 (pin walls): the resolver calls `crate::maps::geom_cache::cached_silhouette(&mesh)` (`maps/geom_cache.rs:338`) in place of `model_silhouette(&mesh, Some(0.5))`. This is a DEVIATION from the brief's "cache per obstacle set digest". | `cached_silhouette` memoises exactly the default-cell raster by mesh identity, with the sound `Weak` key (`geom_cache.rs` module doc). The mesh identity contains everything the raster reads; a set digest adds nothing to the raster key. The `grow` offsets that remain are small. A set memo (D9b) is a follow-up only if a profile shows `grow` cost. |
| D9b | (Follow-up, NOT in this run.) A bounded set memo in `obstacle.rs` keyed on the enabled `Obstacle` records themselves (`PartialEq`, exact, not a hash) plus a `Weak` per model `mesh` / `polygons` `Arc`, `ptr_eq` on lookup. | Only if a profile needs it. |
| D10 | The renderer resolves ONE obstacle at a time (`ObstacleSet::resolve(std::slice::from_ref(o), models)`), not the set. | `resolve` is all-or-nothing on `Err`. One broken obstacle must not blank the others. The error text goes to the list row. |
| D11 | Overlay merge: delete the `keep_outs` row. Rename the `fixtures` row to id `obstacles`, label "Obstacles", flag `viewport.show_obstacles`. | The K1 ruling: one concept. Operator ruling 2026-09-16: no legacy, breaking OK. The touch list is the same as a delete-only merge. A saved `[display.overlays] fixtures = …` preference is then ignored (the map keeps unknown ids, `rs_cam_core/src/settings/mod.rs:236-240`). State the break in the commit. |
| D12 | The obstacle plane line: draw it when the active setup has an enabled obstacle AND the selection is `Selection::Setup(active)` or `Selection::Obstacle(active, _)`, AND the `obstacles` flag is on. No new overlay row. | The brief marks it optional. Selection-scoped drawing adds no registry row and no flag. |
| D13 | A model obstacle mesh: exclude from the part-mesh upload every model that an obstacle of the ACTIVE setup reads as `ObstacleGeometry::Model`, when a setup is active. The obstacle line prism stands in for it. | `gpu_upload.rs:214-290` draws every model mesh through `transform_mesh(setup)` (the part frame). An obstacle is table-fixed (obstacle.rs module doc), so on a flipped setup the mesh draws in the wrong place. This is a decision row for the lead: the alternative is "accept and document". Recommendation: exclude. |

## 2. Files to change

### Core (`crates/rs_cam_core`)

1. `src/compute/obstacle.rs`
   - :364-367 `SILHOUETTE_CELL_MM`: make it
     `crate::geometry::boundary::SILHOUETTE_CELL_SIZE` (make that const
     `pub(crate)` in `src/geometry/boundary.rs:677`). The two values then
     cannot drift, and `cached_silhouette` (default cell) stays equal to
     the old call.
   - :509-526 the `Model` arm: call
     `crate::maps::geom_cache::cached_silhouette(&mesh)` (the `mesh` is an
     `Arc<TriangleMesh>` from `LoadedModel.mesh`, `session/mod.rs:373`),
     then `grow(&silhouette, SILHOUETTE_CELL_MM, …)` as today.
   - `ResolvedObstacle` (:372): add three display fields:
     `outline: Vec<Polygon2>` (the section before the clearance grow: the
     box rectangle, the outline union, or the silhouette after its
     one-cell grow), `outline_bottom_z: f64`, `outline_top_z: f64`. Fill
     them in `resolve` (:530-536) and in the test literal (the only other
     literal, `rg "ResolvedObstacle \{"` = 2 sites, both in this file).
   - Tests (in the file's test module): see §5 T1.
2. `src/geometry/boundary.rs:677`: `const` → `pub(crate) const`.
3. `src/compute/CLAUDE.md`: one line under the K1 bullet: "A mesh obstacle
   reads `geom_cache::cached_silhouette`; the cell is
   `boundary::SILHOUETTE_CELL_SIZE`." (Keep the file at 40 lines or
   fewer.)

### Viz state

4. `src/state/panels.rs`: add `ObstacleEditor { setup_id: SetupId,
   obstacle_id: ObstacleId, draft: Obstacle, refusal: Option<String> }`
   beside `StockChangeEditor` (:91), and the field
   `obstacle_editor: Option<ObstacleEditor>` beside :118.
5. `src/state/history.rs`: delete `obstacle_draft` (:101-103, :127). Add
   the `ObstacleChange` arm (D8) to `UndoAction` (:19).
6. `src/state/viewport.rs:75-80, :230-231`: rename `show_fixtures` →
   `show_obstacles`; delete `show_keep_outs`.
7. `src/state/job.rs:255-275`: delete `obstacle_box` and
   `obstacle_clearance_box`. Add one helper
   `obstacle_display_extent(o: &Obstacle, models: &[LoadedModel]) -> Option<BoundingBox3>`:
   the grown footprint bbox from a one-obstacle `resolve`, Z from grown
   `bottom_z`..`top_z`. `None` for a disabled or an unresolvable obstacle.
   Picking (item 14) and the plane extent read it.

### Viz UI

8. NEW `src/ui/properties/obstacles.rs` (the S4 file is the model; keep it
   under about 400 lines):
   - `pub fn draw_list(ui, inputs: &ObstacleListInputs, events)`: one row
     per obstacle: the enable checkbox (pushes
     `AppEvent::SetObstacleEnabled`), the name as a `selectable_label`
     (click → `UiCommand::Select(Selection::Obstacle(..))`), a muted second
     line `row_summary` ("box · clearance 3.0 mm", "outline extrude ·
     2 outlines · Z −20.0 to 0.0", "model · clamp.stl"), a quiet
     "Remove" button, and the context menu "Delete" kept. When the row has
     a resolve error (D10), a `Banner::new(Role::Danger, text)` under the
     row. Then "+ Add Obstacle" (pushes `AppEvent::AddObstacle`).
   - `pub fn draw_editor(ui, inputs: &ObstacleEditorInputs, editor: &mut ObstacleEditor) -> EditorAction`
     with `EditorAction::{None, Apply, Cancel, Remove}`: name, a
     `ChoiceRow` for the kind (Box / Extrude / Model), the model
     pick (item 9 helpers), the Z rows (`ValueRow`, `FRAME_HOVER` text:
     "setup frame, fixed to the table"), the box corners (move the grid at
     `setup.rs:351-371` here), the clearance as a `ValueRow` (0..=100 mm),
     the refusal `Banner`, then Apply (`Button::primary`), Cancel
     (`Button::quiet`), Remove (`Button::quiet`, `Role::Danger` text).
     Clear `refusal` when the draft changes (the S4 rule, :470-472).
     The editor has NO enabled checkbox: the list-row toggle (D7) is the
     one control for that field. `apply_obstacle_draft` writes the draft
     with the STORED `enabled` value, so an open editor cannot undo a row
     toggle.
   - `pub enum ObstacleKind { Box, OutlineExtrude, Model }` with `of`.
   - `pub fn set_obstacle_kind(geometry, kind, models, stock_z)`: the
     `ObstacleGeometry` form of S4's `set_geometry_kind` (:205-245). Box →
     Extrude keeps the box Z range; Extrude → Box takes the default spans
     (`DEFAULT_OBSTACLE_BOX_MM`) at the old `z_bottom`; any → Model takes
     the first mesh model; any → Extrude keeps the outline ids that have a
     closed outline.
   - `#[cfg(test)] mod tests`: §5 T4.
9. `src/ui/properties/stock_changes.rs:499-557`: split the private
   `draw_model_pick` into two `pub(crate)` helpers that take the plain
   fields and an egui id salt:
   `draw_mesh_pick(ui, salt, &mut ModelId, models)` and
   `draw_outline_pick(ui, &mut Vec<ModelId>, models)`.
   `draw_model_pick` calls them with salt `"stock_change_mesh"`; the
   obstacle editor calls them with `"obstacle_mesh"`. The salt prevents an
   egui id clash when both editors are on screen.
10. `src/ui/properties/mod.rs`
    - :1160-1175 the `ModelChoice` builder: extract to
      `fn model_choices(session) -> Vec<stock_changes::ModelChoice>` and
      call it from the stock-change section and the obstacle editor.
    - :1191-1219 `draw_obstacle_selection`: open
      `state.panels.obstacle_editor` on the selected obstacle when it is
      not already open for that id (seed `draft` from the stored record).
      Draw `obstacles::draw_editor`. On `Apply` call
      `apply_obstacle_draft(state, setup_index, &stored, draft)`; on `Ok`
      close the editor; on `Err(text)` write `editor.refusal`. On `Cancel`
      reset the draft to the stored record. On `Remove` push
      `AppEvent::RemoveObstacle`.
    - The setup page (around :1138): call `obstacles::draw_list` in place
      of the old list in `setup.rs`.
    - :23 re-exports: keep `apply_obstacle_draft`.
11. `src/ui/properties/setup.rs:296-401`: delete the old list and
    `draw_obstacle_properties`. Remove the `Obstacle, ObstacleGeometry`
    imports (:8). Fix the doc line :18 ("fixtures list, keep-out list").
12. `src/ui/properties/panel_apply.rs:303-325` `apply_obstacle_draft`:
    new signature
    `(state, setup_index, stored: &Obstacle, obstacle: Obstacle) -> Result<(), String>`.
    Return `Ok(())` and apply nothing when `obstacle == *stored` (the
    `PartialEq` is there for this, obstacle.rs:167-168). Call
    `state.session.apply(Command::ReplaceObstacle(..))` directly (not
    `apply_panel_command`, which drops the error), with the same
    `stamp_stale` + `panel_side_effects.invalidate_simulation` mirror as
    `apply_panel_command` (:234-241). On `Ok`: `mark_edited`,
    `panel_side_effects.upload = true`, push
    `UndoAction::ObstacleChange { old: Some(stored), new: Some(obstacle) }`
    (the `ToolChange` push at :49 is the model). On `Err`:
    `Err(error.to_string())`.
13. `src/ui/properties/CLAUDE.md`: add `obstacles.rs` (K2) to the file map
    line (`setup.rs`, `stock_changes.rs` (S4), …).
14. `src/ui/mod.rs:154-156`: add
    `SetObstacleEnabled(SetupId, ObstacleId, bool)`.

### Viz controller

15. `src/controller/events/model.rs`
    - `handle_add_obstacle` (:535-560): take the `Effects` (no
      `let _ =`), and only on `Some` set the selection, open
      `state.panels.obstacle_editor` on the new record, push
      `ObstacleChange { old: None, new: Some(record) }`, `pending_upload`,
      `mark_edited`. Today it marks the project edited on a refusal too.
    - `handle_remove_obstacle` (:563-586): read the stored record first;
      on success push `ObstacleChange { old: Some(record), new: None }`,
      close an editor open on that id, keep the selection fallback.
    - NEW `handle_set_obstacle_enabled(setup_id, obstacle_id, enabled)`:
      stored record + new flag → `ReplaceObstacle` via `apply_quietly`;
      no-op when the flag equals the stored flag; push the undo entry;
      `pending_upload`; `mark_edited`.
    - NEW `pub(crate) fn apply_obstacle_snapshot(setup_id, obstacle_id, target: Option<Obstacle>)`:
      look up the present record by id; (Some, None) → `RemoveObstacle`,
      (None, Some) → `AddObstacle`, (Some, Some) → `ReplaceObstacle`,
      (None, None) → nothing. All through `apply_quietly`. Then
      `pending_upload`; fix the selection when the record went away;
      close a stale editor.
16. `src/controller/events/mod.rs:209-212`: dispatch
    `SetObstacleEnabled`.
17. `src/controller/events/undo.rs`: the `ObstacleChange` arm in `undo`
    (target = `old`) and `redo` (target = `new`), both through
    `apply_obstacle_snapshot`. `mark_edited` already runs after the match
    (:67, :113).

### Viz render and upload

18. `src/render/colors.rs`: `OBSTACLE`, `OBSTACLE_SELECTED`,
    `OBSTACLE_OUTLINE_SELECTED` (the old inner box white),
    `OBSTACLE_CLEARANCE` (a dim, desaturated form of `OBSTACLE`: the line
    pipeline has no alpha, so "faint" is a darker colour), `OBSTACLE_PLANE`.
    Move the literals at `gpu_upload.rs:652-659` here (render/CLAUDE.md:
    a colour belongs in `colors.rs`).
19. `src/render/fixture_render.rs`: add CPU builders (no GPU, testable):
    - `pub fn push_prism_lines(out, ring: &[P2], z0, z1, rgb, shift)`:
      the bottom ring, the top ring, and vertical edges at the CORNERS only
      (a turn of more than 30 degrees, at most 64 per ring). A silhouette
      ring from a 0.5 mm raster has hundreds of vertices; a vertical at
      each one is noise and buffer bulk.
    - `pub fn push_ring_lines(out, ring, z, rgb, shift)` (the clearance
      ring and the plane rectangle use it).
    - Keep `from_boxes_and_lines`, but route the buffer through
      `gpu_safety::try_create_buffer` (render/CLAUDE.md: check a buffer
      against the device limit).
20. NEW `src/app/obstacle_marks.rs` (sibling of `app/pin_marks.rs`, add
    `mod obstacle_marks;` at `app.rs:8`):
    `pub(super) fn push_obstacle_lines(state: &AppState, setup: &SetupFrame, sd: &SetupData, out: &mut Vec<LineVertex>)`.
    For each enabled obstacle, when `state.viewport.show_obstacles`:
    resolve it alone (D10); draw `outline` as a prism from
    `outline_bottom_z` to `outline_top_z` (`OBSTACLE` or
    `OBSTACLE_SELECTED`); draw the grown `footprint` as a ring at the
    grown `top_z` (`OBSTACLE_CLEARANCE`); and when D12 holds, the plane
    rectangle at `ObstacleSet::plane_z()` of the setup's enabled set,
    over the union of the stock display XY and every grown footprint, plus
    10 mm. Every vertex takes `setup.emission_to_display_shift(stock)`, as
    the box path does today (`gpu_upload.rs:637-641`).
21. `src/app/gpu_upload.rs:600-662`: replace the box loop with
    `obstacle_marks::push_obstacle_lines` into the line vertex list (pass
    an empty box list). Gate the rebuild with an
    `upload_cache::refresh_key` key: (active setup id, face, rotation, the
    stock, `Vec<(ObstacleId, Obstacle::effect_digest, enabled)>`, the
    `ArcId` of each model mesh / polygons the obstacles read, the
    selection, `show_obstacles`). The line buffer also carries pins and the
    datum, so the key gates only the obstacle vertex rebuild; keep the
    obstacle vertices in a cached `Vec<LineVertex>` on `resources` and
    append them each upload.
    D13: filter the models in
    `active setup obstacles → ObstacleGeometry::Model { model_id }` out of
    `plain_mesh_ids` / `enriched_ids` (:214-221) AND out of the two upload
    loops (:248-292), so the mesh key changes with the filter.
22. `src/app/viewport.rs:494-500`: `show_fixtures: state.viewport.show_obstacles || …`;
    drop the `show_keep_outs` term. Keep `render::mod`'s
    `show_fixtures` field name (it is the line-buffer gate for five
    kinds), or rename it `show_line_marks` (cosmetic; the editor decides).
23. `src/app.rs:1396, :1432-1433` (`OverlayUploadKey`): rename `fixtures`
    → `obstacles`, delete `keep_outs`. ADD
    `selected_obstacle: Option<(SetupId, ObstacleId)>` (derived from
    `state.selection`). The highlight and the plane (D12) follow the
    selection, and `handle_pick_hit` (`app/viewport.rs:166`) sets
    `Selection::Obstacle` with no `set_pending_upload`. Without the key
    field, a pick of a second obstacle in the same setup does not
    re-upload. That is also a K1 highlight lag today.
24. `src/interaction/picking.rs:123-153`: pick obstacles of the ACTIVE
    setup only (the renderer draws only those). Use the renderer's own
    predicate, `state::pin_view::active_display_setup(&session, &selection)`
    (`state/pin_view.rs:29`), so the pick set and the draw set are one
    answer; with
    `state::job::obstacle_display_extent` shifted by the display shift.

### Overlays

25. `src/ui/overlays/registry.rs`: delete the `keep_outs` row
    (:627-645). Row :607-626 → id `obstacles`, label "Obstacles", flag
    `viewport.show_obstacles`, `get`/`set` on `show_obstacles`, hover
    "Obstacles of this setup: fixtures and keep-out zones (one concept).
    The solid line is the geometry, the dim line is the clearance, the
    flat rectangle is the obstacle plane." `PREFERENCE_OVERLAYS`
    (:1644-1645): `"obstacles"`, delete `"keep_outs"`.
26. `src/mcp_server.rs:1749` and `crates/rs_cam_mcp/src/server.rs:381`:
    the overlay id lists: `fixtures, keep_outs` → `obstacles`.
27. `tests/snapshots/mcp_wire_surface.json`: re-bless with
    `RS_CAM_UPDATE_WIRE_SNAPSHOT=1 cargo test -p rs_cam_viz -q --test mcp_wire_surface_pin`,
    then run it again without the variable.

### MCP leftover

28. `src/app/mcp/commands/obstacles.rs:32`: delete the top-level `"kind"`
    key. `geometry.kind` (snake_case serde tag) is the one spelling, and it
    equals the `ObstacleGeometryKindParam` input spelling
    (`rs_cam_mcp/src/server.rs:1434-1446`). Check the three tool
    descriptions; none names `obstacles[].kind` today.
29. `src/app/mcp/commands/obstacles.rs` `describe_obstacle` (around
    :335): on "Removed", close `state.panels.obstacle_editor` when it is
    open on the removed id, and move a `Selection::Obstacle` of that id to
    `Selection::Setup` (the S4 rule at `stock_changes.rs` MCP :463-474).

### Sentries that change in the same run

30. `src/controller/holder_clearance_staleness_g_holderstale.rs:269-296`:
    the call to `apply_obstacle_draft` takes the new signature
    (`&stored`, returns `Result`; `.expect` the `Ok`). :159, :298 keep
    `AppEvent::AddObstacle`.
31. `tests/egui_draw_sites_write_through_commands_wp6.rs`: no change
    needed (D3). Run it.

## 3. The command flow

```
List row checkbox ─► AppEvent::SetObstacleEnabled ─► controller
    handle_set_obstacle_enabled ─► apply_quietly(ReplaceObstacle{stored+flag})
    ─► stamp_stale + invalidate_simulation ─► undo push, pending_upload, mark_edited

"+ Add Obstacle" ─► AppEvent::AddObstacle ─► handle_add_obstacle
    ─► apply_quietly(AddObstacle{new_default_box}) ─► on Some: select,
    open ObstacleEditor, undo push (None→Some), pending_upload, mark_edited

Editor Apply (Selection::Obstacle page) ─► panel_apply::apply_obstacle_draft
    ─► session.apply(ReplaceObstacle) ─► Ok: stamp_stale, sim mirror,
    upload, undo push (Some→Some), close editor
                                       └► Err: editor.refusal = core text

Row "Remove" / editor "Remove" / context "Delete" ─► AppEvent::RemoveObstacle
    ─► handle_remove_obstacle ─► apply_quietly(RemoveObstacle)
    ─► undo push (Some→None), close editor, selection → Setup

Undo / Redo ─► UndoAction::ObstacleChange ─► apply_obstacle_snapshot
    ─► Add / Replace / Remove through apply_quietly; mark_edited after match

MCP add/edit/remove_obstacle ─► CoreRequest ─► the same three Commands
    ─► describe_obstacle (closes a stale editor, fixes the selection)
```

Every write is `ProjectSession::apply(Command)`. The view mirrors
`Effects.stale` and `Effects.simulation_cleared` at each door (WP19).

## 4. Reuse points

| What | Where |
|---|---|
| Editor shape, Apply/Cancel, refusal banner | `ui/properties/stock_changes.rs:406-496` |
| Kind switch with carry-over | `stock_changes.rs:205-245` `set_geometry_kind` |
| Mesh pick, outline pick | `stock_changes.rs:499-557` `draw_model_pick` (split, item 9) |
| `ModelChoice`, facts builder | `stock_changes.rs:83-92`; `ui/properties/mod.rs:1160-1175`; core `stock_change_model_facts` (`session/mutation/stock_change.rs:48`) |
| Stock Z in the setup frame | `ui/properties/mod.rs:1176-1179` (`SetupEvalContext::build_for_setup(..).heights_stock_bbox`) |
| Components | `ChoiceRow`, `ValueRow`, `Banner`, `Button`, `SectionHeader` (`ui/components/`) |
| Toggle with stored record | `controller/events/model.rs:619-633` (S4 `SetEnabled`) |
| Controller door with the mirror | `controller/events/mod.rs:100-111` `apply_quietly` |
| Panel door with the mirror | `ui/properties/panel_apply.rs:230-247` |
| Undo push from a panel | `panel_apply.rs:49` (`ToolChange`) |
| CPU line builder in `app/` | `app/pin_marks.rs:27` `push_pin_lines` |
| Display shift | `state/job.rs:119` `emission_to_display_shift` |
| Upload key guard | `render/upload_cache.rs` `refresh_key`, `ArcId` |
| Silhouette memo | `rs_cam_core/src/maps/geom_cache.rs:338` `cached_silhouette` |
| Plane height | `rs_cam_core/src/compute/obstacle.rs:577` `ObstacleSet::plane_z` |
| Test with a 2D outline model | `controller/tests/stock_frame.rs:217` |
| Panel controller tests to copy | `controller/tests/crud.rs:597-800` `stock_change_panel_s4` |

## 5. Tests

- T1 core, `compute/obstacle.rs` test module:
  - `a_resolved_obstacle_keeps_its_outline_before_the_clearance_k2`: a box
    with clearance 3 gives `outline` = the box rectangle, Z = the box Z,
    and a `footprint` that contains the outline and is larger.
  - `a_mesh_obstacle_resolves_to_the_same_outline_twice_k2`: two
    resolves of one mesh obstacle give equal `outline` and `footprint`.
- T1b core, `tests/geometry_cache_g8.rs` (already
  `required-features = ["test-support"]`, Cargo.toml:209-210). The file
  serialises its counter tests with `counter_lock()` (:38-42) and resets
  the counters with `geom_cache::reset_stats()` (:340). Use that idiom:
  new case `a_mesh_obstacle_rasterises_its_silhouette_once_k2`. Take
  `counter_lock()`, call `reset_stats()`, build a FRESH mesh `Arc`,
  resolve a one-obstacle set (Model kind on that mesh) three times, and
  assert `stats().silhouette_builds == 1` and
  `stats().silhouette_hits == 2`. Before the change both stay 0 (the
  resolver did not call the cache), so the case is red on HEAD. Run with
  `cargo test -p rs_cam_core -q --features test-support --test geometry_cache_g8`.
  (The shared const in item 1 makes a separate cell-drift test
  unnecessary.)
- T2 viz controller, `controller/tests/crud.rs` new
  `mod obstacle_panel_k2` (copy `stock_change_panel_s4`):
  - `every_obstacle_action_is_one_command_k2`: AddObstacle → one box,
    selected, editor open; `apply_obstacle_draft` with an extrude on a
    polygon model (add the model by the `stock_frame.rs:217` pattern) →
    stored geometry is the extrude; SetObstacleEnabled(false) → stored
    flag false, rest unchanged; a Model kind on `models()[0]` → stored;
    RemoveObstacle → list empty, editor closed, selection
    `Selection::Setup`. Each step: `gui.dirty`.
  - `a_refusal_reaches_the_obstacle_editor_in_the_core_words_k2`: a Model
    kind on the polygon-only model → `Err` that contains "a model obstacle
    needs a mesh" (core text `compute/stock_change.rs:261` with the noun
    swap at `obstacle.rs:266-270`), the stored record is
    unchanged, `dirty` is false. Non-vacuity: an `EmptyBox` draft (min =
    max on x) → "min below max on the x axis".
  - `an_unchanged_draft_applies_nothing_k2`: `apply_obstacle_draft` with
    the stored record → `Ok`, no stale stamp, no undo entry.
- T3 viz undo, `controller/tests/undo.rs`:
  - Add an `"ObstacleChange"` row to the arms `Vec` of
    `every_undo_arm_marks_the_project_edited_g_undofresh` (:20-76):
    `old: Some(Obstacle::new_default_box(ObstacleId(0))), new: None` on
    setup 0. Do NOT use `old: None`: the fixture holds no obstacle 0, so
    an undo to "absent" applies nothing, the simulation stays, and the
    row's "cleared or stale" assert goes red. With `old: Some`, the undo
    ADDS an enabled obstacle and the redo removes it; both mutate.
  - `an_obstacle_add_edit_remove_each_undo_and_redo_k2`: add → undo →
    absent → redo → present (same record); edit clearance 3→5 → undo → 3
    → redo → 5; remove → undo → present with the same record (it is now
    LAST in the list; assert by id, not by index) → redo → absent. After
    each undo, the setup's toolpaths read stale (`stale_since` set).
- T4 viz UI unit, `ui/properties/obstacles.rs` tests:
  `set_obstacle_kind` carry-over (box Z → extrude Z; extrude → model picks
  the first mesh; model → extrude keeps no non-outline id) and
  `row_summary` text for each kind.
- T5 renderer, `render/fixture_render.rs` tests (CPU only; no stock-change
  renderer test exists to copy):
  - `push_prism_lines` on a 4-corner ring gives 2×4 ring segments + 4
    verticals = 24 vertices; every vertex carries the shift.
  - A 400-vertex circle-like ring gives at most 64 verticals.
  - `push_obstacle_lines` (app/obstacle_marks.rs test, `AppState` from the
    controller fixture): one box obstacle with clearance → prism in
    `OBSTACLE`, ring in `OBSTACLE_CLEARANCE`; no plane line when the
    selection is `None`; a plane line at `plane_z + shift.z` when the
    selection is the setup; nothing when `show_obstacles` is false; a
    second, broken obstacle (a missing model id) does not remove the first
    one's lines (D10).
- T6 MCP: in `app/mcp/commands/obstacles.rs` tests,
  `the_obstacle_json_names_the_kind_once_k2`: `obstacle_json` of an
  extrude has no top-level `kind`, and `geometry.kind == "outline_extrude"`.

## 6. The editor's gate run (in this order; the lane is single)

1. `cargo test -p rs_cam_core -q --lib compute::obstacle`, then
   `cargo test -p rs_cam_core -q --features test-support --test geometry_cache_g8`
2. `cargo test -p rs_cam_viz -q --lib` (13 s; holds T2-T6, `g_holderstale`,
   `g_undofresh`, `pin_edit_one_door_p5`).
3. `cargo test -p rs_cam_viz -q --test egui_draw_sites_write_through_commands_wp6 --test effects_are_stamped_wp19 --test apply_contract_a3`
4. `cargo test -p rs_cam_viz -q --test overlays_registry --test the_dock_states_read_live_state_g_vpstate --test the_viewport_dock_reaches_every_option_g_vpdock --test reach_overlay_p5`
5. `cargo test -p rs_cam_viz -q --test render_pipelines_headless_g_pipesmoke --test viewport_draws_selected_only_wp27`
6. `cargo test -p rs_cam_viz -q --test panel_drafts_leave_egui_memory_ui09 --test ui_string_hygiene --test the_inspector_nests_once_dc5 --test panels_read_the_token_module_up1`
7. The wire snapshot re-bless (item 27), then `--test mcp_wire_surface_pin`,
   `--test command_registry_surfaces`, `cargo test -p rs_cam_mcp -q`.
8. `cargo test -p rs_cam_core -q --test alignment_pin_walls_and_2d_placer_p6`
9. `cargo fmt --all -- --check`; the full clippy line from root `CLAUDE.md`.

Wrap each test command in `timeout` (memory note: one hung test blocks the
lane). Route each through `scripts/cargo_lane.sh`.

## 7. Risks

- R1 The wp6 sentry needs `Command::ReplaceObstacle(` under `src/ui/`.
  D3 keeps it in `panel_apply.rs`. If an editor moves the construction to
  the controller, the sentry goes red.
- R2 Undo of a remove re-adds the obstacle at the END of the list (core
  `AddObstacle` appends). The order has no effect on `resolve`, the plane
  (a max) or the check. The list order on screen changes. Accept it, and
  say so in the commit.
- R3 `cached_silhouette` is the default cell only. Item 1 ties the
  obstacle cell to that const. If someone later gives obstacles their own
  cell, the call must go back to the uncached raster.
- R4 The line buffer has no alpha. "Faint" is a dim colour. On a dark
  background the dim clearance ring can be hard to see; the live look
  decides the value.
- R5 D13 (mesh exclusion) changes what the part-mesh pass draws. A model
  that is BOTH a part and an obstacle model disappears from the part draw
  on that setup. That is a modelling error; the obstacle prism still draws.
  If the lead rejects D13, delete item 21's filter and write the defect in
  the module doc.
- R6 A panel-side refusal: `apply_obstacle_draft` runs in a draw site with
  no notification door (`push_notification` is on the controller). The
  banner alone carries the refusal. S4 also notifies; that difference is
  acceptable because the obstacle editor stays open with the banner.
- R7 The overlay rename drops a saved `[display.overlays] fixtures` value.
  Ruled acceptable (no legacy). The rename touches the MCP descriptions and
  the wire snapshot; re-bless in the same run.
- R8 `pin_obstacles` still resolves every setup per frame. After D9 the
  raster is memoised; `grow` and the outline union remain per call. If the
  live GUI still stutters on the pin panel, D9b is the next step.
- R9 Two near-duplicate fact helpers exist: core
  `compute::obstacle::model_geometry_facts` (:325) and
  `session::mutation::stock_change::loaded_model_facts` (:24). Not in K2
  scope; note it for the duplicate sweep.
- R10 The GUI must be rebuilt from HEAD before the live look (root
  `CLAUDE.md`); the release binary is what the MCP proxy runs.
