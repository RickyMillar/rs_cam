# Review: Project Curve Operation

## Summary

Project Curve is a well-structured 3D surface engraving operation that projects 2D polygon paths onto a mesh using vertical drop-cutter queries. It resamples input curves at configurable spacing (default 0.5mm), drops each point to the mesh surface, offsets Z by a depth parameter, and generates toolpaths with automatic gap handling for off-mesh regions. The code is clean (zero unwrap/panic in library code) with complete GUI integration, but lacks integration tests and tool compensation.

## Findings

### Algorithm Correctness

**Projection method** (`crates/rs_cam_core/src/project_curve.rs:157-161`):
- Vertical ray per curve point via `point_drop_cutter(pt.x, pt.y, mesh, index, cutter)` from dropcutter module
- Returns `CLPoint` with `z` (contact height) and `contacted` flag
- Actual cut Z = `cl.z - params.depth`

**Sampling** (lines 40-94, `resample_polyline()`):
- Walks polygon edges accumulating distance, emits interpolated points when distance exceeds `point_spacing`
- Preserves first and last points exactly
- Skips zero-length segments (1e-12 threshold)
- Tests at lines 196-244 verify correct interpolation on straight/multi-segment paths

**Ring closing** (lines 107-122):
- `close_ring()` appends first point to close polygon for proper loop traversal
- Avoids duplicate if already closed (1e-9 threshold)
- Handles exterior and holes uniformly

**Off-mesh gap handling** (lines 157-174):
- When `cl.contacted == false`: current chain flushed via `tp.emit_path_segment()`, chain accumulation pauses, point skipped
- No bridge moves generated over gaps — clean disconnect with rapids between segments

### Use Cases

- **Primary**: Engraving text/graphics (SVG import) onto curved wooden surfaces
- **Multi-ring**: Exterior + multiple holes in one polygon projected in a single operation (line 145: `for ring in rings`)
- **Tool types**: Any `dyn MillingCutter` — flat, ball, V-bit, tapered, bull nose all supported via `point_drop_cutter` interface

### Tool Compensation

**No tool compensation is implemented.** The operation uses tool center (CL) path directly. Comparison:
- Trace operation has `TraceCompensation::Left/Right` via `offset_polygon()`
- ProjectCurve follows curves exactly at tool center

**Impact**: User must import curves with proper tool-center offsets pre-applied. If engraving a 2mm wide line with a 3mm endmill, the tool will not fit.

### Edge Case Handling

| Case | Behavior | Status |
|------|----------|--------|
| Curve entirely outside mesh | All points `contacted=false` → empty toolpath | Safe |
| Partial overlap | Gap segments flushed, separate chains, rapids bridge gaps | Safe |
| Curve crosses hole in mesh | Hole detected → chain flush → safe disconnect | Safe |
| Degenerate polygon (< 2 points) | Skipped at line 146 | Safe |
| Zero spacing | Returns original points unchanged | Safe |
| Multiple Z intersections (overhangs) | Uses highest Z only (dropcutter behavior) | Design choice |
| Numerical near-duplicates | 1e-9 epsilon in ring closing and segment skipping | Robust |

### Integration

**Fully wired in GUI**:
- Operation enum: `OperationType::ProjectCurve` (catalog.rs:82)
- Config: `ProjectCurveConfig` with depth (1.0mm), point_spacing (0.5mm), feed_rate (800mm/min), plunge_rate (400mm/min) (configs.rs:651-667)
- UI panel: `draw_project_curve_params()` (properties/mod.rs:2368-2404) — shows label "Projects 2D curves onto 3D mesh" with grid inputs
- Execution: `run_project_curve()` (execute.rs:945-966) — iterates polygons, calls `project_curve_toolpath()`, requires mesh + spatial index + cutter
- Semantic tracing: `ToolpathSemanticKind::Curve` (execute.rs:2452-2479)
- Project IO: `"project_curve" => OperationType::ProjectCurve` (project.rs:1157)

**Not exposed in CLI** (GUI-only, per FEATURE_CATALOG.md).

### Code Quality

- Zero `unwrap()`, `expect()`, or `panic!` in library code
- Proper `?Sized` trait bounds on `cutter: &dyn MillingCutter`
- Module-level and function-level doc comments
- Safe defaults in `ProjectCurveParams::default()`
- Numeric precision: 1e-9 epsilon appropriate for mm-scale CAM

**Documentation gaps**:
- No mention that projection is vertical-only (no angled/swept projection)
- No mention that no tool compensation is applied
- No mention of overhang behavior (highest Z only)

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Medium | No tool compensation — user must pre-offset curves | project_curve.rs (entire design) |
| 2 | Medium | No integration test for `project_curve_toolpath()` — algorithm correctness not verified against actual mesh | project_curve.rs:129 |
| 3 | Low | Vertical-only projection undocumented — could mislead users on highly curved surfaces | project_curve.rs:1-5 |
| 4 | Low | Overhang behavior undocumented — only highest Z used, could cause unexpected results on concave surfaces | project_curve.rs:1-5 |
| 5 | Low | `safe_z` not independently configurable per-operation — uses global `effective_safe_z(req)` | configs.rs:651 |

## Test Gaps

**Unit tests present** (lines 191-273): 7 tests for helpers (`resample_polyline`, `close_ring`, `polyline_length`) covering normal and edge cases.

**Missing**:
- No `project_curve_toolpath()` integration test (no mock mesh, no polygon projection verification)
- No gap-handling test against actual mesh with holes
- No depth-offset verification
- No end-to-end test

**Coverage estimate**: Helpers ~90%, main algorithm ~20%, real-world scenarios 0%.

## Suggestions

**High priority**:
1. Add integration test for `project_curve_toolpath()` — create mock mesh (simple dome/pyramid), project a square polygon, verify Z values = mesh height - depth and gap handling
2. Document tool compensation behavior in module doc: "No tool compensation — curves should be tool-center paths"

**Medium priority**:
3. Consider optional tool compensation via `compensation: Option<f64>` in `ProjectCurveParams`, reusing `offset_polygon()`
4. Add per-operation safe_z override (some operations like Face already support this)

**Low priority**:
5. Expose to CLI for batch projection workflows (minimal changes needed — already wired in GUI)
6. Batch spatial queries instead of per-point `point_drop_cutter()` calls (likely not a bottleneck given dropcutter optimization)
