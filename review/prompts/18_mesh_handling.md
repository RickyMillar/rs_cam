# Review: Mesh Handling & STL Import

## Scope
Triangle mesh representation, STL file import, mesh operations used across the engine.

## Files to examine
- `crates/rs_cam_core/src/mesh.rs`
- STL import code (grep for `stl_io` usage)
- `crates/rs_cam_core/src/geo.rs` (geometry utilities)
- Mesh usage in dropcutter, simulation, collision
- GUI import path: `crates/rs_cam_viz/src/io/import.rs`
- GPU mesh upload: `crates/rs_cam_viz/src/render/mesh_render.rs`

## What to review

### Mesh representation
- TriangleMesh struct: vertices, triangles, normals
- Is it indexed (shared vertices) or flat (per-triangle vertices)?
- Normal computation: per-face or per-vertex? Consistent winding?
- Bounding box computation and caching

### STL import
- Binary vs ASCII STL support
- Unit scaling (mm, inch, m, cm)
- Winding consistency check (mentioned in import flow)
- Error handling for malformed STL files
- Large file performance

### Mesh operations
- Spatial indexing for triangle queries (k-d tree via kiddo)
- Triangle-ray intersection
- Triangle-point distance
- Any mesh repair / cleanup?

### Edge cases
- Non-manifold meshes
- Degenerate triangles (zero area)
- Very large meshes (millions of triangles)
- Meshes with inconsistent winding

### Testing & code quality

## Output
Write findings to `review/results/18_mesh_handling.md`.
