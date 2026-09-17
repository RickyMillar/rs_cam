# `ui/properties/` — the inspector

The tabs that edit a setup, a tool, the stock, the post and one operation.

## Files

- `mod.rs` — `draw`, `PanelEdit`, `ToolpathTab`, the snapshot and inputs
  builders; beside it `panel_apply.rs` (the command door),
  `model_sim_panels.rs`, `machine_panel.rs`, `feeds_speeds.rs`,
  `tab_badges.rs`, `tests.rs`, `linking_dressup.rs` (`Param`, `dv`,
  `dv_pill`, `dv_dressup`), `toolpath_panel.rs` (one fn per tab).
- `operations/` — one editor per operation family (2D boundary, drilling,
  engraving, finishing, 3D surface, project curve), plus
  `shape_diagrams.rs`, `height_diagram.rs`, `validate.rs`.
- `setup.rs`, `stock.rs`, `tool.rs`, `post.rs` — the four resource tabs;
  `pills.rs` — the per-field pills.

## Invariants

- Every edit writes through the core command path. An inspector field is
  not a place to hold a value.
- The inspector nests ONCE. A tab does not open a second scroll area.
- The toolpath panel takes `ToolpathPanelSnapshot` (edits) and
  `ToolpathPanelInputs` (reads). Add a field there, not a parameter.
- The inspector width does not depend on the selected tab.
- A panel door marks the project edited on success; `app.rs` guards on it.
- A pinned Bottom Z note names the operations that honour it; the core
  answer is `OperationType::honors_pinned_bottom_z()`.
- Do not draw a raw egui widget where `ui/components/` has the renderer.
- A tooltip keys on the registry parameter NAME, never the label.

## Sentries

- `cargo test -p rs_cam_viz -q --test bottom_z_pin_note_g_bottompin`
- `cargo test -p rs_cam_viz -q --test boundary_controls_always_visible_g_boundaryinherit`
- `cargo test -p rs_cam_viz -q --test the_inspector_nests_once_dc5`
- `cargo test -p rs_cam_viz -q --test inspector_width_is_tab_independent_up4`
- `cargo test -p rs_cam_viz -q --test depth_beyond_stock_cautions_g_depthstock`
- `cargo test -p rs_cam_viz -q --test the_help_key_is_the_param_name_ui04`
- `cargo test -p rs_cam_viz -q --test a_post_edit_marks_the_project_dirty_ui08`
