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
| Core only (dev loop) | `cargo test -p rs_cam_core -q` |
| Core FULL gate | `cargo test -p rs_cam_core --features heavy-tests,research,test-support --no-fail-fast -- -q` |
| CLI integration | `cargo test -p rs_cam_cli --test integration` |
| Viz regression | `cargo test -p rs_cam_viz controller::tests::` |
| Compute worker | `cargo test -p rs_cam_viz compute::worker::tests::` |
| MCP param structs | `cargo test -p rs_cam_mcp -q` |
| Single test | `cargo test <name> -- --nocapture` |

Note: run the folder sentries and the focused crate tests locally; CI owns the FULL gate. Ask before a run over about three minutes. On a shared machine, run cargo through `scripts/cargo_lane.sh`.

Note: avoid workspace-wide `cargo test` from the repo root — it can loop / thrash on this repo. Run per-crate instead.

Note: the 12 heaviest core binaries sit behind the `heavy-tests` feature (75% of the serial suite, 2026-08-27 profile), so the dev-loop row does not compile or run them. CI runs the FULL gate row; locally it needs the operator's go-ahead (30+ min); one binary alone runs as `cargo test -p rs_cam_core --features heavy-tests --test <name>`. This is separate from `#[ignore]`, which stays what it has always been here — instruments and evidence runs you invoke by name.

## Quality

| What | Command |
|------|---------|
| Lint | `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research,rs_cam_core/test-support -- -D warnings` |
| Format check | `cargo fmt --check` |
| Format fix | `cargo fmt` |
| Benchmark | `cargo bench -p rs_cam_core` |

## Module Map

Each folder with an invariant carries a `CLAUDE.md` beside its code
(`crates/rs_cam_core/src/<folder>/CLAUDE.md`, and the same under
`crates/rs_cam_viz/src/`). Read it for that folder's invariants and sentries.

### rs_cam_core — CAM engine (folders since 2026-09-17)

The crate root holds `lib.rs` and the spine only: `geo.rs`, `polygon.rs`,
`mesh.rs`, `toolpath.rs`, `ids.rs`, `interrupt.rs`, `measurement.rs`.
Every other module sits in a folder. `folder/mod.rs` carries the folder's
principal type where one exists (`io`, `machine`, `dressup`, `material`).

| Folder | Holds |
|---|---|
| `ops/` | 2.5D and drilling operations: `pocket`, `profile`, `face`, `drill`, `drill_op`, `drill_metrics`, `vcarve`, `inlay`, `chamfer`, `waterline`, `zigzag`, `trace_path`, `rest`, `project_curve`, `depth`, `adaptive_shared` |
| `finish/` | 3D finishing: `scallop`, `scallop_isofield`, `scallop_math`, `pencil`, `pencil_dihedral`, `unified_finish`, `finish_planner`, `finish_setup`, `conformal_spiral`, `direction_field` (these two and `spiral_finish_compact` build only with `--features research`), `crest_lines`, `crease_paths`, `classify_probe`, `steep_shallow`, `surface_link`, `ramp_finish`, `spiral_finish`, `spiral_finish_compact`, `radial_finish`, `horizontal_finish` |
| `adaptive/`, `adaptive3d/` | adaptive clearing, 2D and 3D |
| `geometry/` | derived geometry: `boundary`, `contour_extract`, `edge_distance`, `enriched_mesh`, `fiber`, `grid2`, `grid_field`, `marching_squares`, `monotone_cells`, `nn_order`, `point_runs`, `region_mask`, `region_set`, `arc_util` |
| `surface/` | surface queries: `dropcutter`, `pushcutter`, `slope`, `rest_field`, `reach`, `flow_accum` |
| `maps/` | tier and reach maps and their caches: `tier_map`, `tier_islands`, `reach_map`, `rest_heatmap_mesh`, `*_cache`, `memo`, `grid`, `tool_shape_key` |
| `stock/`, `dexel_stock/` | stock model and simulation: `dexel`, `dexel_mesh`, `dexel_mesh_mc`, `simulation_cut`, `sim_triage`, `sim_measurability`, `collision`, `stock_mesh`, `radial_profile`; the engine is `dexel_stock/` |
| `dressup/` | post-generation passes: `dressup` (mod), `arcfit`, `condition`, `feedopt`, `feed_modulation`, `tsp`, `entry_audit` |
| `feeds/`, `tool_load/` | feeds and speeds; load, chipload and power gates; the optimizer |
| `tool/`, `material/`, `machine/` | cutter families; material library; machine profile, `kinematics`, `kinematic_utilization`, `strategy_advisor` |
| `io/` | importers and libraries: `svg_input`, `dxf_input`, `step_input`, `tool_library`, `machine_library`, `named_toml_library`; the project-file reader is `io/mod.rs` |
| `gcode/`, `export/` | post-processors; `viz`, `fingerprint`, `gcode_validator`, `artifact_io` |
| `trace/`, `diagnostics/` | `debug_trace`, `semantic_trace`, `toolpath_spans`, `narrate`, `transform_provenance`; typed findings |
| `session/`, `compute/` | `ProjectSession` and `Command`; operation dispatch, catalog, config, simulate |
| `metrology/`, `util/` | measurement instruments; `panic_message`, `build_info` |

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
