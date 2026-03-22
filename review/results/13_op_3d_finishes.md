# Review: 3D Finishing Strategies

## Summary

The rs_cam project implements five distinct 3D finishing strategies (Steep/Shallow, Ramp, Spiral, Radial, Horizontal) alongside shared slope analysis infrastructure. All five strategies are complete, well-tested (~56 tests total), and correctly wired into both CLI and GUI. Code quality is high with no unwrap() in library code, robust edge-case handling, and clear parameter validation through default trait implementations.

## Findings

### Steep/Shallow
- **Status**: Fully implemented and mature.
- **Algorithm**: Threshold-angle classification splits mesh into steep and shallow regions. Steep areas use waterline contours (constant-Z passes via waterline module), shallow areas use drop-cutter raster with zigzag pattern.
- **Key parameters**: `threshold_angle` (40deg default), `overlap_distance` (4mm default), `wall_clearance` (2mm default), `steep_first` (true default).
- **Region handling**: Morphological operations (dilate/erode) on boolean grids expand/contract region boundaries for overlap and clearance. `dilate_grid()` and `erode_grid()` functions are private helpers with correct boundary checks.
- **Contour filtering**: Inner filter at steep_shallow.rs:144 uses hardcoded 30deg threshold to reject contours mostly in shallow regions before grid membership check. Intentional two-stage filter but could be parameterized.
- **Test coverage**: 9 tests covering grid morphology, classification, overlap behavior, zigzag ordering, and integration with hemisphere mesh.
- **Tool support**: Works with any MillingCutter via drop-cutter and waterline infrastructure.

### Ramp Finish
- **Status**: Fully implemented and mature.
- **Algorithm**: Generates continuous helical descent on steep walls by:
  1. Creating waterline contours at multiple Z levels
  2. Parameterizing each contour by arc-length
  3. Matching adjacent Z-level contours by nearest centroid
  4. Interpolating XY/Z across matched pairs to create smooth ramp path
  5. Optionally confining to slope angle range
- **Key parameters**: `max_stepdown` (1mm default), `slope_from`/`slope_to` (0-90deg default), `direction` (Climb/Conventional/BothWays), `order_bottom_up`.
- **Path interpolation**: `ParamContour::point_at()` uses binary search on normalized arc-length parameter; handles degenerate cases (few points, zero length).
- **Contour matching**: Centroid-based with distance threshold (max extent x 0.5). Unmatched contours silently dropped — reasonable but could be logged.
- **Test coverage**: 12 tests covering parameterization, interpolation, contour matching, slope confinement, stepdown limits, and direction alternation.

### Spiral Finish
- **Status**: Fully implemented and operational.
- **Algorithm**: Generates Archimedean spiral `r(theta) = stepover * theta / (2*pi)` from center outward (or reversed), drop-cutting each point.
- **Key parameters**: `stepover` (radial spacing), `direction` (InsideOut/OutsideIn), standard rates.
- **Adaptive angular stepping**: `d_theta = stepover / max(r, stepover)` ensures roughly constant linear spacing. The max() prevents near-zero division at spiral center.
- **Center detection**: Uses bounding box center; max radius to farthest corner. Single continuous toolpath.
- **Test coverage**: 7 tests covering spiral point generation, direction reversal, stock_to_leave offset, safe_z validation, and full hemisphere integration.

### Radial Finish
- **Status**: Fully implemented and operational.
- **Algorithm**: Generates radial spokes at fixed angular intervals from center outward, sampling each spoke via drop-cutter.
- **Key parameters**: `angular_step` (5deg default), `point_spacing` (0.5mm default).
- **Zigzag linking**: Even-numbered spokes run center-to-edge; odd-numbered spokes reverse (edge-to-center) for efficient linking.
- **Uncontacted point trimming**: Function `trim_uncontacted()` removes leading/trailing points that miss the mesh; keeps longest contiguous run of contacted points.
- **Fallback Z handling**: Non-contacted points use `min_z_fallback` (bbox.min.z - 1000.0); trimming prevents these from being cut.
- **Test coverage**: 11 tests covering max radius computation, trim logic, spoke count, Z at surface, stock_to_leave, and zigzag direction alternation.

### Horizontal Finish
- **Status**: Fully implemented and operational.
- **Algorithm**:
  1. Classifies mesh triangles as flat (normal.z.abs() > cos(threshold_angle))
  2. Groups flat triangles by Z height (within stepover/2 tolerance)
  3. Sorts regions top-down (avoid collisions)
  4. Rasters each region in zigzag pattern, including only points over flat triangles
- **Flat detection**: Uses cross product magnitude check for degenerate triangles; normal-based flatness test (angle-based via cos threshold).
- **Region grouping**: Sweeps sorted triangles; groups within z_tolerance. Drain-based re-construction.
- **Spatial filtering**: `is_point_over_flat_triangle()` queries spatial index for nearby triangles, checks flat_face_set membership, then XY containment.
- **Test coverage**: 4 tests covering flat mesh, default params, steep mesh (empty result), and stock_to_leave offset validation.

### Slope Analysis (slope.rs)
- **Status**: Fully implemented as shared infrastructure.
- **SurfaceHeightmap**: Precomputed Z grid via drop-cutter at initialization (one parallel batch). O(1) lookups thereafter. Supports cancellation.
- **SlopeMap**: Builds surface normals and slope angles from Z grid using finite differences (central for interior, forward/backward at boundaries).
- **Normal calculation**: `n = (-dz/dx, -dz/dy, 1).normalize()`; angle = acos(n.z) — correctly maps [0, pi/2].
- **Curvature**: Mean curvature via second derivatives; positive = convex, negative = concave. Available but not currently applied in finishing strategies.
- **Classification function**: `classify_steep_shallow()` returns boolean grid (threshold in radians).
- **Test coverage**: 13 tests covering heightmap lookups, slope calculations on flat/ramp/dome surfaces, curvature (convex/concave/flat/bowl), and threshold classification.

### Shared Patterns
- All strategies use `SurfaceHeightmap` + `SlopeMap` for surface analysis (except spiral/radial which use direct drop-cutter sampling)
- Steep/shallow and ramp both use `waterline_contours()` from waterline.rs
- Spiral and radial use `point_drop_cutter()` for individual point sampling
- All use `Toolpath::emit_path_segment()` and `final_retract()` for output
- All param structs implement `Default`, providing safe baseline values
- All strategies use `dyn MillingCutter` trait, supporting all 5 tool families
- No panics or unwrap() in library code

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Low | Hardcoded 30deg threshold in steep_shallow for contour filtering differs from user-facing threshold_angle parameter. Intentional two-stage filter but asymmetry could confuse users if documented. | steep_shallow.rs:144 |
| 2 | Low | Unmatched contours in ramp_finish (walls appearing/disappearing between Z levels) are silently discarded. Could benefit from debug-level logging. | ramp_finish.rs:369 |
| 3 | Low | Spiral center detection uses bounding box center; assumes roughly rectangular mesh. Off-center or highly asymmetric meshes will have suboptimal spiral placement. | spiral_finish.rs:57-58 |
| 4 | Low | Radial spoke fallback_z = bbox.min.z - 1000.0 is a magic number. Should use a configurable parameter or derive from bounding box size. | radial_finish.rs:61 |
| 5 | Low | Horizontal finish comment at line 27 says "Added to" but should clarify direction of stock_to_leave offset. | horizontal_finish.rs:27 |

## Test Gaps

- No integration tests comparing output of different strategies on the same mesh to validate relative coverage/efficiency
- Tests use only BallEndmill; no tests with FlatEndmill, BullNoseEndmill, or VBit to validate tool-specific behavior
- No GUI parameter persistence round-trip tests (config -> core params -> result)
- No tests with non-manifold or self-intersecting meshes
- No tests for very small stepover relative to mesh size (spiral/radial may generate excessive points)
- ramp_finish and steep_shallow use drop-cutter with cancellation support but tests don't exercise cancel path
- No performance benchmarks for large meshes or high point densities

## Suggestions

1. **Parameterize hardcoded constants**: The 30deg threshold in steep_shallow and -1000.0 fallback in radial should be exposed as configurable parameters or documented constants.
2. **Add debug-level logging**: Unmatched contours in ramp_finish, filtered-out points in radial trim, and skipped regions in horizontal finish would benefit from trace/debug logs for troubleshooting.
3. **Validate parameter ranges at boundary**: Add simple checks in core param structs (e.g., `stepover > 0`, `threshold_angle in [0, 90]`) or document that UI is responsible for range validation.
4. **Merge spiral/radial center detection**: Both use identical bounding-box center logic. Consider extracting to shared helper.
5. **Document finish strategy differences**: Add capability matrix (supported tool types, mesh requirements, surface quality, tool engagement model) to FEATURE_CATALOG.md so users know when to choose each strategy.
6. **Slope confinement in all strategies**: Ramp finish has excellent slope_from/slope_to filtering; consider exposing similar for steep_shallow and other strategies.
7. **Test with varied tool types**: Add parameterized tests that run each strategy with multiple MillingCutter implementations to catch tool-specific edge cases.
