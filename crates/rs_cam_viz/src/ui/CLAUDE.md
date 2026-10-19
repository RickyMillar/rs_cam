# `ui/` — the egui surfaces

Every panel, modal and workspace. `ui::mod` runs once per frame from `../app.rs`.

## Files

- `mod.rs`, `workspace_bar.rs`, `menu_bar.rs`, `status_bar.rs` — the chrome.
- `tokens.rs` — every token (`PATH_OPEN` too); `theme.rs` re-exports it.
- `components/`, `feeds/`, `overlays/`, `properties/` — see each child file.
- `setup_panel.rs`, `toolpath_panel.rs`, `toolpath_row_controls.rs` — the Setup rail (an eye per model row)
  and the toolpath rows with the gutter connector; `add_menu_offers` gates the 2D ops on `has_selectable_item`.
- `sim_*.rs` — the Simulation workspace, with `draw_trace_badge`. The inspector is the verdict line, then
  ONE `TabStrip`: Metrics (default, the charts first), Warnings (count badge), Navigate, Project (IC3).
  A card flips to a median line on a min-max band; `< >` opens it large.
- `readiness.rs`, `readiness_panel.rs`, `preflight.rs` — is this safe to cut?
  `shoe_list.rs` — the H2 run sheet; Readiness and the export wizard draw it.
- `export_wizard.rs`, `pin_wizard.rs` (R4: two steps, the only pin editor), `multitool_planner.rs`, `*_modal.rs`.
- `import_modal.rs` — the drawing import dialog (declared size, scale, result, join, report). A number reads
  the file again on commit only (`committed_number`; its draft is in `state.panels.number_drafts`).
- `viewport_overlay.rs`, `automation.rs`, `shortcuts_window.rs`, `preferences.rs` — dock (legend: `overlays/`), automation, shortcuts, prefs.
- `pick_bar.rs` — the pick-mode filter bar and the tab editor bar, top centre, no width (L-4). A disabled
  chip names the core reason. `COMPUTING_FACES` shows while the faces of an STL are cold.

## Invariants

- The selected toolpath is the default viewport draw set (WP27). Visibility still bites; an eye control
  must stay usable when a hidden selection would draw nothing.
- The Simulation workspace has ONE visible full-run primary. No surface adds a second run route.
- Every Run Simulation affordance uses `simulation_request_is_buildable`.
- Declutter removes a control; it does not hide a duplicate route. An empty queue keeps its "Add toolpath" (g_emptyadd).
- A dependency between operations is ONE line in the card gutter, read from
  `dependencies::primary_edges`. No surface draws a second badge for it.
- The crate is on egui 0.36 with `egui_plot` 0.37; the two differ on purpose.

## Sentries

- `cargo test -p rs_cam_viz -q --test viewport_draws_selected_only_wp27 --test pick_mode_picks_by_kind_p4_5`
- `cargo test -p rs_cam_viz -q --test the_simulation_page_is_summary_first_dc6 --test the_sim_inspector_is_tabbed_ic3`
- `cargo test -p rs_cam_viz -q --test panels_read_the_token_module_up1 --test ui_string_hygiene`
- `cargo test -p rs_cam_viz -q --test connector_reads_edge_state_g_connector --test the_dot_becomes_a_ring_in_flight_g_ring --test cut_metric_cards_read_the_gate_g_cutcards`
