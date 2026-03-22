# Review: Pencil & Scallop Operations

## Summary

Both pencil and scallop operations are well-architected 3D finishing strategies that properly separate mathematical concerns from toolpath generation. Pencil detects concave mesh edges (creases) and traces along them, while scallop generates concentric offset contours with variable stepover to maintain constant scallop height. Both use drop-cutter for Z queries and are tool-agnostic (accept any MillingCutter). Code quality is high with comprehensive tests covering edge cases.

## Findings

### Pencil Operation

**Crease Detection:**
- Detects via dihedral angle (angle between adjacent face normals)
- Filters edges where `dihedral_angle > (pi - bitangency_angle_threshold)`
- At default 160deg threshold, only acute creases (<20deg fold) are machined
- Concavity determined by: `sign = (n1 x n2) . edge_vec > 0` (depends on consistent outward normals)
- Only processes shared edges (2-adjacent-face edges); ignores boundary and non-manifold edges

**Tracing Behavior:**
- Traces ALONG the crease (not across it)
- Chains connected concave edges into polylines via graph traversal (degree-2 path extraction)
- Handles endpoints, junctions, and closed loops
- Samples points at fixed spacing (0.5mm default)
- Applies offset passes perpendicular to the centerline path in XY plane (left/right sides)

**Offset Passes:**
- Offset uses 2D perpendicular direction (rotate tangent 90deg CCW in XY)
- Each side offset = `pass_num * offset_stepover` (configurable per side)
- Offset polylines lifted back to mesh surface via drop-cutter after offset
- Default: 1 pass each side with 0.5mm stepover

**Tool Assumption:**
- No tool type assumption enforced in code (uses `&dyn MillingCutter`)
- Comments mention "ball/bull nose cutters" but the drop-cutter algorithm works for any cutter shape

**Stock Leaving:**
- Only uses `stock_to_leave_axial` parameter (radial variant stored but unused)
- Applied uniformly to all Z values from drop-cutter

### Scallop Operation

**Variable Stepover Calculation:**
- Implements **curvature-aware** variable stepover via `variable_stepover()` function
- Three adjustments applied in sequence:
  1. **Slope adjustment**: `R_slope = R / cos(slope_angle)` (capped at cos=0.05, ~87deg)
  2. **Curvature adjustment**: Effective radius from convex/concave formula
  3. **Final cap**: Result clamped to `[0.05*R, 4.0*R]`

**Curvature Awareness:**
- Samples slope and curvature at each ring sample point via `SlopeMap`
- Convex surfaces: `R_eff = R * Rc / (R + Rc)` (tighter stepover, finer scallops)
- Concave surfaces: `R_eff = R * |Rc| / (|Rc| - R)` (wider stepover allowed)
- Flat surfaces: `R_eff = R`
- SlopeMap computed from drop-cutter heightmap with gradient/curvature derivatives

**Ring Generation:**
- Iteratively offsets boundary polygon inward using `offset_polygon()`
- Stepover recalculated at each ring based on average local slope/curvature
- Each 2D ring lifted to 3D via drop-cutter Z queries
- Stops when polygon collapses to empty set (max_rings bounded)

**Scallop Direction:**
- `OutsideIn` (default): Starts at boundary, works inward
- `InsideOut`: Reverses ring order to start from center
- No climb/conventional distinction (all rings follow same Z-up contouring)

**Continuous Mode:**
- Optional spiral connection: connects adjacent rings at nearest point to reduce rapids
- Chains rings with helix-like transitions instead of discrete rapid moves

### Scallop Math

**Module Separation:**
- `scallop_math.rs`: Pure math functions (no I/O, no mesh operations)
  - `scallop_height_flat(R, stepover)`: h = R - sqrt(R^2 - (stepover/2)^2)
  - `stepover_from_scallop_flat(R, h)`: stepover = 2*sqrt(2Rh - h^2)
  - `effective_radius(R, curvature_radius)`: Handles convex/concave
  - `variable_stepover(R, h, slope, curvature)`: Full 3D formula
- `scallop.rs`: Toolpath generation (mesh operations, drop-cutter, polygon offsetting)

### Edge Case Handling

- **Flat surfaces**: variable_stepover degenerates to flat formula correctly; no "infinite stepover" risk; clamped to `[0.05*R, 4.0*R]`
- **Sharp creases on faceted STL**: Pencil works correctly (dihedral angles exact from face normals); V-groove test validates 90deg dihedral fold
- **Very high scallop height**: If `scallop_height >= tool_radius`, `stepover_from_scallop_flat()` returns 0; single ring generated; loop exits
- **Pencil on convex-only mesh**: Tested; confirms empty output (no concave edges)
- **Non-manifold edges**: Skipped (correct behavior)

### Integration (CLI/GUI)

- **CLI**: Both exposed as CLI subcommands
- **GUI**: `PencilConfig` and `ScallopConfig` in `state/toolpath/configs.rs`, serializable for project I/O, mapped to core params in `execute.rs`, integrated as `SemanticToolpathOp` for debugger support
- **Config note**: `stock_to_leave_radial` field exists in GUI configs but is unused (never passed to core functions)

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Medium | Concavity detection in pencil depends on consistent outward-facing normals; no validation that mesh normals are actually outward | pencil.rs:130-134 |
| 2 | Medium | Scallop stepover clamped to `[0.05*R, 4.0*R]` per ring; very high scallop_height may result in stepover > expected, violating constant-height promise | scallop.rs:199-201 |
| 3 | Low | `stock_to_leave_radial` field defined in GUI configs but never used; always passes `stock_to_leave_axial` to core functions | configs.rs:461,492; execute.rs:787,812 |
| 4 | Low | Pencil offset passes apply 2D XY offset but Z is re-queried; offset direction is pure XY, may not follow surface curvature properly on highly curved surfaces | pencil.rs:359-396 |
| 5 | Low | Pencil line 212: `*chain.last().unwrap_or(&start)` after appending to chain — safe but could use clearer pattern | pencil.rs:212 |

## Test Gaps

- No test for mesh with inconsistent (inward) normal orientation (pencil)
- No test for sharp fold (0deg dihedral) vs soft crease; threshold edge cases (pencil)
- No test for `continuous=true` spiral ordering correctness (scallop)
- No test combining slope filtering with curvature on complex surfaces (scallop)
- No test for degenerate case where polygon offset produces multiple disjoint polygons (scallop)
- No performance tests with large meshes (>100K triangles) for either operation

## Suggestions

1. **Clarify `stock_to_leave_radial` usage**: Either implement radial stock offset or remove the unused field from configs to avoid user confusion.
2. **Add normal orientation validation**: Pencil could log a warning if normals appear inward-facing, or document the requirement clearly.
3. **Stepover clamping transparency**: Document in UI/tooltip that scallop stepover is clamped to `[0.05*R, 4.0*R]`, so very high scallop heights may not be honored exactly.
4. **Pencil offset follow curvature**: Consider using 3D offset (following surface normal) instead of 2D XY offset for offset passes on highly curved surfaces.
5. **Add continuous mode test**: Verify ring ordering is sensible in `continuous=true` (spatial continuity metrics).
6. **Extend mesh validation**: Both operations assume well-formed STL (manifold, consistent winding). Consider pre-flight checks or logging warnings for non-manifold geometry.
