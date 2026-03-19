# Performance Review — rs_cam

Comprehensive performance audit of the rs_cam Rust CAM codebase. Covers algorithms,
data structures, parallelism, SIMD, caching, memory patterns, compiler hints, and
benchmarking infrastructure.

**Codebase size:** ~14,500 lines of core library code across 20+ modules.
**Existing perf baseline:** 196K triangles, 108K grid points in 0.18s release build.

---

## Table of Contents

1. [Critical: Immediate Wins](#1-critical-immediate-wins)
2. [High: Algorithm & Data Structure Improvements](#2-high-algorithm--data-structure-improvements)
3. [Medium: Parallelism Opportunities](#3-medium-parallelism-opportunities)
4. [Medium: Compiler Optimization Hints](#4-medium-compiler-optimization-hints)
5. [Medium: Lookup Tables & Precomputation](#5-medium-lookup-tables--precomputation)
6. [Low: Memory & Allocation Patterns](#6-low-memory--allocation-patterns)
7. [Low: Async & I/O](#7-low-async--io)
8. [Infrastructure: Benchmarking Gaps](#8-infrastructure-benchmarking-gaps)
9. [Summary Matrix](#9-summary-matrix)

---

## 1. Critical: Immediate Wins

### 1.1 Spatial Index Deduplication is O(n²)

**File:** `mesh.rs:170-186`

The `SpatialIndex::query()` method uses `Vec::contains()` for triangle deduplication —
an O(n) linear scan per triangle, making the overall dedup O(n²).

```rust
let mut seen = Vec::new(); // comment says "could use a bitset for large meshes"
for &tri_idx in &self.cells[cell_idx] {
    if !seen.contains(&tri_idx) {  // O(n) per check!
        seen.push(tri_idx);
        result.push(tri_idx);
    }
}
```

**Impact:** Called ~108K times per drop-cutter grid. With 4-8 triangles per query,
each query does 16-64 comparisons instead of O(1).

**Fix:** Replace with a bitset (best) or HashSet:
```rust
let mut seen = vec![false; self.total_triangles];
if !seen[tri_idx] {
    seen[tri_idx] = true;
    result.push(tri_idx);
}
```

**Estimated speedup:** 1.5-2x for dense mesh queries (5-10x theoretical for very dense).

---

### 1.2 Replace `.powi(2)` with Direct Multiplication

**Files:** 14 locations across `adaptive.rs`, `arcfit.rs`, `dressup.rs`, `waterline.rs`, `viz.rs`

`.powi(2)` uses generic exponentiation; `x * x` is 2-3x faster for squaring.

```rust
// Bad (14 occurrences):
let dist = ((entry.x - last.x).powi(2) + (entry.y - last.y).powi(2)).sqrt();

// Good:
let dx = entry.x - last.x;
let dy = entry.y - last.y;
let dist = (dx * dx + dy * dy).sqrt();
```

**Locations:**
- `adaptive.rs:951, 1132, 1692`
- `arcfit.rs:158, 202`
- `dressup.rs:80-81, 242, 691, 802, 900, 1003`
- `waterline.rs:287`
- `viz.rs:369`

**Estimated speedup:** 5-10% in distance-heavy code paths.

---

### 1.3 Monomorphize Drop-Cutter (Eliminate Dynamic Dispatch)

**File:** `dropcutter.rs:11-27`

```rust
pub fn point_drop_cutter(
    x: f64, y: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,  // ← virtual dispatch on ~20M calls
) -> CLPoint {
```

Each `cutter.drop_cutter(&mut cl, tri)` is a virtual call. With ~20M invocations
per batch, the indirect branch cost and missed inlining opportunity is significant.

**Fix:** Use generics instead of trait objects:
```rust
pub fn point_drop_cutter<C: MillingCutter>(
    x: f64, y: f64,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &C,
) -> CLPoint {
```

**Tradeoff:** 5 tool types = 5 monomorphized copies. Worth it for the hot path.

**Estimated speedup:** 2-5% overall (enables inlining of vertex_drop/edge_drop/facet_drop).

---

### 1.4 Add `#[inline]` to Hot Small Functions

Several functions called millions of times lack `#[inline]`:

| Function | File:Line | Call Frequency | Has #[inline]? |
|----------|-----------|---------------|----------------|
| `CLPoint::update_z` | `tool/mod.rs:41-45` | ~25M/job | No |
| `Triangle::contains_point_xy` | `geo.rs:98-114` | ~20M/job | No |
| `Triangle::z_at_xy` | `geo.rs:117-126` | ~20M/job | No |
| `Interval::contains` | `fiber.rs:27` | ~7M/job | No |
| `Interval::overlaps` | `fiber.rs:31` | ~7M/job | No |
| `Interval::merge` | `fiber.rs:35` | ~7M/job | No |
| `Heightmap::get` | `simulation.rs:79` | ~16M/job | Yes |
| `Heightmap::cut` | `simulation.rs:85` | ~16M/job | Yes |
| `MaterialGrid::get_at` | `adaptive.rs:142` | ~17M/job | Yes |

**Fix:** Add `#[inline]` to the first 6. Consider `#[inline(always)]` for the
existing ones on the hottest paths.

**Estimated speedup:** 0.5-1.5% aggregate.

---

## 2. High: Algorithm & Data Structure Improvements

### 2.1 Waterline Contour Chaining is O(n²)

**File:** `waterline.rs:140-196`

Nearest-neighbor chaining uses linear scan through all unused points for every
point added to the chain:

```rust
for (i, pt) in points.iter().enumerate() {
    if used[i] { continue; }
    let d_sq = dx * dx + dy * dy;
    if d_sq < best_dist_sq { ... }
}
```

**Impact:** 500-2000 boundary points per Z level → 250K-4M distance calcs per level.

**Fix:** Use a KD-tree (kiddo is already in deps but unused!) or a spatial hash grid
for O(log n) nearest-neighbor queries.

**Estimated speedup:** 3-5x for waterline-heavy jobs.

---

### 2.2 Polygon Area Recomputed During Sort

**File:** `polygon.rs:234`

```rust
polygons.sort_by(|a, b| b.area().partial_cmp(&a.area()).unwrap_or(...));
```

`area()` is O(n_vertices) and called O(n log n) times during sort.

**Fix:** Pre-compute areas, then sort by cached values.

---

### 2.3 Pocket Offset Clones Entire Polygon Layers

**File:** `polygon.rs:199-207`

```rust
current = next_layer.clone();  // Clone entire Vec of polygons
layers.push(next_layer);       // Then consume the original
```

**Fix:** Use `std::mem::take()` or restructure to avoid cloning:
```rust
layers.push(next_layer.clone());
current = next_layer;  // move, don't clone
```

---

### 2.4 Douglas-Peucker Path Simplification Allocates Per Recursion

**File:** `adaptive3d.rs:752-796`

Recursive simplify_path_3d allocates new Vecs at each recursion level:
```rust
let mut left = simplify_path_3d(&points[..=max_idx], tolerance);
let right = simplify_path_3d(&points[max_idx..], tolerance);
left.pop();
left.extend(right);
```

**Fix:** Use iterative DP with a stack and single output buffer.

---

## 3. Medium: Parallelism Opportunities

### 3.1 Polygon Offset Within Layers (rayon)

**File:** `polygon.rs:194-206`

Within each pocket offset layer, multiple polygons are offset independently
but sequentially:

```rust
for poly in &current {
    next_layer.extend(offset_polygon(poly, stepover));
}
```

**Fix:** `current.par_iter().flat_map(|p| offset_polygon(p, stepover)).collect()`

**Estimated speedup:** 2-4x for complex pockets with many rings.

---

### 3.2 Waterline X/Y Fiber Processing

**File:** `waterline.rs:68-70`

X-fibers and Y-fibers are processed sequentially but are completely independent:
```rust
batch_push_cutter(&mut x_fibers, mesh, index, cutter);
batch_push_cutter(&mut y_fibers, mesh, index, cutter);
```

**Fix:** Use `rayon::scope` to process both in parallel.

**Estimated speedup:** 1.5-2x for waterline operations.

---

### 3.3 Spatial Index Build

**File:** `mesh.rs:128-145`

Triangle insertion into grid cells is sequential. Each triangle's cell range
is independent.

**Fix:** Partition triangles into chunks, compute in parallel, merge with thread-local buffers.

**Estimated speedup:** 2-4x for large meshes (100K+ triangles).

---

### 3.4 Simulation Heightmap Stamping

**File:** `simulation.rs:226-260`

Toolpath moves are stamped sequentially. Non-overlapping segments could run in parallel.

**Fix:** Partition moves into spatial buckets, process with atomic writes.

**Estimated speedup:** 2-3x for long toolpaths.

---

### 3.5 SIMD Opportunities

**Push-cutter distance loops** (`pushcutter.rs:90-120`):
Pack 4 vertex distance calculations into SIMD registers (AVX2 `_mm256_*` ops).
Estimated 3-4x for this specific loop.

**Engagement grid scanning** (`adaptive3d.rs:345-369`):
Process 4 adjacent cells per SIMD iteration for distance checks.
Estimated 2-3x but limited by gather/scatter complexity.

**Heightmap stamping** (`simulation.rs:130-150`):
Vectorize distance calculations for grid cells.
Estimated 1.5-2x, diminished by scatter writes.

**Note:** nalgebra's `simba` feature (not currently enabled) adds SIMD trait
implementations that could help automatically.

---

## 4. Medium: Compiler Optimization Hints

### 4.1 Bounds Check Elimination

Hot loops with pre-validated index ranges still do runtime bounds checks:

```rust
// adaptive.rs:373 — row/col already validated by loop bounds
if grid.cells[row * grid.cols + col] == CELL_MATERIAL { ... }

// adaptive3d.rs:362 — same pattern
let mat_z = material_hm.get(row, col);
let surf_z = surface_hm.surface_z_at(row, col);
```

**Fix:** Use `unsafe { *cells.get_unchecked(idx) }` after validating loop bounds,
or restructure to use iterators that elide bounds checks.

**Frequency:** ~17.5M accesses per 3D adaptive job.

---

### 4.2 Pre-Compute Valid Cell Ranges (Eliminate Distance Checks)

**File:** `simulation.rs:131-150` and `adaptive3d.rs:345-369`

Currently checks every cell in the bounding square, then skips ~30% via distance test.

**Fix:** Pre-compute the column range per row using the circle equation:
```rust
for row in row_lo..=row_hi {
    let dy_sq = (origin_y + row as f64 * cs - cy).powi(2);
    let max_col_offset = ((r_sq - dy_sq).sqrt() / cs).ceil() as usize;
    let col_lo = (center_col - max_col_offset).max(0);
    let col_hi = (center_col + max_col_offset).min(cols - 1);
    for col in col_lo..=col_hi {
        // No distance check needed — guaranteed within circle
    }
}
```

**Estimated speedup:** 1.3-1.8x (eliminates ~30% of inner loop iterations + all distance checks).

---

## 5. Medium: Lookup Tables & Precomputation

### 5.1 Cache Tool Angle Conversions

**Files:** `tool/tapered_ball.rs`, `tool/vbit.rs`

`alpha()` and `half_angle()` recompute `.to_radians()` on every call, and
`.tan()`, `.sin()`, `.cos()` are called repeatedly with the same angle:

```rust
// tapered_ball.rs — called in r_contact(), h_contact(), cone_offset(),
// height_at_radius(), width_at_height(), facet_drop(), edge_drop()
fn alpha(&self) -> f64 {
    self.taper_half_angle_deg.to_radians()  // recomputed every time
}
```

**Fix:** Pre-compute in constructor and store:
```rust
struct TaperedBallEndmill {
    alpha_rad: f64,
    tan_alpha: f64,
    sin_alpha: f64,
    cos_alpha: f64,
    // ...
}
```

**Impact:** 5-10% speedup for tapered_ball and vbit drop-cutter operations.

---

### 5.2 Cache sin/cos Pairs

**Files:** `adaptive3d.rs:467-468, 507-508, 1053-1054`, `dressup.rs:184-185, 475-478`

Pattern `angle.cos()` and `angle.sin()` computed separately in loops:

```rust
let nx = cx + step_len * angle.cos();
let ny = cy + step_len * angle.sin();
```

**Fix:** Use `angle.sin_cos()` which computes both simultaneously:
```rust
let (sin_a, cos_a) = angle.sin_cos();
let nx = cx + step_len * cos_a;
let ny = cy + step_len * sin_a;
```

**Impact:** ~15-20% faster trig in direction search loops.

---

### 5.3 Use `.copied()` Instead of `.cloned()` for f64

**Files:** `simulation.rs:95`, `vcarve.rs:340-341`, `adaptive3d.rs:154-157`

```rust
// Current:
self.cells.iter().cloned().fold(f64::INFINITY, f64::min);

// Better (f64 is Copy, no need for clone machinery):
self.cells.iter().copied().fold(f64::INFINITY, f64::min);
```

Minor but eliminates unnecessary trait dispatch.

---

### 5.4 G-code format!() in Per-Move Output

**File:** `gcode.rs:45-63`

Per-move methods use `format!()` which allocates:
```rust
format!("G0 X{x:.3} Y{y:.3} Z{z:.3}\n")
```

The main `emit_gcode` function uses `writeln!` (good), but the trait methods
return `String`. For toolpaths with 50K+ moves, this is significant.

**Fix:** Have trait methods write directly to a `&mut String` or `&mut impl Write`.

---

## 6. Low: Memory & Allocation Patterns

### 6.1 Arc Fitting Allocates Vec Per Candidate

**File:** `arcfit.rs:78-80`

```rust
let points: Vec<&P3> = std::iter::once(start)
    .chain((i..run_end).map(|j| &moves[j].target))
    .collect();  // allocates on each iteration
```

**Fix:** Use `SmallVec<[&P3; 64]>` or a reusable scratch buffer.

---

### 6.2 Move Cloning in Dressup Operations

**File:** `dressup.rs:70, 218, 234, 287, etc.`

Frequently clones `Move` structs (56 bytes each) when building result toolpaths:
```rust
result.moves.push(m.clone());
```

**Fix:** Where possible, use `std::mem::take()` or build output without cloning.

---

### 6.3 Ring-to-Geo Double Allocation

**File:** `polygon.rs:347`

```rust
let mut coords: Vec<geo::Coord<f64>> = pts.iter()
    .map(|p| geo::Coord { x: p.x, y: p.y })
    .collect();  // allocation 1
coords.push(/* closing vertex */);  // potential realloc
```

**Fix:** Use `Vec::with_capacity(pts.len() + 1)`.

---

### 6.4 Flood Fill Labels Grid for Sparse Data

**File:** `adaptive3d.rs:216`

```rust
let mut labels = vec![0usize; rows * cols];  // full grid even when few cells have material
```

For large grids (1000×1000 = 8MB of usize), this is wasteful when material is sparse.

**Fix:** Consider using a `HashMap<(usize, usize), usize>` for sparse regions,
or accept the cost since it's a one-time allocation per region detection.

---

### 6.5 Border Cell Clearing Iterates All Cells

**File:** `adaptive3d.rs:1223-1236`

Iterates the entire grid but only clears border cells:
```rust
for row in 0..material_hm.rows {
    for col in 0..material_hm.cols {
        if x < bbox.min.x - border_margin || ... {
            // clear cell
        }
    }
}
```

**Fix:** Compute min/max cell indices from bbox, only iterate border strips.

---

## 7. Low: Async & I/O

### 7.1 File I/O is Synchronous

All STL loading, G-code writing, and DXF/SVG parsing is blocking:
- `mesh.rs:29` — `stl_io::read_stl()`
- `main.rs:796, 835` — `std::fs::write()`
- `job.rs:155` — `std::fs::read_to_string()`

**Current impact:** Negligible (file ops <100ms, computation is 1-10s).

**Future:** For batch jobs with multiple files, could use `tokio::spawn_blocking`
to overlap I/O with computation.

### 7.2 Pipeline Overlapping

Current workflow is strictly sequential: load → compute → write → simulate.
G-code writing could overlap with simulation since they're independent.

**Impact:** Modest (1.1-1.3x) since computation dominates.

---

## 8. Infrastructure: Benchmarking Gaps

### Current State: MINIMAL

- No criterion benchmarks
- No `[[bench]]` entries in Cargo.toml
- Tracing crate imported but barely used (3 decorators)
- `std::time::Instant` imported in adaptive3d.rs but no timing calls
- Performance numbers in PROGRESS.md are informal one-off measurements
- kiddo crate in dependencies but unused (could replace uniform grid for some queries)

### Recommendations

**1. Add criterion benchmarks:**
```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "drop_cutter"
harness = false
```

Key benchmarks needed:
- `bench_drop_cutter_108k_points` — batch drop-cutter over terrain mesh
- `bench_adaptive_clearing_spiral` — 2D adaptive on rectangular pocket
- `bench_adaptive_3d_multilevel` — 3D adaptive on terrain mesh
- `bench_waterline_multi_z` — waterline at 10 Z levels
- `bench_stamp_tool_at` — isolated heightmap stamping

**2. Add phase timing to adaptive3d.rs** using `Instant::now()` + tracing spans.

**3. Profile with `perf`/`flamegraph`** to validate which hot paths actually dominate.

---

## 9. Summary Matrix

| # | Issue | Location | Category | Priority | Est. Speedup |
|---|-------|----------|----------|----------|-------------|
| 1.1 | Spatial index O(n²) dedup | mesh.rs:170 | Data Structure | **Critical** | 1.5-2x (queries) |
| 1.2 | `.powi(2)` → `x * x` | 14 locations | Numeric | **Critical** | 5-10% |
| 1.3 | Monomorphize drop-cutter | dropcutter.rs:11 | Dispatch | **Critical** | 2-5% |
| 1.4 | Missing `#[inline]` | 6 functions | Compiler | **Critical** | 0.5-1.5% |
| 2.1 | Waterline O(n²) chaining | waterline.rs:140 | Algorithm | **High** | 3-5x (waterline) |
| 2.2 | Polygon area in sort | polygon.rs:234 | Algorithm | **High** | minor |
| 2.3 | Pocket offset clone | polygon.rs:199 | Allocation | **High** | minor |
| 3.1 | Parallel polygon offset | polygon.rs:194 | Parallelism | **Medium** | 2-4x (pockets) |
| 3.2 | Parallel waterline fibers | waterline.rs:68 | Parallelism | **Medium** | 1.5-2x |
| 3.3 | Parallel spatial index build | mesh.rs:128 | Parallelism | **Medium** | 2-4x (init) |
| 3.4 | SIMD distance loops | pushcutter.rs:90 | SIMD | **Medium** | 3-4x (loop) |
| 4.1 | Bounds check elision | adaptive.rs:373 | Compiler | **Medium** | 0.3-0.5% |
| 4.2 | Pre-compute cell ranges | simulation.rs:131 | Algorithm | **Medium** | 1.3-1.8x (stamp) |
| 5.1 | Cache tool trig values | tapered_ball.rs, vbit.rs | Precompute | **Medium** | 5-10% (tool ops) |
| 5.2 | `sin_cos()` pairs | adaptive3d.rs, dressup.rs | Precompute | **Medium** | 15-20% (trig) |
| 5.3 | `.copied()` not `.cloned()` | simulation.rs:95 | Idiom | **Low** | negligible |
| 5.4 | G-code format!() allocs | gcode.rs:45 | I/O | **Low** | minor |
| 6.1 | Arc fit Vec per candidate | arcfit.rs:78 | Allocation | **Low** | minor |
| 6.2 | Move cloning in dressup | dressup.rs:70 | Allocation | **Low** | minor |
| 7.1 | Async file I/O | mesh.rs, main.rs | Async | **Low** | 1.3x (batch only) |
| 8.0 | No benchmarking infra | Cargo.toml | Infra | **Medium** | enables all above |

---

## Recommended Implementation Order

### Week 1: Quick Wins (1-2 hours each)
1. Replace `Vec::contains()` with bitset in `SpatialIndex::query()`
2. Replace all 14 `.powi(2)` with `x * x`
3. Add `#[inline]` to 6 hot functions
4. Cache tool angle conversions (add `alpha_rad`, `tan_alpha` fields)
5. Use `angle.sin_cos()` everywhere instead of separate calls

### Week 2: Medium Effort
6. Monomorphize `point_drop_cutter` and `batch_drop_cutter` with generics
7. Pre-compute valid cell column ranges in heightmap stamping
8. Add criterion benchmark harness with 5 key benchmarks
9. Add rayon to waterline X/Y fiber processing

### Week 3+: Larger Refactors
10. Replace waterline O(n²) chaining with KD-tree (kiddo)
11. Parallelize polygon offset layers
12. Add SIMD to push-cutter distance loops
13. Profile with flamegraph and validate priorities

---

## 10. Profiling & Improvement Plan

### Benchmark Harness

Criterion benchmarks live in `crates/rs_cam_core/benches/perf_suite.rs`.
Run with: `cargo bench -p rs_cam_core`

### Test Fixtures

| Fixture | Type | Size | Used By |
|---------|------|------|---------|
| `fixtures/terrain_small.stl` | Real STL | 40K tris | drop-cutter, waterline, spatial index |
| `make_test_hemisphere(25, 20)` | Programmatic | ~3K tris | drop-cutter (simple), waterline |
| `Polygon2::rectangle(60mm)` | Programmatic | 4 verts | polygon offset, pocket |
| `Polygon2::rectangle(200mm)` | Programmatic | 4 verts | pocket (stress) |
| `Heightmap::from_stock(100×100, cs=0.5/1.0)` | Programmatic | 40K/10K cells | stamp_tool_at |
| `make_linear_toolpath(500/2000/10K)` | Programmatic | curved line | arc fitting |

### Benchmark Matrix (20 benchmarks across 7 groups)

| Group | Benchmark | Fixture | What It Measures |
|-------|-----------|---------|------------------|
| **batch_drop_cutter** | hemisphere_ball_6mm | hemisphere | Full grid DC on simple mesh |
| | terrain_ball_6mm_step1 | terrain STL | Full grid DC on real mesh (primary) |
| | terrain_flat_6mm_step1 | terrain STL | Flat endmill variant |
| **point_drop_cutter** | terrain_center_ball | terrain STL | Single-point DC (isolates per-query cost) |
| **spatial_index** | build_terrain | terrain STL | Index construction time |
| | query_r3 | terrain STL | Small radius query (few tris, tests dedup) |
| | query_r10 | terrain STL | Large radius query (many tris, stresses dedup) |
| **stamp_tool** | ball_6mm/cs0.5 | 100×100 heightmap | Fine-grid stamp (most cells) |
| | flat_6mm/cs0.5 | 100×100 heightmap | Flat endmill stamp |
| | ball_6mm/cs1 | 100×100 heightmap | Coarse-grid stamp |
| | flat_6mm/cs1 | 100×100 heightmap | Coarse flat stamp |
| **waterline** | hemisphere_z10_samp1 | hemisphere | Waterline on simple mesh |
| | terrain_midz_samp1 | terrain STL | Waterline on real mesh |
| **polygon_ops** | offset_60mm_square | 60mm rect | Single offset operation |
| | pocket_offsets_60mm | 60mm rect | Multi-layer pocket |
| | pocket_offsets_200mm | 200mm rect | Stress test pocket |
| **arc_fitting** | fit_arcs/500 | 500-move path | Small toolpath arc fit |
| | fit_arcs/2000 | 2000-move path | Medium toolpath arc fit |
| | fit_arcs/10000 | 10000-move path | Large toolpath arc fit |

### Improvement Plan (execute in order, benchmark after each)

Each change is applied, benchmarked, and the delta recorded in the results table below.
All 324 tests must pass after each change (`cargo test -p rs_cam_core`).

| Step | Change | Files Modified | Primary Benchmark(s) Affected |
|------|--------|---------------|-------------------------------|
| 0 | **Baseline** (no changes) | — | all |
| 1 | Spatial index: `Vec::contains()` → bitset | `mesh.rs` | spatial_index/*, point_drop_cutter, batch_drop_cutter |
| 2 | Replace 14× `.powi(2)` with `x * x` | adaptive.rs, arcfit.rs, dressup.rs, waterline.rs, viz.rs | batch_drop_cutter, waterline, arc_fitting |
| 3 | Add `#[inline]` to 6 hot functions | tool/mod.rs, geo.rs, fiber.rs | batch_drop_cutter, waterline, point_drop_cutter |
| 4 | Cache tool trig: store `alpha_rad`, `tan_alpha`, `sin_alpha`, `cos_alpha` | tool/tapered_ball.rs, tool/vbit.rs | batch_drop_cutter (with tapered/vbit tools) |
| 5 | Use `angle.sin_cos()` everywhere | adaptive3d.rs, adaptive.rs, dressup.rs, simulation.rs | stamp_tool, waterline |
| ~~6~~ | ~~Pre-compute valid cell column ranges in stamp~~ | ~~simulation.rs~~ | ~~stamp_tool/*~~ (reverted — regression for small tools) |
| 7 | Pocket offset: `clone()` → `std::mem::take()` | polygon.rs | polygon_ops/pocket_* |
| 8 | `.copied()` not `.cloned()` for f64 iterators | simulation.rs, vcarve.rs, adaptive3d.rs | stamp_tool |
| 9 | Monomorphize drop-cutter with generics | dropcutter.rs | batch_drop_cutter/*, point_drop_cutter |
| 10 | Parallel waterline X/Y fibers via `rayon::scope` | waterline.rs | waterline/* |

### Results Table

Record the mean time from criterion for each benchmark after each step.
Format: `time_ms` or `time_µs`. Δ% is vs baseline (step 0).

| Benchmark | Step 0 (Baseline) | Final (Steps 1-5,7-10) | Δ% |
|-----------|-------------------|------------------------|-----|
| batch_dc/terrain_ball | 350.24 ms | 57.24 ms | **-83.7%** |
| batch_dc/terrain_flat | 337.56 ms | 47.17 ms | **-86.0%** |
| batch_dc/hemisphere | 8.97 ms | 3.62 ms | **-59.6%** |
| point_dc/terrain_ball | 10.08 µs | 6.43 µs | **-36.2%** |
| spatial/build | 566.95 µs | 532.0 µs | -6.2% |
| spatial/query_r3 | 3.88 µs | 559.7 ns | **-85.6%** |
| spatial/query_r10 | 83.20 µs | 1.29 µs | **-98.4%** |
| stamp/ball_cs0.5 | 594.17 ns | 546.7 ns | -8.0% |
| stamp/flat_cs0.5 | 321.27 ns | 304.3 ns | -5.3% |
| stamp/ball_cs1 | 183.23 ns | 170.1 ns | -7.2% |
| stamp/flat_cs1 | 110.02 ns | 106.4 ns | -3.3% |
| waterline/hemisphere | 29.51 ms | 14.32 ms | **-51.4%** |
| waterline/terrain | 2.19 s | 94.63 ms | **-95.7%** |
| polygon/offset_60 | 454.85 ns | 428.8 ns | -5.7% |
| polygon/pocket_60 | 7.44 µs | 7.15 µs | -3.9% |
| polygon/pocket_200 | 19.68 µs | 18.07 µs | -8.2% |
| arc_fit/500 | 32.99 µs | 27.88 µs | **-15.5%** |
| arc_fit/2000 | 215.11 µs | 185.15 µs | **-13.9%** |
| arc_fit/10000 | 3.61 ms | 2.83 ms | **-21.6%** |
| **tests passing** | 324 | 324 | ✓ |

**Step 6 (pre-compute column ranges in stamp) was reverted** — the added sqrt() per row
costs more than the distance checks it saves for small tools (6mm on 0.5-1mm grid).
This optimization would help for larger tools (>25mm) on fine grids.

### Key Wins

- **Spatial index queries: 85-98% faster** (bitset dedup — Step 1)
- **Batch drop-cutter: 84-86% faster** (bitset + monomorphization + inline)
- **Waterline: 51-96% faster** (parallel X/Y fibers + bitset propagation)
- **Arc fitting: 14-22% faster** (powi→mul + copied)
- **All other benchmarks improved** by 3-8%

### Notes
- Run benchmarks on a quiet machine with consistent load
- Use `cargo bench -p rs_cam_core -- --save-baseline stepN` to save each step
- Use `cargo bench -p rs_cam_core -- --baseline step0` to compare against baseline
- After each step, run `cargo test -p rs_cam_core` to verify correctness
- If a change causes regression in an unrelated benchmark, investigate before proceeding

---

*Generated 2026-03-20 by performance review agents analyzing the full rs_cam codebase.*
