# Tri-Dexel Simulation: Design Document

Replace the 2.5D heightmap simulation with a tri-dexel volumetric
representation that handles multi-directional machining natively.

## Why

The current heightmap (`Vec<f64>` with one Z per cell) can only model cuts
from one direction. `Heightmap::cut(row, col, z)` does `if z < cell { cell = z }`
— strictly top-down. Flipping the stock to cut from the bottom causes the
heightmap to interpret shallow bottom cuts as full-depth top-to-bottom gouges.

A tri-dexel representation stores **segments** along each ray, not just one
Z value. This naturally handles cuts from any direction, through-cuts, and
multi-setup material carry-forward.

## What is a tri-dexel?

Three orthogonal 2D grids of ray segments:

- **Z-grid** (rays along Z, indexed by X,Y) — handles top/bottom cuts
- **X-grid** (rays along X, indexed by Y,Z) — handles left/right cuts
- **Y-grid** (rays along Y, indexed by X,Z) — handles front/back cuts

Each ray stores a list of segments `[(enter, exit), ...]` representing where
material exists. A fresh stock block has one segment per ray spanning the
full stock dimension. Cutting removes material by performing 1D interval
subtraction on each affected ray.

```
Fresh stock (one segment per Z-ray):
  ray[5,3] = [(0.0, 10.6)]     // material from z=0 to z=10.6

After 3mm top cut:
  ray[5,3] = [(0.0, 7.6)]      // top removed

After flip and 2mm bottom cut:
  ray[5,3] = [(2.0, 7.6)]      // bottom also removed

After through-cut:
  ray[5,3] = []                 // empty — hole
```

## 3-Axis Optimizations

For a 3-axis router (tool always vertical), several major simplifications apply:

### Single-segment fast path

When cutting from the top of a solid block, every Z-ray has exactly one
segment, and cutting can only shorten it from the top. This is equivalent
to the current heightmap. Use `SmallVec<[Segment; 1]>` so the common
single-segment case has zero heap allocation.

### Reduced grid usage

| Face being machined | Grids needed |
|---------------------|-------------|
| Top or Bottom | Z-grid only |
| Front or Back | Y-grid only |
| Left or Right | X-grid only |

For the overwhelmingly common top+bottom workflow, only the Z-grid is needed.
The X and Y grids can be created lazily on first use.

### Tool stamp is 2D

Since the tool is always vertical, its cross-section at any Z is a circle.
The stamp operation for a Z-grid ray at (x,y) is:

```
new_z = tool_tip_z + cutter.height_at_radius(dist(x, y, tool_center))
ray.subtract_above(new_z)  // remove material above this Z
```

For bottom cuts, the tool approaches from below:
```
ray.subtract_below(new_z)  // remove material below this Z
```

This reuses the existing `RadialProfileLUT` and cutter profile math.

## Architecture

### Core data types (in rs_cam_core)

```rust
/// A single material segment along a ray.
#[derive(Clone, Copy)]
pub struct DexelSegment {
    pub enter: f32,  // start of material
    pub exit: f32,   // end of material
}

/// One ray's segment list. SmallVec avoids heap for the common 1-segment case.
pub type DexelRay = SmallVec<[DexelSegment; 1]>;

/// A 2D grid of rays along one axis.
pub struct DexelGrid {
    pub rays: Vec<DexelRay>,
    pub rows: usize,
    pub cols: usize,
    pub origin_u: f64,    // grid origin in the first planar axis
    pub origin_v: f64,    // grid origin in the second planar axis
    pub cell_size: f64,
    pub axis: DexelAxis,  // which axis the rays run along
}

pub enum DexelAxis { X, Y, Z }

/// The complete tri-dexel stock representation.
pub struct TriDexelStock {
    pub z_grid: DexelGrid,            // always present
    pub x_grid: Option<DexelGrid>,    // lazy, created for Left/Right cuts
    pub y_grid: Option<DexelGrid>,    // lazy, created for Front/Back cuts
    pub stock_bbox: BoundingBox3,     // in whatever frame the stock lives in
}
```

### Key operations

```rust
impl DexelRay {
    /// Remove material above z (top-down cut). O(n) in segment count.
    fn subtract_above(&mut self, z: f32);

    /// Remove material below z (bottom-up cut). O(n) in segment count.
    fn subtract_below(&mut self, z: f32);

    /// General boolean subtract: remove an interval [a, b]. O(n).
    fn subtract_interval(&mut self, a: f32, b: f32);

    /// Is this ray empty (no material)?
    fn is_empty(&self) -> bool;

    /// Total material length along this ray.
    fn material_length(&self) -> f32;
}

impl TriDexelStock {
    /// Create from stock bounding box.
    fn from_bounds(bbox: &BoundingBox3, cell_size: f64) -> Self;

    /// Stamp a vertical tool at position (cx, cy) with tip at tip_z,
    /// cutting from the given direction.
    fn stamp_tool(
        &mut self,
        cx: f64, cy: f64, tip_z: f64,
        cutter: &dyn MillingCutter,
        direction: CutDirection,  // FromTop or FromBottom
    );

    /// Simulate a full toolpath.
    fn simulate_toolpath(
        &mut self,
        toolpath: &Toolpath,
        cutter: &dyn MillingCutter,
        direction: CutDirection,
    );

    /// Convert to renderable mesh.
    fn to_mesh(&self) -> TriDexelMesh;

    /// Clone the stock state (for checkpoints).
    fn checkpoint(&self) -> Self;

    /// Transform the stock to a different setup's frame.
    /// For cardinal orientations, this is an axis permutation.
    fn transform_to_setup(&self, from: &Setup, to: &Setup, stock: &StockConfig) -> Self;
}
```

### Mesh extraction

Two approaches, in order of recommendation:

**Phase 1 (quick): Heightmap-compatible mesh for Z-grid**

For top/bottom-only workflows, extract a heightmap-compatible mesh from the
Z-grid by reading the top segment exit of each ray. This produces the same
`HeightmapMesh` format the current renderer already supports. Zero GPU
pipeline changes needed.

```rust
fn z_grid_to_heightmap_mesh(grid: &DexelGrid) -> HeightmapMesh {
    // For each ray, use the last segment's exit as the surface Z
    // (same as current heightmap_to_mesh but reading from segments)
}
```

**Phase 2 (full): Contour-tiling mesh for all grids**

For side cuts and complex geometry, use the tri-dexel contour-tiling
algorithm (slice each grid into contours, tile adjacent contours into
triangles). Reference: bernhardmgruber/tridexel C++ implementation.

### Integration with existing code

The replacement is behind a clean interface boundary. The existing consumers:

| Consumer | Current API | New API |
|----------|------------|---------|
| `execute::run_simulation` | `Heightmap::from_bounds` | `TriDexelStock::from_bounds` |
| `execute::run_simulation` | `simulate_toolpath_with_cancel` | `stock.simulate_toolpath` |
| `execute::run_simulation` | `heightmap_to_mesh` | `stock.to_mesh` |
| `execute::run_simulation` | `heightmap.clone()` (checkpoint) | `stock.checkpoint()` |
| `app.rs::update_live_sim` | `simulate_toolpath_range` | `stock.simulate_range` |
| `app.rs::update_live_sim` | `heightmap_to_mesh` | `stock.to_mesh` |
| Backward scrub | `Heightmap::from_bounds` (fresh) | `TriDexelStock::from_bounds` |

The `SimulationRequest` and `SimulationResult` structs change minimally:
- Replace `Heightmap` with `TriDexelStock` in checkpoint storage
- `HeightmapMesh` output format stays the same (or becomes `StockMesh`)
- Cutter trait and implementations are unchanged

### Multi-setup stock carry-forward

After simulating Setup 1 (top cuts), the `TriDexelStock` contains the
remaining material with segments shortened from the top. To simulate
Setup 2 (bottom cuts):

1. The Z-grid segments already represent the remaining material
2. Bottom cuts call `ray.subtract_below(z)` instead of `subtract_above(z)`
3. No grid transformation needed for Top↔Bottom (same Z-grid, different cut direction)

For Front/Back/Left/Right setups, the grid axis changes. The `transform_to_setup`
method permutes axes:
- Top→Front: Z-grid becomes Y-grid (rays along Y instead of Z)
- Top→Left: Z-grid becomes X-grid

This is an O(n) copy with axis remapping, not a full resimulation.

## Implementation Phases

### Phase 1: DexelRay + DexelGrid (core data types)

**Crate:** `rs_cam_core`
**Effort:** ~200 lines

- `DexelSegment`, `DexelRay` with `SmallVec<[Segment; 1]>`
- `subtract_above`, `subtract_below`, `subtract_interval`
- `DexelGrid::from_bounds`, `world_to_cell`, `cell_to_world`
- Unit tests for all segment operations

### Phase 2: TriDexelStock + tool stamping

**Crate:** `rs_cam_core`
**Effort:** ~400 lines

- `TriDexelStock::from_bounds` (Z-grid only initially)
- `stamp_tool` using existing `RadialProfileLUT`
- `stamp_linear_segment` (swept tool along line)
- `simulate_toolpath` and `simulate_range`
- Port existing arc linearization
- Benchmarks comparing to heightmap performance

### Phase 3: Mesh extraction (Z-grid heightmap compat)

**Crate:** `rs_cam_core`
**Effort:** ~100 lines

- `z_grid_to_heightmap_mesh` — read top segment exit per ray
- Produces identical `HeightmapMesh` format
- Colors: same wood-tone depth gradient

### Phase 4: Wire into viz

**Crate:** `rs_cam_viz`
**Effort:** ~200 lines

- Replace `Heightmap` with `TriDexelStock` in:
  - `SimulationRequest` / `SimulationResult`
  - `execute::run_simulation`
  - Checkpoint storage
  - `update_live_sim`
- Keep `HeightmapMesh` as the GPU mesh format (no render changes)
- Remove per-setup frame hacks (stock carries forward naturally)

### Phase 5: Multi-setup carry-forward

**Crate:** `rs_cam_viz`
**Effort:** ~150 lines

- `run_simulation_with_all` simulates setups sequentially, passing
  the `TriDexelStock` from one to the next
- Cut direction derived from `FaceUp`: Top→FromTop, Bottom→FromBottom
- Setup boundaries and checkpoints work as before
- Test with Top+Bottom two-sided job

### Phase 6 (future): Side-face grids + full contour mesh

- Lazy X/Y grid creation for Front/Back/Left/Right
- Contour-tiling mesh extraction for non-Z surfaces
- GPU-accelerated stamping via wgpu compute shaders

## Performance Expectations

| Metric | Current (heightmap) | Phase 2 (dexel, single-segment) | Phase 5 (multi-segment) |
|--------|--------------------|---------------------------------|------------------------|
| Memory (110x110mm @ 0.25mm) | 77 KB (cells) | ~85 KB (SmallVec overhead) | ~170 KB (2 segments avg) |
| Stamp time (6mm endmill) | ~0.05ms | ~0.06ms (segment ops) | ~0.08ms |
| Full sim (10K moves) | ~500ms | ~600ms | ~800ms |
| Mesh extraction | 1ms | 1ms (same format) | 5-10ms (contour tiling) |

Single-segment fast path should be within 20% of current heightmap performance.
Multi-segment adds ~60% overhead but eliminates all frame-switching complexity.

## Key References

- bernhardmgruber/tridexel (C++ reference implementation): github.com/bernhardmgruber/tridexel
- "Surface Reconstruction Using Dexel Data" (ASME 2009): contour-tiling algorithm
- ModuleWorks GPU tri-dexel: industry standard used by Mastercam, Siemens NX
- SmallVec crate: already in rs_cam_viz dependencies (check Cargo.toml)

## Files to Create/Modify

**New files:**
- `crates/rs_cam_core/src/dexel.rs` — DexelSegment, DexelRay, DexelGrid
- `crates/rs_cam_core/src/dexel_stock.rs` — TriDexelStock, stamping, simulation
- `crates/rs_cam_core/src/dexel_mesh.rs` — mesh extraction

**Modified files:**
- `crates/rs_cam_core/src/lib.rs` — add dexel modules
- `crates/rs_cam_core/src/simulation.rs` — keep for backward compat, delegate to dexel
- `crates/rs_cam_viz/src/compute/worker.rs` — SimulationRequest type change
- `crates/rs_cam_viz/src/compute/worker/execute.rs` — use TriDexelStock
- `crates/rs_cam_viz/src/app.rs` — update_live_sim, checkpoints
- `crates/rs_cam_viz/src/controller/events.rs` — simulation submission
- `crates/rs_cam_viz/src/state/simulation.rs` — checkpoint type change
