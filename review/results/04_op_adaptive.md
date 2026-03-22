# Review: Adaptive Clearing (2.5D)

## Summary

The 2.5D adaptive clearing algorithm (2383 LOC) is well-structured and algorithmically sound. Engagement tracking uses a disk-area model with a three-phase direction search that keeps evaluation count manageable (~20 per step). The code is safe (all `unwrap()` calls are guarded or in tests) and has strong test coverage (37 unit tests). Key gaps: no Rayon parallelism despite being the single largest computational hotspot, several GUI parameters not exposed in UI, and missing tests for narrow slots, multi-island pockets, and parameter validation.

## Algorithm Review

### Engagement Angle Tracking
- `compute_engagement()` (`adaptive.rs:385-425`) uses disk-area sampling: counts fraction of grid cells within the tool circle that contain uncut material
- Returns `material_cells / total_cells` (value in [0.0, 1.0])
- Constant engagement maintained via `search_direction_with_metrics()` (`adaptive.rs:484-639`):
  - Phase 1 (narrow interpolation, lines 542-589): 7 candidates near previous angle; refines with `refine_angle_bracket()` if bracketing found
  - Phase 2 (coarse 360-degree scan, lines 591-639): 18 candidates at 20-degree intervals; fresh brackets refined
  - Fallback to best-any result
- Tolerance band: +/-20% of target (`adaptive.rs:495-497`)
- Direction smoothing buffer averages last 3 angles via `average_angles()` (`adaptive_shared.rs:13-21`)
- Wall-tangent bias (`adaptive.rs:520-536`): when near boundary, penalizes perpendicular approach to steer tool along walls

### Path Generation
- **Not true trochoidal**: greedy stepping walk where each step finds the angle maintaining constant engagement
- Creates an emergent adaptive spiral pattern — tightens near material, loosens in open regions
- Step length: `step_len` fixed, position updated per step (`adaptive.rs:1144-1145`)

### Slot Clearing Pre-Pass
- Optional (`params.slot_clearing: bool`), `adaptive.rs:988-1022`
- Finds longest bounding box axis, generates single center zigzag line
- Walks line clearing material in `cell_size * 1.5` steps
- Reduces initial adaptive pass idle time
- Tested: `test_slot_clearing_reduces_material` (line 2038)

### Min Cutting Radius / Arc Blending
- `blend_corners()` (`adaptive_shared.rs:69-149`)
- For each corner: computes bisector, places tangent points at setback distance `min_radius / tan(half_angle)`
- Skips near-straight corners (>170 degrees) and corners too tight for the radius (setback > 40% of edge)
- Tessellates arcs with 2-20 points
- Applied at `adaptive.rs:1398` when `min_cutting_radius > 0.0`
- Geometrically correct

### Tolerance Parameter
- Controls grid cell size: `cell_size = (tool_radius / 6.0).max(tolerance)` (`adaptive.rs:966`)
- Also used for Douglas-Peucker path simplification (`adaptive.rs:1396`)
- Larger tolerance = faster but coarser engagement tracking

### Rest Material Detection Between Passes
- Tracks `grid.material_count` before/after each step; breaks after 15 idle steps (`adaptive.rs:1157-1169`)
- Forced wide clear (`2x tool_radius`) at break point to prevent re-entry (`adaptive.rs:1190-1204`)
- Entry point spreading via boundary walk + grid scan fallback (`adaptive.rs:646-874`) ensures next pass starts in a different material region

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Med | `min_cutting_radius` not exposed in GUI for either 2D or 3D adaptive | `properties/mod.rs` |
| 2 | Med | 3D adaptive `entry_style`, `detect_flat_areas`, `region_ordering`, `fine_stepdown` not exposed in GUI | `properties/mod.rs` |
| 3 | Med | `stock_to_leave_radial` (3D) not exposed in GUI (only axial shown) | `configs.rs` |
| 4 | Low | `#[allow(dead_code)]` on `search_direction()` (`adaptive.rs:457`) — unused wrapper around `search_direction_with_metrics` | `adaptive.rs:457` |
| 5 | Low | `adaptive_segments_with_debug()` spans ~335 lines (948-1283) — could extract pass loop | `adaptive.rs:948-1283` |
| 6 | Low | Wall-tangent bias magic constants (penalty 0.15, gradient threshold) lack justification | `adaptive.rs:520-536` |
| 7 | Low | Link distance threshold `tool_radius * 6.0` undocumented | `adaptive.rs:1073-1087` |
| 8 | Low | Forced clear radius `2x tool_radius` aggressive for narrow pockets | `adaptive.rs:1190-1204` |

## Performance Notes

- **No Rayon parallelism** anywhere in adaptive.rs or adaptive3d.rs despite `parallel` feature flag existing in Cargo.toml
- **Main bottleneck**: `compute_engagement()` called ~20 times per step x thousands of steps; each call scans O(tool_radius^2/cell_size^2) cells
- Estimated for large region: 250M+ cell checks per operation
- Opportunities: parallel candidate evaluation in direction search, SIMD for cell iteration
- Allocations are O(n) in grid size — no O(n^2) patterns found
- One-time BFS for boundary distances (`compute_boundary_distances`, `adaptive.rs:278-313`) scales with region area

## Test Coverage

**37 unit tests** covering:
- Material grid operations (6 tests)
- Boundary distance field (3 tests)
- Engagement computation (3 tests)
- Direction search including wall bias (3 tests)
- Entry point spreading (2 tests)
- Path linking (2 tests)
- Coarse search phases (2 tests)
- Slot clearing (2 tests)
- Full toolpath generation (3 tests)
- Corner blending (4 tests)
- Path simplification (2 tests)
- Min cutting radius integration (1 test)

**Shared code (adaptive_shared.rs, 150 LOC)**: tested indirectly via adaptive.rs tests; clean split, no duplication.

## Test Gaps

- No tests for very narrow slots (< 2x tool_radius)
- No tests for complex multi-island pockets (only 1 holes test exists)
- No tests for stepover near tool diameter (only 20% tested)
- No tests for deep cuts with many depth passes (50+ levels)
- No parameter validation tests (0, negative, NaN stepover/radius)
- No cancellation mid-operation tests
- No L-shaped, T-shaped, C-shaped pocket tests
- No extreme tolerance tests (very fine grid causing memory pressure)
- No CLI-to-output or GUI-config-to-worker integration tests
- No property-based / fuzz tests

## Suggestions

- Expose `min_cutting_radius` in the GUI — it's wired end-to-end but invisible to users
- Expose 3D adaptive `entry_style`, `detect_flat_areas`, and `region_ordering` in GUI
- Add parameter validation at the core level (reject stepover <= 0, tool_radius <= 0)
- Consider parallelizing direction search candidate evaluation with Rayon
- Document magic constants (wall bias 0.15, link threshold 6x, forced clear 2x)
- Add narrow-slot and multi-island tests as the highest-priority test gap
