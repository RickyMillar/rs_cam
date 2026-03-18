# High-Level Architecture

## Design Philosophy

1. **Library-first**: The core is a library crate (`rs_cam`). The CLI is a thin binary crate that calls the library.
2. **Trait-based extensibility**: Tool geometry, operations, dressups, and post-processors are all defined by traits.
3. **Toolpath as intermediate representation**: Operations produce a typed toolpath (not G-code). G-code is a final serialization step.
4. **2D/3D independence**: 2.5D operations use a 2D polygon engine. 3D operations use mesh/drop-cutter. They compose but don't depend on each other.
5. **Performance by default**: Spatial indexing and parallelism are not optional -- they're built into the core algorithms.

---

## Crate Structure

```
rs_cam/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── rs_cam_core/        # Core library (no GUI, WASM-compatible)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── geo/        # Geometry primitives
│   │   │   ├── tool/       # Tool definitions
│   │   │   ├── mesh/       # STL/mesh handling
│   │   │   ├── ops2d/      # 2.5D operations
│   │   │   ├── ops3d/      # 3D operations
│   │   │   ├── toolpath/   # Toolpath IR
│   │   │   ├── dressup/    # Post-generation modifications
│   │   │   ├── gcode/      # G-code emission + post-processors
│   │   │   └── stock/      # Stock modeling
│   │   └── Cargo.toml
│   ├── rs_cam_cli/         # CLI binary
│   │   ├── src/main.rs
│   │   └── Cargo.toml
│   └── rs_cam_viz/         # Optional visualization (egui + wgpu)
│       ├── src/
│       └── Cargo.toml
├── tests/                  # Integration tests
├── fixtures/               # Test STL/SVG/DXF files
└── research/               # This research directory
```

---

## Module Design

### `geo/` -- Geometry Primitives

```rust
// 2D
pub struct Point2 { pub x: f64, pub y: f64 }
pub struct Line2 { pub start: Point2, pub end: Point2 }
pub struct Arc2 { pub start: Point2, pub end: Point2, pub center: Point2, pub clockwise: bool }
pub struct Polyline2 { pub segments: Vec<Segment2> }  // mixed lines + arcs
pub struct Polygon2 { pub outer: Polyline2, pub holes: Vec<Polyline2> }

// 3D
pub struct Point3 { pub x: f64, pub y: f64, pub z: f64 }
pub struct Triangle { pub v: [Point3; 3], pub normal: Point3 }
pub struct BoundingBox3 { pub min: Point3, pub max: Point3 }
```

Uses `nalgebra` for vector math internally. Provides conversions to/from `geo-types` for 2D polygon operations.

### `mesh/` -- Mesh Handling

```rust
pub struct TriangleMesh {
    pub vertices: Vec<Point3>,
    pub triangles: Vec<[u32; 3]>,
    pub normals: Vec<Point3>,       // per-face normals
    pub bbox: BoundingBox3,
}

pub struct SpatialIndex { /* KD-tree or BVH over triangles */ }

pub struct Heightmap {
    pub data: Vec<f32>,
    pub width: usize,
    pub height: usize,
    pub origin: Point2,
    pub resolution: f64,
}

impl TriangleMesh {
    pub fn from_stl(path: &Path) -> Result<Self>;
    pub fn build_spatial_index(&self) -> SpatialIndex;
    pub fn to_heightmap(&self, resolution: f64) -> Heightmap;
    pub fn slice_at_z(&self, z: f64) -> Vec<Polygon2>;
}
```

### `tool/` -- Tool Definitions

```rust
/// The core trait. Every tool type implements this.
pub trait MillingCutter: Send + Sync {
    fn diameter(&self) -> f64;
    fn radius(&self) -> f64 { self.diameter() / 2.0 }
    fn length(&self) -> f64;

    /// Profile height at radial distance r from tool axis
    fn height_at_radius(&self, r: f64) -> f64;

    /// Profile radius at height h above tool tip
    fn width_at_height(&self, h: f64) -> f64;

    /// Key geometric parameters for contact computation
    fn center_height(&self) -> f64;
    fn normal_length(&self) -> f64;
    fn xy_normal_length(&self) -> f64;

    /// Drop-cutter: compute CL Z for this cutter contacting a triangle
    fn vertex_drop(&self, cl: &mut CLPoint, vertex: &Point3);
    fn facet_drop(&self, cl: &mut CLPoint, tri: &Triangle) -> bool;
    fn edge_drop(&self, cl: &mut CLPoint, p1: &Point3, p2: &Point3);

    /// Push-cutter: compute contact interval along a fiber
    fn vertex_push(&self, fiber: &Fiber, interval: &mut Interval, vertex: &Point3);
    fn facet_push(&self, fiber: &Fiber, interval: &mut Interval, tri: &Triangle);
    fn edge_push(&self, fiber: &Fiber, interval: &mut Interval, p1: &Point3, p2: &Point3);
}

// Concrete implementations
pub struct FlatEndmill { diameter: f64, length: f64 }
pub struct BallEndmill { diameter: f64, length: f64 }
pub struct BullNoseEndmill { diameter: f64, corner_radius: f64, length: f64 }
pub struct VBit { diameter: f64, half_angle: f64, length: f64 }
pub struct TaperedBallEndmill { tip_diameter: f64, shaft_diameter: f64, half_angle: f64 }
pub struct CompositeCutter { cutters: Vec<Box<dyn MillingCutter>>, boundaries: Vec<f64> }
pub struct GenericProfile { profile_points: Vec<(f64, f64)> }  // discretized fallback
```

### `toolpath/` -- Toolpath Intermediate Representation

```rust
#[derive(Debug, Clone)]
pub enum MoveType {
    Rapid,                          // G0
    Linear { feed: f64 },          // G1
    ArcCW { center: Point3, feed: f64 },   // G2
    ArcCCW { center: Point3, feed: f64 },  // G3
    Helix { center: Point2, feed: f64 },   // Helical entry
}

#[derive(Debug, Clone)]
pub struct Move {
    pub target: Point3,
    pub move_type: MoveType,
}

#[derive(Debug, Clone)]
pub struct ToolpathSegment {
    pub moves: Vec<Move>,
    pub tool_id: usize,
}

#[derive(Debug, Clone)]
pub struct Toolpath {
    pub segments: Vec<ToolpathSegment>,
    pub metadata: ToolpathMetadata,
}

pub struct ToolpathMetadata {
    pub total_cutting_distance: f64,
    pub total_rapid_distance: f64,
    pub estimated_time: Option<Duration>,
    pub bounding_box: BoundingBox3,
}
```

This IR is the single representation consumed by G-code emitters, visualizers, analyzers, and dressups.

### `ops2d/` -- 2.5D Operations

```rust
/// Common trait for all CAM operations
pub trait CamOperation {
    fn execute(&self, progress: &dyn Fn(f64)) -> Result<Toolpath, CamError>;
}

pub struct PocketOp {
    pub boundary: Polygon2,
    pub tool: Arc<dyn MillingCutter>,
    pub start_depth: f64,
    pub final_depth: f64,
    pub step_down: f64,
    pub step_over: f64,
    pub pattern: PocketPattern,     // Offset, Zigzag, Spiral
    pub entry: EntryStrategy,       // Helix, Ramp, Plunge
    pub safe_z: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
}

pub struct ProfileOp {
    pub contour: Polyline2,
    pub tool: Arc<dyn MillingCutter>,
    pub side: ProfileSide,          // Inside, Outside, OnLine
    pub direction: CutDirection,    // Climb, Conventional
    pub start_depth: f64,
    pub final_depth: f64,
    pub step_down: f64,
    pub tabs: Vec<Tab>,
    // ... safety params
}

pub struct TraceOp { /* follow a vector path at depth */ }
pub struct DrillOp { /* drilling at XY locations */ }
pub struct FaceOp { /* flatten top surface */ }
pub struct VCarveOp { /* V-carving with variable depth */ }
pub struct AdaptiveClearOp { /* constant engagement clearing */ }
```

### `ops3d/` -- 3D Operations

```rust
pub struct DropCutterOp {
    pub mesh: Arc<TriangleMesh>,
    pub spatial_index: Arc<SpatialIndex>,
    pub tool: Arc<dyn MillingCutter>,
    pub step_over: f64,
    pub direction: f64,             // angle in degrees
    pub boundary: Option<Polygon2>, // limit region
    pub min_z: f64,
    pub safe_z: f64,
    pub feed_rate: f64,
    pub adaptive: bool,             // use adaptive sampling
}

pub struct WaterlineOp {
    pub mesh: Arc<TriangleMesh>,
    pub spatial_index: Arc<SpatialIndex>,
    pub tool: Arc<dyn MillingCutter>,
    pub z_start: f64,
    pub z_end: f64,
    pub z_step: f64,
    pub sampling: f64,
    pub adaptive: bool,
}

pub struct HeightmapSurfaceOp {
    pub heightmap: Heightmap,
    pub tool: Arc<dyn MillingCutter>,
    pub step_over: f64,
    // ... alternative to DropCutterOp, faster for large models
}
```

### `dressup/` -- Post-Generation Modifications

```rust
pub trait Dressup {
    fn apply(&self, toolpath: &mut Toolpath) -> Result<(), CamError>;
}

pub struct TabDressup { pub tabs: Vec<TabSpec> }
pub struct RampEntryDressup { pub angle: f64 }
pub struct HelixEntryDressup { pub diameter: f64 }
pub struct LeadInOutDressup { pub radius: f64 }
pub struct DogboneDressup { pub radius: f64 }
pub struct ArcFitDressup { pub tolerance: f64 }
pub struct SimplifyDressup { pub tolerance: f64 }
```

Dressups are applied **after** the operation generates a toolpath. They modify the toolpath in-place. This keeps operations simple and dressups composable.

### `gcode/` -- G-code Emission

```rust
pub trait PostProcessor {
    fn preamble(&self, job: &Job) -> String;
    fn postamble(&self) -> String;
    fn rapid(&self, target: &Point3) -> String;
    fn linear(&self, target: &Point3, feed: f64) -> String;
    fn arc_cw(&self, target: &Point3, center: &Point3, feed: f64) -> String;
    fn arc_ccw(&self, target: &Point3, center: &Point3, feed: f64) -> String;
    fn tool_change(&self, tool_id: usize) -> String;
    fn spindle_on(&self, rpm: u32) -> String;
    fn spindle_off(&self) -> String;
    fn comment(&self, text: &str) -> String;
}

pub struct GrblPost { pub decimal_places: usize }
pub struct LinuxCncPost { pub decimal_places: usize }
pub struct Mach3Post { pub decimal_places: usize }

pub fn emit_gcode(toolpath: &Toolpath, post: &dyn PostProcessor) -> String;
```

### `stock/` -- Stock Modeling

```rust
pub enum StockShape {
    Box { width: f64, height: f64, depth: f64 },
    Cylinder { diameter: f64, height: f64 },
    Mesh(TriangleMesh),
}

pub struct StockModel {
    heightmap: Heightmap,
}

impl StockModel {
    pub fn new(shape: &StockShape, resolution: f64) -> Self;
    pub fn apply_toolpath(&mut self, toolpath: &Toolpath, tool: &dyn MillingCutter);
    pub fn remaining_above(&self, target: &Heightmap, threshold: f64) -> Vec<Polygon2>;
}
```

---

## Data Flow

```
                    ┌──────────────┐
                    │  Input Files │
                    │ STL/SVG/DXF  │
                    └──────┬───────┘
                           │
                    ┌──────▼───────┐
                    │  Model/Mesh  │
                    │  + Spatial   │
                    │    Index     │
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
       ┌──────▼──────┐  ┌─▼──────┐  ┌──▼─────────┐
       │  2.5D Ops   │  │ 3D Ops │  │ V-Carve /  │
       │ Pocket/Prof │  │ Drop/  │  │ Adaptive   │
       │ Drill/Face  │  │ Water  │  │            │
       └──────┬──────┘  └─┬──────┘  └──┬─────────┘
              │            │            │
              └────────────┼────────────┘
                           │
                    ┌──────▼───────┐
                    │   Toolpath   │
                    │     (IR)     │
                    └──────┬───────┘
                           │
                    ┌──────▼───────┐
                    │   Dressups   │
                    │ Tabs/Ramp/   │
                    │ ArcFit/etc   │
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
       ┌──────▼──────┐  ┌─▼──────┐  ┌──▼─────────┐
       │  G-code     │  │  Viz   │  │  Analysis  │
       │  (via Post) │  │ (3D)   │  │ (stats)    │
       └─────────────┘  └────────┘  └────────────┘
```

---

## Key Design Decisions

### D1: nalgebra as primary math, NOT custom Point types

Use `nalgebra::Point3<f64>` and `nalgebra::Vector3<f64>` throughout. This ensures interop with parry3d (same types) and avoids conversion overhead. Provide type aliases:

```rust
pub type P2 = nalgebra::Point2<f64>;
pub type P3 = nalgebra::Point3<f64>;
pub type V2 = nalgebra::Vector2<f64>;
pub type V3 = nalgebra::Vector3<f64>;
```

### D2: geo-types for 2D polygon operations

When interfacing with `geo`/`i_overlay`/`clipper2-rust` for polygon booleans and offsets, convert to/from `geo_types::Polygon`. Keep conversion at the boundary.

### D3: Arc preservation where possible

Use `cavalier_contours` for 2D offset operations that feed into G-code. This preserves arcs through the offsetting process, producing G2/G3 commands instead of thousands of G1 segments. Fall back to `i_overlay`/`clipper2-rust` when arc preservation isn't needed.

### D4: Spatial indexing is mandatory

Every operation that touches the mesh must use a spatial index. The `SpatialIndex` type wraps either a KD-tree (via `kiddo`) or BVH (via `bvh`), chosen at construction time. Default: KD-tree for drop-cutter, BVH for ray queries.

### D5: rayon for parallelism

All embarrassingly parallel operations use `rayon::par_iter()`:
- Drop-cutter grid computation
- Batch push-cutter over fibers
- Heightmap rasterization
- Path simplification per segment

### D6: Toolpath IR is the single source of truth

No operation directly emits G-code. Every operation produces a `Toolpath` (the IR). This enables:
- Dressup composition (tabs, ramp, arc-fit applied in sequence)
- Visualization without G-code parsing
- Analysis (cutting distance, estimated time)
- Post-processor independence

### D7: Operation parameters via builder pattern

```rust
let toolpath = PocketOp::builder()
    .boundary(pocket_polygon)
    .tool(quarter_inch_flat)
    .final_depth(-10.0)
    .step_down(3.0)
    .step_over(3.0)
    .pattern(PocketPattern::Offset)
    .entry(EntryStrategy::Helix { diameter: 4.0 })
    .build()?
    .execute(&|progress| println!("{:.0}%", progress * 100.0))?;
```

### D8: TOML job files for the CLI

```toml
[model]
file = "part.stl"
units = "mm"

[stock]
shape = "box"
width = 200.0
height = 150.0
depth = 25.0

[tools.quarter_flat]
type = "flat"
diameter = 6.35
flutes = 2
cutting_length = 25.0

[tools.eighth_ball]
type = "ball"
diameter = 3.175
flutes = 2
cutting_length = 12.0

[tools.tapered_ball]
type = "tapered_ball"
tip_diameter = 3.175
shaft_diameter = 6.35
taper_angle = 10.0  # degrees per side
cutting_length = 20.0

[[operations]]
type = "adaptive_clear"
tool = "quarter_flat"
step_over_factor = 0.2
step_down = 8.0
final_depth = -24.0
feed_rate = 2000
spindle_rpm = 18000

[[operations]]
type = "drop_cutter"
tool = "tapered_ball"
step_over = 0.3
direction = 0.0  # degrees
feed_rate = 1500
spindle_rpm = 18000

[output]
post_processor = "grbl"
file = "output.nc"
safe_z = 10.0
clearance_z = 5.0
```

---

## Implementation Phases

### Phase 1: Foundation
- [ ] Workspace setup, crate structure
- [ ] nalgebra type aliases, basic geometry primitives
- [ ] STL loading and indexed mesh construction
- [ ] KD-tree / BVH spatial indexing
- [ ] MillingCutter trait + FlatEndmill + BallEndmill implementations
- [ ] Drop-cutter algorithm (vertex, facet, edge tests)
- [ ] BatchDropCutter with rayon parallelism
- [ ] Basic Toolpath IR
- [ ] G-code emitter (G0, G1) with GRBL post-processor
- [ ] CLI skeleton with clap

### Phase 2: 2.5D Operations
- [ ] Polygon offsetting (via cavalier_contours or clipper2-rust)
- [ ] Pocket clearing (offset pattern)
- [ ] Profile cutting with tool radius compensation
- [ ] Zigzag infill pattern
- [ ] Depth stepping
- [ ] SVG/DXF input
- [ ] Helix/ramp entry dressup
- [ ] Tab dressup

### Phase 3: Advanced Tools & 3D
- [ ] BullNoseEndmill, VBit, TaperedBallEndmill implementations
- [ ] CompositeCutter delegation logic
- [ ] Push-cutter algorithm
- [ ] Waterline (Fiber + Weave) algorithm
- [ ] Heightmap-based surface operations
- [ ] Arc fitting dressup (biarc)
- [ ] G2/G3 arc output
- [ ] LinuxCNC and Mach3 post-processors

### Phase 4: High-Value Features
- [ ] Adaptive clearing (constant engagement)
- [ ] V-carving (medial axis + depth computation)
- [ ] Rest machining (stock model difference)
- [ ] Dogbone dressup
- [ ] Lead-in/lead-out dressup
- [ ] TOML job file parsing
- [ ] Feed rate optimization based on engagement

### Phase 5: Visualization & Polish
- [ ] egui + wgpu 3D toolpath viewer
- [ ] Material removal simulation visualization
- [ ] Inlay operations
- [ ] Constant scallop height finishing
- [ ] Pencil finishing
- [ ] GPU-accelerated heightmap operations
- [ ] WASM compilation target

---

## Dependency Graph

```
rs_cam_core
├── nalgebra          (math)
├── geo + geo-types   (2D geometry)
├── i_overlay         (2D booleans, via geo)
├── cavalier_contours (arc-aware offset)
├── clipper2-rust     (polygon offset with round join)
├── parry3d           (3D queries)
├── kiddo             (KD-tree)
├── rstar             (R-tree)
├── stl_io            (STL loading)
├── dxf               (DXF loading)
├── usvg              (SVG loading)
├── rayon             (parallelism)
├── serde + toml      (config)
├── thiserror         (errors)
└── tracing           (logging)

rs_cam_cli
├── rs_cam_core
├── clap              (CLI)
└── anyhow            (error handling)

rs_cam_viz
├── rs_cam_core
├── egui + eframe     (GUI)
└── wgpu              (3D rendering)
```
