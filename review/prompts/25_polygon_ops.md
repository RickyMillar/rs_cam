# Review: Polygon Operations

## Scope
2D polygon handling — offsets, boolean ops, clipping — used by all 2.5D operations.

## Files to examine
- `crates/rs_cam_core/src/polygon.rs`
- Usage of `cavalier_contours` crate (arc-preserving offsets)
- Usage of `geo` crate
- How polygons flow from import → operations → toolpaths

## What to review

### Offset operations
- Inward/outward offset for tool compensation
- Arc-preserving offsets via cavalier_contours — quality?
- Self-intersection handling after offset
- Multiple offset levels (pocket clearing)

### Boolean operations
- Union, intersection, difference — which are used?
- Are they from `geo` or custom?
- Robustness with degenerate inputs

### Winding and orientation
- CW vs CCW conventions
- How is winding enforced or detected?
- Island/hole detection

### Edge cases
- Very thin slivers after offset
- Coincident edges
- Self-intersecting input polygons
- Polygons with zero-length segments

### Testing & code quality

## Output
Write findings to `review/results/25_polygon_ops.md`.
