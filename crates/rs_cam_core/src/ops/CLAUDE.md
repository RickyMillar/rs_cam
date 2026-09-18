# `ops/` — the 2.5D and drilling operations

One module per operation, plus the depth-stepping helper they share. Each
module is reached through `compute::execute_operation_annotated`.

## Files

- `mod.rs`, `depth.rs`, `adaptive_shared.rs` — facade, depth stepping, shared
  adaptive helpers.
- `pocket.rs`, `profile.rs`, `face.rs`, `zigzag.rs`, `rest.rs` — the 2.5D
  clearing family. `waterline.rs` — closed contours at constant Z.
- `drill.rs`, `drill_op.rs`, `drill_metrics.rs` — the drilling family.
- `vcarve.rs`, `inlay.rs`, `chamfer.rs`, `trace_path.rs`, `project_curve.rs`
  — the engraving and curve family.

## Invariants

- A pinned Bottom Z is honoured only by `Adaptive3d`, `UnifiedFinish` and
  `Waterline`. Query `OperationType::honors_pinned_bottom_z()` before you assume.
- An explicit-depth operation diagnoses a cut below the stock bottom. Another
  depth semantic abstains; it does not pass.
- Drill removal uses the supplied `StockCutDirection`. Never infer the
  direction from the order of a hole's Z pair.
- The drill metrics live in `drill_metrics.rs` and reach the report as
  `drill_summaries`; the gates that read them live in `tool_load/`.
- `depth.rs` holds the crate's ONE Z-ladder, `z_ladder`, with two named arms
  (CUT-14): `EvenRedistributed` for 2.5D (equal passes,
  `total / ceil(total / per_pass)`, top excluded) and `ConstantStep` for
  finishing (constant step from the top, top included). Build no third.
- `ops/trace_path.rs` is the follow-path OPERATION and `trace/` holds the
  toolpath records. The two read alike and are not the same; do not confuse
  them.

## Sentries

- `cargo test -p rs_cam_core -q --test drill_op_step3`
- `cargo test -p rs_cam_core -q --test drill_flip_removal_g_drillflip`
- `cargo test -p rs_cam_core -q --test depth_beyond_stock_core_g_depthstockcore`
- `cargo test -p rs_cam_core -q --test project_curve_depth_sign`
- `cargo test -p rs_cam_core -q --test waterline_auto_ladder_is_model_top_to_bottom_r3 --test waterline_respects_a_vertical_wall_r9`
