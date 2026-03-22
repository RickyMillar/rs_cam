# Review: VCarve Operation

## Summary

The VCarve operation is a well-designed engraving toolpath generator using scan-line sampling to compute variable depth V-bit cuts. The core algorithm correctly computes distances to polygon boundaries and converts them to depths via the half-angle tangent formula. Tool validation is properly enforced at runtime, rejecting non-V-bit tools. Two medium-severity issues found: misleading max_depth=0.0 documentation and missing validation that computed depth doesn't exceed tool cone height.

## Findings

### Correctness

**Algorithm approach (vcarve.rs:68-120)**
- Uses distance-from-width approach (not medial axis/Voronoi): scans horizontal lines across inset polygon boundary using `zigzag_lines()`, computes minimum Euclidean distance to any polygon edge for each sample point
- Converts distance to depth via: `depth = distance / tan(half_angle)`, then clamps to `max_depth`
- Geometrically sound for V-bit engraving: V-groove naturally tapers linearly with width

**V-bit angle conversion (execute.rs:527-550)**
- V-bit tool validation correctly enforces V-Bit tool requirement (line 531): `"VCarve requires V-Bit tool"`
- Half-angle computed correctly from included angle: `(req.tool.included_angle / 2.0).to_radians()` (line 530)
- Angle validation in vbit.rs:27-32 asserts `0 < included_angle < 180`, preventing degenerate cones

**Max depth limiting (vcarve.rs:108)**
- Uses `.min(params.max_depth)` to clamp depth correctly
- Comment at line 23 says "If 0.0, uses the full cone depth" but `min(anything, 0.0)` always returns 0.0 — producing zero-depth cuts everywhere, not the documented behavior

**Sharp corners (geo.rs:194-214, vcarve.rs:41-64)**
- Point-to-segment distance correctly clamps parameter t to [0.0, 1.0], ensuring distance to vertices is computed properly
- At sharp corners where multiple edges meet, minimum distance is correctly chosen
- Distance approaches zero at corners → depth → 0, which is correct V-carve behavior

**Degenerate segment handling (geo.rs:199-203)**
- If ab_len_sq < 1e-20, returns distance to point `a`, avoiding division by zero

**Scan line generation (vcarve.rs:82-84, 99)**
- Inset = `params.tolerance.min(0.05)` ensures small insets don't exceed tool size
- Sample step = `params.tolerance.max(0.05)` ensures minimum 0.05mm sampling
- Sampling strategy can miss narrow features narrower than the sampling interval

### Integration

**CLI wiring (main.rs:31)**
- VCarveParams and vcarve_toolpath properly exported and used in CLI

**GUI operation execution (execute.rs:30-56, 527-550)**
- VCarveConfig correctly implements SemanticToolpathOp trait (line 1807)
- Tool requirement checked at runtime with clear error message
- Parameters properly passed from VCarveConfig to VCarveParams (lines 537-545)
- Multiple polygons handled correctly: iterates and merges toolpaths

**Tool selection validation**
- Runtime check enforces V-Bit tools only — clear error if user selects flat end mill
- UI does not currently prevent V-Carve from being selectable with non-V-Bit tools (relies on runtime error)

**Config structure (configs.rs:267-285)**
- VCarveConfig correctly includes max_depth, stepover, feed_rate, plunge_rate, tolerance
- No tool included_angle field in config; angle is read from selected tool at run time (execute.rs:530)
- Clean design but angle validation happens late (at compute time, not config time)

### Edge Cases

- **Very thin strokes (< tolerance)**: If stroke width < sample_step (default 0.05mm), sampling may be sparse or miss interior regions entirely
- **Tool angle vs feature width mismatch**: Very sharp V-bits with wide features could compute depths exceeding tool cone height. `VBitEndmill.height_at_radius()` returns None for r > radius (vbit.rs:65-66) but vcarve operation doesn't validate this
- **Empty/degenerate polygons**: Polygon with area ~0 produces empty zigzag_lines, resulting in empty toolpath. Tested in `test_vcarve_empty_polygon()` (line 296)
- **Degenerate angle**: tan_half < 1e-10 silently returns empty toolpath (vcarve.rs:78-80)

### Code Quality

- **No unwrap() or panic()** in vcarve.rs
- **Error handling**: execute.rs `run_vcarve` returns `Result<Toolpath, String>` with clear error for wrong tool type
- **Numerical stability**: Reasonable thresholds (1e-10 for tan_half, 1e-20 for segment length, 1e-8 for boundary checking in vbit.rs)
- **Code clarity**: Well-commented with clear algorithm explanation (lines 1-12, 68-75)
- **Test coverage**: 6 tests covering basic functionality, depth variation, max depth clamping, different tool angles, holes, empty polygons

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Medium | max_depth=0.0 documentation says "uses full cone depth" but actually clamps all depths to 0.0 (no cut) | vcarve.rs:23, line 108 |
| 2 | Medium | No validation that computed depth <= tool cone height (tool_radius / tan(half_angle)) | vcarve.rs:108, vbit.rs:50 |
| 3 | Low | Very thin strokes (< tolerance) may have sparse/missing depth samples | vcarve.rs:86, 99-103 |
| 4 | Low | No UI prevention of V-Carve with non-V-Bit tools (relies on runtime error) | execute.rs:531 |
| 5 | Low | Degenerate angle < 1e-10 silently produces empty toolpath with no user feedback | vcarve.rs:78-80 |

## Test Gaps

- No test for max_depth=0.0 behavior (actual vs. documented)
- No test for computed depth exceeding tool cone height
- No test for very thin features (width < sample tolerance)
- No test for holes with varying widths
- No test for sampling with large tolerance values
- No test comparing multiple tool angles on same geometry (to verify angle affects depth linearly)

## Suggestions

1. **Fix max_depth=0.0 semantics**: Either implement "unlimited depth" behavior (e.g. treat 0.0 as f64::MAX) or update the comment to document that 0.0 means "no cutting"
2. **Validate cone height**: Add check that max_depth <= tool_radius / tan(half_angle) in `run_vcarve()`, or clamp to achievable tool depth
3. **UI tool filter**: Add V-Bit tool type filter to operation creation UI for V-Carve, preventing late-stage runtime errors
4. **Improve thin feature handling**: Consider adaptive sampling based on local feature width, or document that features narrower than sample step may be unreliable
5. **Surface degenerate angle**: Return an error rather than silently producing empty toolpath when tool angle is near-zero
