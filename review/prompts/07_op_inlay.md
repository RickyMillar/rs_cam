# Review: Inlay Operation

## Scope
Inlay operation — generates matching male and female toolpaths for wood inlays.

## Files to examine
- `crates/rs_cam_core/src/inlay.rs`
- V-bit geometry interaction
- CLI wiring (inlay command)
- GUI wiring

## What to review

### Correctness
- Male vs female path generation: are they geometrically complementary?
- Half-angle parameter: does it match V-bit geometry correctly?
- Depth calculation: do male and female depths produce a flush fit?
- Glue gap / tolerance: is there any allowance?

### Edge cases
- Complex shapes with islands
- Very small features that can't be inlayed
- Depth exceeds material thickness

### Integration
- CLI produces two output files — how does GUI handle this (two toolpaths? one with sub-paths?)
- End-to-end wiring

### Testing & code quality

## Output
Write findings to `review/results/07_op_inlay.md`.
