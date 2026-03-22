# Review: Minor 2D Operations (Zigzag, Trace, Drill, Chamfer)

## Summary

All four operations are fully implemented, integrated end-to-end through GUI, and have comprehensive test coverage. They are thin, focused layers on top of shared geometry and simulation infrastructure. The implementations are clean, well-documented, and follow consistent patterns with dressups and boundary clipping support. No critical issues found.

## Findings

### Zigzag (`crates/rs_cam_core/src/zigzag.rs`)

- **Distinct from pocket-zigzag**: Standalone operation in `zigzag.rs`, also reused as `PocketPattern::Zigzag` within pocket ops (execute.rs:433)
- **Algorithm** (lines 44-146): Polygon inset by tool radius, scan lines perpendicular to raster angle, line intersections via projection, alternating direction for true zigzag
- **Parameters** (lines 12-27): `tool_radius`, `stepover`, `cut_depth`, `feed_rate`, `plunge_rate`, `safe_z`, `angle` (degrees, converted to radians on wiring at execute.rs:625)
- **Depth stepping**: Fully integrated via `depth_stepped_toolpath()` and `depth_per_pass` config (ZigzagConfig:345)
- **Empty handling**: Returns empty if polygon or inset collapses (lines 50-62)
- **Tests** (lines 214-392): Square polygon scan lines, alternating direction, multi-pass depth, non-convex L-shape, too-small pockets, 90° angle
- **Code quality**: No unwrap() in core logic; uses `unwrap_or()` for NaN comparisons

### Trace (`crates/rs_cam_core/src/trace.rs`)

- **Use case**: Contour following (engraving/decorative routing), NOT offset cutting — follows polygon paths exactly at specified depth
- **Compensation model** (lines 11-20, 54-70): `None` (center on path), `Left` (offset outward), `Right` (offset inward), uses `offset_polygon()`; returns empty if offset collapses
- **Execution** (lines 86-112): Traces exterior ring first, then holes; rapid approach → plunge → feed around contour → close loop → retract
- **Integration**: `UiOperationFamily::Trace` (catalog.rs:226), `PassRole::Finish`, full dressup/boundary support via semantic tracing (execute.rs:1919-1934)
- **Tests** (lines 115-408): 4-vertex exact sequence, compensation offset both directions, depth-stepping 3-level, polygon with holes, empty polygon, compensation collapse, rapids-at-safe-z
- **Code quality**: Clean; one `panic!()` in test only (line 168)

### Drill (`crates/rs_cam_core/src/drill.rs`)

- **Drill cycles** (lines 9-25):
  - `Simple` (G81): Feed to depth, rapid out
  - `Dwell(duration)` (G82): Same as Simple, dwell is post-processor concern
  - `Peck(peck_depth)` (G83): Full retract between pecks
  - `ChipBreak(peck_depth, retract_amount)` (G73): Small retract for chip breaking
- **Hole generation** (execute.rs:1944-1960): Extracts centroid from each polygon `(sum_x/n, sum_y/n)` — works for circles, less accurate for irregular polygons
- **Peck mechanics** (lines 78-109): Iterative feed with 0.5mm clearance constant, full retract to retract_z between pecks, re-entry at `previous_depth + 0.5mm`
- **Visit order**: Caller's responsibility (no TSP optimization)
- **Tests** (lines 143-364): Single-hole simple drill, dwell equivalence, peck multi-plunge with retract, peck depth > total depth (clamping), chip-break, multiple holes, empty hole list
- **Code quality**: Well-formed, no safety issues

### Chamfer (`crates/rs_cam_core/src/chamfer.rs`)

- **Algorithm** (lines 32-87): V-bit edge chamfering; `depth = (chamfer_width + tip_offset) / tan(half_angle)`; effective radius at depth: `r = depth * tan(half_angle) = chamfer_width + tip_offset`; delegates to `profile_toolpath()` with computed offset
- **Tool support**: V-Bit only — error if non-V-bit selected (execute.rs:1058)
- **Geometry** (lines 50-68): Profile offset places tool center at distance r from edge; chamfer starts at polygon edge, extends w mm inward
- **Tests** (lines 89-273): 45° V-bit depth calc, 30° V-bit with tan() verification, tip offset depth increase, square chamfer produces moves, cut depth verification, offset outside boundary, effective radius formula
- **Code quality**: Clean, no safety issues

### Shared Concerns

- **Consistent architecture**: All four follow identical wiring in execute.rs — `require_polygons(req)`, per-polygon accumulate, depth stepping, semantic tracing, return empty on failure
- **Heights**: All use `effective_safe_z(req)` for rapids; cutting Z from config; profile-based heights handled by dressup layer
- **Dressups & boundary**: Raw toolpaths generated first, dressups applied (execute.rs:289), boundary clipping after (execute.rs:292-342)
- **Multi-pass depth**: Zigzag/Trace use `depth_stepped_toolpath()` inline; Drill/Chamfer are single-pass
- **Tool compensation**: Zigzag/Drill inset by tool radius; Trace has left/right/none; Chamfer uses V-bit effective radius at depth — all via shared `offset_polygon()`
- **Empty input**: All handle gracefully (empty return or error for drill with no circles)

### Integration Status

| Operation | Core | Config | Dispatch | Execute | GUI Panel | Catalog | CLI |
|-----------|------|--------|----------|---------|-----------|---------|-----|
| Zigzag    | Yes  | Yes    | Yes      | Yes     | Yes       | Yes     | No  |
| Trace     | Yes  | Yes    | Yes      | Yes     | Yes       | Yes     | No  |
| Drill     | Yes  | Yes    | Yes      | Yes     | Yes       | Yes     | No  |
| Chamfer   | Yes  | Yes    | Yes      | Yes     | Yes       | Yes     | No  |

None have CLI commands — available only via GUI and project TOML.

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Low | Drill centroid uses simple average `(sum_x/n, sum_y/n)` — works for circles but less accurate for irregular polygons | execute.rs:1949-1954 |
| 2 | Low | No explicit validation of angle bounds in Zigzag — any f64 accepted (safe geometrically, but could confuse users) | zigzag.rs:12-27 |
| 3 | Low | Chamfer requires V-Bit but only validates at execution time, not at operation creation in UI | execute.rs:1058 |

## Test Gaps

Current coverage is good across all four operations. Potential additions (enhancements, not critical gaps):

- Zigzag: Very large stepover (single line) and very small stepover (many lines)
- Trace: Multiple disconnected polygons with order verification
- Drill: `retract_z < depth` edge case
- Chamfer: Variety of V-bit angles (60°, 90°, 120° included)

## Suggestions

1. **Drill hole ordering**: Consider adding TSP-like ordering option to minimize rapid travel between holes
2. **Trace compensation docs**: The CCW exterior convention for left/right compensation could be more explicit in user-facing help text
3. **V-bit tool gating in UI**: Gate chamfer operation creation to V-Bit tool selection rather than failing at execution
4. **Zigzag angle normalization**: Normalize angle to [0°, 360°) in config defaults to prevent user confusion
5. **Drill centroid documentation**: Add comment explaining hole location is polygon centroid (for use cases beyond circles)
