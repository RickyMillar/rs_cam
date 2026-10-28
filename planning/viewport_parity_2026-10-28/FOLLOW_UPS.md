# Viewport parity: follow-ups for a second pass (operator, 2026-10-29)

Not in this session. Start here after the DESIGN.md phases land and the
lead has had one live look.

## 1. Display modes as in Fusion and Blender

One three-way control for the model look: **Wireframe**, **Shaded**,
**Shaded with edges**. Today the look is spread over overlay rows
(`model`, the face edges row that D4 adds, the colour choices).

- Shaded with edges = the D4 face-edge slot on. Shaded = the slot off.
- Wireframe = the triangle edges of the mesh. wgpu draws lines only with
  `PolygonMode::Line` behind the `POLYGON_MODE_LINE` feature, which the egui
  device descriptor does not request; the portable path is a line buffer of
  the unique mesh edges (`EnrichedMesh.edges` holds them unordered, which is
  enough for a wireframe) drawn with the existing line pipeline. Measure the
  line count on the terrain before choosing.
- Fusion keeps the mode in the display bar at the bottom of the canvas;
  Blender in the header shading pie. Keep it to one segmented control, no
  text label when the icons read.
- MCP: one `set_ui_view` field `display_mode` with the three tokens, and
  `get_ui_view` reports it. The overlay rows it replaces leave the catalogue
  (no legacy alias, ruling 2026-09-16).

## 2. The view / scene / paths selector is cluttered

The operator's words: "Our current view/scene/paths selector is very
over-done and cluttered we should tidy that up on a second pass". Scope for
that pass:

- Count the rows the catalogue shows per workspace and which ones a user
  touches in a session (an instrument first, as the overlays audit did).
- Fold the model-look rows into the display mode above.
- Keep the one-click rows (grid, stock, rapids, collisions) on the bar;
  move the rare rows behind one "More" popover.
- The wizard rule applies: no clutter in main panels (operator 2026-10-22).
