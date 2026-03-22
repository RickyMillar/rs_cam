# Review: Pocket Operation

## Summary

The Pocket operation is a 2.5D contour-parallel or zigzag clearing operation for flat-bottomed pockets. The implementation is solid: core generation (`pocket.rs`) produces concentric inward offsets using `offset_polygon` with correct tool radius compensation, polygon offsetting (`polygon.rs`) uses cavalier_contours for arc-preserving parallel offset with proper hole handling, and both GUI and CLI are fully wired with feature parity. 9 comprehensive tests cover the main paths and key edge cases. Only low-severity issues found.

## Findings

### Correctness

- **Offset sign convention**: `pocket.rs:48` calls `offset_polygon(polygon, tool_radius)` with positive distance. `polygon.rs:128-130` documents distance > 0 = inward (shrink). Correct — tool edge touches wall, not center.
- **Contour generation**: `pocket.rs:46-98` generates concentric contours via repeated stepover offset until collapse. Loop at `pocket.rs:70-94` correctly terminates when `offset_polygon` returns empty.
- **Islands/holes**: Holes are correctly reversed to CCW at lines 62-64, 80-82 for directional consistency.
- **Climb vs conventional**: `pocket.rs:110-114` reverses contour traversal when `params.climb == true`. Test `test_pocket_climb_vs_conventional` (lines 269-317) verifies both paths produce same distance but opposite directions.
- **Depth stepping**: `PocketConfig` has `depth` and `depth_per_pass` but no depth_per_pass handling in `pocket.rs` core. GUI (`execute.rs:1628-1629`) uses `make_depth_with_finishing()` creating a `DepthStepping` object and calls `pocket_contours()` at each Z level. CLI uses `depth_stepped_toolpath()` wrapper. Both approaches are equivalent and correct — depth stepping is caller responsibility per architectural design.
- **Zigzag pattern**: `execute.rs:1691-1696` calls `zigzag_lines()`, then wraps each line via `line_toolpath()`. `zigzag.rs:59` offsets inward by tool_radius before generating scan lines. `zigzag.rs:106-146` generates perpendicular scan lines clipped to polygon with automatic zigzag alternation. Correctly implemented.

### Integration

- **Full wiring GUI → compute → core**: User sets pattern/stepover/depth/depth_per_pass/feed_rate/plunge_rate/climb in GUI properties panel → `GenerateToolpath` event → `execute_toolpath()` in compute worker → `PocketConfig` instantiated → semantic `generate_with_tracing()` → contour or zigzag generation → dressups applied → G-code. Complete chain.
- **CLI parity**: `job.rs:310-350` implements pocket via `pocket_toolpath()` and `zigzag_toolpath()` with `depth_stepped_toolpath()`. Parameters: stepover, depth, depth_per_pass, feed_rate, plunge_rate, pattern, climb, angle all present. Full parity confirmed.

### Edge Cases

- **Very small pockets (< tool diameter)**: First offset by tool radius collapses immediately → returns empty Vec → empty toolpath. Correctly handled.
- **Zero or negative depth**: GUI slider enforces `0.1..=100.0` range. Core `pocket_toolpath()` accepts any `cut_depth` with no validation. `cut_depth = 0.0` produces contours at Z=0 (no actual cut).
- **Stepover > tool diameter**: `offset_polygon()` returns empty on first iteration → no intermediate contours. Geometrically valid but may surprise users.
- **Complex polygons with multiple holes**: `pocket.rs:52-95` iterates all compensated results. `polygon.rs:157-199` pairs holes to boundaries via containment test with conservative fallback at lines 192-195.
- **Non-convex (concave) pockets**: Tested in `polygon.rs:587-612` (L-shaped offset) and `pocket.rs:423-452` (L-shaped pocket). Works correctly.

### Code Quality

- **unwrap() audit**: No unwrap() calls in `pocket.rs` or `polygon.rs`. Uses `.ok_or()`, `.map()`, pattern matching throughout.
- **Error paths**: Core operations return `Vec<Polygon2>` (empty if failed) or `Toolpath` (empty if failed). GUI/CLI layer handles empty results gracefully.
- **Shared offset logic**: Both pocket (`positive distance`) and profile (`negative distance`) use the same underlying `offset_polygon()` from `polygon.rs`. Good code reuse, single source of truth.
- **Test coverage**: 9 tests in `pocket.rs` (lines 141-453): basic contour generation, move structure, rapid-plunge-feed-retract pattern, collapse handling, climb vs conventional, Z height validation, island handling, non-convex geometry. Good coverage.

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Low | No validation that `cut_depth` is negative (non-zero) — silent no-op if zero | `pocket.rs:38` |
| 2 | Low | `finishing_passes` field exists in `PocketConfig` but is not exposed in GUI properties panel | `configs.rs:187` vs `pocket.rs:102-185` |
| 3 | Low | `finishing_passes` concept is irrelevant for Zigzag pattern but still read from config | `execute.rs:1629` |

## Test Gaps

- No test for zero/positive `cut_depth` behavior (produces no actual cut)
- No explicit test for large stepover (> pocket width)
- No performance/degeneracy test for very small stepover (< 0.1 mm, excessive contours)
- No test for hole island that collapses during first offset while outer boundary survives

## Suggestions

1. **Validate depth sign** in `pocket_toolpath()`: return empty toolpath or error if `cut_depth >= 0.0` to prevent silent no-ops
2. **Expose or remove `finishing_passes`**: either wire it to the GUI properties panel if the feature is intended, or remove the unused field
3. **Grey out irrelevant fields**: hide `finishing_passes` in UI when Zigzag pattern is selected
4. **Add stepover range guidance**: warn if stepover suggests pocket will be roughed in one pass
5. **Clarify depth stepping in docstring**: note that `pocket_toolpath()` generates contours for a single Z depth; depth stepping is caller's responsibility
