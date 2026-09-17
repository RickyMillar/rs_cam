# `geometry/` — derived geometry

Regions, grids, distance fields, contours and the machining boundary. The
principal type is `RegionSet`; the boundary entry points are
`effective_boundary` and `clip_annotated_to_boundary_set`.

## Files

- `mod.rs` — the facade.
- `boundary.rs` — the machining boundary and `ToolContainment`.
- `region_set.rs`, `region_mask.rs` — the disjoint machining-region set and
  the mask to closed-polygon extraction.
- `grid2.rs`, `grid_field.rs` — the generic row-major grid and its scalar math.
- `marching_squares.rs`, `contour_extract.rs` — contour extraction.
- `monotone_cells.rs` — the boustrophedon decomposition of a finish region.
- `edge_distance.rs` — the indexed point-to-boundary distance field.
- `fiber.rs` — the fiber and interval types for the push cutter and waterline.
- `enriched_mesh.rs` — a triangle mesh with BREP face-group metadata.
- `nn_order.rs`, `point_runs.rs`, `arc_util.rs` — shared primitives.

## Invariants

- A setup-local boundary and a world-frame boundary are different. Keep the
  frame with the boundary; never mix the two in one comparison.
- `ToolContainment::Inside` can fall back to unclipped geometry when the
  boundary offset collapses. Inspect `boundary_clip_dropped` before you
  assert containment.

## Sentries

- `cargo test -p rs_cam_core -q --test boundary_clip_escape_f1`
- `cargo test -p rs_cam_core -q --test monotone_cell_decomposition_c2`
- `cargo test -p rs_cam_core -q --test skipped_boundary_offset_f8`
- `cargo test -p rs_cam_core -q --test nn_order_scaling_g5_g6`
- `cargo test -p rs_cam_core -q --test thin_organic_island_widths`

## Do not

- Do not copy a grid walk. Use `grid2.rs` or `maps/grid.rs`.
