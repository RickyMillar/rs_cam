# `app/` — the application shell and the embedded MCP server

The egui application, its per-frame work and the GUI-side MCP request
processing. The server lives here, not in `rs_cam_mcp`.

## Files

- `../app.rs` — `RsCamApp` and the frame update loop.
- `mcp.rs` + `mcp/` — the MCP pump (`handle_mcp_request`, `ui_query`,
  export, notifications); children `commands.rs` (the core command route),
  `project.rs` (project reads, `load_project`), `generation.rs` (add,
  generate, optimize, feeds, multi-tool), `diagnostics.rs` (diagnostics,
  narration, debug trace), `simulation.rs` (run, scrub, cut trace,
  collisions), `view.rs` (reach map, screenshots, `set_ui_view`), `tests.rs`.
- `export.rs` — the GUI export path.
- `simulation.rs` — the per-frame simulation work.
- `viewport.rs`, `gpu_upload.rs` — the viewport frame and the GPU upload pass.
- `input.rs` — keyboard and mouse handling at the application level.

## Invariants

- An MCP mutation dispatches through the same command path as the UI.
- A setter stales the result. Regenerate before you export. Tool and model
  rebinding uses its own endpoint, not a generic parameter setter.
- Export refuses missing or stale enabled geometry, and an unsafe or
  unmodelled load verdict, unless the caller accepts the override.
- Use `generation_status` and `cancel_generation` for a long job. Do not
  queue a second generation behind an unknown first one.
- An MCP screenshot is visual evidence, not a replacement for a core test.
  Capture only after the frame has applied the requested view state.

## Sentries

- `cargo test -p rs_cam_viz -q --test mcp_authoring_surface`
- `cargo test -p rs_cam_viz -q --test mcp_core_arm_describes_every_row`
- `cargo test -p rs_cam_viz -q --test mcp_escape_hatches`
- `cargo test -p rs_cam_viz -q --test mcp_wire_surface_pin`
- `cargo test -p rs_cam_viz -q --test export_parity_core_vs_gui_p0`

## Do not

- `.mcp.json` runs `cargo run --release`. Build the release binary BEFORE an
  MCP live test; a cold compile passes the 30 s connect timeout.
