# Review: Tri-Dexel Simulation

## Summary

The tri-dexel simulation is a well-designed volumetric material removal system with clean separation between dexel representation, stock operations, mesh extraction, and GUI integration. The architecture supports all 6 cardinal cutting directions through axis-agnostic coordinate decomposition, with lazy grid initialization avoiding memory waste. Mesh extraction produces watertight solids. Multi-setup carry-forward and checkpoint-based scrubbing are correctly implemented. The 88-test suite is solid on core operations but has gaps in tool-type coverage (only Flat/Ball tested in stamping) and no stress/performance tests.

## Findings

### Architecture Review

**Three-layer design:**

1. **Dexel primitives** (`dexel.rs`, 624 LOC) — Ray segments and grid data structures
2. **Stock operations** (`dexel_stock.rs`, ~1515 LOC) — Stamping, simulation, metrics
3. **Mesh extraction** (`dexel_mesh.rs`, 466 LOC) — Renderable mesh from dexel state
4. **Coordinator** (`simulation.rs`, 1132 LOC) — RadialProfileLUT, legacy heightmap, HeightmapMesh

**Key data structures:**

| Type | Location | Purpose |
|------|----------|---------|
| `DexelSegment` | `dexel.rs:14-33` | Single material interval (enter, exit) as f32 |
| `DexelRay` | `dexel.rs:39` | `SmallVec<[DexelSegment; 1]>` — inline for common single-segment case |
| `DexelGrid` | `dexel.rs:150-280` | 2D grid of rays along one axis (rows x cols) |
| `TriDexelStock` | `dexel_stock.rs:72-91` | Z-grid (always) + optional X/Y grids |
| `StockCutDirection` | `dexel_stock.rs:19-68` | 6-variant enum routing cuts to correct grid/axis |
| `RadialProfileLUT` | `simulation.rs:111-176` | 256-sample precomputed cutter profile (indexed by dist_sq) |

### Dexel Correctness

**Ray operations** — All O(n) where n is segment count (typically 1-2 for wood routing):

- `ray_subtract_above(ray, z)` (`dexel.rs:44-59`): Removes material above z. Walks backward for efficiency.
- `ray_subtract_below(ray, z)` (`dexel.rs:64-81`): Removes material below z. Walks forward, handles element shifting.
- `ray_subtract_interval(ray, a, b)` (`dexel.rs:87-113`): General 1D boolean subtraction. Correctly handles 4 cases: no overlap, entirely inside, straddles, partial. Splits segments when interval is interior (line 101).

**SmallVec optimization:** Inline storage for 1 segment (the overwhelmingly common case). Spills to heap only after splits. For 110x110mm stock at 0.25mm: ~193K rays * ~24 bytes = ~4.6 MB per grid.

**Grid construction (lazy):**
- Z-grid always initialized from stock bbox (`dexel.rs:185-200`)
- X-grid created on first FromLeft/FromRight cut (`dexel_stock.rs:130-152`)
- Y-grid created on first FromFront/FromBack cut
- Zero cost for top/bottom-only jobs

**Axis-agnostic coordinate decomposition** (`dexel_stock.rs:59-68`):
- `StockCutDirection::decompose(x, y, z)` remaps global coords to grid-local `(u, v, depth)`
- Z-grid: identity `(x, y, z)`; Y-grid: `(x, z, y)`; X-grid: `(y, z, x)`
- `cuts_from_high_side()` selects `subtract_above` vs `subtract_below`
- All 6 directions work through this single decomposition + direction flag

**Stamping operations:**

- **Point stamp** (`dexel_stock.rs:541-591`): Iterates cells in tool bounding box, looks up radial profile via LUT, subtracts from each ray. O(cells in tool footprint).
- **Linear segment stamp** (`dexel_stock.rs:598-661`): Swept stadium algorithm — for each cell, projects onto segment via parametric `t = clamp(dot / len_sq, 0, 1)`, interpolates Z, looks up profile. Guarantees no gaps.
- **Arc handling** (`dexel_stock.rs:892-926`): Linearizes arcs into sub-segments controlled by cell_size. Linear Z interpolation along arc.

**RadialProfileLUT** (`simulation.rs:111-176`): 256 samples indexed by dist_sq (avoids sqrt per cell). Bilinear interpolation. O(1) lookup with ~1% accuracy vs analytical.

### Mesh Extraction

**Z-grid solid mesh** (`dexel_mesh.rs:51-189`) — three components:

1. **Top face:** `(rows-1)*(cols-1)` quads, CCW winding (normals +Z). Vertices at `ray_top()`. Color: depth-based gradient (uncut tan to cut walnut).
2. **Bottom face:** Same quads, CW winding (normals -Z). Vertices at `ray_bottom()`.
3. **Perimeter skirt:** 4 edge strips connecting top/bottom vertices. Normals face outward.

**Watertight guarantee:** Every boundary edge of top face connects to exactly one wall quad; same for bottom face. Wall quads share edges at corners. Result: manifold, no boundary loops.

**Through-hole handling** (`dexel_mesh.rs:64-65`): Empty rays (all material removed) collapse both top and bottom to `stock_bottom_z`. Creates degenerate zero-area triangles (non-rendering, not a correctness issue).

**Side-grid mesh** (`dexel_mesh.rs:191-266`): Heightmap-style single layer from `ray_top()`. Coordinate remapping: Y-grid `(u=X, v=Z, depth=Y)` -> `(X, Y, Z)`; X-grid `(u=Y, v=Z, depth=X)` -> `(X, Y, Z)`.

**Multi-grid composition** (`dexel_mesh.rs:268-275`): `append_mesh()` reindexes appended vertices with offset. Single combined mesh for GPU.

### Multi-Setup

**Material carry-forward** (in `compute/worker/execute.rs:86-139`):
```
stock = TriDexelStock::from_bounds(...)
for group in groups:           // iterate setups in order
    for toolpath in group:
        stock.simulate_toolpath(..., group.direction, ...)
        checkpoints.push(stock.checkpoint())
```
Stock is **never reset** between setups. Cuts accumulate.

**Coordinate frame transforms** (`state/job.rs:446-487`):
- 6 FaceUp orientations each have a point transform and inverse
- Z-rotation (0/90/180/270) applied after FaceUp
- Toolpaths pre-transformed to global stock frame before simulation (`controller/events.rs:1148-1231`)
- Arc center offsets transformed by linear part only; CW/CCW flipped on reflection (determinant < 0)

**Checkpoint system** for scrubbing:
- `SimCheckpoint` stores `HeightmapMesh` + `Option<TriDexelStock>` at each toolpath boundary
- **Forward scrub:** Simulate additional moves from `live_stock` state. O(moves forward).
- **Backward scrub:** Find highest checkpoint at/before target, clone stock, simulate forward from there. O(1) lookup + O(moves from checkpoint).
- Avoids recomputing entire history on rewind.

### Performance

**Stamp operation complexity:**

| Operation | Cost | Notes |
|-----------|------|-------|
| Point stamp (6mm tool, 0.25mm grid) | ~0.03ms | ~576 cells in footprint |
| Linear segment stamp | ~0.06ms | Closest-point projection per cell |
| Ray subtraction | O(n), n~1-2 | Move, truncate, or split |
| SmallVec spill | ~2x segment cost | Only after first split |

**Full simulation (10K moves):** ~500-600ms single-segment, ~800ms multi-segment. ~20% overhead vs legacy heightmap for top-down only.

**Resolution trade-offs:**

| Resolution | Cells (110x110mm) | Memory/grid | Accuracy |
|------------|-------------------|-------------|----------|
| 0.5mm | 48K | ~1.2 MB | ~0.5mm |
| 0.25mm | 194K | ~4.6 MB | ~0.25mm |
| 0.1mm | 1.2M | ~29 MB | ~0.1mm |

**No parallelism currently** in stamping or mesh extraction (single-threaded). GPU compute mentioned as future work.

### GUI Integration

- `SimulationRequest` assembles per-setup groups with pre-transformed toolpaths (`controller/events.rs:458-536`)
- Playback state in `SimulationPlayback` struct (`state/simulation.rs:217-238`) tracks `current_move`, `live_stock`, `live_sim_move`, `display_mesh`
- Display mesh transformed from global stock frame to active setup's local frame for tool alignment (`app.rs:607-628`)
- Simulation metrics capture: axial/radial engagement, removed volume, MRR, semantic attribution (`dexel_stock.rs:258-386`)

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Low | **f32 precision for dexel segments**: `DexelSegment` uses f32 for enter/exit. For large stocks (>500mm) at fine resolution, this gives ~0.06mm precision — marginal for 0.1mm grid. | `dexel.rs:14-17` |
| 2 | Low | **Through-hole degenerate triangles**: Empty rays produce zero-area triangles in mesh. Harmless for rendering but wastes index space and could confuse mesh analysis tools. | `dexel_mesh.rs:64-65` |
| 3 | Low | **Side-grid mesh not watertight**: Side grids produce open heightmap-style meshes (single layer), not closed solids. Combined mesh has gaps where side meshes meet Z-grid solid. | `dexel_mesh.rs:191-266` |
| 4 | Info | **Legacy heightmap still present**: `simulation.rs:12-109` contains the old single-Z heightmap code. Used for backward compatibility but is dead weight if fully replaced by tri-dexel. | `simulation.rs:12-109` |
| 5 | Info | **No parallelism in stamping**: Single-threaded iteration over cells in tool footprint. Could benefit from rayon or GPU compute for large grids. | `dexel_stock.rs:541-591` |

## Test Gaps

### Coverage Summary

**Total: 88 tests** (verified count)

| Module | Test Count | Coverage |
|--------|-----------|----------|
| `dexel.rs` (ray ops, grid construction) | 32 | Comprehensive |
| `dexel_stock.rs` (stamping, simulation) | 21 | Good, but limited tool types |
| `dexel_mesh.rs` (mesh extraction) | 7 | Adequate |
| `simulation.rs` (legacy heightmap) | 24 | Legacy |
| `simulation_cut.rs` (metrics) | 2 | Minimal |
| `end_to_end.rs` (integration) | 2 | Basic |

### Specific Gaps

1. **Only Flat and Ball tested in dexel stamping** — BullNose, VBit, TaperedBall stamping not tested in `dexel_stock.rs`. The RadialProfileLUT tests exist in `simulation.rs` but actual stamp-through-dexel-grid paths are unvalidated for these tool types.
2. **No volume removal validation tests** — No tests asserting that cumulative material removed equals expected volumes for known geometries.
3. **No segment merging/coalescence tests** — Tests cover splits but not whether adjacent segments merge when cuts create contiguous material.
4. **No cancellation tests** — `simulate_toolpath_with_cancel` exists but no unit tests exercise the cancellation path.
5. **No grid dimension edge cases** — No tests for 1x1, 2x2, or highly asymmetric grids.
6. **No stress/performance tests** — No tests for large grids (1000x1000+) or high move counts.
7. **Metrics layer barely tested** — Only 2 tests for entire SimulationCut data pipeline (serialization only, no MRR/DoC/engagement validation).
8. **No overlapping cut tests** — No tests of toolpath visiting same cell twice or rapid cuts at slight offsets.
9. **Side-grid + Z-grid simultaneous tests limited** — `multi_grid_simulation_preserves_z_grid` verifies independence but no tests verify visual correctness of combined mesh.
10. **Equivalence tests rely on legacy heightmap** — Several dexel_stock tests compare against heightmap output. If heightmap has bugs, dexel tests give false confidence.

## Suggestions

1. **Add BullNose/VBit/TaperedBall stamp tests** to `dexel_stock.rs` — follow existing Flat/Ball patterns.
2. **Add volume validation tests** — Known pocket geometry with calculable removed volume.
3. **Add cancellation tests** — Verify `simulate_toolpath_with_cancel` stops mid-toolpath and state is consistent.
4. **Consider f64 for DexelSegment** if large-stock support becomes important — or document the 500mm practical limit.
5. **Explore rayon parallelism** for stamp_point_on_grid — cells in the tool footprint are independent and could be processed in parallel (need atomic or per-row locking for ray mutations).
6. **Clean up legacy heightmap** if tri-dexel is the sole production path — reduces maintenance burden.
7. **Contour-tiling for side grids** (noted as Phase 6 future work) would improve visual quality at grid junctions.
