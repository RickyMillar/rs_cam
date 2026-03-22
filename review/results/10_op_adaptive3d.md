# Review: Adaptive 3D (Rough)

## Summary

Adaptive 3D is a 3556-line implementation of constant-engagement roughing on mesh surfaces, extending the 2D adaptive algorithm to 3D via heightmap-based material tracking and surface-following. The recent merge (2c7a011) added rich semantic tracing with hotspot attribution. The code is well-tested (31 tests), properly avoids unwrap() in library paths, and maintains clear layering between shared math (adaptive_shared.rs) and 2D/3D-specific implementations.

## Findings

### Algorithm Design

- **Heightmap vs Boolean Grid**: 3D uses f64 heightmap for material (not u8 grid). This naturally represents partial depth removal and surface-following, but doubles memory vs 2D boolean grid.
- **Surface Model**: Precomputes SurfaceHeightmap via rayon parallel drop-cutter queries, then uses O(1) lookups via `SurfaceHeightmap::surface_z_at_world()` in direction search. This avoids per-step drop-cutter overhead.
- **Engagement Computation** (adaptive3d.rs:259-318): Counts cells where `material_z > max(surface_z + stock_to_leave, z_level) + 0.01`. Correctly handles both surface-relative and absolute floor definitions. Clamps grid bounds defensively.
- **Direction Search** (adaptive3d.rs:392-603): Three-phase search (narrow -> coarse 360deg -> fallback). Phase 1 uses 7 candidates near prev_angle with bracket refinement. Phase 2 uses 18 coarse candidates. Both phases track engagement brackets and use `refine_angle_bracket()` from adaptive_shared.rs for O(log) interpolation — clean code reuse.
- **Entry Point Finding** (adaptive3d.rs:615-790): Growing-radius O(local) search when bbox is None; full scan when region-constrained. Returns (entry_xy, entry_z) from drop-cutter. The spreading logic (adaptive3d.rs:746) skips endpoints if `pass_endpoints.len() < 50`, which can be expensive if many endpoints accumulate.
- **Region Ordering** (adaptive3d.rs:52-58, 1918-2040): Two strategies — Global (per-Z-level) and ByArea (flood-fill connected regions). ByArea reduces tool travel but adds overhead of region detection.
- **Fine Stepdown** (adaptive3d.rs:1837-1868): Inserts intermediate Z levels between major levels for heavy engagement areas. Correctly deduplicates levels within 0.01mm tolerance.
- **Flat Area Detection** (adaptive3d.rs:1796-1834): Histogram of surface Z values; inserts levels at shelf peaks (>2% of cells). Prevents isolated material pockets.
- **Pre-stamp Thin Bands** (adaptive3d.rs:949-997): Only stamps cells on steep walls (>60deg) with thin bands (<30% depth_per_pass). Avoids unproductive contour re-tracing on shallow areas. Uses slope_map angle thresholds — good separation of concerns.
- **Loop Detection** (adaptive3d.rs:1297-1368): Detects when adaptive spiral returns within `tool_radius * 1.5` of entry after travelling >4x tool radius away. Minimum threshold prevents false positives on tight spirals.
- **Idle & Low-Yield Bail** (adaptive3d.rs:1382-1483): Exits pass if engagement drops below 0.05 after 20 idle steps. Low-yield detection (yield_ratio < 0.05) bails if pass removes <5% expected material. Prevents unproductive thin-wall contour tracing.
- **Path Widening** (adaptive3d.rs:1502-1537): Stamps double ring at 1x and 2x stepover after loop-close or long passes to cover adjacent parallel contours. Uses normal offsets from path segments.
- **Waterline Cleanup** (adaptive3d.rs:1572-1679): Traces contours at bottom Z but filters steep-only (>30deg slope). Avoids re-tracing shallow areas cleared by adaptive spiral. Samples 10 points per contour to decide slope threshold.

### Performance

- **Surface Heightmap**: O(cells) parallel drop-cutter on first call, then O(1) per lookup. Total grid is `(bbox / cell_size)^2`, typically 4000-40,000 cells for small parts. Cell size defaults to `tool_radius / 6.0`.
- **Direction Search**: Phase 1 = 7 evals, Phase 2 = 18 evals, plus interpolation. Capped at ~30 evals per step. Step loops up to 5000 iterations per pass, passes up to 500 per Z level.
- **Memory**: Material heightmap + surface heightmap + slope map + region labels for ByArea. ~3 f64 per cell minimum. For 10k cells, ~240 KB + overhead.
- **Engagement Computation**: O(tool_radius^2 / cell_size^2) cells per evaluation (tool circle size). Typical 10-50 cells per eval.
- **Local Material Sum** (adaptive3d.rs:101-121): O(local_cells) idle detection instead of summing entire grid. Critical for performance on large grids.
- **Region Flood Fill** (adaptive3d.rs:141-249): 8-connected BFS, O(cells). Occurs once at start if ByArea mode.

### Shared Code (2D/3D Split)

**adaptive_shared.rs (149 lines):**
- `target_engagement_fraction()`: Compute target engagement angle from stepover and radius (2D/3D both use)
- `average_angles()`: Circular mean of angle buffer (shared smoothing)
- `angle_diff()`: Normalize angle difference to [-pi, pi] (shared)
- `refine_angle_bracket()`: Binary search on engagement bracket (shared interpolation kernel)
- `blend_corners()`: Arc blending of 2D paths (shared geometry)

**2D/3D Differences:**
- Material model: Boolean grid (2D) vs f64 heightmap (3D)
- Z model: Constant per level (2D) vs variable per position (3D)
- Engagement: Binary (material/cleared) vs continuous height-based
- Boundary cleanup: Polygon offset contours (2D) vs waterline contours (3D)
- Entry/navigation: Find next uncut cell vs growing-radius entry search

The split is **clean** — no leaky abstractions. Core direction-search logic (angle bracketing, phase strategy) is nearly identical; shared utilities handle it. Engagement computation is reimplemented per model type, which is appropriate.

### Semantic Trace Attribution

Recent commit 2c7a011 added:
- **Debug spans**: `surface_heightmap`, `region_detect`, `z_level`, `adaptive_pass`, `entry_search`, `preflight`, `widen_band`, `waterline_cleanup` with counters (rows, cols, passes, evaluations)
- **Hotspots**: `adaptive3d_pass` records pass center (x, y), z_level, tool_radius x 2.0, tolerance as search_radius, elapsed_us, pass count, step count, and is_low_yield flag
- **Annotations**: (move_index, label) pairs for simulation display. Parsed in GUI worker/execute.rs via `parse_adaptive3d_runtime_label()` (execute.rs:1210-1313)
- **Runtime Parsing**: Region start/end, Z level, pass entry, preflight skip, pass summary (steps, exit reason, yield ratio) extracted from labels for semantic annotation tree

Code is well-instrumented but dense. ~1949 lines in execute.rs to handle parsing — indicates semantic parsing is becoming complex and could benefit from being split into a dedicated module.

### Edge Cases

1. **Deep Pockets**: Fine stepdown handles steep pocket walls by adding intermediate levels. Pre-stamp thin bands prevents unproductive contour re-tracing on steep walls. However, if pocket is very narrow (< tool diameter), adaptive spiral may loop-close prematurely and not fully clear depth.
2. **Thin Walls**: Low-yield bail detects thin-wall contour re-tracing. However, if wall is just above thin_threshold (0.9mm for 3mm depth_per_pass), it won't be pre-stamped but may waste many idle steps.
3. **Steep vs Shallow**: Slope-aware pre-stamp and waterline cleanup separate steep (>60deg/30deg) from shallow areas, avoiding redundancy. However, slope computation (finite differences on heightmap) can be noisy near sharp transitions; slight overstamping possible.
4. **Mesh Holes/Islands**: Border clearing (adaptive3d.rs:1749-1782) marks cells outside mesh bbox as cleared, preventing phantom deep material. Good defensive design.
5. **Very Small Stepover**: If stepover << cell_size, engagement computation may undersample (few cells in tool circle). Defaults to cell_size = tool_radius / 6.0, which usually keeps 100+ cells per tool radius, so acceptable.

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Medium | `pre_stamp_thin_bands()` only runs on steep walls (>60deg). Shallow walls with thin bands are left for adaptive spiral to clear, but low-yield detection may bail before clearing them. | adaptive3d.rs:958-997 |
| 2 | Low | `is_clear_path_3d()` has three unused parameters (`_surface_hm`, `_z_level`, `_stock_to_leave`). Ignores surface for collision check (samples only material_hm). Should document why or refactor. | adaptive3d.rs:796-832 |
| 3 | Low | Debug context cloned once per Z level (line 1899). If ToolpathDebugContext grows, this could be expensive. Consider passing reference or Arc. | adaptive3d.rs:1899 |
| 4 | Low | `spread_endpoints` logic (line 746: skip if `pass_endpoints.len() < 50`) creates a cliff — first 50 passes spread aggressively, then stop. Consider smoother decay. | adaptive3d.rs:746 |
| 5 | Low | `refine_angle_bracket()` performs up to 1 iteration in Phase 2 (line 591). Might miss tight brackets; consider 2-3 iterations for higher precision. | adaptive3d.rs:591 |

## Test Gaps

- **Fine stepdown tests (0)**: No dedicated tests for intermediate level insertion or histogram shelf detection and dedup
- **Flat area detection tests (0)**: No tests for flat-level insertion
- **Region ordering integration (minimal)**: Only smoke tests, no coverage of actual region ordering semantics or travel reduction
- **Traced annotation content (0)**: Smoke test confirms annotations are emitted but no content validation

## Suggestions

1. **Refine low-yield detection on shallow walls**: Shallow walls with thin bands need clearing but may be skipped by low-yield bail. Consider lowering steep_threshold for pre-stamp, adding a separate shallow-band pass, or adjusting yield_ratio based on local slope.
2. **Clean up `is_clear_path_3d()` signature**: Remove unused parameters or document why surface model isn't used in collision detection. Consider whether ignoring surface Z is correct for steep mesh regions.
3. **Add integration tests for region ordering**: Test ByArea vs Global on multi-pocket part to verify travel reduction and correctness.
4. **Document fine stepdown and flat area detection parameters**: Histogram bin size, flat threshold (>2% cells), and interaction with depth_per_pass. Consider exposing as tuneable params.
5. **Consider surface-aware collision in `is_clear_path_3d()`**: Currently ignores surface Z model. If traveling between two high Z points over a valley, may incorrectly report "clear" when material fills valley.
6. **Separate semantic parsing from compute**: Extract adaptive3d label parsing to a dedicated module for maintainability.
7. **Profile direction search evals on large grids**: Default 18 coarse + 7 narrow candidates may be slow on 10k cell grids. Consider adaptive phase skipping if Phase 1 score is very good.
