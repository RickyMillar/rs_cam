# rs_cam Complete Feature Catalog

Every implemented feature, whether it's surfaced in the GUI, and what remains to wire up.

---

## Operations (22 total)

| # | Operation | Core Module | GUI Menu | Parameter Panel | Worker | Fully Functional |
|---|-----------|-------------|----------|-----------------|--------|-----------------|
| 1 | Face | `face.rs` | 2.5D menu | `draw_face_params` | `run_face` | Yes |
| 2 | Pocket | `pocket.rs` | 2.5D menu | `draw_pocket_params` | `run_pocket` | Yes |
| 3 | Profile | `profile.rs` | 2.5D menu | `draw_profile_params` | `run_profile` | Yes |
| 4 | Adaptive | `adaptive.rs` | 2.5D menu | `draw_adaptive_params` | `run_adaptive` | Yes |
| 5 | VCarve | `vcarve.rs` | 2.5D menu | `draw_vcarve_params` | `run_vcarve` | Yes |
| 6 | Rest Machining | `rest.rs` | 2.5D menu | `draw_rest_params` | `run_rest` | Yes |
| 7 | Inlay | `inlay.rs` | 2.5D menu | `draw_inlay_params` | `run_inlay` | Yes |
| 8 | Zigzag | `zigzag.rs` | 2.5D menu | `draw_zigzag_params` | `run_zigzag` | Yes |
| 9 | Trace | `trace.rs` | 2.5D menu | `draw_trace_params` | `run_trace` | Yes |
| 10 | Drill | `drill.rs` | 2.5D menu | `draw_drill_params` | `run_drill` | Yes |
| 11 | Chamfer | `chamfer.rs` | 2.5D menu | `draw_chamfer_params` | `run_chamfer` | Yes |
| 12 | 3D Finish | `dropcutter.rs` | 3D menu | `draw_dropcutter_params` | `run_dropcutter` | Yes |
| 13 | 3D Rough | `adaptive3d.rs` | 3D menu | `draw_adaptive3d_params` | `run_adaptive3d` | Yes |
| 14 | Waterline | `waterline.rs` | 3D menu | `draw_waterline_params` | `run_waterline` | Yes |
| 15 | Pencil | `pencil.rs` | 3D menu | `draw_pencil_params` | `run_pencil` | Yes |
| 16 | Scallop | `scallop.rs` | 3D menu | `draw_scallop_params` | `run_scallop` | Yes |
| 17 | Steep/Shallow | `steep_shallow.rs` | 3D menu | `draw_steep_shallow_params` | `run_steep_shallow` | Yes |
| 18 | Ramp Finish | `ramp_finish.rs` | 3D menu | `draw_ramp_finish_params` | `run_ramp_finish` | Yes |
| 19 | Spiral Finish | `spiral_finish.rs` | 3D menu | `draw_spiral_finish_params` | `run_spiral_finish` | Yes |
| 20 | Radial Finish | `radial_finish.rs` | 3D menu | `draw_radial_finish_params` | `run_radial_finish` | Yes |
| 21 | Horizontal Finish | `horizontal_finish.rs` | 3D menu | `draw_horizontal_finish_params` | `run_horizontal_finish` | Yes |
| 22 | Project Curve | `project_curve.rs` | 3D menu | `draw_project_curve_params` | `run_project_curve` | Yes |

---

## Tool Types (5)

| Tool | Core Type | GUI Selector | Cross-Section Preview | Feeds Geometry |
|------|-----------|-------------|----------------------|----------------|
| End Mill (Flat) | `FlatEndmill` | Yes | Yes | `ToolGeometryHint::Flat` |
| Ball Nose | `BallEndmill` | Yes | Yes | `ToolGeometryHint::Ball` |
| Bull Nose | `BullNoseEndmill` | Yes (+ corner radius) | Yes | `ToolGeometryHint::Bull` |
| V-Bit | `VBitEndmill` | Yes (+ included angle) | Yes | `ToolGeometryHint::VBit` |
| Tapered Ball Nose | `TaperedBallEndmill` | Yes (+ taper angle, shaft dia) | Yes | `ToolGeometryHint::TaperedBall` |

**Tool properties in GUI:** name, type, diameter, cutting length, flute count, material (Carbide/HSS), cut direction (Up/Down/Compression), holder diameter, shank diameter, shank length, stickout, vendor, product ID.

---

## Dressups & Post-Processing (per-toolpath, in "Modifications" collapsible)

| Feature | Core Function | GUI Control | Worker Applied |
|---------|--------------|-------------|----------------|
| Ramp Entry | `apply_entry(Ramp)` | Entry style dropdown + angle | Yes |
| Helix Entry | `apply_entry(Helix)` | Entry style dropdown + radius/pitch | Yes |
| Dogbone Overcuts | `apply_dogbones()` | Checkbox + angle | Yes |
| Lead-in/Lead-out | `apply_lead_in_out()` | Checkbox + radius | Yes |
| Link Moves | `apply_link_moves()` | Checkbox + max distance + feed | Yes |
| Arc Fitting (G2/G3) | `fit_arcs()` | Checkbox + tolerance | Yes |
| Feed Optimization | `optimize_feed_rates()` | Checkbox + max rate + ramp | Yes |
| TSP Rapid Order | `optimize_rapid_order()` | Checkbox | Yes |
| Retract Strategy | — | Dropdown (Full/Minimum) | **UI only** — dressup logic not yet in worker |

---

## Heights System (per-toolpath, in "Heights" collapsible)

| Height | Default Auto Value | GUI Control | Worker Usage |
|--------|-------------------|-------------|-------------|
| Clearance Z | retract_z + 10 | Auto checkbox + Manual DragValue | Available via `heights.clearance_z` |
| Retract Z | PostConfig.safe_z | Auto checkbox + Manual DragValue | `effective_safe_z(req)` — **all 22 ops use this** |
| Feed Z | retract_z - 2 | Auto checkbox + Manual DragValue | Available (not yet used for approach transitions) |
| Top Z | 0.0 | Auto checkbox + Manual DragValue | Available via `heights.top_z` |
| Bottom Z | -operation_depth | Auto checkbox + Manual DragValue | Available via `heights.bottom_z` |

---

## Feeds & Speeds Calculator

| Feature | Core | GUI | Status |
|---------|------|-----|--------|
| Material catalog (23 materials, 5 families) | `material.rs` | Stock panel material picker | **Fully surfaced** |
| Machine profiles (3 presets + custom) | `machine.rs` | Machine panel in project tree | **Fully surfaced** |
| 10-step calculation pipeline | `feeds/mod.rs` | Auto-runs on toolpath selection | **Fully surfaced** |
| Per-field auto/manual toggles | `FeedsAutoMode` | Toggle per: feed, plunge, stepover, depth_per_pass, RPM | **Fully surfaced** |
| Feeds summary card | — | Shows RPM, chipload, feed, plunge, DOC, WOC, power, MRR, warnings | **Fully surfaced** |
| Vendor LUT chipload seeding (61 observations) | `feeds/vendor_lut.rs` | Shows "Source: {id}" in feeds card | **Partially surfaced** |
| Workholding rigidity selector | `WorkholdingRigidity` | — | **Not surfaced** — needs ComboBox in setup/machine panel |
| Tool overhang L/D derate display | L/D calculation in feeds | — | **Not surfaced** — needs label near stickout showing ratio + derate % |
| Vendor source human-readable labels | `VendorObservation` | Shows raw ID only | **Needs polish** — format as "Amana 6mm Flat, Softwood" |
| Vendor LUT loading UI | `VendorLut::load_dir()` | — | **Not surfaced** — needs File menu item |
| Derate breakdown display | Calculated in pipeline | — | **Not surfaced** — needs display showing L/D, workholding, combined effect |

---

## Machining Boundary (per-toolpath, in "Machining Boundary" collapsible)

| Feature | Core | GUI | Worker | Status |
|---------|------|-----|--------|--------|
| Enable boundary clipping | `clip_toolpath_to_boundary()` | Checkbox | Applied after dressups | **Fully functional** |
| Containment mode | `effective_boundary()` | Dropdown (Center/Inside/Outside) | Maps to `ToolContainment` | **Fully functional** |
| Custom boundary polygon | `Polygon2` input | — | Uses stock bbox only | **Not surfaced** — always clips to stock |

---

## Manual NC Insertion (per-toolpath, in "Manual G-code" collapsible)

| Feature | State Field | GUI | Export | Status |
|---------|------------|-----|--------|--------|
| Pre-operation G-code | `pre_gcode: String` | Multiline text editor | — | **UI surfaced, export not wired** |
| Post-operation G-code | `post_gcode: String` | Multiline text editor | — | **UI surfaced, export not wired** |

---

## Simulation & Verification

| Feature | Core | GUI | Status |
|---------|------|-----|--------|
| Heightmap simulation | `simulate_toolpath()` | "Simulate" button + mesh display | **Fully functional** |
| Playback (play/pause/scrub) | `SimulationState` | Timeline + speed control | **Fully functional** |
| Per-toolpath progress | `ToolpathBoundary` | Colored progress bars + op info | **Fully functional** |
| Checkpoint rewind | `SimCheckpoint` | Scrub backward loads checkpoint | **Fully functional** |
| Tool model during playback | `ToolModelGpuData` | Wireframe tool at position | **Fully functional** |
| Tool position readout | `tool_position` | X/Y/Z display in overlay | **Fully functional** |
| Holder/shank collision | `check_collisions_interpolated()` | Red markers in viewport | **Fully functional** |
| Rapid collision detection | `check_rapid_collisions()` | — | **Core ready, not rendered** — needs red line segments on collision rapids |
| Stock deviation coloring | `deviation_colors()` | — | **Infrastructure ready** — needs model mesh in SimRequest + shader changes |

---

## File I/O

| Feature | Core/Module | GUI | Status |
|---------|------------|-----|--------|
| STL import | `mesh.rs` | File menu + project tree button | **Fully functional** |
| SVG import | `svg_input.rs` | File menu + project tree button | **Fully functional** |
| DXF import | `dxf_input.rs` | File menu + project tree button | **Fully functional** |
| TOML job save/load | `io/project.rs` | File menu (Save/Open) | **Fully functional** |
| G-code export | `gcode.rs` + `io/export.rs` | File menu (Export G-code) | **Fully functional** |
| SVG preview export | `viz::toolpath_to_svg()` | File menu (Export SVG Preview) | **Fully functional** |
| Setup sheet (HTML) | `io/setup_sheet.rs` | File menu (Export Setup Sheet) | **Fully functional** |
| Operation presets | `io/presets.rs` | — | **Core ready, not surfaced** — needs Save/Load buttons in properties |

---

## G-code Post-Processors (3)

| Post | Format | Canned Cycles | High Feedrate Mode |
|------|--------|--------------|-------------------|
| GRBL | `GrblPost` | Default impls (G81-G83) | Yes (toggle + rate in UI) |
| LinuxCNC | `LinuxCncPost` | Default impls | Yes |
| Mach3 | `Mach3Post` | Default impls | Yes |

---

## Viewport & Interaction

| Feature | Implementation | GUI | Status |
|---------|---------------|-----|--------|
| Orbit/Pan/Zoom camera | `OrbitCamera` | Mouse drag/scroll | **Fully functional** |
| View presets (Top/Front/Right/Iso) | `ViewPreset` | Buttons + keyboard 1-4 | **Fully functional** |
| Wireframe/Shaded toggle | `RenderMode` | Toggle button | **Fully functional** |
| Per-toolpath colors (8 palette) | `palette_color()` | Automatic by index | **Fully functional** |
| Click-to-select toolpath | `project_to_screen()` | Click in viewport | **Fully functional** |
| Toolpath isolation (I key) | `isolate_toolpath` | Press I to solo | **Fully functional** |
| Keyboard shortcuts | `handle_keyboard_shortcuts()` | Delete/G/Shift+G/Space/I/H/1-4 | **Fully functional** |
| Collision markers | Red crosses in viewport | Checkbox toggle | **Fully functional** |
| Stock wireframe | `StockGpuData` | Always shown with model | **Fully functional** |
| Ground grid | `GridGpuData` | Always shown | **Fully functional** |
| Entry point markers | `entry_marker_vertices()` | — | **Render ready, not drawn** — needs GPU upload in `upload_gpu_data()` |
| Slope angle shading | — | — | **Not implemented** — needs per-vertex color in mesh shader |

---

## Per-Toolpath Infrastructure

| Feature | State Field | GUI | Worker/Export | Status |
|---------|------------|-----|--------------|--------|
| Enable/Disable | `enabled: bool` | Context menu toggle | Filters in sim/export | **Fully functional** |
| Visibility | `visible: bool` | Context menu toggle | Filters GPU upload | **Fully functional** |
| Locked | `locked: bool` | — | Skips auto-regen | **Not surfaced** — needs lock icon + context menu item |
| Stock Source | `stock_source: StockSource` | — | — | **Not surfaced** — enum exists but no UI selector |
| Finishing passes | `finishing_passes: usize` | DragValue on Pocket/Profile | `make_depth_with_finishing()` | **Fully functional** |
| Radial/Axial stock-to-leave | Two fields on 8 configs | "Wall Stock"/"Floor Stock" | Passes axial to core | **Fully functional** |
| Skip air cuts | `skip_air_cuts: bool` | Checkbox on 3D Finish | — | **UI only** — filtering logic not in worker |
| Slope confinement | `slope_from/to` on DropCutter | DragValues | — | **UI only** — filtering logic not in worker |
| Compensation type | `CompensationType` on Profile | Dropdown | — | **UI only** — G41/G42 not emitted |
| Continuous waterline | `continuous: bool` on Waterline | Checkbox | — | **UI only** — spiral logic not in waterline.rs |

---

## Summary: What Needs Wiring

### Quick Wins (< 1 session each)
1. **Lock icon in project tree** — show 🔒 for locked ops, add context menu Lock/Unlock
2. **Stock Source selector** — add dropdown (Fresh/FromRemainingStock) to toolpath properties
3. **Entry point markers rendering** — call `entry_marker_vertices()` in `upload_gpu_data()`, draw in line pipeline
4. **Manual NC in export** — insert `pre_gcode`/`post_gcode` strings in `export_gcode()`
5. **Workholding rigidity selector** — add ComboBox (Low/Medium/High) to machine/setup panel
6. **Tool L/D ratio display** — show "L/D: X.X (-Y%)" near stickout in tool panel

### Medium Effort (1-2 sessions each)
7. **Operation presets UI** — Save/Load buttons in properties panel, dropdown of saved presets
8. **Rapid collision rendering** — call `check_rapid_collisions()` after simulation, render as red line segments
9. **Skip air cuts logic** — post-filter moves in worker where tool above mesh + threshold
10. **Slope confinement logic** — post-filter moves by surface normal angle in worker
11. **Retraction strategy dressup** — implement Minimum retract in `apply_dressups()`
12. **Vendor LUT human-readable labels** — format observation IDs nicely in feeds card

### Larger Effort (2+ sessions each)
13. **Compensation G41/G42 emission** — skip tool offset in worker when InControl, emit G41/G42/G40 in gcode.rs
14. **Continuous waterline** — Z-interpolating spiral logic in waterline.rs
15. **Stock deviation coloring** — pass model mesh to SimulationRequest, drop-cutter for deviation, shader changes
16. **Slope angle shading** — per-vertex color attribute in mesh shader, toggle in viewport overlay
17. **Custom boundary polygons** — allow user-selected boundary instead of stock bbox only
