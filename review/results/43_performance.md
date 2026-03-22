# Review: Performance & Parallelism

## Summary
The codebase has solid memory-conscious design (SmallVec for dexels, LUT-based simulation avoiding per-cell sqrt, pre-allocation throughout) but minimal parallelism — only one site uses rayon (waterline). The dropcutter batch loop is the most impactful parallelization candidate. The spatial index deduplication allocates a full `Vec<bool>` per query which is wasteful for large meshes. Benchmark coverage is good for existing hot paths but missing for newer 3D operations.

## Findings

### Parallelism Status

#### Current Rayon Usage (1 site only)
- `crates/rs_cam_core/src/waterline.rs:72` — `rayon::join()` parallelizes X and Y fiber processing
- Feature-gated: `#[cfg(feature = "parallel")]`, enabled by default
- No other parallelism in the codebase

#### Sequential Hot Paths (Parallelization Candidates)

| Hot Path | File | Parallelizable? | Impact |
|----------|------|-----------------|--------|
| Dropcutter batch grid | dropcutter.rs:106-119 | Yes — independent per-point queries | 4-8x speedup |
| Adaptive material grid construction | adaptive.rs:83-90 | Yes — independent point-in-polygon tests | 2-4x speedup |
| Pocket offset layers | polygon.rs:209-216 | Yes — independent polygon offsets | 2-4x speedup |
| Simulation stamping | simulation.rs:444-473 | No — sequential heightmap mutation | N/A |
| Adaptive path search | adaptive.rs | No — depends on prior state | N/A |

### Spatial Indexing

#### Uniform Grid (Not KD-tree)
- `mesh.rs:365-480` — Comment says "KD-tree" but code is a uniform spatial grid
- Cell size auto-tuned to ~50 cells per axis, clamped to >=1.0mm (line 391)
- Grid is actually appropriate for uniform mesh distributions (faster than KD-tree for this use case)
- **Issue:** Comment should be updated to match reality

#### Deduplication Waste
- `mesh.rs:464` — `let mut seen = vec![false; self.total_triangles]` allocated **per query**
- For 100k-triangle mesh: 100KB allocation per drop-cutter point
- Batch drop-cutter with 200 points: 20MB total allocations (freed after each)
- **Fix:** Use bitset/bitvec for 8x space reduction, or pre-allocate and clear

### Simulation Performance (Well-Optimized)

#### LUT-Based Stamping (sqrt regression fixed)
- `simulation.rs:111-115` — `RadialProfileLUT` indexes by `dist_sq` to avoid per-cell sqrt
- 256 samples with bilinear interpolation gives sub-micron accuracy
- `stamp_tool_at_lut()` (lines 224-290): nested loops, per-cell `dist_sq` with LUT lookup
- `stamp_linear_segment_lut()` (lines 307-367): swept-stadium algorithm, single pass, no sqrt
- **Known issue "sqrt-per-row regresses small tools" is FIXED** via LUT approach

#### Memory Layout
- Heightmap: row-major `cells[row * cols + col]`, pre-allocated `vec![top_z; rows * cols]`
- Cache-friendly for linear access patterns
- At 0.25mm grid, 100x100mm stock = 1.3MB — reasonable
- At 0.1mm grid, 500x500mm stock = 200MB — could stress memory for large jobs

### Memory Patterns (Good)

#### Pre-allocation
- `dropcutter.rs:105` — `Vec::with_capacity(total)` for batch results
- `simulation.rs:39` — Heightmap cells
- `simulation.rs:129` — LUT heights
- `adaptive.rs:80` — Material grid
- `dexel.rs:39` — `SmallVec<[DexelSegment; 1]>` avoids heap allocation for common single-segment case

#### Arc<T> Sharing
- Worker threads share `TriangleMesh`, `Polygon2 Vec`, `Toolpath` via Arc
- Appropriate for zero-copy sharing between worker and main thread

### Cancellation Support
- Cooperative interrupts via `interrupt.rs`
- Dropcutter: checks every 64 grid points (`dropcutter.rs:107-109`)
- Simulation: checks between moves (`simulation.rs:445, 459, 466`)
- Rayon interaction: no preemptive cancellation, but cooperative model is sufficient for UI responsiveness

### Benchmark Coverage

| Hot Path | Benchmarked | Notes |
|----------|-------------|-------|
| batch_drop_cutter | Yes | 2 meshes, 1mm stepover |
| point_drop_cutter | Yes | Single point on terrain |
| SpatialIndex build/query | Yes | Terrain, 2 radii |
| stamp_tool | Yes | 0.5mm and 1.0mm cell sizes |
| stamp_linear_segment | Yes | 50mm segment |
| simulate_toolpath | Yes | 2000 moves, 0.25mm cells |
| waterline | Yes | Hemisphere and terrain |
| polygon offset | Yes | 3 test cases |
| arc fitting | Yes | 3 path sizes (500-10k pts) |
| adaptive clearing | **No** | Not benchmarked |
| surface heightmap ops | **No** | adaptive3d, scallop, ramp not benchmarked |
| fine-resolution sim (<0.25mm) | **No** | Missing regression guard |

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Med | Only 1 of ~4 parallelizable hot paths uses rayon | dropcutter.rs, adaptive.rs |
| 2 | Med | Spatial index dedup allocates Vec<bool> per query (wasteful for large meshes) | mesh.rs:464 |
| 3 | Low | Comment says "KD-tree" but code is uniform grid | mesh.rs:367 |
| 4 | Low | No benchmarks for adaptive clearing or 3D surface operations | perf_suite.rs |
| 5 | Low | No fine-resolution (<0.25mm) simulation benchmarks | perf_suite.rs |

## Test Gaps
- No benchmark for adaptive clearing inner loops
- No benchmark for surface-heightmap generation (adaptive3d, scallop, ramp finish)
- No end-to-end workflow benchmark (import → generate → simulate → export)

## Suggestions

### High Impact, Low Effort
1. **Parallelize dropcutter batch grid** — `par_iter()` over grid points; each has independent mesh queries. 4-8x speedup on multi-core. ~1-2 hours including cancellation integration.
2. **Replace `seen` Vec<bool> with bitset** in `mesh.rs:464` — 8x memory reduction for spatial queries. ~30 minutes.

### Medium Impact
3. **Parallelize adaptive material-grid construction** — partition grid rows, process in parallel. 2-4x speedup for large polygons.
4. **Add fine-resolution simulation benchmarks** — 0.1mm and 0.05mm cell sizes as regression guards.
5. **Benchmark adaptive clearing and 3D surface ops** — understand performance before optimizing.

### Low Priority
6. **Update mesh.rs comment** — says KD-tree, is uniform grid (grid is fine; comment is wrong)
7. **Parallelize pocket offset layers** — independent polygon offsets could run in parallel
