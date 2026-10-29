# `app/` — the application shell and the embedded MCP server

The egui application, its per-frame work and the GUI-side MCP requests. The server lives here, not in `rs_cam_mcp`.

## Files

- `../app.rs` — `RsCamApp` and the frame update loop.
- `mcp.rs` + `mcp/` — the MCP pump (`handle_mcp_request`, export, notifications); `commands.rs` (core route);
  `commands/`: `stock_changes.rs` (S4), `selection.rs` (P6.8: `hole_fit` rows, `extended` (also the "Cut past open sides" areas, lib `mcp_and_gui_give_one_open_sides_pocket_opensides`); R-E: `hole_json` on list rows, `rows`, `holes`; `item_states` reads the unfolded model items; R-A: `outline_role` gives `role: "part_outline"` on list and status rows), `import.rs` (`import_model` options,
  `set_model_import_options`), `drawing.rs` (`edit_model_drawing`), `floor.rs` (`set_toolpath_floor`, the `heights` JSON), `obstacles.rs` (K1 obstacle tools), each with its GUI parity tests;
  `project.rs`, `generation.rs`, `diagnostics.rs` (span-kind synonyms from core `DebugSpanKind::structural`, F017), `simulation.rs`, `view.rs` (screenshots, `set_ui_view`).
- `export.rs` — the GUI export path. `simulation.rs` — per-frame simulation work. `input.rs` — app input. `file_dialog.rs` (F218) — every `rfd` dialog, off the frame, one at a time; `AfterPick` acts on the pick.
- `viewport.rs`, `gpu_upload.rs` + `gpu_upload/` (F132: a file per slot, key and build read one `SlotInputs`), `pin_marks.rs` (P7), `obstacle_marks.rs` (K2) — the frame and the upload.
  - The drawing takes the item state colours, open path ends, lost-pick ghosts and the boundary outline. P6.7: a mid-path arrow, the tool side mark of a Cut path (drawing frame, then `display`), each report `extended` ring (no rays), a `HOLE_MISFIT` drill marker.
    It uploads on a change of the report `Arc` (`ReportDetector`), never per frame.
  - A drawing that no drawn toolpath reads uploads as `neutral_drawing_vertices` (active setup frame).
  - A hidden model (`ModelVisibility`) uploads no mesh and no drawing. Every pick reads `PickContext::visibility`: a hidden model takes no face, item, open path, hole or tab-loop pick.
  - A click goes to the tab editor, then pick mode, then the drawing popover, then the plain pick.
    The tab editor also wins the right-click and a drag that starts on a tab (no orbit; B2: `step` claims it first). A C3a: the orbit pivot and the pan grab depth come from the point under the press origin (`drag_anchor`).
  - Pick mode: `pick_bar::click_event` maps a click (P3: shift = `AddPick`, right = `PickLayer`; B4: `PickClick::primary(modifiers)`, Ctrl = `RemovePick`; E5: right-click on an item = `pick_bar::PickMenu` (layer row, then the `list_rule_offers` rows, `panels.pick_menu`), Ctrl+A = `select_all_edit` → core `kind_rule_edit`; B3: `InputEnv.scheme` = `app_settings.display.mouse_scheme` each frame; B6: a left drag in pick mode is a box (`handle_box_release` → `gesture_events(BoxRelease)`, `pick_bar::paint_box`; Escape cancels it first, `SelectionUi::cancel_box`)); `pick_mode_item` (P4) picks a chip-off kind second; the ray hits an STL mesh.
  - B1: `hover_result` picks once per frame; every hover reader reads it. F4: one `HoverTarget` (pick item, else `row_hover`, else the face); a click hit goes through `interaction::intent::gesture_events` (focus by `UiCommand::Select`); Escape sends `EndViewportMode`. The hover ring (`upload_hover_lines`)
    and the tab marks (`upload_tab_lines`) are their own buffers. D5: the hover ring reads `face_items` for an STL face.
  - D3: `gpu_upload/face_state.rs` writes the STEP face state words (`(ModelId, FaceGroupId)` marks) after `meshes::upload`; a hover builds no mesh. D4: `gpu_upload/model_edges.rs` builds the face edges from the `meshes::inputs` walk.
  - `set_ui_view`: `model_visibility` sends `SetModelVisibility`; `frame_items` calls `frame_selection_items`; `camera` calls `state::camera::apply_camera_request` (instant, checked first); `selection` and `get_ui_view` read `focus_summary`.

## Invariants

- An MCP mutation dispatches through the same command path as the UI. S11: `add_toolpath` drafts with core `draft_toolpath` and replies `unset` (R2/Q7: no `tool_index` = first fitting tool, reply `no_tool`; no `model_id` = the core model rule; `add_toolpath_via_gui` `model_id` = the selected model; parity `every_add_door_takes_one_model_and_tool_q7`); `get_toolpath_params` gives `unset`; `import_model` gives `stock_thickness_set`.
- A setter stales the result. Regenerate before you export. Tool and model rebinding uses its own endpoint, not a generic setter.
- Export refuses missing or stale enabled geometry, and an unsafe or unmodelled load verdict, unless
  the caller accepts the override. An MCP export reports the machine-safety findings beside the path
  it wrote: take `io::export`'s reporting door; do not re-run the validator.
- Use `generation_status` and `cancel_generation` for a long job. Do not queue a second generation.
- An MCP screenshot is visual evidence, not a core test. Capture after the frame applied the view.
- Read `mcp/CLAUDE.md` before you change the pump, the `mcp-server` thread or the winit host exit.

## Sentries

- `cargo test -p rs_cam_viz -q --test <name>`: `mcp_authoring_surface`, `mcp_core_arm_describes_every_row`,
  `mcp_escape_hatches`, `mcp_wire_surface_pin`, `export_parity_core_vs_gui_p0`, `a_click_picks_the_drawn_item_p2_6`, `file_dialogs_run_off_the_frame_f218`
