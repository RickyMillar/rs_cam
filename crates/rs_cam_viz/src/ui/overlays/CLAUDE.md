# `ui/overlays/` — the overlay registry, the catalogue and the legend rail

One registry for every viewport overlay; the dock (`../viewport_overlay.rs`)
and the All viewport options catalogue read it. Entry: `ui::overlays::mod`.

## Files

- `registry.rs` — every overlay, `dock_section`, exclusivity, the legends.
- `live.rs` — computing, stale and failed per analysis, from live fields.
- `panel.rs` — the catalogue, the shared row renderer, `RowState` (seven
  arms), Compute versus Compute & show, the Compute rest confirm.
- `legend_rail.rs` — one block per rendered colour encoding, above the dock.

## Invariants

- Registration, MCP, the dock and the catalogue read the SAME registry.
- Every row has ONE dock section and is listed in the catalogue.
- An unavailable overlay is refused with a reason, never silently accepted.
- A row state is derived each frame; never store a computing, stale or
  failed flag. `live.rs` names the field that answers.
- A `Compute & show` result applies only when the target and the mode still
  match (`settle_pending_show`). Never switch a row on at submit.
- A legend describes the rendered result, not the flag. A render colour
  with no public source is mirrored in `legend_rail::mirrored`, and the
  vpstate sentry pins the render literal.
- The dock and the catalogue float over the 3D view. Neither takes width.
- The reach map overlay is an UPPER estimate; see
  `../../../../rs_cam_core/src/maps/CLAUDE.md`.

## Sentries

- `cargo test -p rs_cam_viz -q --test overlays_registry`
- `cargo test -p rs_cam_viz -q --test reach_overlay_p5`
- `cargo test -p rs_cam_viz -q --test the_viewport_dock_reaches_every_option_g_vpdock`
- `cargo test -p rs_cam_viz -q --test the_dock_states_read_live_state_g_vpstate`

## Do not

- Draw an overlay without a registry row, or put dock state in a
  `pub … : bool` on `ViewportState` (use `state/overlays.rs`).
