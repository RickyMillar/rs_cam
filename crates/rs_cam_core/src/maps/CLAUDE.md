# `maps/` — grid-walk maps and their bounded caches

One grid walk over the model bounding box, labelled per cell. The entry points
are `maps::tier_map::compute_tier_map` and `maps::reach_map::compute_reach_map`.

## Files

- `mod.rs`, `grid.rs` — the facade, the grid and the row walk.
- `tier_map.rs`, `tier_islands.rs` — the multi-tool tier label per cell, and
  the per-tier island sets. `tier_flute_reach.rs` — the F3c advisory: owned
  cells the tool reaches only with the body above its flutes.
- `reach_map.rs` — the per-tool reach map. `rest_heatmap_mesh.rs` — a map to
  a mesh. `tool_shape_key.rs` — the bit-exact cutter shape identity.
- `memo.rs`, `geom_cache.rs`, `tier_map_cache.rs`, `reach_map_cache.rs`,
  `finish_surface_cache.rs` — the bounded, mesh-identity memos.

## Invariants

- The reach map is a top-down, rim-eroded UPPER estimate. Red at or below
  the discretisation floor is unresolved arithmetic, not proven geometry.
- `stock_to_leave` is an intentional offset. It is never a reach tolerance.
- A cache key holds the mesh identity and the tool shape key (GUI: `ReachRequestKey`).
- `reach_map_for_mesh`, `stats`, `reset_stats`, `cache_len` and `clear` are
  `test-support` doors, not product API.
- A map states its grid once, as `map.grid` (`ReachMap.grid` is flattened).
- The flute-reach pass calls `stock::collision::body_segment_penetration_mm`.
  It reports; it never moves a cell out of a tier.

## Sentries

- With `--features test-support`: `reach_map_p5`, `reach_map_residual_p5_1`,
  `tier_map_walk_t1`, `tier_map_slope_t2`, `tier_map_cache_t3`.
- Without it: `tier_islands_i1`, `tier_islands_speck_weld`,
  `tier_map_flute_reach`, `the_test_doors_are_gated_fld0405`.

## Do not

- Do not read a tier seam as a defect. The band swallows the coarse tool's
  slivers by design (G-OVERLAPFILL); its width is plan F2, still open.
- Do not raise the raise bound (default 0, ceiling 3): it welds specks (F1).
