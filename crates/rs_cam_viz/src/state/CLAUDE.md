# `state/` — GUI state

The view-side state. It is not an alternate data model; the core owns the
project. The entry point is `state::AppState` in `mod.rs`.

## Files

- `mod.rs` — `AppState` and the facade.
- `freshness.rs` — one freshness state per toolpath, DERIVED, never stored.
- `stale.rs` — the one place the GUI stamps `stale_since` from a core answer.
- `toolpath.rs` and `toolpath/` — the toolpath rows, their catalogue entries
  and configuration support.
- `simulation.rs`, `job.rs`, `runtime.rs` — the simulation state, the job
  lane state and the GUI-only runtime overlay state.
- `viewport.rs`, `selection.rs`, `overlays.rs` — the viewport, the selection
  and the Overlays panel state.
- `history.rs`, `wizard.rs`, `multitool_planner.rs`, `rest_dependency.rs` —
  undo history, the export wizard, the planner dialog and the one rule for a
  Rest operation's predecessor.

## Invariants

- Metric-capture staleness is derived from the accepted run's capture
  revision. Do not reintroduce a mutable stale boolean, and do not clear it
  on an un-stamped, cancelled or failed result.
- Freshness is derived. A stored freshness flag drifts from the core answer.
- Selection, viewport and overlay state are view state. They never decide
  what the core generates.

## Sentries

- `cargo test -p rs_cam_viz -q --test freshness_surfaces_g_freshrender`
- `cargo test -p rs_cam_viz -q --test rest_badge_one_predicate_g_restbadge`
- `cargo test -p rs_cam_viz -q --test effects_are_stamped_wp19`
- `cargo test -p rs_cam_viz -q --test load_requests_only_25d_regen_g_loadregen`
