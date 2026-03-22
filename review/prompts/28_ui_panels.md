# Review: UI Panels (Layout, Usability, Consistency)

## Scope
All 20 egui UI panel files — layout, interaction patterns, consistency.

## Files to examine
- `crates/rs_cam_viz/src/ui/mod.rs` (coordinator)
- `crates/rs_cam_viz/src/ui/menu_bar.rs`
- `crates/rs_cam_viz/src/ui/project_tree.rs`
- `crates/rs_cam_viz/src/ui/setup_panel.rs`
- `crates/rs_cam_viz/src/ui/toolpath_panel.rs`
- `crates/rs_cam_viz/src/ui/properties/mod.rs` + all sub-files
- `crates/rs_cam_viz/src/ui/sim_timeline.rs`
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs`
- `crates/rs_cam_viz/src/ui/sim_op_list.rs`
- `crates/rs_cam_viz/src/ui/sim_debug.rs`
- `crates/rs_cam_viz/src/ui/workspace_bar.rs`
- `crates/rs_cam_viz/src/ui/viewport_overlay.rs`
- `crates/rs_cam_viz/src/ui/status_bar.rs`
- `crates/rs_cam_viz/src/ui/preflight.rs`
- `crates/rs_cam_viz/src/ui/automation.rs`

## What to review

### Layout
- Panel arrangement: left tree, right properties, center viewport, bottom status
- Resize behavior
- Workspace switching (Design vs Simulation)

### Consistency
- Do all property editors use the same patterns (DragValue, ComboBox, etc.)?
- Consistent spacing, labeling, grouping
- Consistent event emission patterns

### Usability concerns
- Are there panels that are too dense or too sparse?
- Missing labels, tooltips, or help text?
- Keyboard accessibility

### Automation IDs
- What's in automation.rs? Deterministic testing support?
- Are IDs comprehensive or patchy?

### Code quality
- Duplication across property editors
- Long functions that should be split
- State access patterns (does UI reach too deep into state?)

## Output
Write findings to `review/results/28_ui_panels.md`.
