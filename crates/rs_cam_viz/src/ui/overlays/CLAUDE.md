# `ui/overlays/` — the overlay registry, the catalogue and the legend

One registry for every viewport overlay; the dock (`../viewport_overlay.rs`)
and the catalogue read it. Entry: `ui::overlays::mod`.

## Files

- `registry.rs` — every overlay, `dock_section`, exclusivity, the legends.
- `live.rs` — computing, stale and failed per analysis, from live fields.
- `panel.rs` — the catalogue, the shared row renderer, `RowState` (seven
  arms), Compute versus Compute & show, the Compute rest confirm.
- `legend_rail.rs` — the bottom-right legend: a chip per encoding, detail on hover.

## Invariants

- Registration, MCP, the dock and the catalogue read the SAME registry, and
  every default reader asks `effective_default` (`[display.overlays]`).
- Every row has ONE dock section and is listed in the catalogue.
- An unavailable overlay is refused with a reason, never silently accepted.
- A row state is derived each frame; never store a computing, stale or
  failed flag. `live.rs` names the field that answers.
- A `Compute & show` result applies only when the target and the mode still
  match (`settle_pending_show`). Never switch a row on at submit.
- A legend describes the rendered result, not the flag. A render colour
  with no public source is mirrored in `legend_rail::mirrored`, and the
  vpstate sentry pins the render literal.
- The dock, legend and catalogue float over the 3D view and take no width.
  The legend never covers the dock or the gizmo; a caveat moves to hover.
- The reach map is an UPPER estimate (`rs_cam_core/src/maps/CLAUDE.md`).

## Sentries

- `cargo test -p rs_cam_viz -q --test overlays_registry` and `reach_overlay_p5`
- `cargo test -p rs_cam_viz -q --test the_dock_states_read_live_state_g_vpstate` and `the_viewport_dock_reaches_every_option_g_vpdock`

## Do not

- Draw an overlay without a registry row, or put dock state in a
  `pub … : bool` on `ViewportState` (use `state/overlays.rs`).
- Size an `egui::Area`'s scroll area from its max rect (last frame's rect).
