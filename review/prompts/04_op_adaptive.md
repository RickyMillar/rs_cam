# Review: Adaptive Clearing (2.5D)

## Scope
The 2.5D adaptive clearing algorithm — the largest single algorithm file in the project (2383 LOC).

## Files to examine
- `crates/rs_cam_core/src/adaptive.rs` (2383 LOC — read thoroughly)
- `crates/rs_cam_core/src/adaptive_shared.rs` (shared with adaptive3d)
- Tests in those files and any integration tests
- CLI wiring, GUI compute, GUI config

## What to review

### Algorithm correctness
- Engagement angle tracking: is constant engagement maintained?
- Spiral / trochoidal path generation
- Slot clearing pre-pass (if enabled)
- Min cutting radius: arc blending behavior
- Tolerance parameter: how does it affect path quality vs speed?
- Rest material detection between passes

### Performance
- Is this the bottleneck for large operations?
- Are there unnecessary allocations or copies?
- Rayon parallelism used here?

### Edge cases
- Very narrow slots
- Complex multi-island pockets
- Stepover near tool diameter
- Deep cuts with many depth passes

### Shared code with adaptive3d
- What's in adaptive_shared.rs? Is the split clean?
- Any duplication between 2D and 3D adaptive?

### Testing & code quality
- Test coverage for a 2383 LOC file
- unwrap() count and locations
- Comments on non-obvious algorithm steps

## Output
Write findings to `review/results/04_op_adaptive.md` with sections: Summary, Algorithm Review, Issues Found, Performance Notes, Test Gaps.
