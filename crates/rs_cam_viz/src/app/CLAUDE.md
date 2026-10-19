# `app/` — the application shell and the embedded MCP server

The egui application, its per-frame work and the GUI-side MCP request
processing. The server lives here, not in `rs_cam_mcp`.

## Files

- `../app.rs` — `RsCamApp` and the frame update loop.
- `mcp.rs` + `mcp/` — the MCP pump (`handle_mcp_request`, export, notifications);
  `commands.rs` (core route; S4 rows in `commands/stock_changes.rs`; P2.7 selection rows and the
  `selection_status` parity test in `commands/selection.rs`; P4.8 `import_model` options, `set_model_import_options`, `model_import_json` and their GUI parity tests in `commands/import.rs`; P4.9 `edit_model_drawing`, `drawing_ledger_json` (the reply and `inspect_model`) and the GUI parity test in `commands/drawing.rs`), `project.rs`, `generation.rs` (add, generate, feeds), `diagnostics.rs`,
  `simulation.rs` (run, scrub, cut trace), `view.rs` (reach map, screenshots).
- `export.rs` — the GUI export path.
- `simulation.rs` — the per-frame simulation work.
- `viewport.rs`, `gpu_upload.rs`, `pin_marks.rs` (P7 pins), `obstacle_marks.rs` (K2) — the frame and the GPU upload. P4.4: the drawing takes the item state colours, open path ends, lost-pick ghosts and the boundary outline; it uploads on a change of the report `Arc` (`state::selection_highlight::ReportDetector`), never per frame. P4.5: pick mode wins every click (shift = `PickLayer`); the hover ring is its own buffer (`upload_hover_lines`), never the drawing. P4.6: the tab editor wins the click, the right-click and a primary drag that starts on a tab (no orbit); `upload_tab_lines` runs on a `TabMarksDetector` change only. P4.7: in pick mode the ray also hits an STL mesh (`PickHit::MeshFace` → `face_pick_key_at`); a plain click does not. P4.9: with a model selected, a click on an open path opens the drawing popover (`panels.drawing_popover`; the two close acts).
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
- The MCP launch, the host exit and the shutdown watchdog: read `mcp/CLAUDE.md`
  before you change the pump, the `mcp-server` thread or the winit host exit.

## Sentries

- `cargo test -p rs_cam_viz -q --test mcp_authoring_surface`
- `cargo test -p rs_cam_viz -q --test mcp_core_arm_describes_every_row`
- `cargo test -p rs_cam_viz -q --test mcp_escape_hatches`
- `cargo test -p rs_cam_viz -q --test mcp_wire_surface_pin`
- `cargo test -p rs_cam_viz -q --test export_parity_core_vs_gui_p0`
