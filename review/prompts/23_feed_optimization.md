# Review: Feed Optimization

## Scope
Stock-aware feed rate adjustment based on engagement heightmap.

## Files to examine
- `crates/rs_cam_core/src/feedopt.rs`
- How it's invoked from compute worker
- FEATURE_CATALOG limitation: "Limited to fresh-stock, flat-stock workflows"

## What to review

### Algorithm
- How is the stock heightmap built?
- How is engagement computed per move?
- How is feed rate scaled based on engagement?
- Min/max feed rate bounds

### Limitations
- Fresh stock only — what breaks with non-fresh stock?
- Flat stock only — what breaks with mesh-derived stock?
- Which operations support it? Which are disabled?
- Is the limitation fundamental or fixable?

### Integration
- How does the GUI expose this? Always-on or opt-in?
- Does the CLI support it?

### Testing & code quality

## Output
Write findings to `review/results/23_feed_optimization.md`.
