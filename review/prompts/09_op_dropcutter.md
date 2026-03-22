# Review: Dropcutter (3D Finish)

## Scope
The drop-cutter 3D finishing algorithm — projects tool onto mesh surface.

## Files to examine
- `crates/rs_cam_core/src/dropcutter.rs` (263 LOC)
- Tool edge_drop methods in `crates/rs_cam_core/src/tool/` (all 5 families)
- K-d tree usage (kiddo) for spatial queries
- CLI wiring (drop-cutter command is the most full-featured CLI command)
- GUI wiring

## What to review

### Algorithm correctness
- Grid-based XY sampling: stepover determines grid spacing
- For each XY point, drop tool onto mesh: find highest Z where tool contacts surface
- Per-tool-type edge_drop: flat (simple), ball (sphere tangent), bullnose, vbit, tapered_ball
- Triangle intersection: does it check all triangles or use spatial index?

### Performance
- This was mentioned in perf feedback (sqrt-per-row regresses small tools)
- K-d tree query pattern: rebuild frequency, query efficiency
- Is rayon used for parallel row computation?

### Edge cases
- Mesh with holes / non-manifold edges
- Overhangs (tool can't reach)
- Very fine stepover on large mesh
- Tool larger than feature being machined

### Integration
- CLI has extensive options (view, simulate, holder collision)
- GUI wiring completeness

### Testing & code quality

## Output
Write findings to `review/results/09_op_dropcutter.md`.
