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

## 3. Open items (kept current; 2026-10-29 evening)

- ~~Hole-rim folding of a batch~~: done b5b903961 (in the door; a single
  named floor-circle pick still refuses, R-B claim 4).
- ~~Laptop trackpads: the gizmo drag-to-orbit~~: done af5ea02a7.
- STEP edge items exist only for edges in a horizontal plane; a sloped or
  vertical edge gives no item (an import extension).
- ~~The `curves` row precondition text~~ and ~~the silhouette ring start~~: done
  24aa5bf8f (the label "Curves (DXF/SVG)" stays; a rename is the operator's).
- Still flaky: `maps::geom_cache::tests::a_dropped_mesh_releases_its_entry`
  asserts `cache_len() == 1` while sibling tests write the shared cache
  without `cache_test_lock`; assert only that the dropped entry is gone, or
  take the lock in every writer test.
- The dock declutter: approved by the operator 2026-10-29 ("declutter
  sounds good"); built from `DOCK_DECLUTTER.md` with the obvious answers to
  its four questions (Collisions on the bar everywhere, a three-way Stock
  control, Entry markers in More, More only).
- ~~STL face keys share a footprint~~: done 6fd51227a (an STL face key carries
  its plane Z; stored picks without z show as lost).
- Materials and a lighting ruling beyond the headlight (a material table
  per model, imported STEP colours) are a later pass.

Done since the list was first written: every phase of DESIGN.md, B3, D6,
"select other", the screenshot pairs (evidence 01-12), three live-look fix
rounds.

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
- **Ruling (live look, 2026-10-29 evening):** the box selects visible items
  only. From the top, a box over the three step faces must not take the
  base face underneath (design B §3.3 said the CPU test selects through).
  Built as a ray visibility test through `LoadedModel::ray_pick` (the BVH),
  not the GPU ID pass; the click and Tab keep reaching hidden items.

## 8. Round 4 (operator, 2026-10-29 evening, away): "finish off the rest"

Operator: "its feeling fantastic. finish off the rest and then I can look
at doing a full review." Two UX rulings, left to the lead's judgement:
- **Pick mode engages with the toolpath.** "When I select something like
  profile mode, the picker should already be engaged. Either I should have
  no selection or all the possible ones for that toolpath." Decision: a
  focused 2.5D op in Toolpaths is in pick mode at once (its op target; a 3D
  op with a Selection boundary on its boundary target); the chips start with
  every kind the op serves; Escape pauses, Pick resumes; a click on nothing
  changes nothing; a click on another toolpath's path moves the focus and
  the pick mode to it.
- **Hover and select feedback follow the op's use of the item.** A Profile
  cuts a contour, so a face under the pointer lights as its ring (the edge),
  and a picked face shows as its green ring; an area op (Pocket, Face,
  Zigzag, Adaptive 2D) lights the fill. The rule is core's item use
  (`target_item_use`), not the op name.

The rest of the queue for the lead: the hole-rim fold, the STL face key
plane Z, the gizmo drag-to-orbit, the curves row text, the flaky
silhouette test. Sloped-edge items and materials stay for a later ruling.
- ~~The picked STL face ring under a Ring op~~: done 6fd51227a (no pixel test;
  the drawing upload needs a GPU).

## 9. Live look round 5 (operator, 2026-10-29 night, build 24aa5bf8f)

"The bottom nav bar is good." Findings and rulings:
- **Ruling:** "only select what is visible" is generic for selection, not
  only the box: a click on the middle step face in a Profile picked the
  base face under it. The click takes the nearest VISIBLE candidate; Tab
  still reaches hidden ones.
- Bug: Wireframe rendered nothing until a click (the mode change raised no
  upload, so the wireframe slot built on the next unrelated upload).
- Bug: hiding the curves row hid the selected edges (the draw-gate fix of
  ec419f648 put the selection and hover rings behind `show_polygons`).
- **Ruling:** the gizmo becomes a larger view cube whose faces click to the
  six views (Top, Bottom, Front, Back, Left, Right), the preset rows leave
  More, and an ortho/perspective toggle sits in that corner.
- **Ruling:** review the whole More menu: what is valid when, and how the UI
  could be polished. Fix the clear findings; list the questionable ones.
- Questionable (R5a): with the Faces chip on, a click over a pickable face
  always takes the face; a STEP edge ring on that face is reachable only
  with Tab or with the chip off. Alternative: rank a visible ring within a
  few pixels of the pointer above the face. Operator's call.
