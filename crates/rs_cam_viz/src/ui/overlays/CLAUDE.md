# `ui/overlays/` — the overlay registry, the catalogue and the legend rail

One declarative registry for every viewport overlay. The viewport dock
(`../viewport_overlay.rs`) and the All viewport options catalogue read it.
The entry point is `ui::overlays::mod`.

## Files

- `mod.rs` — the facade.
- `registry.rs` — the list of every overlay, `dock_section`, the `,` / `.`
  surfaces.
- `panel.rs` — the catalogue window, the shared row renderer, `RowState`,
  `MIN_VIEWPORT_WIDTH`.
- `legend_rail.rs` — one block per active colour encoding, above the dock.

## Invariants

- Registration, MCP overlay handling, the dock and the catalogue read the
  SAME registry. Do not add an overlay to one route alone.
- Every row has ONE dock section and is listed in the catalogue.
- An unavailable overlay is refused with a reason. It is never silently
  accepted.
- The dock and the catalogue float over the 3D view. Neither takes width.
- The reach map overlay is an UPPER estimate; see
  `../../../../rs_cam_core/src/maps/CLAUDE.md`.

## Sentries

- `cargo test -p rs_cam_viz -q --test overlays_registry`
- `cargo test -p rs_cam_viz -q --test reach_overlay_p5`
- `cargo test -p rs_cam_viz -q --test the_viewport_dock_reaches_every_option_g_vpdock`

## Do not

- Do not draw an overlay from a render pipeline without a registry row.
- Do not add a `pub … : bool` to `ViewportState` for dock state; use
  `state/overlays.rs`.
