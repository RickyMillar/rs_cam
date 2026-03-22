# Review: Pencil & Scallop Operations

## Scope
Two 3D finishing strategies: pencil (crease finishing) and scallop (constant-cusp-height finishing).

## Files to examine
- `crates/rs_cam_core/src/pencil.rs`
- `crates/rs_cam_core/src/scallop.rs`
- `crates/rs_cam_core/src/scallop_math.rs`
- CLI and GUI wiring

## What to review

### Pencil
- How are creases / concave edges detected on the mesh?
- Does it trace along the crease or across it?
- Stepover meaning in this context
- Ball nose tool assumed?

### Scallop
- Scallop height → variable stepover calculation
- Is it curvature-aware (adapting stepover to local surface curvature)?
- What's in scallop_math.rs vs scallop.rs?
- Scallop direction (climb/conventional)

### Edge cases
- Flat surfaces (infinite scallop stepover? capped?)
- Sharp creases (pencil) on faceted STL mesh
- Very high scallop height tolerance

### Testing & code quality

## Output
Write findings to `review/results/12_op_pencil_scallop.md`.
