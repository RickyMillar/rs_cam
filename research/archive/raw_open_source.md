# Open-Source CAM Libraries & Programs: Comprehensive Research Report

## Table of Contents
1. [OpenCAMLib](#1-opencamlib)
2. [libactp / Freesteel Adaptive Clearing](#2-libactp--freesteel-adaptive-clearing)
3. [PyCAM](#3-pycam)
4. [FreeCAD CAM Workbench](#4-freecad-cam-workbench)
5. [Kiri:Moto](#5-kirimoto)
6. [Clipper / Clipper2 Polygon Library](#6-clipper--clipper2)
7. [Rust-Based CAM & Computational Geometry Libraries](#7-rust-based-cam--computational-geometry-libraries)
8. [Architecture Patterns & Lessons for a Rust CAM System](#8-architecture-patterns--lessons-for-a-rust-cam-system)

---

## 1. OpenCAMLib

**Repository:** https://github.com/aewallin/opencamlib
**License:** LGPL v2.1
**Language:** C++ (core), with Python, Node.js, and WASM bindings
**Status:** Actively maintained, published on PyPI and npm

### Architecture & Core Abstractions

OpenCAMLib is arguably the cleanest open-source CAM library from an architectural perspective. It has a layered design:

```
src/
  geo/          -- Geometry primitives: Point, Line, Arc, Path, Triangle, STLSurf, Bbox
  cutters/      -- Tool geometry: MillingCutter base class + 5 cutter types
  dropcutter/   -- Drop-cutter algorithms (PointDropCutter, BatchDropCutter, etc.)
  algo/         -- Higher-level algorithms: Waterline, AdaptiveWaterline, Weave, Fiber, ZigZag, TSP
  common/       -- Shared utilities
  pythonlib/    -- Python bindings
  nodejslib/    -- Node.js bindings
  emscriptenlib/ -- WASM bindings
```

**Operation base class** (`Operation`):
- Pure virtual `run()` method
- Configuration via `setSTL(const STLSurf& s)`, `setCutter(const MillingCutter* c)`, `setSampling(double s)`
- Support for sub-operations (composite pattern)
- Results returned via `getCLPoints()` or `getFibers()`

This is a **library**, not an application. It generates cutter-location (CL) points; the caller is responsible for converting those into G-code.

### Algorithms Implemented

**Drop-Cutter** (vertical tool positioning):
- Given an (x,y) location, "drops" a cutter along the Z-axis until it contacts the STL surface
- Implemented in `PointDropCutter` (single point) and `BatchDropCutter` (batch processing)
- `BatchDropCutter` has 5 progressive optimization levels:
  1. Naive (tests all triangles)
  2. Kd-tree spatial indexing
  3. Kd-tree + explicit overlap testing
  4. OpenMP multi-threading
  5. Final optimized (default)
- Used to generate parallel raster toolpaths for 3D surface finishing

**Push-Cutter** (horizontal fiber-based):
- Pushes a cutter along a horizontal "Fiber" (line at constant Z) until it contacts the model
- `FiberPushCutter` and `BatchPushCutter` implementations
- Produces intervals along fibers where the cutter is in contact

**Waterline** (constant Z-height contours):
- Creates X-fibers and Y-fibers at a given Z height
- Uses `BatchPushCutter` to find contact intervals on each fiber
- Builds a `Weave` graph from intersecting X and Y fiber intervals
- Extracts closed loops from the weave via planar face traversal
- Result: contour loops at constant Z

**Adaptive Waterline**:
- Extends Waterline with recursive subdivision sampling
- `xfiber_adaptive_sample()` and `yfiber_adaptive_sample()` refine sampling where curvature is high
- Flatness detection (`flat()` predicates) controls when to stop subdividing
- Configurable via `setMinSampling()` and `setCosLimit()`

**Weave Algorithm**:
- Abstract base class with `build()` pure virtual method
- `SimpleWeave` and `SmartWeave` implementations
- Constructs a planar graph (`WeaveGraph`) from fiber intersections
- `face_traverse()` extracts closed loops
- This is the bridge between push-cutter results and waterline contours

**ZigZag**:
- Simple parallel line generation with configurable direction and step-over
- Iterates along perpendicular direction at regular intervals
- Suitable for basic pocketing

**TSP Solver**:
- Uses Boost Graph Library's `metric_tsp_approx()` for toolpath ordering
- Builds complete graph with Euclidean edge weights
- Optimizes rapid-move distances between disconnected path segments

### Tool Geometry Handling

**Class hierarchy:**
```
MillingCutter (abstract base)
  +-- CylCutter     (flat endmill)
  +-- BallCutter    (ball nose)
  +-- BullCutter    (bull nose / toroidal)
  +-- ConeCutter    (tapered / V-bit)
  +-- CompositeCutter (compound geometry)
```

`MillingCutter` defines the template-method interface:
- **Drop-cutter methods:** `vertexDrop()`, `facetDrop()`, `edgeDrop()` -- each cutter type overrides these to compute contact with triangle vertices, faces, and edges
- **Push-cutter methods:** `vertexPush()`, `facetPush()`, `edgePush()` -- horizontal contact computation
- **Geometric queries:** `height(r)` and `width(h)` define the cutter profile curve
- **Key parameters:** `diameter`, `radius`, `length`, `xy_normal_length`, `normal_length`, `center_height`

The `ellipse` and `ellipseposition` support classes handle the mathematical complexity of computing contacts between toroidal/conical cutters and triangle edges.

### STL/Mesh Handling

- `STLSurf`: Simple list of `Triangle` objects (`std::list<Triangle>`) with a bounding box
- `STLReader`: Parses STL files into `STLSurf`
- **No inherent spatial structure** in the surface representation itself
- Spatial acceleration is handled at the algorithm level via **kd-tree** indexing in `BatchDropCutter`
- `Triangle` stores three vertices and a normal
- `CLPoint` (Cutter Location Point): extends Point with contact classification
- `CCPoint` (Cutter Contact Point): records where the cutter touches the surface

### Strengths
- Clean separation of concerns (geometry, tools, algorithms)
- Well-defined abstract interfaces (Operation, MillingCutter, Weave)
- Progressive optimization levels showing the algorithm development path
- Multi-language bindings from a single C++ core
- Template-method pattern for cutter-triangle interaction is elegant
- The Fiber/Weave abstraction for waterline generation is clever and efficient

### Weaknesses
- Limited to drop-cutter and push-cutter paradigms (no adaptive clearing, no pocketing)
- No G-code generation (library only)
- STL surface is just a list of triangles (no half-edge mesh, no topology)
- No stock modeling or material removal simulation
- Weave algorithm is complex and hard to extend
- Depends on Boost heavily
- No 2D polygon operations (no offsetting, no boolean ops)

### Key Lessons for Rust CAM
- The Operation/MillingCutter/STLSurf separation is an excellent architecture to emulate
- Template-method pattern for cutter-geometry interaction (vertexDrop, facetDrop, edgeDrop) maps well to Rust traits
- Fiber/Weave approach is powerful for waterline but could be simplified
- Kd-tree spatial indexing is essential for performance on real STL models
- Progressive optimization (naive -> spatial index -> threading) is a good development strategy
- Consider making the cutter profile a continuous function `height(r) -> z` rather than discrete types

---

## 2. libactp / Freesteel Adaptive Clearing

**Repository:** https://github.com/Heeks/libactp-old
**License:** GPL
**Language:** C++ (90.6%), Python (7.1%), C (2.3%)
**Status:** Archived, last updated 2011. Algorithm lives on inside FreeCAD's libarea.

### Architecture

The Freesteel codebase has an unusual directory structure:

```
freesteel/src/
  bolts/    -- Fundamental data structures: P2 (2D point), P3 (3D point), I1 (1D interval),
               S1 (1D segment), Partition1 (1D partitioning), smallfuncs, debugfuncs
  cages/    -- Surface and path containers: PathX, PathXboxed, SurfX, SurfXboxed,
               S2weave (2D weaving), S1stockcircle, Area2_gen, pathxseries
  pits/     -- Core algorithms: CoreRoughGeneration, toolshape, S2weaveCell,
               S2weaveCellLinearCut, S2weaveCellLinearCutTraverse, S2weaveCircle,
               CircCrossingStructure, SurfXSliceRay, SurfXbuildcomponents,
               NormRay_gen, SLi_gen
```

### How Adaptive Clearing Works

The adaptive clearing algorithm (now maintained as `Adaptive2d` in FreeCAD's libarea) works as follows:

1. **Input:** 2D stock boundary polygon + 2D part boundary polygon + tool diameter
2. **Constant tool engagement:** Rather than fixed step-over, the algorithm dynamically adjusts the cutting direction to maintain a target cutting area (proportional to tool engagement angle)
3. **Iterative angle search:** At each step, it searches for the tool direction angle that produces a cutting area between `minCutArea` and `maxCutArea` thresholds
4. **Entry point discovery:** `FindEntryPoint()` locates valid positions to start cutting, preferring outside-in approaches
5. **Helix ramping:** Tools enter the material via helical ramp paths (configurable diameter and cone angle)
6. **Multi-pass:** After completing accessible regions, the algorithm searches for remaining uncleared material and makes additional passes
7. **Path smoothing:** `SmoothPaths()` reduces point density while preserving critical geometry
8. **Path chaining:** `PopPathWithClosestPoint()` reorders segments to minimize rapid moves

**Key constants from the implementation:**
- `AREA_ERROR_FACTOR = 0.05` (5% tolerance in cut area matching)
- `MAX_ITERATIONS = 10` per angle search
- `ANGLE_HISTORY_POINTS = 3` for directional prediction
- `MIN_STEP_CLIPPER = 16.0 * 3` (minimum step distance)

### The "Agent-Based" Approach

The Freesteel approach treats the cutter as an "agent" that:
- Has a current position and direction
- Samples its environment (calculates cut area at candidate positions)
- Makes local decisions about direction changes
- Attempts to maintain constant engagement through reactive adjustment
- When blocked or finished, seeks new entry points

This is fundamentally different from the "offset contour" approach used by most pocket operations.

### Stock Modeling

The original Freesteel code uses:
- `SurfX` / `SurfXboxed`: Bounded surface representations
- `S2weave`: 2D weave grid for tracking cleared/uncleared areas
- `S2weaveCell`: Cell-based subdivision of the 2D workspace
- Circular crossing structures for tool-boundary intersection

In the modern FreeCAD implementation (`Adaptive2d`), stock is represented as 2D polygon paths processed through Clipper boolean operations. The algorithm maintains a running record of cleared area by subtracting tool-swept regions.

### API Design

The `Adaptive2d` class exposes:
```cpp
class Adaptive2d {
public:
    double toolDiameter = 5;
    double helixRampTargetDiameter = 0;
    double stepOverFactor = 0.2;
    double tolerance = 0.1;
    double stockToLeave = 0;
    bool forceInsideOut = true;
    bool finishingProfile = true;
    double keepToolDownDistRatio = 3.0;
    OperationType opType;  // clearing vs profiling

    std::list<AdaptiveOutput> Execute(
        const DPaths& stockPaths,
        const DPaths& paths,
        std::function<bool(TPaths)> progressCallbackFn
    );
};
```

Output is `AdaptiveOutput` containing: helix center, start point, motion-typed paths (cutting/link-clear/link-not-clear), and return motion type.

### Strengths
- Produces high-quality toolpaths with constant tool engagement
- Reduces tool wear and machining time vs. conventional pocketing
- Clean API for the modern Adaptive2d version
- Progress callback for visualization

### Weaknesses
- Original Freesteel code is hard to read (unusual naming, complex state)
- Only 2D (2.5D with depth stepping handled externally)
- GPL license limits integration options
- No direct 3D surface awareness (relies on 2D projections)
- Heavily dependent on Clipper for polygon operations

### Key Lessons for Rust CAM
- Adaptive clearing is the single most valuable algorithm for practical CNC use
- The "constant engagement" principle is the key insight -- better than fixed step-over
- A 2D polygon boolean engine (like Clipper) is a prerequisite for adaptive clearing
- The agent-based approach (local decisions + area calculation) is elegant
- Progress callbacks are essential for CAM operations that can take minutes
- Separating 2D clearing logic from 3D depth stepping is a good architectural choice

---

## 3. PyCAM

**Repository:** https://github.com/pycam/pycam (main branch)
**License:** GPL v3
**Language:** Python
**Status:** Revived in 2017 after 5-year hiatus, periodic updates

### Architecture Overview

PyCAM is a complete 3-axis toolpath generator:
- Input: STL (3D) or DXF/SVG (2D contours)
- Output: G-code (LinuxCNC compatible)
- GUI: GTK-based

The architecture follows a traditional monolithic Python application with these key modules:
- **Geometry:** Point, Line, Triangle, Plane, Polygon, Model (STL mesh)
- **Cutters:** SphericalCutter, CylindricalCutter, ToroidalCutter
- **PathGenerators:** Drop-cutter, push-cutter based strategies
- **PathProcessors:** Toolpath optimization and filtering
- **Toolpath:** Strategy orchestration

### Toolpath Strategies

PyCAM implements several 3-axis strategies:
1. **Surface (drop-cutter raster):** Parallel lines at constant Z, drop-cutter to find surface contact
2. **Contour/Waterline:** Horizontal slicing at constant Z heights
3. **Engrave:** Follow 2D contour paths
4. **Pocketing:** 2D pocket clearing with offset contours

### Tool Definitions

Three tool types:
- **Spherical** (ball nose)
- **Cylindrical** (flat endmill)
- **Toroidal** (bull nose)

Each defined by diameter and (for toroidal) corner radius.

### Performance Characteristics

- Pure Python with optional multi-processing support
- Extremely slow compared to C++ implementations
- A complex STL surface finish can take hours
- The project acknowledges performance as its primary limitation
- Some acceleration via OpenGL for visualization

### Strengths
- Complete workflow from STL to G-code
- Easy to understand and modify (Python)
- Good reference for what a complete CAM pipeline looks like
- Supports 2D contour input (DXF/SVG) as well as 3D

### Weaknesses
- Very slow (orders of magnitude slower than C++ alternatives)
- Limited to 3-axis
- Basic algorithms (no adaptive clearing)
- GTK dependency makes it hard to deploy
- Limited development activity

### Key Lessons for Rust CAM
- A complete CAM pipeline needs: model input, tool definition, strategy selection, toolpath generation, G-code output
- PyCAM shows the minimum viable feature set for a useful CAM program
- Performance is everything -- Python's limitations here validate the Rust choice
- The separation of PathGenerators and PathProcessors is a useful pattern
- Post-processing (path smoothing, filtering, optimization) is as important as generation

---

## 4. FreeCAD CAM Workbench

**Repository:** https://github.com/FreeCAD/FreeCAD (src/Mod/CAM/)
**License:** LGPL v2
**Language:** C++ (core) + Python (operations, GUI)
**Status:** Actively maintained, large community

### Architecture

FreeCAD CAM has a layered architecture:

```
src/Mod/CAM/
  App/             -- C++ core: Path object, command structure
  Path/
    Op/            -- Python operation implementations
      Base.py      -- ObjectOp base class (template method pattern)
      Adaptive.py  -- Adaptive clearing (uses libarea)
      Surface.py   -- 3D surface (uses OpenCamLib)
      Waterline.py -- Waterline (uses OpenCamLib)
      Profile.py   -- 2D profiling (uses libarea)
      Pocket.py    -- Pocketing (uses libarea)
      Drilling.py  -- Hole operations
      ...16+ operations
    Dressup/       -- Post-generation modifications (dogbone, ramp, tags, etc.)
    Post/          -- Post-processors for various machine dialects
  libarea/         -- Embedded C++ library for 2D operations
    Adaptive.cpp   -- Adaptive2d algorithm
    clipper.cpp    -- Clipper polygon library
    AreaPocket.cpp -- Pocket offset operations
    Area.cpp       -- 2D area operations
  Gui/             -- Qt-based UI
  PathSimulator/   -- Toolpath simulation
```

**Operation Lifecycle** (from `Base.py` / `ObjectOp`):
1. `__init__()` -- Property registration via feature flags
2. `setDefaultValues()` -- Initial configuration
3. `execute()` -- Validates operation, sets up tool, calls `opExecute()`
4. `opExecute()` -- Subclass implements actual toolpath generation
5. Post-processing: dressups, path optimization

**Feature flag system:**
```python
FeatureTool          # Uses a ToolController
FeatureDepths        # Start/final depth
FeatureHeights       # Clearance/safe height
FeatureBaseGeometry  # Operates on selected geometry
FeatureCoolant       # M7/M8/M9 codes
# etc.
```

### Supported Operations

**2.5D Operations:**
- **Profile** -- Edge/face/contour following with offset. Uses libarea for polygon offsetting. Supports CRC (cutter radius compensation).
- **Pocket** -- Material removal from enclosed regions. Envelope-based removal: creates envelope, subtracts model, mills the difference. Uses libarea for offset contour generation.
- **Adaptive** -- Constant-engagement clearing using libarea's Adaptive2d. Projects 3D geometry to 2D, runs adaptive algorithm, applies to multiple Z depths.
- **Face** -- Facing operation (inherits from PocketBase). Boundary strategies: boundbox, stock, perimeter, face region.
- **Helix** -- Helical hole milling
- **Slot** -- Slot cutting along edges
- **Drilling/Tapping** -- Hole operations with peck cycles
- **Engrave/V-Carve** -- Text and decorative cutting
- **Deburr** -- Edge chamfering
- **Thread Milling** -- Internal/external threads

**3D Operations (require OpenCamLib):**
- **3D Surface** -- Full 3D surfacing using OCL drop-cutter. Supports 6 cutting patterns: Line, ZigZag, Circular, CircularZigZag, Spiral, Offset. Uses mesh tessellation with configurable deflection. Supports 4th-axis rotational scanning.
- **Waterline** -- Constant-Z contours using OCL. Three algorithm options: OCL Dropcutter, OCL Adaptive (AdaptiveWaterline), Experimental (non-OCL).

**Dressup Modifications:**
- Dogbone fillets (for inside corners)
- Ramp entry (helical/zigzag/circular)
- Holding tags (tabs)
- Lead in/out
- Boundary constraints
- Z-depth correction (probe-based)

### How Adaptive Clearing Integrates

1. Python `Adaptive.py` projects 3D geometry to 2D paths using `DraftGeomUtils`
2. Creates `area.Adaptive2d()` instance
3. Configures parameters (step-over factor, tolerance, helix settings, etc.)
4. Calls `a2d.Execute(stockPath2d, path2d, progressFn)`
5. Receives motion-typed path segments
6. Generates G-code with helix entry, adaptive cuts, and link moves
7. For multi-depth: `ExecuteModelAware()` handles region-based depth-first or breadth-first strategies

### ToolBit System

FreeCAD has a sophisticated tool management system:
- **ToolShape** -- Geometric definition (endmill, ballnose, v-bit, etc.)
- **ToolBit** -- Instance with specific dimensions
- **ToolBit Library** -- Collection of tools
- **ToolController** -- Runtime binding: tool + spindle speed + feed rates

### Strengths
- Most complete open-source CAM solution
- Excellent integration with parametric 3D modeling
- Modular operation system with clean base class
- Leverages proven external libraries (OpenCamLib, libarea/Clipper)
- Active community and development
- Dressup system for post-generation modifications is powerful
- Post-processor architecture for multi-machine support

### Weaknesses
- Performance limited by Python layer (even with C++ backends)
- 3D operations require optional OpenCamLib dependency
- "Most operations are 2.5D capable" -- true 3D is experimental
- Cannot account for tool shapes in all operations (endmill assumed)
- Complex codebase with legacy migration issues
- libarea's Clipper integration is based on older Clipper v1

### Key Lessons for Rust CAM
- The feature-flag-based operation system is excellent for extensibility
- Separating 2D engines (libarea) from 3D engines (OpenCamLib) is pragmatic
- Dressup/post-processing as a separate layer is very valuable
- ToolBit/ToolController separation (geometry vs. runtime parameters) is well-designed
- Post-processor architecture is essential for real-world use
- A progress callback system is mandatory for interactive use
- State caching (comparing input parameters to avoid recomputation) is important for UX

---

## 5. Kiri:Moto

**Repository:** https://github.com/GridSpace/grid-apps
**License:** MIT
**Status:** Actively maintained, browser-based

### Architecture

Kiri:Moto runs entirely in the browser:

```
src/kiri/
  mode/cam/
    core/
      op.js       -- CamOp base class
      ops.js      -- Operation registry (21 operations)
      tool.js     -- Tool definition system
    work/
      op-rough.js     -- Roughing (delegates to OpArea)
      op-pocket.js    -- Pocketing (delegates to OpArea)
      op-contour.js   -- Contour following (uses Topo generator)
      op-area.js      -- Core clearing algorithm (3 modes: clear, trace, surface)
      op-drill.js     -- Drilling
      op-outline.js   -- Outline profiling
      op-helical.js   -- Helical cutting
      op-lathe.js     -- Lathe operations
      op-level.js     -- Leveling/facing
      slicer-cam.js   -- Z-level slicing engine
      slicer-topo.js  -- Topographic slicing (triangle-plane intersection)
      topo3.js        -- 3D surface milling via heightmap rasterization
      prepare.js      -- Operation preparation
      export.js       -- G-code export
```

### How It Handles 3-Axis Milling

**Z-Level Slicing** (`slicer-cam.js`):
- Slices the 3D model at multiple Z heights
- Extracts 2D polygonal contours at each level
- Operations work on these 2D slices

**Topographic Surface Milling** (`topo3.js`):
- Builds a **heightmap** (Float32Array grid) from the model's triangle mesh
- Optionally uses **WebGPU** for rasterization when available
- A `Probe` class evaluates tool engagement at any XY by sampling the tool profile against the heightmap
- A `Trace` class generates contour polylines by scanning across the grid
- Flatness detection simplifies output where the surface is planar

**Area Clearing** (`op-area.js`) -- Three modes:
1. **Clear:** Progressive inward polygon offsetting (like conventional pocketing)
2. **Trace:** Edge-following for profiling
3. **Surface:** Raster scanning with parallel rays at configurable angle

### Performance Strategy

Kiri:Moto achieves reasonable performance in JavaScript through:
- **Web Workers:** All CAM computation runs in background workers, keeping the UI responsive
- **WebGPU acceleration:** Terrain rasterization offloaded to GPU when available
- **Heightmap approach:** Converts 3D problem to 2D grid sampling (O(1) lookup per point)
- **Float32Array:** Typed arrays for memory efficiency
- **SharedArrayBuffer:** Zero-copy data sharing between workers
- **Progressive simplification:** Flatness detection removes unnecessary points

### Tool Definitions

Four tool types:
- **ballmill** -- Spherical
- **tapermill** -- Conical
- **taperball** -- Taper with ball tip
- **drill** -- With point angle

Parameters: `flute_len`, `flute_diam`, `taper_tip`, `shaft_len`, `shaft_diam`, metric flag.
Tools generate a **rasterized Float32Array profile** at the required resolution for heightmap-based operations.

### Strengths
- Runs anywhere (browser-based, zero install)
- MIT license -- most permissive of all reviewed
- Clean operation registry pattern
- WebGPU acceleration is forward-looking
- Heightmap approach is simple and fast for 3-axis
- Good UX with real-time preview

### Weaknesses
- JavaScript performance ceiling (despite optimizations)
- No adaptive clearing
- Heightmap resolution limits accuracy
- Limited to what's achievable in a browser context
- No true waterline (contour tracing on heightmap is approximate)

### Key Lessons for Rust CAM
- Heightmap/rasterization is a valid and fast approach for 3D surface milling
- The tool profile rasterization idea is clever -- precompute the tool's footprint as an array
- Operation composition (roughing = multiple area operations) is a good pattern
- GPU acceleration for terrain rasterization is worth considering
- The operation registry pattern (string ID -> implementation) is clean
- Even without adaptive clearing, a useful CAM program can be built with basic operations

---

## 6. Clipper / Clipper2

**Repository:** https://github.com/AngusJohnson/Clipper2
**License:** Boost Software License 1.0
**Language:** C++, C#, Delphi
**Status:** Actively maintained

### Why Clipper Matters for CAM

Clipper/Clipper2 is the **foundational polygon library** used by most open-source CAM software. It provides the 2D polygon operations that are prerequisites for:
- Pocket toolpath generation (inward offsetting)
- Profile toolpaths (outward offsetting)
- Adaptive clearing (cut area calculation via boolean ops)
- Stock boundary computation
- Tab/holding tag placement
- 2D boolean operations on slice contours

### Algorithm Foundation

Based on Bala Vatti's polygon clipping algorithm (1992), extensively enhanced:

**Boolean Operations:**
- Intersection, union, difference, XOR
- Complex self-intersecting polygons supported
- Fill rules: EvenOdd, NonZero, Positive, Negative
- Both open paths (polylines) and closed paths (polygons)

**Polygon Offsetting:**
- Inflate/deflate (positive/negative offset)
- Configurable join types (round, square, miter)
- Configurable end types for open paths
- Based on Chen & McMains' "Polygon Offsetting by Computing Winding Numbers"

**Additional:**
- Constrained Delaunay Triangulation
- Z-coordinate tracking through operations

### Precision Strategy

**Integer-based internal calculations** for numerical robustness:
- `Clipper64`: Works with 64-bit integer paths directly
- `ClipperD`: Accepts double-precision input, scales to integers internally
- Coordinate range: +/- 4.6 x 10^18 (62-bit)
- Accuracy degrades beyond +/- 1.0 x 10^15
- User responsible for range checking (performance tradeoff)

### API Design

Two levels:
1. **Simplified API:** `InflatePaths()`, `BooleanOp()` -- single function calls
2. **Full API:** `Clipper64` / `ClipperD` class with `AddSubject()`, `AddClip()`, `Execute()`

Output paths are positive-oriented by default (outer CCW, holes CW).

### Rust Availability

Multiple Rust crates:
- **`clipper2`** (0.5.3, 94K downloads/month) -- C++ wrapper via `clipper2c-sys`, f64 API with i64 internals. "Super early stage" warning.
- **`clipper2-rust`** (1.0.1, 350 downloads) -- Pure Rust port
- **`geo-clipper`** (0.9.0, 41K downloads) -- Boolean ops on `geo` types
- **`i_overlay`** (4.4.1, 475K downloads/month) -- Pure Rust alternative, powers the `geo` crate's boolean ops

### Key Lessons for Rust CAM
- A robust 2D polygon boolean/offset engine is **absolutely essential** -- it's the foundation of most 2.5D CAM operations
- Integer-based arithmetic is key to robustness (floating point will cause failures at scale)
- `i_overlay` is the most mature and performant pure-Rust option (475K downloads/month, used by `geo`)
- The `clipper2` crate wraps C++ which avoids reimplementation but adds build complexity
- Polygon offsetting is the single most important operation for toolpath generation
- Consider using `i_overlay` for booleans + `cavalier_contours` for arc-aware offsetting

---

## 7. Rust-Based CAM & Computational Geometry Libraries

### Directly CAM-Related

| Crate | Downloads/Month | Description | Relevance |
|-------|----------------|-------------|-----------|
| `cnccoder` 0.2.0 | 146 | Programmatic G-code generation for GRBL machines | G-code output reference |
| `gcode` 0.7.0-rc.1 | 78 | No-std G-code parser, O(n), zero-alloc option | G-code parsing |
| `svg2gcode` 0.3.4 | -- | SVG to G-code for plotters/lasers | 2D path to G-code reference |
| `bulge_gcode` 0.1.2 | -- | G-code from polylines with arcs, WASM | Arc-aware G-code |

**Assessment:** No serious Rust CAM library exists. The space is wide open.

### 2D Computational Geometry (Critical for CAM)

| Crate | Downloads/Month | Description | CAM Use |
|-------|----------------|-------------|---------|
| `i_overlay` 4.4.1 | 475K | Boolean ops, offsetting, predicates | **Core 2D engine candidate** |
| `cavalier_contours` 0.7.0 | 391 | Polyline offsetting with arcs, booleans | **Toolpath offsetting** |
| `clipper2` 0.5.3 | 94K | Clipper2 C++ wrapper | Alternative 2D engine |
| `geo` 0.32.0 | 1.08M | Full geospatial primitives + algorithms | Geometry foundation |
| `geo-clipper` 0.9.0 | 41K | Boolean ops on geo types | Integration layer |

### Spatial Indexing (Essential for Performance)

| Crate | Downloads/Month | Description | CAM Use |
|-------|----------------|-------------|---------|
| `rstar` 0.12.2 | 2.9M | R*-tree, n-dimensional | Triangle/shape queries |
| `kiddo` 5.2.4 | 628K | High-performance kd-tree | Drop-cutter acceleration |
| `kdbush` 0.2.0 | 410 | Static 2D point index | Fast point lookups |

### 3D Geometry & Mesh

| Crate | Downloads/Month | Description | CAM Use |
|-------|----------------|-------------|---------|
| `stl_io` 0.11.0 | 390K | STL read/write | Model input |
| `parry3d` | -- | Collision detection, geometric queries | Tool-model intersection |
| `baby_shark` | -- | Mesh processing, volume offsetting, CSG | Stock modeling |
| `csgrs` 0.20.1 | -- | CSG on meshes via BSP trees | Boolean on 3D models |
| `mesh-repair` 0.2.0 | -- | Hole filling, winding consistency | STL cleanup |
| `nalgebra` | huge | Linear algebra | Math foundation |

### DXF/File Format

| Crate | Downloads/Month | Description |
|-------|----------------|-------------|
| `dxf` 0.6.1 | 4.8K | DXF/DXB read/write |
| `stl_io` 0.11.0 | 390K | STL read/write |

### Cavalier Contours -- Deep Dive

This crate deserves special attention for CAM. It's a Rust rewrite of a C++ library specifically designed for CAD/CAM polyline operations:

- **Polylines with true arcs:** Line and arc segments stored with bulge values (not approximated as line segments)
- **Parallel offset:** Preserves arc curvature, handles self-intersections, supports multi-polyline with islands
- **Boolean operations:** Union, intersection, difference on closed polylines
- **Spatial indexing:** Uses `static_aabb2d_index` for high vertex-count optimization
- **FFI bindings:** C FFI available for cross-language use
- **WASM support:** Runs in browser
- **License:** MIT/Apache-2.0

This is the closest thing to a CAM-specific geometry library in Rust.

---

## 8. Architecture Patterns & Lessons for a Rust CAM System

### Common Architectural Patterns Across All Projects

**1. Layered Separation**
Every successful project separates:
- Geometry primitives (points, lines, arcs, polygons, meshes)
- Tool definitions (geometry + cutting parameters)
- Algorithms/operations (drop-cutter, waterline, pocket, adaptive)
- Path post-processing (smoothing, optimization, dressup)
- Output generation (G-code, simulation)

**2. Template Method / Strategy Pattern for Cutter-Geometry Interaction**
OpenCAMLib's approach is definitive: the base cutter class defines `vertexDrop()`, `facetDrop()`, `edgeDrop()` and each cutter type implements these. In Rust, this maps to a `MillingCutter` trait:
```rust
trait MillingCutter {
    fn vertex_drop(&self, cl: &mut CLPoint, vertex: &Point3, tri: &Triangle);
    fn facet_drop(&self, cl: &mut CLPoint, tri: &Triangle);
    fn edge_drop(&self, cl: &mut CLPoint, edge: &Edge, tri: &Triangle);
    fn height_at_radius(&self, r: f64) -> f64;  // cutter profile function
    fn width_at_height(&self, h: f64) -> f64;    // inverse profile
}
```

**3. 2D/3D Split**
FreeCAD and others show that most operations are fundamentally 2D (with Z depth stepping). Only drop-cutter-based surface finishing is truly 3D. This suggests:
- Build a strong 2D polygon engine first (booleans + offsetting)
- Add 3D drop-cutter as a separate module
- Compose 2.5D operations from 2D + depth control

**4. Progress Callbacks**
Every project that handles real workloads implements progress reporting. Essential for:
- User feedback during long operations
- Cancellation support
- Visualization of in-progress paths

**5. Spatial Indexing is Non-Negotiable**
OpenCAMLib's 5x optimization progression (naive -> kd-tree -> kd-tree+overlap -> OpenMP -> optimized) shows that spatial indexing is the difference between "works on test models" and "works on real parts."

### Recommended Rust CAM Architecture

```
rs_cam/
  geo/
    point.rs, line.rs, arc.rs, polyline.rs, polygon.rs  -- 2D primitives
    point3.rs, triangle.rs, mesh.rs, stl.rs              -- 3D primitives
    bbox.rs, spatial_index.rs                             -- Spatial acceleration

  tool/
    mod.rs              -- MillingCutter trait
    endmill.rs          -- Flat endmill
    ballnose.rs         -- Ball nose
    bullnose.rs         -- Bull nose / toroidal
    vbit.rs             -- V-bit / engraving
    profile.rs          -- Rasterized tool profile (Kiri:Moto approach)

  ops2d/
    offset.rs           -- Polygon offsetting (use cavalier_contours or i_overlay)
    boolean.rs          -- Union, intersection, difference (use i_overlay)
    pocket.rs           -- Offset-contour pocketing
    profile.rs          -- 2D profile following
    adaptive.rs         -- Adaptive clearing (constant engagement)

  ops3d/
    dropcutter.rs       -- Drop-cutter algorithm
    pushcutter.rs       -- Push-cutter / fiber algorithm
    waterline.rs        -- Waterline from fibers + weave
    surface.rs          -- 3D surface finishing (raster/spiral/etc.)
    heightmap.rs        -- Heightmap-based surface operations (Kiri:Moto approach)

  depth/
    stepping.rs         -- Z-level iteration
    ramping.rs          -- Helix/zigzag/ramp entry

  path/
    toolpath.rs         -- Typed path segments (rapid, feed, arc, helix)
    optimizer.rs        -- Path ordering (TSP), rapid minimization
    smoother.rs         -- Point reduction, arc fitting
    dressup.rs          -- Dogbone, tabs, lead-in/out

  gcode/
    parser.rs           -- G-code parsing (or use gcode crate)
    emitter.rs          -- G-code generation
    postprocessor.rs    -- Machine-specific dialect translation

  stock/
    model.rs            -- Stock definition (box, cylinder, mesh)
    simulation.rs       -- Material removal simulation
```

### Critical Dependencies for Rust CAM

| Need | Recommended Crate | Alternative |
|------|-------------------|-------------|
| 2D boolean operations | `i_overlay` | `clipper2` |
| 2D polyline offsetting with arcs | `cavalier_contours` | `i_overlay` offset |
| STL reading | `stl_io` | `nom_stl` |
| Spatial indexing (kd-tree) | `kiddo` | `rstar` |
| Spatial indexing (R-tree) | `rstar` | -- |
| Linear algebra | `nalgebra` or `glam` | -- |
| G-code parsing | `gcode` | custom |
| DXF reading | `dxf` | -- |
| Mesh processing | `baby_shark` | `parry3d` |

### Algorithm Priority for Implementation

Based on practical value and what existing projects teach us:

1. **Polygon offsetting** (prerequisite for everything)
2. **2D pocket clearing** (offset contours, most common operation)
3. **2D profiling** (edge following with offset)
4. **Drop-cutter** (3D surface finishing)
5. **Adaptive clearing** (highest-value advanced feature)
6. **Waterline** (3D contouring)
7. **Drilling** (simple but essential)
8. **Path optimization** (TSP for rapid minimization)
9. **Ramp/helix entry** (dressup)
10. **Heightmap surface** (alternative to drop-cutter, potentially faster)

### Key Design Decisions

**Integer vs. Float for 2D operations:**
Following Clipper2's lead, use integer arithmetic internally for 2D polygon operations. This eliminates an entire class of numerical robustness bugs. Accept f64 input, scale to i64, compute, scale back.

**Arc preservation:**
`cavalier_contours` preserves arcs through offsetting operations, which produces better G-code (G2/G3 arcs instead of thousands of tiny line segments). This is a significant quality advantage.

**Trait-based tool abstraction:**
Define cutters as traits with `height_at_radius(r) -> z` and `width_at_height(h) -> r` profile functions. This allows arbitrary tool profiles without enum matching.

**Operation as trait with builder pattern:**
```rust
trait CamOperation {
    type Output;
    fn execute(&self, progress: &dyn Fn(f64)) -> Result<Self::Output, CamError>;
}
```

**Separate toolpath representation from G-code:**
Use a typed toolpath intermediate representation (rapid moves, feed moves, arcs, helixes) that can be post-processed into any G-code dialect. FreeCAD's approach here is correct.

### What Each Project Teaches Us

| Project | Primary Lesson |
|---------|---------------|
| OpenCAMLib | Clean trait-based cutter/operation architecture; kd-tree is essential |
| libactp/Adaptive | Constant engagement > fixed step-over; 2D polygon engine is prerequisite |
| PyCAM | Complete pipeline matters; Python is too slow (validates Rust choice) |
| FreeCAD CAM | Feature-flag operations; dressup layer; post-processor architecture |
| Kiri:Moto | Heightmap approach is fast and simple; tool profile rasterization; GPU potential |
| Clipper2 | Integer arithmetic for robustness; polygon offset is the #1 primitive |
| cavalier_contours | Arc-aware offsetting produces superior output |
