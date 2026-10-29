# `render/` — the wgpu draw pipelines

Every viewport pipeline. The entry point is `render::mod`, called from `app/viewport.rs`.

## Files

- `camera.rs` — the camera and its projection. `CameraPose`, `basis`; `orbit_about` a pivot,
  `zoom_toward` a point, `pan_pixels` (step from `world_per_pixel`); `preset_pose`, `fit_pose` (A C1).
- `toolpath_render.rs`, `sim_render.rs`, `stock_render.rs`, `mesh_render.rs` — the four content pipelines;
  `grid_render.rs`, `fixture_render.rs`, `height_planes.rs` — the reference geometry.
- `mesh_render.rs` `FaceMeshVertex` (28 B: position, normal, `face_id`), `face_mesh_gpu_data` — the STEP face
  pipeline (D2). The colour is the face state word + the palette uniform; the buffer holds no colour.
- `colors.rs` — every colour constant. A selection state colour is its UI token / 255 (`token_rgb`); `DRAWING_NEUTRAL`,
  P6.7 `PATH_SIDE`, `HOLE_MISFIT`. `face_palette`: the `FACE_SLOT_*` slots the face WGSL reads (`face_shader_reads_the_palette_slots`).
- `mod.rs` — the pipeline facade. `hover_lines` (the pick hover ring) and `tab_lines` (the tab editor marks, `TAB_AUTO`, `TAB_HAND`,
  `TAB_DRAGGED`) — each its own buffer, drawn after the drawing. The tab marks draw with the curves off too.
- `sim_render.rs` `head_draw_parts` — the H2 machine head and shoe; a soft part draws lighter.
- `upload_cache.rs` — the content-addressed keys for the GPU upload pass.
- `gpu_safety.rs` — buffer size guards; `offscreen_bytes` + `log_offscreen_budget` (scene target bytes, a log, not a gate).

## Invariants

- A colour belongs in `colors.rs`. Do not write a literal colour in a pipeline. Only `colors.rs` makes an `Rgb` (F173).
- Every upload goes through `upload_cache.rs`. `refresh` is the one compare-then-rebuild guard; it takes a `SlotInputs` value (F132).
- Check a buffer against the device limit before you create it.
- MSAA (D1): one `sample_count` field feeds every scene pipeline and target; the blit stays single-sample. A mismatch fails at pass time; the G-PIPESMOKE frame arm (`render_headless_frame`) covers it.
- The face pipeline needs a fragment storage buffer: Vulkan, Metal or DX12 (no GL, D-Q5). Only `app/gpu_upload/face_state.rs` writes its words.
- `LineUniforms` is 96 B. Only `line_edge_bind_group` sets `edge_tol` > 0 (`frame_set`, D4): the model edges (`edge_line_pipeline`, no depth write) and the overlay lines on the model move toward the eye; the grid and the toolpaths do not.
- A move draws and picks through `toolpath_render::move_draw_points_into`: an arc walks core `linearize_arc_into` (R-ARC).

## Sentries

- `cargo test -p rs_cam_viz -q --test render_pipelines_headless_g_pipesmoke --test reach_overlay_p5`
- `cargo test -p rs_cam_viz -q --test viewport_draws_selected_only_wp27 --test an_arc_move_draws_on_its_arc_r_arc`
- `cargo test -p rs_cam_viz -q --test the_selection_colours_live_in_colors_p4_4 --test panels_read_the_token_module_up1` (arm 5: no literal colour)
- `cargo test -p rs_cam_viz -q --test a_hover_rewrites_face_state_not_the_mesh_d3`

## Do not

- Draw cost scales with the drawn toolpath count. `ui/` decides the draw set.
