# Review: Toolpath IR (Intermediate Representation)

## Scope
The toolpath data structure that sits between operation generation and post-processing/output.

## Files to examine
- `crates/rs_cam_core/src/toolpath.rs` (342 LOC)
- How every operation produces toolpaths (grep for `Toolpath` or `ToolpathSegment` in operation files)
- How dressups consume/transform toolpaths
- How G-code output consumes toolpaths
- How simulation consumes toolpaths
- GUI toolpath state: `crates/rs_cam_viz/src/state/toolpath/`

## What to review

### Data structure
- What does a Toolpath contain? (moves, metadata, stats?)
- Move types: rapid, feed, arc (G0/G1/G2/G3)?
- Does it carry feed rate per move or globally?
- Does it carry tool info or is that external?
- Is it a flat Vec<Move> or a tree/segment structure?

### Sufficiency
- Can all 22 operations express their output through this IR?
- Can all dressups transform it without losing information?
- Does simulation need anything the IR doesn't provide?
- Are there operations that bypass the IR or bolt on extra data?

### Boundary role
- Per architecture guardrails: "treat the toolpath IR as the boundary between planning and post-processing"
- Is this actually respected? Or do G-code / simulation reach back into operation data?

### Serialization
- Project save/load round-trips toolpath data — how?
- Is the serialized form compact?

### Testing & code quality

## Output
Write findings to `review/results/20_toolpath_ir.md`.
