# Tri-Dexel Stock Simulation

## The Problem: Why 2.5D Heightmaps Break on Multi-Setup Jobs

rs_cam models CNC stock removal using a **heightmap**: a 2D grid where each
cell stores a single Z value representing the height of remaining material
at that point. The simulation kernel is one line:

```rust
if z < cell { cell = z; }
```

This works perfectly for single-setup machining where the tool always
approaches from above. A 6mm endmill plunging 3mm into a 10mm stock block
changes the cell from 10.0 to 7.0. Simple, fast, correct.

But CNC woodworking regularly requires **multi-setup machining** — flipping
the stock to cut from different faces. A two-sided terrain carving has
Setup 1 cutting from the top and Setup 2 cutting from the bottom. When we
try to simulate both setups on one heightmap, the bottom cut gets
misinterpreted:

```
Setup 1 (top, 3mm deep):  cell goes from 10.0 → 7.0  ✓
Setup 2 (bottom, 2mm deep):
  After inverse-transform to global: tool_z ≈ 2.0
  Heightmap: if 2.0 < 7.0 → cell = 2.0  ← WRONG
  This removed 5mm from the top, not 2mm from the bottom!
```

The heightmap has no concept of "which side the tool came from." It only
knows "material exists below this Z." A bottom cut that removes material
from z=0 up to z=2 cannot be represented — the heightmap interprets it as
"cut everything from the top down to z=2."

This is a **fundamental data structure limitation**, not a transform bug.

## The Solution: Tri-Dexel Representation

Instead of storing one Z per cell, store a **list of material segments**
along each ray. A segment is a pair `(enter, exit)` representing where
material begins and ends along that ray.

### How Segments Work

A fresh 10mm stock block has one segment per ray:

```
ray = [(0.0, 10.0)]    ← material from z=0 to z=10
```

A 3mm cut from the top shortens the top segment:

```
ray = [(0.0, 7.0)]     ← top 3mm removed
```

A 2mm cut from the bottom shortens the bottom segment:

```
ray = [(2.0, 7.0)]     ← bottom 2mm also removed, 5mm of material remains
```

A through-cut removes all material:

```
ray = []                ← hole
```

A deep pocket that doesn't break through might create multiple segments:

```
ray = [(0.0, 3.0), (7.0, 10.0)]  ← island of material with air gap
```

Each ray independently tracks where material exists. The segment operations
are simple 1D interval arithmetic — no matrices, no transforms, no frame
confusion.

### Why "Tri-Dexel" (Three Grids)?

A single grid of Z-rays handles top and bottom cuts (both along the Z axis).
But if you need to cut from the front, back, left, or right, the tool
approaches along a different axis. A Z-ray can't efficiently represent a
horizontal cut from the front.

The solution is three orthogonal grids:

```
Z-grid: rays along Z, indexed by (x, y) → handles top/bottom
Y-grid: rays along Y, indexed by (x, z) → handles front/back
X-grid: rays along X, indexed by (y, z) → handles left/right
```

Together, these three grids can represent the stock from any of the six
cardinal directions that a 3-axis router uses.

### 3-Axis Router Optimization

For a 3-axis wood router, the tool is always vertical. This gives us a
major simplification: **you only need the grid whose axis matches the
tool's approach direction.**

| Setup Face | Tool approaches from | Grid needed |
|-----------|---------------------|-------------|
| Top       | above (+Z)          | Z-grid      |
| Bottom    | below (-Z)          | Z-grid      |
| Front     | front (+Y)          | Y-grid      |
| Back      | back (-Y)           | Y-grid      |
| Left      | left (+X)           | X-grid      |
| Right     | right (-X)          | X-grid      |

The overwhelmingly common case for wood routing is **top + bottom** (two
setups). This only needs the Z-grid — which is essentially the current
heightmap with segment lists instead of single values. The X and Y grids
can be created lazily if the user adds side-face setups.

### Performance: SmallVec Fast Path

For single-setup top-down machining, every Z-ray has exactly one segment
(material can only be shortened from the top, never split). We use Rust's
`SmallVec<[Segment; 1]>` type, which stores up to one segment inline (on
the stack / in the Vec element) without any heap allocation. This means:

- **Single-setup performance:** Within 20% of the current heightmap.
  The segment subtract operation is slightly more work than a simple
  `min()`, but the memory layout is nearly identical.

- **Multi-setup performance:** When segments split (through-cuts, multi-side
  machining), SmallVec spills to the heap. This costs ~60% more per stamp,
  but eliminates all the coordinate frame complexity that currently makes
  multi-setup simulation impossible.

### The Key Advantage: Multi-Setup Carry-Forward

After simulating Setup 1 (top cuts), the Z-grid contains segments shortened
from the top. To simulate Setup 2 (bottom cuts):

1. Use the **same Z-grid** (it already represents the remaining material)
2. Call `ray.subtract_below(z)` instead of `subtract_above(z)`
3. No coordinate transforms. No separate heightmaps. No frame switching.

The stock state naturally carries forward between setups. Through-cuts are
detected automatically (empty rays). Remaining material from any setup is
visible to subsequent setups.

For side-face setups (Front/Back/Left/Right), the axis changes. But this
is just an axis permutation — the Z-grid data is reinterpreted as an X or
Y grid with an O(n) copy.

## What This Replaces

The tri-dexel stock replaces the `Heightmap` struct and its associated
functions in `rs_cam_core::simulation`:

| Current | Replacement |
|---------|-------------|
| `Heightmap` (Vec<f64>, one Z per cell) | `TriDexelStock` (Vec<SmallVec<Segment>>, segments per ray) |
| `Heightmap::cut(row, col, z)` | `DexelRay::subtract_above(z)` / `subtract_below(z)` |
| `stamp_tool_at_lut()` | `TriDexelStock::stamp_tool()` |
| `stamp_linear_segment_lut()` | `TriDexelStock::stamp_linear_segment()` |
| `simulate_toolpath_with_cancel()` | `TriDexelStock::simulate_toolpath()` |
| `heightmap_to_mesh()` | `TriDexelStock::to_mesh()` |

The cutter trait (`MillingCutter`), cutter implementations (flat, ball,
bull nose, V-bit, tapered ball), the `RadialProfileLUT`, and arc
linearization are all reused unchanged. The tool profile math is the same
— only the "how to apply the profile to the stock" changes.

The rendering pipeline is also unchanged for the initial implementation.
`to_mesh()` produces the same `HeightmapMesh` format (flat Vec<f32>
vertices + Vec<u32> indices + Vec<f32> colors). The GPU pipeline, shaders,
and color modes (solid, deviation, by-height) all work as-is.

## What This Enables

### Immediate (Phases 1-5)

- Two-sided machining simulation without gouging
- Stock carry-forward between setups (remaining material visible)
- Through-cut detection (empty rays)
- Correct tool position tracking across setup boundaries
- Single "Run Simulation" button that processes all setups in sequence

### Future (Phase 6+)

- Side-face machining simulation (Front/Back/Left/Right setups)
- Full contour-tiling mesh extraction for arbitrary cut geometry
- GPU-accelerated stamping via wgpu compute shaders
- Remaining stock visualization in Setup workspace
- In-process stock for rest-material-aware toolpath generation
- Undercut detection

## Industry Context

This approach matches what commercial CAM software uses:

- **ModuleWorks** (simulation engine for Mastercam, Siemens NX, many others):
  uses tri-dexel with GPU acceleration
- **MecSoft CAM 2026** (RhinoCAM, VisualCAD/CAM): GPU tri-dexel, reports
  450% speedup over polygonal simulation on RTX 3060
- **Fusion 360**: uses "in-process stock" (IPS) that propagates material
  state across setups — same concept as our carry-forward

The tri-dexel is the industry standard for 3-axis simulation. It's simpler
than full voxel/octree (which is needed for 5-axis), and much more capable
than the 2.5D heightmap.

## Implementation Plan Summary

Six phases, each independently testable. Phases 1-3 are pure core library
work with no GUI changes. Phase 4 swaps the backend. Phase 5 enables
multi-setup. Phase 6 is future work.

| Phase | Scope | Crate | Lines | Depends on |
|-------|-------|-------|-------|------------|
| 1 | DexelRay + DexelGrid data types | rs_cam_core | ~200 | nothing |
| 2 | TriDexelStock + tool stamping | rs_cam_core | ~400 | Phase 1 |
| 3 | Mesh extraction (HeightmapMesh compat) | rs_cam_core | ~100 | Phase 2 |
| 4 | Wire into viz (replace Heightmap) | rs_cam_viz | ~200 | Phase 3 |
| 5 | Multi-setup carry-forward | rs_cam_viz | ~150 | Phase 4 |
| 6 | Side grids + contour mesh + GPU | both | TBD | Phase 5 |

Total new code for Phases 1-5: approximately 1,050 lines of Rust.

See `planning/VOXEL_SIM_DESIGN.md` for the detailed implementation plan
with struct definitions, API signatures, and per-phase task breakdowns.
