# Review: Waterline Operation

## Scope
Constant-Z waterline finishing — horizontal slices through 3D mesh at regular Z intervals.

## Files to examine
- `crates/rs_cam_core/src/waterline.rs` (410 LOC)
- Contour extraction: `crates/rs_cam_core/src/contour_extract.rs`
- Push cutter: `crates/rs_cam_core/src/pushcutter.rs`
- CLI and GUI wiring

## What to review

### Algorithm correctness
- Z-level selection: stepdown from top to bottom
- At each Z, how is the contour extracted? Mesh slicing or push-cutter contact?
- Contour ordering: inside-out? outside-in? climb direction?
- Tool offset at each level
- Linking between Z levels (ramp or retract?)

### Edge cases
- Flat areas (no contour change between levels)
- Overhangs / undercuts
- Very steep walls (many levels, minimal contour change)
- Islands at certain Z levels

### Integration & testing

## Output
Write findings to `review/results/11_op_waterline.md`.
