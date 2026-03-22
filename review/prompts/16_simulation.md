# Review: Tri-Dexel Simulation

## Scope
The volumetric material removal simulation system — recently completed through Phase 6.

## Files to examine
- `crates/rs_cam_core/src/simulation.rs` (1132 LOC — coordinator)
- `crates/rs_cam_core/src/dexel.rs` (624 LOC — ray segment representation)
- `crates/rs_cam_core/src/dexel_stock.rs` (stock grid)
- `crates/rs_cam_core/src/dexel_mesh.rs` (mesh extraction)
- Architecture doc: `architecture/TRI_DEXEL_SIMULATION.md`
- Planning doc: `planning/VOXEL_SIM_DESIGN.md`
- GUI simulation state: `crates/rs_cam_viz/src/state/simulation.rs`
- GUI sim rendering: `crates/rs_cam_viz/src/render/sim_render.rs`
- Compute worker simulation: `crates/rs_cam_viz/src/compute/worker/`

## What to review

### Dexel representation
- Tri-dexel grid: 3 orthogonal ray planes (XY, XZ, YZ)
- Ray segment representation: how are cuts stored per ray?
- Lazy grid initialization from stock bbox
- Memory footprint for typical resolution (0.25mm)

### Stamp operations
- How is tool geometry stamped into dexel grid?
- Per-tool-type stamping correctness
- Axis-agnostic stamping (works for all 6 faces)

### Mesh extraction
- How are dexels converted back to a renderable mesh?
- Vertex mapping across grids
- Side face generation
- Closed solid mesh requirement

### Multi-setup
- Material carry-forward between setups
- Coordinate frame transforms for different FaceUp orientations
- Checkpoint system for scrubbing

### Performance
- Resolution vs speed tradeoff
- Memory usage
- Any parallelism in stamping or mesh extraction?

### Testing
- 60 core dexel tests — are they comprehensive?
- Multi-setup test coverage

## Output
Write findings to `review/results/16_simulation.md` with sections: Architecture Review, Dexel Correctness, Mesh Extraction, Multi-Setup, Performance, Test Gaps.
