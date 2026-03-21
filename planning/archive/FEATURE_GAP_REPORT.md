# rs_cam vs Fusion 360 CAM — Feature Gap Report

## Methodology

Compared rs_cam's implemented feature set (PROGRESS.md, 14 operations + GUI) against Autodesk Fusion 360 Manufacturing workspace features documented in:
- `research/fusion360_cam_reference.docx` — Complete feature reference (30K words)
- `research/fusion360_ui_reference.docx` — Every UI panel, dialog, and element (30K words)
- `research/08_ux_terminology.md` — UX terminology mapping
- `reference/shapeoko_feeds_and_speeds/` — Fusion API, parameter maps, strategy docs
- `ramp_docs.md`, `scallop_reference.md`, `steep_shallow_docs.md` — Autodesk reference

---

## What rs_cam Already Has (Parity or Better)

| Fusion Feature | rs_cam Equivalent |
|---|---|
| 2D Pocket (contour/zigzag) | Pocket (contour + zigzag patterns, island support) |
| 2D Contour/Profile | Profile (inside/outside, climb/conventional, tabs) |
| 2D Adaptive Clearing | Adaptive (constant engagement, slot clearing, boundary cleanup, link-vs-retract) |
| 3D Adaptive Roughing | Adaptive3d (heightmap tracking, multi-level, region ordering, flat area detection) |
| 3D Parallel (raster finish) | DropCutter (batch raster with all 5 tool types) |
| 3D Contour/Waterline | Waterline (fiber + push-cutter + marching squares) |
| Pencil Finishing | Pencil (dihedral angle analysis, concave edge chaining, offset passes) |
| Scallop Finishing | Scallop (constant-offset, inside-out/outside-in, continuous mode) |
| Steep/Shallow Composite | SteepShallow (threshold angle, overlap, wall clearance) |
| Ramp Finishing | RampFinish (ramping Z passes, slope confinement, climb/conventional/both) |
| VCarve/Engrave | VCarve (scan-line, variable Z, max depth clamp) |
| Rest Machining | Rest (geometric tool comparison, masked zigzag) |
| Inlay Operations | Inlay (female pocket + male plug with glue gap) |
| Ramp/Helix Entry | Dressup: ramp entry, helix entry |
| Tabs/Bridges | Dressup: tab/bridge with even spacing |
| Lead-in/Lead-out | Dressup: quarter-circle arc entry/exit |
| Dogbone Fillets | Dressup: inside corner overcuts |
| Arc Fitting / Smoothing | Dressup: biarc + Kasa least-squares (G2/G3 output) |
| Link-vs-Retract / Keep Tool Down | Dressup: keep tool down between nearby passes |
| Feed Rate Optimization | Dressup: RCTF chip thinning, engagement estimation |
| Material Removal Simulation | Heightmap sim with wood-tone mesh, tool model, checkpoints, scrubber |
| Tool Holder Collision Detection | Holder/shank check, interpolated path, multi-segment holders |
| 5 Tool Types | Flat, Ball, BullNose, VBit, TaperedBall |
| 3 Post-Processors | GRBL, LinuxCNC, Mach3 |
| STL/SVG/DXF Import | All three supported |
| TOML Job Files | Multi-tool, multi-operation, save/load |
| Depth Stepping | Even distribution, finish allowance |
| Undo/Redo | Stock, post, tool, toolpath params |
| Project Tree with Status Icons | Colored circles, palette swatches, context menus |
| Toolpath Reordering | Drag up/down, duplicate, enable/disable |
| Per-Toolpath Colors | 8-color palette with Z-depth blend |
| Viewport Click-to-Select | Screen-space picking |
| Keyboard Shortcuts | Delete, G, Shift+G, Space, I, H, 1-4 |
| Simulation Playback Controls | Play/pause, scrubber, speed, per-toolpath progress |
| Cancel Compute | AtomicBool cancellation |
| Normal Flip Warning | Winding consistency check on STL import |
| Stock Wireframe | Bounding box overlay in viewport |

---

## Feature Gaps: What Fusion Has That rs_cam Doesn't

### TIER 1 — Quick Wins (1-2 sessions, high UX impact)

| # | Feature | What Fusion Does | Difficulty | Implementation Notes |
|---|---------|-----------------|------------|---------------------|
| 1 | **Face/Surfacing Operation** | Levels stock top with wide passes (~80% tool diameter stepover). First op in every job. | **Low** (1 session) | A Zigzag at Z=0 with stock-width boundary and large stepover. Dedicated UI entry with smart defaults. |
| 2 | **Trace/Follow Path** | Follows SVG/DXF path at a specified depth. For engraving, fluting, decorative routing. | **Low** (1 session) | Convert polygon exterior directly to toolpath moves at fixed Z. Simpler than profile (no tool radius offset). |
| 3 | **Separate Radial vs Axial Stock-to-Leave** | Two independent values: wall stock (radial) and floor stock (axial). | **Low** (1 session) | Split single `stock_to_leave` into `stock_to_leave_radial` + `stock_to_leave_axial` on all ops. |
| 4 | **Finishing Passes / Spring Passes** | Repeat the final pass N times at the same depth for dimensional accuracy (deflection recovery). | **Low** (1 session) | For Profile/Contour: repeat the last depth level N times. `finishing_passes: usize` param. |
| 5 | **High Feedrate Mode** | Convert G0 rapids to G1 at high feed rate for machines with unpredictable rapid behavior (GRBL "dogleg"). | **Low** (1 session) | PostProcessor option: replace G0 with G1 F{high_feed}. One flag + one value. |
| 6 | **Even Stepdown Exposure** | UI toggle for even vs constant depth distribution (already implemented in core). | **Low** (< 1 session) | `DepthStepping` already supports `DepthDistribution::Even`. Just add a checkbox in properties panel. |
| 7 | **Operation Presets / Templates** | Save and recall named parameter sets ("Hardwood Roughing", "MDF Pocket"). | **Low** (1 session) | Serialize OperationConfig + DressupConfig to named TOML files. Load/save in properties panel. |
| 8 | **Flute Count on Tools** | Needed for feed-per-tooth calculations. Fusion stores it per tool. | **Low** (< 1 session) | Add `flute_count: u32` to ToolConfig. Display in tool properties. |
| 9 | **Tool Stickout Warning** | Warn when L:D ratio is high (deflection risk). | **Low** (< 1 session) | Yellow banner when `stickout / diameter > 4`. Already have stickout field. |

### TIER 2 — Medium Effort, High Impact (2-4 sessions each)

| # | Feature | What Fusion Does | Difficulty | Implementation Notes |
|---|---------|-----------------|------------|---------------------|
| 10 | **Feeds & Speeds Calculator** | Material + tool → recommended RPM, feed, stepover, stepdown. Linked calculations (change one, others adjust). | **Medium** (3 sessions) | Use vendor LUT data in `reference/shapeoko_feeds_and_speeds/`. Core: `F = fz × flutes × RPM`. Chip thinning, depth-tier derating. Biggest UX win for hobbyists. |
| 11 | **Drill Operation** | Peck drilling (G83), dwell (G82), spot drill (G81). Dedicated canned cycles. | **Medium** (2 sessions) | New operation type: point list + peck_depth + dwell_time. G-code: G81/G82/G83 with R-plane. No XY motion. |
| 12 | **Machine Profile / Limits** | Define max RPM, axis feed rates, acceleration. Clamp computed values to safe limits. | **Medium** (2 sessions) | `MachineProfile` struct with spindle range, per-axis max rates. Validate feed/plunge/rapid. Map to GRBL `$110-$122`. |
| 13 | **Heights System** | 5 distinct height references: Clearance, Retract, Feed, Top, Bottom. Each with configurable reference + offset. | **Medium** (2 sessions) | Currently just `safe_z`. Add `HeightsConfig { clearance_z, retract_z, feed_z, top_z, bottom_z }`. Visual height planes in viewport. |
| 14 | **Machining Boundary / Containment** | Restrict toolpaths to selected region. 3 containment modes: tool center / inside / outside boundary. | **Medium** (3 sessions) | `MachiningBoundary` polygon. Ops clip toolpath to boundary. Containment offsets boundary by ±tool_radius. |
| 15 | **Retraction Strategy** | Full retract (safe Z), Minimum retract (just above stock), Direct (straight line). | **Medium** (2 sessions) | Currently always full retract. `RetractStrategy` enum. Minimum retract needs stock height. Direct needs collision check. |
| 16 | **Chamfer Operation** | V-bit edge break along selected edges. Depth from chamfer width + tool angle. | **Medium** (2 sessions) | Profile variant with V-bit depth control. `chamfer_width` → `Z = -width / tan(half_angle)`. Tip offset for tool life. |
| 17 | **Compensation Type** | In Computer / In Control / Wear / Inverse Wear. Controls whether tool offset is in the CAM or on the CNC controller (G41/G42). | **Medium** (2 sessions) | "In Computer" = current behavior. "In Control" emits G41/G42 with tool number. Wear = offset register. |
| 18 | **Fusion Tool Library Import** | Import `.tools` / `.json` Fusion tool files. JSON schema is documented in `reference/`. | **Medium** (2 sessions) | Parse Fusion JSON: `DC` (diameter), `OAL` (overall length), `LCF` (flute length), `RE` (corner radius), `NOF` (flutes). Map to ToolConfig. |
| 19 | **Entry Position Selection** | Click in viewport to set where on the boundary the tool enters. | **Medium** (2 sessions) | Add entry_point to toolpath config. Use viewport click to set XY. Rearrange boundary start point to nearest vertex. |
| 20 | **Contact-Only Toolpath** | Skip moves where tool doesn't touch the part (skip air cutting over gaps). | **Medium** (2 sessions) | Filter moves where CLPoint `contacted == false` (already in core). Add checkbox "Skip air cuts" on 3D ops. |

### TIER 3 — Larger Features (4+ sessions, high differentiation)

| # | Feature | What Fusion Does | Difficulty | Implementation Notes |
|---|---------|-----------------|------------|---------------------|
| 21 | **Slot Operation** | Optimized narrow slot cutting where tool width ≈ slot width. Controlled entry, roughing + finishing passes. | **Medium-High** (3 sessions) | Specialized zigzag/ramp pattern for slots. Auto-detects narrow features. Full-radial engagement management. |
| 22 | **Spiral Finishing** | Continuous spiral from center outward (or reverse). No step-between-pass transitions. For domes/convex. | **Medium** (3 sessions) | Archimedean spiral with stepover control. Eliminate retracts. Good for bowls and domes. |
| 23 | **Radial Finishing** | Spoked passes radiating from a center point. For circular/rotational features. | **Medium** (2 sessions) | Lines from center at angular intervals, Z from drop-cutter. Simple geometry. |
| 24 | **Horizontal (Flat Area) Finishing** | Detect and finish only horizontal/flat areas. Cleanup after contour/parallel. | **Medium** (3 sessions) | Surface normal analysis → flat region detection → targeted raster within flat regions. |
| 25 | **Project (Curve-on-Surface)** | Project 2D curves onto 3D mesh surface, then follow them as toolpath. For 3D engraving. | **High** (4 sessions) | Ray-cast 2D curve points onto mesh. Needs XY→Z mapping via drop-cutter. Useful for 3D surface engraving. |
| 26 | **Continuous Spiral Waterline** | Replace closed contour passes with continuous Z-interpolating spiral. Eliminates seams. | **Medium** (3 sessions) | Interpolate Z between contour levels. Scallop already has `continuous` flag — extend to Waterline. |
| 27 | **TSP Rapid Optimization** | Reorder toolpath segments to minimize total rapid travel distance. | **Medium** (2 sessions) | Nearest-neighbor + 2-opt on segment endpoints. Dressup pass after generation. Measurable cycle time savings. |
| 28 | **Setup Sheet / Job Documentation** | Auto-generate HTML/PDF with part views, tool list, operation details, cycle times. | **Medium** (3 sessions) | Generate HTML from JobState. Tool table, operation table, stock dims, estimated times. Export as HTML. |
| 29 | **Slope Confinement (generalized)** | Restrict any 3D op to a slope angle range. | **Medium** (2 sessions) | Already on Scallop/RampFinish. Generalize to all 3D ops: filter drop-cutter points by surface normal angle. |
| 30 | **Automatic Strategy Selection** | Analyze mesh → auto-pick op types (steep walls → waterline, shallow → raster, corners → pencil). | **High** (4-5 sessions) | Surface normal classification, region segmentation, strategy assignment. SteepShallow already does half of this. |
| 31 | **Trochoidal Milling** | Circular patterns for slot cutting with controlled engagement. | **High** (3-4 sessions) | Trochoidal path generator: circles along a spine curve. Alt to adaptive for narrow features. |

### TIER 4 — Advanced / Aspirational

| # | Feature | What Fusion Does | Difficulty | Implementation Notes |
|---|---------|-----------------|------------|---------------------|
| 32 | **Morphed Spiral** | Spiral morphs between two boundaries. Variable stepover for complex geometry. | **Very High** (5+ sessions) | Needs boundary interpolation + spiral generation. Premium surface finish for mold cavities. |
| 33 | **4th Axis / Rotary** | Wrap 2D ops around cylinders. A-axis in G-code. | **Very High** (5+ sessions) | Coordinate transform XY→XA, rotary G-code output, new Toolpath IR move types. |
| 34 | **Custom/Form Tool Profiles** | User-defined arbitrary tool cross-section. | **High** (3-4 sessions) | Discretize profile into radius-height samples. Generic drop-cutter against sampled profile. |
| 35 | **Dexel Stock Model** | Ray-based stock that handles overhangs (vs heightmap Z-buffer). | **Very High** (5+ sessions) | Store (z_enter, z_exit) intervals per ray. Major infrastructure change. |
| 36 | **Full Machine Simulation** | 3D kinematic machine model with axis collision checking. | **Very High** | Needs machine kinematic definition, 3D machine models, multi-body collision. |
| 37 | **Probing / WCS Setting** | Touch-probe ops for setting work offsets and part inspection. | **High** (4+ sessions) | Probe G-code generation (G38.2), measurement routines, offset storage. |
| 38 | **G-code Backplotter** | Parse and visualize existing G-code files (not just our output). | **High** (4+ sessions) | Full G-code parser (G0/G1/G2/G3/canned cycles). Large scope. |
| 39 | **2D Part Nesting** | Pack multiple parts on stock to minimize waste. | **High** (4+ sessions) | NFP (no-fit polygon) + bin packing algorithm. |
| 40 | **Thread Milling** | Helical interpolation for internal/external threads. | **Medium** (3 sessions) | Helical G-code path at thread pitch. Need thread pitch param + major diameter. |
| 41 | **Bore Milling** | Circular interpolation for precision holes. | **Medium** (2 sessions) | Helical boring: circular G2/G3 at decreasing Z. Spring passes at final diameter. |

---

## Fusion UI Features We Don't Have

| Feature | What It Does | Difficulty | Priority |
|---------|-------------|------------|----------|
| **5-Level Heights System** | Clearance / Retract / Feed / Top / Bottom heights with visual planes in viewport | Medium (2 sessions) | High — professional users expect this |
| **Entry Position Marker** | Arrow showing where tool enters, editable by click | Low (1 session) | Medium |
| **Slope Angle Shading** | Color gradient on model showing steep vs shallow regions | Medium (2 sessions) | Medium — great for steep/shallow setup |
| **Rest Material Shading** | Colored overlay showing remaining material from prior ops | Medium (2 sessions) | Medium — helps plan multi-tool strategies |
| **Section Analysis** | Cross-section view through model and stock | Medium (2 sessions) | Low-Medium |
| **Operation Suppression** | Suppress (exclude) operations without deleting them | Low (< 1 session) | Medium — already have enable/disable |
| **Operation Locking** | Protect op from regeneration to preserve manual edits | Low (1 session) | Low |
| **Pattern Operations** | Linear/circular/mirror patterns of operations | Medium (3 sessions) | Low for wood routing |
| **Manual NC Insertion** | Insert raw G-code commands between operations | Low (1 session) | Medium — useful for tool changes, pauses |
| **S-Key Command Palette** | Searchable command palette (like VS Code Ctrl+P) | Medium (2 sessions) | Nice-to-have |
| **Marking Menu** | Radial right-click context menu | Medium (2 sessions) | Nice-to-have |
| **Colorized Stock by Deviation** | Color sim mesh by deviation from model | Medium (3 sessions) | Medium — quality verification |
| **Rapid Collision Detection** | Highlight dangerous G0 moves through stock | Medium (2 sessions) | High — safety feature |
| **Setup Sheet (HTML/PDF)** | Auto-generated shop documentation | Medium (3 sessions) | Medium — production shops need this |

---

## Priority Recommendations

### Ship First (1-2 sessions each, immediate UX impact)
1. **Face Operation** — Every job starts with this
2. **Trace/Follow Path** — Common for decorative work
3. **Drill Operation** — Missing from the 14 ops, commonly needed
4. **Separate Radial/Axial Stock-to-Leave** — Split one param into two
5. **Finishing/Spring Passes** — Repeat last pass for accuracy
6. **High Feedrate Mode** — One PostProcessor flag
7. **Operation Presets** — Save/load parameter sets
8. **Flute Count + Tool Stickout Warning** — Two quick tool improvements

### Build Next (high-value medium effort)
9. **Feeds & Speeds Calculator** — #1 hobbyist pain point, reference data already in repo
10. **Heights System** — Professional users expect 5-level height control
11. **Machine Limits Profile** — Prevents impossible feed rates
12. **Fusion Tool Library Import** — Leverage existing tool definitions
13. **Machining Boundary / Containment** — Restrict toolpath region
14. **Retraction Strategy** — Full/minimum/direct retract selection

### Differentiation (stretch goals)
15. **Setup Sheet Generation** — HTML output for shop floor
16. **Automatic Strategy Selection** — No hobbyist tool does this well
17. **TSP Rapid Optimization** — Measurable cycle time savings
18. **Rapid Collision Detection** — Safety feature
19. **Continuous Spiral Waterline** — Professional-quality finish

---

## Summary Statistics

| Category | Fusion 360 | rs_cam | Gap Count |
|----------|-----------|--------|-----------|
| 2D Operations | 11 (Face, Adaptive, Pocket, Contour, Slot, Trace, Thread, Bore, Circular, Chamfer, Engrave) | 7 | 4 missing (Face, Trace, Chamfer, Slot) |
| 3D Operations | 12+ (Adaptive, Pocket, Parallel, Contour, Ramp, Pencil, Scallop, Spiral, Radial, Morphed Spiral, Project, Horizontal, Steep/Shallow, Morph/Flow/Blend) | 7 | 5-7 missing (Spiral, Radial, Horizontal, Project, Morphed) |
| Drilling | 12 canned cycles (G81-G87, tapping, reaming, boring, thread milling) | 0 | All missing |
| Tool types | 16 (incl. face mill, slot mill, lollipop, thread mill, spot drill, tap, reamer, boring bar, countersink, form tool) | 5 | 11 missing (mostly irrelevant for wood) |
| Post-processors | 100+ (community) | 3 | Adequate for target audience |
| Dressups/linking | 10+ | 9 | Close to parity |
| Stock-to-leave | Radial + Axial (independent) | Single value | Easy fix |
| Feeds/speeds | Built-in linked calculator | Manual entry only | **Major gap** |
| Machine limits | Full kinematic model | None | Medium gap |
| Heights | 5-level system with visual planes | Single safe_z | Medium gap |
| Boundary control | 3 containment modes + slope confinement | None (generalized) | Medium gap |
| Simulation | Full machine kinematic + deviation mapping | Heightmap + tool model | Adequate for 3-axis |
| Documentation | Setup sheet (HTML/PDF) + NC editor | None | Medium gap |
| Import formats | STEP, OBJ, 3MF, F3D, STL, SVG, .tools, .json | STL, SVG, DXF | Low priority (STL covers wood routing) |

**Overall: rs_cam covers ~80% of Fusion 360's 3-axis milling algorithms and ~65% of its workflow/UI features.** The algorithmic gaps are mostly niche finishing strategies (Spiral, Radial, Morphed). The workflow gaps (feeds/speeds calculator, heights system, machine limits, setup sheets) are where hobbyist users would feel the most pain.
