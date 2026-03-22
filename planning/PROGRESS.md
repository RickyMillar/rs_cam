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
- tri-dexel stock simulation (Z/X/Y grids, all 6 cardinal faces) and holder/shank collision checks
- typed GUI project persistence with missing-model warnings and editable-state round-trip
- dual-lane compute backend with lane-status reporting and active cancel support
- deterministic renderless `rs_cam_viz` regression harness in CI
- controller-first GUI architecture with canonical operation metadata and split compute/controller modules
- shared adaptive support module used by both 2D and 3D adaptive search/control code

## Current priorities

- **Tri-dexel simulation** — Phases 1–6 complete (core types, stamping, mesh extraction, viz wiring, multi-setup carry-forward, side-face grids). Design doc: `architecture/TRI_DEXEL_SIMULATION.md`, implementation plan: `planning/VOXEL_SIM_DESIGN.md`
- keep public docs aligned with the actual code surface
- preserve explicit attribution for algorithms, datasets, and runtime assets
- maintain the lint/test gate as the default merge bar

## Recent work (2026-03-22)

### Continuous multi-setup simulation display

Fixed four visual bugs that made the simulation look like separate per-setup runs instead of one continuous process on a block of material:
- **Checkpoint mesh frame**: checkpoint meshes were displayed in global frame without the setup transform — now `load_checkpoint_for_move` applies the same global→local transform as live playback
- **Bottom surface visibility**: `dexel_stock_to_mesh` now generates a bottom-surface mesh (from `ray_bottom`) when bottom cuts exist, so `FromBottom` cuts are visible
- **Playback starts from uncut block**: simulation results now initialize with `current_move=0, playing=true` instead of jumping to the end — the user sees the tool progressively cutting from an uncut block
- **Stock carry-forward in partial re-sim**: `run_simulation_with_ids` now includes all enabled toolpaths from preceding setups as additional groups, so Setup 2 shows Setup 1's residual stock (through-holes, prior cuts)
- Extracted `transform_mesh_to_local_frame` helper for shared mesh frame transform logic

### Tri-dexel Phase 6: Side-face grids and multi-grid mesh

Extended the tri-dexel simulation to support all six cardinal face orientations:
- `DexelGrid::x_grid_from_bounds` and `y_grid_from_bounds` constructors (rays along X/Y, indexed by YZ/XZ)
- `StockCutDirection` extended with `FromFront`, `FromBack`, `FromLeft`, `FromRight`
- Lazy grid initialization: X/Y grids created on first side-face stamp from `stock_bbox`
- Factored out axis-agnostic `stamp_point_on_grid` / `stamp_segment_on_grid` — Z/X/Y grids share the same inner loop with axis decomposition
- `face_up_to_direction` now returns the correct direction for all `FaceUp` variants
- Multi-grid mesh extraction: `dexel_stock_to_mesh` combines Z-grid surface with side-grid surfaces (using `ray_top` heightmap per grid, vertex positions mapped back to world XYZ)
- Checkpoint correctly deep-copies lazily-created side grids
- 15 new tests covering X/Y grid constructors, all four side-face stamp directions, linear segment on Y-grid, multi-grid simulation isolation, checkpoint with side grids, and multi-grid mesh vertex count

### Tri-dexel simulation backend (Phases 1–5)

Replaced the 2.5D heightmap simulation backend with a tri-dexel volumetric
representation throughout the viz crate:
- Core data types: `DexelSegment`, `DexelRay` (SmallVec), `DexelGrid`, `TriDexelStock` (`dexel.rs`, `dexel_stock.rs`)
- Tool stamping and toolpath simulation with `StockCutDirection` (FromTop / FromBottom)
- Mesh extraction via `dexel_stock_to_mesh` producing the same `HeightmapMesh` format (`dexel_mesh.rs`)
- Viz wiring: `SimulationRequest`, `SimulationResult`, `SimCheckpoint`, and `SimulationPlayback` all use `TriDexelStock` instead of `Heightmap`
- Live playback (`update_live_sim`) uses `simulate_toolpath_range` on `TriDexelStock`
- `StockCutDirection` derived from setup's `FaceUp` orientation
- GPU pipeline unchanged — `HeightmapMesh` remains the render format
- **Phase 5: Multi-setup sequential simulation** — `run_simulation_with_all` now simulates ALL setups sequentially on one `TriDexelStock` in the global stock frame. Toolpaths are pre-transformed from each setup's local frame to the global stock-relative frame using `FaceUp`/`ZRotation` inverse transforms (including arc direction correction). `SimulationRequest` carries per-setup `SetupSimGroup`s with direction. Per-boundary direction stored in results enables correct multi-direction live playback scrubbing. Checkpoints at each toolpath boundary support backward scrub across setup transitions.
- 60 core dexel tests + 59 viz tests pass

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

- **tri-dexel contour-tiling mesh** — full surface reconstruction for non-heightmap views (current side-grid mesh uses ray_top heightmap)
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
