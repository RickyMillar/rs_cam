# Review: Profile Operation

## Scope
The 2.5D profile (contour following) operation — inside/outside, with tabs and dogbone support.

## Files to examine
- `crates/rs_cam_core/src/profile.rs`
- Tab logic (grep for `tab` in core)
- Dogbone logic (grep for `dogbone` in core)
- `crates/rs_cam_core/src/polygon.rs` (offset)
- CLI and GUI wiring (same pattern as other ops)
- Properties panel for profile config

## What to review

### Correctness
- Inside vs outside profile: offset direction, tool compensation
- Tab placement: even distribution, correct height, tab shape
- Dogbone overcuts: corner detection, quarter-circle radius
- Depth stepping with tabs (tabs only on final pass?)
- Lead-in/lead-out arcs

### Edge cases
- Open contours vs closed contours
- Very small features (< tool diameter)
- Tabs on short segments
- Dogbone on obtuse vs acute corners

### Integration
- End-to-end wiring
- G41/G42 compensation — FEATURE_CATALOG says "In Control" is in UI but not emitted. Verify.

### Testing & code quality
- Coverage, especially tabs and dogbone
- Error handling

## Output
Write findings to `review/results/03_op_profile.md` with sections: Summary, Issues Found, Suggestions, Test Gaps.
