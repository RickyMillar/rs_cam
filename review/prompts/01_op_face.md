# Review: Face Operation

## Scope
The face surfacing operation in rs_cam_core.

## Files to examine
- `crates/rs_cam_core/src/face.rs`
- Any tests referencing face in `crates/rs_cam_core/`
- CLI wiring: grep for `face` in `crates/rs_cam_cli/src/`
- GUI wiring: grep for `Face` in `crates/rs_cam_viz/src/compute/worker/execute.rs`
- GUI config: grep for `Face` in `crates/rs_cam_viz/src/state/toolpath/`

## What to review

### Correctness
- Does the algorithm produce correct face passes for the full stock surface?
- Are stepover, depth_per_pass, and boundary handled correctly?
- Does it work with all 5 tool types or only flat?
- Edge cases: stock wider than tool, zero depth, very small stepover

### Integration
- Is the operation wired end-to-end from GUI config → compute → toolpath → G-code?
- Does the CLI expose all the same parameters the GUI does?
- Are heights (clearance, retract, feed, top, bottom) applied correctly?

### Testing
- What tests exist? Are they sufficient?
- Are edge cases tested?

### Code quality
- unwrap() usage, error propagation
- Duplication with other operations (pocket, zigzag)
- Documentation / comments on non-obvious logic

## Output
Write findings to `review/results/01_op_face.md` with sections: Summary, Issues Found, Suggestions, Test Gaps.
