# `render/` — the wgpu draw pipelines

Every viewport pipeline. The entry point is `render::mod`, called from
`app/viewport.rs`.

## Files

- `mod.rs` — the pipeline facade.
- `camera.rs` — the camera and its projection.
- `toolpath_render.rs`, `sim_render.rs`, `stock_render.rs`,
  `mesh_render.rs` — the four content pipelines.
- `grid_render.rs`, `fixture_render.rs`, `height_planes.rs` — the reference
  geometry.
- `colors.rs` — every colour constant, including `COLLISION_POINT`.
- `upload_cache.rs` — the content-addressed keys for the GPU upload pass.
- `gpu_safety.rs` — buffer size guards against a device-limit crash.

## Invariants

- A colour belongs in `colors.rs`. Do not write a literal colour in a
  pipeline.
- Every upload goes through `upload_cache.rs`, so that an unchanged buffer is
  not re-uploaded each frame.
- Check a buffer against the device limit before you create it.

## Sentries

- `cargo test -p rs_cam_viz -q --test render_pipelines_headless_g_pipesmoke`
- `cargo test -p rs_cam_viz -q --test reach_overlay_p5`
- `cargo test -p rs_cam_viz -q --test viewport_draws_selected_only_wp27`

## Do not

- Draw cost scales with the drawn toolpath count. `ui/` decides the draw set.
