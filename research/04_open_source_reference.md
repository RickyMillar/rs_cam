# Open Source CAM Reference

Analysis of existing open-source CAM projects and what to learn from each.

---

## 1. OpenCAMLib (C++, LGPL v2.1)

**Repository**: https://github.com/aewallin/opencamlib
**Architecture**: Library only (no application). Generates CL points; caller converts to G-code.

### What It Implements
- Drop-cutter (PointDropCutter, BatchDropCutter, PathDropCutter, AdaptivePathDropCutter)
- Push-cutter (FiberPushCutter, BatchPushCutter)
- Waterline + AdaptiveWaterline (Fiber/Weave contour extraction)
- 5 cutter types: Flat, Ball, Bull, Cone, Composite (tapered variants)
- KD-tree spatial indexing
- TSP path optimization (via Boost)
- LineCLFilter (co-linearity point reduction)
- ZigZag pattern generation

### Key Architecture Patterns
- **Template Method**: MillingCutter base class defines vertexDrop/facetDrop/edgeDrop; subclasses implement.
- **Operation base class**: Pure virtual run(), configuration via setSTL/setCutter/setSampling.
- **Progressive optimization**: 5 levels from naive to KD-tree + OpenMP.
- **Clean separation**: geo/ (primitives), cutters/ (tools), dropcutter/ (algorithms), algo/ (higher-level).

### What It Does NOT Have
- No 2D polygon operations (no offsetting, no booleans)
- No adaptive/trochoidal clearing
- No arc fitting (only co-linearity filter)
- No rest machining
- No stock modeling
- No G-code generation

### Key Lessons
- The MillingCutter trait with height(r)/width(h) profile functions is the right abstraction
- KD-tree spatial indexing is essential for real-world performance
- The Fiber/Weave approach for waterline is powerful but complex
- Separate CL-point generation from G-code output

---

## 2. libactp / Adaptive2d (C++, GPL)

**Repository**: https://github.com/Heeks/libactp-old (original), lives on in FreeCAD libarea.

### The Adaptive Clearing Algorithm
- Treats cutter as an "agent" making local decisions about direction
- Maintains constant tool engagement (not fixed step-over)
- Iterative angle search: find direction producing target cut area (5% tolerance)
- Helix entry into material
- Multi-pass: discovers remaining uncleared regions automatically
- Path smoothing and chaining for minimal rapids

### Key Constants
```
AREA_ERROR_FACTOR = 0.05
MAX_ITERATIONS = 10
ANGLE_HISTORY_POINTS = 3
MIN_STEP_CLIPPER = 16.0 * 3
```

### Architecture
- Clean Adaptive2d API: tool diameter, step-over factor, tolerance, output is motion-typed paths
- Entirely depends on Clipper for 2D polygon booleans
- 2D only; depth stepping handled externally

### Key Lessons
- Adaptive clearing is the single most valuable advanced algorithm
- A 2D polygon boolean engine is prerequisite
- The agent-based approach (local decisions + area calculation) is elegant
- Progress callbacks are essential

---

## 3. PyCAM (Python, GPL v3)

**Repository**: https://github.com/pycam/pycam

### What It Implements
- Complete pipeline: STL -> toolpath -> G-code
- Drop-cutter raster finishing
- Waterline contouring
- 2D engrave/follow path
- Pocketing with offset contours
- 3 tool types: Spherical, Cylindrical, Toroidal

### Key Lessons
- Shows the minimum viable feature set: model input, tool definition, strategy selection, toolpath generation, G-code output
- Performance is everything -- orders of magnitude slower than C++ (validates Rust choice)
- PathGenerators/PathProcessors separation is useful

---

## 4. FreeCAD CAM Workbench (LGPL v2)

**Repository**: https://github.com/FreeCAD (src/Mod/CAM/)

### Architecture
- 16+ operations with ObjectOp base class (Template Method)
- Feature flags: FeatureTool, FeatureDepths, FeatureHeights, FeatureBaseGeometry, FeatureCoolant
- 2D engine: libarea (Clipper-based)
- 3D engine: OpenCamLib (optional dependency)
- Dressup layer: post-generation modifications (dogbone, ramp entry, tabs, lead-in/out, boundary)
- Post-processor architecture for multi-machine G-code dialects
- ToolBit system: ToolShape (geometry) + ToolBit (instance) + ToolController (runtime binding)

### Operations
- 2.5D: Profile, Pocket, Adaptive, Face, Helix, Slot, Drilling, Engrave, V-Carve, Deburr, Thread
- 3D: Surface (OCL drop-cutter), Waterline (OCL)
- Patterns: Line, ZigZag, Circular, Spiral, Offset

### Key Lessons
- Feature-flag operation system is excellent for extensibility
- Separating 2D/3D engines is pragmatic
- Dressup as a separate post-processing layer is very valuable
- ToolBit/ToolController separation (geometry vs runtime params) is well-designed
- Post-processor architecture is essential for real-world use
- State caching (avoid recomputation) is important for UX

---

## 5. Kiri:Moto (JavaScript, MIT)

**Repository**: https://github.com/GridSpace/grid-apps

### Key Innovation: Heightmap Approach
- Rasterizes 3D model to a Float32Array grid
- Tool profile is also rasterized to an array
- Drop-cutter becomes O(1) grid lookup per point
- WebGPU acceleration for rasterization when available

### Architecture
- 21 operations via CamOp base class and operation registry
- Composition: roughing delegates to area clearing
- Web Workers for background computation
- 4 tool types: ballmill, tapermill, taperball, drill

### Key Lessons
- Heightmap rasterization is valid and fast for 3D surface milling
- Tool profile rasterization is clever (precompute footprint as array)
- GPU acceleration is worth considering for heightmap operations
- Even without adaptive clearing, a useful CAM program can be built

---

## 6. Clipper2 (Boost License)

**Repository**: https://github.com/AngusJohnson/Clipper2

### Why It's Foundation
- Polygon boolean operations: union, intersection, difference, XOR
- Polygon offsetting: inflate/deflate with join types (round, miter, square)
- Integer-based arithmetic for numerical robustness
- Used by virtually all open-source CAM for 2D operations

### Rust Availability
| Crate | Approach | Downloads/month |
|-------|----------|-----------------|
| `i_overlay` | Pure Rust, powers `geo` crate | 475K |
| `clipper2` | C++ wrapper | 94K |
| `clipper2-rust` | Pure Rust Clipper2 port | new |
| `cavalier_contours` | Arc-preserving offset | 391 |

### Key Lessons
- Integer arithmetic eliminates floating-point robustness bugs
- Round-join offsetting is what CAM needs for cutter compensation
- `i_overlay` is the best pure-Rust boolean engine (integrated into geo)
- `cavalier_contours` preserves arcs (better G-code output)

---

## Architecture Pattern Summary

### What Every Project Has
```
Geometry Primitives  ->  Tool Definitions  ->  Algorithms/Operations
                                                        |
                                                Post-Processing
                                                        |
                                                  G-code Output
```

### What the Best Projects Add
- Progress callbacks (FreeCAD, Adaptive2d)
- Operation composition (Kiri:Moto -- roughing reuses area clearing)
- Dressup layer (FreeCAD -- tabs, dogbone, ramp entry)
- Post-processor architecture (FreeCAD -- machine-specific dialects)
- Spatial indexing (OpenCAMLib -- KD-tree, essential for performance)

### The Definitive Lesson from Each

| Project | Primary Lesson |
|---------|---------------|
| OpenCAMLib | Trait-based cutter architecture; KD-tree is essential |
| libactp | Constant engagement > fixed step-over; 2D booleans are prerequisite |
| PyCAM | Complete pipeline matters; Python is too slow |
| FreeCAD | Feature-flag operations; dressup layer; post-processor architecture |
| Kiri:Moto | Heightmap is fast and simple; tool profile rasterization; GPU potential |
| Clipper2 | Integer arithmetic for robustness; polygon offset is the #1 primitive |
