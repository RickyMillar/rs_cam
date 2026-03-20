# rs_cam Feature Implementation Plan

Covers all gaps from FEATURE_GAP_REPORT.md **except** feeds & speeds calculator and tool library features (handled by separate agent).

## Status Summary

| Category | Done | Core-only | Not started | Total |
|----------|------|-----------|-------------|-------|
| New operations | 8 | 0 | 0 | 8 |
| Infrastructure | 1 | 3 | 6 | 10 |
| UI/Workflow | 0 | 0 | 5 | 5 |
| Simulation | 0 | 0 | 2 | 2 |
| G-code | 1 | 0 | 1 | 2 |
| **Total** | **10** | **3** | **14** | **27** |

---

## COMPLETED (for reference)

These are done with core algorithms, state enums, worker dispatch, UI panels, tooltips, and tests:

- ~~1.1 Face operation~~ (face.rs, 4 tests)
- ~~1.2 Trace operation~~ (trace.rs, 8 tests)
- ~~1.3 Drill operation~~ (drill.rs, 7 tests)
- ~~1.4 Chamfer operation~~ (chamfer.rs, 7 tests)
- ~~5.1 Spiral Finish~~ (spiral_finish.rs, 7 tests)
- ~~5.2 Radial Finish~~ (radial_finish.rs, 11 tests)
- ~~5.3 Horizontal Finish~~ (horizontal_finish.rs, 4 tests)
- ~~5.4 Project Curve~~ (project_curve.rs, 7 tests)
- ~~9.2 High Feedrate Mode~~ (gcode.rs + PostConfig + UI)

Core algorithms done, need GUI wiring:
- ~~3.1 Boundary/containment~~ core (boundary.rs, 12 tests)
- ~~4.1 Finishing passes~~ core (DepthStepping.finishing_passes field)
- ~~10.2 TSP optimization~~ core (tsp.rs, 6 tests)

---

## REMAINING WORK — Phased Plan

### Phase A: Wire Existing Core Modules (1-2 sessions)

These have working core algorithms with tests but aren't connected to the GUI yet. Low risk, high reward — just plumbing.

#### A1. Wire boundary/containment into GUI
**Session: 1** | Files: `toolpath.rs`, `worker.rs`, `properties/mod.rs`

1. Add to `ToolpathEntry`:
   ```
   pub boundary_enabled: bool,
   pub boundary_containment: ToolContainment,  // from boundary.rs
   ```
2. In `worker.rs` after `apply_dressups()`: if boundary_enabled, call `clip_toolpath_to_boundary()` using stock bbox as the boundary polygon
3. In `properties/mod.rs`: add collapsible "Machining Boundary" section with enable checkbox + containment dropdown (Center/Inside/Outside)

#### A2. Wire finishing passes into UI
**Session: < 1** | Files: `toolpath.rs` (Profile/Pocket configs), `worker.rs`, `properties/mod.rs`

1. Add `finishing_passes: usize` to `ProfileConfig` and `PocketConfig` (default: 0)
2. In `worker.rs` `make_depth()`: pass `finishing_passes` from config to `DepthStepping`
3. In `properties/mod.rs`: add "Finishing Passes:" DragValue (0-10) in profile and pocket param grids
4. Add tooltip: "Number of spring passes at final depth for dimensional accuracy"

#### A3. Wire TSP optimization as dressup option
**Session: < 1** | Files: `toolpath.rs` (DressupConfig), `worker.rs`, `properties/mod.rs`

1. Add `optimize_rapid_order: bool` to `DressupConfig` (default: false)
2. In `worker.rs` `apply_dressups()`: if enabled, call `tsp::optimize_rapid_order()`
3. In `properties/mod.rs` dressup section: add checkbox "Optimize rapid travel order"
4. Add tooltip: "Reorder toolpath segments to minimize rapid travel distance (TSP optimization)"

---

### Phase B: Quick Infrastructure Wins (2-3 sessions)

Small features that touch few files and provide outsized UX value.

#### B1. Radial/axial stock-to-leave split
**Session: 1** | Files: `toolpath.rs`, `worker.rs`, `properties/mod.rs`, core op files

1. On these configs, replace `stock_to_leave: f64` with `stock_to_leave_radial: f64` + `stock_to_leave_axial: f64`:
   - `Adaptive3dConfig`, `ScallopConfig`, `SteepShallowConfig`, `RampFinishConfig`, `PencilConfig`
   - Also the new: `SpiralFinishConfig`, `RadialFinishConfig`, `HorizontalFinishConfig`
2. In worker run_* functions: pass `stock_to_leave_axial` for Z offset (drop-cutter ops), `stock_to_leave_radial` for XY offset (push-cutter/waterline ops)
3. In UI: replace single "Stock to Leave" DragValue with two: "Wall Stock:" and "Floor Stock:"
4. Defaults: both 0.0 (same behavior as before)

#### B2. Contact-only toolpath
**Session: 1** | Files: `toolpath.rs` (3D configs), `worker.rs`, `properties/mod.rs`

1. Add `skip_air_cuts: bool` (default: false) to `DropCutterConfig`, `SpiralFinishConfig`, `RadialFinishConfig`, `HorizontalFinishConfig`
2. In worker, after generating 3D toolpath: walk moves, convert sequences where tool is above mesh + threshold into rapids
3. In UI: checkbox "Skip air cuts" on the relevant 3D ops
4. Implementation: compare each move's Z against `point_drop_cutter(mesh, x, y).z` — if tool Z is more than 5mm above surface AND moving laterally, convert to rapid

#### B3. Canned drilling cycles in PostProcessor
**Session: 1** | Files: `gcode.rs`

Add default-implemented methods to `PostProcessor` trait:
```rust
fn drill_simple(&self, x: f64, y: f64, z: f64, r: f64, feed: f64) -> String {
    format!("G81 X{x:.4} Y{y:.4} Z{z:.4} R{r:.4} F{feed:.1}\n")
}
fn drill_dwell(&self, x: f64, y: f64, z: f64, r: f64, feed: f64, dwell: f64) -> String {
    format!("G82 X{x:.4} Y{y:.4} Z{z:.4} R{r:.4} P{dwell:.2} F{feed:.1}\n")
}
fn drill_peck(&self, x: f64, y: f64, z: f64, r: f64, feed: f64, peck: f64) -> String {
    format!("G83 X{x:.4} Y{y:.4} Z{z:.4} R{r:.4} Q{peck:.4} F{feed:.1}\n")
}
fn drill_cancel(&self) -> String { "G80\n".to_string() }
```
These are default impls so existing post-processors work without changes. drill.rs can optionally use them instead of expanded moves (future enhancement).

#### B4. Operation suppression & locking
**Session: < 1** | Files: `toolpath.rs`, `project_tree.rs`, `app.rs`

1. Add `locked: bool` to `ToolpathEntry` (default: false)
2. In `project_tree.rs`: show lock icon (🔒 unicode) for locked ops
3. In `app.rs` auto-regen loop: skip locked toolpaths
4. In context menu: add "Lock / Unlock" option

---

### Phase C: Heights System (2-3 sessions)

The biggest single infrastructure change. Replaces `safe_z` with a professional 5-level height model.

#### C1. Heights data model
**Session: 1** | Files: `toolpath.rs` (new HeightsConfig), `worker.rs`, `app.rs`

```rust
#[derive(Debug, Clone)]
pub struct HeightsConfig {
    pub clearance_z: f64,  // safe travel between ops (default: safe_z + 10)
    pub retract_z: f64,    // between passes within one op (default: safe_z)
    pub feed_z: f64,       // switch from rapid to feed (default: safe_z - 2)
    pub top_z: f64,        // top of cut (default: 0.0)
    pub bottom_z: f64,     // final depth (default: -depth, auto-computed)
}
```

Add `heights: HeightsConfig` to `ToolpathEntry`. In worker, pass `heights.retract_z` where `safe_z` was used, `heights.clearance_z` for inter-operation rapids.

**Key constraint**: This changes the signature of all `run_*` functions. Do it as a refactor pass across all 22 operations in worker.rs.

#### C2. Heights UI panel
**Session: 1** | Files: `properties/mod.rs`

Add collapsible "Heights" section in toolpath properties:
- 5 DragValues: Clearance Z, Retract Z, Feed Z, Top Z, Bottom Z
- Each shows suffix " mm" and appropriate range
- "Auto" checkbox for Bottom Z (computed from depth param)
- Tooltips explaining each height level

#### C3. Heights viewport visualization
**Session: 1** | Files: `app.rs`, `render/mod.rs`

When editing heights, draw 5 colored horizontal planes in the viewport at the height levels:
- Blue = clearance, Green = retract, Yellow = feed, White = top, Red = bottom
- Use the line pipeline to draw rectangle outlines at stock XY bounds at each Z
- Only show when a toolpath is selected and heights section is expanded

---

### Phase D: Retraction & Motion Control (2 sessions)

#### D1. Retraction strategy
**Session: 1** | Files: `dressup.rs` (core), `toolpath.rs`, `worker.rs`, `properties/mod.rs`

```rust
enum RetractStrategy {
    Full,       // always retract to retract_z (current, safest)
    Minimum,    // retract just above max Z on nearby path + safe_distance
    Direct,     // straight line if clear (needs heightmap check)
}
```

Implement as post-processing dressup in `apply_dressups()`:
- Full: no change (current behavior)
- Minimum: for each retract-rapid-plunge, lower retract Z to `max_z_nearby + 2mm`
- Direct: replace retract-rapid-plunge with single feed move (only if heightmap shows no stock in the way — approximate with stock bbox Z check)

Add to DressupConfig: `retract_strategy: RetractStrategy` (default: Full).
Add to UI dressups section: dropdown.

#### D2. Generalized slope confinement
**Session: 1** | Files: `toolpath.rs` (3D configs), `worker.rs`, `properties/mod.rs`

1. Add `slope_from: f64` (default: 0.0) and `slope_to: f64` (default: 90.0) to all 3D configs that don't already have them: `DropCutterConfig`, `Adaptive3dConfig`, `WaterlineConfig`, `PencilConfig`, `SpiralFinishConfig`, `RadialFinishConfig`, `HorizontalFinishConfig`, `ProjectCurveConfig`
2. In worker, after generating 3D toolpath: post-filter moves where surface normal angle falls outside `[slope_from, slope_to]` range — convert those segments to rapids
3. Need helper: `fn surface_slope_at(mesh, index, x, y) -> Option<f64>` — find nearest triangle, compute `acos(normal.z).to_degrees()`
4. In UI: add "Slope From/To" DragValues on all 3D ops (some already have them — skip those)

---

### Phase E: G-code & Compensation (1-2 sessions)

#### E1. Compensation type (G41/G42)
**Session: 1** | Files: `gcode.rs`, `toolpath.rs`, `worker.rs`, `properties/mod.rs`

```rust
enum CompensationType { InComputer, InControl }
```

Add to `PostProcessor` trait (default impls):
```rust
fn tool_comp_left(&self, tool_num: u32) -> String { format!("G41 D{tool_num}\n") }
fn tool_comp_right(&self, tool_num: u32) -> String { format!("G42 D{tool_num}\n") }
fn tool_comp_cancel(&self) -> String { "G40\n".to_string() }
```

Add `compensation: CompensationType` to `ProfileConfig` (default: InComputer).
When InControl: skip tool radius offset in CAM, emit G41/G42 before profile pass and G40 after.

#### E2. Continuous spiral waterline
**Session: 1** | Files: `waterline.rs`, `toolpath.rs` (WaterlineConfig), `worker.rs`, `properties/mod.rs`

Add `continuous: bool` to `WaterlineConfig` (default: false).
After generating waterline contours at discrete Z levels:
1. Instead of closed loops at each Z, connect end of one contour to start of the next via helical ramp
2. Interpolate Z continuously along each contour for smooth transition
3. Result: single continuous toolpath with no seam lines

Scallop already has a `continuous` flag — follow the same pattern.

---

### Phase F: Simulation & Verification (3-4 sessions)

#### F1. Rapid collision detection
**Session: 2** | Files: `collision.rs` (core), `worker.rs`, `app.rs`, `viewport_overlay.rs`

```rust
pub struct RapidCollision {
    pub move_index: usize,
    pub start: P3,
    pub end: P3,
}

pub fn check_rapid_collisions(toolpath: &Toolpath, stock_bbox: &BoundingBox3) -> Vec<RapidCollision>
```

Algorithm: For each Rapid move, sample points at 1mm intervals. If any point is below stock_top_z AND within stock XY bounds, it's a collision.

Wire into simulation: after sim completes, run rapid collision check. Display red line segments on collision rapids (extend existing collision marker infrastructure). Show count in viewport overlay.

#### F2. Stock deviation coloring
**Session: 2** | Files: `sim_render.rs`, `simulation.rs` (state), `worker.rs`, `viewport_overlay.rs`, `render/mod.rs`

After simulation:
1. For each heightmap mesh vertex, drop-cutter the model mesh at that XY
2. Compute deviation: `sim_z - model_z`
3. Color-map: green (0) → yellow (overcut) → red (major overcut) → blue (undercut)

Add `SimColorMode { ByWood, ByDeviation }` to SimulationState. Toggle in viewport overlay. Requires model mesh accessible during visualization — compute deviation map on worker thread, pass as vertex colors to sim mesh.

---

### Phase G: UI & Workflow (4-5 sessions)

#### G1. Operation presets
**Session: 1** | Files: `io/presets.rs` (new), `properties/mod.rs`, `app.rs`, `ui/mod.rs`

1. Serialize `OperationConfig` + `DressupConfig` to TOML
2. Save to `~/.rs_cam/presets/{name}.toml`
3. UI: "Save Preset..." / "Load Preset..." buttons at top of toolpath properties
4. Dropdown of saved presets
5. Ship with 3-4 built-in presets: "Hardwood Roughing", "MDF Pocket", "Finishing Pass"

#### G2. Setup sheet generation
**Session: 2** | Files: `io/setup_sheet.rs` (new), `io/mod.rs`, `ui/mod.rs`, `app.rs`, `menu_bar.rs`

Generate HTML doc from JobState:
```html
<h1>Job: {name}</h1>
<h2>Stock</h2> <p>{x} x {y} x {z} mm</p>
<h2>Tools</h2> <table>...</table>
<h2>Operations</h2> <table>name, tool, strategy, feeds, depth, est. time</table>
<h2>Estimated Total Time: {min}:{sec}</h2>
```
Add File > Export Setup Sheet menu item. Save as HTML via file dialog.

#### G3. Manual NC insertion
**Session: 1** | Files: `toolpath.rs`, `io/export.rs`, `properties/mod.rs`, `project_tree.rs`

Add `pre_gcode: String` and `post_gcode: String` to `ToolpathEntry` (both default empty).
- `pre_gcode`: inserted before this operation's toolpath in G-code output
- `post_gcode`: inserted after
- UI: collapsible "Manual G-code" section with two multiline text fields
- Common uses: `M0` (optional stop), `M8`/`M9` (coolant), `G4 P2.0` (dwell)

#### G4. Viewport enhancements
**Session: 1** | Files: `toolpath_render.rs`, `render/mod.rs`, `viewport_overlay.rs`

**Entry point markers**: For each toolpath, find the first cutting move. Render a small arrowhead (3 line segments) at that position pointing in the cut direction. Use the toolpath's palette color.

**Slope angle shading**: Add toggle button "Slope" in viewport overlay. When active, color the model mesh vertices by surface normal angle: green (flat, 0°) → yellow (45°) → red (vertical, 90°). Requires computing slope per vertex and passing as vertex color to the mesh shader (add a color attribute to MeshVertex, or use a separate shader pass).

---

## Implementation Order (Recommended)

| Session | Phase | Item | Impact |
|---------|-------|------|--------|
| 1 | A | Wire boundary, finishing passes, TSP into GUI | Unlock 3 existing modules |
| 2-3 | B | Stock-to-leave split, contact-only, canned cycles, locking | Quick parameter wins |
| 4-5 | C | Heights system (data + UI + viewport) | Major professional feature |
| 6-7 | D | Retraction strategy + slope confinement | Motion control |
| 8 | E | Compensation type + continuous waterline | G-code quality |
| 9-10 | F | Rapid collision detection + deviation coloring | Safety & verification |
| 11-13 | G | Presets, setup sheet, manual NC, viewport enhancements | Workflow polish |

**Total remaining: ~13 sessions**

---

## Verification Checklist

After each phase:
1. `cargo check --workspace` — zero warnings
2. `cargo test -p rs_cam_core` — 539+ tests pass
3. Manual testing per phase (see items above for specific test scenarios)
