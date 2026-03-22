# Review: Project Curve Operation

## Scope
Projects a 2D curve onto a 3D mesh surface.

## Files to examine
- `crates/rs_cam_core/src/project_curve.rs`
- Interaction with dropcutter (likely uses similar Z-projection)
- CLI and GUI wiring

## What to review

### Correctness
- How is the 2D curve projected? Vertical ray from each curve point to mesh?
- Sampling resolution along the curve
- Does it handle curves that go off the mesh edge?
- Tool compensation: is the projected curve offset by tool geometry?

### Use cases
- Engraving on curved surfaces?
- Following a design curve on a 3D part?

### Edge cases
- Curve crosses a hole in the mesh
- Multiple Z intersections (overhangs)
- Very dense or very sparse curve sampling

### Integration & testing

## Output
Write findings to `review/results/14_op_project_curve.md`.
