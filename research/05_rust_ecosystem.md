# Rust Ecosystem for CAM

Evaluated crates organized by function, with recommendations.

---

## Recommended Core Stack

| Layer | Crate | Purpose | Downloads |
|-------|-------|---------|-----------|
| Math | `nalgebra` | Linear algebra, 3D transforms | 59.5M |
| 2D Geometry | `geo` + `geo-types` | 2D primitives, booleans, spatial relations | 14.4M |
| 3D Queries | `parry3d` | Ray casting, point projection, collision | 1.4M |
| Spatial Index | `rstar` | R*-tree for spatial lookups | 19.5M |
| KD-Tree | `kiddo` | Fast KD-tree for drop-cutter | 628K |
| 2D Booleans | `i_overlay` (via geo) | Union, intersection, difference, offset | 2.55M |
| 2D Offset | `cavalier_contours` | Arc-preserving polyline offset | 391 |
| Polygon Offset | `clipper2-rust` | Clipper2 port, round-join offset | new |
| Triangulation | `spade` | Delaunay/CDT | 10.8M |
| Parallelism | `rayon` | Data parallelism | 333.7M |
| STL I/O | `stl_io` | Read/write STL | 2.5M |
| DXF I/O | `dxf` | Read/write DXF | 112K |
| SVG Input | `usvg` | SVG simplification/parsing | 12M |
| G-code Parse | `gcode` | G-code parser | 26K |
| 2D Curves | `kurbo` | Bezier/arc ops, curve offset | 17M |
| Contours | `contour` | Marching squares | 74K |
| CLI | `clap` | Argument parsing | 719.6M |
| Config | `serde` + `toml` | Serialization + TOML config | huge |
| Errors | `thiserror` + `anyhow` | Error types | huge |
| Logging | `tracing` | Structured logging | 508M |

---

## Detailed Evaluations

### Computational Geometry

**geo + geo-types** (ESSENTIAL)
- Most comprehensive 2D geometry library in Rust
- Boolean ops (via i_overlay), buffer/offset, simplification, triangulation
- Distance, spatial relations, affine transforms, Chaikin smoothing
- 2D only. No 3D support.

**parry3d** (ESSENTIAL for 3D)
- Collision detection, ray casting, point projection, contact detection
- Shapes: Ball, Cylinder, Cone, TriMesh, HeightField, Voxels
- HeightField maps directly to drop-cutter output grids
- Cylinder/Ball shapes map to endmill cutters
- Uses nalgebra natively

**spade** (USEFUL)
- Delaunay and Constrained Delaunay Triangulation
- Exact geometric predicates (robust computation)
- Natural neighbor interpolation (heightmap smoothing)

**robust** (foundation)
- Robust floating-point predicates (Shewchuk's)
- Used internally by geo and spade

---

### Polygon Operations

**i_overlay** (RECOMMENDED for booleans)
- Pure Rust, powers geo crate's BooleanOps trait
- 475K downloads/month, actively maintained
- Handles self-intersections, holes, multiple contours

**clipper2-rust** (RECOMMENDED for offset)
- Pure Rust port of Clipper2 (the CAM industry standard)
- 444 tests, feature-complete
- Round-join offset is exactly what CAM needs for cutter compensation
- Very new (Feb 2026), needs vetting

**cavalier_contours** (HIGHLY RELEVANT)
- Arc-preserving polyline offset -- produces G2/G3-compatible output
- Polylines with bulge values (native arc segments)
- Boolean operations on closed polylines
- MIT license. Specifically designed for CAD/CAM use.
- This is the closest thing to a CAM-specific geometry library in Rust

**geo-clipper** (alternative)
- Boolean ops on geo-types via Clipper v1 C++ wrapper
- More mature binding but older algorithm

---

### Linear Algebra

**nalgebra** (RECOMMENDED primary)
- 59.5M downloads. Extremely mature.
- Native parry3d interop (same maintainer, Dimforge)
- Compile-time dimension checking
- Matrix factorizations (LU, QR, SVD, Cholesky)
- Sparse matrix support, no_std

**glam** (secondary, for rendering)
- SIMD-optimized, fastest for basic vector ops
- Default math for Bevy
- No matrix factorizations, limited to 2D/3D/4D
- Use for visualization code where raw speed matters

**cgmath** -- DEPRECATED. Do not use.

---

### Spatial Indexing

**rstar** (R*-tree)
- 19.5M downloads. Part of GeoRust.
- N-dimensional, integrates with geo-types
- Nearest-neighbor, range queries, bulk loading
- Good for 2D polygon/contour queries

**kiddo** (KD-tree)
- 628K downloads. SIMD-accelerated.
- ImmutableKdTree for static data (perfect for STL triangles)
- Sphere queries
- Best for 3D point/triangle spatial queries in drop-cutter

**bvh** (BVH)
- Surface Area Heuristic construction
- Rayon parallelism support
- Alternative to KD-tree for triangle queries

---

### File Formats

**STL**: `stl_io` (2.5M DL, read/write, actively maintained)
**OBJ**: `tobj` (1.65M DL, lightweight OBJ loader)
**DXF**: `dxf` (112K DL, read/write, ixmilia, active)
**SVG**: `usvg` (12M DL, simplifies SVG to basic commands)
**3MF**: `threemf` (27K DL)
**G-code parse**: `gcode` (26K DL, streaming, no_std, O(n))
**G-code emit**: Custom (no mature emitter exists)
**STEP**: `ruststep` (21K DL) or `opencascade` bindings (immature)

---

### Visualization & GUI

**egui + eframe** (RECOMMENDED for UI)
- 14.1M downloads. Immediate-mode GUI.
- Native + WASM. Embedded 3D viewport via wgpu.
- Ideal for real-time parameter tweaking.
- Used by KeloCAM (Rust CAM attempt).

**wgpu** (ESSENTIAL for 3D rendering)
- 18.2M downloads. Cross-platform WebGPU.
- Vulkan/Metal/DX12/OpenGL/WebGPU backends.
- Embeds in egui via eframe's wgpu backend.

**kiss3d** (USEFUL for prototyping)
- Simple 3D graphics. Now uses wgpu + egui integration.
- From Dimforge (same as nalgebra/parry).
- Lines, meshes, camera controls out of the box.

**bevy** -- Overkill for CAM. ECS complexity not needed.

**plotters** (USEFUL for debug viz)
- 2D plotting. SVG/PNG/WASM. Good for heightmap/toolpath debug images.

---

### Additional Useful Crates

**kurbo** (2D curves)
- Bezier, arc, line operations. Arc length, curvature, offset, flatten.
- From linebender project. 17M downloads.
- Curve offsetting directly applicable to cutter compensation.

**lyon** (path tessellation)
- GPU-oriented 2D path tessellation. Convert toolpaths to triangle meshes for visualization.

**contour** (marching squares)
- Generate contour polygons from grids. Directly applicable to waterline generation from heightmaps.

**voronator / voronoice** (Voronoi diagrams)
- Useful for medial axis computation (V-carving, pocket center-line strategies).

---

### Existing Rust CAM Projects

**None implement real 3-axis CAM toolpath generation.** The space is completely open.

| Project | Status | What It Does |
|---------|--------|-------------|
| KeloCAM | Paused | UI only (egui+wgpu), no CAM ops |
| svg2gcode | Active, 393 stars | SVG to G-code (2D only) |
| GladiusSlicer | Alpha | 3D printing slicer (not CAM) |
| cnccoder | Small | Programmatic G-code gen for GRBL |
| cncsim | Small | G-code simulation to STL |

**svg2gcode** is the most relevant -- its arc fitting and G-code emission patterns are directly reusable.

---

### CAD Kernels (for context)

| Project | Status | Notes |
|---------|--------|-------|
| Truck | Active, 1.4K stars | B-rep + NURBS + STEP I/O. Most mature Rust CAD kernel. |
| Fornjot | Paused | B-rep experiments. Not production-ready. |
| opencascade-rs | WIP | Rust bindings to C++ OpenCASCADE. STEP support. |

---

## What Must Be Built from Scratch

1. Drop-cutter algorithm (cutter profile vs triangle mesh)
2. Push-cutter algorithm (horizontal fiber contact)
3. Waterline contour extraction (Weave graph)
4. 2D pocket clearing (offset + infill patterns)
5. Adaptive clearing (constant engagement)
6. G-code emitter (formatted text output)
7. Toolpath linking/optimization
8. Stock modeling (heightmap updates)
9. V-carving (medial axis + depth computation)
