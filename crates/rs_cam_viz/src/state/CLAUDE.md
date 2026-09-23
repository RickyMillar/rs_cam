# `state/` — GUI state

The view-side state. It is not an alternate data model; the core owns the
project. The entry point is `state::AppState` in `mod.rs`.

## Files

- `mod.rs` — `AppState` and the facade.
- `freshness.rs` — `FreshnessState` per toolpath and `SimFreshness` for the
  simulation. Both DERIVED, never stored.
- `stale.rs` — the one place the GUI stamps `stale_since` from a core answer.
- `toolpath.rs` and `toolpath/` — the toolpath rows, their catalogue entries
  and configuration support.
- `simulation.rs`, `job.rs`, `runtime.rs` — the simulation state, the job
  lane state and the GUI-only runtime overlay state.
- `simulation/` — `playback_state.rs`, `issue_triage.rs`, `semantic_trace.rs`.
- `viewport.rs`, `selection.rs`, `overlays.rs` — the viewport, the selection
  and the viewport dock and catalogue state.
- `history.rs`, `wizard.rs`, `multitool_planner.rs`, `rest_dependency.rs` —
  undo, the export wizard, the planner and the Rest predecessor rule.
- `panels.rs` — the panel drafts that outlive a frame.

## Invariants

- Metric-capture staleness is derived from the accepted run's capture
  revision. Do not reintroduce a mutable stale boolean, and do not clear it
  on an un-stamped, cancelled or failed result.
- Freshness is derived. A stored freshness flag drifts from the core answer.
- `AppState::simulation_is_stale` is the ONE door onto simulation freshness.
  The core answers project inputs, the GUI capture options. Never the counter.
- A panel draft with CONTENT lives here, not in egui temporary memory. A
  view toggle or a drag index may stay in egui memory, named with a reason.

## Sentries

- `cargo test -p rs_cam_viz -q --test freshness_surfaces_g_freshrender`
- `cargo test -p rs_cam_viz -q --test connector_reads_edge_state_g_connector`
- `cargo test -p rs_cam_viz -q --test effects_are_stamped_wp19`
- `cargo test -p rs_cam_viz -q --test load_requests_only_25d_regen_g_loadregen`
- `cargo test -p rs_cam_viz -q --test panel_drafts_leave_egui_memory_ui09`
