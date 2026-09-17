# `ui/properties/` — the inspector

The right-hand inspector: the tabs that edit a setup, a tool, the stock, the
post and one operation. The entry point is `ui::properties::mod`.

## Files

- `mod.rs` — `draw`, `PanelEdit`, `ToolpathTab`, the snapshot API; children
  beside it: `panel_apply.rs` (apply/flush, the command door),
  `model_sim_panels.rs`, `machine_panel.rs`, `feeds_speeds.rs` (Feeds tab,
  LUT viewer), `tab_badges.rs`, `linking_dressup.rs` (+ `dv` grid helpers),
  `toolpath_panel.rs` (`draw_toolpath_panel`, one function), `tests.rs`.
- `operations/` — one editor per operation family: 2D boundary, drilling,
  engraving, finishing, 3D surface, project curve.
  Its children beside `mod.rs`: `shape_diagrams.rs` (the thirteen parameter
  minimaps, `StepoverPattern`), `height_diagram.rs` (height-versus-stock
  profile), `validate.rs` (toolpath validation, inspector diagnostics).
- `setup.rs`, `stock.rs`, `tool.rs`, `post.rs` — the four resource tabs.
- `pills.rs` — `PillSuggestions`, what the per-field pills offer.

## Invariants

- Every edit writes through the core command path. An inspector field is not
  a place to hold a value.
- The inspector nests ONCE. A tab does not open a second scroll area.
- The inspector width does not depend on the selected tab.
- A pinned Bottom Z note must say which operations honour it; the core answer
  is `OperationType::honors_pinned_bottom_z()`.

## Sentries

- `cargo test -p rs_cam_viz -q --test bottom_z_pin_note_g_bottompin`
- `cargo test -p rs_cam_viz -q --test boundary_controls_always_visible_g_boundaryinherit`
- `cargo test -p rs_cam_viz -q --test the_inspector_nests_once_dc5`
- `cargo test -p rs_cam_viz -q --test inspector_width_is_tab_independent_up4`
- `cargo test -p rs_cam_viz -q --test depth_beyond_stock_cautions_g_depthstock`

## Do not

- Do not draw a raw egui widget where `ui/components/` has the renderer.
