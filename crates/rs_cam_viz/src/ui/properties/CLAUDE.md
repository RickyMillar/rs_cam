# `ui/properties/` — the inspector

The tabs that edit a setup, a tool, the stock, the post and an operation.

## Files

- `mod.rs` — `draw`, `PanelEdit`, `ToolpathTab`, the snapshot and inputs
  builders, `apply_auto_enable`; beside it `panel_apply.rs`,
  `model_sim_panels.rs`, `machine_panel.rs`, `feeds_speeds.rs`,
  `tab_badges.rs`, `tests.rs`, `linking_dressup.rs` (`Param`, `dv`,
  `dv_pill`, `dv_dressup`, `advanced`), `toolpath_panel.rs` (one fn per tab),
  `selection_section.rs` (P4.3: the Selection section, op and boundary target; P4.4: "Show me" on a note row and a lost pick; P4.7: the Cuts row reads "Computing faces…" while `faces_not_ready`; P4.D: the op section is at the TOP of the Geometry tab, below Input and above `tab_badges::draw_start_from`; `PICK_A_FACE_FIX` (+ Pick) on an `Empty` refusal when the model holds faces; an op with no slot draws `Pick faces`, the boundary target, any source).
- `operations/` — one editor per op family, plus `registry.rs`, `shape_diagrams.rs`, `height_diagram.rs`, `validate.rs`.
- `model_sim_panels.rs` P4.8: a drawing gets an "Import" section (scale, join tolerance, report; the rows of `ui/import_modal.rs`); P4.9 "Open shapes" (core `open_shapes`, Show me, the two close acts, Join all within, a lost edit as a DANGER row, the ledger in one `Details`). `setup.rs`, `obstacles.rs` (K2: list + editor), `stock_changes.rs` (S4), `stock.rs`, `pins/` (P7: one block, the wizard parts), `tool.rs`, `post.rs`, `pills.rs`, `head.rs` (H2: the Machine panel's Head section), `shoe_row.rs` (H2: the Heights tab's dust shoe row).

## Invariants

- Every edit writes through the core command path. Helix r: None = rule.
- The inspector nests ONCE. A tab does not open a second scroll area.
- The toolpath panel takes `ToolpathPanelSnapshot` and `ToolpathPanelInputs`.
  Add a field there, not a parameter. The width does not depend on the tab.
- A panel door marks the project edited on success; `app.rs` guards on it.
  A pinned Bottom Z note names the ops `honors_pinned_bottom_z()` lists.
- A draw never writes `rest_analysis.enabled`; `apply_auto_enable` is the
  door. Do not draw a raw egui widget where `ui/components/` has one.
- A tooltip keys on the registry parameter NAME, never the label. The selection
  section writes through `EditToolpathSelection` only (L-1), never the draft. P4.6: the Profile Tabs row toggles the tab editor (`OpDrawCtx::selection_ui`); the editor writes through `EditTabs`, never the draft.
- One `OpUiRow` per operation carries the editor, the diagram decision
  and the validation arm. A blank diagram states its reason.
- IC1/IC2: a field is one row; its explanation is the hover. Derived data is
  `DataTable` rows, derivation in ONE closed `Details`; only a blocking error
  wraps, in `DANGER`. A finding has one owner, the footer row. Rare dials go
  in the tab's ONE `advanced` section. A diagram comes before its rows.

## Sentries

- `cargo test -p rs_cam_viz -q --test bottom_z_pin_note_g_bottompin --test height_diagram_drag_prefers_bottom_and_rounds_r5 --test stock_panel_edit_clears_auto_from_model_r12`
- `cargo test -p rs_cam_viz -q --test boundary_controls_always_visible_g_boundaryinherit --test depth_beyond_stock_cautions_g_depthstock`; `--lib a_setup_map_op_draws_no_boundary_controls_d3` (D3: the Geometry tab reads `territory_xy`, Pin Drill draws no boundary)
- `cargo test -p rs_cam_viz -q --test the_inspector_nests_once_dc5 --test inspector_width_is_tab_independent_up4 --test the_inspector_text_is_bounded_ic1 --test tabular_data_reads_one_table_ic2 --test the_head_and_shoe_rows_fit_the_rail_h2`
- `cargo test -p rs_cam_viz -q --test the_help_key_is_the_param_name_ui04 --test operations_registry_ui05 --test the_selection_section_writes_edits_only_p4_3 --test show_me_frames_the_items_p4_4`; `--lib p4_d`; `--test a_2d_op_cuts_a_step_face_p4_d`; `--test one_start_from_row_g_startfrom --test keyed_pin_panel_reads_the_real_strips_g_pinpanel --test pin_editor_and_viewport_p7`
