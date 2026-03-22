# Review: Adaptive 3D (Rough)

## Scope
3D adaptive roughing — constant engagement clearing on mesh surfaces.

## Files to examine
- `crates/rs_cam_core/src/adaptive3d.rs`
- `crates/rs_cam_core/src/adaptive_shared.rs`
- Interaction with `adaptive.rs` (2D version)
- CLI and GUI wiring
- Semantic trace attribution (mentioned in recent commits)

## What to review

### Algorithm correctness
- How does 3D adaptive differ from 2D? Heightmap-driven depth adjustment?
- Engagement tracking on 3D surfaces
- Fine stepdown in heavy engagement areas
- Flat area detection
- Max stay-down distance (spiral compression)
- Region ordering strategies

### Performance
- Large mesh performance
- Memory usage for engagement tracking data structures

### Shared code
- What's shared via adaptive_shared.rs vs duplicated?
- Is the 2D/3D split clean or are there leaky abstractions?

### Semantic trace
- Recent commits added "richer math-stage attribution" — review that tracing

### Edge cases
- Deep pockets in 3D
- Thin walls
- Steep vs shallow regions

### Testing & code quality

## Output
Write findings to `review/results/10_op_adaptive3d.md`.
