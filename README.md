# rs_cam

`rs_cam` is a Rust CAM workspace for 3-axis wood routers. It combines a reusable CAM core library, a batch CLI, and a desktop GUI for interactive toolpath generation, simulation, and G-code export.

## What ships today

- Input formats: STL, SVG, DXF, STEP
- Output formats: GRBL, grblHAL, LinuxCNC, and Mach3 G-code, plus SVG preview and HTML setup-sheet export
- Tool families: flat end mill, ball nose, bull nose, V-bit, tapered ball nose
- Operations: 23 GUI-exposed machining strategies across 2.5D, roughing, and 3D finishing
- Verification: tri-dexel volumetric stock simulation (Z/X/Y grids, 6 cardinal cut directions), playback, and holder/shank collision checks
- Responsive compute: separate toolpath and analysis lanes with explicit queue/cancel state
- Automation: direct CLI commands plus TOML-driven job execution
- Feeds and speeds: machine + material models with vendor-LUT-assisted recommendations, plus a literature-matrix validation suite (56 cited cells × 19 invariants) that pins engine output to vendor and handbook bands, and a citation-freshness reporter for ongoing source maintenance
- Internal architecture: controller-first GUI shell, canonical operation metadata, and shared adaptive support code across 2D/3D adaptive

## Workspace layout

- `crates/rs_cam_core`: geometry, importers, cutter math, toolpath generation, dressups, simulation, feeds/speeds, and G-code
- `crates/rs_cam_cli`: batch interface and TOML job runner
- `crates/rs_cam_viz`: `egui`/`wgpu` desktop application (`rs_cam_gui`)
- `crates/rs_cam_mcp_proxy`: stdio MCP supervisor that lets an agent restart the GUI
- `architecture/`: durable design docs
- `research/`: algorithm notes, provenance, and exploratory research
- `planning/`: current status, open plans, and the evidence live sentries cite

## Quick start

Run the desktop app:

```bash
cargo run -p rs_cam_viz --bin rs_cam_gui
```

Inspect the CLI surface:

```bash
cargo run -p rs_cam_cli -- --help
```

The CLI `project` command needs `--output-dir <DIR>`. It writes the large
per-sample `simulation.json` only with `--sim-artifact` (default off, as the
GUI cut-trace file). The memory budget of
the GUI and the CLI is half of the system RAM by default. To change it, use
**File ▸ Preferences** in the GUI, or set `[memory] limit` in
`~/.config/rs_cam/settings.toml` (`"24GiB"`, `"unlimited"` or `"default"`).
The CLI flag `--memory-limit` overrides the file for one CLI run.
**File ▸ Preferences** also sets the other app settings in the same file:
the window, undo and toast values, the viewport and simulation defaults,
the tool, machine and screenshot folders (the CLI reads the same folders),
and the diagnostics files. The simulation cut-trace file is off by default.
See [`FEATURE_CATALOG.md`](FEATURE_CATALOG.md#settings-file).

Run the test suite:

```bash
cargo test -q
```

## Agent-restartable MCP

Claude Code talks to the GUI over stdio MCP (`rs_cam_gui --mcp`). Claude Code
does not restart a stdio server, so a GUI rebuild or a GUI crash removes the
tools until a human types `/mcp`. `rs_cam_mcp_proxy` prevents this. The proxy
is the MCP server that Claude Code starts, and the proxy starts the GUI as
its child. When the GUI exits, the proxy stays up. The agent then calls
`gui_status` for the reason (an OOM kill included) and `gui_restart` to start
the GUI again.

Build the proxy and the GUI:

```bash
cargo build --release -p rs_cam_mcp_proxy
cargo build --release -p rs_cam_viz --bin rs_cam_gui
```

Register the proxy for this checkout (local scope overrides the `rs-cam`
entry in `.mcp.json`):

```bash
claude mcp add -s local -e RUST_LOG=info -e RUST_BACKTRACE=1 rs-cam -- \
  /home/ricky/personal_repos/rs_cam/target/release/rs_cam_mcp_proxy \
  --log /home/ricky/.rs_cam_logs/mcp_proxy.log -- \
  systemd-run --user --scope -q -p MemoryMax=16G -p MemorySwapMax=0 \
  /home/ricky/personal_repos/rs_cam/target/release/rs_cam_gui --mcp
```

The first `--` starts the child command. Keep the proxy outside the
`systemd-run` scope: the cgroup OOM kill then stops only the GUI, and the
proxy reports it. After a GUI rebuild, `gui_status` shows
`binary.newer_than_process: true`; call `gui_restart` to load the new binary.
A restart loses the unsaved project state of the GUI.

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
