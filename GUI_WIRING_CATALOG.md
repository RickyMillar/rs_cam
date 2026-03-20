# GUI Wiring Catalog — Feature Gaps Branch

All features implemented on `worktree-feature-gaps`. This catalog documents every function, type, and UI element for future GUI integration work.

---

## 1. New Operations (8) — Fully Wired

Each operation has: core algorithm + OperationType enum + OperationConfig variant + config struct + worker run_* function + UI draw_*_params function + tooltips.

### 1.1 Face (Surfacing)
- **Core**: `rs_cam_core::face::face_toolpath(bounds: &BoundingBox3, params: &FaceParams) -> Toolpath`
- **State**: `OperationType::Face`, `OperationConfig::Face(FaceConfig)`
- **Config**: `FaceConfig { stepover, depth, depth_per_pass, feed_rate, plunge_rate, stock_offset, direction: FaceDirection }`
- **Worker**: `run_face(req, cfg)` — uses `req.stock_bbox` (unique: stock-based, not mesh/polygon)
- **UI**: `draw_face_params()` — direction dropdown (OneWay/Zigzag), stepover, depth, feed, plunge, stock offset
- **Tests**: 4 in `face.rs`

### 1.2 Trace (Follow Path)
- **Core**: `rs_cam_core::trace::trace_toolpath(polygon: &Polygon2, params: &TraceParams) -> Toolpath`
- **State**: `OperationType::Trace`, `OperationConfig::Trace(TraceConfig)`
- **Config**: `TraceConfig { depth, depth_per_pass, feed_rate, plunge_rate, compensation: TraceCompensation }`
- **Worker**: `run_trace(req, cfg)` — uses polygons, depth stepping
- **UI**: `draw_trace_params()` — compensation dropdown (None/Left/Right), depth, feed, plunge
- **Tests**: 8 in `trace.rs`

### 1.3 Drill
- **Core**: `rs_cam_core::drill::drill_toolpath(holes: &[[f64; 2]], params: &DrillParams) -> Toolpath`
- **Core types**: `DrillCycle { Simple, Dwell(f64), Peck(f64), ChipBreak(f64, f64) }`
- **State**: `OperationType::Drill`, `OperationConfig::Drill(DrillConfig)`
- **Config**: `DrillConfig { depth, cycle: DrillCycleType, peck_depth, dwell_time, retract_amount, feed_rate, retract_z }`
- **Worker**: `run_drill(req, cfg)` — extracts hole positions from polygon centroids
- **UI**: `draw_drill_params()` — cycle dropdown (Simple/Dwell/Peck/ChipBreak), conditional fields
- **Tests**: 7 in `drill.rs`

### 1.4 Chamfer
- **Core**: `rs_cam_core::chamfer::chamfer_toolpath(polygon: &Polygon2, params: &ChamferParams) -> Toolpath`
- **State**: `OperationType::Chamfer`, `OperationConfig::Chamfer(ChamferConfig)`
- **Config**: `ChamferConfig { chamfer_width, tip_offset, feed_rate, plunge_rate }`
- **Worker**: `run_chamfer(req, cfg)` — requires V-Bit tool, computes depth from angle
- **UI**: `draw_chamfer_params()` — V-Bit required label, width, tip offset, feed, plunge
- **Validation**: "Chamfer requires a V-Bit tool" error in `validate_toolpath()`
- **Tests**: 7 in `chamfer.rs`

### 1.5 Spiral Finish
- **Core**: `rs_cam_core::spiral_finish::spiral_finish_toolpath(mesh, index, cutter, params) -> Toolpath`
- **State**: `OperationType::SpiralFinish`, `OperationConfig::SpiralFinish(SpiralFinishConfig)`
- **Config**: `SpiralFinishConfig { stepover, direction: SpiralDirection, feed_rate, plunge_rate, stock_to_leave_radial, stock_to_leave_axial }`
- **Worker**: `run_spiral_finish(req, cfg)` — maps SpiralDirection enum
- **UI**: `draw_spiral_finish_params()` — stepover, direction dropdown, feed, plunge, wall/floor stock
- **Tests**: 7 in `spiral_finish.rs`

### 1.6 Radial Finish
- **Core**: `rs_cam_core::radial_finish::radial_finish_toolpath(mesh, index, cutter, params) -> Toolpath`
- **State**: `OperationType::RadialFinish`, `OperationConfig::RadialFinish(RadialFinishConfig)`
- **Config**: `RadialFinishConfig { angular_step, point_spacing, feed_rate, plunge_rate, stock_to_leave_radial, stock_to_leave_axial }`
- **Worker**: `run_radial_finish(req, cfg)`
- **UI**: `draw_radial_finish_params()` — angular step, point spacing, feed, plunge, wall/floor stock
- **Tests**: 11 in `radial_finish.rs`

### 1.7 Horizontal Finish
- **Core**: `rs_cam_core::horizontal_finish::horizontal_finish_toolpath(mesh, index, cutter, params) -> Toolpath`
- **State**: `OperationType::HorizontalFinish`, `OperationConfig::HorizontalFinish(HorizontalFinishConfig)`
- **Config**: `HorizontalFinishConfig { angle_threshold, stepover, feed_rate, plunge_rate, stock_to_leave_radial, stock_to_leave_axial }`
- **Worker**: `run_horizontal_finish(req, cfg)`
- **UI**: `draw_horizontal_finish_params()` — angle threshold, stepover, feed, plunge, wall/floor stock
- **Tests**: 4 in `horizontal_finish.rs`

### 1.8 Project Curve
- **Core**: `rs_cam_core::project_curve::project_curve_toolpath(polygon, mesh, index, cutter, params) -> Toolpath`
- **State**: `OperationType::ProjectCurve`, `OperationConfig::ProjectCurve(ProjectCurveConfig)`
- **Config**: `ProjectCurveConfig { depth, point_spacing, feed_rate, plunge_rate }`
- **Worker**: `run_project_curve(req, cfg)` — needs BOTH polygons AND mesh
- **UI**: `draw_project_curve_params()` — depth, point spacing, feed, plunge
- **Tests**: 7 in `project_curve.rs`

---

## 2. Infrastructure Modules (2) — Core + GUI Wired

### 2.1 Machining Boundary (boundary.rs)
- **Core functions**:
  - `effective_boundary(polygon, containment, tool_radius) -> Vec<Polygon2>`
  - `clip_toolpath_to_boundary(tp, boundary, safe_z) -> Toolpath`
  - `point_in_polygon(px, py, polygon) -> bool` (internal)
- **Core types**: `ToolContainment { Center, Inside, Outside }`
- **State**: `boundary_enabled: bool` + `boundary_containment: BoundaryContainment` on `ToolpathEntry`
- **Worker**: applied in `run_compute()` after dressups, clips to stock bbox
- **UI**: collapsible "Machining Boundary" section with enable checkbox + containment dropdown
- **Tests**: 12 in `boundary.rs`
- **Future**: support custom polygon boundaries (not just stock bbox), boundary offset DragValue

### 2.2 TSP Rapid Optimization (tsp.rs)
- **Core functions**:
  - `optimize_rapid_order(toolpath, safe_z) -> Toolpath`
  - Internal: `split_into_segments()`, `xy_distance()`, nearest-neighbor + 2-opt
- **State**: `optimize_rapid_order: bool` on `DressupConfig`
- **Worker**: applied in `apply_dressups()` chain (last step)
- **UI**: checkbox "Optimize rapid travel order" in Modifications section
- **Tests**: 6 in `tsp.rs`

---

## 3. Heights System — Fully Wired

### Types (in `state/toolpath.rs`)
- `HeightMode { Auto, Manual(f64) }` — per-height auto/manual toggle
- `HeightsConfig { clearance_z, retract_z, feed_z, top_z, bottom_z }` — all `HeightMode`
- `ResolvedHeights { clearance_z, retract_z, feed_z, top_z, bottom_z }` — concrete f64 values
- `HeightsConfig::resolve(safe_z, op_depth) -> ResolvedHeights` — auto-compute resolution

### Worker integration
- `effective_safe_z(req) -> f64` — returns `req.heights.retract_z` (32 call sites)
- `make_depth_from_heights(heights, per_pass, finishing_passes) -> DepthStepping`
- `operation_depth(op) -> f64` — extracts depth from any OperationConfig variant (22 match arms)
- Heights resolved in `app.rs::submit_toolpath_compute()` before passing to worker

### UI
- `draw_heights_params(ui, heights)` — collapsible "Heights" section
- `draw_height_row(ui, label, mode, tooltip, id)` — per-height: label + Auto checkbox + Manual DragValue
- 5 rows: Clearance Z, Retract Z, Feed Z, Top Z, Bottom Z
- Each has tooltip explaining purpose and auto-compute formula

### Auto-compute defaults
- `clearance_z` = retract_z + 10mm
- `retract_z` = PostConfig.safe_z
- `feed_z` = retract_z - 2mm
- `top_z` = 0.0 (stock surface)
- `bottom_z` = -operation_depth

---

## 4. Parameter Improvements — Fully Wired

### 4.1 Radial/Axial Stock-to-Leave
- **Changed**: `stock_to_leave: f64` → `stock_to_leave_radial: f64` + `stock_to_leave_axial: f64`
- **Affected configs** (8): Adaptive3dConfig, PencilConfig, ScallopConfig, SteepShallowConfig, RampFinishConfig, SpiralFinishConfig, RadialFinishConfig, HorizontalFinishConfig
- **UI**: "Wall Stock:" and "Floor Stock:" DragValues (was single "Stock to Leave:")
- **Worker**: passes `stock_to_leave_axial` to core ops (Z-offset for drop-cutter based ops)

### 4.2 Finishing Passes (Spring Passes)
- **Core**: `finishing_passes: usize` field on `DepthStepping` — repeats final Z level N times
- **State**: `finishing_passes: usize` on `PocketConfig` and `ProfileConfig`
- **Worker**: `make_depth_with_finishing(depth, per_pass, finishing_passes)`
- **UI**: "Finishing Passes:" DragValue (0-10) on Pocket and Profile panels

### 4.3 Contact-Only Toolpath
- **State**: `skip_air_cuts: bool` on `DropCutterConfig`
- **UI**: "Skip Air Cuts:" checkbox on 3D Finish (DropCutter) panel
- **Future**: implement actual move filtering in worker using CLPoint.contacted flag

### 4.4 Slope Confinement (on DropCutter)
- **State**: `slope_from: f64`, `slope_to: f64` on `DropCutterConfig`
- **UI**: "Slope From:" and "Slope To:" DragValues (0-90 degrees)
- **Future**: implement actual slope filtering in worker post-generation

---

## 5. Dressup / Motion Control — UI Wired

### 5.1 Retraction Strategy
- **State**: `RetractStrategy { Full, Minimum }` on `DressupConfig`
- **UI**: "Retract Strategy:" dropdown in Modifications section
- **Future**: implement Minimum retract logic as post-processing dressup in worker

### 5.2 Compensation Type
- **State**: `CompensationType { InComputer, InControl }` on `ProfileConfig`
- **UI**: "Compensation:" dropdown on Profile panel (In Computer / In Control G41/G42)
- **Future**: emit G41/G42/G40 in gcode.rs when InControl, skip tool radius offset in worker

### 5.3 Continuous Waterline
- **State**: `continuous: bool` on `WaterlineConfig`
- **UI**: "Continuous:" checkbox on Waterline panel
- **Future**: implement Z-interpolating spiral logic in waterline.rs

---

## 6. G-code & Post-Processing — Fully Wired

### 6.1 Canned Drilling Cycles
- **PostProcessor trait** new default-implemented methods:
  - `drill_simple(x, y, z, r, feed) -> String` — G81
  - `drill_dwell(x, y, z, r, feed, dwell) -> String` — G82
  - `drill_peck(x, y, z, r, feed, peck) -> String` — G83
  - `drill_chip_break(x, y, z, r, feed, peck) -> String` — G73
  - `drill_cancel() -> String` — G80
- All existing post-processors (GRBL, LinuxCNC, Mach3) inherit defaults automatically
- **Future**: drill.rs can optionally use these instead of expanded G1 moves

### 6.2 High Feedrate Mode
- **State**: `high_feedrate_mode: bool` + `high_feedrate: f64` on `PostConfig`
- **Core**: `replace_rapids_with_feed(gcode, high_feedrate) -> String` — G0 → G1 text replacement
- **Export**: applied automatically in `export_gcode()` when enabled
- **UI**: checkbox + DragValue in Post Processor properties panel

---

## 7. Manual NC Insertion — UI Wired

- **State**: `pre_gcode: String` + `post_gcode: String` on `ToolpathEntry`
- **UI**: collapsible "Manual G-code" section with "Before:" and "After:" multiline text fields
- **Future**: insert `pre_gcode`/`post_gcode` in `io/export.rs::export_gcode()` during G-code generation

---

## 8. Operation Locking — Fully Wired

- **State**: `locked: bool` on `ToolpathEntry`
- **App**: locked ops skip debounced auto-regeneration
- **Future**: show lock icon in project tree, add Lock/Unlock context menu item

---

## 9. Rapid Collision Detection — Core Ready

- **Core**: `collision::check_rapid_collisions(toolpath, stock_bbox) -> Vec<RapidCollision>`
- **Type**: `RapidCollision { move_index, start: P3, end: P3 }`
- **Algorithm**: samples G0 rapids at 1mm intervals, checks against stock bounds
- **Tests**: 3 in `collision.rs`
- **Future wiring needed**:
  - Call after simulation in worker, pass results to app
  - Render red line segments on collision rapids (use existing collision marker pipeline)
  - Show collision count in viewport overlay
  - Red segments on simulation timeline

---

## 10. Stock Deviation Coloring — Infrastructure Ready

- **Render**: `sim_render::deviation_colors(deviations: &[f32]) -> Vec<[f32; 3]>`
  - Green (on-target ±0.1mm) → Yellow (slight overcut) → Red (major overcut) → Blue (remaining)
- **State**: `deviations: Option<Vec<f32>>` on `SimulationResult`
- **Future wiring needed**:
  - Add model mesh (`Arc<TriangleMesh>`) to `SimulationRequest`
  - In `run_simulation()`: after heightmap, drop-cutter model mesh at each vertex → compute deviations
  - In `app.rs`: when deviations are present, use `from_heightmap_mesh_with_deviation()` (to be written)
  - Add `SimColorMode { ByWood, ByDeviation }` toggle to viewport overlay
  - Requires per-vertex color in mesh shader (new attribute or second pipeline)

---

## 11. Operation Presets — Core Ready

- **Module**: `io/presets.rs`
- **Functions**: `presets_dir()`, `list_presets()`, `save_preset()`, `load_preset()`, `delete_preset()`
- **Type**: `Preset { name, operation_label, toml_content }`
- **Storage**: `~/.rs_cam/presets/{sanitized_name}.toml`
- **Tests**: 9 in `presets.rs`
- **Future wiring needed**:
  - Add "Save Preset..." / "Load Preset..." buttons to toolpath properties panel
  - Serialize OperationConfig fields to TOML string for `toml_content`
  - Parse TOML string back to update OperationConfig fields
  - Preset dropdown at top of parameter section
  - Ship 3-4 built-in presets

---

## 12. Setup Sheet Generation — Fully Wired

- **Module**: `io/setup_sheet.rs`
- **Function**: `generate_setup_sheet(job: &JobState) -> String` — produces self-contained HTML
- **Content**: header, stock table, tool table, operations table, post-processor info, per-op details, estimated times
- **Event**: `AppEvent::ExportSetupSheet`
- **Menu**: File > Export Setup Sheet...
- **App handler**: saves HTML via file dialog
- **Tests**: 12 in `setup_sheet.rs`

---

## 13. Entry Point Markers — Render Ready

- **Render**: `toolpath_render::entry_marker_vertices(tp: &Toolpath, palette_color: [f32; 3]) -> Vec<LineVertex>`
- **Geometry**: 2mm arrowhead at first cutting move, pointing in cut direction (3 line segments)
- **Future wiring needed**:
  - In `app.rs::upload_gpu_data()`: for each toolpath, call `entry_marker_vertices()`, accumulate verts, upload as additional line buffer
  - Draw in the line pipeline alongside toolpath lines
  - Toggle via viewport overlay button

---

## Summary Table

| Feature | Core Algorithm | State/Config | Worker Dispatch | UI Panel | Fully Functional |
|---------|---------------|--------------|-----------------|----------|-----------------|
| Face | face.rs | FaceConfig | run_face | draw_face_params | Yes |
| Trace | trace.rs | TraceConfig | run_trace | draw_trace_params | Yes |
| Drill | drill.rs | DrillConfig | run_drill | draw_drill_params | Yes |
| Chamfer | chamfer.rs | ChamferConfig | run_chamfer | draw_chamfer_params | Yes |
| Spiral Finish | spiral_finish.rs | SpiralFinishConfig | run_spiral_finish | draw_spiral_finish_params | Yes |
| Radial Finish | radial_finish.rs | RadialFinishConfig | run_radial_finish | draw_radial_finish_params | Yes |
| Horizontal Finish | horizontal_finish.rs | HorizontalFinishConfig | run_horizontal_finish | draw_horizontal_finish_params | Yes |
| Project Curve | project_curve.rs | ProjectCurveConfig | run_project_curve | draw_project_curve_params | Yes |
| Boundary | boundary.rs | boundary_enabled + containment | clips in run_compute | collapsible section | Yes |
| TSP | tsp.rs | optimize_rapid_order | in apply_dressups | checkbox | Yes |
| Heights | — (viz-only) | HeightsConfig | effective_safe_z (32 sites) | 5-row collapsible | Yes |
| Stock-to-leave split | — | radial + axial fields (8 configs) | axial passed to core | Wall/Floor Stock | Yes |
| Finishing passes | depth.rs field | on Pocket + Profile | make_depth_with_finishing | DragValue (0-10) | Yes |
| High feedrate | gcode.rs | PostConfig fields | export applies | checkbox + value | Yes |
| Canned cycles | gcode.rs (5 methods) | — | — | — | Core ready |
| Retraction strategy | — | RetractStrategy enum | — | dropdown | UI ready |
| Compensation | — | CompensationType enum | — | dropdown | UI ready |
| Manual NC | — | pre/post gcode strings | — | multiline text | UI ready |
| Continuous waterline | — | continuous bool | — | checkbox | UI ready |
| Contact-only | — | skip_air_cuts bool | — | checkbox | UI ready |
| Slope confinement | — | slope_from/to | — | DragValues | UI ready |
| Locking | — | locked bool | skip auto-regen | — | Partial |
| Rapid collision | collision.rs (3 tests) | — | — | — | Core ready |
| Deviation coloring | sim_render.rs | deviations field | — | — | Infra ready |
| Presets | io/presets.rs (9 tests) | — | — | — | Core ready |
| Setup sheet | io/setup_sheet.rs (12 tests) | — | — | menu item | Yes |
| Entry markers | toolpath_render.rs | — | — | — | Render ready |

**Legend**: "Yes" = end-to-end functional. "Core/UI/Infra/Render ready" = algorithm done, needs final pipeline connection.
