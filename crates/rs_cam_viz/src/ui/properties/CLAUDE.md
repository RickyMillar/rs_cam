# `ui/properties/` — the inspector: the tabs that edit a setup, a tool, the stock, the post and an op

## Files
- `mod.rs` — `draw`, `PanelEdit`, `ToolpathTab`, the snapshot and inputs builders, `apply_auto_enable`.
  Beside it: `panel_apply.rs`, `machine_panel.rs`, `feeds_speeds.rs`, `tab_badges.rs`, `tests.rs`,
  `linking_dressup.rs` (`Param`, `dv`, `dv_pill`, `dv_dressup`, `advanced`), `toolpath_panel.rs` (a fn per tab).
- `selection_section.rs` — one section per selection: the op target at the TOP of the Geometry tab (below
  Input, above `tab_badges::draw_start_from`), the boundary target under Machining Boundary. ONE Pick, in
  the action row (`PICK_A_FACE_FIX` has none). An op with no op selection draws `Pick faces` (the boundary
  target, any source). The Cuts row reads "Computing faces…" while `faces_not_ready`. Rows have "Show me". P6.7: a Drill or Bore gets one row per hole size (`hole_sizes`: report `hole_fits`, `tool_diameter`); a Drill with a `Larger` size one `ADD_A_BORE` line (`AppEvent::AddBoreFromDrill`); a Pocket Extend one "Open paths" line.
- `model_sim_panels.rs` — a drawing gets "Import" (scale, join tolerance, report) and "Open shapes" (core
  `open_shapes`, Show me, the two close acts, Join all within, a lost edit in DANGER, the ledger in `Details`).
- `operations/` — an editor per op family, `registry.rs`, `shape_diagrams.rs`, `height_diagram.rs`, `validate.rs`;
  `setup.rs`, `obstacles.rs` (K2: list + editor), `stock_changes.rs` (S4), `stock.rs`, `pins/` (P7), `tool.rs`,
  `post.rs`, `pills.rs`, `head.rs` (H2: the Machine panel's Head section), `shoe_row.rs` (H2: the dust shoe row).

## Invariants
- Every edit writes through the core command path. Helix r: None = rule. The inspector nests ONCE.
- The toolpath panel takes `ToolpathPanelSnapshot` and `ToolpathPanelInputs`. Add a field there, not a
  parameter. The width does not depend on the tab.
- A panel door marks the project edited on success; `app.rs` guards on it. A pinned Bottom Z note names
  the ops that `capabilities().floor_owner` does not mark `HeightsBottom`. An `OpFloor` op has the Floor row (`floor_row`); a reference change and a diagram drag use `FloorReferences` (L-10); the Heights tab has a read-only Floor line, no Bottom row (L-11); the O-2 hint is under the Selection section. The diagram floor runs come from `floor_runs` (`FloorInputs::face_floors`: Z and face count); every run has a hover. F033: every heights write (a row, an Auto button, a diagram drag and its rounding) goes through `admit_heights_edit` (core `ToolpathHeights::edit_refusal`); the badge is `height_order_badge`; the refusal text is GUI state (`state.panels.heights_refusal`, scoped to the toolpath), not egui memory (ui09).
  "Show me" frames `selection_highlight::note_item_keys`: the skipped items of a note's kind and layer, or the one face of an at-the-top note.
- A draw never writes `rest_analysis.enabled`; `apply_auto_enable` is the door. Do not draw a raw egui
  widget where `ui/components/` has one. A tooltip keys on the registry parameter NAME, never the label.
- The Selection section writes `EditToolpathSelection` only (L-1), never the draft. The Profile Tabs
  row toggles the tab editor (`OpDrawCtx::selection_ui`); the editor writes `EditTabs`.
- F216: the pills and the Feeds card read `entry.feeds_memo`. A new feeds input goes in `FeedsInputs`, not a second read.
- One `OpUiRow` per op carries the editor, the diagram decision and the validation arm; a blank diagram says why.
- IC1/IC2: a field is one row, its explanation the hover. Derived data is `DataTable` rows, derivation in ONE
  `Details`; only a blocking error wraps (`DANGER`). A finding has one owner, the footer row. Rare dials go in
  the tab's ONE `advanced` section. A diagram comes before its rows.

## Sentries (`cargo test -p rs_cam_viz -q`)
- `--test bottom_z_pin_note_g_bottompin --test height_diagram_drag_prefers_bottom_and_rounds_r5 --test stock_panel_edit_clears_auto_from_model_r12`; floor: `--test every_depth_shows_its_reference_p3_7 --test a_floor_reference_change_keeps_the_cut_p3_7 --test the_heights_diagram_fits_f2 --test one_height_order_rule_for_every_surface_f033`
- `--test boundary_controls_always_visible_g_boundaryinherit --test depth_beyond_stock_cautions_g_depthstock`; `--lib a_setup_map_op_draws_no_boundary_controls_d3`
- `--test the_inspector_nests_once_dc5 --test inspector_width_is_tab_independent_up4 --test the_inspector_text_is_bounded_ic1 --test tabular_data_reads_one_table_ic2`
- `--test the_head_and_shoe_rows_fit_the_rail_h2 --test the_help_key_is_the_param_name_ui04 --test operations_registry_ui05 --test one_start_from_row_g_startfrom`
- `--test keyed_pin_panel_reads_the_real_strips_g_pinpanel --test pin_editor_and_viewport_p7 --test the_selection_section_writes_edits_only_p4_3`
- `--test show_me_frames_the_items_p4_4 --test a_2d_op_cuts_a_step_face_p4_d --test the_panel_feeds_run_once_per_input_f216`; `--lib p4_d` (one Pick: `the_selection_section_shows_one_pick_p4_d`); P6.7: `--test side_on_greys_in_control_p6_7 --test the_hole_rows_show_each_fit_p6_7 --test the_pocket_open_paths_row_p6_7 --test add_bore_copies_the_selection_p6_7` (Profile On: core `domain::side_on_*` predicates)
