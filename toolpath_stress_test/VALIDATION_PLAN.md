# Validation Plan

How to verify each parameter change does what we expect, using simulation, G-code inspection, semantic debugger, and visual checks.

---

## Validation Tools Available

| Tool | What it checks | How to use |
|------|---------------|------------|
| **Tri-dexel simulation** | Material removal shape, depth, residual stock | Run sim, inspect mesh at checkpoints; compare before/after parameter change |
| **Semantic debugger** | Engagement traces, pass ordering, strategy attribution, timing | Open Simulation workspace → semantic tree; look for trace entries per parameter |
| **G-code export** | Feed rates, Z levels, G-code commands (G0/G1/G2/G3/G41/G73/G81/G82/G83) | Export G-code, diff two exports with different parameter values |
| **Viewport visual** | Toolpath shape, pass count, direction, coverage area | Toggle between parameter values and observe toolpath rendering |
| **Move count / stats** | Total moves, cutting distance, rapid distance | Check ToolpathStats after generation |
| **CLI batch mode** | Automated parameter sweeps without GUI | `cargo run -p rs_cam_cli -- <op> --param=value` for supported ops |

---

## Per-Parameter Validation Procedures

### Template

For each parameter test:
1. **Baseline**: Generate toolpath with default parameters
2. **Change**: Modify ONE parameter (increase or decrease)
3. **Regenerate**: Re-run the operation
4. **Check**: Verify the expected effect occurred using the specified method
5. **Negative check**: Verify nothing else changed unexpectedly

---

## 2.5D Operations

### Face

| Parameter | Test change | Expected result | Check method | Negative check |
|-----------|------------|-----------------|--------------|----------------|
| stepover | 5→10 mm | Half as many passes; wider ridges visible in sim | sim: count passes; gcode: count Y-offset lines | Depth unchanged |
| stepover | 5→2 mm | 2.5× more passes; smoother surface | sim: count passes | Feed rate unchanged |
| depth | 0→3 mm | Surface drops 3mm; multiple Z passes appear | sim: final surface Z = -3 | Stepover unchanged |
| depth_per_pass | 1→0.5 mm | 2× more Z levels for same depth | gcode: count Z levels | Total depth unchanged |
| stock_offset | 5→0 mm | Passes stop at stock edge (no overshoot) | visual: compare pass extents to stock boundary | |
| stock_offset | 5→20 mm | Passes extend well beyond stock | visual: overshoot visible | |
| direction | Zigzag→OneWay | All passes go same X direction; rapid returns visible | gcode: X direction constant; G0 returns | |
| feed_rate | 1500→3000 | F values double on cutting moves | gcode: F value on G1 | Z levels unchanged |
| plunge_rate | 500→250 | F values halve on Z-only moves | gcode: F on Z plunge | Lateral F unchanged |

### Pocket

| Parameter | Test change | Expected result | Check method | Negative check |
|-----------|------------|-----------------|--------------|----------------|
| pattern | Contour→Zigzag | Pattern changes from spiraling inward to parallel raster | visual: toolpath shape changes completely | |
| stepover | 2→4 mm | Half as many loops (contour) or lines (zigzag) | visual: pass count; sim: wall ridges | |
| depth | 3→6 mm | Pocket twice as deep | sim: floor Z = -6 | |
| depth_per_pass | 1.5→0.5 mm | 3× more Z levels for 3mm depth | gcode: Z level count = 6 instead of 2 | |
| climb | true→false | Loop direction reverses (CW→CCW or vice versa) | visual: direction arrows; gcode: coordinate progression | |
| angle (zigzag) | 0→45 | Raster lines rotate 45° | visual: line angle changes | |
| finishing_passes | 0→2 | 2 extra passes at final depth | gcode: repeated Z-level passes at bottom | |

### Profile

| Parameter | Test change | Expected result | Check method | Negative check |
|-----------|------------|-----------------|--------------|----------------|
| side | Outside→Inside | Tool jumps to other side of contour | visual: path moves inward | Depth unchanged |
| tab_count | 0→4 | 4 raised bridges appear in profile cut | visual: tabs visible; sim: material left at tabs | |
| tab_width | 6→12 mm | Tabs twice as wide | sim: measure tab width | Tab count unchanged |
| tab_height | 2→1 mm | Tabs half as tall | sim: tab Z relative to floor | Tab width unchanged |
| compensation | InComputer→InControl | Path should be on-line (no offset) with G41/G42 in gcode | gcode: check for G41/G42 | **KNOWN ISSUE: G41/G42 not emitted** |
| finishing_passes | 0→2 | 2 spring passes at final depth | gcode: repeated bottom passes | |

### Adaptive (2D)

| Parameter | Test change | Expected result | Check method | Negative check |
|-----------|------------|-----------------|--------------|----------------|
| stepover | 2→1 mm | Tighter spiral, more passes, better floor quality | debug: engagement trace stays ≤ 1mm; sim: smoother floor | |
| tolerance | 0.1→0.01 | More points, smoother path curves | gcode: point count increases significantly | Engagement unchanged |
| slot_clearing | true→false | No center slot; spiral starts from boundary | visual: center slot disappears | |
| min_cutting_radius | 0→2 mm | Sharp inside corners become 2mm arcs | visual: corner blending visible | |

### VCarve

| Parameter | Test change | Expected result | Check method | Negative check |
|-----------|------------|-----------------|--------------|----------------|
| max_depth | 5→2 mm | Shallow carve; wide features clipped at 2mm | sim: no point deeper than 2mm | |
| max_depth | 5→0 mm | Unlimited depth; narrow features cut very deep | sim: depths match geometry | |
| stepover | 0.5→0.2 mm | Denser scan lines; smoother carve but slower | visual: line density; gcode: line count | |
| tolerance | 0.05→0.01 | More points per scan line | gcode: point count per line | |

### Rest Machining

| Parameter | Test change | Expected result | Check method | Negative check |
|-----------|------------|-----------------|--------------|----------------|
| prev_tool | Tool A→Tool B (different diameter) | Rest region changes (cuts only where prev tool couldn't reach) | visual: rest region shape changes | |
| angle | 0→90 | Scan lines rotate 90° | visual: line orientation | |

### Drill

| Parameter | Test change | Expected result | Check method | Negative check |
|-----------|------------|-----------------|--------------|----------------|
| cycle | Peck→Simple | No peck retracts; single plunge to depth | gcode: G83→G81; no intermediate retracts | |
| cycle | Peck→Dwell | Dwell at bottom instead of peck retracts | gcode: G83→G82; P value present | |
| cycle | Peck→ChipBreak | Partial retracts instead of full retracts | gcode: G83→G73 | |
| peck_depth | 3→1 mm | 3× more pecks for same depth | gcode: peck count | Total depth unchanged |
| retract_z | 2→5 mm | R-plane higher above surface | gcode: R value changes | |

### Chamfer

| Parameter | Test change | Expected result | Check method | Negative check |
|-----------|------------|-----------------|--------------|----------------|
| chamfer_width | 1→2 mm | Wider chamfer; tool offset increases | visual: chamfer width doubles | |
| tip_offset | 0.1→0.5 mm | Tool sits higher; chamfer shifted slightly | gcode: Z values shift up | |

---

## 3D Operations

### 3D Finish (Drop Cutter)

| Parameter | Test change | Expected result | Check method | Negative check |
|-----------|------------|-----------------|--------------|----------------|
| stepover | 1→0.5 mm | 2× more raster lines; better surface finish | visual: line density doubles; gcode: Y-offset count | |
| min_z | -50→-10 mm | Tool won't descend below -10mm even if mesh goes deeper | sim: no cuts below -10 | |

### Adaptive 3D

| Parameter | Test change | Expected result | Check method | Negative check |
|-----------|------------|-----------------|--------------|----------------|
| stepover | 2→1 mm | Tighter clearing loops; better floor finish | debug: engagement trace ≤ 1mm | |
| depth_per_pass | 3→1 mm | 3× more Z levels | gcode: Z level count triples | |
| stock_to_leave_axial | 0.5→0 mm | Cuts flush to mesh surface | sim: no residual above mesh | |
| stock_to_leave_radial | 0.5→0 mm | Cuts flush to mesh walls | sim: no residual on walls | |
| stock_top_z | 30→10 mm | First Z level starts at 10 instead of 30 | gcode: first Z level = 10 | |
| entry_style | Plunge→Helix | Plunge moves become helical spirals | gcode: arcs at entry; visual: helix visible | |
| entry_style | Plunge→Ramp | Plunge moves become angled ramps | gcode: simultaneous XZ at entry | |
| fine_stepdown | 0→0.5 mm | Extra intermediate Z levels inserted | gcode: more Z levels than depth_per_pass alone | |
| detect_flat_areas | false→true | Z levels align with mesh shelves/plateaus | gcode: Z levels snap to flat area heights | |
| region_ordering | Global→ByArea | Each pocket cleared fully before next | debug: ordering trace changes | |
| clearing_strategy | ContourParallel→Adaptive | Curvature-adjusted offsets instead of uniform | debug: strategy trace differs | |
| z_blend | false→true | Outer contours at z_level, inner contours descend toward surface | sim: terrain-following Z visible | |

### Waterline

| Parameter | Test change | Expected result | Check method | Negative check |
|-----------|------------|-----------------|--------------|----------------|
| z_step | 1→0.5 mm | 2× more contour rings | visual: ring count doubles | |
| sampling | 0.5→0.2 mm | Smoother contours (more fiber intersections) | visual: contour smoothness; gcode: point count | |
| start_z | 0→-5 mm | Skip top 5mm of model | visual: no contours above -5 | |
| final_z | -25→-10 mm | Stop 10mm above model bottom | visual: no contours below -10 | |
| continuous | false→true | Contours linked into spiral (fewer retracts) | gcode: retract count drops dramatically | |

### Pencil Finish

| Parameter | Test change | Expected result | Check method | Negative check |
|-----------|------------|-----------------|--------------|----------------|
| bitangency_angle | 160→120 | More creases detected (broader definition of "crease") | visual: more paths appear | |
| bitangency_angle | 160→175 | Almost no creases detected | visual: most paths disappear | |
| min_cut_length | 2→10 mm | Short fragments removed | visual: fewer, longer paths | |
| hookup_distance | 5→20 mm | More chains linked across gaps | visual: fewer disconnected segments | |
| num_offset_passes | 1→3 | Wider cleaning band around each crease | visual: thicker paths | |
| offset_stepover | 0.5→1.0 mm | Offset passes spaced farther apart | visual: visible gaps between offsets | |

### Scallop Finish

| Parameter | Test change | Expected result | Check method | Negative check |
|-----------|------------|-----------------|--------------|----------------|
| scallop_height | 0.1→0.05 mm | 2× more contour rings (finer finish) | visual: ring density increases; gcode: line count | |
| scallop_height | 0.1→0.5 mm | Fewer rings (coarser, faster) | visual: ring density decreases | |
| direction | OutsideIn→InsideOut | Contouring starts from center | visual: toolpath origin moves | |
| continuous | false→true | Contours linked into spiral | gcode: retract count drops | |
| slope_from | 0→30 | Skip areas shallower than 30° | visual: flat areas not machined | |
| slope_to | 90→60 | Skip areas steeper than 60° | visual: steep areas not machined | |

### Steep/Shallow

| Parameter | Test change | Expected result | Check method | Negative check |
|-----------|------------|-----------------|--------------|----------------|
| threshold_angle | 45→30 | More area classified as "steep" | visual: steep/shallow boundary moves | |
| threshold_angle | 45→60 | More area classified as "shallow" | visual: boundary moves other way | |
| overlap_distance | 1→5 mm | Wider overlap zone between strategies | visual: overlap band widens | |
| steep_first | true→false | Shallow areas machined before steep | debug: ordering in semantic trace | |

### Ramp Finish

| Parameter | Test change | Expected result | Check method | Negative check |
|-----------|------------|-----------------|--------------|----------------|
| max_stepdown | 0.5→0.2 mm | More revolutions per terrace (shallower descent) | gcode: more G1 moves per Z change | |
| direction | Climb→Conventional | Path direction reverses | visual: direction arrows change | |
| direction | Climb→BothWays | Alternating directions | visual: alternating arrows | |
| order_bottom_up | false→true | Start from deepest terrace, work up | debug: Z ordering reverses in trace | |

---

## Shared System Validation

### Heights

| Test | Procedure | Expected | Check |
|------|-----------|----------|-------|
| Auto heights update on stock change | Change stock Z dimensions → regenerate | All Auto heights shift proportionally | gcode: Z values change |
| Manual height overrides Auto | Set clearance_z to Manual(50) → regenerate | Clearance stays at 50 regardless of stock | gcode: G0 Z50 |
| FromReference works | Set top_z to FromReference(ModelTop, -2) | Top Z = model_top - 2mm | gcode: first cut Z |
| bottom_z controls depth | Set bottom_z Manual(-10) on a pocket with depth=3 | Pocket cuts to -10 not -3 | sim: floor Z = -10 |

### Dressups

| Test | Procedure | Expected | Check |
|------|-----------|----------|-------|
| Ramp entry | Enable Ramp, angle=3° on pocket | Plunges replaced with angled ramps | gcode: no vertical G1 Z; simultaneous XZ moves |
| Helix entry | Enable Helix, radius=3, pitch=1 on pocket | Plunges replaced with helical spirals | gcode: G2/G3 with Z change at entry |
| Dogbones | Enable on pocket, angle=90° | Inside corners get overcuts | visual: corner extensions; gcode: extensions past corners |
| Lead-in/out | Enable on profile, radius=2 | Arc approach/departure at cut start/end | gcode: G2/G3 at start/end of each pass |
| Link moves | Enable, max_distance=10, on pocket | Short retracts become direct feeds | gcode: fewer G0 Z moves; replaced with G1 |
| Arc fitting | Enable, tolerance=0.05, on any op | Linear segments become G2/G3 arcs | gcode: G2/G3 present; file size smaller |
| Feed optimization | Enable on pocket (fresh stock) | F values vary along cuts based on engagement | gcode: multiple different F values within one pass |
| Retract strategy | Full→Minimum | Retracts go to nearby Z + 2mm instead of retract_z | gcode: variable retract Z values (lower than retract_z) |
| Rapid order | Enable TSP optimization | Disconnected segments reordered to minimize rapids | visual: rapid travel distance shrinks |

### Stock Awareness

| Test | Procedure | Expected | Check |
|------|-----------|----------|-------|
| FromRemainingStock | Create pocket, then profile with FromRemainingStock | Profile only cuts where pocket left material | sim: compare with fresh — air cuts eliminated |
| Clip to Stock | Enable on any op | Toolpath truncated at stock boundary | visual: no path outside stock |
| Containment Inside | Enable clip + Inside containment | Tool edge stays inside boundary | visual: path shrinks inward by tool radius |
| Containment Outside | Enable clip + Outside containment | Tool edge stays outside boundary | visual: path extends outward by tool radius |

---

## Automated Sweep Strategy

For the next session, we can use this approach to systematically verify parameters:

### Per-operation sweep

For each operation:
1. Generate baseline toolpath with defaults and a known test model
2. For each parameter:
   a. Set to minimum of range → regenerate → capture stats + gcode
   b. Set to maximum of range → regenerate → capture stats + gcode
   c. Compare: verify the expected metric changed (move count, Z levels, F values, etc.)
   d. Verify no unexpected side effects

### Recommended test models

| Model type | Best for testing |
|-----------|-----------------|
| Simple rectangle SVG (50×50mm) | 2.5D ops: pocket, profile, adaptive, zigzag, face, trace |
| Circle SVG (30mm Ø) | Profile tabs, lead-in/out, adaptive spiral |
| Text SVG ("CAM") | VCarve, inlay, chamfer, rest machining |
| Hemisphere STL | 3D finish, waterline, pencil, scallop, steep/shallow |
| Terrain STL (irregular surface) | Adaptive3D z_blend, horizontal finish, ramp finish |
| Multi-level step STL | Adaptive3D detect_flat_areas, fine_stepdown |
| Bowl STL (concave) | Spiral finish, radial finish, scallop direction |

### Diagnostic output to capture per test

1. `ToolpathStats` (move_count, cutting_distance, rapid_distance)
2. G-code line count
3. Distinct Z levels in G-code
4. Min/max F values in G-code
5. Semantic debugger trace (if available for that op)
6. Simulation mesh checksum (detect geometry changes)
