# `state/` — GUI state

The view-side state; the core owns the project, so it is not an alternate data model. Entry: `state::AppState`.

## Files

- `mod.rs` — `AppState`. F096: `session` is private. Read `session()`; write only `apply` (stamps `Effects.stale`,
  raises the sim flag, logs a refusal), `replace_session`, `start_job`; `split_mut` gives disjoint borrows.
- `freshness.rs` — `FreshnessState` per toolpath and `SimFreshness`: both DERIVED, never stored.
- `stale.rs` — the one place the GUI stamps `stale_since` from a core answer.
- `toolpath.rs` and `toolpath/` — the toolpath rows, their catalogue entries and configuration support. R2: `ToolpathEntry.tool_id` is `Option` (an op with no tool), as `ShoePlanRow.tool_id` and `RunLine.tool_id`.
- `simulation.rs`, `job.rs`, `runtime.rs` — sim, job lanes, GUI runtime; `FeedsMemo::refresh` is the one `feeds_result` writer (F216);
  `ToolpathRun` is the run and its result as one value (F031), `status()` derives `ComputeStatus`.
- `simulation/` — `playback_state.rs`, `issue_triage.rs`, `semantic_trace.rs`. `viewport_input.rs` — `DragState` (B2), held on `SelectionUi::drag`.
- `camera.rs` — `CameraState` (A C2): the pose is private; `pose()` reads, the methods write; `first_model_bounds`, `fit_to_first_model`.
- `viewport.rs` — `drawing_draws`: every visible drawing draws; the selected op's model draws its states.
- `selection.rs` — the tree focus `Selection`, `wire_kind`. F2: the field is private; `focus()` reads, `set_focus()` writes (returns `FocusChange`). `overlays.rs` — the dock and catalogue; `ModelVisibility` (view state). `selection_highlight.rs` — the report the viewport colours, `ReportDetector`, `HoverDetector`, the note keys (a `NoOpenSide` note frames its first face, a `FreesThePart` note the part outline); P6.7 open path marks: `open_path_side`, `path_middle`, `side_point` (drawing frame).
- `pick.rs` — `SelectionUi` on `GuiState`: one `ViewportMode { Pick, Tabs }` at a time (L-3 by type) and
  the Geometry tab state (`offers_for`: the open "Rule…" menu; `row_hover`: the `HoverDetector` draws it when no pick
  hovers; `list_filter`); `PickFilter::effective_kinds` (on, served, held), `cut_kinds` (served, held: P4); `held_kinds` holds `Face` for an STL.
- `tab_edit.rs` — the `tab_layout` read per toolpath revision and `TabMarksDetector`.
- `history.rs`, `wizard.rs`, `multitool_planner.rs` — undo (`ModelDrawingEdits`; P6.7 `BoreAdded`, `rebind_bore` after a redo), the export wizard, the planner.
- `panels.rs` — panel drafts (`join_all_mm`, `drawing_popover`, `number_drafts`: the value in flight of
  `committed_number`, ui09). `app_settings.rs`; `pin_view.rs` (pin frame, C8); `shoe_plan.rs` (shoe plan, H2).

## Invariants

- `last_run` marks the last accepted run; it pairs with `has_results()`
  except in `SimFreshness::Released`. A cancel or a failure keeps it.
- Every run captures the cut trace (ruling 2026-10-02). `save_cut_trace` decides the FILE only.
- Freshness is derived. A stored freshness flag drifts from the core answer.
- `AppState::simulation_is_stale` is the ONE door onto simulation freshness: core inputs, never the counter.
- A panel draft with CONTENT lives here, not in egui temporary memory. A
  view toggle or a drag index may stay in egui memory, named with a reason.

## Sentries

- `cargo test -p rs_cam_viz -q --test <name>`: `freshness_surfaces_g_freshrender`, `connector_reads_edge_state_g_connector`,
  `effects_are_stamped_wp19`, `load_requests_only_25d_regen_g_loadregen`, `panel_drafts_leave_egui_memory_ui09`,
  `the_tab_editor_writes_one_step_per_act_p4_6`, `the_panel_feeds_run_once_per_input_f216`
