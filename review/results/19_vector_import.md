# Review: Vector Import (SVG + DXF)

## Summary
Both SVG and DXF import are functional with correct polygon output (CCW exterior, CW holes, containment detection). SVG leverages usvg for robust element support and transform handling with good curve flattening (0.1mm tolerance). DXF supports 4 entity types (LwPolyline, Polyline, Circle, Ellipse) but lacks Lines, Arcs, and Splines. Both importers silently ignore open paths and enforce consistent winding. Main concerns are unused error variants in both modules, hardcoded tolerances with no UI, and missing entity support in DXF.

## Findings

### SVG Import (svg_input.rs)

#### Elements Supported
- **Paths** (`<path>`): Directly parsed. `svg_input.rs:41-52`
- **Rects, Circles, Polygons**: Transparently supported — usvg normalizes them to paths before code sees them
- **Groups** (`<g>`): Recursive traversal via `visit_group()`. `svg_input.rs:41-52`
- **Text**: NOT explicitly handled — depends on usvg conversion (may or may not work)
- **Dependency**: usvg 0.47 (workspace Cargo.toml)

#### Curve Flattening
- **Tolerance**: 0.1mm hardcoded in GUI import path. `import.rs:45`
- **Method**: Recursive De Casteljau binary subdivision at t=0.5 for both quadratic (svg_input.rs:121-140) and cubic (svg_input.rs:143-166) beziers
- **Flatness test**: `d^2 <= tol^2 * chord^2` — standard and correct
- **Degenerate curves**: `chord_sq < 1e-20` fallback prevents infinite recursion. `svg_input.rs:128, 151`
- **Quality**: 0.1mm is reasonable for wood routing (well below visible cut tolerance)

#### Transform & Units
- **Transforms**: Handled by usvg — all nested transforms and viewBox flattened before reaching code. `svg_input.rs:33`
- **Units**: No unit interpretation. Assumes 1 SVG unit = 1 mm. GUI sets `ModelUnits::Millimeters`. `import.rs:59`
- **Limitation**: Users must pre-scale SVGs to mm (e.g., Inkscape default pixels won't import at correct size)

#### Path Handling
- **Multi-path**: Each closed path becomes separate `Polygon2`, then `detect_containment()` nests them. `svg_input.rs:37, polygon.rs:236`
- **Open paths**: Silently ignored — only emits paths after `PathSegment::Close`. `svg_input.rs:94`
- **Winding**: All polygons normalized via `ensure_winding()` — CCW exterior, CW holes. `svg_input.rs:109`
- **Nested paths**: One level deep containment (rect > hole supported, rect > hole > island only partially). `polygon.rs:271`
- **Vertex dedup**: Duplicate closing vertex removed if within 1e-6. `svg_input.rs:98-105`
- **Minimum vertices**: Paths with < 3 vertices filtered. `svg_input.rs:96`

#### Testing (9 tests, svg_input.rs:168-332)
- `test_load_rect` — rectangle parsing + area
- `test_load_triangle` — polygon parsing
- `test_load_circle` — bezier flattening, vertex count, area
- `test_load_multiple_paths` — 2 independent shapes
- `test_empty_svg` — empty input returns empty Vec
- `test_open_path_ignored` — open paths rejected
- `test_closed_path_with_curves` — cubic splines
- `test_containment_rect_with_circle_hole` — nested hole detection + area
- `test_winding_is_ccw` — winding correctness

---

### DXF Import (dxf_input.rs)

#### Entities Supported
- **LwPolyline**: With bulge arcs. `dxf_input.rs:42-50`
- **Polyline**: Old-style with bulge arcs. `dxf_input.rs:52-63`
- **Circle**: Simple circles. `dxf_input.rs:65-76`
- **Ellipse**: Rotated ellipses. `dxf_input.rs:78-84`
- **NOT supported**: Lines, standalone Arcs, Splines — all silently ignored via `_ => {}`. `dxf_input.rs:86`

#### Arc Tolerance
- **Parameter**: 5.0 degrees hardcoded in GUI. `import.rs:67`
- **Converted to radians**: `arc_step_rad = arc_tolerance_deg.to_radians()`. `dxf_input.rs:38`
- **Step count**: `n_steps = (abs_sweep / arc_step).ceil()`, minimum 8 for circles/ellipses. `dxf_input.rs:175, 192-193, 218-219`
- **Reasonableness**: 5.0 deg = ~72 segments per full circle — adequate for wood routing

#### Layer & 3D Support
- **Layers**: NOT supported. Layer information discarded during parsing. No filtering by layer.
- **3D entities**: Z-coordinates silently dropped from all entity types. Intentional for 2.5D CAM.

#### Path Handling
- **Closed paths required**: LwPolyline/Polyline must have `is_closed() == true`. `dxf_input.rs:43, 53`
- **Open paths**: Silently ignored (no error, no log)
- **Minimum vertices**: Polylines with < 3 vertices ignored. `dxf_input.rs:43, 55`
- **Winding**: `ensure_winding()` called on every imported polygon. `dxf_input.rs:47, 59, 74, 82`
- **Bulge direction**: Sign determines arc direction (positive = CCW, negative = CW). `dxf_input.rs:135, 167, 180`

#### Testing (7 tests, dxf_input.rs:242-409)
- `test_lwpolyline_rectangle` — basic 4-point polyline + area
- `test_lwpolyline_open_ignored` — open polyline rejected
- `test_circle` — circle tessellation + area within 5%
- `test_lwpolyline_with_bulge_arcs` — bulge tessellation creates extra vertices
- `test_multiple_entities` — mixed entity types (rect + circle)
- `test_winding_is_ccw` — CW input corrected to CCW
- `test_too_few_vertices_ignored` — 2-vertex polyline rejected

---

### Polygon Representation (polygon.rs)
- **Type**: `Polygon2 { exterior: Vec<P2>, holes: Vec<Vec<P2>> }`. `polygon.rs:16-18`
- **Winding convention**: Exterior CCW (positive signed area), holes CW (negative)
- **Containment detection**: Sorts by area (largest first), ray-casting point-in-polygon test, one level deep nesting. `polygon.rs:236-244`
- **Shared**: Both SVG and DXF importers output the same `Vec<Polygon2>` type with identical winding/containment guarantees

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Med | DXF: Lines, standalone Arcs, and Splines silently ignored — common DXF entities | dxf_input.rs:86 |
| 2 | Med | DXF: `path.to_str().unwrap_or("")` — non-UTF-8 path silently loads empty file as success | dxf_input.rs:24 |
| 3 | Low | SVG: `NoPaths` error variant defined but never raised — dead code | svg_input.rs:11-19 |
| 4 | Low | DXF: `NoEntities` error variant defined but never raised — dead code | dxf_input.rs:11-17 |
| 5 | Low | SVG: Units fixed at mm — no px/in/cm conversion; users must pre-scale | import.rs:59 |
| 6 | Low | DXF: Layer information discarded — cannot filter or organize by layer | dxf_input.rs:40-86 |
| 7 | Low | Both: Tolerances hardcoded in GUI (SVG 0.1mm, DXF 5.0 deg) — no user configuration | import.rs:45, 67 |
| 8 | Low | SVG: Multi-level nesting only one level deep (island inside hole not nested correctly) | polygon.rs:271 |
| 9 | Low | DXF: No Polyline entity tests (only LwPolyline tested) | dxf_input.rs:242-409 |
| 10 | Low | DXF: No Ellipse entity tests | dxf_input.rs:242-409 |

## Test Gaps

### SVG
- No test for malformed SVG parse error recovery
- No test for text elements
- No test for multi-level nesting (island inside hole)
- No test for very complex paths (thousands of segments)

### DXF
- No test for Polyline entities (only LwPolyline)
- No test for Ellipse entities
- No test for empty DXF file
- No test for malformed DXF data
- No test for containment detection with DXF-imported polygons
- No test for non-UTF-8 file paths
- No test for `NoEntities` error variant

### Both
- No test for I/O error propagation
- No performance test for large/complex files

## Suggestions
- Add DXF Line and Arc entity support (common in CAD exports)
- Fix `path.to_str().unwrap_or("")` in DXF — should propagate error for non-UTF-8 paths
- Remove or use the `NoPaths`/`NoEntities` error variants (currently dead code)
- Consider exposing curve tolerance in the GUI for both importers
- Add Ellipse and Polyline test cases for DXF
- Consider adding SVG unit detection (viewport dimensions can hint at intended units)
- Document the one-level nesting limitation for complex SVG artwork
