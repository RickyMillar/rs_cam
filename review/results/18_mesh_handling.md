# Review: Mesh Handling & STL Import

## Summary
The mesh system uses a clean indexed triangle representation with cached bounding boxes, automatic winding detection/repair, and a 2D uniform grid spatial index. STL import supports both binary and ASCII with unit scaling and auto-repair at >5% inconsistency. The main concern is missing bounds validation on triangle indices after STL parsing, and GPU upload duplicates all vertices (no index sharing) which wastes memory on large meshes.

## Findings

### Mesh Representation
- **TriangleMesh struct** (mesh.rs:29-35): Indexed representation with `vertices: Vec<P3>`, `triangles: Vec<[u32; 3]>`, `faces: Vec<Triangle>` (precomputed), `bbox: BoundingBox3` (cached)
- **Normal computation**: Per-face via cross product, normalized. Degenerate triangles (norm < 1e-15) default to `(0, 0, 1)`. No per-vertex normals. `geo.rs:139-156`
- **Bounding box**: Computed once during construction via `BoundingBox3::from_points()`. Not updated after winding fixes. `mesh.rs:87`
- **Winding consistency**: Actively checked and auto-fixed on STL import (mesh.rs:97-109, 174-186)

### STL Import
- **Binary + ASCII**: Both supported — `stl_io::read_stl()` (v0.11) auto-detects. `mesh.rs:41-117`
- **Unit scaling**: Explicit scale factor parameter. GUI maps scale=1.0 to mm, otherwise `ModelUnits::Custom(scale)`. Conversion: `v.0[0] as f64 * scale`. `mesh.rs:55-57, import.rs:20-24`
- **Vertex indexing**: stl_io returns welded vertices; code directly casts indices to u32 with NO bounds checking. `mesh.rs:68-70, 146-148`
- **Winding auto-repair**: If >1% inconsistent, logs warning. If >5%, auto-fixes via BFS propagation from most-upward-facing triangle. `mesh.rs:96-109, 253-341`
- **Empty mesh**: Returns `MeshError::EmptyMesh` if no faces loaded. `mesh.rs:45-46`
- **Error propagation**: `MeshError::StlRead(io::Error)` for file I/O errors. `mesh.rs:9-15`
- **Large files**: No streaming/chunking — entire STL loaded into memory. Conversion is O(n).

### Spatial Indexing
- **Type**: 2D uniform grid in XY plane (NOT k-d tree). `mesh.rs:365-480`
- **Auto-sizing**: Targets ~50 cells per axis, minimum 1.0mm cell size. Cell size clamped to `max_extent / 4.0`. `mesh.rs:452-479`
- **Query**: `query(cx, cy, radius) -> Vec<usize>` — rectangle intersection to find overlapping cells, dedup via `seen` bitmap. `mesh.rs:458-479`
- **`kiddo` dependency**: Listed in Cargo.toml but UNUSED anywhere in codebase — dead dependency

### Triangle Operations
- **Ray-AABB intersection**: Kay/Kajiya slab method. Handles origin-inside, parallel rays (1e-12 tolerance). `geo.rs:90-128`
- **Triangle contains point XY**: Barycentric coordinates with EPS = -1e-8 for edge tolerance. `geo.rs:161-177`
- **Z-at-XY**: Plane equation projection, returns None if nz < 1e-15 (vertical triangle). `geo.rs:181-190`
- **Point-to-segment distance**: Orthogonal projection with clamping. Degenerate segments (len < 1e-20) collapse to point. `geo.rs:194-214`

### Drop-Cutter Integration
- **Pattern**: facet_drop (most specific) -> vertex_drop (3 vertices) -> edge_drop (3 edges). `tool/mod.rs:142-157`
- **Facet drop**: Projects cutter center to triangle plane, checks XY containment, computes Z. Skips vertical triangles (nz < 1e-12). `tool/mod.rs:96-136`
- **Edge drop**: Tool-specific — Ball uses sphere-line intersection, Flat uses circle-line, VBit uses cone-line. Each tool file has own implementation.

### Mesh Repair
- **Winding check**: Edge direction histogram — counts directed edges per face pair. Reports consistent/inconsistent/boundary edges. `mesh.rs:197-245`
- **Winding fix**: BFS from seed (highest normal.z triangle), flips neighbors by swapping vertices [0,2,1], recomputes normals. `mesh.rs:253-341`
- **No degenerate triangle filtering**: Zero-area triangles kept in mesh with default normal (0,0,1)
- **No mesh simplification/decimation**

### GPU Upload (mesh_render.rs)
- **Flat shading**: Creates duplicate vertex data per triangle (3 vertices per tri, even if shared). `mesh_render.rs:45-70`
- **Memory**: N faces -> 3N vertices * 24 bytes. 1M triangles -> ~72MB vertex buffer.
- **Index buffer**: Redundant (sequential 0,1,2,3,4,5...) since every vertex is unique.
- **Upload**: Synchronous via `DeviceExt::create_buffer_init`. `mesh_render.rs:72-82`

### Testing
**mesh.rs**: 10 tests (mesh.rs:542-720)
- Mesh generation (hemisphere, flat)
- Spatial index (basic query, auto-sizing, oversized cell clamping)
- Winding detection (consistent hemisphere/flat, detect flipped, fix flipped, normals updated after fix)

**geo.rs**: 12 tests (geo.rs:216-380)
- BoundingBox (from_points, contains_point, overlaps_xy)
- Triangle (normal, contains_point_xy, z_at_xy)
- Ray-AABB (6 cases: hit, miss, inside, behind, diagonal)
- Point-segment distance + degenerate

**Drop-cutter tools**: Each tool file (ball.rs, flat.rs, vbit.rs) has vertex/facet/edge drop tests plus full hemisphere tests.

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | High | No bounds validation on triangle indices after STL parsing — panics if stl_io returns invalid indices | mesh.rs:68-70, 80-82 |
| 2 | Med | Bounding box not recomputed after winding fix (normals change, but vertices could theoretically shift in future) | mesh.rs:341 |
| 3 | Med | `kiddo` crate in Cargo.toml but never imported or used — dead dependency | Cargo.toml |
| 4 | Med | GPU upload duplicates all vertices for flat shading — ~3x memory overhead | mesh_render.rs:45-70 |
| 5 | Low | Degenerate triangles (zero area) kept in mesh with fake normal (0,0,1), not filtered or flagged | geo.rs:145, mesh.rs:79-84 |
| 6 | Low | `indices.len() as u32` in mesh_render.rs could panic if mesh has >2^32 indices (>1.4B triangles) | mesh_render.rs:87 |
| 7 | Low | No streaming/chunking for large STL files — entire file loaded into RAM | mesh.rs:41-117 |

## Test Gaps
- No test for malformed STL files (corrupt header, truncated data)
- No test for invalid triangle indices from stl_io
- No test for very large meshes (performance / memory)
- No test for degenerate triangle behavior in drop-cutter
- No benchmark comparing uniform grid vs k-d tree spatial index
- No test for mesh with inconsistent winding going through full drop-cutter pipeline

## Suggestions
- Add bounds check on triangle indices after STL parsing (validate all indices < vertices.len())
- Remove `kiddo` from Cargo.toml if not used, or document why it's kept
- Consider smooth shading (shared vertices + per-vertex normals) for GPU upload to reduce memory
- Add optional degenerate triangle filtering (zero-area detection and removal)
- Add malformed STL test cases for robustness
