# Review: Pocket Operation

## Scope
The 2.5D pocket clearing operation — contour and zigzag patterns.

## Files to examine
- `crates/rs_cam_core/src/pocket.rs`
- Polygon offset logic in `crates/rs_cam_core/src/polygon.rs`
- Tests referencing pocket in core and viz
- CLI wiring in `crates/rs_cam_cli/src/`
- GUI config in `crates/rs_cam_viz/src/state/toolpath/configs.rs`
- GUI compute in `crates/rs_cam_viz/src/compute/worker/execute.rs`
- Properties panel: `crates/rs_cam_viz/src/ui/properties/pocket.rs`

## What to review

### Correctness
- Contour pattern: are offsets computed correctly? Does it handle islands?
- Zigzag pattern: raster angle, clipping to boundary, stepover accuracy
- Depth stepping: depth_per_pass logic, final pass depth
- Tool compensation: does it offset by tool radius correctly?
- Climb vs conventional: direction handling

### Edge cases
- Very narrow pockets (< tool diameter)
- Complex polygons with holes/islands
- Zero or negative depth
- Stepover > tool diameter

### Integration
- Full wiring: GUI config → compute → core → dressups → G-code
- CLI parity with GUI parameters

### Testing & code quality
- Test coverage, edge case tests
- unwrap() audit, error paths
- Shared code with profile (offset logic)

## Output
Write findings to `review/results/02_op_pocket.md` with sections: Summary, Issues Found, Suggestions, Test Gaps.
