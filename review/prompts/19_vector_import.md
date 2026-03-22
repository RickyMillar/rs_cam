# Review: Vector Import (SVG + DXF)

## Scope
2D vector file import for 2.5D operations.

## Files to examine
- `crates/rs_cam_core/src/svg_input.rs`
- `crates/rs_cam_core/src/dxf_input.rs`
- `usvg` and `dxf` crate usage
- Polygon representation after import
- GUI import path
- Error handling

## What to review

### SVG import
- What SVG elements are supported? (paths, rects, circles, text?)
- Curve flattening: tolerance parameter (0.1), quality
- Transform handling (nested transforms, viewBox)
- Units: mm, px, in — how are they handled?
- Multi-path SVGs: separate polygons or merged?

### DXF import
- What DXF entities are supported? (lines, arcs, polylines, splines?)
- Resolution parameter (5.0) — what does it control?
- Layer support?
- 3D DXF entities — ignored?

### Polygon representation
- How are imported paths stored? `Vec<Polygon2>`?
- Open vs closed paths
- Winding direction: CW vs CCW, does it matter?
- Nested paths (holes/islands)

### Edge cases
- Empty files
- Files with no geometry
- Very complex paths (thousands of segments)
- Text in SVG (should be pre-converted to paths)

### Testing & code quality

## Output
Write findings to `review/results/19_vector_import.md`.
