# Requirements

## Functional Requirements

### FR-1: Model Input
- FR-1.1: Load binary and ASCII STL files into an indexed triangle mesh
- FR-1.2: Load SVG files, converting all paths to polyline approximations
- FR-1.3: Load DXF files, extracting LINE, ARC, CIRCLE, POLYLINE, LWPOLYLINE entities
- FR-1.4: Compute mesh bounding box, normals, and basic repair (consistent winding)
- FR-1.5: Generate heightmap from STL mesh at configurable resolution

### FR-2: Tool Definition
- FR-2.1: Define flat end mills (diameter, cutting length, flute count)
- FR-2.2: Define ball end mills (diameter, cutting length, flute count)
- FR-2.3: Define bull nose end mills (diameter, corner radius, cutting length, flute count)
- FR-2.4: Define V-bits / cone cutters (diameter, included angle, cutting length)
- FR-2.5: Define tapered ball end mills (tip diameter, shaft diameter, taper angle)
- FR-2.6: Define tapered flat/bull end mills (composite geometry)
- FR-2.7: Define arbitrary tool profiles via discretized cross-section points
- FR-2.8: All tools implement a common trait with `height(r)` and `width(h)` profile functions
- FR-2.9: Tools store cutting parameters: flute count, max RPM, recommended chip load

### FR-3: 2.5D Operations
- FR-3.1: Pocket clearing with offset-contour and zigzag patterns
- FR-3.2: Profile/contour cutting (inside/outside, climb/conventional)
- FR-3.3: Facing (surface flattening)
- FR-3.4: Drilling (simple, peck)
- FR-3.5: Trace/follow path at specified depth
- FR-3.6: Multi-depth stepping with configurable step-down

### FR-4: 3D Operations
- FR-4.1: Drop-cutter based raster/parallel finishing for all tool types
- FR-4.2: Waterline (Z-level contour) finishing
- FR-4.3: Adaptive sampling (subdivide where surface curvature is high)
- FR-4.4: Support both analytical (exact) and heightmap-based approaches

### FR-5: Advanced Operations
- FR-5.1: Adaptive clearing with constant tool engagement
- FR-5.2: V-carving (depth varies with design width)
- FR-5.3: Rest machining (smaller tool cleans up after larger tool)
- FR-5.4: Inlay generation (matching male/female V-carved pieces)

### FR-6: Dressup / Post-Processing
- FR-6.1: Tabs/bridges (rectangular, triangular, rounded)
- FR-6.2: Lead-in/lead-out arcs
- FR-6.3: Ramp/helix entry (configurable angle, helix diameter)
- FR-6.4: Dogbone fillets on inside corners
- FR-6.5: Arc fitting (convert G1 sequences to G2/G3)
- FR-6.6: Path simplification (Douglas-Peucker within tolerance)

### FR-7: G-code Output
- FR-7.1: Generate valid G-code with G0, G1, G2, G3 commands
- FR-7.2: Support metric (G21) and imperial (G20) units
- FR-7.3: Include safe Z retract before all rapid XY moves
- FR-7.4: Include preamble (modal state init) and postamble (spindle off, return)
- FR-7.5: Post-processor system supporting GRBL, LinuxCNC, Mach3 dialects
- FR-7.6: Configurable decimal precision, line numbering, comment style

### FR-8: CLI Interface
- FR-8.1: Accept job definition via command-line arguments or TOML file
- FR-8.2: Display progress during long operations
- FR-8.3: Output G-code to file or stdout
- FR-8.4: Dry-run mode showing toolpath statistics without generating G-code

---

## Non-Functional Requirements

### NFR-1: Performance
- NFR-1.1: Drop-cutter on a 100K triangle mesh at 0.1mm resolution must complete in under 60 seconds on a modern 8-core CPU
- NFR-1.2: Spatial indexing (KD-tree or BVH) must be used for all mesh queries
- NFR-1.3: Parallelism via rayon for embarrassingly parallel operations (drop-cutter grid, batch push-cutter)
- NFR-1.4: Adaptive sampling to avoid unnecessary computation on flat regions

### NFR-2: Extensibility
- NFR-2.1: New tool types can be added by implementing a trait, not modifying existing code
- NFR-2.2: New operations can be added by implementing a trait
- NFR-2.3: New post-processors can be added by implementing a trait
- NFR-2.4: New dressup modifications can be added without changing the toolpath representation
- NFR-2.5: The library crate exposes a clean public API independent of the CLI

### NFR-3: Correctness
- NFR-3.1: No gouge: the generated toolpath must never cut below the target surface
- NFR-3.2: Polygon boolean operations must use integer arithmetic internally for robustness
- NFR-3.3: Floating-point comparisons must use appropriate epsilon tolerances
- NFR-3.4: All toolpath moves must respect safe Z height
- NFR-3.5: Unit tests for every cutter type's contact geometry (vertex, facet, edge)

### NFR-4: Usability
- NFR-4.1: Clear error messages for invalid input (bad STL, unsupported features)
- NFR-4.2: Sensible defaults for all parameters (step-over, step-down, feed rate)
- NFR-4.3: Progress reporting with ETA for long operations
- NFR-4.4: G-code output includes comments explaining each section

### NFR-5: Architecture
- NFR-5.1: Clean separation between geometry, tools, operations, toolpath, and output
- NFR-5.2: Toolpath intermediate representation is independent of G-code dialect
- NFR-5.3: 2D and 3D algorithm modules are independent
- NFR-5.4: No GUI dependency in the core library
- NFR-5.5: The core library must be compilable to WASM (no OS-specific dependencies)
