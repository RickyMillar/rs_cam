# Review: Depth, Boundary, and TSP

## Scope
Three supporting systems: depth stepping logic, stock boundary management, and rapid optimization.

## Files to examine
- `crates/rs_cam_core/src/depth.rs`
- `crates/rs_cam_core/src/boundary.rs`
- `crates/rs_cam_core/src/tsp.rs`

## What to review

### Depth
- Step-down calculation: total depth ÷ depth_per_pass
- Final pass handling: remainder pass or equal redistribution?
- Integration with heights system (top Z, bottom Z)

### Boundary
- Stock boundary clipping modes: center, inside, outside
- How does the boundary interact with keep-out zones?
- Model boundary vs stock boundary

### TSP (Traveling Salesman for rapids)
- What algorithm? Nearest neighbor? 2-opt improvement?
- Input: set of toolpath segments to reorder
- Solution quality vs runtime tradeoff
- Is it applied to all operations or opt-in?

### Testing & code quality

## Output
Write findings to `review/results/26_depth_boundary_tsp.md`.
