# Review: G-code Output & Arc Fitting

## Scope
G-code generation for 3 dialects (GRBL, LinuxCNC, Mach3) and arc fitting (G2/G3).

## Files to examine
- `crates/rs_cam_core/src/gcode.rs` (606 LOC)
- `crates/rs_cam_core/src/arcfit.rs`
- Post-processor config in GUI: `crates/rs_cam_viz/src/ui/properties/post.rs`
- Export paths in GUI: `crates/rs_cam_viz/src/io/export.rs`
- CLI G-code output

## What to review

### G-code correctness
- Header/footer per dialect
- G0 (rapid), G1 (feed), G2/G3 (arc) emission
- Spindle on/off (M3/M5), coolant
- Tool change (M6) — is it emitted? Multi-tool jobs?
- Coordinate format: decimal places, absolute vs incremental
- Safe Z / retract behavior
- Multi-setup: M0 pauses between setups

### Dialect differences
- GRBL: what's specific? Line numbers? Modal groups?
- LinuxCNC: tool table, canned cycles?
- Mach3: any Mach3-specific codes?
- Are the differences well-documented in code?

### Arc fitting
- Tolerance: how tight?
- Algorithm: least-squares fit? geometric?
- G2 vs G3 direction detection
- Center point calculation (IJK format)
- Does it handle 3D arcs or XY only?

### High feedrate mode
- Replaces G0 with G1 at high feed — why? (some controllers jerk on G0)
- Is the feedrate configurable?

### Edge cases
- Empty toolpath → empty G-code?
- Very long toolpaths (millions of lines)
- Pre/post G-code fields — FEATURE_CATALOG says "not emitted" — verify

### Testing & code quality

## Output
Write findings to `review/results/22_gcode_output.md`.
