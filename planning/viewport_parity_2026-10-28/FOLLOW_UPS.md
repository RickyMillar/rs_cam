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

## 3. Small items left open by the build

- B §3.4 hole-rim folding of a batch: `AddPicks` does not run `hole_rim_key`
  per key, so a box over a Drill hole sends the rim circle and the floor
  circle as two keys. Fold them in core (`Selection::apply(AddPicks)`), so
  MCP gives the same result; one test on a Drill fixture.
- The pick-mode right-click menu (E5), the box (B6), the eased presets and
  the face edges have not been seen on screen. Take screenshot pairs on a
  quiet GUI (`planning/viewport_parity_2026-10-28/evidence/`).
- B3 (the `MouseScheme` setting, right-drag orbit default) waits for the
  operator's live look (A-Q9, B-O2). D6 (ID pass, outline) and the "select
  other" cycle are phase 4.

## 4. Operator rulings of 2026-10-29 (after phases 0-3)

- **Scheme (A-Q9, B-O2): the desktop-CAD scheme is the default** (middle drag
  orbits, Ctrl + middle pans, right drag pans; left selects). B3 lands it.
  Laptops with no middle button get fallback rows in BOTH schemes:
  Alt + left drag orbits (Blender's three-button emulation), Alt + right
  drag pans, and a two-finger scroll zooms as today; the orientation gizmo
  gains a drag-to-orbit later. The shortcuts window prints every row.
- **Edges**: the operator finds the 1 px edges thin against old builds.
  Plan the quad-expanded line pipeline (D §3.5) for edges, the hover ring
  and the tab marks together, with a width token; take the before/after
  pair on `stepped_block.step`.
- **Edge picks**: STEP edge items exist only for edges in a horizontal
  plane (the importer skips vertical and sloped edges: "edge_items=35,
  skipped_edges=8" on the stepped block). The operator has not seen one
  picked; a demo screenshot pair is owed (a ring pick in pick mode with
  the `path`/`loop` chip). Sloped edges are a later import extension.
- **Hole folding (B §3.4)**: the operator asked what other tools do.
  Fusion and Onshape treat one hole as one feature whatever you click (the
  cylinder face, its top edge or the floor): a selection never counts a
  hole twice. Fold in core.
- **Phase 4 order (operator: "whatever makes sense", not now)**: display
  modes + the quad line pipeline (fixes the thin edges and removes dock
  rows) → the ID pass outline → "select other" → materials and lighting →
  the dock declutter.
- The `curves` overlay row says "this project carries no 2D curves" for a
  STEP-only project, but `viewport.show_polygons` now also gates the STEP
  highlight and the hover ring there (the draw-gate fix of 2026-10-29).
  Reword the precondition or split the row.

## 5. Live look round 1 (operator, 2026-10-29, build ec419f648)

Works: orbit about the point, pan, zoom to the cursor, presets, F, undo per
click, the ring pick (after the draw-gate fix), the right-click menu, the
hand cursor. Fixes ordered from the look:
- The box was invisible: `paint_box` ran before the 3D paint callback, so
  the scene composited over it (selection still happened on release).
- The pick-mode hover note is one line, not a paragraph.
- Inverse hover: in pick mode only a pickable face lights; a vertical side
  face gets no tint.
- **Ruling:** Ctrl+A always selects every item of the chip kinds, also on
  the default selection (no "default already selects" notice); MCP parity.
- **Ruling:** the selected ring is too thin and too low in contrast on a
  white part: the quad line pipeline (D §3.5) is pulled forward from phase
  4 with width tokens, and the selected colour tries the green token.
- Flaky core lib test under a parallel run: `compute::obstacle::tests::
  a_mesh_obstacle_resolves_to_the_same_outline_twice_k2` failed once inside
  `cargo test -p rs_cam_core --lib mesh` (2026-10-29) with the same ring
  rotated to another start vertex, and passed 3 of 3 alone. The shared
  silhouette memo (`geom_cache`, capacity 4) is evicted by sibling tests, and
  a rebuilt silhouette starts at another vertex: make the silhouette builder
  deterministic (no HashMap order) or key the test on a private cache.

## 6. Live look round 2 (operator, 2026-10-29, build 76fb3a2f4)

"Mostly looking good." Rulings and findings:
- **Ruling:** selected faces blue, selected lines green (they merged in one
  green; evidence 06). Selected lines twice as thick (5 px).
- Shaded and Shaded with edges looked the same: the INK_65 edges at 1.25 px
  are nearly invisible on the pale faces (evidence 08/09). Edges become the
  darkest ink at 1.5 px.
- A selected face seen at a grazing angle turned ink grey-blue (evidence
  10): the outline's fold test compared adjacent depths, which a planar face
  at a low angle exceeds on every pixel. Fix: a planarity (second
  difference) test.
- The operator could not tell whether the selection outline works: with the
  fold fix, it is the accent line just outside a selected face's silhouette.

## 7. Live look round 3 (operator, 2026-10-29, build 712c7bf24)

"Nice. Few small details."
- **Ruling:** the green selected line replaces the dark edge; no overlap of
  the two lines (evidence 11 shows both).
- **Ruling:** the height planes show only while the Heights tab is open, by
  default; the overlay row switches them on for good.
- **Ruling:** the shading looks cartoony (per-face pastels + a flat
  hemisphere). One neutral material and a camera headlight for the diffuse,
  a modest ambient; the per-face palette goes; the edges delineate faces.
