# Review: Rest Machining

## Summary

Rest machining is compact, clean, and algorithmically correct. The core logic (rest.rs) correctly identifies regions left by a larger previous tool using polygon offset + containment testing, with 12 passing unit tests, zero `unwrap()` calls, and solid CLI wiring. **One critical bug found**: the GUI double-converts the scan line angle from degrees to radians (execute.rs:570), producing wrong scan directions for any non-zero angle.

## Findings

### Algorithm Correctness
- **Swept area calculation** (`rest.rs:63`): uses `offset_polygon(polygon, prev_tool_radius)` to compute the region reachable by the previous (larger) tool's center — everything inside this offset is already cleared
- **Remaining material identification** (`rest.rs:110-118`): for each sampled point on zigzag scan lines, checks `point_in_any_polygon(&p, &large_reachable)` — points NOT in the reachable set are rest regions
- **Tool type handling**: purely geometric (uses radius only, not tool shape) — works for any flat-endmill combination
- **Depth handling**: `cut_depth` is a single Z level; depth stepping handled by caller via `depth_stepped_toolpath()` in both CLI (`job.rs:483`) and GUI (`execute.rs:559`)

### Edge Case Handling
- Same-size tools: returns empty toolpath (`rest.rs:58-59`) — tested
- Previous tool can't fit in polygon: falls back to full zigzag covering entire region (`rest.rs:74-86`) — tested
- Narrow channels: entire channel treated as rest region — tested
- Polygon with holes: handled by `offset_polygon` → `zigzag_lines` — tested
- Angled scan lines: passed through to `zigzag_lines` — tested

### Integration (CLI)
- `job.rs:458-501`: properly validates `prev_tool` exists and resolves diameter
- Angle passed as degrees (correct) — matches `RestParams` and `zigzag_lines` expectations
- Depth stepping applied correctly

### Integration (GUI)
- `configs.rs`: `RestConfig` has `prev_tool_id: Option<ToolId>`
- `properties/mod.rs:1323-1338`: ComboBox UI for selecting previous tool
- `events.rs:801-812`: resolves `prev_tool_id` to radius at compute time
- Validation present: checks prev_tool is set and is larger than current tool

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | **Critical** | GUI double-converts angle: `cfg.angle.to_radians()` passed to `RestParams::angle` which is documented as degrees and forwarded to `zigzag_lines(angle_deg)` which converts internally. Result: 45-degree angle becomes ~0.785-degree angle. | `execute.rs:570` |
| 2 | Low | No parameter validation at core level (negative stepover, zero tool_radius) | `rest.rs` |
| 3 | Low | `stock_source` (Fresh vs Remaining) not used in `run_rest()` — rest always uses 2D polygon geometry regardless of stock state | `execute.rs:552-577` |

## Test Gaps

- No integration test comparing CLI vs GUI output (would have caught the angle bug)
- No test for angle != 0 in the GUI path specifically
- No test for multiple sequential rest passes (rest-of-rest)
- No degenerate polygon tests (collinear, self-intersecting)
- No performance test for very fine stepover on large polygons

## Suggestions

- **Fix immediately**: remove `.to_radians()` from `execute.rs:570` — change `angle: cfg.angle.to_radians()` to `angle: cfg.angle`
- Add an integration test that runs rest machining through both CLI and GUI paths with a non-zero angle and asserts equivalent output
- Consider adding tool-radius validation at the core level (currently only GUI validates prev > current)
- Document that rest machining is purely 2D-geometric and does not consume simulation stock history
