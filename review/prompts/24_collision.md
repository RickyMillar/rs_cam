# Review: Collision Detection

## Scope
Tool holder and shank collision detection against workpiece mesh.

## Files to examine
- `crates/rs_cam_core/src/collision.rs`
- Tool holder/shank geometry in `tool/mod.rs`
- How collision check is triggered from GUI
- Collision visualization in render

## What to review

### Algorithm
- How are holder/shank collisions detected?
- Interpolation between moves or only at move endpoints?
- What geometry is checked against? Raw mesh? Simulation result mesh?
- Collision envelope: how is holder/shank geometry represented?

### Accuracy
- False positives: does it flag collisions that wouldn't happen?
- False negatives: can it miss real collisions?
- Resolution of interpolation between moves

### Integration
- GUI: collision check is a separate action, not part of simulation — why?
- FEATURE_CATALOG says collision is "detected but not rendered" — verify current state
- Are collision results shown in timeline? In viewport?

### Testing & code quality

## Output
Write findings to `review/results/24_collision.md`.
