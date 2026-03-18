# Rust Ecosystem for CAM (Computer-Aided Manufacturing) - Comprehensive Research Report

*Research date: March 19, 2026*

---

## Table of Contents

1. [Computational Geometry](#1-computational-geometry)
2. [Polygon Boolean Operations & Clipping](#2-polygon-boolean-operations--clipping)
3. [Polygon Offset / Buffer](#3-polygon-offset--buffer)
4. [Mesh Processing](#4-mesh-processing)
5. [Linear Algebra & Math](#5-linear-algebra--math)
6. [Parallel Computation](#6-parallel-computation)
7. [File Format Crates](#7-file-format-crates)
8. [Visualization & GUI](#8-visualization--gui)
9. [CLI & Configuration](#9-cli--configuration)
10. [Existing Rust CAM / CNC Projects](#10-existing-rust-cam--cnc-projects)
11. [CAD Kernels in Rust](#11-cad-kernels-in-rust)
12. [Additional Useful Crates](#12-additional-useful-crates)
13. [Recommendations & Architecture](#13-recommendations--architecture)

---

## 1. Computational Geometry

### geo + geo-types (GeoRust ecosystem)
- **Crate**: [geo](https://crates.io/crates/geo) / [geo-types](https://crates.io/crates/geo-types)
- **Repository**: https://github.com/georust/geo
- **Latest version**: geo 0.32.0 (Dec 2025), geo-types 0.7.18 (Dec 2025)
- **Downloads**: geo 14.4M, geo-types 17M
- **License**: MIT OR Apache-2.0

**What it provides**: The most comprehensive 2D computational geometry library in Rust. geo-types provides primitive types (Point, Line, LineString, Polygon, MultiPolygon, Rect, Triangle, Coord, GeometryCollection). geo provides algorithms on those types.

**Key algorithms for CAM**:
- **Boolean operations**: intersection, union, difference, XOR on (Multi)Polygons (via `BooleanOps` trait, powered by i_overlay internally)
- **Buffer/Offset**: Create offset geometries at specified distance (`Buffer` trait)
- **Convex hull, concave hull**: Hull computation
- **Simplification**: Ramer-Douglas-Peucker, Visvalingam-Whyatt
- **Triangulation**: Earcut and Delaunay triangulation
- **Distance**: Euclidean, Hausdorff, Frechet distance
- **Spatial relations**: Contains, Intersects, Within, Covers, DE-9IM (Relate)
- **Line intersection**: Sweep-line Bentley-Ottmann segment intersection
- **Affine transforms**: Rotate, Scale, Translate, Skew, composable AffineOps
- **Centroid, area, bounding rect, minimum rotated rect**
- **Chaikin smoothing, densification, coordinate iteration**

**Maturity**: Very mature, actively maintained, large community (GeoRust org). Adheres to OGC Simple Feature standards. Edition 2024. 35K+ lines of code.

**Relevance**: ESSENTIAL. Core geometry library for 2D toolpath operations, pocket boundaries, polygon processing, spatial queries. The BooleanOps and Buffer traits are directly applicable to CAM offset operations.

**Limitations**: 2D only. No 3D geometry support. Some algorithms may not be as optimized as dedicated C++ libraries (GEOS/JTS) but the gap has narrowed significantly.

---

### parry2d / parry3d (Dimforge)
- **Crate**: [parry2d](https://crates.io/crates/parry2d) / [parry3d](https://crates.io/crates/parry3d)
- **Repository**: https://github.com/dimforge/parry
- **Latest version**: 0.26.0 (Jan 2026)
- **Downloads**: parry2d 1.2M, parry3d 1.4M
- **License**: Apache-2.0

**What it provides**: 2D and 3D collision detection and spatial query library. Originally designed for physics engines but highly useful for CAM geometric queries.

**Key shapes (3D)**: Ball (sphere), Capsule, Cone, Cuboid, Cylinder, Segment, Triangle, Tetrahedron, HalfSpace, ConvexPolyhedron, TriMesh, Polyline, HeightField, Voxels, Compound shapes. Plus rounded variants (RoundCuboid, RoundCylinder, etc.).

**Key queries**:
- **Ray casting**: Ray-shape intersection (RayCast trait) -- critical for drop-cutter algorithms
- **Point projection**: Closest point on shape (PointQuery trait)
- **Distance computation**: Between any two shapes
- **Contact detection**: Contact points with penetration depth
- **Intersection testing**: Boolean overlap check
- **Shape casting**: Translational/nonlinear motion collision (time-of-impact)
- **Contact manifolds**: Multi-point contact information

**Algorithms**: GJK (Gilbert-Johnson-Keerthi), EPA (Expanding Polytope), SAT (Separating Axis Theorem).

**Maturity**: Very mature, actively maintained by Dimforge (Sebastien Crozet). Used widely in game engines and physics simulations. ~35K lines of code.

**Relevance**: HIGHLY RELEVANT. The HeightField shape + ray casting + point projection queries are directly applicable to 3-axis CAM drop-cutter algorithms. TriMesh support enables working with STL models. The Cylinder/Ball/Cone shapes map directly to common CNC cutter geometries (flat endmill, ball endmill, bull-nose). Contact queries can determine cutter-workpiece intersection.

**Limitations**: Designed for physics/games, not specifically CAM. No toolpath generation logic. You would build CAM algorithms on top of its primitives.

---

### ncollide2d / ncollide3d (DEPRECATED)
- **Crate**: [ncollide2d](https://crates.io/crates/ncollide2d)
- **Latest version**: 0.33.0 (Mar 2022)
- **Downloads**: 415K
- **Status**: **DEPRECATED** -- superseded by parry2d/parry3d

**Verdict**: Do not use. Use parry instead.

---

### spade
- **Crate**: [spade](https://crates.io/crates/spade)
- **Repository**: https://github.com/Stoeoef/spade
- **Latest version**: 2.15.0 (Aug 2025)
- **Downloads**: 10.8M
- **License**: MIT OR Apache-2.0

**What it provides**: Delaunay triangulations with exact geometric predicates.

**Key features**:
- Delaunay Triangulation (standard 2D)
- Constrained Delaunay Triangulation (CDT) with constrained edges and vertex removal
- Natural neighbor interpolation
- Line intersection traversal across triangulations
- Delaunay refinement
- Barycentric interpolation
- Exact geometric predicate evaluation (prevents precision errors)
- `no_std` compatible, serde support

**Maturity**: Very mature, well-maintained. 10M+ downloads.

**Relevance**: USEFUL for mesh generation, terrain/heightmap interpolation, and surface sampling. CDT is useful for creating meshes from boundary polygons. Natural neighbor interpolation is useful for heightmap smoothing.

**Limitations**: 2D only (for triangulation on a plane).

---

### rstar
- **Crate**: [rstar](https://crates.io/crates/rstar)
- **Repository**: https://github.com/georust/rstar
- **Latest version**: 0.12.2 (Nov 2024)
- **Downloads**: 19.5M
- **License**: MIT OR Apache-2.0

**What it provides**: R*-tree spatial index for n-dimensional data. Optimized for nearest-neighbor queries.

**Key features**:
- R*-tree insertion heuristic
- Nearest neighbor search
- Spatial indexing of arbitrary geometry types
- Works with geo-types (GeoRust ecosystem)
- Arbitrary dimensionality (n-dimensional)
- Bulk loading support
- serde and mint compatibility

**Maturity**: Very mature, part of GeoRust. 19.5M downloads.

**Relevance**: ESSENTIAL for spatial indexing of triangles in STL meshes, points in toolpath grids, and any operation requiring fast nearest-neighbor or range queries. Critical for efficient drop-cutter (finding which triangles a cutter might contact).

**Limitations**: None significant for our use case.

---

### robust
- **Crate**: [robust](https://crates.io/crates/robust)
- **Repository**: https://github.com/georust/robust
- **Latest version**: 1.2.0 (May 2025)
- **Downloads**: 14.5M
- **License**: MIT OR Apache-2.0

**What it provides**: Robust adaptive floating-point predicates for computational geometry (orientation, in-circle tests). Port of Jonathan Shewchuk's predicates.

**Relevance**: USEFUL as a foundation for robust geometric computation. Used internally by geo and spade.

---

## 2. Polygon Boolean Operations & Clipping

### i_overlay (RECOMMENDED)
- **Crate**: [i_overlay](https://crates.io/crates/i_overlay)
- **Repository**: https://github.com/iShape-Rust/iOverlay
- **Latest version**: 4.4.1 (Mar 2026)
- **Downloads**: 2.55M
- **License**: MIT

**What it provides**: High-performance 2D polygon boolean engine.

**Operations**: Union, intersection, difference, XOR, polyline clip/slice, polygon offsetting/buffering.

**Key features**:
- Handles polygons with holes, self-intersections, multiple contours
- Spatial predicates with early-exit optimization (intersects, disjoint, within, covers, touches)
- Fill rules: even-odd, non-zero, positive, negative
- Supports i32, f32, f64 coordinate types
- Fixed-scale and adaptive floating-point modes
- Integrated into the `geo` crate (powers BooleanOps trait)

**Maturity**: Very actively maintained (752 commits, Mar 2026 update). Optimized for large/complex inputs.

**Relevance**: ESSENTIAL. This is the best pure-Rust polygon boolean library. Already integrated into `geo`. Directly applicable to toolpath pocket operations, boundary computation, and polygon clipping.

---

### clipper2-rust (Pure Rust port of Clipper2)
- **Crate**: [clipper2-rust](https://crates.io/crates/clipper2-rust)
- **Repository**: https://github.com/larsbrubaker/clipper2-rust
- **Latest version**: 1.0.1 (Feb 2026)
- **Downloads**: 570

**What it provides**: Complete, feature-complete pure Rust port of the Clipper2 C++ library.

**Operations**:
- Boolean ops: intersection, union, difference, XOR
- Polygon offsetting: inflate/deflate with join types (Miter, Square, Bevel, Round)
- Rectangle clipping
- Minkowski sum and difference
- Path simplification (Ramer-Douglas-Peucker)
- PolyTree hierarchical polygon structures
- Both i64 and f64 coordinate systems

**Maturity**: Claims feature-complete with 444 tests (392 unit + 52 integration). 100% safe Rust. New (Feb 2026), low download count. Created by MatterHackers.

**Relevance**: HIGHLY RELEVANT. Clipper2 is the industry standard for polygon clipping/offsetting in CAM software. Having a pure Rust port is extremely valuable. The offset with Round join type is directly what CAM needs for cutter compensation. However, very new -- needs vetting.

**Limitations**: Brand new, minimal community adoption so far. Needs real-world testing.

---

### clipper2 (C++ wrapper)
- **Crate**: [clipper2](https://crates.io/crates/clipper2)
- **Repository**: https://github.com/tirithen/clipper2
- **Latest version**: 0.5.3 (Jun 2025)
- **Downloads**: 259K

**What it provides**: Rust wrapper around the C++ Clipper2 library. Polygon clipping and offsetting.

**Relevance**: Alternative to clipper2-rust if you prefer battle-tested C++ backend. More mature binding but has C++ dependency.

---

### geo-clipper
- **Crate**: [geo-clipper](https://crates.io/crates/geo-clipper)
- **Repository**: https://github.com/lelongg/geo-clipper
- **Latest version**: 0.9.0 (Feb 2025)
- **Downloads**: 448K

**What it provides**: Boolean operations and polygon offsetting using Clipper (v1) C++ library, with geo-types integration.

**Relevance**: USEFUL as it bridges Clipper with geo-types. However, uses older Clipper v1 (not Clipper2).

---

### geo-booleanop
- **Crate**: [geo-booleanop](https://crates.io/crates/geo-booleanop)
- **Latest version**: 0.3.2 (Jun 2020)
- **Downloads**: 87K

**What it provides**: Martinez-Rueda polygon clipping algorithm implementation.

**Status**: Effectively superseded by i_overlay integration in `geo`. Not actively maintained.

---

## 3. Polygon Offset / Buffer

### geo (Buffer trait)
The `geo` crate now includes a `Buffer` trait for creating offset geometries. This is the easiest path if already using geo-types.

### clipper2-rust / clipper2
Best-in-class polygon offsetting with multiple join types (Round, Miter, Square, Bevel). Round join is what CAM cutter compensation needs.

### geo-offset
- **Crate**: [geo-offset](https://crates.io/crates/geo-offset)
- **Latest version**: 0.4.0 (Feb 2025)
- **Downloads**: 12K

**What it provides**: Margin and padding for geometric shapes. Works with geo-types.

**Relevance**: Simple offset operations. Less capable than Clipper2 for complex polygons.

### geo-buffer
- **Crate**: [geo-buffer](https://crates.io/crates/geo-buffer)
- **Latest version**: 0.2.0 (Jun 2023)
- **Downloads**: 58K

**What it provides**: Inflate/deflate geometric primitives via straight skeleton algorithm.

**Relevance**: Alternative approach to polygon buffering using straight skeletons.

### polygon-offsetting
- **Crate**: [polygon-offsetting](https://crates.io/crates/polygon-offsetting)
- **Latest version**: 0.1.9 (Jun 2024)
- **Downloads**: 10K

### offroad
- **Crate**: [offroad](https://crates.io/crates/offroad)
- **Latest version**: 0.5.7 (Nov 2025)
- **Downloads**: 3.7K

**What it provides**: 2D offsetting specifically for arc polylines/polygons. Handles arcs natively (not just line segments).

**Relevance**: POTENTIALLY USEFUL for toolpaths that include arc segments (G2/G3 moves). 7.8K lines of code with good documentation.

---

## 4. Mesh Processing

### stl_io
- **Crate**: [stl_io](https://crates.io/crates/stl_io)
- **Repository**: https://github.com/hmeyer/stl_io
- **Latest version**: 0.11.0 (Mar 2026)
- **Downloads**: 2.5M

**What it provides**: Read/write both ASCII and binary STL files.

**Maturity**: Well-maintained (Mar 2026 update), 2.5M downloads.

**Relevance**: ESSENTIAL for loading STL models for 3D CAM operations.

---

### nom_stl
- **Crate**: [nom_stl](https://crates.io/crates/nom_stl)
- **Latest version**: 0.2.2 (Mar 2021)
- **Downloads**: 36K

**What it provides**: Fast STL parser using nom. Single-file implementation (662 LOC).

**Status**: Not maintained since 2021. Use stl_io instead.

---

### tobj (OBJ loader)
- **Crate**: [tobj](https://crates.io/crates/tobj)
- **Repository**: https://github.com/Twinklebear/tobj
- **Latest version**: 4.0.3 (Jan 2025)
- **Downloads**: 1.65M

**What it provides**: Lightweight OBJ loader (inspired by tinyobjloader). Supports async loading, f64 mode, mesh merging, index reordering.

**Relevance**: USEFUL for loading OBJ format models.

---

### meshx
- **Crate**: [meshx](https://crates.io/crates/meshx)
- **Repository**: https://github.com/elrnv/meshx
- **Latest version**: 0.7.0 (Apr 2025)
- **Downloads**: 228K

**What it provides**: Mesh exchange library with format conversion. Supports VTK, OBJ, MSH formats. 15K lines of code.

**Relevance**: USEFUL for mesh format conversion.

---

### Half-Edge Mesh Data Structures

| Crate | Version | Downloads | Description |
|-------|---------|-----------|-------------|
| [hedge](https://crates.io/crates/hedge) | 0.2.1 | 20K | Index-based half-edge mesh |
| [half_edge_mesh](https://crates.io/crates/half_edge_mesh) | 1.1.8 | 14K | Basic half-edge mesh data structure |
| [tri-mesh](https://crates.io/crates/tri-mesh) | 0.6.1 | 18K | Triangle mesh with basic operations |
| [plexus](https://crates.io/crates/plexus) | 0.0.11 | 22K | 2D and 3D mesh processing |
| [lox](https://crates.io/crates/lox) | 0.1.1 | 5K | Fast polygon mesh library with multiple data structures |

**Relevance**: Half-edge meshes are useful for mesh traversal operations in CAM (finding adjacent triangles, edge loops, etc.). None of these are very mature. For CAM, parry3d's TriMesh or a custom structure may be more practical.

---

### CSG (Constructive Solid Geometry)

#### csgrs
- **Crate**: [csgrs](https://crates.io/crates/csgrs)
- **Repository**: https://github.com/timschmidt/csgrs
- **Latest version**: 0.20.1 (Jul 2025)
- **Downloads**: 33K

**What it provides**: Full CSG with BSP trees. Union, difference, intersection, XOR. 25+ 2D primitives (including involute gears, airfoils, Bezier, B-splines, text from TTF). 3D primitives (cube, sphere, cylinder, cone, torus, Platonic solids, SDF meshing, triply periodic minimal surfaces). OpenSCAD-like syntax.

**Relevance**: INTERESTING for programmatic model generation and mesh boolean operations. Could be useful for defining workpiece/fixture geometry.

#### boolmesh
- **Crate**: [boolmesh](https://crates.io/crates/boolmesh)
- **Latest version**: 0.1.9 (Feb 2026)
- **Downloads**: 1.3K

**What it provides**: 3D mesh boolean operations. Very new.

---

## 5. Linear Algebra & Math

### nalgebra (RECOMMENDED for CAM)
- **Crate**: [nalgebra](https://crates.io/crates/nalgebra)
- **Repository**: https://github.com/dimforge/nalgebra
- **Latest version**: 0.34.1 (Sep 2025)
- **Downloads**: 59.5M
- **License**: Apache-2.0

**What it provides**: General-purpose linear algebra library.

**Key types**: Vector1-6, Matrix (any size), Point1-6, Rotation2/3, Translation2/3, Isometry2/3, UnitQuaternion, Similarity, Affine/Projective transforms, Perspective3, Orthographic3.

**Features**:
- Static and dynamic matrix sizes
- Compile-time dimension checking
- Matrix factorizations (LU, QR, SVD, Cholesky, etc.)
- Sparse matrix support
- `no_std` compatible

**Maturity**: Extremely mature (59.5M downloads, 121 versions). Same maintainer as parry (Dimforge). Interoperates natively with parry.

**Relevance**: RECOMMENDED as the primary math library. Provides everything needed for 3D transforms, rotations, and linear algebra in CAM. Native interop with parry3d is a significant advantage since parry uses nalgebra types internally.

---

### glam
- **Crate**: [glam](https://crates.io/crates/glam)
- **Repository**: https://github.com/bitshifter/glam-rs
- **Latest version**: 0.32.1 (Mar 2026)
- **Downloads**: 52.9M
- **License**: MIT OR Apache-2.0

**What it provides**: Fast, simple 3D math library. Vec2/3/4, Mat2/3/4, Quat, Affine2/3A. f32 and f64 variants. Integer vectors.

**Key strength**: SIMD-optimized (SSE2, NEON, WASM SIMD). Types like Vec3A and Mat3A use 128-bit SIMD storage. Benchmarked faster than nalgebra for basic operations.

**Maturity**: Very mature (53M downloads). Default math library for Bevy game engine. Used by parry (as internal math type via feature flag).

**Relevance**: USEFUL if performance of basic vector/matrix ops is critical. However, nalgebra's richer type system and direct parry interop make it a better default for CAM. glam is better suited for rendering/visualization code.

**Limitations**: No matrix factorizations, no sparse matrices, limited to 2D/3D/4D.

---

### ultraviolet
- **Crate**: [ultraviolet](https://crates.io/crates/ultraviolet)
- **Latest version**: 0.10.0 (Apr 2025)
- **Downloads**: 915K

**What it provides**: SIMD-focused linear algebra with "wide" types for batched operations.

**Relevance**: NICHE. The "wide" SIMD types could be interesting for batched geometry operations (e.g., testing many triangles against a cutter simultaneously). But lower adoption than nalgebra/glam.

---

### cgmath (DEPRECATED)
- **Crate**: [cgmath](https://crates.io/crates/cgmath)
- **Latest version**: 0.18.0 (Jan 2021)
- **Downloads**: 9M

**Status**: **NOT MAINTAINED** since January 2021. Do not use for new projects. Use nalgebra or glam instead.

---

### Math Library Comparison for CAM

| Feature | nalgebra | glam | ultraviolet |
|---------|----------|------|-------------|
| SIMD optimization | Partial | Excellent | Excellent |
| Matrix factorizations | Yes (LU, QR, SVD...) | No | No |
| Arbitrary dimensions | Yes | No (2D/3D/4D only) | No |
| Type safety | Excellent (compile-time dims) | Good | Good |
| parry3d interop | Native | Via feature flag | No |
| geo ecosystem compat | Via converters | No | No |
| Performance (basic ops) | Good | Best | Very good |
| Maturity/adoption | 59.5M DL | 52.9M DL | 915K DL |

**Recommendation**: Use **nalgebra** as the primary math library for CAM computation (due to parry interop, rich linear algebra, compile-time dimension safety). Use **glam** in visualization/rendering code if using Bevy or wgpu directly.

---

## 6. Parallel Computation

### rayon
- **Crate**: [rayon](https://crates.io/crates/rayon)
- **Repository**: https://github.com/rayon-rs/rayon
- **Latest version**: 1.11.0 (Aug 2025)
- **Downloads**: 333.7M
- **License**: MIT OR Apache-2.0

**What it provides**: Work-stealing data parallelism.

**Key primitives**:
- `par_iter()` / `par_iter_mut()` -- parallel iterators on collections
- `par_sort()` -- parallel sorting
- `join()` -- fork-join parallelism for two tasks
- `scope()` / `scope_fifo()` -- dynamic task spawning
- `ThreadPoolBuilder` -- custom thread pool configuration
- All standard iterator combinators: map, filter, fold, reduce, for_each, etc.

**Relevance for CAM**: ESSENTIAL. Drop-cutter across a grid is a textbook embarrassingly parallel workload. Each grid point can be computed independently. Simply replace `.iter()` with `.par_iter()` on the grid computation. Rayon's work-stealing scheduler automatically balances load across cores.

**Example CAM parallelization opportunities**:
- Drop-cutter: parallelize over grid points or grid rows
- Toolpath offsetting: parallelize over path segments
- STL triangle processing: parallelize over triangles
- Multi-pass roughing: parallelize level computation

**Maturity**: Extremely mature (334M downloads). The de facto standard for data parallelism in Rust.

---

## 7. File Format Crates

### G-code

| Crate | Version | Downloads | Description |
|-------|---------|-----------|-------------|
| [gcode](https://crates.io/crates/gcode) | 0.7.0-rc.1 | 26K | G-code parser, no_std, streaming, visitor pattern. Best G-code parser in Rust. |
| [async-gcode](https://crates.io/crates/async-gcode) | 0.3.0 | 5K | Async G-code parser for no_std targets |
| [gen_gcode](https://crates.io/crates/gen_gcode) | 0.1.0 | 3K | Functional G-code generator (3D printing focus) |
| [nom-gcode](https://crates.io/crates/nom-gcode) | 0.1.1 | 4K | G-code parser using Nom |
| [gcode-nom](https://crates.io/crates/gcode-nom) | 0.6.3 | 7K | G-code visualization/inspection |
| [bulge_gcode](https://crates.io/crates/bulge_gcode) | 0.1.2 | 24 | G-code for polylines with bulge (DXF arcs), WASM |
| [cnccoder](https://crates.io/crates/cnccoder) | 0.2.0 | 4K | G-code generation for GRBL CNC machines + CAMotics simulation |
| [svg2gcode](https://crates.io/crates/svg2gcode) | 0.3.4 | 27K | SVG path to G-code conversion (pen plotters, lasers, CNC) |

**gcode** (Michael-F-Bryan/gcode-rs) is the most mature parser: O(n) streaming, no_std, both allocation-based and zero-allocation visitor APIs, follows NIST G-code spec. 98 GitHub stars, 409 commits.

**svg2gcode** is notable: 393 GitHub stars, active development, converts SVG paths to G1/G2/G3 commands with configurable tolerances. Has CLI tool, web interface, and library crate.

**For G-code generation**: No single dominant library exists. You will likely need to write a custom G-code emitter (it's straightforward: writing formatted text lines like `G1 X10.000 Y20.000 Z-1.500 F500`).

---

### SVG

| Crate | Version | Downloads | Description |
|-------|---------|-----------|-------------|
| [svg](https://crates.io/crates/svg) | 0.18.0 | 5M | SVG composer and parser |
| [usvg](https://crates.io/crates/usvg) | 0.47.0 | 12M | SVG simplification (resolves styles, transforms, etc.) |
| [resvg](https://crates.io/crates/resvg) | 0.47.0 | 10.9M | Full SVG rendering library |

**usvg** is the best choice for reading SVGs: it simplifies the full SVG spec down to a minimal tree of resolved paths. **resvg** renders SVGs to pixels. **svg** is for generating SVG output.

---

### DXF

| Crate | Version | Downloads | Description |
|-------|---------|-----------|-------------|
| [dxf](https://crates.io/crates/dxf) | 0.6.1 | 112K | Read/write DXF and DXB CAD files |

- **Repository**: https://github.com/ixmilia/dxf-rs
- **Updated**: Mar 2026. Actively maintained.
- **Relevance**: ESSENTIAL for importing 2D CAD drawings (profiles, pockets, etc.).

---

### 3D Model Formats

| Crate | Version | Downloads | Description |
|-------|---------|-----------|-------------|
| [stl_io](https://crates.io/crates/stl_io) | 0.11.0 | 2.5M | STL read/write (ASCII + binary) |
| [tobj](https://crates.io/crates/tobj) | 4.0.3 | 1.65M | OBJ loader (lightweight) |
| [obj](https://crates.io/crates/obj) | 0.10.2 | 951K | OBJ loader (not maintained since 2020) |
| [ply-rs](https://crates.io/crates/ply-rs) | 0.1.3 | 1.4M | PLY reader/writer (ASCII + binary) |
| [threemf](https://crates.io/crates/threemf) | 0.8.0 | 27K | 3MF format support |
| [three-d-asset](https://crates.io/crates/three-d-asset) | 0.9.2 | 4.8M | Multi-format loader (glTF, OBJ, STL, PCD) |

**For CAM**: stl_io (STL) + tobj (OBJ) + threemf (3MF) covers the primary 3D model formats.

---

### STEP / IGES (Advanced CAD formats)

| Crate | Version | Downloads | Description |
|-------|---------|-----------|-------------|
| [truck-stepio](https://crates.io/crates/truck-stepio) | 0.3.0 | 12K | STEP I/O for truck CAD kernel |
| [ruststep](https://crates.io/crates/ruststep) | 0.4.0 | 21K | STEP toolkit |
| [iso-10303](https://crates.io/crates/iso-10303) | 0.5.0 | 12K | STEP reader code generation |
| [opencascade](https://crates.io/crates/opencascade) | 0.2.0 | 3.4K | OpenCASCADE Rust bindings (includes STEP) |

**Note**: STEP support in Rust is still immature. For production STEP import, the opencascade-rs bindings are the most capable option but require the C++ OpenCASCADE library.

---

## 8. Visualization & GUI

### egui + eframe (RECOMMENDED for CAM UI)
- **Crate**: [egui](https://crates.io/crates/egui) / [eframe](https://crates.io/crates/eframe)
- **Repository**: https://github.com/emilk/egui
- **Latest version**: 0.33.3 (Dec 2025)
- **Downloads**: egui 14.1M, eframe 10.5M

**What it provides**: Immediate-mode GUI framework. Runs natively (via eframe) and on web (WASM). Includes widgets: buttons, sliders, text inputs, panels, docking, plots, tables, tree views, color pickers, etc.

**Relevance**: RECOMMENDED for the CAM application UI. Immediate-mode is ideal for real-time parameter tweaking (feeds, speeds, step-over). Can embed 3D viewport via wgpu integration. Used by KeloCAM.

---

### wgpu
- **Crate**: [wgpu](https://crates.io/crates/wgpu)
- **Repository**: https://github.com/gfx-rs/wgpu
- **Latest version**: 28.0.0 (Dec 2025)
- **Downloads**: 18.2M

**What it provides**: Cross-platform, safe, pure-Rust WebGPU graphics API. Works on Vulkan, Metal, DX12, OpenGL, and WebGPU (browser).

**Relevance**: ESSENTIAL for custom 3D rendering of toolpaths, STL models, and heightmaps. Can be embedded in egui via eframe's wgpu backend.

---

### bevy
- **Crate**: [bevy](https://crates.io/crates/bevy)
- **Repository**: https://github.com/bevyengine/bevy
- **Latest version**: 0.18.1 (Mar 2026)
- **Downloads**: 4.7M

**What it provides**: Data-driven game engine with ECS architecture, 3D rendering, asset loading, input handling.

**Relevance**: OVERKILL for a CAM application. The ECS architecture adds complexity that isn't needed. Better to use egui + wgpu directly.

---

### kiss3d
- **Crate**: [kiss3d](https://crates.io/crates/kiss3d)
- **Repository**: https://github.com/dimforge/kiss3d
- **Latest version**: 0.40.0 (Jan 2026)
- **Downloads**: 446K

**What it provides**: Simple 3D graphics ("Keep It Simple, Stupid"). Now uses wgpu backend. Supports egui integration. From Dimforge (same as nalgebra/parry).

**Relevance**: USEFUL for quick prototyping and debugging 3D visualization. Much simpler than wgpu directly. Good for visualizing STL models and toolpaths during development.

---

### three-d
- **Crate**: [three-d](https://crates.io/crates/three-d)
- **Repository**: https://github.com/asny/three-d
- **Latest version**: 0.18.2 (Jan 2025)
- **Downloads**: 266K

**What it provides**: 2D/3D renderer with cross-platform support (native + web). Higher-level than wgpu but lower-level than bevy.

**Relevance**: Alternative to kiss3d for 3D visualization.

---

### plotters
- **Crate**: [plotters](https://crates.io/crates/plotters)
- **Repository**: https://github.com/plotters-rs/plotters
- **Latest version**: 0.3.7 (Sep 2024)
- **Downloads**: 138.9M

**What it provides**: 2D plotting/charting library. Supports multiple backends (SVG, PNG, WASM canvas).

**Relevance**: USEFUL for debugging visualizations -- plotting toolpath profiles, heightmaps as 2D images, feed rate graphs, etc.

---

### winit
- **Crate**: [winit](https://crates.io/crates/winit)
- **Latest version**: 0.30.13 (Mar 2026)
- **Downloads**: 34.7M

**What it provides**: Cross-platform window creation. Used by egui/eframe, wgpu, and most Rust graphics applications.

---

## 9. CLI & Configuration

### clap
- **Crate**: [clap](https://crates.io/crates/clap)
- **Repository**: https://github.com/clap-rs/clap
- **Latest version**: 4.6.0 (Mar 2026)
- **Downloads**: 719.6M

**What it provides**: Full-featured CLI argument parser with derive macros, subcommands, shell completions, help generation.

**Relevance**: ESSENTIAL for CLI interface.

---

### serde + toml
- **Crates**: [serde](https://crates.io/crates/serde) (v1.0.228, 876M DL) + [toml](https://crates.io/crates/toml) (v1.0.7, 534M DL)

**What they provide**: Serialization/deserialization framework + TOML config file parsing.

**Relevance**: ESSENTIAL for configuration files (tool libraries, machine configs, job parameters).

---

### Error Handling & Logging

| Crate | Version | Downloads | Description |
|-------|---------|-----------|-------------|
| [anyhow](https://crates.io/crates/anyhow) | 1.0.102 | 587M | Flexible error type for applications |
| [thiserror](https://crates.io/crates/thiserror) | 2.0.18 | 831M | Derive macro for custom error types |
| [log](https://crates.io/crates/log) | 0.4.29 | 774M | Logging facade |
| [tracing](https://crates.io/crates/tracing) | 0.1.44 | 508M | Structured, async-aware tracing/logging |

**Recommendation**: `thiserror` for library error types, `anyhow` for application-level error handling, `tracing` for structured logging with performance timing.

---

## 10. Existing Rust CAM / CNC Projects

### KeloCAM
- **Repository**: https://github.com/lbirkert/KeloCAM
- **Stars**: 9
- **Status**: **PAUSED** -- maintainer lacks resources

**Tech stack**: Rust + egui + wgpu. Intended for hobbyist CNC milling.

**Current state**: Only basic editor UI implemented. No actual CAM operations (no toolpath generation, no G-code output). Roadmap included STL import, toolpath generation, multi-machine support, and real-time monitoring.

**Relevance**: Validates the tech stack choice (egui + wgpu) but provides no reusable CAM logic.

---

### GladiusSlicer
- **Repository**: https://github.com/GladiusSlicer/GladiusSlicer
- **Stars**: 72

**What it does**: FDM 3D printing slicer. Supports perimeters, infill patterns (linear, rectilinear, triangle, cubic), brim/skirt, roof/floor, retraction, STL input.

**Status**: Alpha. Working for basic slicing on Prusa Mk3 and CR10.

**Relevance**: Not directly CAM (it's 3D printing), but demonstrates that Rust can handle the slicing/toolpath-generation workload. Some concepts overlap (slicing STL at Z heights, generating contour paths, infill patterns).

---

### cnccoder
- **Repository**: https://github.com/tirithen/cnccoder
- **Stars**: 11

**What it does**: Programmatic G-code generation for GRBL CNC machines. Defines cutting operations in code rather than from 3D models. Generates CAMotics simulation files.

**Status**: Active but small (v0.2.0, 75 commits). Missing WASM support, text V-carving.

**Relevance**: Demonstrates a code-first CNC approach. Could be studied for G-code generation patterns.

---

### svg2gcode
- **Repository**: https://github.com/sameer/svg2gcode
- **Stars**: 393

**What it does**: Converts SVG vector paths to G-code for pen plotters, laser engravers, and CNC machines. Supports G1/G2/G3, configurable feed rates, tool on/off, arc interpolation.

**Status**: Most mature Rust CNC project (251 commits, active development, Mar 2026 update). Has CLI, web UI, and library interfaces.

**Relevance**: USEFUL reference implementation. The arc fitting (SVG curves to G2/G3) and G-code emission patterns are directly relevant.

---

### cncsim
- **Repository**: https://github.com/Monksc/cncsim
- **Stars**: 14

**What it does**: Simulates G-code from a CNC router, converts to STL or image output.

---

### pcb_forge
- **Repository**: https://github.com/IamTheCarl/pcb_forge
- **Stars**: 7

**What it does**: Generates G-code for rapid prototyping PCBs.

---

### Key Finding
**No existing Rust project implements 3-axis CAM toolpath generation** (drop-cutter, waterline, pocket milling). This is an open niche in the Rust ecosystem. The closest reference implementations are in C++ (OpenCAMLib) and Python (opencamlib Python bindings, pycam).

---

## 11. CAD Kernels in Rust

### Truck
- **Repository**: https://github.com/ricosjp/truck
- **Stars**: 1.4K
- **Commits**: 2,708

**What it is**: B-rep CAD kernel with NURBS. Most mature Rust CAD kernel.

**Modules**: truck-base, truck-geometry (B-splines, NURBS), truck-topology (vertex/edge/wire/face/shell/solid), truck-modeling, truck-shapeops (boolean operations), truck-polymesh (polygon mesh), truck-meshalgo (tessellation), truck-stepio (STEP I/O).

**Relevance**: Could be relevant for importing STEP files and working with B-rep geometry. Not directly CAM-related but could feed geometry to CAM toolpath algorithms.

---

### Fornjot
- **Repository**: https://github.com/hannobraun/fornjot
- **Stars**: Medium
- **Status**: **PAUSED** -- mainline code not developed in over a year, experimenting with alternative approaches.

**What it is**: Early-stage B-rep CAD kernel. Currently only supports very simple models.

**Relevance**: Not usable for production work. Interesting conceptually but not practical.

---

### OpenCASCADE Rust Bindings
- **Repository**: https://github.com/bschwind/opencascade-rs
- **Stars**: 225

**What it provides**: High-level Rust bindings to the C++ OpenCASCADE CAD kernel. Supports fillets, chamfers, lofts, extrusions, revolutions, boolean operations, STEP/STL/SVG/DXF I/O.

**Status**: Work in progress, spare-time project (159 commits).

**Relevance**: USEFUL if you need full parametric CAD kernel capabilities (STEP import, B-rep operations). Requires C++ OpenCASCADE dependency.

---

### vcad
- **Crate**: [vcad](https://crates.io/crates/vcad)
- **Latest version**: 0.1.0 (Jan 2026)
- **Downloads**: 138

**What it provides**: Pure Rust parametric CAD (CSG + B-Rep + tessellation). Exports to glTF, STL, USD, optional STEP via OpenCASCADE.

**Status**: Very early. Minimal adoption.

---

## 12. Additional Useful Crates

### 2D Curves & Paths

#### kurbo
- **Crate**: [kurbo](https://crates.io/crates/kurbo)
- **Repository**: https://github.com/linebender/kurbo
- **Latest version**: 0.13.0 (Nov 2025)
- **Downloads**: 17M

**What it provides**: Comprehensive 2D curves library.

**Curve types**: Line, Arc, CubicBez, QuadBez, Ellipse, Circle, Rect, RoundedRect, Triangle, BezPath (composite).

**Operations**: Arc length, curvature, area, bounding box, nearest point, line intersection, affine transforms, path flattening, curve fitting, stroking, dashing, simplification, **curve offsetting**.

**Relevance**: HIGHLY RELEVANT for toolpath curve operations. Curve offsetting is directly applicable to cutter compensation. Arc length calculation is useful for feed rate normalization. From linebender project (same as resvg/usvg).

---

#### lyon
- **Crate**: [lyon](https://crates.io/crates/lyon)
- **Repository**: https://github.com/nical/lyon
- **Latest version**: 1.0.19 (Mar 2026)
- **Downloads**: 3.5M

**What it provides**: GPU-oriented 2D path tessellation. Sub-crates: lyon_path, lyon_tessellation, lyon_algorithms, lyon_geom.

**Relevance**: USEFUL for converting toolpath curves to triangle meshes for visualization. The path tessellation (fill and stroke) can visualize cut width.

---

### Contour Generation

#### contour
- **Crate**: [contour](https://crates.io/crates/contour)
- **Repository**: https://github.com/mthh/contour-rs
- **Latest version**: 0.13.1 (Apr 2024)
- **Downloads**: 74K

**What it provides**: Isorings and contour polygons via marching squares algorithm.

**Relevance**: RELEVANT for waterline toolpath generation. Slicing a heightmap at Z levels to produce contour paths is a marching-squares operation.

---

### Interpolation

#### enterpolation
- **Crate**: [enterpolation](https://crates.io/crates/enterpolation)
- **Latest version**: 0.3.0 (May 2025)
- **Downloads**: 98K

**What it provides**: Linear interpolation, Bezier curves, B-splines, NURBS.

**Relevance**: USEFUL for smooth toolpath interpolation and curve fitting.

---

### Voronoi Diagrams

| Crate | Version | Downloads | Description |
|-------|---------|-----------|-------------|
| [voronator](https://crates.io/crates/voronator) | 0.2.1 | 135K | Voronoi via Delaunay dual |
| [voronoice](https://crates.io/crates/voronoice) | 0.2.0 | 20K | Fast 2D Voronoi |

**Relevance**: Voronoi diagrams can be useful for medial axis computation (for pocket milling center-line strategies).

---

## 13. Recommendations & Architecture

### Core Stack Recommendation

| Layer | Crate(s) | Purpose |
|-------|----------|---------|
| **Math** | nalgebra | Linear algebra, 3D transforms, points/vectors |
| **2D Geometry** | geo + geo-types | 2D primitives, boolean ops, spatial relations |
| **3D Collision/Query** | parry3d | Ray casting, point projection, shape queries |
| **Spatial Index** | rstar | R*-tree for fast spatial lookups |
| **Triangulation** | spade | Delaunay/CDT triangulation |
| **Polygon Boolean** | i_overlay (via geo) | Union, intersection, difference, offset |
| **Polygon Offset** | clipper2-rust OR geo Buffer | Cutter compensation, pocket boundaries |
| **2D Curves** | kurbo | Bezier/arc operations, curve offset |
| **Parallelism** | rayon | Parallel grid operations |
| **STL I/O** | stl_io | Load/save STL files |
| **DXF I/O** | dxf | Load DXF drawings |
| **G-code parse** | gcode | Parse existing G-code |
| **G-code emit** | Custom | Write G-code output |
| **SVG** | usvg + svg | Read SVG paths, write SVG previews |
| **3MF** | threemf | 3MF format support |
| **OBJ** | tobj | OBJ format support |
| **Contours** | contour | Marching squares for waterline |
| **GUI** | egui + eframe | Application UI |
| **3D Viz** | wgpu (via eframe) | 3D toolpath/model rendering |
| **2D Plot** | plotters | Debug visualization |
| **CLI** | clap | Command-line interface |
| **Config** | serde + toml | Configuration files |
| **Errors** | thiserror + anyhow | Error handling |
| **Logging** | tracing | Structured logging |

### What Must Be Built from Scratch

Since no Rust CAM toolpath generation library exists, these core algorithms must be implemented:

1. **Drop-cutter algorithm** (3-axis surfacing): For each grid point, find the maximum Z where the cutter contacts the STL model. Uses parry3d ray casting and shape queries. Parallelizable with rayon.

2. **Waterline/contour algorithm**: Slice the drop-cutter heightmap at Z levels using the `contour` crate (marching squares), then offset the resulting contours for cutter compensation.

3. **2D pocket milling**: Inward offsetting of pocket boundaries using clipper2-rust or geo Buffer, with zigzag or spiral infill patterns.

4. **Profile/contour toolpaths**: Offset 2D profiles by cutter radius using clipper2-rust or geo Buffer.

5. **G-code emitter**: Generate G0/G1/G2/G3 with feed rates, spindle commands, tool changes.

6. **Toolpath linking/optimization**: Minimize rapid traverse distances between cuts.

### Architecture Notes

- **parry3d's HeightField** shape directly represents the output of a drop-cutter grid
- **parry3d's Cylinder/Ball shapes** can represent flat/ball endmill cutters for contact queries
- **rstar** can index STL triangles for fast spatial lookup during drop-cutter
- **rayon::par_iter** trivially parallelizes the grid computation: `grid_points.par_iter().map(|pt| drop_cutter(pt, mesh))`
- **geo BooleanOps** handles pocket boundary computation (difference of outer boundary minus islands)
- **kurbo** handles arc fitting for G2/G3 output optimization

This is a viable and performant stack. Rust's zero-cost abstractions, memory safety, and rayon's parallelism should yield CAM computation performance competitive with C++ implementations.
