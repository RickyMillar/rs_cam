# Progress Tracker

Read this FIRST at the start of every session. Update LAST before ending.

## Current Phase: 4 - High-Value Features (NEXT)

### What Exists
- [x] Research complete (research/ directory - 8 synthesized docs + 4 raw dumps)
- [x] Architecture complete (architecture/ directory - user stories, requirements, high-level design)
- [x] CLAUDE.md guardrails in place
- [x] Cargo workspace initialized
- [x] Core library + CLI compiling, 466 tests passing (464 unit + 2 integration)
- [x] Phase 1 complete: STL → drop-cutter → G-code pipeline with 3D HTML viewer
- [x] Phase 2 complete: 2.5D operations (pocket, profile, zigzag, depth stepping, SVG/DXF input, dressups, CLI)
- [x] Phase 3 complete: Advanced tools (BullNose, VBit, TaperedBall), push-cutter, waterline, arc fitting, G2/G3

### Phase 1: Foundation (COMPLETE)
Goal: Load an STL, drop a ball cutter onto it, emit G-code.

- [x] 1.1 Cargo workspace setup (core, cli crates)
- [x] 1.2 Type aliases (P2, P3, V2, V3) and basic geometry (BoundingBox3, Triangle)
- [x] 1.3 STL loading -> indexed TriangleMesh (via stl_io IndexedMesh)
- [x] 1.4 Spatial index over triangles (uniform grid, not KD-tree yet - works fine)
- [x] 1.5 MillingCutter trait definition
- [x] 1.6 FlatEndmill implementation (height/width/vertex_drop/facet_drop/edge_drop)
- [x] 1.7 BallEndmill implementation
- [x] 1.8 PointDropCutter algorithm (single point against mesh)
- [x] 1.9 BatchDropCutter with rayon (parallel grid)
- [x] 1.10 Toolpath IR types (Move, MoveType, Toolpath)
- [x] 1.11 Raster/parallel toolpath from drop-cutter grid (zigzag pattern)
- [x] 1.12 G-code emitter (G0/G1, GRBL + LinuxCNC post-processors)
- [x] 1.13 CLI skeleton: `rs_cam drop-cutter input.stl --tool ball:6.35 --stepover 1.0 -o output.nc`
- [x] 1.14 End-to-end test: STL -> drop-cutter -> G-code file (terrain_small.stl fixture + hemisphere)
- [x] 1.15 SVG 2D toolpath preview and interactive 3D HTML viewer

### Phase 2: 2.5D Operations (COMPLETE)
- [x] 2.1 Polygon2 type with geo-types conversion (polygon.rs)
- [x] 2.2 Polygon offsetting — cavalier_contours, arc-preserving (polygon.rs)
- [x] 2.3 Pocket clearing — contour-parallel offset pattern (pocket.rs)
- [x] 2.4 Profile cutting with tool radius compensation (profile.rs)
- [x] 2.5 Zigzag/raster clearing pattern with angle support (zigzag.rs)
- [x] 2.6 Depth stepping — even/constant distribution, finish allowance (depth.rs)
- [x] 2.7 SVG input — usvg, bezier flattening, containment detection (svg_input.rs)
- [x] 2.8 DXF input — LwPolyline, Circle, Ellipse, bulge arcs, containment (dxf_input.rs)
- [x] 2.9 Helix/ramp entry dressup — rapid to clearance then ramp/helix (dressup.rs)
- [x] 2.10 Tab/bridge dressup — segment-interpolated sharp tabs (dressup.rs)
- [x] 2.11 CLI: pocket/profile subcommands with all options + 3D viewer
- [x] Polygon containment detection — ray-casting point-in-polygon for SVG/DXF islands
- [x] Standalone 3D HTML viewer for 2.5D operations (viz.rs)

### Phase 3: Advanced Tools & 3D (COMPLETE)
- [x] 3.1 BullNoseEndmill — flat bottom + toroidal corner (R1/R2), full drop-cutter (bullnose.rs)
- [x] 3.2 VBitEndmill — conical profile, tip + surface contact, hyperbola edge contact (vbit.rs)
- [x] 3.3 TaperedBallEndmill — composite ball tip + cone taper, validated junction continuity (tapered_ball.rs)
- [x] 3.4 Push-cutter algorithm — vertex/facet/edge push, batch with rayon (pushcutter.rs)
- [x] 3.5 Fiber and Interval types — parameterized line segments, merged intervals (fiber.rs)
- [x] 3.6 Weave graph — SKIPPED (nearest-neighbor contour chaining used instead)
- [x] 3.7 Waterline algorithm — X/Y fiber grids, push-cutter, contour extraction (waterline.rs)
- [x] 3.8 Heightmap-based surface operations — DEFERRED to Phase 4
- [x] 3.9 Arc fitting dressup — greedy biarc fitting, circle-from-3-points (arcfit.rs)
- [x] 3.10 G2/G3 arc output — ArcCW/ArcCCW in toolpath IR, PostProcessor arc methods
- [x] 3.11 Mach3 post-processor + waterline CLI subcommand (gcode.rs, main.rs)
- [x] CLI: all 5 tool types parseable (ball, flat, bullnose, vbit, tapered_ball)

### Phase 4: High-Value Features
- [x] 4.1 Adaptive clearing (constant engagement) — grid-based engagement, direction smoothing, idle detection, CLI subcommand (adaptive.rs)
- [x] 4.1e Adaptive: boundary distance field + wall-tangent bias (BFS distance, gradient-based tangential scoring near walls)
- [x] 4.1f Adaptive: entry point spreading (avoid corner clustering by penalizing proximity to previous endpoints)
- [x] 4.1g Adaptive: slot clearing (Fusion-style center slot before adaptive spiral, reuses zigzag_lines)
- [x] 4.1h Adaptive: minimum cutting radius (blend sharp corners with arcs, configurable)
- [x] 4.1i Adaptive: increased angle continuity weight (0.05 → 0.12 for smoother curves)
- [x] 4.1j Adaptive: boundary cleanup pass (auto contour trace of all walls + island boundaries after adaptive passes)
- [x] 4.1a Adaptive: interpolation-based angle search — narrow 7-candidate search + bracket interpolation, falls back to broad sweep
- [x] 4.1b Adaptive: boundary walking for entry points — walks machinable polygon contours systematically, falls back to grid scan
- [x] 4.1c Adaptive: disk-area engagement — replaced 24-point circumference sampling with full disk-area cell counting for precise engagement
- [x] 4.1d Adaptive: link vs retract logic — keep tool down between nearby passes when path is clear (56% rapid reduction)
- [x] 4.2 V-carving — scan-line V-carve with exact Euclidean distance, variable Z, max depth clamp, CLI subcommand (vcarve.rs)
- [x] 4.3 Rest machining — geometric comparison (large vs small tool offset), masked zigzag scan lines, CLI + TOML integration (rest.rs)
- [x] 4.7 3D adaptive clearing — heightmap material tracking, precomputed surface Z, multi-level passes, waterline cleanup, CLI + TOML (adaptive3d.rs)
- [x] 4.7a 3D adaptive improvements — Z-rate clamping (no deep plunges), helix/ramp entry, improved idle detection, fine stepdown, flat area detection, configurable max_stay_down_dist (adaptive3d.rs)
- [x] 4.7b Region-based pocket ordering — BFS flood fill detects connected material regions, clears each fully before moving to next (--order-by by-area), reduces tool travel on scattered terrain (adaptive3d.rs)
- [x] 4.4 TOML job file parsing — multi-tool, multi-operation job files with per-op overrides (job.rs, demo_job.toml)
- [x] 4.5 Dogbone dressup — inside corner overcuts with configurable angle threshold (dressup.rs)
- [x] 4.6 Lead-in/lead-out dressup — quarter-circle arc entry/exit for clean profile cuts (dressup.rs)

### Phase 5: Visualization & Polish
- [x] 5.1 egui + wgpu 3D viewer — Phase 1+2: eframe app, dark Zed theme, 3-panel layout, custom WGSL shaders (mesh Phong + line + blit), offscreen depth buffer rendering, orbit/pan/zoom camera, STL import via rfd, mesh upload to GPU with flat shading, ground grid + axis indicators, stock wireframe box, project tree, properties panel, status bar, view presets (rs_cam_viz crate)
- [x] 5.1a GUI Phase 3: State management — tool library (5 tool types with type-specific params, add/duplicate/delete, context menu), editable stock config (DragValue widgets, auto-from-model), post-processor config (GRBL/LinuxCNC/Mach3, spindle speed, safe Z), tool cross-section 2D preview (egui Painter), selection-driven properties panel, inline parameter editing with stock wireframe GPU re-upload
- [x] 5.1b GUI Phase 4: First toolpath (Pocket) — OperationConfig enum, PocketConfig with contour/zigzag pattern, background compute worker (mpsc channels), depth-stepped pocket_toolpath/zigzag_toolpath integration, toolpath line rendering with Z-depth coloring (blue→cyan), SVG import for 2D geometry, toolpath properties panel (tool/model selector, all pocket params), generate button with status (pending/computing/done/error), toolpath stats (moves/cutting/rapid distance), project tree with toolpath list + visibility toggle
- [x] 5.2 Material removal simulation — heightmap stamping, wood-tone mesh, animated 3D replay with tool model (simulation.rs, viz.rs)
- [x] 5.2a Simulation performance — swept segment stamping (10x), radial LUT (no sqrt), early-out skip, benchmarks (simulation.rs, adaptive3d.rs)
- [x] 5.3 Inlay operations — female V-carve pocket, flat area clearing, male plug with glue gap, CLI subcommand (inlay.rs)
- [x] 5.5 Pencil finishing — mesh edge dihedral angle analysis, concave edge chaining, offset passes, drop-cutter Z lift, CLI subcommand (pencil.rs)
- [x] 5.6 WASM readiness — feature-gated rayon (`parallel` feature, default on), `std::time::Instant` gated for wasm32, `from_stl_bytes()` API for in-memory loading
- [x] 5.7 Collision detection MVP — tool holder/shank collision check via drop-cutter at larger radii, penetration depth + min safe stickout (collision.rs)
- [x] 5.7a Collision detection expanded — interpolated path checking (catch mid-move collisions), multi-segment holders (tapered collet nuts), CLI integration (--holder-diameter etc.), TOML holder fields
- [x] 5.8 Pipeline cache foundation — dirty-flag invalidation cache for mesh + surface heightmap across multi-operation jobs (pipeline.rs)
- [x] 5.9 Feed rate optimization — RCTF chip thinning compensation, heightmap engagement estimation, forward/backward smoothing, post-process dressup for any operation (feedopt.rs)
- [x] 5.4 Tech debt cleanup — removed all unwrap() from library code (partial_cmp NaN safety, last() safety), wired per-operation spindle speed, spatial index auto-sizing + cell clamp, CLPoint.contacted flag for boundary detection, fixed duplicate rapid in raster toolpath, emit_gcode_phased for multi-operation jobs
- [x] 5.10 Shared toolpath utilities — point_to_segment_distance in geo.rs, emit_path_segment/final_retract/simplify_path_3d in toolpath.rs, eliminated duplication across 9 files
- [x] 5.11 Link-vs-retract dressup — general post-processing dressup that replaces short retract→rapid→plunge with direct feeds (dressup.rs), CLI --link-moves flag
- [x] 5.12 Flipped normal detection — check_winding/fix_winding on STL load, auto-fix if >5% inconsistent edges (mesh.rs)
- [x] 5.13 Module rename — weave.rs → contour_extract.rs (reflects actual algorithm: marching squares, not Weave graph)
- [x] 5.14 Adaptive speed optimizations — coarse 360° scan + bracket refinement replaces 55-eval brute-force sweep (~21 evals), growing-radius entry point search replaces O(rows×cols) grid scans (adaptive.rs, adaptive3d.rs)
- [x] 5.15 Pocket island support — contour-parallel pocket emits hole contours alongside exterior rings, islands no longer cut through (pocket.rs)
- [x] 5.16 Push-cutter edge accuracy — coarse+bisection sampling (9+10≈19 evals) replaces 32-step uniform, higher boundary accuracy (pushcutter.rs)
- [x] 5.17 Polygon offset hole pairing — containment-based re-pairing via point-in-polygon instead of blind attachment to first polygon (polygon.rs)
- [x] 5.18 Arc fitting least-squares — Kåsa's algebraic circle fit replaces 3-point fit for 5+ points, lower mean error on noisy/partial arcs (arcfit.rs)

## Module Map (for new agents)

| Module | File | Purpose |
|--------|------|---------|
| geo | `rs_cam_core/src/geo.rs` | P2/P3/V2/V3 aliases, BoundingBox3, Triangle, point_to_segment_distance |
| mesh | `rs_cam_core/src/mesh.rs` | TriangleMesh (STL loading, winding check/fix), SpatialIndex (uniform grid) |
| tool | `rs_cam_core/src/tool/` | MillingCutter trait, FlatEndmill, BallEndmill, BullNoseEndmill, VBitEndmill, TaperedBallEndmill |
| dropcutter | `rs_cam_core/src/dropcutter.rs` | point_drop_cutter, batch_drop_cutter (rayon parallel) |
| toolpath | `rs_cam_core/src/toolpath.rs` | Move, MoveType, Toolpath IR, emit_path_segment, final_retract, simplify_path_3d |
| gcode | `rs_cam_core/src/gcode.rs` | PostProcessor trait, GrblPost, LinuxCncPost, Mach3Post, emit_gcode |
| viz | `rs_cam_core/src/viz.rs` | SVG preview, 3D HTML viewer (mesh+toolpath, standalone, simulation w/ animation) |
| simulation | `rs_cam_core/src/simulation.rs` | Heightmap, tool stamping, arc linearization, heightmap-to-mesh export |
| polygon | `rs_cam_core/src/polygon.rs` | Polygon2, offset, pocket_offsets, containment detection |
| pocket | `rs_cam_core/src/pocket.rs` | PocketParams, pocket_toolpath, pocket_contours |
| profile | `rs_cam_core/src/profile.rs` | ProfileParams, ProfileSide, profile_toolpath |
| zigzag | `rs_cam_core/src/zigzag.rs` | ZigzagParams, zigzag_toolpath (scan-line with angle) |
| depth | `rs_cam_core/src/depth.rs` | DepthStepping, depth_stepped_toolpath, finish allowance |
| svg_input | `rs_cam_core/src/svg_input.rs` | load_svg (usvg, bezier flattening, containment) |
| dxf_input | `rs_cam_core/src/dxf_input.rs` | load_dxf (LwPolyline, Circle, Ellipse, bulge arcs) |
| dressup | `rs_cam_core/src/dressup.rs` | Ramp/helix entry, tab/bridge, dogbone, lead-in/out, link-vs-retract |
| fiber | `rs_cam_core/src/fiber.rs` | Fiber (parameterized line at Z), Interval with merging |
| pushcutter | `rs_cam_core/src/pushcutter.rs` | push_cutter_triangle, batch_push_cutter (rayon) |
| waterline | `rs_cam_core/src/waterline.rs` | waterline_contours, waterline_toolpath (multi-Z) |
| arcfit | `rs_cam_core/src/arcfit.rs` | fit_arcs (biarc fitting, linear → G2/G3) |
| adaptive | `rs_cam_core/src/adaptive.rs` | Adaptive clearing: MaterialGrid, engagement, direction search, path generation |
| vcarve | `rs_cam_core/src/vcarve.rs` | V-carving: distance-to-boundary, variable-depth scan-line toolpath |
| rest | `rs_cam_core/src/rest.rs` | Rest machining: geometric comparison, masked zigzag in unreachable corners |
| adaptive3d | `rs_cam_core/src/adaptive3d.rs` | 3D adaptive clearing: heightmap material tracking, surface-following engagement, multi-level passes |
| pencil | `rs_cam_core/src/pencil.rs` | Pencil finishing: concave mesh edge detection, dihedral angle analysis, polyline chaining |
| inlay | `rs_cam_core/src/inlay.rs` | Inlay operations: female V-carve pocket, male plug with inverted depth profile |
| collision | `rs_cam_core/src/collision.rs` | Tool holder/shank collision detection, interpolated path checking, multi-segment holders |
| pipeline | `rs_cam_core/src/pipeline.rs` | Incremental computation cache with dirty-flag invalidation |
| contour_extract | `rs_cam_core/src/contour_extract.rs` | Marching squares contour extraction for waterline (replaces nearest-neighbor) |
| CLI | `rs_cam_cli/src/main.rs` | drop-cutter, pocket, profile, adaptive, adaptive3d, vcarve, rest, waterline, pencil, inlay subcommands |

## Decisions Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-03-19 | nalgebra over glam for primary math | Native parry3d interop, richer type system |
| 2026-03-19 | Toolpath IR not raw G-code | Enables dressups, viz, analysis without G-code parsing |
| 2026-03-19 | "Toolpath" in UX, "Operation" in code | Target audience is hobbyist wood CNC operators |
| 2026-03-19 | F3D output not feasible | Proprietary format, no public spec |
| 2026-03-19 | cavalier_contours for polygon offset | Arc-preserving (G2/G3 compatible), Shape API handles holes, purpose-built for CAM |
| 2026-03-19 | geo crate for type conversions, not offset | geo::Buffer works but approximates arcs as line segments |
| 2026-03-19 | KD-tree for drop-cutter, BVH for ray queries | Follows OpenCAMLib's proven approach |
| 2026-03-19 | Zigzag for pockets with islands | Contour-parallel rings don't avoid holes; zigzag scan-lines clip correctly against hole edges |
| 2026-03-19 | Nearest-neighbor over Weave for waterline | Simpler implementation, sufficient for initial waterline support; Weave graph is complex and can be added later for adaptive refinement |
| 2026-03-20 | Weave graph evaluated, marching squares chosen | Topologically correct, precision within fiber sampling resolution, full OCL Weave deferred (20-40h) |
| 2026-03-20 | Link-vs-retract as general dressup, not per-operation | Avoids code duplication across 14 files; operations produce standard retract patterns, dressup optimizes post-hoc |

## Known Issues / Tech Debt
- ~~Spatial index degrades when cell_size >> model extent~~ FIXED: `build()` auto-clamps oversized cells; `build_auto()` added.
- ~~Points outside mesh boundary hit min_z clamp~~ FIXED: CLPoint now has `contacted` flag to detect no-contact points.
- ~~Flipped normals on some triangles could cause facet_drop to miss contacts~~ FIXED: check_winding/fix_winding on STL load, auto-fix if >5% inconsistent.
- ~~Duplicate rapid at start of each row in raster toolpath~~ FIXED: removed duplicate initial rapid.
- ~~Waterline contour chaining uses nearest-neighbor which can produce artifacts~~ FIXED: marching squares contour extraction (contour_extract.rs).
- ~~Contour-parallel pocket pattern does NOT avoid islands~~ FIXED: hole contours emitted alongside exterior contours in pocket_contours().
- Bull nose edge_drop uses simplified tube-circle approach (not full offset-ellipse with Brent's solver). Accurate for most cases but may have slight errors on highly sloped edges.
- ~~Push-cutter edge_push uses sampling (32 steps) rather than analytical solution~~ FIXED: coarse+bisection (9 coarse + ~10 bisection = ~19 evals) with higher boundary accuracy.
- ~~Polygon offset blindly assigns all holes to first polygon~~ FIXED: containment-based hole pairing via point-in-polygon test.

## Test Fixtures
- fixtures/terrain_small.stl: 40K triangle terrain mesh (100x73mm, from rivmap project)
- fixtures/demo_pocket.svg: Rounded rect with circle island (tests containment + pocket)
- fixtures/demo_star.svg: 5-pointed star (tests profile + tabs)
- Programmatic: make_test_hemisphere(), make_test_flat() in mesh.rs

## Performance Benchmarks
- 196K triangles, 108K grid points, 0.18s release build (terrain.stl, ball:3.175, stepover 1.0)
- 40K triangles, 2.2K grid points, 3.1s debug / ~0.1s release (terrain_small.stl, ball:6.35, stepover 2.0)
- Criterion benchmark suite: 22 benchmarks across 9 groups (crates/rs_cam_core/benches/perf_suite.rs)
- Performance optimization pass complete (2026-03-20): batch_drop_cutter 84-86% faster, spatial queries 85-98% faster, waterline 51-96% faster. See Performance_review.md for full results.
- Simulation optimization (2026-03-20): swept segment stamping 10.4x faster (367µs→35µs per 50mm segment), RadialProfileLUT eliminates sqrt per cell, early-out skip for unchanged stamps

## Key References for Phase 4
- `research/02_algorithms.md` — Adaptive clearing (constant engagement), V-carving algorithms
- `research/08_ux_terminology.md` — User-facing parameter names for TOML job files
- `rs_cam_core/src/polygon.rs` — Polygon2 offset infrastructure for adaptive engagement
- `rs_cam_core/src/tool/vbit.rs` — V-bit geometry needed for V-carving
- `rs_cam_core/src/depth.rs` — depth_stepped_toolpath pattern for composing operations

## Adaptive Clearing Refinements (from FreeCAD/Freesteel research)

Current implementation (4.1) uses grid-based engagement sampling and brute-force angle sweep.
Items 4.1a-d are implemented at a basic level (checked above). The Freesteel-level versions
documented below are aspirational targets, not blockers — they describe the full algorithms
from libactp that would further improve quality and performance.

### 4.1a — Interpolation-Based Angle Search
**What**: Replace brute-force sweep (19+36 candidates) with history-predicted interpolation.
**How (from Freesteel)**: Maintain an `angleHistory` of the last 3 angles. Predict the next
deflection angle from the trend. Use the `Interpolation` class to do bracketed linear
interpolation on `(angle → area_error)`, converging on the target cut area in 2–4 iterations
instead of 19+36 brute-force candidates. Essentially bisection search with linear interpolation
hints that converges fast.
**Impact**: Faster (fewer engagement evaluations per step) AND smoother (continuous angle
function rather than discrete candidates). Medium effort.

### 4.1b — Boundary Walking for Entry Points
**What**: Replace nearest-material grid scan with systematic boundary traversal.
**How (from Freesteel)**: The `EngagePoint` class walks along the tool boundary paths (the
offset contour where the tool center can legally be). It maintains a position via path index,
segment index, and parameter. `moveForward()` advances along the boundary by a step distance.
`nextEngagePoint()` iterates forward, calling `CalcCutArea()` at each candidate position to
test if there's enough uncleared material to engage. Threshold is `ENGAGE_AREA_THR_FACTOR *
optimalCutAreaPD` (~30% of optimal cut area). When all boundary points are exhausted, it checks
for remaining internal uncleared paths and processes those as sub-regions. `ResetPasses()` is
called after a successful pass to allow re-scanning.
**Impact**: More systematic entry point discovery, no missed regions. Medium effort.

### 4.1c — Exact Sweep-Line Area Calculation
**What**: Replace grid sampling engagement with exact geometric area calculation.
**How (from FreeCAD `CalcCutArea()` at line 1427)**: Two circles represent the tool at old
position (c1) and new position (c2), both with `toolRadiusScaled` radius. Cleared polygons are
fetched from a cached bounding-box lookup (`GetBoundedClearedAreaClipped`). They compute all
x-coordinates of interest: polygon vertices, line-circle intersections (tool circles vs polygon
edges), circle-circle intersections (c1 vs c2), and tangent points. For each x-interval between
sorted coordinates, cast a vertical line through the midpoint, find all y-intersections with
polygons and circles, sort by y, compute exact area — using circular segments (sector minus
triangle) for circle crossings and trapezoids for polygon crossings. Result is the exact boolean
area of `(c2 minus c1 minus cleared_polygons)` — the new material the tool will remove.
`outsideCount` is a sweep-line winding number tracking inside/outside the cut region. Much more
precise than grid sampling and the key to consistent engagement.
**Impact**: Precise engagement → consistent chip load → better surface finish. High effort
(polygon boolean ops, circle-polygon intersections, sweep-line area). Could use ClipperLib
(clipper2-rust already in deps) for polygon operations.

### 4.1d — Link vs Retract Logic
**What**: Keep tool down between passes when safe, instead of always retracting.
**How (from Freesteel)**: `keepToolDownDistRatio` defaults to 3.0 — keep the tool down if the
safe-travel path length is less than 3× the direct straight-line distance between points.
`ResolveLinkPath()` tries to find a clear path between two points by walking along previously-
cleared contours, using `IsClearPath()` to verify safety. Has a time limit
(`keepToolDownDistRatio * CLOCKS_PER_SEC / 6`) to prevent spending too long searching. If the
path length exceeds `keepToolDownDistRatio * directDistance`, give up and retract. Path-finding
works by offsetting the cleared area boundary and testing progressively larger offsets to find
a walkable corridor.
**Impact**: Fewer rapids and retracts → faster cycle time, less tool wear from plunging.
Medium effort. Requires tracking cleared polygon contours (not just grid cells).

## 3D Adaptive Clearing Deep Dive

True 3D adaptive clearing: maintain constant engagement while following a mesh surface.
Unlike 2.5D adaptive (which clears flat polygon regions at discrete Z levels), 3D adaptive
follows the actual STL surface — the tool Z changes continuously based on drop-cutter queries.

### Why It Matters
- 2.5D adaptive works for pockets/profiles but can't rough a sculpted surface efficiently
- Current 3D options (drop-cutter raster, waterline) don't control engagement — they use
  fixed stepover which means variable chip load on slopes
- 3D adaptive = the marquee feature of high-end CAM (Fusion 360 "3D Adaptive", Mastercam
  "OptiRough", HSMWorks)

### Architecture: Heightmap-Based Material Tracking

Replace the 2D `MaterialGrid` (boolean cells) with a `MaterialHeightmap` (f64 heights):

| Component | 2.5D (current) | 3D (new) |
|-----------|----------------|----------|
| Material state | `MaterialGrid` (0/1/2 cells) | `MaterialHeightmap` (f64 heights per cell) |
| "Is material here?" | `cell == CELL_MATERIAL` | `heightmap[cell] > mesh_z_at(x,y) + stock_to_leave` |
| Clear material | `clear_circle(cx, cy, radius)` | `stamp_tool(cx, cy, z, cutter)` (lower heights) |
| Engagement | Count samples on tool circle hitting material cells | Count samples on tool circle where `hm[x,y] > surface_z + threshold` |
| Z at position | Constant `cut_depth` | `point_drop_cutter(mesh, cutter, x, y)` |

The `MaterialHeightmap` starts at stock top (Z=0). As the tool cuts, heights are lowered
by stamping the cutter profile — exactly like `simulation.rs` already does.

### Step-by-Step Implementation

**Step 1: MaterialHeightmap** — New struct wrapping `simulation::Heightmap` with engagement
query methods. Initialize from stock bounding box. Provides `is_material_at(x, y, z)` and
`engagement_3d(cx, cy, cz, radius)`.

**Step 2: 3D direction search** — Same angular sweep as 2D, but each candidate position
gets its Z from `point_drop_cutter()`. Engagement is computed in 3D: sample points on the
tool circle at the surface-following Z, check which hit material above the surface.

**Step 3: 3D entry point finding** — Scan the heightmap for cells where material remains
above the mesh surface. Entry Z comes from drop-cutter at that XY.

**Step 4: adaptive_3d_segments()** — Main loop, same structure as 2D:
```
while material_remaining > threshold:
    entry = find_3d_entry(heightmap, mesh, cutter)
    while can_continue:
        z = point_drop_cutter(mesh, cutter, cx, cy)
        angle = search_direction_3d(heightmap, mesh, cutter, cx, cy, z, ...)
        move to (cx + step*cos(angle), cy + step*sin(angle), z_new)
        stamp_tool(heightmap, cx, cy, z, cutter)
```

**Step 5: Multi-level roughing** — For deep stock (stock_top >> mesh), do multiple Z
levels. At each level, limit the drop-cutter Z to `max(mesh_z, current_level_z)`. This
prevents the tool from plunging to full depth on steep walls. Freesteel calls this
"waterline-bounded adaptive" — combine waterline Z levels with adaptive XY motion.

**Step 6: 3D boundary cleanup** — After adaptive passes, run waterline contours at each
Z level to clean walls, analogous to the 2D boundary cleanup pass.

### Key Reuse from Existing Code

| Existing Module | Reuse For |
|----------------|-----------|
| `simulation.rs` `Heightmap` | Material tracking (stamp_tool already works) |
| `dropcutter.rs` `point_drop_cutter` | Z-height queries at each step |
| `adaptive.rs` `search_direction` | Angular sweep logic (add Z param) |
| `adaptive.rs` `blend_corners` | Path post-processing |
| `adaptive.rs` `simplify_path` | Path simplification |
| `waterline.rs` | Boundary cleanup at each Z level |
| `tool/*.rs` all cutter types | Drop-cutter + stamping already implemented |

### Performance Considerations

- Drop-cutter query per step is the bottleneck (~1µs per query with spatial index)
- At step_len ≈ 0.5mm over a 100×100mm part: ~40K steps per pass, ~200K total queries
- With spatial index: ~0.2s for the drop-cutter queries alone
- Heightmap stamping is O(cells_in_tool_radius) per step, already fast
- Total: should be under 1s for a typical part in release build

### Effort Estimate
- MaterialHeightmap + engagement: 1 session
- 3D direction search + entry points: 1 session
- Main loop + multi-level: 1 session
- CLI integration + testing + boundary cleanup: 1 session
- Total: ~4 focused sessions, high complexity
