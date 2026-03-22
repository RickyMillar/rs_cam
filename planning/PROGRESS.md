# Progress

## Current snapshot

`rs_cam` is now a desktop CAM application plus shared engine, not just an algorithm sandbox.

### Shipped surface

- 3-crate Rust workspace: core library, CLI, and desktop GUI
- 22 GUI-exposed operations
- 14 direct CLI commands plus TOML job execution
- STL, SVG, and DXF import
- 5 cutter families
- GRBL, LinuxCNC, and Mach3 post-processors
- feeds/speeds calculator with machine, material, and vendor-LUT inputs
- heightmap stock simulation and holder/shank collision checks
- typed GUI project persistence with missing-model warnings and editable-state round-trip
- dual-lane compute backend with lane-status reporting and active cancel support
- deterministic renderless `rs_cam_viz` regression harness in CI
- controller-first GUI architecture with canonical operation metadata and split compute/controller modules
- shared adaptive support module used by both 2D and 3D adaptive search/control code

## Current priorities

- **Tri-dexel simulation** — replace the 2.5D heightmap with a segment-based volumetric representation to enable multi-setup stock removal simulation. Design doc: `architecture/TRI_DEXEL_SIMULATION.md`, implementation plan: `planning/VOXEL_SIM_DESIGN.md`
- keep public docs aligned with the actual code surface
- preserve explicit attribution for algorithms, datasets, and runtime assets
- maintain the lint/test gate as the default merge bar

## Recent work (2026-03-22)

### Workspace UX redesign — multi-setup coordinate frames

Unified the coordinate frame pipeline so all toolpaths are generated and
displayed in setup-local coordinates. Previously, identity setups (Top+Deg0)
generated in global coords while non-identity setups used local, causing
intermittent alignment bugs.

Key changes:
- Generation always transforms mesh/stock to local frame (even for identity setups)
- All workspaces (including Simulation) display in the active setup's local frame
- Per-workspace display rules: solid stock in Setup only, model hidden in Simulation, etc.
- Setup panel shows effective stock dimensions for active orientation
- "Toolpaths use fresh stock" badge on non-first setups

### Multi-setup simulation — 2.5D heightmap limitation discovered

The 2.5D heightmap can only model cuts from one direction (top-down).
Attempting to simulate flipped setups on one heightmap causes gouging:
bottom cuts are misinterpreted as deep top-to-bottom cuts. This is a
fundamental data structure limitation, not a transform bug.

Current workaround: each setup simulates independently in its own local
frame with fresh stock. Correct for single-setup and same-orientation
multi-setup. Cross-setup material carry-forward deferred to tri-dexel.

### Tri-dexel simulation design

Completed research and design for replacing the heightmap with a tri-dexel
representation. Three orthogonal grids of ray segments (Z, X, Y) handle
cuts from any cardinal direction natively. SmallVec fast path keeps
single-setup performance within 20% of the current heightmap. Six-phase
implementation plan from core data types through multi-setup carry-forward.
See `architecture/TRI_DEXEL_SIMULATION.md`.

## Known open work

- **tri-dexel implementation** (6 phases, see `planning/VOXEL_SIM_DESIGN.md`)
- emit per-operation manual pre/post G-code in export
- wire profile controller compensation (`G41` / `G42`)
- surface rapid-collision rendering and simulation deviation coloring
- expose workholding rigidity and vendor-LUT management in the GUI
- continue optional cleanup in `adaptive.rs` / `adaptive3d.rs`, but structural blockers are no longer the active tranche

## Verification

- `cargo run -q -p rs_cam_cli -- --help` succeeds
- `cargo fmt --check` passes
- `cargo test -q` passes on the workspace
- `cargo clippy --workspace --all-targets -- -D warnings` passes

Update this file when the shipped surface or verification status changes materially.
