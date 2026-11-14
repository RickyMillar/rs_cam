# `ui/` — the egui surfaces

Every panel, modal and workspace. `../app.rs` calls the `draw` of each surface once per frame.

## Files

- `mod.rs`, `workspace_bar.rs`, `menu_bar.rs`, `status_bar.rs` — the chrome.
- `tokens.rs` — every token (`PATH_OPEN` too); `theme.rs` re-exports it.
- `components/`, `feeds/`, `overlays/`, `properties/` — see each child file.
- `setup_panel.rs`, `toolpath_panel.rs`, `toolpath_row_controls.rs` — the Setup rail (an eye per model row)
  and the toolpath rows with the gutter connector; `add_menu_offers` calls `LoadedModel::serves` for each model (M10).
- `sim_*.rs` — the Simulation workspace, with `draw_trace_badge`. The inspector is the verdict line, then
  ONE `TabStrip`: Metrics (default: FOUR cards chipload/power/deflection/cut force, one hairline, then the width x depth chart `sim_cut_chart.rs` on the kit with the cut samples; P17 layout B), Warnings (count badge), Navigate, Project (IC3). P30-1: under the Metrics "Operation" row, the feeds-that-run line of the Passes tab (`properties::draw_emitted_feeds_line`). P17b (operator 2026-11-07): the samples are a heat map (one ink hue, `heat_hue`, a rank ramp (histogram equalisation, datashader `eq_hist`) of `HEAT_STEPS` from the histogram's wash to full strength: brighter where more cut time; `square_rows`: square cells on the screen; no cluster box, no "steady" text); faint iso-force contours from the core `ForcePlane` (P29-10 10g: every op with a force line; the budget line `CAUTION` "100 % budget" where it is a limit, else `TEXT_FAINT` "budget, no limit here"; each named on its line, under the fills); the planned cuts at the stated point as hollow squares (`stated_marks`, no tone unless the stated point is the proposal; hover only, a click seeks the cell under it); `depth_view`: the heat grid covers the DATA range, a flat depth limit above it extends the AXIS to the lowest such limit so it draws and is named inside the frame, a limit over `OFF_SCALE_FACTOR` x the range keeps one quiet gutter row; the planned dot paints LAST on a `SURFACE_BASE` halo. Sentry `the_sim_chart_seeks_on_a_dot_p17`.
  A card flips to a median line on a min-max band; `< >` opens it large. Span help is core `DebugSpanKind::help` (F017). P29-10 10e: every limit row on the sim surfaces (cards, "Other limits", the closed "Cut groups · n" drawer, the op list flags, Readiness) is a core `CheckRow` from `cached_limit_rows`: core face, detail and tone, `role_of_tone` the one colour map, `CheckRow::worse_than` the one rank; no viz match on a limit or state enum (`no_surface_matches_a_limit_p29_10`, its allow-list empty since 10g).
- `readiness.rs`, `readiness_panel.rs`, `preflight.rs` — is this safe to cut? K6 (ruling Q6): the pre-flight
  refuses nothing. It draws the check cards, ONE chip row (`danger_counts`: a chip per `Danger`, colour and count,
  hover lists `id · toolpath`) and ONE Export button, Danger style while a Machine warning stands
  (`export_asks_a_click`); no checkbox (`a_job_with_each_finding_exports_and_names_it_on_every_surface_k6`).
  `shoe_list.rs` — the H2 run sheet; Readiness and the export wizard draw it.
- `export_wizard.rs`, `pin_wizard.rs` (R4: two steps, the only pin editor), `multitool_planner.rs`, `*_modal.rs`.
- `import_modal.rs` — the drawing import dialog (declared size, scale, result, join, report; S11: it asks the stock thickness while the stock has none, `ImportDialog::stock_z`). A number reads
  the file again on commit only (`committed_number`; its draft is in `state.panels.number_drafts`).
- `viewport_overlay.rs`, `view_cube.rs`, `automation.rs`, `shortcuts_window.rs`, `preferences.rs` — dock (legend: `overlays/`), view cube corner (logic: `../interaction/view_cube.rs`), automation, shortcuts, prefs.
- `pick_bar.rs` — the pick-mode filter bar and the tab editor bar, top centre, no width (L-4). A disabled
  chip names the core reason; a chip is a view filter (P4). `click_event` is THE click map (P3). `COMPUTING_FACES`.

## Invariants

- The selected toolpath is the default viewport draw set (WP27). Visibility still bites; an eye control
  must stay usable when a hidden selection would draw nothing.
- The Simulation workspace has ONE visible full-run primary. No surface adds a second run route.
- Every Run Simulation button is enabled by `simulation_request_is_buildable`; the `AppEvent::RunSimulation` handler refuses through the builder.
- Declutter removes a control; it does not hide a duplicate route. An empty queue keeps its "Add toolpath" (g_emptyadd).
- A dependency between operations is ONE line in the card gutter, read from `dependencies::primary_edges`. No surface draws a second badge for it.
- The crate is on egui 0.36 with `egui_plot` 0.37; the two differ on purpose. The envelope plot, the nomogram and the cut-metric charts use the kit (`components/chart.rs`, `histogram.rs`), not `egui_plot`; only the readiness scatter (`readiness_panel.rs`, its machine-envelope layer included) still does.
- A panel draws a field, a list, a table or a message with the kit (`components/`); the toolpath inspector is held to it by `the_inspector_draws_only_kit_rows_w6`.

## Sentries

- `cargo test -p rs_cam_viz -q --test viewport_draws_selected_only_wp27 --test pick_mode_picks_by_kind_p4_5`
- `cargo test -p rs_cam_viz -q --test the_simulation_page_is_summary_first_dc6 --test the_sim_inspector_is_tabbed_ic3`
- `cargo test -p rs_cam_viz -q --test panels_read_the_token_module_up1 --test ui_string_hygiene --test the_inspector_draws_only_kit_rows_w6`
- `cargo test -p rs_cam_viz -q --test connector_reads_edge_state_g_connector --test the_dot_becomes_a_ring_in_flight_g_ring --test cut_metric_cards_read_the_gate_g_cutcards`
