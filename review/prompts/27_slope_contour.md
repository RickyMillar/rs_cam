# Review: Slope & Contour Analysis

## Scope
Surface analysis utilities used by 3D operations.

## Files to examine
- `crates/rs_cam_core/src/slope.rs`
- `crates/rs_cam_core/src/contour_extract.rs`
- `crates/rs_cam_core/src/fiber.rs` (fiber/grain analysis)
- Usage in steep_shallow, waterline, pencil

## What to review

### Slope analysis
- How is surface slope computed? Per-triangle normal angle?
- Steep vs shallow classification threshold
- Is it a per-point or per-region classification?

### Contour extraction
- What contours are extracted? Z-level slices? Surface features?
- How are contours connected from scattered points?
- Open vs closed contour handling

### Fiber analysis
- What is this? Wood grain direction awareness?
- How is it used in operations?
- Is it actually wired to anything?

### Testing & code quality

## Output
Write findings to `review/results/27_slope_contour.md`.
