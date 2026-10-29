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

Landed: R5a bd1355196 (click visibility, wireframe upload, curves gate), R5b
21b1dc275 (view cube, projection toggle, Bottom/Back/Left presets, the preset
rows leave More), R5c d195a1e80 (More menu review; `MORE_MENU_REVIEW.md` §4
holds ten questions for the operator). Breaks stated in the commits: MCP
`set_ui_view overlays` refuses a row that is out of scope for the current
workspace; the More menu lists 24/28/29 rows (Setup/Toolpaths/Simulation)
instead of 37 everywhere.

## 10. Open questions for the operator's full review

1. The ten questions in `MORE_MENU_REVIEW.md` §4.
2. Ring versus face ranking under a click (R5a, above; also 961c2684f).
3. The label of the curves row ("Curves and edges"): keep or rename.
4. MCP `add_pick` of a single floor circle on the stepped block folds no
   rim; the GUI click on the same rim folds (the asymmetry is the batch
   fold door, by design; confirm).
5. Sloped-edge items (a ring on a non-planar face) have no pickable key.
6. Materials: one neutral material only; per-model colour is not built.
7. `a_dropped_mesh_releases_its_entry` is flaky (`cache_len` race).

8. **Profile: a click on the bottom face rarely gives the outline (operator,
   2026-10-30, build d195a1e80).** Probed through MCP (`add_pick` of face
   brep 1 on Profile 1 of `stepped_block.step`): the door accepts the pick
   and refuses it with "this face faces down in this setup. Fix: Pick it in
   a setup that faces it, or pick the face above it."; both rows (the face
   and the default outline loop) then read `skipped` and the op is blocked.
   In the viewport the Faces chip is on, so a click over that face takes
   the face (R5a ranking); only a click within the line tolerance of the
   edge takes the outline loop, which works. This is the ring-versus-face
   ranking question (item 2 above) with a sharper case: a face the op
   refuses should not win the click over a ring it serves. Candidate fixes:
   rank a served ring above a refused face; or drop refused faces from the
   click candidates in pick mode; or keep the default outline cut when a
   later pick is refused.

## 11. Round 6 rulings (operator, 2026-10-30, on §10 and MORE_MENU_REVIEW.md §4)

- Down-facing face in a Profile: "facing down shouldn't really be an issue."
  → a ring-use op keeps a down face and reads its outline at the face plane;
  fill-use ops keep the refusal (R6a, core).
- Q1 stock colour with the stock: "put it in stock. or just generic rendering
  space that can be shared by other things.. like tools/clamps/models" → the
  Scene section becomes per-object groups (Model, Stock, Tool, Obstacles,
  Pins); Inspect dissolves into them.
- Q2 "sounds like so? should be simple UI" → Highlight active step is a child
  check under Generator steps.
- Q3 Setup draws toolpaths: "no" → the Setup workspace draws no moves; the
  Paths rows leave More in Setup; the bar greys Cutting moves and Rapids there.
- Q4 renderer-less rows: "we used to just shade the stock with this? It was
  useful... maybe check again?" → the shading is the rest heat map (still
  there, Model colour ▸ Rest); the two rows are deleted; the derived
  rest-region islands are investigated for the Boundary outline row.
- Q5 Model colour Off: "special cases like this... should come up as more
  prominent options with an eye visibility toggle next to them. maybe next
  to the legend." → a context rail beside the legend: an eye chip per
  context row whose precondition holds now.
- Q6 Selection outline: "not in setup".
- Q7 pin ghost: "I don't know what a pin ghost is, that whole UX might need
  rework?" → context chip only while the pins do not key; one-sentence hover;
  a rework note in MORE_MENU_REVIEW.md §5.
- Q8 "Drawing states": ok. Q9 spans under Palette: yes. Q10 Collisions bar: no.

## 12. Round 7 findings (operator live look, 2026-10-30, build a8598ca32)

Evidence 19 shows the state as found: Parallel finish 2 on the STEP with a
face boundary (faces Z10 and Z30), tool 3mm End Mill 2F.

1. **The translucent solid stock hides the toolpaths.** `render/mod.rs`:
   the solid stock draws with `sim_mesh_pipeline` (alpha blend, depth WRITE
   on) before the toolpath lines (drawn after line 2007). A line behind the
   stock's front face fails the depth test and vanishes instead of showing
   through. Fix: draw the solid stock with the read-only depth state (as the
   height planes do) after the lines, so a line behind the surface tints
   through it.
2. **The face finish spills past the faces.** `get_toolpath_params`: the
   boundary is `selection` (two faces), containment `center`, offset 3.0.
   The offset is the tool DIAMETER copied by `BoundaryConfig::for_3d_op`
   for the silhouette default; `click_edit` (L-8 revised) keeps "every
   other field" when a face click turns the source into a selection, so the
   face boundary inherits the diameter offset: the tool centre runs 3 mm
   past each face edge and the drop cutter follows the walls down (the
   green strips on the walls in evidence 19). Fix (proposed): when the
   source becomes a selection, set the offset to 0 (the diameter offset is
   the silhouette's own derivation); question: containment for a face
   boundary (Center keeps the cutter one radius past the edge at an open
   side and against the wall at a closed side).
3. **A model change keeps the picks of the old model.** `set_toolpath_model`
   changes `model_id` and invalidates; the selection and boundary picks
   stay (DESIGN 9.2: a pick that names no item on the new model is
   `LostPick`), so the face boundary of the STEP refuses on the mesh.
   Question: on a model change, drop every pick that names no item on the
   new model (with a note), or keep the refusal.
4. **The reach map shows no unreachable corner on the stepped block.**
   `reach_map` for index 1: tool 0 is the 3 mm END MILL (a flat tool
   reaches a square inside corner, so 0 % is right for it). For the Ø2.3
   ball the corner gap is at most its radius 1.15 mm at the wall and 0.14 mm
   one cell (0.6 mm) out, under the grid's own resolution floor (1.25 mm,
   `discretisation_floor_mm`, from the sharp edges' curvature term) and far
   under the 5 mm max-gap colour scale, and the report already says
   `tolerance_below_floor: true`. The terrain mesh has multi-mm gaps, so
   the map lights there. Not a defect of the walk; a scale question: a
   tool-relative gap scale (gap over tool radius) and a visible floor note.

Round 7 rulings (operator, 2026-10-30):
- §12.1 solid stock: build (read-only depth after the lines).
- §12.2 face finish: "the finish can spill over but only on that flat plane
  where it can, into air"; the user still chooses containment; adjacent
  selected faces link into one region; "a generic solve not a super
  case-by-case code" → a face boundary resolves through THE open-sides rule
  (grow past open sides by the tool radius, never past a wall), the faces
  union; the inherited diameter offset resets to 0 on the face click.
- §12.3 model change: "sounds good" → drop the picks that name no item on
  the new model, with a note.
- §12.4 reach: with a Ø10 ball the Reach toggle only shows/hides the other
  mesh and colours nothing on the STEP ("this does need a look") → the reach
  overlay replaces every model's draw (render/mod.rs if/else chain); fix per
  model through the face pipeline; tool-relative gap scale.

Round 7 landed: core 2dc5e5130, render a5366bb7c, tail (see git log).
Open after round 7: the inspector undo snapshot holds no `model_id`, so an
undo after a model change can put the old picks back on the new model; an
MCP model change shows the GUI only the "Bound model" toast, not the
dropped-picks note; the Z10 and Z30 faces give two polygons (the Z20 step
between them is wider than two radii), faces that touch give one.
