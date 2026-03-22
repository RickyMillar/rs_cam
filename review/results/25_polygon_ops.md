# Review: Polygon Operations

## Summary
The 2D polygon system is well-designed and robust for its scope. It uses `cavalier_contours` v0.7 for arc-preserving offsets with clear CCW/CW winding conventions, defensive collapse handling, and 433 lines of tests. The main risks are a silent hole re-pairing fallback that could mask errors, no input validation for degenerate polygons, and dead `geo` conversion code that's tested but never called in production.

## Findings

### Offset Operations
- **Arc-preserving offsets** via `cavalier_contours::Polyline::parallel_offset()` for simple polygons and `Shape::parallel_offset()` for polygons with holes (`polygon.rs:124-200`)
- **Sign convention** well-documented (`polygon.rs:128-130`): positive = inward (shrink), negative = outward (grow) per cavalier_contours CCW convention
- **Collapse handling**: returns empty `Vec<Polygon2>` when offset exceeds polygon half-width; all callers check `.is_empty()` before using results
- **Pocket clearing** via `pocket_offsets()` (`polygon.rs:202-224`): loops inward offsets by stepover until collapse, returns `Vec<Vec<Polygon2>>` layers
- **Arc flattening caveat** (`polygon.rs:103-108`): despite cavalier_contours being arc-preserving, output `Polyline::from_pline()` flattens arcs to line segments — arc data is not preserved through the pipeline

### Hole Re-Pairing After Offset
- After offsetting a polygon-with-holes, results are separated into CCW (exterior) and CW (hole) polylines (`polygon.rs:160-196`)
- Each hole is matched to its containing exterior via `contains_point()` (`polygon.rs:186`)
- **Silent fallback** at `polygon.rs:192-194`: if containment test fails, hole is attached to first polygon without any warning or logging

### Boolean Operations
- **Not implemented** — no union/intersection/difference operations in polygon.rs
- `geo` crate imported only for type conversion (`to_geo_polygon()` / `from_geo_polygon()`, `polygon.rs:79-90`) — tested but never called in production code
- Boolean-like behavior achieved implicitly via containment nesting (`detect_containment()`) and point-in-polygon exclusion checks

### Winding and Orientation
- **Convention**: exterior = CCW (positive signed area), holes = CW (negative signed area), documented at `polygon.rs:11-12`
- **Enforcement**: `ensure_winding()` (`polygon.rs:59-68`) reverses rings as needed; called at SVG import (`svg_input.rs:109`), DXF import (`dxf_input.rs:47,59,73`), and containment detection (`polygon.rs:243`)
- **Island/hole detection**: `detect_containment()` (`polygon.rs:236-295`) sorts by area, ray-casting containment, single-level nesting only (each polygon can be a hole of at most one outer)

### Data Flow: Import → Operations → Toolpaths
```
SVG/DXF → flatten curves → Vec<Polygon2> → detect_containment() (nest holes)
  → 2.5D ops: offset_polygon(boundary, tool_radius) → tool center paths
  → 3D ops: contains_point checks, marching squares on grids
  → Output: Vec<P2> → 3D via Z depth → toolpath IR
```

### Callers
- **Pocket** (`pocket.rs:48`): inward offset by tool_radius, then repeated stepover offsets
- **Profile** (`profile.rs:61`): single offset with side-dependent sign
- **Rest** (`rest.rs:63`): offset by prev_tool_radius to define reachable region
- **Inlay** (`inlay.rs:78`): inward offset by `pocket_depth * tan(half_angle)`
- All callers check `exterior.len() >= 3` before using results

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Med | Silent hole re-pairing fallback: if containment test fails, hole attaches to first polygon without warning — could silently produce wrong geometry | `polygon.rs:192-194` |
| 2 | Low | Dead code: `to_geo_polygon()` / `from_geo_polygon()` are tested but never called in production (42 lines) | `polygon.rs:79-90`, `polygon.rs:368-392` |
| 3 | Low | Arc data lost through offset pipeline despite using arc-preserving library — comment acknowledges this but limits offset quality | `polygon.rs:103-108` |
| 4 | Low | `polygon_bbox()` returns `(INF, INF, -INF, -INF)` on empty input — no validation | `polygon.rs` |

## Test Gaps
- No tests for degenerate inputs (< 3 vertices before offset)
- No tests for self-intersecting input polygons
- No tests for very small polygons or slivers after offset
- No tests for the hole re-pairing fallback path (unmatched holes)
- No tests for zero-length segments in polygon rings
- No tests for floating-point precision edge cases in containment

## Suggestions
- Add logging or return an error in the hole re-pairing fallback (`polygon.rs:192-194`) instead of silently attaching to first polygon
- Add a test case that triggers the fallback path to verify behavior
- Consider removing dead `geo` conversion code, or document it as reserved for future boolean ops
- Add an area-based sliver filter after offset to prevent degenerate thin-wall toolpaths
- Consider adding input validation (min vertex count, zero-length segment removal) in `Polygon2::new()`
