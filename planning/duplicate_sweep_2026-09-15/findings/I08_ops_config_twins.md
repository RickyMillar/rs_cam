# I08 — ops/config twins: per-op helpers, config enums, drill-op accessors, finding records
Verdict: MIXED — per sub-pair below (2 FALSE_POSITIVE, 1 SIBLING, 1 DRIFTED_DUP, label-helper family TRUE_DUP)

## Pair A — `resample_polyline`: pencil.rs:705-746 ↔ project_curve.rs:80-141 (0.9216) — DRIFTED_DUP
- P3 copy `crates/rs_cam_core/src/pencil.rs:712` (`pub(crate)`, also called by `crease_paths.rs:222`,
  `pencil.rs:2131`); added c73b38c3 2026-07-03 replacing dihedral `sample_chain`
  (`pencil_dihedral.rs:344,558` name it canonical). Pinned by `pencil.rs:2574`.
- P2 copy `crates/rs_cam_core/src/project_curve.rs:86` (private, caller `:341` ring spacing); added
  a1a8c31b 2026-03-20. Pinned by in-file tests `:406-451` (exact sample coords). Both sides are
  `crate::geo` nalgebra points (`geo.rs:6-8`); same walk-and-emit algorithm — hand-trace shows
  project_curve's exact-point tests (0,3,6,9,10) pass under the pencil algorithm too.

## Pair B — `ProjectSide` (project_curve.rs:28-39) ↔ `ProjectCurveSide` (operation_configs.rs:1583-1596) (0.9434) — SIBLING
- Wire vs engine layering by design: `operation_configs.rs` enums derive `serde` + `label()` for UI;
  `project_curve.rs` enums are engine params. Explicit adapter maps config→engine at
  `compute/execute.rs:2072-2090` (same pairing for the `*Direction` enums, :2072-2076). Config flows
  via `OperationConfig::ProjectCurve` (`execute.rs:3635`, `session/compute.rs:2661,2940`,
  `viz/ui/properties/operations/project.rs:80-84`). Engine enums pinned by
  `tests/project_curve_depth_sign.rs:33`, `capability_link_moves_safety.rs:1031`,
  `param_sweep.rs:2184`, `project_curve_deviation.rs:338`. Unifying couples engine to wire names.

## Pair C — `OpData::annotated` (drill_op.rs:160-182) ↔ `ToolpathComputeResult::annotated` (session/mod.rs:942-966) (0.9287) — FALSE_POSITIVE
- Facade delegation: `session/mod.rs:948` is `self.op_data.annotated()` (same for `drill_op`,
  `is_drill_op`). ~20+ facade callers (`gcode/mod.rs:421,575`, `session/compute.rs:3983,4725`,
  viz `controller/results_parity_tests.rs:212`, cli `smoke.rs:843`). drill_op.rs is the sole
  implementation of the dual-representation invariant. No action.

## Pair D — `ClippedBandFinding` (compute/config.rs:1124-1182) ↔ `ClippedBand` (unified_finish.rs:1007-1035) (0.9517) — FALSE_POSITIVE
- Aggregate vs per-region record of the same C8 instrument: `ClippedBand`+`BandHeightClip` live in
  `UnifiedFinishReport`; `clipped_band_finding()` (unified_finish.rs:~1036) folds them into ONE
  Copy finding on the exec result (`execute.rs:98-99,2808`, `narrate.rs:126`). One side builds the
  other; near-identical docs deliberate ("messages read alike"). Pinned by
  `tests/findings_transport_join_h21.rs:97`. Different granularities; no action.

## Pair E — `runtime_annotations_to_labels`: ramp_finish.rs:471-479 ↔ spiral_finish.rs:118-126 (0.9529) — TRUE_DUP
## Pair F — same helper: adaptive3d/path.rs:1653-1661 ↔ adaptive/path.rs:1603-1611 (0.9815) — TRUE_DUP (engines SIBLING)
- Six copies, byte-identical modulo the annotation type: also `scallop.rs:1863`, `pencil.rs:698`
  (adaptive pair is `pub(super)`). Every annotation struct is `{move_index, event}` with
  `event.label() -> String` (`adaptive/mod.rs:214`, `adaptive3d/mod.rs:397`, `ramp_finish.rs:86`,
  `spiral_finish.rs:68`, `scallop.rs:112`, `pencil.rs:269`). Callers: the `*_annotated` wrappers
  (`adaptive/mod.rs:289`, `adaptive3d/mod.rs:524`, `ramp_finish.rs:889`, `spiral_finish.rs:319`,
  `scallop.rs:2813`, `pencil.rs:2544`). Labels pinned by `tests/agent_search_coverage.rs:252`,
  `tests/wanaka_z_layer_render.rs:240`; structured forms feed `compute/annotate.rs:744,808`.
- The 2D/3D adaptive engines and ramp/spiral strategies are intentional SIBLINGs; only this
  7-line projection helper is copy-paste. Same purpose, same behavior → merge safe.

## Drift / differences (Pair A — authoritative: pencil.rs)
- pencil: spacing guard `<= 1e-6`, zero-seg skip `< 1e-9`, last-point dedup on XY only `@1e-6`.
- project_curve: guard `spacing <= 0.0` (tiny positive spacing → `len/spacing` capacity blow-up),
  zero-seg `< 1e-12`, dedup full-2D `@1e-9`, plus a private P2 `polyline_length` duplicating
  `geo.rs:255`. No test depends on the epsilon choice.

## Proposed cleanup
- Pair A — home: `crates/rs_cam_core/src/geo.rs` (generic resample over nalgebra points, pencil
  guard semantics; drop project_curve's copy + private P2 `polyline_length`). risk: low-med.
  proof: pencil.rs:2574 + project_curve.rs:406-451 green on the shared fn; gate
  `cargo test -p rs_cam_core -q` + clippy `-D warnings`.
- Pairs E/F — home: `crates/rs_cam_core/src/compute/spans.rs` (annotation-adjacent): one
  `pub(crate) fn runtime_annotations_to_labels<A: RuntimeLabel>` over a 2-method trait
  (`move_index`, `label`) + six impls. risk: low. proof: agent_search_coverage +
  wanaka_z_layer_render + in-file ramp/spiral tests; gate `cargo test -p rs_cam_core -q`.
- Pairs B, C, D — no action (verified layering / facade / aggregate relationships).
