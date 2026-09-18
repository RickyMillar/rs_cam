# `ui/` — the egui surfaces

Every panel, modal and workspace. `ui::mod` runs once per frame from `../app.rs`.

## Files

- `mod.rs`, `workspace_bar.rs`, `menu_bar.rs`, `status_bar.rs` — the chrome.
- `tokens.rs` — every token; `theme.rs` re-exports it.
- `components/`, `feeds/`, `overlays/`, `properties/` — see each child file.
- `setup_panel.rs`, `toolpath_panel.rs`, `toolpath_row_controls.rs` — the
  Setup rail and the toolpath rows, with the card's gutter connector.
- `sim_*.rs` — the Simulation workspace, with `draw_trace_badge`.
- `readiness.rs`, `readiness_panel.rs`, `preflight.rs` — is this safe to cut?
- `export_wizard.rs`, `optimize_modal.rs`, `optimize_project.rs`,
  `multitool_planner.rs`, `*_modal.rs` — the modals.
- `viewport_overlay.rs`, `automation.rs`, `shortcuts_window.rs` — the strip
  above the 3D view, automation, the shortcuts window.

## Invariants

- The selected toolpath is the default viewport draw set (WP27). Visibility
  still bites; an eye control must stay usable when a hidden selection would
  draw nothing.
- The Simulation workspace has ONE visible full-run primary. No surface may
  add a second run route.
- Use `simulation_request_is_buildable` for every Run Simulation affordance,
  not a simpler "has a generated toolpath" check.
- Declutter removes a control; it does not hide a duplicate route.
- A dependency between operations is ONE line in the card gutter, read from
  `dependencies::primary_edges`. No surface draws a second badge for it.
- The crate is on egui 0.34.3 with `egui_plot` 0.35; the two differ on purpose.

## Sentries

- `cargo test -p rs_cam_viz -q --test viewport_draws_selected_only_wp27`
- `cargo test -p rs_cam_viz -q --test the_simulation_page_is_summary_first_dc6`
- `cargo test -p rs_cam_viz -q --test panels_read_the_token_module_up1`
- `cargo test -p rs_cam_viz -q --test ui_string_hygiene`
- `cargo test -p rs_cam_viz -q --test connector_reads_edge_state_g_connector`
- `cargo test -p rs_cam_viz -q --test the_dot_becomes_a_ring_in_flight_g_ring`
