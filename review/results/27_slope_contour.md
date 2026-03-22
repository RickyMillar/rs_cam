# Review: Slope & Contour Analysis

## Summary
Three well-engineered surface analysis modules form the foundation for all 3D finishing strategies. `slope.rs` provides per-cell slope classification via finite differences on a drop-cutter heightmap. `contour_extract.rs` implements marching squares on a boolean fiber grid to produce topologically correct closed loops. `fiber.rs` manages blocking interval merging for push-cutter waterline operations. All modules are mathematically sound, have meaningful tests (32 total), and contain zero `unwrap()` in production code.

## Findings

### Slope Analysis (`slope.rs`, ~590 lines, 13 tests)

#### How Slope Is Computed
- **SurfaceHeightmap** (`slope.rs:17-134`): grid of Z values from rayon-parallelized drop-cutter queries, O(1) lookup per cell after init
- **SlopeMap** (`slope.rs:139-281`): per-cell surface normals, slope angles (radians from horizontal), and mean curvature via:
  - First derivatives (`slope.rs:179-193`): central differences for interior cells, forward/backward at boundaries
  - Surface normal (`slope.rs:196`): `n = (-dz/dx, -dz/dy, 1).normalize()`
  - Slope angle (`slope.rs:197`): `acos(n.z)` — 0 = horizontal, π/2 = vertical
  - Mean curvature (`slope.rs:199-218`): `(d²z/dx² + d²z/dy²) / 2` — positive = convex, negative = concave

#### Classification
- **Per-point** (grid cell), not per-region (`slope.rs:283-294`)
- Binary grid (`Vec<bool>`) where `true = angle >= threshold_rad`
- Threshold in degrees, converted to radians

#### Numerical Stability
- `n.z` clamped to [0,1] before acos
- Boundary cells: second derivatives default to 0 (safe simplification, loses curvature at edges)
- World-to-cell conversion returns Option (`slope.rs:257-280`)

#### Consumers
- `steep_shallow.rs:313-319`: builds heightmap → slope map → classification
- `scallop.rs:280-283`: variable stepover based on slope
- `ramp_finish.rs:298-301`: filter paths by slope angle range
- `adaptive3d.rs:1722-1730, 949-1011`: material detection + pre-stamp steep walls at > 30° threshold

### Contour Extraction (`contour_extract.rs`, ~495 lines, 5 tests)

#### Pipeline
1. **Boolean grid** from X/Y fiber intervals (`contour_extract.rs:54-80`): cell is `true` (inside) if BOTH X-fiber and Y-fiber are blocked at that intersection
2. **Marching squares** (`contour_extract.rs:89-190`): standard 16-case algorithm on 4-corner cells
   - Case index: `bl | (br<<1) | (tr<<2) | (tl<<3)`
   - Saddle cases (5, 10) emit TWO segments — topologically correct
   - **Edge points use exact fiber interval endpoints** (`contour_extract.rs:194-271`), not grid cell centers. Tolerance: 1e-10. Fallback to midpoint if no endpoint found
3. **Segment chaining** (`contour_extract.rs:273-340`): greedy nearest-neighbor matching (eps=1e-6), loop closure detection, max-iterations guard, min 3-point filter

#### Accuracy
- Edge point precision comes from actual fiber geometry, not approximations
- Topological correctness from proper saddle case handling
- Multiple disconnected contours handled correctly

#### Performance Concern
- Segment chaining is O(remaining²) worst case (`contour_extract.rs:310-325`) — acceptable for typical grids (<10k segments) but not optimized for very large grids

### Fiber Analysis (`fiber.rs`, ~306 lines, 14 tests)

#### What It Is
- **NOT wood grain direction** — fiber is a line segment in XY at constant Z used by push-cutter and waterline
- Parameterized as `point(t) = p1 + t·(p2−p1)` for `t ∈ [0,1]`
- Stores merged blocking intervals where cutter cannot go (would gouge)

#### Interval Merging (`fiber.rs:116-145`)
- Clamps to [0, 1], skips width < 1e-15
- Insert maintaining sorted order, merges overlaps on the fly
- Example: adding [0.4, 0.7] to existing [0.2, 0.5] → merge to [0.2, 0.7]

#### Key Methods
- `point(t)` (`fiber.rs:96-102`): linear interpolation to 3D point
- `tval(p)` (`fiber.rs:105-113`): project 3D point onto fiber, zero-length guard at `len_sq < 1e-20`
- `is_blocked(t)` (`fiber.rs:164-166`): check if parameter t is inside any interval (1e-10 tolerance)
- `cl_points()` (`fiber.rs:154-161`): extract cutter-location points at interval boundaries

#### Usage Pattern
1. Waterline (`waterline.rs:54-83`): create X/Y fiber grids → push-cutter → populate intervals → weave contours
2. Steep-shallow (`steep_shallow.rs:148`): extract intervals → waterline contours on steep surfaces
3. Pencil: trace concave edges via fiber intervals

### Data Flow
```
Mesh + Cutter
    ↓
SurfaceHeightmap (drop-cutter queries)
    ↓
SlopeMap (finite differences)
    ├→ classify_steep_shallow() [Boolean grid]
    ├→ steep_shallow_toolpath()
    ├→ ramp_finish_toolpath()
    └→ adaptive3d (pre-stamp, material detection)

Mesh + Cutter + Z
    ↓
X-fibers and Y-fibers (grid)
    ↓
push-cutter (populate intervals)
    ↓
weave_contours (marching squares + chaining)
    ├→ waterline_toolpath()
    └→ steep_shallow (steep passes)
```

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Low | Boundary cells lose curvature detail (second derivatives default to 0) — silent, affects contours near mesh edges | `slope.rs:208-213` |
| 2 | Low | Segment chaining epsilon (1e-6) hardcoded — may be insufficient for coarse fiber sampling; no warning if segments fail to chain | `contour_extract.rs:279` |
| 3 | Low | Segment chaining is O(n²) worst case — could be slow for very large grids (>10k segments) | `contour_extract.rs:310-325` |
| 4 | Low | Fallback edge points use midpoint when no interval endpoint found in cell range — may reduce contour accuracy | `contour_extract.rs:251, 270` |

## Test Gaps
- No integration test: mesh → slope → classification → steep/shallow passes (tested implicitly in pipeline.rs only)
- No performance benchmarks for marching squares on million-cell grids
- No robustness tests for degenerate inputs (single-row grid, single fiber)
- No test for segment chaining failure (segments that don't close into loops)
- No curvature validation on real-world mesh geometry (only synthetic dome/bowl/hemisphere)

## Suggestions
- Parameterize segment chaining epsilon (`contour_extract.rs:279`) instead of hardcoding 1e-6
- Document that boundary cells lose curvature detail in `slope.rs` header comments
- Add integration test: mesh → slope → classification → steep/shallow pass generation
- For large grids, consider optimizing segment chaining with a spatial hash or segment graph instead of O(n²) scan
- Wire fiber grain direction (already designed in `research/feeds_and_speeds_integration_plan.md`) when feeds/speeds integration matures
