# Review: VCarve Operation

## Scope
V-carving / engraving operation using V-bit tools.

## Files to examine
- `crates/rs_cam_core/src/vcarve.rs`
- V-bit tool geometry in `crates/rs_cam_core/src/tool/vbit.rs`
- CLI and GUI wiring

## What to review

### Correctness
- Medial axis / voronoi approach or simpler depth-from-width?
- V-bit angle → depth calculation
- Max depth limiting
- Narrow vs wide feature handling
- Sharp corners: does depth go to zero correctly?

### Edge cases
- Text / complex SVG paths
- Very thin strokes
- Intersecting paths
- Tool angle vs feature width mismatch

### Integration
- End-to-end wiring
- Does it only work with V-bit tools? What happens if user selects flat end mill?

### Testing & code quality

## Output
Write findings to `review/results/05_op_vcarve.md`.
