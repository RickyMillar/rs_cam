# rs_cam

`rs_cam` is a Rust CAM workspace for 3-axis wood routers. It combines a reusable CAM core library, a batch CLI, and a desktop GUI for interactive toolpath generation, simulation, and G-code export.

## What ships today

- Input formats: STL, SVG, DXF
- Output formats: GRBL, LinuxCNC, and Mach3 G-code, plus SVG preview and HTML setup-sheet export
- Tool families: flat end mill, ball nose, bull nose, V-bit, tapered ball nose
- Operations: 22 GUI-exposed machining strategies across 2.5D, roughing, and 3D finishing
- Verification: heightmap stock simulation, playback, and holder/shank collision checks
- Responsive compute: separate toolpath and analysis lanes with explicit queue/cancel state
- Automation: direct CLI commands plus TOML-driven job execution
- Feeds and speeds: machine + material models with vendor-LUT-assisted recommendations
- Internal architecture: controller-first GUI shell, canonical operation metadata, and shared adaptive support code across 2D/3D adaptive

## Workspace layout

- `crates/rs_cam_core`: geometry, importers, cutter math, toolpath generation, dressups, simulation, feeds/speeds, and G-code
- `crates/rs_cam_cli`: batch interface and TOML job runner
- `crates/rs_cam_viz`: `egui`/`wgpu` desktop application (`rs_cam_gui`)
- `architecture/`: durable design docs
- `research/`: algorithm notes, provenance, and exploratory research
- `planning/`: active backlog, status, and archived planning snapshots

## Quick start

Run the desktop app:

```bash
cargo run -p rs_cam_viz --bin rs_cam_gui
```

Inspect the CLI surface:

```bash
cargo run -p rs_cam_cli -- --help
```

Run the test suite:

```bash
cargo test -q
```

## Documentation map

- Product surface: [`FEATURE_CATALOG.md`](FEATURE_CATALOG.md)
- Source and algorithm attribution: [`CREDITS.md`](CREDITS.md)
- Architecture overview: [`architecture/README.md`](architecture/README.md)
- Research and background material: [`research/README.md`](research/README.md)
- Planning and backlog: [`planning/README.md`](planning/README.md)

## Current gaps

The repo is well past the prototype stage, but some edges are still being finished:

- per-operation manual pre/post G-code is editable in the GUI but not emitted during export
- GUI project save/load round-trips editable state and model references, but computed toolpaths, simulation caches, and collision outputs are intentionally regenerated after load
- profile “In Control” compensation exists in UI/state, but `G41`/`G42` emission is not wired
- feed optimization is limited to fresh-stock, flat-stock workflows with known stock bounds; unsupported cases are disabled instead of approximated
- rapid-collision rendering and simulation deviation coloring have core/helpers in place but are not fully surfaced

## Verification gate

The repo currently keeps these gates green:

- `cargo fmt --check`
- `cargo test -q`
- `cargo clippy --workspace --all-targets -- -D warnings`

Linux CI also runs a dedicated `rs_cam_viz` regression lane for the renderless GUI harness and compute-lane queue/cancel tests.

## Open-source provenance

This repo carries explicit attribution for algorithm lineage, data sources, and external runtime assets in [`CREDITS.md`](CREDITS.md). The short version: the cutter-contact and waterline stack is heavily informed by OpenCAMLib, adaptive-clearing ideas are informed by Freesteel/libactp and FreeCAD CAM, the 2D offset/boolean layer builds on `geo`/`i_overlay` and `cavalier_contours`, and the feeds/speeds system is attributed directly to its vendor charts, material-property references, and formula sources rather than to an old imported precursor project.
