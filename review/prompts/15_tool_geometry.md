# Review: Tool Geometry (5 Families)

## Scope
The tool abstraction layer — 5 cutter types with shared trait interface.

## Files to examine
- `crates/rs_cam_core/src/tool/mod.rs` (2284 LOC — the trait + shared logic)
- `crates/rs_cam_core/src/tool/flat.rs`
- `crates/rs_cam_core/src/tool/ball.rs`
- `crates/rs_cam_core/src/tool/bullnose.rs`
- `crates/rs_cam_core/src/tool/vbit.rs`
- `crates/rs_cam_core/src/tool/tapered_ball.rs`
- Tests in each file

## What to review

### Trait design
- What methods does the tool trait expose?
- Is it dyn-compatible? (Previous feedback mentions `?Sized` for dyn-compatible generics)
- Is the trait sufficient for all operations, or do operations type-match on concrete types?

### Per-tool correctness
- **Flat**: Simple cylinder — edge_drop is just radius check
- **Ball**: Hemisphere — edge_drop needs sphere-triangle tangent
- **Bullnose**: Cylinder + torus blend — most complex geometry
- **V-bit**: Cone — angle-based depth
- **Tapered ball**: Cone + sphere tip — combination geometry

### edge_drop accuracy
- This is the core geometric primitive. Each tool must compute Z contact with triangles.
- Are the formulas correct? Cross-reference with `CREDITS.md` algorithm sources.
- Numerical stability: degenerate triangles, grazing angles

### Collision envelope
- Holder/shank geometry: diameter, length, stickout
- Is this used correctly by collision detection?

### Testing
- Are all 5 tool types tested for edge_drop?
- Are degenerate cases tested (zero-area triangles, vertical surfaces)?

## Output
Write findings to `review/results/15_tool_geometry.md` with sections: Trait Design, Per-Tool Review, Numerical Issues, Test Gaps.
