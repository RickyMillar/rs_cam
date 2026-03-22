# Review: Dropcutter (3D Finish)

## Summary

The dropcutter algorithm is mathematically correct, well-tested (47 unit tests across core and tools), and properly integrated into both CLI and GUI. It uses a uniform-grid spatial index with bitset dedup for triangle queries, supports five tool types with verified edge/vertex/facet contact methods, and includes cancellation support. No critical bugs found — minor opportunities around epsilon consistency and integration test coverage.

## Findings

### Algorithm Correctness

**Core grid algorithm** (`crates/rs_cam_core/src/dropcutter.rs`):
- Uniform grid spatial index (SpatialIndex) queries only cells within cutter radius + triangle extent, with bitset dedup (mesh.rs:464)
- Axis-aligned fast path (lines 94-129): detects near-zero angles (0°, 90°, 180°), skips rotation overhead
- Rotated grid support (lines 132-188): forward/inverse rotation mathematically sound
- Cancellation checked every 64 points (lines 107-109, 160-162) — prevents UI lock
- Min-Z clamping (lines 115-117, 174-176) allows safe horizontal passes even if no mesh contact

**Verified by tests**: flat surface landing (line 198), contacted flag (line 220), 45° rotated grid (line 239)

### Tool Edge_Drop Methods

All five tool types implement edge_drop for tool-edge intersection geometry, following OpenCAMLib's "dual geometry" approach.

**Flat Endmill** (flat.rs:56-104):
- Circle-line intersection formula, standard and correct
- `test_flat_edge_drop()`: edge at (3, ±10, 7), radius 5, d=3, s=sqrt(25-9)=4, expected z=7 ✓
- Edge parameter clamped to [0,1] with 1e-8 tolerance

**Ball Endmill** (ball.rs:63-158):
- Sphere-cylinder intersection with tangent slope condition
- Two solutions (sign=[1,-1]), filtered by `sin_a >= -1e-10` (lower hemisphere only)
- `test_ball_edge_drop_horizontal_edge()`: validates tip_z=-1 ✓
- Matches OpenCAMLib's published math

**Bull Nose** (bullnose.rs:96-242):
- Three-phase: flat region (d < r1), torus region (d > r1), validated at both boundaries
- Degenerate cases: corner_radius=0 (pure flat) and corner_radius=R (pure ball) both tested (lines 300, 310) ✓
- `test_bullnose_edge_drop_horizontal_in_torus_region()`: d_torus=1, s=sqrt(3), tip_z≈-0.268 ✓

**V-Bit** (vbit.rs:147-277):
- Cone-edge intersection via hyperbola: `ccu = sqrt(R²·slope²·d² / (L² - R²·slope²))`
- Handles degenerate case (slope = cone angle) by testing rim contact instead (line 229)
- Rim contact tested separately (lines 265-276)
- `test_vbit_edge_drop_horizontal()`: cone contact z=-3 vs rim contact z=-5, max=-3 ✓

**Tapered Ball** (tapered_ball.rs:230-340):
- Composite: hemispherical tip → cone transition
- Junction validated: `r_contact = R_ball·cos(α)`, `h_contact = R_ball·(1-sin(α))`
- Profile continuity test (line 403): h_at, h_below, h_above within 0.01 ✓

### Performance

**Spatial index** (mesh.rs:365-480):
- Uniform 2D grid, O(T·C) build, O(Q·M) query
- Bitset dedup (`vec![false; total_triangles]` per query) — O(1) dedup, cache-friendly
- Adequate for wood router CAM (typical meshes < 50k triangles)

**sqrt-per-row regression**: The code does NOT pre-compute sqrt per row — loops through all grid points and tests each triangle. Already optimized against the known issue. ✓

**Edge_drop cost**: O(1) per edge for all tool types. For typical run (10k grid points × ~100 triangles per query × 3 edges = 3M edge tests), performance is bounded by trig functions, not algorithmic complexity.

**Fine stepover**: 100mm×100mm mesh with 0.1mm stepover = 1M points × 25 bytes ≈ 25MB. Acceptable for batch; marginal for interactive GUI.

### Edge Case Handling

- **Mesh holes**: Grid points above holes get `contacted=false`, z=NEG_INFINITY, clamped to min_z. Acceptable.
- **Non-manifold edges**: Tests all triangles regardless of topology. Robust to malformed input.
- **Overhangs**: Ball geometry wraps around edges, finds contact on sloped surfaces. Correct.
- **Tool > feature**: Tool lands on highest contact (e.g., slot floor, not walls). Correct.
- **Mesh winding**: `check_winding()` (mesh.rs:197-245) auto-fixes >5% inconsistent normals. Prevents inverted contact tests.

### Integration

**CLI** (rs_cam_cli/src/job.rs): Invoked as `"drop-cutter"` / `"drop_cutter"`, grid computed → `raster_toolpath_from_grid()`. ✓

**GUI** (execute.rs:647-676): `run_dropcutter()` calls `batch_drop_cutter_with_cancel()` with cancellation token, debug spans for "dropcutter_grid" and "rasterize_grid", phase tracker for UI progress. ✓

**Properties UI**: `DropCutterConfig` with stepover, min_z, feed_rate, plunge_rate fields.

**Missing**: No semantic tracing integration (unlike Adaptive3d which has extensive runtime annotations). Nice-to-have.

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Medium | No subdivision for coarse mesh with fine stepover — grid points may miss features smaller than triangle size | dropcutter.rs (inherent CAM limitation, not a bug) |
| 2 | Low | Epsilon tolerance inconsistency: 1e-8 (edge param), 1e-10 (profile), 1e-20 (degenerate edge), 1e-12 (normal) — all reasonable but not unified | flat.rs:99, ball.rs:139, tool/mod.rs:99 |
| 3 | Low | Ball edge_drop sign convention subtle — `sin_a >= -1e-10` filter correct but could use clarifying comment | ball.rs:125-154 |
| 4 | Low | No min_z validation against mesh — user can set min_z above surface, causing non-cutting passes (expected behavior, but undocumented) | dropcutter.rs:115-117 |

## Test Coverage

**47 unit tests** across dropcutter and tool modules:
- dropcutter.rs: 3 tests (flat surface, contacted flag, rotated grid)
- flat.rs: 4 tests (profile, vertex, facet, edge drop)
- ball.rs: 7 tests (profile, vertex, facet, edge drop, hemisphere mesh)
- bullnose.rs: 14 tests (construction, profile, degeneracies, edge drop, hemisphere)
- vbit.rs: 10 tests (construction, profile, edge drop, sloped)
- tapered_ball.rs: 9 tests (junction, continuity, edge drop, hemisphere)

## Test Gaps

- No integration test combining dropcutter grid → rasterize → simulate pipeline
- No negative case tests (mesh with holes, zero-area triangles, colinear vertices)
- No performance benchmarks to catch regressions
- No degenerate mesh tests (single triangle, empty mesh)

## Suggestions

**Priority 1 (Safety/Correctness)**:
1. Define symbolic epsilon constants (`EPSILON_ZERO`, `EPSILON_EDGE`, etc.) per file for consistency
2. Add integration test for dropcutter → rasterize → simulate pipeline

**Priority 2 (Clarity)**:
1. Document ball.rs edge_drop sign convention with comment explaining lower-hemisphere filtering
2. Add note in DropCutterConfig documenting min_z semantics

**Priority 3 (Enhancements)**:
1. Add semantic tracing for dropcutter (row progress, triangle count per cell) for debugger UI
2. Instrument SpatialIndex::query to log cell hit rate
3. Warn if mesh has >5% non-manifold edges at operation start

## Algorithm Soundness

| Component | Status | Confidence |
|-----------|--------|-----------|
| Grid generation (axis-aligned) | Correct | Very High |
| Grid generation (rotated) | Correct | High |
| Spatial indexing | Correct | Very High |
| Flat endmill contact | Correct | Very High |
| Ball endmill contact | Correct | Very High |
| Bull nose contact | Correct | High |
| V-bit contact | Correct | High |
| Tapered ball contact | Correct | High |
| Cancellation | Correct | Very High |
| Rasterization | Correct | Very High |
| CLI integration | Correct | Very High |
| GUI integration | Correct | Very High |
