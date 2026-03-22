# Review: Interaction (Input, Picking, Camera)

## Scope
Mouse/keyboard input handling and 3D viewport interaction.

## Files to examine
- `crates/rs_cam_viz/src/interaction/mod.rs`
- `crates/rs_cam_viz/src/interaction/picking.rs`
- Camera controls in `crates/rs_cam_viz/src/render/camera.rs`
- Keyboard shortcuts (grep for key binding patterns)
- `crates/rs_cam_viz/src/app.rs` (input routing in update loop)

## What to review

### Input handling
- Mouse: left-click, right-click, drag, scroll
- Keyboard: shortcuts (Ctrl+S, Ctrl+Z, Space, arrow keys, etc.)
- Are shortcuts documented? Configurable?
- Input conflicts between UI panels and viewport

### 3D picking
- How does clicking in viewport select objects?
- Ray casting from screen to world coordinates?
- What can be picked? (toolpath, mesh, fixture?)
- Selection feedback (highlight, outline?)

### Camera controls
- Orbit: center point, sensitivity
- Zoom: scroll, limits (min/max distance)
- Pan: drag or keyboard?
- Preset views: smooth transition or snap?

### Edge cases
- Picking through UI overlays
- Mouse capture during drag
- Window resize handling

### Testing & code quality

## Output
Write findings to `review/results/34_interaction.md`.
