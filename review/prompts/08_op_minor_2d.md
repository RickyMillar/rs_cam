# Review: Minor 2D Operations (Zigzag, Trace, Drill, Chamfer)

## Scope
Four smaller 2.5D operations grouped together for review efficiency.

## Files to examine
- `crates/rs_cam_core/src/zigzag.rs` (if separate from pocket zigzag)
- `crates/rs_cam_core/src/trace.rs`
- `crates/rs_cam_core/src/drill.rs` (grep for drill)
- `crates/rs_cam_core/src/chamfer.rs`
- CLI and GUI wiring for each

## What to review

### Per-operation correctness
- **Zigzag**: Is this distinct from pocket-zigzag? Raster angle, clipping, stepover
- **Trace**: What does it trace? Contour following without offset? What's the use case?
- **Drill**: Point drilling / peck drilling? Retract cycles? Spot drill support?
- **Chamfer**: Edge chamfering with what tool types? Depth from edge detection?

### Shared concerns
- Are these operations thin wrappers around shared utilities?
- Do they all handle heights, boundaries, dressups consistently?
- Are they all wired end-to-end?

### Testing & code quality
- Coverage for each
- Any that seem incomplete or stub-like?

## Output
Write findings to `review/results/08_op_minor_2d.md`.
