# Review: Render Pipeline

## Scope
GPU rendering via wgpu — mesh, toolpath, simulation, stock, fixtures, grid.

## Files to examine
- `crates/rs_cam_viz/src/render/mod.rs` (coordinator)
- `crates/rs_cam_viz/src/render/camera.rs`
- `crates/rs_cam_viz/src/render/mesh_render.rs`
- `crates/rs_cam_viz/src/render/toolpath_render.rs`
- `crates/rs_cam_viz/src/render/sim_render.rs`
- `crates/rs_cam_viz/src/render/stock_render.rs`
- `crates/rs_cam_viz/src/render/fixture_render.rs`
- `crates/rs_cam_viz/src/render/grid_render.rs`
- `crates/rs_cam_viz/src/render/height_planes.rs`

## What to review

### Architecture
- How does the render pipeline integrate with egui + wgpu?
- Callback-based rendering? Custom render pass?
- GPU buffer management: upload patterns, buffer reuse
- Frame loop: when does re-render happen?

### Camera
- Orthographic and perspective support?
- Orbit, zoom, pan controls
- Camera presets (isometric, top, front, etc.)

### Mesh rendering
- STL mesh: vertex/index buffers, normals, lighting
- Simulation mesh: different material?
- Wireframe vs solid modes?

### Toolpath rendering
- Line rendering: thick lines or geometry?
- Color coding: by operation, by move type (cut/rapid)?
- Isolation mode: hide all but one

### Performance
- Re-upload frequency: every frame or only on change?
- Large mesh performance
- Many toolpath lines performance

### Testing
- Are there any render tests? Visual regression?

## Output
Write findings to `review/results/31_render.md`.
