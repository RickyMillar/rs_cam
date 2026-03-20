# rs_cam Feature Implementation Plan

Covers all gaps from FEATURE_GAP_REPORT.md **except** feeds & speeds calculator and tool library features (handled by separate agent).

## Architecture Pattern for New Features

Every new operation follows this checklist:
1. **Core**: `rs_cam_core/src/{module}.rs` — params struct + algorithm function → `Toolpath`
2. **State**: `rs_cam_viz/src/state/toolpath.rs` — `OperationType` variant, `OperationConfig` variant, config struct
3. **Worker**: `rs_cam_viz/src/compute/worker.rs` — `run_{op}()` function + dispatch case
4. **UI**: `rs_cam_viz/src/ui/properties/mod.rs` — `draw_{op}_params()` function + dispatch case
5. **Tooltips**: Add parameter descriptions to `tooltip_for()` in properties/mod.rs
6. **Tests**: `#[cfg(test)] mod tests` in the core module

---

## Phase 1: Missing Operations (4 new ops, ~6 sessions)

### 1.1 Face / Surfacing Operation
**Sessions: 1** | Priority: Critical — first op in every job

The simplest new operation. A zigzag at Z=0 across the stock boundary with large stepover.

**Core** (`rs_cam_core/src/face.rs`):
```
FaceParams {
    stepover: f64,        // default: 80% tool diameter
    depth: f64,           // default: 0 (single pass at stock top)
    depth_per_pass: f64,  // for thick face removal
    feed_rate: f64,
    plunge_rate: f64,
    direction: FaceDirection,  // OneWay, Zigzag
    stock_offset: f64,    // extra distance beyond stock boundary
}

fn face_toolpath(stock_bounds: &BoundingBox3, params: &FaceParams) -> Toolpath
```

Algorithm:
1. Create a rectangle polygon from stock XY bounds + `stock_offset`
2. Call `zigzag_toolpath()` with that rectangle at each depth level
3. Stepover defaults to `0.8 * tool_diameter` (user-adjustable)

**State**: Add `Face` to `OperationType`, `OperationConfig::Face(FaceConfig)`, `FaceConfig` struct.

**Worker**: `run_face()` uses stock bbox from `req` (no mesh/polygon input required — this is unique among ops). Need to pass stock bounds through `ComputeRequest`.

**UI**: Simple grid with stepover, depth, direction, feed_rate, plunge_rate.

**Files touched**: `face.rs` (new), `lib.rs`, `toolpath.rs`, `worker.rs`, `properties/mod.rs`, `project_tree.rs`

---

### 1.2 Trace / Follow Path Operation
**Sessions: 1** | Priority: High — common for decorative work

Follows SVG/DXF geometry exactly at a specified depth. No tool radius offset.

**Core** (`rs_cam_core/src/trace.rs`):
```
TraceParams {
    depth: f64,
    depth_per_pass: f64,
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
    compensation: TraceCompensation,  // None, Left, Right
}

fn trace_toolpath(polygon: &Polygon2, params: &TraceParams) -> Toolpath
```

Algorithm:
1. For each polygon exterior + holes:
   - Rapid to safe_z above first point
   - Plunge to cut depth
   - Feed along all points in sequence
   - Close the polygon (return to first point)
   - Retract to safe_z
2. With compensation: offset by ±tool_radius before tracing
3. Depth stepping via `depth_stepped_toolpath()` as usual

**This is the simplest possible 2D operation** — literally polygon vertices → Move::Linear.

**Files touched**: `trace.rs` (new), `lib.rs`, `toolpath.rs`, `worker.rs`, `properties/mod.rs`

---

### 1.3 Drill Operation
**Sessions: 2** | Priority: High — commonly needed, zero drilling support today

Plunge drilling with canned cycle support (G81/G82/G83).

**Core** (`rs_cam_core/src/drill.rs`):
```
DrillCycle { Simple, Dwell(f64), Peck(f64), ChipBreak(f64) }

DrillParams {
    holes: Vec<P2>,         // XY positions
    depth: f64,             // total drill depth (positive = below stock top)
    cycle: DrillCycle,
    feed_rate: f64,         // plunge feed
    spindle_speed: u32,
    safe_z: f64,
    retract_z: f64,         // R-plane (rapid to here, then feed)
}

fn drill_toolpath(params: &DrillParams) -> Toolpath
```

Algorithm:
- For `Simple`: Rapid to XY → rapid to R-plane → feed to depth → rapid out
- For `Peck`: Repeat (feed peck_depth, retract to R, rapid back to last Z + clearance) until depth reached
- For `Dwell`: Same as simple but pause at bottom (add dwell to toolpath IR? or emit as comment)
- For `ChipBreak`: Partial retract (0.5mm) between pecks

**G-code extension**: Add `MoveType::Dwell { seconds: f64 }` to toolpath IR, and `fn dwell(&self, seconds: f64) -> String` to `PostProcessor` trait. Emit G82 for dwell cycles in post-processors.

**UI**: Hole positions from SVG/DXF point geometry, or manual XY entry. Cycle type dropdown, peck depth, dwell time.

**Note**: Hole auto-detection from SVG circles is a nice addition — `load_svg()` already returns circle centers.

**Files touched**: `drill.rs` (new), `lib.rs`, `toolpath.rs` (add Dwell), `gcode.rs` (add dwell to PostProcessor), `toolpath.rs` (state), `worker.rs`, `properties/mod.rs`

---

### 1.4 Chamfer Operation
**Sessions: 2** | Priority: Medium — edge finishing

V-bit edge break along profile edges at controlled width/depth.

**Core** (`rs_cam_core/src/chamfer.rs`):
```
ChamferParams {
    chamfer_width: f64,     // width of chamfer on the face
    tip_offset: f64,        // distance from tip to prevent wear (default: 0.1mm)
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
}

fn chamfer_toolpath(polygon: &Polygon2, tool_half_angle: f64, params: &ChamferParams) -> Toolpath
```

Algorithm:
1. Depth = `(chamfer_width + tip_offset) / tan(tool_half_angle)`
2. Offset = tool center offset from edge: `chamfer_width / 2`
3. Profile cut at computed Z depth with computed XY offset
4. Essentially a profile toolpath at a specific Z computed from chamfer geometry

Reuses `profile_toolpath()` internally with Z and offset derived from chamfer params.

**Files touched**: `chamfer.rs` (new), `lib.rs`, `toolpath.rs`, `worker.rs`, `properties/mod.rs`

---

## Phase 2: Heights System & Retraction (3 sessions)

### 2.1 Multi-Level Heights
**Sessions: 2** | Priority: High — professional users expect this

Replace single `safe_z` with 5-level height system.

**Core changes** (`rs_cam_core/src/toolpath.rs` or new `heights.rs`):
```
struct Heights {
    clearance_z: f64,   // highest: safe travel between ops (default: safe_z + 10)
    retract_z: f64,     // between passes within one op (default: safe_z)
    feed_z: f64,        // switch from rapid to feed on approach (default: safe_z - 2)
    top_z: f64,         // top of cut (default: 0.0 = stock top)
    bottom_z: f64,      // final depth (default: -depth)
}
```

**Behavior changes**:
- Rapid moves between operations use `clearance_z`
- Rapid moves between passes within one op use `retract_z`
- Approach from retract to top uses feed rate below `feed_z`
- `top_z` replaces hardcoded `0.0` start
- `bottom_z` replaces the per-operation depth calculation

**State** (`toolpath.rs`): Add `Heights` to every operation config, or add a shared `HeightsConfig` on `ToolpathEntry`.

**Worker**: Pass `Heights` through to operations. Operations use `heights.retract_z` instead of `safe_z`, etc.

**UI**: Collapsible "Heights" section in toolpath properties. Each height is a DragValue with a reference dropdown (Stock Top, Model Top, Model Bottom, Absolute).

**Viewport visualization**: Draw colored horizontal planes at each height level when editing heights (like Fusion's height planes). Use the line pipeline — 5 transparent quads.

**Files touched**: `toolpath.rs` (state), `worker.rs` (all run_* functions), `properties/mod.rs`, `app.rs` (viewport planes)

### 2.2 Retraction Strategy
**Sessions: 1** | Priority: Medium

```
enum RetractStrategy {
    Full,       // always retract to retract_z (current behavior, safest)
    Minimum,    // retract just above stock surface + safe_distance
    Direct,     // straight line between cuts (fastest, needs clearance check)
}
```

**Implementation**: This is best done as a post-processing dressup (like link-vs-retract). After toolpath generation:
1. `Full`: no change (current behavior)
2. `Minimum`: for each retract-rapid-plunge sequence, lower the retract Z to `max_z_on_path + safe_distance`
3. `Direct`: replace retract-rapid-plunge with a direct feed move (only if the line doesn't intersect stock — approximate with heightmap check)

**Files touched**: `dressup.rs` (add `apply_retract_strategy()`), `toolpath.rs` (state), `worker.rs` (apply in dressup chain), `properties/mod.rs`

---

## Phase 3: Boundary & Containment (3 sessions)

### 3.1 Machining Boundary
**Sessions: 2** | Priority: Medium-High

Restrict toolpath to a user-defined region.

**Core** (`rs_cam_core/src/boundary.rs`):
```
enum ToolContainment {
    Center,   // tool center stays inside boundary
    Inside,   // entire tool stays inside (boundary - tool_radius)
    Outside,  // tool edge can go outside (boundary + tool_radius)
}

struct MachiningBoundary {
    polygon: Polygon2,
    containment: ToolContainment,
}

fn effective_boundary(boundary: &MachiningBoundary, tool_radius: f64) -> Polygon2
fn clip_toolpath_to_boundary(tp: &Toolpath, boundary: &Polygon2, safe_z: f64) -> Toolpath
```

**Algorithm for `clip_toolpath_to_boundary`**:
1. For each move segment, check if start/end are inside boundary polygon (point-in-polygon test)
2. If both inside → keep
3. If both outside → convert to rapid (skip)
4. If crossing → find intersection point, split segment, insert retract/plunge at boundary crossing

**State**: Add `machining_boundary: Option<MachiningBoundary>` to `ToolpathEntry`. Boundary polygon comes from model selection (a polygon from SVG/DXF, or stock bbox, or manual rectangle).

**UI**: Boundary dropdown: None, Stock Boundary, Model Silhouette, Custom Rectangle. Containment dropdown. Additional offset DragValue.

**Files touched**: `boundary.rs` (new), `lib.rs`, `toolpath.rs` (state), `worker.rs` (apply boundary clip after dressups), `properties/mod.rs`

### 3.2 Slope Confinement (Generalized)
**Sessions: 1** | Priority: Medium

Already on Scallop and RampFinish. Generalize to all 3D ops.

**Implementation**: Add `slope_from: f64, slope_to: f64` to all 3D config structs. In worker, after generating toolpath, filter moves where the mesh surface normal at the tool contact point falls outside the slope range.

Needs: `fn surface_normal_at(mesh, index, x, y) -> Option<V3>` helper — compute from nearest triangle normal via spatial index lookup.

**Files touched**: All 3D config structs in `toolpath.rs`, `worker.rs` (add slope filtering), `properties/mod.rs` (add slope fields to 3D ops)

---

## Phase 4: Finishing Passes & Compensation (2 sessions)

### 4.1 Finishing / Spring Passes
**Sessions: 1** | Priority: High — dimensional accuracy

Repeat the final depth pass N times for deflection recovery.

**Implementation**: Add `finishing_passes: usize` (default: 0) to Profile, Contour, Pocket configs.

In worker, after generating the toolpath for the final depth level:
1. Extract the last pass (moves between final plunge and retract)
2. Repeat those moves N times
3. Insert retract-rapid-plunge between repeats (or keep tool down)

Alternative (simpler): in `depth_stepped_toolpath()`, repeat the final Z level N+1 times.

**Files touched**: `toolpath.rs` (state), `depth.rs` (add finishing_passes to DepthStepping), `worker.rs`, `properties/mod.rs`

### 4.2 Compensation Type
**Sessions: 1** | Priority: Low-Medium

Add In-Control (G41/G42) compensation output.

```
enum CompensationType {
    InComputer,  // default — tool offset computed in CAM (current behavior)
    InControl,   // emit G41/G42 for CNC controller to apply offset
}
```

**PostProcessor changes**: Add `fn tool_comp_left/right(&self, tool_num: u32) -> String` emitting G41/G42 D{tool}, and `fn tool_comp_cancel() -> String` emitting G40.

**In-Control mode**: Don't apply tool radius offset in CAM. Instead, emit G41/G42 before the profile pass and G40 after. Only applicable to Profile/Contour.

**Files touched**: `gcode.rs` (add G41/G42/G40 to PostProcessor), `toolpath.rs` (state), `worker.rs` (skip offset when InControl), `properties/mod.rs`

---

## Phase 5: 3D Finishing Strategies (6-8 sessions)

### 5.1 Spiral Finishing
**Sessions: 3** | Priority: Medium — for domes/bowls

Continuous Archimedean spiral from center outward (or reverse), Z from drop-cutter.

**Core** (`rs_cam_core/src/spiral.rs`):
```
SpiralParams {
    stepover: f64,
    direction: SpiralDirection,  // InsideOut, OutsideIn
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
    stock_to_leave: f64,
}

fn spiral_toolpath(mesh: &TriangleMesh, index: &SpatialIndex, cutter: &dyn MillingCutter, params: &SpiralParams) -> Toolpath
```

Algorithm:
1. Find model center XY from bounding box
2. Compute max radius from center to model corners
3. Generate Archimedean spiral points: `r = stepover * θ / (2π)`, stepping θ at angular resolution
4. For each (x, y) on spiral: `z = point_drop_cutter(mesh, index, cutter, x, y).z - stock_to_leave`
5. Feed along the continuous spiral with Z from drop-cutter

**Files touched**: `spiral.rs` (new), `lib.rs`, `toolpath.rs`, `worker.rs`, `properties/mod.rs`

### 5.2 Radial Finishing
**Sessions: 2** | Priority: Low-Medium

Spoke-like passes radiating from center.

**Core** (`rs_cam_core/src/radial.rs`):
```
RadialParams {
    angular_step: f64,     // degrees between spokes (default: 5.0)
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
    stock_to_leave: f64,
}

fn radial_toolpath(mesh: &TriangleMesh, index: &SpatialIndex, cutter: &dyn MillingCutter, params: &RadialParams) -> Toolpath
```

Algorithm:
1. Find center and max radius from bounding box
2. For each angle (0, step, 2*step, ...):
   - Generate line of points from center to perimeter
   - Drop-cutter each point for Z
   - Alternate direction (in→out, out→in) for zigzag linking
3. Rapid between spokes

**Files touched**: `radial.rs` (new), `lib.rs`, `toolpath.rs`, `worker.rs`, `properties/mod.rs`

### 5.3 Horizontal / Flat Area Finishing
**Sessions: 3** | Priority: Medium

Detect and finish only flat (horizontal) areas — cleanup after waterline/contour.

**Core** (`rs_cam_core/src/horizontal.rs`):
```
HorizontalParams {
    angle_threshold: f64,  // max slope to consider "flat" (default: 5 degrees)
    stepover: f64,
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
    stock_to_leave: f64,
}

fn horizontal_toolpath(mesh: &TriangleMesh, index: &SpatialIndex, cutter: &dyn MillingCutter, params: &HorizontalParams) -> Toolpath
```

Algorithm:
1. Classify all mesh triangles by face normal: flat = `normal.z > cos(threshold_angle)`
2. Group adjacent flat triangles into connected regions (flood-fill on mesh adjacency)
3. For each flat region:
   - Compute bounding polygon (convex hull or alpha shape of flat triangle vertices)
   - Generate zigzag raster within that polygon at the flat Z height
   - Z from drop-cutter for precision

**Files touched**: `horizontal.rs` (new), `lib.rs`, `toolpath.rs`, `worker.rs`, `properties/mod.rs`

### 5.4 Project / Curve-on-Surface
**Sessions: 3-4** | Priority: Low-Medium — 3D engraving

Project 2D curves onto 3D mesh, follow them as toolpath.

**Core** (`rs_cam_core/src/project.rs`):
```
ProjectParams {
    depth: f64,           // cut depth below surface
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
}

fn project_toolpath(polygon: &Polygon2, mesh: &TriangleMesh, index: &SpatialIndex, cutter: &dyn MillingCutter, params: &ProjectParams) -> Toolpath
```

Algorithm:
1. For each point on the 2D polygon path:
   - Ray-cast downward (or use `point_drop_cutter`) to find Z on mesh surface
   - Offset Z by `-depth` below surface
2. Chain the projected 3D points into toolpath moves
3. Simplify with `simplify_path_3d()` for tolerance

**Files touched**: `project.rs` (new), `lib.rs`, `toolpath.rs`, `worker.rs`, `properties/mod.rs`

---

## Phase 6: Simulation & Verification Enhancements (4 sessions)

### 6.1 Rapid Collision Detection
**Sessions: 2** | Priority: High — safety feature

Detect G0 rapid moves that pass through stock material.

**Core** (`rs_cam_core/src/collision.rs` — extend existing):
```
struct RapidCollision {
    move_index: usize,
    start: P3,
    end: P3,
    penetration_z: f64,
}

fn check_rapid_collisions(toolpath: &Toolpath, stock_bbox: &BoundingBox3) -> Vec<RapidCollision>
```

Algorithm:
1. For each `MoveType::Rapid` move:
   - Sample points along the move at ~1mm intervals
   - If any point is below stock top Z AND within stock XY bounds → collision
   - More precisely: if Z < heightmap_z_at(x, y) after prior cuts → collision
2. Return list of collision positions

**Visualization**: Red line segments on rapid collision moves. Already have collision marker infrastructure — extend it.

**Simulation integration**: After simulation completes, run rapid collision check. Red segments on timeline (like Fusion).

**Files touched**: `collision.rs` (extend), `worker.rs` (add check after simulation), `app.rs` (render rapid collision segments), `viewport_overlay.rs` (show collision count)

### 6.2 Stock Coloring by Deviation
**Sessions: 2** | Priority: Medium — quality verification

Color the simulation mesh by deviation from the target model surface.

**Implementation**: After simulation, for each vertex of the heightmap mesh:
1. Find the corresponding model surface Z via drop-cutter at that XY
2. Compute deviation: `sim_z - model_z`
3. Color-map: green (on-target) → yellow (slight overcut) → red (major overcut) → blue (undercut / material remaining)

**State**: Add `SimColorMode { ByOperation, ByDeviation, ByHeight }` to `SimulationState`.

**Rendering**: The `SimMeshGpuData` already stores per-vertex data. Add a vertex color attribute to the mesh shader, or compute deviation colors in `from_heightmap_mesh()`.

**Note**: Needs the model mesh accessible during visualization — store a reference to the drop-cutter grid or run deviation computation on the worker thread.

**Files touched**: `sim_render.rs` (deviation coloring), `simulation.rs` (state), `worker.rs` (compute deviation map), `viewport_overlay.rs` (color mode toggle), `render/mod.rs` (shader changes)

---

## Phase 7: UI & Workflow Features (5 sessions)

### 7.1 Operation Presets / Templates
**Sessions: 1** | Priority: High — saves time on repeat jobs

Save and load named parameter sets.

**Implementation**:
1. Serialize `OperationConfig` + `DressupConfig` to TOML
2. Save to `~/.rs_cam/presets/{name}.toml`
3. UI: "Save Preset" / "Load Preset" buttons in toolpath properties panel
4. Preset dropdown at top of parameter section

**Files touched**: `io/` (new `presets.rs`), `properties/mod.rs` (preset buttons), `app.rs` (save/load events)

### 7.2 Setup Sheet Generation
**Sessions: 2** | Priority: Medium — production shops need this

Auto-generate HTML documentation for the job.

**Implementation** (`rs_cam_viz/src/io/setup_sheet.rs`):
1. Generate HTML from `JobState`:
   - Header: job name, date, total estimated time
   - Stock dimensions and material
   - Tool table: number, type, diameter, flute length, etc.
   - Operation table: name, tool, strategy, feeds/speeds, depth, cycle time
   - Per-operation detail: move count, cutting distance, estimated time
2. Save as HTML file via file dialog

**Files touched**: `io/setup_sheet.rs` (new), `io/mod.rs`, `ui/mod.rs` (AppEvent), `app.rs` (event handler), `menu_bar.rs` (menu item)

### 7.3 Manual NC Insertion
**Sessions: 1** | Priority: Medium

Insert raw G-code commands between operations.

**Implementation**:
1. New node type in project tree: "Manual NC" between toolpath entries
2. Stores raw G-code text (e.g., `M0 ; Optional Stop`, `G4 P2.0 ; Dwell`, `M8 ; Coolant On`)
3. During G-code export, insert the raw text at the appropriate position

**State**: Add `ManualNc { text: String }` variant alongside toolpaths in the job's operation list. Or simpler: add `pre_gcode: String, post_gcode: String` fields to `ToolpathEntry`.

**Files touched**: `toolpath.rs` (state), `io/export.rs` (insert manual NC in output), `properties/mod.rs` (text editor for NC), `project_tree.rs` (display)

### 7.4 Operation Suppression & Locking
**Sessions: < 1** | Priority: Low-Medium

Already have enable/disable. Add:
- **Suppress**: strikethrough in tree, excluded from G-code export and simulation (= disabled, rename for clarity)
- **Lock**: prevent auto-regeneration, show lock icon

**Implementation**: Add `locked: bool` to `ToolpathEntry`. Locked ops skip auto-regen. Show lock icon in tree.

**Files touched**: `toolpath.rs` (add `locked`), `project_tree.rs` (lock icon), `app.rs` (skip locked in auto-regen)

### 7.5 Viewport Enhancements
**Sessions: 1** | Priority: Medium

- **Slope angle shading**: Color model mesh by surface normal angle. Green = flat, yellow = moderate, red = steep. Toggle via viewport overlay button. Useful for planning steep/shallow operations.
- **Entry point marker**: Small arrow at the first cutting move of each toolpath. Rendered as 3 line segments forming an arrowhead.

**Slope shading**: Compute per-vertex slope from mesh normals. Add a toggle to swap between Phong shading and slope coloring. Needs a vertex color attribute in mesh shader (or a second mesh pipeline).

**Entry marker**: Extract first non-rapid move position from each toolpath. Render as small line arrow pointing in the direction of the first cut.

**Files touched**: `mesh_render.rs` or `render/mod.rs` (slope coloring shader), `toolpath_render.rs` (entry markers), `viewport_overlay.rs` (toggle buttons)

---

## Phase 8: Shared Parameter Improvements (2 sessions)

### 8.1 Separate Radial/Axial Stock-to-Leave
**Sessions: 1** | Priority: High — easy fix, professional expectation

Split `stock_to_leave` into `stock_to_leave_radial` and `stock_to_leave_axial` on all operations that have it.

**Affected configs**: Adaptive3dConfig, ScallopConfig, SteepShallowConfig, RampFinishConfig, PencilConfig.

**Core changes**: Operations that use stock_to_leave need to apply radial offset to XY and axial offset to Z separately. For drop-cutter based ops: `z = drop_cutter_z - axial_stock`. For push-cutter/waterline: offset contours by `radial_stock`.

**Files touched**: `toolpath.rs` (state — all affected configs), `worker.rs` (pass both values), core operation files (use separate values), `properties/mod.rs` (two DragValues instead of one)

### 8.2 Contact-Only Toolpath
**Sessions: 1** | Priority: Medium

Skip air cutting moves where tool doesn't contact the part.

**Implementation**: Add `skip_air_cuts: bool` to 3D operation configs. In worker, after generating toolpath, filter moves where `CLPoint.contacted == false`.

The `contacted` flag already exists on `CLPoint` in `dropcutter.rs`. For raster toolpaths, need to track which grid points had contact and skip non-contacting segments.

**More precisely**: After generating the toolpath, walk through moves. If the tool is above the mesh surface by more than a threshold AND moving in XY, convert the segment to a rapid (or remove it).

**Files touched**: 3D config structs in `toolpath.rs`, `worker.rs` (filtering pass), `properties/mod.rs` (checkbox)

---

## Phase 9: G-code & Post-Processing (2 sessions)

### 9.1 Canned Drilling Cycles
**Sessions: 1** | Priority: High (paired with drill operation in Phase 1)

Extend PostProcessor trait for canned cycles.

```rust
// Add to PostProcessor trait:
fn drill_simple(&self, x: f64, y: f64, z: f64, r: f64, feed: f64) -> String;  // G81
fn drill_dwell(&self, x: f64, y: f64, z: f64, r: f64, feed: f64, dwell: f64) -> String;  // G82
fn drill_peck(&self, x: f64, y: f64, z: f64, r: f64, feed: f64, peck: f64) -> String;  // G83
fn drill_cancel(&self) -> String;  // G80
```

Default implementations can emit expanded G1 moves for post-processors that don't support canned cycles.

**Files touched**: `gcode.rs` (PostProcessor trait + impls), `drill.rs` (use canned cycles when available)

### 9.2 High Feedrate Mode
**Sessions: < 1** | Priority: Medium

```rust
// Add to PostConfig / PostProcessor:
high_feedrate_mode: bool,
high_feedrate: f64,  // e.g., 5000 mm/min
```

When enabled, replace all `G0` rapid moves with `G1 F{high_feedrate}`. One-line change in each PostProcessor's `rapid()` method.

**Files touched**: `gcode.rs` (modify rapid() when high-feed mode active), `job.rs` (add to PostConfig), `properties/post.rs` (checkbox + value)

---

## Phase 10: Continuous Spiral & TSP Optimization (4 sessions)

### 10.1 Continuous Spiral Waterline
**Sessions: 2** | Priority: Medium — professional finish quality

Replace stepped Z-level contours with a continuous Z-interpolating spiral.

**Implementation**: After generating waterline contours at discrete Z levels:
1. Instead of separate closed loops at each Z, connect the end of one contour to the start of the next via a helical ramp
2. Interpolate Z continuously along each contour so it transitions smoothly from one Z level to the next
3. Result: a single continuous toolpath with no seam lines

Already have `continuous` flag on Scallop. Apply the same concept to Waterline.

**Files touched**: `waterline.rs` (add continuous mode), `toolpath.rs` (state), `worker.rs`, `properties/mod.rs`

### 10.2 TSP Rapid Optimization
**Sessions: 2** | Priority: Medium — measurable cycle time reduction

Reorder independent toolpath segments to minimize total rapid travel.

**Core** (`rs_cam_core/src/tsp.rs`):
```
fn optimize_segment_order(toolpath: &Toolpath, safe_z: f64) -> Toolpath
```

Algorithm:
1. Split toolpath into segments (groups of cutting moves between rapids)
2. Build distance matrix between segment endpoints
3. Nearest-neighbor heuristic: start at first segment, always go to nearest unvisited
4. 2-opt improvement: swap pairs of segments if it reduces total distance
5. Reassemble toolpath in optimized order

Apply as a dressup after generation, before other dressups.

**Files touched**: `tsp.rs` (new), `lib.rs`, `dressup.rs` (or standalone), `toolpath.rs` (state — checkbox), `worker.rs`, `properties/mod.rs`

---

## Implementation Order (Session-by-Session)

| Session | Phase | What | Files | Cumulative |
|---------|-------|------|-------|------------|
| 1 | 1.1 | Face operation | face.rs + wiring | 1 new op |
| 2 | 1.2 | Trace operation | trace.rs + wiring | 2 new ops |
| 3-4 | 1.3 | Drill operation + canned cycles | drill.rs + gcode.rs + wiring | 3 new ops |
| 5-6 | 1.4 | Chamfer operation | chamfer.rs + wiring | 4 new ops |
| 7-8 | 2.1 | Heights system | heights everywhere | Infrastructure |
| 9 | 2.2 | Retraction strategy | dressup.rs extension | Infrastructure |
| 10-11 | 3.1 | Machining boundary/containment | boundary.rs + wiring | Infrastructure |
| 12 | 3.2 | Slope confinement (generalized) | 3D configs + filtering | Infrastructure |
| 13 | 4.1 | Finishing/spring passes | depth.rs + configs | Quality |
| 14 | 4.2 | Compensation type (G41/G42) | gcode.rs + profile | Quality |
| 15 | 8.1 | Radial/axial stock-to-leave | Split params | Quality |
| 16 | 8.2 | Contact-only toolpath | 3D filtering | Efficiency |
| 17 | 9.2 | High feedrate mode | gcode.rs | Quick win |
| 18 | 7.1 | Operation presets | presets.rs | UX |
| 19 | 7.4 | Suppression + locking | State + tree | UX |
| 20 | 7.3 | Manual NC insertion | State + export | UX |
| 21-22 | 6.1 | Rapid collision detection | collision.rs + viz | Safety |
| 23-24 | 7.2 | Setup sheet generation | setup_sheet.rs | Documentation |
| 25 | 7.5 | Slope shading + entry markers | Render enhancements | UX |
| 26-28 | 5.1 | Spiral finishing | spiral.rs + wiring | 3D finishing |
| 29-30 | 5.2 | Radial finishing | radial.rs + wiring | 3D finishing |
| 31-33 | 5.3 | Horizontal flat area finishing | horizontal.rs + wiring | 3D finishing |
| 34-37 | 5.4 | Project / curve-on-surface | project.rs + wiring | 3D finishing |
| 38-39 | 6.2 | Stock deviation coloring | sim_render.rs + worker | Verification |
| 40-41 | 10.1 | Continuous spiral waterline | waterline.rs | Finish quality |
| 42-43 | 10.2 | TSP rapid optimization | tsp.rs + dressup | Efficiency |

**Total: ~43 sessions** covering 4 new operations, 5 new 3D finishing strategies, heights system, boundary control, retraction strategies, finishing passes, compensation, rapid collision detection, setup sheets, presets, and optimization.

---

## Verification Checklist

After each phase:
1. `cargo check --workspace` — zero warnings
2. `cargo test -p rs_cam_core` — all tests pass
3. Manual testing:
   - **Phase 1**: Create Face → single pass at stock top. Create Trace → follows SVG path. Create Drill → pecks at point positions.
   - **Phase 2**: Set different heights → rapids use clearance_z, passes use retract_z. Change retraction strategy → observe fewer rapids.
   - **Phase 3**: Set machining boundary → toolpath stays inside. Enable slope confinement → only steep/shallow areas machined.
   - **Phase 5**: Spiral → continuous spiral over dome. Radial → spoked passes. Horizontal → only flat areas.
   - **Phase 6**: Generate rapid through stock → red collision marker shown.
