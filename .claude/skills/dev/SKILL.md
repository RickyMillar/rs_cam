---
name: dev
description: Build, test, run, and module quick reference for all 4 crates
disable-model-invocation: true
---

# /dev — Development Quick Reference

## Run

| What | Command |
|------|---------|
| Desktop GUI | `cargo run -p rs_cam_viz --bin rs_cam_gui` |
| CLI help | `cargo run -p rs_cam_cli -- --help` |
| CLI subcommand | `cargo run -p rs_cam_cli -- <subcommand> [args]` |
| TOML job | `cargo run -p rs_cam_cli -- job fixtures/demo_job.toml` |

## Test

| What | Command |
|------|---------|
| Per-crate (recommended) | `cargo test -p rs_cam_core -q && cargo test -p rs_cam_cli -q && cargo test -p rs_cam_viz -q && cargo test -p rs_cam_mcp -q` |
| Core only | `cargo test -p rs_cam_core -q` |
| CLI integration | `cargo test -p rs_cam_cli --test integration` |
| Viz regression | `cargo test -p rs_cam_viz controller::tests::` |
| Compute worker | `cargo test -p rs_cam_viz compute::worker::tests::` |
| MCP param structs | `cargo test -p rs_cam_mcp -q` |
| Single test | `cargo test <name> -- --nocapture` |

Note: avoid workspace-wide `cargo test` from the repo root — it can loop / thrash on this repo. Run per-crate instead.

## Quality

| What | Command |
|------|---------|
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Format check | `cargo fmt --check` |
| Format fix | `cargo fmt` |
| Benchmark | `cargo bench -p rs_cam_core` |

## Module Map

### rs_cam_core — CAM engine (56 modules)

**Operations (22):** `adaptive.rs`, `adaptive3d.rs`, `chamfer.rs`, `drill.rs`, `dropcutter.rs`, `face.rs`, `horizontal_finish.rs`, `inlay.rs`, `pencil.rs`, `pocket.rs`, `profile.rs`, `project_curve.rs`, `radial_finish.rs`, `ramp_finish.rs`, `rest.rs`, `scallop.rs`, `spiral_finish.rs`, `steep_shallow.rs`, `trace.rs`, `vcarve.rs`, `waterline.rs`, `zigzag.rs`

**Tools:** `tool/` — `FlatEndmill`, `BallEndmill`, `BullNoseEndmill`, `VBitEndmill`, `TaperedBallEndmill`

**Simulation:** `dexel.rs`, `dexel_stock.rs`, `dexel_mesh.rs`, `simulation.rs`, `simulation_cut.rs`

**Diagnostics:** `debug_trace.rs`, `semantic_trace.rs`, `collision.rs`

**Dressups & post:** `dressup.rs` (includes air-cut filter for stock-aware ops), `feedopt.rs`, `arcfit.rs`, `depth.rs`, `tsp.rs`

**Stock:** `radial_profile.rs`, `stock_mesh.rs`, `arc_util.rs` (extracted shared types)

**Import:** `mesh.rs`, `svg_input.rs`, `dxf_input.rs`

**Export:** `gcode.rs`

**Geometry:** `geo.rs`, `polygon.rs`, `boundary.rs`, `contour_extract.rs`, `fiber.rs`, `slope.rs`, `scallop_math.rs`, `pushcutter.rs`

**Other:** `adaptive_shared.rs`, `feeds/`, `interrupt.rs`, `machine.rs`, `material.rs`, `pipeline.rs`, `toolpath.rs`, `viz.rs`

### rs_cam_viz — Desktop GUI

| Area | Key files |
|------|-----------|
| Entry | `app.rs`, `lib.rs` |
| Controller | `controller.rs`, `controller/events.rs`, `controller/io.rs` |
| State | `state/mod.rs` (`AppState`, `JobState`, `SimulationState`, `ViewportState`) |
| Compute | `compute/worker.rs`, `compute/worker/execute/` (split: `mod.rs`, `operations_2d.rs`, `operations_3d.rs`) |
| UI | `ui/` (properties, project tree, sim timeline, sim diagnostics, setup panel, viewport overlay) |
| Render | `render/` (sim, mesh, toolpath, stock, grid, fixture, camera) |
| Interaction | `interaction/` (picking, mouse, keyboard) |
| IO | `io/` (project, export, setup sheet, presets) |

### rs_cam_cli — Batch CLI

| File | Purpose |
|------|---------|
| `main.rs` | Clap CLI with 14 subcommands |
| `job.rs` | TOML job parsing and execution |
| `helpers.rs` | Shared utilities (tool parsing, polygon loading) |
| `tests/integration.rs` | CLI integration tests |

## Architecture Invariants

- Core must not depend on GUI crates
- Toolpath IR is the boundary between generation and post-processing
- Import, tool modeling, operations, dressups, simulation, and export are distinct layers
- Extend existing core → worker → UI wiring — don't create parallel flows
- Library code must avoid `unwrap()`
- Tests live close to the code they validate
- GUI state field additions require auditing: setup-sheet, project-IO, and test initializers
