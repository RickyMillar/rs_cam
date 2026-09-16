# I10 — small confirmed utilities
Verdict per sub-pair (5 pairs):

## Pair 1 — slugify (TRUE_DUP)
- `crates/rs_cam_viz/src/app/export.rs` L260-271 vs
  `crates/rs_cam_viz/src/ui/export_wizard.rs` L304-315.
- `diff` of both regions: byte-identical (12 lines, chars→'_').

## Pair 2 — components/compare vs feeds/compare (SIBLING)
- Candidate: `ui/components/compare.rs` L85-92 (`mrr_row`) vs
  `ui/feeds/compare.rs` L697-709 (`rail_mrr_row`).
- Drift is intentional: `mrr_row` uses `ui.horizontal` + `ui.label`;
  `rail_mrr_row` uses `ui.horizontal_wrapped` + `.wrap()` labels, with doc
  comment "wrapped like its neighbours" (feeds rail layout).
- Callers: `mrr_row` re-exported via `ui/components/mod.rs:43` (readiness
  panel / rollup surfaces); `rail_mrr_row` only `feeds/compare.rs:370`.
  `components/compare.rs` is the documented shared home (module doc, Wave 1 IA
  cleanup). Not debt; at most reuse `mrr_row` if the rail ever accepts
  non-wrapped layout.

## Pair 3 — sim_render vs mesh_render vertex layouts (SIBLING)
- Candidate: `render/sim_render.rs` L15-40 (`ColoredMeshVertex`, 3 attributes)
  vs `render/mesh_render.rs` L15-35 (`MeshVertex`, 2 attributes).
- Intentional parallel: different vertex formats (position+normal vs
  position+normal+color). mesh_render already reuses
  `sim_render::ColoredMeshVertex` (no color duplication);
  `render/height_planes.rs` also uses `ColoredMeshVertex`.
- Only boilerplate shared is the `VertexBufferLayout` construction shape;
  a macro/helper would save ~15 lines. Not worth merging.

## Pair 4 — rs_cam_cli sweep.rs vs job.rs (DRIFTED_DUP)
- Candidate: `src/sweep.rs` L368-440+ `Serializable*` mirror structs vs
  `src/job.rs` `JobFile`/`JobConfig`/`OperationDef`/`SetupDef`/`ToolDef`.
- Same purpose: round-trip of the job-file schema (job.rs = Deserialize,
  sweep.rs = Serialize for baseline job files in sweeps), plus a
  `From<&job::JobFile>` converter (sweep.rs:489).
- **Authoritative side: `crates/rs_cam_cli/src/job.rs`** (the parse schema;
  sweep output must be re-readable by it).
- Drift: field attributes differ per direction (`serde(default)` vs
  `skip_serializing_if = "Option::is_none"`); sync is manual — adding a field
  to `OperationDef` silently drops it from sweep baseline files. Fields
  currently match (op_type/input/tool/setup + 8 optionals + pocket group).

## Pair 5 — benches rolling_field (TRUE_DUP)
- Candidate: `benches/classification.rs` L62-95 vs `benches/hot_paths.rs`
  L97-126. Bodies identical (only one explanatory comment line differs).
- hot_paths.rs doc admits it: "the same generator shape classification.rs
  uses". Callers: classification L132/L152, hot_paths L715.

## Drift / differences (pair 4)
- sweep.rs `Serializable*` structs hand-mirror job.rs; drift risk = schema
  desync on new job-file fields. Cleanup: derive `Serialize` on job.rs types
  (attributes are direction-compatible) and delete `Serializable*` + `From`
  impl in sweep.rs L360-540.

## Proposed cleanup
- Pair 1: home `crates/rs_cam_viz/src/ui/str_util.rs` (new small module,
  `pub(crate) fn slugify`); both callers import it. risk: low.
  proof test: unit test `slugify("A b/c") == "A_b_c"`, `_` for non-ASCII,
  keeps `-_.` alnum.
- Pair 4: home `crates/rs_cam_cli/src/job.rs` (derive Serialize on existing
  types); sweep.rs keeps only `toml::to_string(&base)`. risk: low-med.
  proof test: round-trip — serialize a `JobFile` from a sweep baseline and
  re-parse it with `job::JobFile`, assert fields preserved (extend existing
  CLI parity/replay test).
- Pair 5: home `crates/rs_cam_core/benches/support/mod.rs` (bench-only
  helper, `#![allow]`-free, clippy-clean) with `rolling_field` +
  `load_terrain`/`repo_root` if identical. risk: low.
  proof test: `cargo bench --no-run -p rs_cam_core` (both benches compile
  with identical outputs before/after).
- Pairs 2, 3: no cleanup — intentional parallels; document if a third copy
  of the MRR row or vertex layout ever appears.
