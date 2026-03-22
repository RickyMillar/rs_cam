# Review: Rest Machining Operation

## Scope
Rest machining — clears material left by a previous larger tool.

## Files to examine
- `crates/rs_cam_core/src/rest.rs`
- How "previous tool" swept volume is computed
- CLI and GUI wiring
- How stock source (Fresh vs Remaining) interacts

## What to review

### Correctness
- How is the previous tool's swept area calculated?
- Does it correctly identify remaining material?
- Does it handle different tool type combinations (flat→ball, flat→flat smaller)?
- Depth handling: does it respect the previous operation's depths?

### Edge cases
- Previous tool same size as current tool (nothing to rest-machine)
- Previous tool smaller than current (everything is rest material)
- Multiple rest passes in sequence
- 3D mesh context vs 2D boundary context

### Integration
- How does the GUI let user specify "previous tool"?
- Is there validation that the previous tool actually exists?

### Testing & code quality

## Output
Write findings to `review/results/06_op_rest.md`.
