# R-A design note: the part-outline item of a mesh model

Status: design only. No repo file changed. No cargo run.
Ruling R-A (PLAN.md, "Operator rulings 2026-10-23 morning"): a STEP or STL
model gives one part-outline `Loop` at the model bottom Z. The default
Profile on a mesh model cuts that outline, Outside, to the bottom.

Line references: R-B edits these files now (working tree): `selection/
{item,rule,refusal,view,mod}.rs`, `compute/catalog.rs`, `compute/catalog/
{capabilities,registry}.rs`, `compute/{floor,domain,operation_configs}.rs`,
`compute/execute/{prologue,drilling}.rs`, `io/brep_items.rs`, `session/
{heights,selection_status,compute}.rs`, `session/compute/generation.rs`,
`feeds/suggest/apply.rs`. For these files each line is from
`git show HEAD:<path>`. Paths are under `crates/rs_cam_core/src/`.

## 1. The source: exact from the triangles, not the raster

Facts (code reading, not a run):
- `cached_silhouette` (maps/geom_cache.rs:350) calls `model_silhouette`
  (geometry/boundary.rs:700) with the 0.5 mm cell (`SILHOUETTE_CELL_SIZE`,
  :677).
- The fill is conservative (`floor(xl)`..`ceil(xr)`, about :823-828) and
  the contour points are cell-edge midpoints (`edge_pos`, about :912-923).
  So the outline is biased OUTWARD by about 0.25 to 0.75 mm, as a 0.5 mm
  stepped line with 45-degree steps on each curve and diagonal wall.
- The grid anchor is `bbox.min - cell` (:704-707): an unchanged file gives
  the same cells, so the key does not jitter on a re-import.
- The loop start is random: each loop starts from `adj.keys()` of a
  `HashMap` (about :929). The key (area centroid, `sqrt(area)`) does not
  change, but a Profile starts at the first vertex, so the G-code changes
  from run to run.

The raster is correct for its use today, a containment clip
(session/compute/generation.rs:759), where an outward bias never cuts the
part. A Profile follows the line: the part comes out oversize by up to
0.75 mm with a stepped wall.

Decision: build the outline EXACT from the mesh triangles. One rule for
STEP (the tessellated `EnrichedMesh::mesh`) and STL:
1. Take each triangle with a non-zero projected XY area. Group by the sign
   of that area (up, down). A vertical triangle gives nothing.
2. Split each group into edge-connected patches. The STL door welds the
   vertices (mesh.rs:48); `check_winding` (mesh.rs:153) shows the edge map.
3. Chain the boundary edges of each patch into rings; project them.
4. `Polygon2::union_all` (polygon.rs:433) of the patch polygons. It also
   closes the T-junction seams between STEP faces, as in
   `flat_region_items` (selection/flat_regions.rs:180, the precedent).
5. `silhouette_machining_outline` (boundary.rs:757) of the union: the
   outer ring of the largest exterior, no holes (R1, one rule with the
   boundary).
6. Canonical ring: counter-clockwise, start at the lowest Y, then X.

Up AND down: a closed solid gives one shadow from both, a bad STL winding
splits the sign groups but the union still covers the shadow, and a
terrain sheet (rivmap) has up triangles only. The result is exact to the
tessellation for every shape. A union of the planar BREP faces only is NOT
exact (a part whose extent is a curved face): do not use it.

Gate before landing: measure the build on `planning/fixtures/` rivmap100
and on `stepped_block.step` against the import time. If it is too slow at
import, use the P4.7 memo pattern (`geom_cache`, `Peek`/`Build`). Fallback
arm, only if the exact build fails a gate: the raster +
`silhouette_machining_outline` + `simplify_bounded` (polygon.rs:1120, no
production caller yet), with the outward bias stated in the item doc.

Re-import: the default rule stores no key, so a default Profile survives
every re-import. A stored pick holds the key (`KEY_MATCH_EPS_MM = 1e-6`,
selection/key.rs:24). An unchanged file gives the same key; a changed part
moves it, and the pick is lost and flagged as for every item (key.rs:1-5).
`CAD_ROUNDING_MM` (key.rs:38) is a size rule and does not apply.

## 2. The item

- `Item::at_z(SourceRef::ToolpathModel, None, Shape::Loop(ring),
  mesh.bbox.min.z)`, kind `Loop`, no layer. `bind_items`
  (session/mod.rs:921) binds it to `SourceRef::Model(id)`.
- Marker: a private field `outline: Option<PartOutline>` on `Item` (HEAD
  item.rs:254), beside `edge` (and R-B's new `hole`), not in the key.
  `PartOutline { top_z }` feeds the frame door (3e). Doors:
  `with_part_outline`, `is_part_outline`. `keep_edge` (HEAD item.rs:492)
  copies it, so the frame door and `bound_to` keep it.
- Do not reuse `ModelEdge { ring: true }`: the R-C test counts rings
  (`a_step_chamfer_cuts_convex_edges_only_rc` :294, `== 3`).
- Place: one post-pass in `io::model_from_geometry` (io/mod.rs:202), after
  the match (:216-232), when `model.mesh` is `Some`. Append it LAST, so
  each STEP item keeps its index. The project door and `adopt_geometry`
  (session/mod.rs:773) take their items from this function. The pure
  builder goes in a new `selection/outline.rs`. A mesh with no area gives
  no item and a log line.
- `is_plane_drawing` is false (z is `Some`): `closed_regions`,
  `has_closed_ring`, stock changes and obstacles do not read it.

## 3. SAFETY: no area default takes the outline

Profile Outside and Pocket share `ItemUse::Regions` (registry.rs:148,
:201; catalog.rs:946 maps only Profile `On` to `Curves`), so
`served_by_default(item, item_use)` (HEAD rule.rs:214, caller :173) cannot
separate them. A new `ItemUse` variant is wrong (277 `ItemUse::` sites).

a) New `OutlineUse { Default, PickOnly, Refuse }` and a field
   `outline_use` on `Capabilities` beside `item_use` (HEAD
   capabilities.rs:159). Each registry row states it (no `Default`):
   - `Default`: Profile, side Outside only (the Profile arm, catalog.rs:946,
     sets Inside and On to `Refuse`).
   - `PickOnly`: Trace, Project Curve, Face, the Selection boundary (a
     `BOUNDARY_OUTLINE_USE` beside `BOUNDARY_ITEM_USE`, HEAD
     selection_status.rs:128), the `ItemUse::None` rows.
   - `Refuse`: Pocket, Adaptive, Zigzag, VCarve, Inlay pocket, Inlay plug,
     Chamfer, Drill, Bore, Pin Drill.
b) `ResolveScope` (selection/resolve.rs:34) gets `outline_use` beside
   `item_use` (:59). Build sites: HEAD selection_status.rs:451,
   selection/tests.rs:112. `ResolvedSelection` carries it to `view`, as it
   carries `mirrored` and `upright`.
c) The rule, at the start of `served_by_default`:
   ```rust
   if item.is_part_outline() {
       return outline_use == OutlineUse::Default;
   }
   if outline_use == OutlineUse::Default && item.z().is_some() {
       return false; // R-A: the default mesh Profile cuts the outline only
   }
   // the HEAD body, unchanged
   ```
   Line 2 is a lead decision ("cuts that outline"). Without it the default
   Profile on stepped_block cuts the outline AND the 3 face rings. A
   drawing item (z `None`) does not change: a DXF Profile keeps every loop.
d) A named pick: `item_serves` (HEAD view.rs:745) refuses the outline
   under `Refuse` with a new `RefusalCause::PartOutlineIsThePart { op }`:
   "The part outline is the whole part. A <op> inside it cuts the part
   away. Use a Profile Outside on the outline." Chamfer refuses it today as
   `NotATopEdge` (a z item with no `ModelEdge`); Drill and Bore refuse
   `KindNotCut`. The new cause is for the `Regions`/`Curves` ops.
e) Frame door: `to_setup_frame` (resolve.rs:448) moves the Z by
   `item_z_in_setup_frame` (:556). On a flipped setup the model bottom is
   the setup TOP, and the floor would refuse `AboveTop`. Rule: for an
   outline under `info.is_z_flipped()` the door maps `PartOutline::top_z`,
   so the Z is the setup bottom of the part. A lateral setup keeps the
   work-plane rule of every model edge item (compute/transform.rs:149).
f) A default Pocket on stepped_block does not take the outline: the row
   stays "Default: every closed loop (3)".

## 4. Profile defaults on a mesh model

- Side: Outside is the default (HEAD operation_configs.rs:886).
- Depth: no new floor kind. `AddToolpath` runs `each_face_floor_due` (HEAD
  session/heights.rs:671): area op, default floor, and
  `has_face_below_stock_top` (:224) on the lowest served Z ring. The
  outline is that ring, so the op gets `Floor::EachFace { depth: 0 }`, and
  the outline region floor is its Z: the part bottom (setup frame, 3e).
- O-2: the auto stock pads the top only, `origin_z = bbox.min.z`
  (compute/stock_config.rs:287-290). So the floor is the stock bottom, a
  cut-out; `DEPTH_BEYOND_STOCK_EPS_MM` (diagnostics/adapters/
  from_static_checks.rs:559-562) calls that "the normal way to cut a part
  out". A manual stock with its bottom above the part bottom would put the
  floor below the stock. Rule: the each-face floor of an outline region is
  `max(outline Z, stock bottom Z)` in `compute::floor::region_floors`, with
  a note "the outline floor stops at the stock bottom". A deeper cut needs
  the operator.
- Tabs: the default stays `tab_count: 0` (HEAD operation_configs.rs:893).
  Recommend no default tabs: the session does not know the workholding, a
  tab that the operator did not place is a guess, and a tab bridge on a
  mesh wall is material that the finish ops did not plan for.
  WARNING for the lead: on the auto stock this default cuts the part free
  on the last level. Recommend a `SelectionNote` when the outline floor is
  at the stock bottom: "This cut frees the part. Add tabs or hold the
  part." The alternative, `tab_count: 2` on this default only, needs a
  ruling.

## 5. STL: no clash with the flat-region faces

- Kinds differ (outline `Loop`, flat region `Face`), so no key matches;
  `KindFilter::Served` admits no `Face`.
- The real clash: `model_items` (resolve.rs:605-613) REPLACES
  `LoadedModel::items` with the flat faces when the selection names a
  face, so an outline pick plus a face pick loses the outline. Fix:
  `bind_faces` (session/mod.rs:872) gives `items ++ faces`, outline first,
  so its own index stays 0 and `item_states` stays aligned.
- `list_model_items_near` (HEAD selection_status.rs:1444), `kind: Face`:
  faces only, no change.

## 6. GUI and MCP

- Viewport: the outline passes `has_model_edges` (HEAD item.rs:615), so
  the gates at rs_cam_viz `app/gpu_upload/drawing.rs:145`,
  `state/viewport.rs:416`, `state/selection_highlight.rs:74` pass. A z
  item draws at its own Z, and only when the selection holds it or it
  flashes (drawing.rs:371-378). So the outline draws at the bottom Z under
  a default Profile, and not under a Pocket.
- Row: `row_label`/`row_full_label` (viz ui/properties/
  selection_section.rs:393, :425) read the key only. Add `outline: bool`
  to `SelectedRow` (HEAD selection_status.rs:338). Row "Part outline Z0",
  hover "Part outline at (x, y) Z 0.00". Title (selection_section.rs:
  478-483): "Default: the part outline" when that is the only row.
- MCP: `model_item_json` (viz app/mcp/commands/selection.rs:151) adds
  `"role": "part_outline"`; core `ModelItem` and the `selection_status`
  rows carry the same flag. One flag in core; GUI and MCP read it.
- `inspect_model` and the model panel count one more closed loop (viz
  app/mcp/project.rs:432, ui/properties/model_sim_panels.rs:273).

## 7. Saved projects and breaks (state each in the commit)

- Items are not saved; the load door builds them. An old project gets the
  outline on load. No format change.
- A saved default Profile on a STEP model (it holds its default rule) now
  cuts the outline only, not the face rings. Its toolpath changes.
- A saved default Profile on an STL refused before (no item); it now
  cuts, with the floor stored in its file.
- An STL now passes `has_selectable_item` (HEAD item.rs:626): the Add menu
  offers the 2D ops, the Geometry check passes, the "Selection" boundary
  button shows. A default Pocket on an STL refuses "nothing that Pocket
  cuts".
- `inspect_model.items.total` grows by one on each mesh model.

## 8. Tests

New, core (`--features step` for STEP):
- `a_mesh_model_gives_one_part_outline_ra`: stepped_block.step and an STL:
  one outline, Loop, z = bbox min z, `Model(id)`, last index, canonical
  start, area = the projected area within the tessellation.
- `the_part_outline_survives_a_reload_ra`: equal keys and vertex order.
- `no_area_default_takes_the_part_outline_ra`: every registry op on
  stepped_block and on an STL; only Profile Outside serves it by default;
  Pocket keeps 3 default loops.
- `a_part_outline_pick_refuses_on_an_area_op_ra`: Pocket, Adaptive,
  Zigzag, VCarve, Inlay x2, Profile Inside/On give `PartOutlineIsThePart`;
  Chamfer `NotATopEdge`; Drill `KindNotCut`.
- `the_default_mesh_profile_cuts_the_outline_to_the_bottom_ra`: floor
  `EachFace{0}`, deepest Z = stock bottom, each move outside the outline
  by the tool radius, the free-part note.
- `the_outline_floor_stops_at_the_stock_bottom_ra` (O-2),
  `a_flipped_setup_puts_the_outline_at_its_bottom_ra`,
  `an_stl_face_pick_keeps_the_outline_ra`,
  `the_part_outline_is_not_a_drawing_region_ra`.
New, viz: `the_part_outline_row_reads_part_outline_ra` (row, title, MCP
`role`, one count on both surfaces).

Existing pins:
- `default_selection_equals_the_old_lists_p2_2` (frozen):
  `step_ring_differences` (:303-319) takes each model with a z Loop, so
  each STL enters the expected list but does not differ. Filter out
  `is_part_outline()` there. The golden loop (:450) skips an empty set;
  STLs now enter it. Check, then pin.
- `a_step_model_gives_edges_circles_and_points_ux7`: `find_sized` (:141)
  and the one-match check (:236) hold (the 4 bottom chains are `Path`s,
  rc :250-255). `keys_survive_a_reload_ux7` (:240) zips by index: it holds
  with the canonical start.
- `every_import_door_gives_items_p2_1`: the STEP block test (:294) counts
  faces only; add an outline count.
- `stl_flat_regions_are_face_items_p4_7`: :205 and :329 count faces;
  check that they read faces only.
- Must stay green: `a_step_chamfer_cuts_convex_edges_only_rc`,
  `a_default_step_pocket_never_cuts_below_a_ring_ux7b`, viz
  `the_geometry_rows_fit_the_inspector_f22` (:570, a DXF).

## 9. File set and overlap with R-B (`*` = R-B edits it now)

Core: `selection/outline.rs` (new), `selection/item.rs`*, `rule.rs`*,
`view.rs`*, `refusal.rs`*, `mod.rs`*, `resolve.rs`, `selection/CLAUDE.md`;
`compute/catalog/capabilities.rs`*, `registry.rs`*, `compute/catalog.rs`*,
`compute/floor.rs`* (O-2 clamp); `io/mod.rs` (`model_from_geometry`, the
STEP door file); `session/mod.rs` (`bind_faces`);
`session/selection_status.rs`* (scope, `SelectedRow`, `ModelItem`).
Viz: `ui/properties/selection_section.rs`, `app/mcp/commands/
selection.rs`. MCP crate: none (no param type changes).
Same code as R-B: the `Item` fields and `keep_edge` (R-B adds `hole`);
`served_by_default`, `item_serves`; `RefusalCause`; the Profile/Drill arms
of catalog.rs; floor.rs; the scope build in selection_status.rs.
R-A does not edit `session/heights.rs` (reused), `io/brep_items.rs` or the
Drill op. Sequence: start R-A after R-B lands.
