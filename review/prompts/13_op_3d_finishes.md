# Review: 3D Finishing Strategies (Steep/Shallow, Ramp, Spiral, Radial, Horizontal)

## Scope
Five 3D finishing strategies grouped for review.

## Files to examine
- `crates/rs_cam_core/src/steep_shallow.rs`
- `crates/rs_cam_core/src/ramp_finish.rs`
- `crates/rs_cam_core/src/spiral_finish.rs`
- `crates/rs_cam_core/src/radial_finish.rs`
- `crates/rs_cam_core/src/horizontal_finish.rs`
- `crates/rs_cam_core/src/slope.rs` (surface slope analysis)
- CLI and GUI wiring for each

## What to review

### Per-strategy correctness
- **Steep/Shallow**: How is the threshold angle used to split regions? Are waterline (shallow) and parallel (steep) combined cleanly?
- **Ramp finish**: Continuous descent on walls — how is the spiral/ramp path generated?
- **Spiral finish**: Outward spiral from center — center detection, spiral spacing
- **Radial finish**: Radial lines from center — center point, angular spacing
- **Horizontal finish**: Constant-Z raster — how does it differ from waterline?

### Shared patterns
- Do these share code or are they independent implementations?
- Consistent parameter handling (stepover, tool type, direction)?
- Slope analysis: is `slope.rs` shared infrastructure for steep/shallow?

### Completeness
- Are any of these stubs or partially implemented?
- Which tool types does each support?

### Testing & code quality

## Output
Write findings to `review/results/13_op_3d_finishes.md`.
