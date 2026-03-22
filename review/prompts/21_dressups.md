# Review: Dressups (Toolpath Modifiers)

## Scope
All toolpath post-processing modifiers: heights, ramp/helix entry, dogbone, lead-in/out, link moves, arc fitting, feed optimization, TSP ordering.

## Files to examine
- `crates/rs_cam_core/src/dressup.rs` (framework)
- `crates/rs_cam_core/src/arcfit.rs` (arc fitting)
- `crates/rs_cam_core/src/feedopt.rs` (feed optimization)
- `crates/rs_cam_core/src/tsp.rs` (rapid ordering)
- Grep for `dressup`, `ramp`, `helix`, `dogbone`, `lead_in`, `lead_out`, `link_move`, `tab` across core
- How dressups are applied in compute worker execute.rs

## What to review

### Framework
- Is there a dressup trait/interface or is each modifier ad-hoc?
- Application order: does order matter? Is it documented/enforced?
- Can dressups be composed arbitrarily or are there incompatible combinations?

### Individual modifiers
- **Heights**: clearance, retract, feed, top, bottom — applied correctly to all operations?
- **Ramp entry**: spiral down into material — angle, radius, depth per turn
- **Helix entry**: helical plunge — similar to ramp but circular
- **Dogbone**: quarter-circle at inside corners — correct radius, correct corner detection
- **Lead-in/out**: arc entry/exit — radius, angle, feed rate during lead
- **Link moves**: replace short rapids with feeds — distance threshold
- **Tabs**: holding tabs on profiles — placement, shape, height
- **Arc fitting**: convert line segments to G2/G3 arcs — tolerance, quality
- **Feed optimization**: adjust feed based on engagement — heightmap approach
- **TSP ordering**: minimize rapids — solution quality, runtime

### Edge cases
- Dressups on empty or single-point toolpaths
- Arc fitting on already-arc segments
- Tabs on very short contours
- Feed optimization with no stock data

### Testing & code quality

## Output
Write findings to `review/results/21_dressups.md`.
