# R03 source census — support agent report (v1, received by message 2026-09-09 16:36 +12)

Read-only Claude Code `Explore` subagent `r03-census`. Delivered in two messages; saved
verbatim below by the orchestrating session `rs-cam-b9`. **Leads, not accepted findings**:
the orchestrator spot-checked the items marked ✔ in `../../R03/REPORT.md` §8.

---

**Evidence label: CODE throughout.** Every claim cites a line I read. Nothing is a live usability observation. Paths are repo-relative to /home/ricky/personal_repos/rs_cam. I made no edits, ran no cargo command, started no process.

# 1. All-operation census

Source: `crates/rs_cam_core/src/compute/catalog.rs`. The enum is generated from the `for_each_op!` X-macro at lines 139-171. Metadata comes from `static REG_*: OpRegistryEntry` blocks at lines 1913-2419.

**Variant count: 24.** 11 in `ALL_2D` (lines 259-271), 12 in `ALL_3D` (lines 273-286), 1 `SystemOnly` in neither menu. The brief's "23" counts the two menus only and excludes `AlignmentPinDrill`.

Columns: variant | GUI label | description quoted verbatim | geometry | tool precondition | primary params | GUI form file | auto-regen

1. `Face` | "Face" | "Level the stock top surface" | Stock | none (ANY_TOOL) | stepover, depth, depth_per_pass, stock_offset, direction | boundary_2d.rs:11 | true
2. `Pocket` | "Pocket" | "Clear material inside a closed region" | Polygons | none | stepover, depth, depth_per_pass, climb, pattern, angle, finishing_passes | boundary_2d.rs:63 | true
3. `Profile` | "Profile" | "Cut along the outside or inside of a boundary" | Polygons | none | side, depth, depth_per_pass, climb, tab_count/width/height, finishing_passes, compensation | boundary_2d.rs:123 | true
4. `Adaptive` | "Adaptive" | "Constant-engagement rough clearing" | Polygons | none | stepover, depth, depth_per_pass, tolerance, slot_clearing, min_cutting_radius, cleanup_strategy, engagement_measure, path_strategy | boundary_2d.rs:219 | true
5. `VCarve` | "VCarve" | "V-bit engraving with variable depth" | Polygons | **V-bit only** (`required_kinds: &[CutterKind::VBit]`, catalog.rs:2005-2008) | max_depth, stepover, tolerance | boundary_2d.rs:298 | true
6. `Rest` | "Rest Machining" | "Clean up areas a larger tool couldn't reach" | Polygons | none | prev_tool_id, stepover, depth, depth_per_pass, angle | boundary_2d.rs:340 | true
7. `Inlay` | "Inlay" | "V-bit pocket and plug for inlay work" | Polygons | **V-bit only** (catalog.rs:2046) | pocket_depth, glue_gap, flat_depth, boundary_offset, stepover, flat_tool_radius, tolerance | boundary_2d.rs:392 | true
8. `Zigzag` | "Zigzag" | "Back-and-forth raster clearing at an angle" | Polygons | none | stepover, depth, depth_per_pass, angle | boundary_2d.rs:458 | true
9. `Trace` | "Trace" | "Follow a path exactly for engraving or scoring" | Polygons | none | depth, depth_per_pass, compensation | engrave.rs:7 | true
10. `Drill` | "Drill" | "Drill holes from SVG circle positions" | Polygons | none | depth, cycle, peck_depth, dwell_time, retract_amount, retract_z | drill.rs:89 | true
11. `Chamfer` | "Chamfer" | "Bevel edges with a V-bit" | Polygons | **V-bit only** (catalog.rs:2127) | chamfer_width, tip_offset | engrave.rs:44 | true
12. `DropCutter` | "3D Finish" | "Parallel raster passes following the surface" | Mesh | none | stepover, min_z, slope_from, slope_to | surface_3d.rs:18 | false
13. `Adaptive3d` | "3D Rough" | "Load-limiting rough mill on a 3D surface" | Mesh | none | stepover, depth_per_pass, stock_to_leave_radial/axial, tolerance, entry_style, clearing_strategy, region_ordering, +14 more | surface_3d.rs:50 | false
14. `Waterline` | "Waterline" | "Horizontal contours at constant Z levels" | Mesh | none | z_step, sampling, continuous | surface_3d.rs:323 | false
15. `Pencil` | "Pencil Finish" | "Trace concave edges and creases on the surface" | Mesh | **none in the registry** (catalog.rs:2218 ANY_TOOL) but see F3 below | bitangency_angle, min_cut_length, hookup_distance, num_offset_passes, offset_stepover, detector, stock_to_leave, +11 more | surface_3d.rs:344 | false
16. `Scallop` | "Scallop Finish" | "Variable stepover for constant scallop height" | Mesh | **Ball or tapered ball** (`&[CutterKind::Ball, CutterKind::TaperedBall]`, `supports_v_bit: false`, catalog.rs:2237-2240) | scallop_height, tolerance, direction, continuous, slope_from/to, stock_to_leave, intra_pass_hookup_mm, iso_field | surface_3d.rs:617 | false
17. `UnifiedFinish` | "Unified Finish" | "Bands the surface by true-surface slope and runs waterline/scallop/raster per band" | Mesh | **Ball or tapered ball** (catalog.rs:2264-2267) | steep/waterline thresholds, overlap, scallop_height, raster_stepover, z_step, stock_to_leave, claims_reference, monotone_cell_decomposition, +10 more | surface_3d.rs:870 | false
18. `SteepShallow` | "Steep/Shallow" | "Waterline on steep areas, raster on shallow" | Mesh | none | threshold_angle, overlap_distance, wall_clearance, steep_first, stepover, z_step, stock_to_leave | surface_3d.rs:962 | false
19. `RampFinish` | "Ramp Finish" | "Continuous Z descent along contours, no retract" | Mesh | none | max_stepdown, slope_from/to, direction, order_bottom_up, stock_to_leave | finishing.rs:10 | false
20. `SpiralFinish` | "Spiral Finish" | "Archimedean spiral passes over the surface" | Mesh | none | stepover, direction, stock_to_leave | finishing.rs:78 | false
21. `RadialFinish` | "Radial Finish" | "Spoke-pattern passes radiating from center" | Mesh | none | angular_step, point_spacing, stock_to_leave | finishing.rs:127 | false
22. `HorizontalFinish` | "Horizontal Finish" | "Finish only flat areas of the surface" | Mesh | none | angle_threshold, stepover, stock_to_leave | finishing.rs:165 | false
23. `ProjectCurve` | "Project Curve" | "Project 2D curves onto a 3D mesh surface" | **Both** | none | depth, point_spacing, surface_model_id, direction, side, chain_distance_mm | project.rs:8 | false
24. `AlignmentPinDrill` | "Pin Drill" | "Drill alignment pin holes through stock" | Stock | none | holes, spoilboard_penetration, cycle, peck_depth, retract_z | drill.rs:182 | true

Form files are under `crates/rs_cam_viz/src/ui/properties/operations/`. Every operation has a dedicated form function; there is no fallback renderer. Feed rate, plunge rate and spindle RPM exist on every operation and are omitted from the params column.

**No deprecated or experimental flag exists.** I grepped catalog.rs for deprecated / experimental / Legacy. The only hit is the string `"enum:Legacy|ResidueMop|ContourParallelNarrow|ContourParallelHybrid"` at line 1529, an `Adaptive` cleanup-strategy VALUE, not an operation flag. A `deprecated_dial` finding exists on `ToolpathStats` (compute/config.rs:291) but it reports a stale project value.

# 2. Add-operation path

Source: `crates/rs_cam_viz/src/ui/toolpath_panel.rs:649-707`.

**Grouping.** Two flat groups under one `+ Add` menu button (lines 654-667). The headings name FILE FORMATS, not machining goals. The 2.5D heading says SVG when DXF also qualifies. Order is registry order, so roughing and finishing interleave. `Pin Drill` never appears because the loop reads only the two menu lists.

**What disables an entry: geometry availability only** (lines 682-687). `has_mesh` / `has_polygons` are PROJECT-WIDE (lines 656-657), computed with `.any()` over all models. The menu does not consider which model the op will actually be assigned.

**Reason text is hover-only, never inline.** Available entry: `.on_hover_text(spec.description)`. Disabled entry: `.on_disabled_hover_text(format!("{}\n{}", spec.description, reason))` (lines 700-706). Reason strings: Polygons → "Requires 2D geometry (SVG/DXF)"; Mesh → "Requires 3D mesh (STL/STEP)"; Both → "Requires both 2D curves and 3D mesh"; Stock → "" (unreachable).

**The menu does NOT check the selected tool's tip shape.** `add_op_menu_item` takes no tool argument. Scallop and Unified Finish appear enabled with a flat end mill in the project.

**There is no search field.**

# 3. Controller defaults on add

Source: `crates/rs_cam_viz/src/controller/events/toolpath.rs:12-173`.

**Target setup** from current selection (13-40); otherwise the first setup.

**Tool: the FIRST tool, never the selected one.** Line 42: `let Some(tool_id) = self.state.session.tools().first().map(|t| t.id.0) else {`. No read of `Selection::Tool` anywhere in this handler.

**Model: the FIRST model, or id 0.** Lines 110-116. A project with an SVG at index 0 and an STL at index 1 assigns the SVG to a fresh 3D op, which then fails validation with "Selected model has no 3D mesh" (operations/mod.rs:1938).

**No tool exists:** early return, warning "Cannot add toolpath: no tools defined" (42-49).

**"Suggest refusal"** = `feeds::suggest::suggest_params` returned `Err(FeedsError)` (91-99): toast `Cannot add toolpath: {e}`. Predicate `feeds::validate_tool_for_operation` (feeds/mod.rs:824-841): fires when the feeds family is `Scallop`, or `Parallel` with a scallop target set, AND tool geometry is `Flat` or `VBit`. Display (feeds/mod.rs:801-810): `scallop requires curved tip (need ball|bull|tapered_ball; got Flat on Scallop)` — enum debug forms, not the GUI label.

**A second early return** guards geometry (103-110): `Cannot add <label> toolpath: import geometry first`.

**Boundary init** (138-152): 3D op + any mesh → enabled, `ModelSilhouette`; else default (disabled, `Stock`). `boundary_inherit` hard-coded `true` at line 153.

**Heights init** = `HeightsConfig::default()` = five `Auto` rows (compute/config.rs:1568-1578). **Feeds** written by `suggest_params`; `SuggestContext::default()` with a `TODO(v1.2)` (82-85). **Naming** `format!("{} {}", label, count + 1)` uses the PROJECT-WIDE count.

# 4. Ball-required refusal path

(a) Menu entry disabled? No. (b) Added in an error state? No — not added at all. (c) Generate refuses when a Scallop reaches the queue another way: `compute/execute.rs:2146-2153` → `Invalid tool: Scallop requires a ball-tip tool (Ball Nose or Tapered Ball Nose)`; Unified Finish twin at 2237-2245. (d) FOUR different sentences for one condition: the add-menu toast; the inspector ribbon Critical row "Scallop, Unified Finish, and Pencil operations require a ball nose tool for correct surface contact." (diagnostics/adapters/from_static_checks.rs:120-144); Feeds tab `Feeds unavailable: scallop requires curved tip …` (properties/mod.rs:1668-1680); queue ERR chip after Generate.

**The Generate button is NOT disabled by this condition.** `validate_toolpath` (operations/mod.rs:1840-1893) has no ball-tip arm for Scallop, UnifiedFinish or Pencil.

Four exact-membership defects: **F1** bull nose passes the add gate (feeds/mod.rs:829-831 accepts `Bull`) but the registry excludes `Bull` → refuses only at Generate. **F2** the Critical diagnostic fires only for `ToolType::EndMill` (from_static_checks.rs:121). **F3** the diagnostic over-claims for Pencil (ANY_TOOL, catalog.rs:2218). **F4** the V-bit trio has no add-time gate; VCarve with a flat end mill IS created and then blocked by the validator — V-bit ops refuse at the button; ball ops refuse at the engine.

# 5. Operation inspector structure

`properties/mod.rs:3571-4813`. Header order: Name; Generate + status word; move count; validation errors; "Show reach map" on the ten `supports_reach_map()` ops (3690-3760); diagnostics ribbon (Actionable / Stateful / `Hints (n)`); the `Geometry` DISCLOSURE (wiring group, distinct from the Geometry TAB), `default_open` when no result.

Five tabs (2984-2994): Geometry, Feeds & Speeds, Linking, Heights, Dressup. Three carry badges (Feeds, Heights, Dressup; 3026-3088); the Dressup badge is derived from Safety/Geometry/ToolLoad/Quality diagnostics so it can light up for a finding unrelated to dressups.

**Where "advanced" fields hide: almost nowhere.** Exactly ONE `collapsing` in the op forms: "Tabs" (boundary_2d.rs:178). All 26 `Adaptive3d` and 21 `UnifiedFinish` params render flat.

**Heights: per-operation, no inheritance.** Tooltips at operations/mod.rs:1893-1966. Auto resolution `HeightsConfig::resolve` (compute/config.rs:1598-1612): retract = ctx.safe_z; clearance = retract + 10; feed = retract − 2; top = stock_top; bottom = top − depth. G-HEIGHTSTAB note at operations/mod.rs:41-60.

**Tool/Input selectors** (3852-3893): plain "Tool:" / "Input:", neither filters by suitability. **Required vs first-use: no distinction in the UI**; `ParamDef.required` reaches only the MCP schema. `tooltip_for` (4900-4998) is a label-keyed ~60-entry tooltip table.

# 6. Status vocabulary

`ComputeStatus` (compute/config.rs:53-70). Card chip vs inspector word: PEND/Ready, GEN/Computing…, OK/Done, WAIT/Waiting on upstream stock (amber, hover names blocker), OFF/Disabled, ERR/Error: {msg}. `ComputeStatus::label()` is a third vocabulary. Extra badges: `MAN` (auto_regen false, hover "Manual generation — press G…"), `TRACE` (debug-trace availability differs from the other cards).

**STALE IS NOT RENDERED ANYWHERE IN THE GUI.** `stale_since` is set at ~20 sites; the only GUI read is toolpath_panel.rs:746 inside `draw_rest_badge`. For every 3D op `default_auto_regen` is false, so the flag persists indefinitely: an edited 3D op keeps showing `OK`/`Done` beside a result computed from the old value. The MCP wire publishes `"stale"` (app/mcp.rs:951, 1251). `is_stale` in workspace_bar/preflight/readiness/sim_timeline is `SimulationState::is_stale(edit_counter)`, a different project-wide quantity.

# 7. Tool authoring

Editable fields (tool.rs:111-365): Name, Type, Diameter, Cutting Length, Flutes, Helix, Corner Radius (end mill), Material, Cut Dir, type-specific angle fields, tapered "Upper shaft ⌀", Holder / Shank (collapsible: Holder Diameter, "Shank ⌀ (in collet)", Shank Length, Stickout), Catalog metadata (Vendor, Product ID). Cross-section preview from `profile_points()`. Type change rewrites geometry (145-149). "⚠ Holder / Shank — no holder, collision check skipped" on the collapsed header when holder < 0.01.

**Apply / Revert**: draft clone in `state.history.tool_draft`; `● modified` + Apply + Revert when modified, else `✓ saved`. `commit_tool_draft` (mod.rs:106-140) pushes `UndoAction::ToolChange`, writes, `invalidate_tool`, marks edited. **Navigation away AUTO-COMMITS** (`flush_tool_draft`, mod.rs:90-101) — affordance and behaviour disagree.

Library → project: two routes, one handler (`AddToolFromLibrary`, events/model.rs:77-86) — a SNAPSHOT COPY, no live link. Library dir: `RS_CAM_TOOL_DIR` → `$XDG_CONFIG_HOME/rs_cam/tools` → `$HOME/.config/rs_cam/tools` (tool_library.rs:56-78). Adding to a project cannot write the catalog. Five GUI actions DO write the real catalog immediately with no project undo (Save to library; modal Save/Delete/move/Dedupe/Delete catalog). **H1** "Save to library" writes the UNCOMMITTED draft (tool.rs:83-101). **H2** it overwrites by `dedupe_key` (type, diameter, flutes, cutting length, taper, included angle, corner radius — tool_library.rs:359-370), silently replacing an entry with a different name/holder/stickout. Reviewers must set `RS_CAM_TOOL_DIR` to a scratch dir before launching to protect the real catalog.

# Cross-cutting findings

**X1 — `boundary_inherit` is a DEAD DIAL.** No generation code reads it; session/compute.rs:1107 takes `tc.boundary.clone()` unconditionally; no "stock-level default boundary" exists. Fresh 3D ops show "Inherit from stock" ticked while generation uses the model silhouette; unticking only reveals the controls.
**X2 — menu geometry test (ANY model) vs validator test (THE assigned model) differ**; SVG-first + STL project offers every 3D op, binds the SVG, then blocks.
**X3 — Drill's description names SVG circles; the picker reads `dxf_input::DrillTarget` and prints "Targets include DXF points and circle/arc centres."**
**X4 — `deprecated_dial`, `claims_reference`, `boundary_clip_dropped` DO reach the GUI ribbon** (diagnostics/adapters/from_generation.rs:42,46,49); CLAUDE.md's coverage note is about MCP narration only.

# 8. Questions only a live test can answer

1. Does the two-group Add menu read as a FORMAT choice or a GOAL choice? 2. Is disabled-hover text discoverable? 3. Does the refused-Scallop toast connect to THE TOOL? 4. How long does an operator work against a stale `OK` chip on a 3D op? 5. Does `● modified` + Apply create an expectation that clicking away DISCARDS? 6. Does anyone find "Save to library"? 7. Where does an operator stop reading 26 flat Adaptive3d params? 8. Does the Dressup badge mislead? 9. Do PEND/Ready, GEN/Computing, OK/Done cause hesitation? 10. Bull-nose project: does a user reach Generate on a Scallop before anything flags it?
