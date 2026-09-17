# `app/` — the application shell and the embedded MCP server

The egui application, its per-frame work and the GUI-side MCP request
processing. The server lives here, not in `rs_cam_mcp`.

## Files

- `../app.rs` — `RsCamApp` and the frame update loop.
- `mcp.rs` + `mcp/` — the MCP pump (`handle_mcp_request`, export,
  notifications); children `commands.rs` (core command route), `project.rs`,
  `generation.rs` (add, generate, optimize, feeds), `diagnostics.rs`,
  `simulation.rs` (run, scrub, cut trace), `view.rs` (reach map, screenshots).
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
- An MCP export reports the machine-safety findings beside the path it
  wrote. Take `io::export`'s reporting door; do not re-run the validator.
- Use `generation_status` and `cancel_generation` for a long job. Do not
  queue a second generation behind an unknown first one.
- An MCP screenshot is visual evidence, not a replacement for a core test.
  Capture only after the frame has applied the requested view state.
- `.mcp.json` launches `target/release/rs_cam_gui --mcp` directly. Rebuild
  the release binary BEFORE an MCP live test, or the server runs old code.

## Sentries

- `cargo test -p rs_cam_viz -q --test mcp_authoring_surface`
- `cargo test -p rs_cam_viz -q --test mcp_core_arm_describes_every_row`
- `cargo test -p rs_cam_viz -q --test mcp_escape_hatches`
- `cargo test -p rs_cam_viz -q --test mcp_wire_surface_pin`
- `cargo test -p rs_cam_viz -q --test export_parity_core_vs_gui_p0`
