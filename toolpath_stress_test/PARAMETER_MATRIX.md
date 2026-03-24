# Parameter Matrix

Every parameter on every operation, with its GUI control, range, default, and expected effect.

## Legend

- **Config field**: Field name in the GUI state config (`configs.rs`)
- **Core field**: Corresponding field in the core params struct
- **GUI**: Whether exposed in the GUI (`Yes` / `No` / `Partial`)
- **Default**: Hardcoded default from `impl Default`
- **Range**: GUI drag value range (min..=max)
- **Expected effect**: What should visibly change when this param moves
- **Validation method**: How to confirm the change happened (`sim` = simulation, `gcode` = G-code export, `visual` = viewport, `debug` = semantic debugger)

---

## 2.5D Operations

### Face

Source: `FaceConfig` → `FaceParams` (`face.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `stepover` | `stepover` | Yes | 5.0 mm | 0.5–100 | Distance between parallel passes changes; wider = fewer passes, more scallop | sim: measure pass spacing; gcode: count Y-offsets |
| `depth` | `depth` | Yes | 0.0 mm | 0–50 | Total material removal depth; 0 = skim at stock top | sim: final surface Z should equal -(depth) |
| `depth_per_pass` | `depth_per_pass` | Yes | 1.0 mm | 0.1–20 | Number of Z passes; depth/depth_per_pass = pass count | gcode: count distinct Z levels |
| `feed_rate` | `feed_rate` | Yes | 1500 mm/min | 1–50000 | Feed rate on cutting moves | gcode: F value on G1 |
| `plunge_rate` | `plunge_rate` | Yes | 500 mm/min | 1–10000 | Feed rate on Z plunges | gcode: F value on Z-only G1 |
| `stock_offset` | `stock_offset` | Yes | 5.0 mm | 0–50 | Extra travel beyond stock edge each side; 0 = no overshoot | visual: passes extend past stock boundary |
| `direction` | `direction` | Yes | Zigzag | OneWay, Zigzag | OneWay = all passes same direction (rapid return); Zigzag = alternating | gcode: alternating X direction between passes |

### Pocket

Source: `PocketConfig` → `PocketParams` / `ZigzagParams` (`pocket.rs` / `zigzag.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `pattern` | (selects pocket vs zigzag gen) | Yes | Contour | Contour, Zigzag | Contour = concentric offsets; Zigzag = parallel raster lines | visual: pattern shape changes entirely |
| `stepover` | `stepover` | Yes | 2.0 mm | 0.05–50 | Pass spacing; lower = more passes, better finish | sim: count concentric loops or raster lines |
| `depth` | `cut_depth` | Yes | 3.0 mm | 0.1–100 | Total pocket depth | sim: final floor Z |
| `depth_per_pass` | (via DepthStepping) | Yes | 1.5 mm | 0.1–50 | Max Z per pass | gcode: count distinct Z levels |
| `feed_rate` | `feed_rate` | Yes | 1000 mm/min | 1–50000 | Cutting feed rate | gcode: F on lateral G1 |
| `plunge_rate` | `plunge_rate` | Yes | 500 mm/min | 1–10000 | Plunge feed rate | gcode: F on Z-only G1 |
| `climb` | `climb` | Yes | true | bool | CW (climb) vs CCW (conventional) tool path direction | visual: loop direction reverses |
| `angle` | `angle` (zigzag only) | Yes (when Zigzag) | 0.0 deg | 0–360 | Raster angle for zigzag pattern | visual: line orientation rotates |
| `finishing_passes` | (via DepthStepping) | Yes | 0 | 0–10 | Extra spring passes at final depth | gcode: repeated Z-level passes at bottom |

### Profile

Source: `ProfileConfig` → `ProfileParams` (`profile.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `side` | `side` | Yes | Outside | Outside, Inside | Tool center offset direction relative to boundary | visual: toolpath flips to other side of contour |
| `depth` | `cut_depth` | Yes | 6.0 mm | 0.1–100 | Total profile depth | sim: cut depth |
| `depth_per_pass` | (via DepthStepping) | Yes | 2.0 mm | 0.1–50 | Max Z per pass | gcode: Z level count |
| `feed_rate` | `feed_rate` | Yes | 1000 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 500 mm/min | 1–10000 | Plunge feed rate | gcode: F value |
| `climb` | `climb` | Yes | true | bool | CW vs CCW | visual: direction reversal |
| `tab_count` | (via Tab dressup) | Yes | 0 | 0–20 | Number of holding tabs evenly spaced | visual: raised bridges in profile |
| `tab_width` | (via Tab.width) | Yes | 6.0 mm | 1–50 | Width of each tab | sim: tab width measurement |
| `tab_height` | (via Tab.height) | Yes | 2.0 mm | 0.5–20 | Height tabs rise above cut floor | sim: tab height measurement |
| `finishing_passes` | (via DepthStepping) | Yes | 0 | 0–10 | Spring passes at final depth | gcode: repeated Z passes |
| `compensation` | (selects comp mode) | Yes | InComputer | InComputer, InControl | InComputer = CAM offsets tool; InControl = emits G41/G42 | gcode: presence of G41/G42 |

**Known partial**: `InControl` compensation is in the GUI but `G41`/`G42` is NOT emitted in export.

### Adaptive (2D)

Source: `AdaptiveConfig` → `AdaptiveParams` (`adaptive.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `stepover` | `stepover` | Yes | 2.0 mm | 0.05–50 | Max radial engagement (WOC); controls spiral spacing | debug: engagement trace should stay ≤ stepover |
| `depth` | `cut_depth` | Yes | 6.0 mm | 0.1–100 | Total depth | sim: cut floor Z |
| `depth_per_pass` | (via DepthStepping) | Yes | 2.0 mm | 0.1–50 | Z per pass | gcode: Z level count |
| `feed_rate` | `feed_rate` | Yes | 1500 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 500 mm/min | 1–10000 | Plunge feed rate | gcode: F value |
| `tolerance` | `tolerance` | Yes | 0.1 mm | 0.01–1 | Path simplification tolerance; lower = more points, smoother | gcode: point count changes |
| `slot_clearing` | `slot_clearing` | Yes | true | bool | Pre-cut center slot before spiral | visual: center slot appears/disappears |
| `min_cutting_radius` | `min_cutting_radius` | Yes | 0.0 mm | 0–50 | Blend sharp corners with arcs of this min radius; 0 = disabled | visual: sharp inside corners become rounded |

### VCarve

Source: `VCarveConfig` → `VCarveParams` (`vcarve.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `max_depth` | `max_depth` | Yes | 5.0 mm | 0.1–50 | Maximum depth limit; 0 = unlimited | sim: no point deeper than max_depth |
| `stepover` | `stepover` | Yes | 0.5 mm | 0.01–10 | Scan line spacing | visual: density of engraving passes |
| `feed_rate` | `feed_rate` | Yes | 800 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 400 mm/min | 1–10000 | Plunge feed rate | gcode: F value |
| `tolerance` | `tolerance` | Yes | 0.05 mm | 0.01–1 | Sampling interval along scan lines | gcode: point density |

**Note**: Tool half-angle is derived from the V-bit tool geometry, not a separate parameter.

### Rest Machining

Source: `RestConfig` → `RestParams` (`rest.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `prev_tool_id` | `prev_tool_radius` | Yes | None | tool list | Previous larger tool; determines rest region geometry | visual: only cuts where prev tool couldn't reach |
| `stepover` | `stepover` | Yes | 1.0 mm | 0.05–50 | Scan line spacing | visual: pass density |
| `depth` | `cut_depth` | Yes | 6.0 mm | 0.1–100 | Cut depth | sim: floor Z |
| `depth_per_pass` | (via DepthStepping) | Yes | 2.0 mm | 0.1–50 | Z per pass | gcode: Z level count |
| `feed_rate` | `feed_rate` | Yes | 1000 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 500 mm/min | 1–10000 | Plunge feed rate | gcode: F value |
| `angle` | `angle` | Yes | 0.0 deg | 0–360 | Scan line angle | visual: line orientation |

### Inlay

Source: `InlayConfig` → `InlayParams` (`inlay.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `pocket_depth` | `pocket_depth` | Yes | 3.0 mm | 0.1–50 | Female pocket depth | sim: pocket floor Z |
| `glue_gap` | `glue_gap` | Yes | 0.1 mm | 0–2 | Gap between mating surfaces | visual: slight offset between male/female |
| `flat_depth` | `flat_depth` | Yes | 0.5 mm | 0–20 | Extra depth below surface for male plug | sim: male cut extends below surface |
| `boundary_offset` | `boundary_offset` | Yes | 0.0 mm | 0–10 | Margin around plug boundary | visual: plug boundary expands |
| `stepover` | `stepover` | Yes | 1.0 mm | 0.05–50 | Scan line spacing for V-carve and flat clearing | visual: pass density |
| `flat_tool_radius` | `flat_tool_radius` | Yes | 3.175 mm | 0.1–50 | Tool radius for flat area clearing; 0 = skip | visual: flat clearing passes appear/disappear |
| `feed_rate` | `feed_rate` | Yes | 800 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 400 mm/min | 1–10000 | Plunge feed rate | gcode: F value |
| `tolerance` | `tolerance` | Yes | 0.05 mm | 0.01–1 | Sampling tolerance | gcode: point density |

**Note**: Tool half-angle derived from V-bit geometry.

### Zigzag

Source: `ZigzagConfig` → `ZigzagParams` (`zigzag.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `stepover` | `stepover` | Yes | 2.0 mm | 0.05–50 | Raster line spacing | visual: line density |
| `depth` | `cut_depth` | Yes | 3.0 mm | 0.1–100 | Cut depth | sim: floor Z |
| `depth_per_pass` | (via DepthStepping) | Yes | 1.5 mm | 0.1–50 | Z per pass | gcode: Z count |
| `feed_rate` | `feed_rate` | Yes | 1000 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 500 mm/min | 1–10000 | Plunge feed rate | gcode: F value |
| `angle` | `angle` | Yes | 0.0 deg | 0–360 | Raster angle | visual: line orientation |

### Trace

Source: `TraceConfig` → `TraceParams` (`trace.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `compensation` | `compensation` | Yes | None | None, Left, Right | Offset tool left/right of path by tool_radius | visual: path shifts to one side |
| `depth` | `depth` | Yes | 1.0 mm | 0.1–50 | Cut depth | sim: trace depth |
| `depth_per_pass` | `depth_per_pass` | Yes | 0.5 mm | 0.1–20 | Z per pass | gcode: Z count |
| `feed_rate` | `feed_rate` | Yes | 800 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 400 mm/min | 1–10000 | Plunge feed rate | gcode: F value |

### Drill

Source: `DrillConfig` → `DrillParams` (`drill.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `depth` | `depth` | Yes | 10.0 mm | 0.5–100 | Total drill depth | gcode: final Z |
| `cycle` | `cycle` | Yes | Peck | Simple, Dwell, Peck, ChipBreak | Drill cycle type; affects retract behavior | gcode: G81/G82/G83/G73 |
| `peck_depth` | `Peck(f64)` / `ChipBreak(f64,_)` | Yes (conditional) | 3.0 mm | 0.5–50 | Depth per peck | gcode: peck increments |
| `dwell_time` | `Dwell(f64)` | Yes (conditional) | 0.5 s | 0.1–10 | Dwell time at bottom | gcode: P value |
| `retract_amount` | `ChipBreak(_,f64)` | Yes (conditional) | 0.5 mm | 0.1–5 | Chip break retract distance | gcode: retract amount |
| `feed_rate` | `feed_rate` | Yes | 300 mm/min | 1–5000 | Plunge feed rate | gcode: F value |
| `retract_z` | `retract_z` | Yes | 2.0 mm | 0.5–50 | R-plane for rapid→feed transition | gcode: R value |

### Chamfer

Source: `ChamferConfig` → `ChamferParams` (`chamfer.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `chamfer_width` | `chamfer_width` | Yes | 1.0 mm | 0.1–10 | Width of chamfer on workpiece face | visual: chamfer size |
| `tip_offset` | `tip_offset` | Yes | 0.1 mm | 0–2 | Distance from V-bit tip to prevent tip wear | gcode: Z offset from surface |
| `feed_rate` | `feed_rate` | Yes | 800 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 400 mm/min | 1–10000 | Plunge feed rate | gcode: F value |

**Note**: `tool_half_angle` and `tool_radius` derived from V-bit tool geometry.

---

## 3D Operations

### 3D Finish (Drop Cutter)

Source: `DropCutterConfig` → `DropCutterGrid` + raster generation (`dropcutter.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `stepover` | `y_step` | Yes | 1.0 mm | 0.05–50 | Raster line spacing | visual: line density on surface |
| `feed_rate` | `feed_rate` | Yes | 1000 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 500 mm/min | 1–10000 | Plunge feed rate | gcode: F value |
| `min_z` | (bounds check) | Yes | -50.0 mm | -500–0 | Floor below which tool won't descend | sim: no cuts below min_z |
| `skip_air_cuts` | (air cut filter) | **No** | false | bool | Skip raster lines entirely in air | visual: fewer passes over flat areas |
| `slope_from` | (slope filter) | **No** | 0.0 deg | 0–90 | Only cut areas steeper than this | visual: flat areas skipped |
| `slope_to` | (slope filter) | **No** | 90.0 deg | 0–90 | Only cut areas shallower than this | visual: steep areas skipped |

**GAP**: `skip_air_cuts`, `slope_from`, `slope_to` exist in config but are NOT in the GUI.

### 3D Rough (Adaptive 3D)

Source: `Adaptive3dConfig` → `Adaptive3dParams` (`adaptive3d.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `stepover` | `stepover` | Yes | 2.0 mm | 0.05–50 | Max radial engagement | debug: engagement trace |
| `depth_per_pass` | `depth_per_pass` | Yes | 3.0 mm | 0.1–50 | Z per level | gcode: Z level spacing |
| `stock_to_leave_axial` | `stock_to_leave` | Yes | 0.5 mm | 0–10 | Material left on surface (axial) | sim: residual above mesh |
| `stock_to_leave_radial` | (added to tool radius) | Yes | 0.5 mm | 0–10 | Material left on walls (radial) | sim: residual on walls |
| `stock_top_z` | `stock_top_z` | Yes | 30.0 mm | -100–200 | Top of uncut material | visual: first Z level starts here |
| `feed_rate` | `feed_rate` | Yes | 1500 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 500 mm/min | 1–10000 | Plunge feed rate | gcode: F value |
| `tolerance` | `tolerance` | Yes | 0.1 mm | 0.01–1 | Path simplification tolerance | gcode: point count |
| `min_cutting_radius` | `min_cutting_radius` | Yes | 0.0 mm | 0–50 | Min corner radius to prevent chatter | visual: sharp corners become arcs |
| `entry_style` | `entry_style` | Yes | Plunge | Plunge, Helix, Ramp | How tool enters material at each pocket | visual/gcode: entry motion changes |
| `fine_stepdown` | `fine_stepdown` | Yes | 0.0 mm | 0–10 | Insert intermediate Z levels; 0 = disabled | gcode: extra Z levels appear |
| `detect_flat_areas` | `detect_flat_areas` | Yes | false | bool | Auto-insert Z levels at mesh shelf heights | gcode: Z levels align with flat areas |
| `region_ordering` | `region_ordering` | Yes | Global | Global, ByArea | Global = all pockets per Z; ByArea = each pocket fully then next | debug: ordering changes in trace |
| `clearing_strategy` | `clearing_strategy` | Yes | ContourParallel | ContourParallel, Adaptive | Contour = EDT offset; Adaptive = curvature-adjusted | debug: strategy trace differs |
| `z_blend` | `z_blend` | Yes | false | bool | Blend Z toward terrain across contour offsets (relief/terrain mode) | sim: outer contours stay near z_level, inner ones descend |

### Waterline

Source: `WaterlineConfig` → `WaterlineParams` (`waterline.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `z_step` | (Z level spacing) | Yes | 1.0 mm | 0.05–20 | Spacing between horizontal slices | visual: number of contour rings |
| `sampling` | `sampling` | Yes | 0.5 mm | 0.1–5 | Fiber sampling spacing | visual: contour smoothness |
| `start_z` | (upper bound) | Yes | 0.0 mm | -200–200 | Top Z to start from | visual: first contour level |
| `final_z` | (lower bound) | Yes | -25.0 mm | -200–200 | Bottom Z to stop at | visual: last contour level |
| `feed_rate` | `feed_rate` | Yes | 1000 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 500 mm/min | 1–10000 | Plunge feed rate | gcode: F value |
| `continuous` | (spiral linking) | Yes | false | bool | Link contours into continuous spiral | visual: fewer retracts between levels |

### Pencil Finish

Source: `PencilConfig` → `PencilParams` (`pencil.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `bitangency_angle` | `bitangency_angle` | Yes | 160.0 deg | 90–180 | Dihedral angle threshold; lower = more creases detected | visual: more/fewer crease paths |
| `min_cut_length` | `min_cut_length` | Yes | 2.0 mm | 0.5–50 | Discard chains shorter than this | visual: short fragments disappear |
| `hookup_distance` | `hookup_distance` | Yes | 5.0 mm | 0.5–50 | Max gap for linking chain endpoints | visual: chains merge across small gaps |
| `num_offset_passes` | `num_offset_passes` | Yes | 1 | 0–10 | Parallel passes on each side of crease | visual: wider cleaning band |
| `offset_stepover` | `offset_stepover` | Yes | 0.5 mm | 0.05–10 | Spacing between offset passes | visual: offset pass density |
| `sampling` | `sampling` | Yes | 0.5 mm | 0.1–5 | Point spacing along paths | gcode: point density |
| `feed_rate` | `feed_rate` | Yes | 800 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 400 mm/min | 1–10000 | Plunge feed rate | gcode: F value |
| `stock_to_leave_axial` | `stock_to_leave` | Yes | 0.0 mm | 0–10 | Axial stock to leave | sim: residual height |
| `stock_to_leave_radial` | (not passed to core) | **No** | 0.0 mm | — | Radial stock to leave (config exists, not in GUI) | — |

### Scallop Finish

Source: `ScallopConfig` → `ScallopParams` (`scallop.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `scallop_height` | `scallop_height` | Yes | 0.1 mm | 0.01–2 | Target scallop height; lower = more passes, better finish | visual: pass density changes |
| `tolerance` | `tolerance` | Yes | 0.05 mm | 0.01–1 | Path simplification tolerance | gcode: point count |
| `direction` | `direction` | Yes | OutsideIn | OutsideIn, InsideOut | Contouring starts from boundary or center | visual: pass order reverses |
| `continuous` | `continuous` | Yes | false | bool | Connect contours into spiral | visual: fewer retracts |
| `slope_from` | `slope_from` | Yes | 0.0 deg | 0–90 | Only machine slopes steeper than this | visual: flat areas skipped |
| `slope_to` | `slope_to` | Yes | 90.0 deg | 0–90 | Only machine slopes shallower than this | visual: steep areas skipped |
| `feed_rate` | `feed_rate` | Yes | 1000 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 500 mm/min | 1–10000 | Plunge feed rate | gcode: F value |
| `stock_to_leave_axial` | `stock_to_leave` | Yes | 0.0 mm | 0–10 | Axial stock to leave | sim: residual |
| `stock_to_leave_radial` | (not passed to core) | **No** | 0.0 mm | — | Radial stock to leave (config exists, not in GUI) | — |

### Steep/Shallow

Source: `SteepShallowConfig` → `SteepShallowParams` (`steep_shallow.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `threshold_angle` | `threshold_angle` | Yes | 45.0 deg | 10–80 | Angle dividing steep/shallow regions | visual: boundary between strategies shifts |
| `overlap_distance` | `overlap_distance` | Yes | 1.0 mm | 0–10 | Both strategies extend into each other's region | visual: overlap zone visible |
| `wall_clearance` | `wall_clearance` | Yes | 0.5 mm | 0–10 | Shallow passes stay this far from steep walls | visual: gap near steep regions |
| `steep_first` | `steep_first` | Yes | true | bool | Machine steep regions before shallow | debug: ordering in trace |
| `stepover` | `stepover` | Yes | 1.0 mm | 0.05–50 | Parallel pass spacing for shallow regions | visual: shallow pass density |
| `z_step` | `z_step` | Yes | 1.0 mm | 0.05–20 | Z step for waterline in steep regions | visual: steep contour count |
| `feed_rate` | `feed_rate` | Yes | 1000 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 500 mm/min | 1–10000 | Plunge feed rate | gcode: F value |
| `sampling` | `sampling` | Yes | 0.5 mm | 0.1–5 | Fiber sampling spacing for contours | visual: contour smoothness |
| `stock_to_leave_axial` | `stock_to_leave` | Yes | 0.0 mm | 0–10 | Axial stock to leave | sim: residual |
| `stock_to_leave_radial` | (not passed to core) | **No** | 0.0 mm | — | Radial stock to leave (config, not in GUI) | — |
| `tolerance` | `tolerance` | Yes | 0.05 mm | 0.01–1 | Path simplification tolerance | gcode: point count |

### Ramp Finish

Source: `RampFinishConfig` → `RampFinishParams` (`ramp_finish.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `max_stepdown` | `max_stepdown` | Yes | 0.5 mm | 0.05–10 | Max Z drop per revolution | debug: Z change per circuit |
| `slope_from` | `slope_from` | Yes | 30.0 deg | 0–90 | Only machine slopes steeper than this | visual: flat areas skipped |
| `slope_to` | `slope_to` | Yes | 90.0 deg | 0–90 | Only machine slopes shallower than this | visual: steep areas skipped |
| `direction` | `direction` | Yes | Climb | Climb, Conventional, BothWays | Cut direction | visual: path direction |
| `order_bottom_up` | `order_bottom_up` | Yes | false | bool | Bottom-to-top instead of top-to-bottom | debug: Z ordering in trace |
| `feed_rate` | `feed_rate` | Yes | 1000 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 500 mm/min | 1–10000 | Plunge feed rate | gcode: F value |
| `sampling` | `sampling` | Yes | 0.5 mm | 0.1–5 | Fiber sampling spacing | visual: contour smoothness |
| `stock_to_leave_axial` | `stock_to_leave` | Yes | 0.0 mm | 0–10 | Axial stock to leave | sim: residual |
| `stock_to_leave_radial` | (not passed to core) | **No** | 0.0 mm | — | Radial stock to leave (config, not in GUI) | — |
| `tolerance` | `tolerance` | Yes | 0.05 mm | 0.01–1 | Path simplification tolerance | gcode: point count |

### Spiral Finish

Source: `SpiralFinishConfig` → `SpiralFinishParams` (`spiral_finish.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `stepover` | `stepover` | Yes | 1.0 mm | 0.05–20 | Radial distance between spiral revolutions | visual: spiral density |
| `direction` | `direction` | Yes | InsideOut | InsideOut, OutsideIn | Spiral starts from center or rim | visual: direction reverses |
| `feed_rate` | `feed_rate` | Yes | 1000 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 500 mm/min | 1–10000 | Plunge feed rate | gcode: F value |
| `stock_to_leave_axial` | `stock_to_leave` | Yes | 0.0 mm | 0–10 | Axial stock to leave | sim: residual |
| `stock_to_leave_radial` | (not passed to core) | **No** | 0.0 mm | — | Radial stock to leave (config, not in GUI) | — |

### Radial Finish

Source: `RadialFinishConfig` → `RadialFinishParams` (`radial_finish.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `angular_step` | `angular_step` | Yes | 5.0 deg | 1–90 | Degrees between spokes | visual: number of radial lines |
| `point_spacing` | `point_spacing` | Yes | 0.5 mm | 0.1–5 | Sample spacing along each spoke | gcode: point density |
| `feed_rate` | `feed_rate` | Yes | 1000 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 500 mm/min | 1–10000 | Plunge feed rate | gcode: F value |
| `stock_to_leave_axial` | `stock_to_leave` | Yes | 0.0 mm | 0–10 | Axial stock to leave | sim: residual |
| `stock_to_leave_radial` | (not passed to core) | **No** | 0.0 mm | — | Radial stock to leave (config, not in GUI) | — |

### Horizontal Finish

Source: `HorizontalFinishConfig` → `HorizontalFinishParams` (`horizontal_finish.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `angle_threshold` | `angle_threshold` | Yes | 5.0 deg | 1–30 | Max slope to consider "flat" | visual: which areas are machined changes |
| `stepover` | `stepover` | Yes | 1.0 mm | 0.05–20 | Raster line spacing | visual: pass density |
| `feed_rate` | `feed_rate` | Yes | 1000 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 500 mm/min | 1–10000 | Plunge feed rate | gcode: F value |
| `stock_to_leave_axial` | `stock_to_leave` | Yes | 0.0 mm | 0–10 | Axial stock to leave | sim: residual |
| `stock_to_leave_radial` | (not passed to core) | **No** | 0.0 mm | — | Radial stock to leave (config, not in GUI) | — |

### Project Curve

Source: `ProjectCurveConfig` → `ProjectCurveParams` (`project_curve.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `depth` | `depth` | Yes | 1.0 mm | 0.1–20 | Cut depth below projected surface | sim: groove depth |
| `point_spacing` | `point_spacing` | Yes | 0.5 mm | 0.1–5 | Resample spacing along curve | gcode: point density |
| `feed_rate` | `feed_rate` | Yes | 800 mm/min | 1–50000 | Cutting feed rate | gcode: F value |
| `plunge_rate` | `plunge_rate` | Yes | 400 mm/min | 1–10000 | Plunge feed rate | gcode: F value |

### Alignment Pin Drill

Source: `AlignmentPinDrillConfig` → `DrillParams` (`drill.rs`)

| Config field | Core field | GUI | Default | Range | Expected effect | Validation |
|---|---|---|---|---|---|---|
| `spoilboard_penetration` | (adds to stock depth) | Yes | 2.0 mm | 0.5–20 | How deep into spoilboard below stock | gcode: total Z depth |
| `cycle` | `cycle` | Yes | Peck | Simple, Dwell, Peck, ChipBreak | Drill cycle type | gcode: G81/G82/G83/G73 |
| `peck_depth` | `Peck(f64)` | Yes (conditional) | 3.0 mm | 0.5–50 | Peck depth | gcode: peck increments |
| `feed_rate` | `feed_rate` | Yes | 300 mm/min | 1–5000 | Plunge feed rate | gcode: F value |
| `retract_z` | `retract_z` | Yes | 2.0 mm | 0.5–50 | R-plane height | gcode: R value |
| `holes` | (XY positions) | No (auto from pins) | [] | — | Hole positions from alignment pin config | visual: hole locations |
