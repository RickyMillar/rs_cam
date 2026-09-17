# `maps/` — grid-walk maps and their bounded caches

One grid walk over the model bounding box, labelled per cell. The entry points
are `maps::tier_map::compute_tier_map` and `maps::reach_map::compute_reach_map`.

## Files

- `mod.rs`, `grid.rs` — the facade, the grid and the row walk.
- `tier_map.rs`, `tier_islands.rs` — the multi-tool tier label per cell, and
  the per-tier island sets.
- `reach_map.rs` — the per-tool reach map over a model surface.
- `rest_heatmap_mesh.rs` — a rest grid to a coloured quad mesh.
- `memo.rs`, `geom_cache.rs`, `tier_map_cache.rs`, `reach_map_cache.rs`,
  `finish_surface_cache.rs` — the bounded, mesh-identity memos.
- `tool_shape_key.rs` — the bit-exact identity of a cutter shape.

## Invariants

- The reach map is a top-down, rim-eroded UPPER estimate. Read the grid note
  and the measured area before you make a reach claim.
- Red at or below the discretisation floor is unresolved arithmetic, not
  proven geometry.
- `stock_to_leave` is an intentional offset. It is never a reach tolerance.
- A cache key must carry the mesh identity and the tool shape key. A stale
  key returns a map for another model.
- `reach_map_for_mesh` and the four `stats()` are `test-support` doors.

## Sentries

- `cargo test -p rs_cam_core --features test-support -q --test reach_map_p5`
- `... --features test-support -q --test reach_map_residual_p5_1`
- `... --features test-support -q --test tier_map_walk_t1`
- `... --features test-support -q --test tier_map_slope_t2`
- `cargo test -p rs_cam_core -q --test tier_islands_i1`
- `cargo test -p rs_cam_core -q --test the_test_doors_are_gated_fld0405`

## Do not

- Do not read a tier seam as a defect. The seam band swallows the coarse
  tool's slivers by design (G-OVERLAPFILL).
