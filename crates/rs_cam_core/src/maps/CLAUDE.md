# `maps/` — grid-walk maps and their bounded caches

One grid walk over the model bounding box, labelled per cell. The entry points
are `tier_map::compute_tier_map` and `reach_map::compute_reach_map`.

## Files

- `mod.rs`, `grid.rs` — the facade, the row walk, and `GridSpec`, the one
  origin/cell-size representation the three map types hold.
- `tier_map.rs`, `tier_islands.rs` — the tier label per cell and the
  per-tier island sets; `reach_map.rs` — the per-tool reach map.
- `rest_heatmap_mesh.rs` — a rest grid to a coloured quad mesh.
- `memo.rs`, `geom_cache.rs`, `tier_map_cache.rs`, `reach_map_cache.rs` and
  `finish_surface_cache.rs` — the bounded mesh-identity memos.
- `tool_shape_key.rs` — the bit-exact identity of a cutter shape.

## Invariants

- The reach map is a rim-eroded UPPER estimate. Read the grid note and the
  measured area first. Red at or below the floor is unresolved arithmetic.
- `stock_to_leave` is an intentional offset, never a reach tolerance.
- A cache key carries the mesh identity and the tool shape key; a stale key
  returns a map for another model.
- `reach_map_for_mesh`, the four `stats()` and the four caches'
  `reset_stats`/`cache_len`/`clear` are `test-support` doors; `clear()` is not
  an embedder release hook.
- A map states its grid once, as `map.grid`. `ReachMap.grid` is
  `#[serde(flatten)]`, so the five keys stay at the JSON top level.

## Sentries

- `cargo test -p rs_cam_core --features test-support -q --test reach_map_p5`
  (also the FLD-01 wire shape); the same for `reach_map_residual_p5_1`,
  `tier_map_walk_t1` and `tier_map_slope_t2`.
- No feature: `tier_islands_i1`, `the_test_doors_are_gated_fld0405`.

## Do not

- Do not read a tier seam as a defect. The seam band swallows the coarse
  tool's slivers by design (G-OVERLAPFILL).
