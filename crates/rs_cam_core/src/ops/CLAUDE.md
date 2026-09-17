# `ops/` — the 2.5D and drilling operations

One module per operation, plus the depth-stepping helper they share. Each
module is reached through `compute::execute_operation_annotated`.

## Files

- `mod.rs`, `depth.rs`, `adaptive_shared.rs` — the facade, depth stepping for
  a multi-pass operation, and the shared adaptive helpers.
- `pocket.rs`, `profile.rs`, `face.rs`, `zigzag.rs`, `rest.rs` — the 2.5D
  clearing family.
- `drill.rs`, `drill_op.rs`, `drill_metrics.rs` — the drilling operation, its
  first-class data model and the per-peck metrics.
- `vcarve.rs`, `inlay.rs`, `chamfer.rs`, `trace_path.rs`,
  `project_curve.rs` — the engraving and curve family.
- `waterline.rs` — closed contours at constant Z.

## Invariants

- A pinned Bottom Z is honoured only by `Adaptive3d`, `UnifiedFinish` and
  `Waterline`. Query `OperationType::honors_pinned_bottom_z()`; do not assume
  a 2.5D operation follows it.
- An explicit-depth operation diagnoses a cut below the stock bottom. Another
  depth semantic abstains; it does not pass.
- Drill removal uses the supplied `StockCutDirection`. Never infer the
  direction from the order of a hole's Z pair.
- The drill metrics live in `drill_metrics.rs` and reach the report as
  `drill_summaries`. The gates that read them live in `tool_load/`.

## Sentries

- `cargo test -p rs_cam_core -q --test drill_op_step3`
- `cargo test -p rs_cam_core -q --test drill_flip_removal_g_drillflip`
- `cargo test -p rs_cam_core -q --test depth_beyond_stock_core_g_depthstockcore`
- `cargo test -p rs_cam_core -q --test project_curve_depth_sign`

## Do not

- `ops/trace_path.rs` is the follow-path OPERATION. `trace/` holds the
  toolpath records. The two read alike and are not the same.
