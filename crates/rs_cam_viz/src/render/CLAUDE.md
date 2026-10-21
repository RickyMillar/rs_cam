# `render/` — the wgpu draw pipelines

Every viewport pipeline. The entry point is `render::mod`, called from `app/viewport.rs`.

## Files

- `mod.rs` — the pipeline facade.
- `camera.rs` — the camera and its projection.
- `toolpath_render.rs`, `sim_render.rs`, `stock_render.rs`, `mesh_render.rs` — the four content pipelines.
- `grid_render.rs`, `fixture_render.rs`, `height_planes.rs` — the reference
  geometry.
- `colors.rs` — every colour constant. A selection state colour is its UI token
  / 255 (`token_rgb`); `DRAWING_NEUTRAL` colours a drawing that no drawn toolpath reads.
- `mod.rs` `hover_lines` (the pick hover ring) and `tab_lines` (the tab editor marks, `TAB_AUTO`, `TAB_HAND`,
  `TAB_DRAGGED`) — each its own buffer, drawn after the drawing. The tab marks draw with the curves off too.
- `sim_render.rs` `head_draw_parts` — the H2 machine head and shoe the tool
  ghost and the tool preview draw; a soft part draws lighter.
- `upload_cache.rs` — the content-addressed keys for the GPU upload pass.
- `gpu_safety.rs` — buffer size guards against a device-limit crash.

## Invariants

- A colour belongs in `colors.rs`. Do not write a literal colour in a
  pipeline. A render-only colour is an `Rgb`; only `colors.rs` makes one (F173).
- Every upload goes through `upload_cache.rs`, so that an unchanged buffer is
  not re-uploaded each frame. `refresh` is the compare-then-rebuild guard; it
  takes a `SlotInputs` value, not a key (F132). Do not hand-roll a second one.
- Check a buffer against the device limit before you create it.
- A move draws and picks through `toolpath_render::move_draw_points_into`: an arc walks core `linearize_arc_into` (R-ARC).

## Sentries

- `cargo test -p rs_cam_viz -q --test render_pipelines_headless_g_pipesmoke`
- `cargo test -p rs_cam_viz -q --test reach_overlay_p5`
- `cargo test -p rs_cam_viz -q --test viewport_draws_selected_only_wp27 --test an_arc_move_draws_on_its_arc_r_arc`
- `cargo test -p rs_cam_viz -q --test the_selection_colours_live_in_colors_p4_4 --test panels_read_the_token_module_up1` (arm 5: no literal colour)

## Do not

- Draw cost scales with the drawn toolpath count. `ui/` decides the draw set.
