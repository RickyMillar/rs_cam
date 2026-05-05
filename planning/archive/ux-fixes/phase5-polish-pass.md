# Phase 5: Polish Pass — Implementation Plan

Grouped by file to minimize context switching. Each finding lists the file, line range, what to change, and risk level.

---

## `app.rs`

### N1-02: Space key different semantics per workspace
**Lines 1864-1869 and 1941-1944**
In Setup/Toolpaths workspaces, Space switches to Simulation workspace (line 1865). In Simulation workspace, Space toggles play/pause (line 1941). Add a tooltip or status bar hint when the user presses Space outside Simulation showing what happened. Alternatively, unify: make Space always toggle sim playback if results exist (switch to Sim workspace first if needed), or remove the Space-to-switch behavior and require a deliberate workspace change.
**Risk: low** — keyboard shortcut semantics only; no data changes.

### N1-10: Switching to Sim with no results = empty view
**Lines 151-176 (SwitchWorkspace handler)**
The workspace switch succeeds unconditionally. The sim_op_list.rs already has an empty state (lines 19-68) but the main viewport shows nothing. Add a guard: when `SwitchWorkspace(Simulation)` fires and `!simulation.has_results()`, still allow the switch (the op list's empty state handles guidance), but consider showing a centered CentralPanel placeholder label "No simulation results yet — run simulation to begin" alongside the existing sim_op_list empty state. The sim_op_list already handles this well, so the main risk is just the blank viewport. A simple overlay in `draw_simulation_layout` before the viewport is enough.
**Risk: safe** — display-only addition.

### N4-09: Toast width uncapped
**Lines 2263-2298**
The `egui::Area` for toast notifications has no max width. Long messages (e.g. file paths in errors) stretch the toast across the screen. Wrap the toast Frame content in `ui.set_max_width(400.0)` or use `ui.label(egui::RichText::new(message).color(text_color))` inside a width-constrained layout. Add `ui.set_max_width(400.0);` as the first line inside the Frame's `show` closure (line 2293).
**Risk: safe** — cosmetic only.

### N4-11: Left panel widths differ per workspace
**Lines 1968-2103**
Setup left panel: `default_width(240.0)` (line 1971). Toolpath left panel: `default_width(230.0)` (line 2015). Simulation left panel: `default_width(200.0)` (line 2102). Standardize all three to the same default width, e.g. `240.0`. Users can still resize, but the starting point is consistent.
**Risk: safe** — default width values only.

### N5-08: Toasts not dismissible
**Lines 2263-2301**
Toasts auto-expire but cannot be clicked to dismiss. Add a small "x" button or make the toast Frame `sense(egui::Sense::click())` and filter out clicked notifications. In the loop at line 2275, after rendering each toast, check if the toast frame response was clicked and if so, mark that notification for removal. Requires adding a dismiss mechanism to the notification controller (e.g. `dismiss_notification(index)` method).
**Risk: low** — needs a small controller API addition.

### N5-10: Project load warnings lack fix guidance
**Lines 2240-2261**
Warning bullets just display the raw warning string. Append actionable hints per warning type: e.g. for missing tool references, add "Re-create tool in Tool Library"; for missing models, add "Re-import the file". This requires matching on the warning variant (or string pattern) when rendering each bullet at line 2257 and appending a suffix. Alternatively, have the warning type itself carry a `fix_hint: Option<String>` populated at load time.
**Risk: low** — display text enhancement.

### N5-12: Auto-regen no visible indicator
**Line 2317**
`process_auto_regen()` fires silently. When auto-regen submits a compute, the toolpath card already shows "Computing..." status. To improve discoverability, add a brief toast or status bar flash when auto-regen triggers. In `controller.rs:process_auto_regen()` (line 216-237), when `stale_ids` is non-empty, push a notification like "Auto-regenerating {n} toolpath(s)". Alternatively, show a small animated indicator on the workspace bar.
**Risk: safe** — notification addition only.

---

## `ui/workspace_bar.rs`

### N1-03: Toolpath tab badge bare number
**Lines 108-126**
Badge text is `format!(" {pending}")` — just a number. Prefix with a label or icon: change line 122 to `format!("{pending} pending")` or `format!("\u{23F3} {pending}")` (hourglass). This makes the badge self-explanatory.
**Risk: safe** — string format change.

### N1-15: Badges detached from tabs
**Lines 98-104**
The badge is rendered as a separate `ui.label()` after the tab button, creating visual separation. Move the badge text inside the tab button label itself: combine `label` and `badge_text` into a single `RichText` or use `ui.horizontal()` inside the button to keep them visually attached. The simplest approach is to change the `workspace_tab` function to append the badge text to the button label when present, using color-differentiated `RichText` sections via `egui::WidgetText` or a `Layout` inside the button.
**Risk: low** — layout adjustment may need sizing tweaks.

### N4-23: Workspace bar fill barely visible
**Lines 58-68, 86-96**
Active tab background `(55, 60, 80)` is very close to the default panel color. Either brighten the active tab fill (e.g. `(65, 72, 95)`) or darken the inactive area. The underline indicator at line 94 helps, but the fill should be more distinct. Adjust the RGB at line 60.
**Risk: safe** — color constant change.

---

## `ui/viewport_overlay.rs`

### N1-05: Visibility toggle labels differ across workspaces
**Lines 47-54 vs app.rs:2063-2066**
Overlay checkboxes: "Cut", "Rapid", "Col", "Fix" (lines 47-54). Sim top bar: "Paths", "Stock", "Fixtures", "Collisions" (app.rs:2063-2066). Standardize to full words everywhere: change overlay labels to "Paths", "Rapids", "Collisions", "Fixtures" to match the sim top bar. Update lines 47-54 accordingly.
**Risk: safe** — label text changes only.

### N1-07: Three naming conventions for sim actions
**Lines 78-100**
Outside Simulation workspace: "Simulate" (line 88), "Open Sim" (line 81), "Reset" (line 84). Inside Sim workspace (app.rs:2082-2086): "Re-run", "Reset". And overlay has "Check Holder" (line 95). Standardize: use "Run Simulation" / "Open Simulation" / "Reset Simulation" consistently. Change "Open Sim" to "Open Simulation", "Simulate" to "Run Simulation", and ensure the sim workspace "Re-run" becomes "Re-run Simulation" for clarity.
**Risk: safe** — button label changes.

### N4-12: Viewport overlay too dense
**Lines 17-101**
All controls are in a single `ui.horizontal()` row. When the window is narrow, items wrap awkwardly. Add `ui.spacing_mut().item_spacing.x = 4.0;` at the top to tighten spacing, and consider grouping related controls (view presets, render mode, visibility, sim actions) with `ui.separator()` already present. For narrow windows, the existing separators help. The main fix is to ensure `ui.horizontal_wrapped()` is used instead of `ui.horizontal()` at line 17 so that overflow wraps cleanly to a second row.
**Risk: safe** — layout wrapping change.

---

## `ui/setup_panel.rs`

### N1-08: Chip abbreviations "KO" unexplained
**Lines 152-191**
The chip label "KO" (line 170) is short for "Keep Out zones" but is not obvious. Change to "Keep Out" or at minimum add a hover tooltip. Change `"KO"` on line 170 to `"KeepOut"` and add `.on_hover_text("Keep-out zones")` to the chip widget. The `chip()` function would need to return a `Response` for the tooltip, or the tooltip can be added to the enclosing widget.
**Risk: safe** — label string change.

### N1-09: "Fresh stock" warning ambiguous
**Lines 203-211**
Text "Toolpaths use fresh stock" could mean the stock is newly created or that each setup starts from uncut stock. Clarify: change to "Starts from uncut stock (multi-setup sim not yet linked)" or "Previous setups not reflected — stock starts fresh". This makes the limitation explicit.
**Risk: safe** — warning text change.

### N1-14: Redundant boolean guard on Add Setup
**Lines 53-58 (setup_panel.rs)**
Guard `state.job.setups.len() > 1 || !state.job.setups.is_empty()` is always true when `setups` is non-empty, and the `> 1` branch is already covered by `!is_empty()`. This simplifies to `!state.job.setups.is_empty()`. However, the intent may be "always show add button when any setup exists" — in which case the condition should just be removed entirely (always show the button).
**Risk: safe** — logic simplification.

---

## `ui/properties/tool.rs`

### N2-04: Flute count shows float noise during drag
**Lines 54-64**
Flute count is stored as `u32` but edited via `DragValue` on a temporary `f64`. During dragging, intermediate float values like `2.7` show. Fix: use an `i32` temporary (as done for `finishing_passes` elsewhere) instead of `f64`. Change line 54 from `let mut flutes_f = tool.flute_count as f64` to `let mut flutes_i = tool.flute_count as i32` and use `egui::DragValue::new(&mut flutes_i).range(1..=8)`. Remove the `.speed(0.1)` or set it to `1.0` so it steps in whole numbers.
**Risk: safe** — type change on local variable.

### N2-17: Holder section collapsed, 0.0 silent
**Lines 139-182**
Holder parameters default to 0.0, and the section is collapsed. Users who never expand it get no holder collision checking. Add a warning below the collapsing header if `holder_diameter == 0.0 && stickout == 0.0`: show a dim italic label "Holder not configured — collision check will be skipped". Place it after line 182 (after the collapsing block).
**Risk: safe** — informational label.

### N2-13: Feed rate drag speed too slow
**`draw_feed_params` in operations.rs lines 8-15**
Feed rate `DragValue` uses `speed(10.0)` for a range of 1..50000. At speed 10, dragging across typical 100px covers only 1000 mm/min. Increase to `speed(50.0)` for feed_rate. Plunge rate is fine at `speed(10.0)` given its smaller typical range.
**Risk: safe** — drag sensitivity tuning.

---

## `ui/properties/operations.rs`

### N2-08: depth_per_pass can exceed total depth
**Lines 36-44 (pocket), 83-90 (profile), 350-358 (adaptive3d)**
`depth_per_pass` has a fixed range `0.1..=50.0` with no validation against the operation's `depth` field. Add a post-edit clamp: after the DragValue, if `cfg.depth_per_pass > cfg.depth`, set `cfg.depth_per_pass = cfg.depth`. Or change the DragValue range upper bound dynamically to `cfg.depth`. The simplest is a clamp after line 44: `cfg.depth_per_pass = cfg.depth_per_pass.min(cfg.depth);`. Apply to all operations that have both fields (Pocket, Profile).
**Risk: low** — adds a runtime clamp; may surprise users who set depth_per_pass first.

### N2-09: Slope From > Slope To = empty toolpath
**Lines 325-342**
DropCutter has `slope_from` and `slope_to` with independent ranges `0.0..=90.0`. When `slope_from > slope_to`, the engine produces an empty toolpath silently. Add validation: in `validate_toolpath` (line 2586), add a `OperationConfig::DropCutter(c)` arm that checks `if c.slope_from > c.slope_to { errs.push("Slope From must be <= Slope To") }`. Also consider a visual hint in the UI: after the slope_to DragValue, show a warning label if the constraint is violated.
**Risk: low** — validation addition.

### N2-12: stepover validation missing for most ops
**Lines 2586-2648**
Only `Pocket` and `Adaptive` check `stepover >= tool_diameter`. Many other operations use stepover (DropCutter, Adaptive3d, Waterline, Zigzag, Scallop, SteepShallow, etc.) but have no stepover validation. Add stepover checks for all stepover-bearing operations in the `validate_toolpath` match arms. Create a helper `validate_stepover(stepover, tool_diameter, errs)` and call it for each relevant variant.
**Risk: low** — validation expansion; may surface new errors for existing projects.

### N2-20: stepover == tool_diameter blocked
**Lines 2598-2606**
Condition is `>=` (greater-or-equal), meaning exactly `tool_diameter` is rejected. For some operations (like DropCutter raster), 100% stepover is valid. Change the check to `>` (strictly greater) or make it operation-specific. For Pocket/Adaptive clearing, `>=` is correct (overlap required). For raster-based 3D ops, `>` may be appropriate. Review per-operation.
**Risk: low** — semantic change in validation; needs per-op consideration.

### N2-14: "Stock to Leave" vs "Wall Stock" ambiguous
**Lines 361-374 (Adaptive3D)**
Adaptive3D uses both "Stock to Leave:" (axial, line 361) and "Wall Stock:" (radial, line 369). Other operations use only "Stock to Leave:". Clarify: rename to "Axial Stock to Leave:" and "Radial Stock to Leave:" (or "Floor Stock:" and "Wall Stock:"). Add hover tooltips via the `tooltip_for()` system in `properties/mod.rs` explaining the difference.
**Risk: safe** — label and tooltip changes.

### N2-16: Finishing passes lacks `.speed()`
**Lines 55-61 (pocket), 131-141 (profile)**
The finishing passes `DragValue` uses `egui::DragValue::new(&mut fp).range(0..=10)` with no `.speed()` call. The default drag speed for integers is fine, but it is inconsistent with other DragValues. Explicitly add `.speed(0.1)` to match the integer-step behavior (since `fp` is `i32`, this makes dragging require deliberate movement to change by 1).
**Risk: safe** — drag speed tuning.

### N2-18: Drill feed tooltip wrong context
**Lines 2223-2260**
Drill uses `draw_feed_params` which applies the generic "Feed Rate" tooltip. For drilling, the feed rate is actually the plunge/peck rate — the tooltip should say "Drilling feed rate (vertical plunge speed)" not the generic milling tooltip. Override by adding drill-specific tooltips. Either pass an optional tooltip override to `draw_feed_params` or add a manual tooltip after the dv call for drill.
**Risk: safe** — tooltip text change.

### N2-19: "High Feedrate Mode" label unclear
**`ui/properties/post.rs` lines 41-57**
Label "High Feedrate Mode (G0->G1)" is jargon. Rename to "Safe Rapids Mode" or "Convert Rapids to Feed Moves" with the existing tooltip expanded. The current tooltip is adequate but the checkbox label itself should be clearer: change line 41 to `"Safe Rapids (G0 -> G1 at feed speed)"`.
**Risk: safe** — label text change.

---

## `ui/properties/mod.rs`

### N5-13: Compute errors truncated
**Lines 1720-1724**
Error display is `format!("Error: {e}")` in a single label. Long error strings are cut off by panel width. Wrap the error in `ui.horizontal_wrapped()` or show it in a tooltip: keep a short prefix visible and put full text in `on_hover_text(e)`. Change to: show `"Error"` as the colored label, then `resp.on_hover_text(e)` with the full message.
**Risk: safe** — display-only.

---

## `ui/sim_diagnostics.rs`

### N3-06: Resolution "(re-run to apply)" even when unchanged
**Lines 96-119**
The "(re-run to apply)" hint shows whenever `!sim.auto_resolution`, even if the resolution value matches what was used in the last sim run. Track the last-used resolution (e.g. `sim.last_run_resolution: Option<f64>`) and only show the hint when `sim.resolution != sim.last_run_resolution.unwrap_or(sim.resolution)`. Set `last_run_resolution` when simulation starts.
**Risk: low** — requires adding a field to SimulationState.

### N3-07: Semantic Context dense
**Lines 124-217**
The Semantic Context section packs label, pinned badge, Start/End buttons, kind, move range, XY bbox, Z range, runtime, and params into a single collapsing header. Break it into sub-sections: (1) header row: label + pinned badge + Start/End buttons; (2) spatial info (moves, XY, Z) in a compact grid; (3) runtime on its own line; (4) params grid. Use `ui.add_space(2.0)` between groups for visual breathing room. The content is already laid out, just add spacing.
**Risk: safe** — spacing/layout adjustment.

### N3-08: Cutting Metrics pipe-delimited
**Lines 287-362**
Metrics are displayed as prose: `"Cut {x}s | rapid {y}s"`, `"Air {x}s | low engage {y}s"`, `"Avg engagement {x}% | avg MRR {y}"`. Replace with a 2-column grid for readability. Use `egui::Grid` with label/value pairs: "Cutting time" / "12.3s", "Rapid time" / "4.5s", etc. This also allows consistent alignment.
**Risk: safe** — layout change.

### N3-15: Summary stats lacks unit labels
**Lines 505-560**
"Cutting dist:" shows `"{:.0} mm"` (has units), but "Operations:" shows just a number. "Est. cycle time:" shows `"{}:{:02}"` without "min:sec" label. Add "min" suffix to cycle time display: `format!("{}:{:02} min", total_min, total_sec)`. Operations count is fine as-is (dimensionless).
**Risk: safe** — format string change.

---

## `ui/sim_timeline.rs`

### N3-09: Debug Trace exposes file paths
**Lines 1303-1316, 1332-1340**
The trace drawer shows raw filesystem paths (`path.display().to_string()`) for debug trace artifacts and cut trace artifacts. These are implementation details that leak to users. Gate these behind `sim.debug.enabled` (they already are — the trace drawer is only shown when debug is on, line 26). However, even in debug mode, shorten to just the filename: `path.file_name().unwrap_or_default().to_string_lossy()`. Keep full path in a tooltip via `.on_hover_text(path.display().to_string())`.
**Risk: safe** — display change within debug-only panel.

### N3-14: Per-op jump buttons ambiguous
**sim_op_list.rs lines 219-246**
Jump buttons use `"|<"` and `">|"` which are ASCII approximations. The transport bar (sim_timeline.rs lines 113-143) uses Unicode symbols `"|◄"` and `"►|"`. Standardize: use the same Unicode glyphs in both places. Change sim_op_list.rs line 221 from `"|<"` to `"|◄"` and line 241 from `">|"` to `"►|"`.
**Risk: safe** — string constant change.

### N4-21: Jump buttons Unicode vs ASCII
**sim_timeline.rs lines 113-143 vs sim_op_list.rs lines 219-246**
Same as N3-14. The transport bar uses Unicode (`|◄`, `◄`, `▶`, `►|`) while per-op buttons use ASCII (`|<`, `>|`). Unify to Unicode everywhere.
**Risk: safe** — same fix as N3-14.

### N3-16: Semantic auto-expand causes layout jump
**sim_diagnostics.rs line 126**
Semantic Context has `default_open(true)`. When scrubbing through the timeline, the content changes (different semantic item or "No semantic item"), causing height changes that shift everything below. Change to `default_open(false)` so the user opts in, or allocate a minimum height for the section to prevent layout jumps: wrap the content in a fixed-height `egui::ScrollArea::vertical().max_height(120.0)`.
**Risk: safe** — layout stabilization.

### N3-17: Timeline click targets small on HiDPI
**Lines 227, 445**
Timeline bars use fixed `height = 12.0` pixels. On HiDPI displays this is physically tiny. Use `ui.spacing().interact_size.y.max(16.0)` or scale by `ui.ctx().pixels_per_point()` to ensure a minimum physical size of ~8mm. Change `let height = 12.0;` to `let height = (12.0 * ui.ctx().pixels_per_point()).max(16.0) / ui.ctx().pixels_per_point();` to ensure a minimum logical size.
**Risk: safe** — sizing adjustment.

### N3-18: Issue navigation gated behind Debug
**Lines 26-101**
The debug drawer (which contains issue navigation via focused_issue_index) is only visible when `sim.debug.enabled`. Users who don't enable debug mode cannot navigate to cutting issues or hotspots. Consider promoting the issue/hotspot navigation to the main cutting metrics panel in sim_diagnostics.rs, ungated by debug mode.
**Risk: low** — feature accessibility change; needs UI space consideration.

### N4-20: Transport buttons inconsistent widths
**Lines 112-143**
Buttons `"|◄"`, `"◄"`, `"▶"/"❚❚"`, `"►"`, `"►|"` have different text widths, causing layout shifts when play/pause toggles. Use `ui.add_sized(egui::vec2(30.0, 20.0), egui::Button::new(...))` for uniform button widths, or set `min_size` on each button.
**Risk: safe** — button sizing.

---

## `ui/preflight.rs`

### N3-11: Cycle time estimate cutting-only
**Lines 130-144, 226-237**
`estimate_total_time` sums `cutting_distance / feed_rate` but ignores rapid distance, tool change time, and plunge time. The displayed time is an undercount. Add rapid time: `rapid_distance / rapid_feed` (use machine rapid rate, e.g. `state.job.machine.rapid_rate` if available, else a default 5000 mm/min). Also add a label suffix: change "Cycle time" detail to include "(cutting only)" until rapids are accounted for.
**Risk: low** — computation change; needs access to rapid feed rate.

### N3-12: Tool change count undercounts
**Lines 239-251**
`count_tool_changes` counts unique tools minus 1. This undercounts when the same tool is used non-consecutively (e.g. Tool A -> Tool B -> Tool A = 2 changes, but function returns 1). Fix: iterate toolpaths in order, increment counter each time `tool_id` differs from the previous enabled toolpath's `tool_id`.
**Risk: safe** — computation fix.

### N4-18: Preflight pass green wrong shade
**Lines 192-196**
`CheckStatus::Pass` uses `\u{2705}` (green checkbox emoji) with color `(100, 200, 100)`. The emoji already has its own green color, so the applied tint may look off. Either use a plain ASCII checkmark `"\u{2713}"` with the green color, or use the emoji without color override. Change line 193: use `("\u{2713}", egui::Color32::from_rgb(80, 180, 80))` for a clean monochrome look.
**Risk: safe** — icon/color tweak.

---

## `ui/project_tree.rs`

### N1-04: Dead code
Audit this file for functions or branches that are unreachable. The file is active (used in Setup workspace's left panel in the older project tree view — but `setup_panel.rs` now draws the left panel for Setup workspace per `app.rs:1976`). Check if `project_tree.rs` `draw()` is called anywhere. If not, the entire file is dead code. Search for `project_tree::draw` call sites — if none, remove the file and its `mod` declaration.
**Risk: low** — need to verify no remaining call sites before removal.

### N1-13: Right-click context menus not discoverable
**Various files (project_tree.rs, toolpath_panel.rs, setup_panel.rs)**
Context menus exist on models, tools, setups, and toolpaths but there is no visual affordance. Add a subtle "..." or kebab-menu button next to items that have context menus, or show a tooltip on first use: "Right-click for more options". The simplest approach: add a dim `ui.label("...")` next to items that have `.context_menu()` attached.
**Risk: safe** — UI affordance addition.

---

## `ui/toolpath_panel.rs`

### N1-12: Drag-drop indicator always at bottom
**Lines 70-102**
The drop target logic (`compute_drop_index`) determines position from pointer, but the visual indicator (if any) may not update. Verify that a visual drop indicator (line/highlight) is shown at the correct position during drag. If missing, add a painted line at the computed drop index position during drag hover.
**Risk: low** — requires understanding of the drag/drop rendering path.

---

## `ui/menu_bar.rs`

### N1-11: Edit menu sparse
**Lines 107-116**
Edit menu contains only Undo and Redo. Consider adding: "Select All Toolpaths", "Deselect", "Delete Selected" (with keyboard shortcut hints). At minimum, add a separator and "Preferences..." placeholder (disabled) so the menu doesn't look vestigial.
**Risk: safe** — menu item additions.

### N5-16: Shortcut text not right-aligned
**Lines 108, 112**
Shortcut text is embedded in the button label: `"Undo  Ctrl+Z"`. This doesn't right-align the shortcut. Use `egui::Button::new("Undo").shortcut_text("Ctrl+Z")` which egui renders with the shortcut right-aligned. Apply to all menu items with shortcuts.
**Risk: safe** — egui API usage improvement.

---

## `ui/properties/post.rs`

### N2-19: (see operations.rs section above)
**Lines 41-57**
Already covered. Rename "High Feedrate Mode (G0->G1)" to "Safe Rapids (G0 -> G1 at feed speed)".

---

## `ui/status_bar.rs`

### N5-17: Save success silent, no dirty indicator
**Lines 88-96**
The dirty indicator ("Modified") is already shown in status_bar.rs:88-95. However, there is no transient "Saved" confirmation. After `save_job_to_path` succeeds (controller/io.rs:146-151), push a notification toast: `self.push_notification("Project saved", Severity::Info)`. The dirty flag is already cleared at line 149.
**Risk: safe** — toast notification addition.

---

## `ui/sim_op_list.rs`

### N3-19: No re-check holder button
The sim op list and sim diagnostics panels lack a "Re-check Holder" button. The overlay has "Check Holder" but it is hidden in Simulation workspace (viewport_overlay.rs:79 gates it to Setup/Toolpaths only). Add a "Check Holder" button to the simulation workspace — either in the sim top bar (app.rs:2081-2088) alongside "Re-run" and "Reset", or in the sim_diagnostics panel near the holder collision section.
**Risk: safe** — button addition with existing event.

---

## `controller/events.rs`

### N5-11: Generate with missing tool silent
**Lines 944-984**
When `submit_toolpath_compute` is called and the tool is not found (line 976-984), it returns silently. The user clicks Generate and nothing happens. Add: when tool lookup fails, set toolpath status to `ComputeStatus::Error("No tool assigned".into())` before returning, or push a warning notification.
**Risk: safe** — error reporting improvement.

### N5-18: Collision check result only logged
**`request_collision_check` (line 837)**
After collision check completes, results are stored in `sim.checks` but no toast/notification is shown. The user must check the sim diagnostics panel to see results. After collision check completion, push a notification summarizing the result: "Holder check: N collisions found" or "Holder check: all clear". This likely happens in the compute result handler, not directly in `request_collision_check`.
**Risk: low** — need to find where collision results are received and add notification there.

---

## `controller.rs`

### N5-12: (see app.rs section above)
**Lines 216-237**
Already covered. Add notification when auto-regen fires.

---

## Cross-file / Multi-panel fixes

### N4-06: Grid row spacing varies
**Multiple panels**
Different grids use different spacing: `[8.0, 4.0]` is most common (operations, tool params), but some use `[8.0, 2.0]` (sim diagnostics params grid, op stats grid). Standardize all property grids to `[8.0, 4.0]` and all compact info grids to `[8.0, 2.0]`. Define constants: `const PROPERTY_GRID_SPACING: [f32; 2] = [8.0, 4.0];` and `const INFO_GRID_SPACING: [f32; 2] = [8.0, 2.0];` in a shared UI constants module.
**Risk: safe** — spacing constant extraction.

### N4-10: Toast rounding mismatch
**app.rs line 2292**
Toasts use `rounding(6.0)`. Other UI frames (cards, panels) use `rounding(3.0)` or `rounding(4.0)`. Standardize: pick one rounding value for floating elements (toasts, popups) and one for inline frames (cards). Use `4.0` for everything or define constants.
**Risk: safe** — cosmetic constant change.

### N4-17: Color swatches 3 sizes
**sim_op_list.rs line 150 (`8x8`), plus any other swatch usages**
Color swatches in the sim op list use `egui::vec2(8.0, 8.0)`. Verify all swatch sizes across the codebase are consistent. Standardize to `8x8` or `10x10` everywhere.
**Risk: safe** — size constant alignment.

---

## Summary by risk level

| Risk | Count | Findings |
|------|-------|----------|
| Safe | 37 | N1-03, N1-04, N1-05, N1-07, N1-08, N1-09, N1-10, N1-11, N1-13, N1-14, N2-04, N2-13, N2-14, N2-16, N2-17, N2-18, N2-19, N3-07, N3-08, N3-09, N3-14, N3-15, N3-16, N3-17, N4-06, N4-09, N4-10, N4-11, N4-12, N4-17, N4-18, N4-20, N4-21, N4-23, N5-10, N5-13, N5-16, N5-17 |
| Low | 12 | N1-02, N1-12, N1-15, N2-08, N2-09, N2-12, N2-20, N3-06, N3-11, N3-12, N3-18, N5-08, N5-11, N5-12, N5-18 |
| Medium | 0 | — |
