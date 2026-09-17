# `ui/overlays/` — the viewport Overlays panel

One declarative registry for every viewport overlay, and the panel that
lists it. The entry point is `ui::overlays::mod`.

## Files

- `mod.rs` — the facade.
- `registry.rs` — the declarative list of every viewport overlay.
- `panel.rs` — the panel that renders the registry.

## Invariants

- Overlay registration, MCP overlay handling and the panel read the SAME
  registry. Do not add an overlay to one route alone.
- An unavailable overlay is refused with a reason. It is never silently
  accepted.
- The reach map overlay is an UPPER estimate; see
  `../../../../rs_cam_core/src/maps/CLAUDE.md`.

## Sentries

- `cargo test -p rs_cam_viz -q --test overlays_registry`
- `cargo test -p rs_cam_viz -q --test reach_overlay_p5`

## Do not

- Do not draw an overlay from a render pipeline without a registry row.
