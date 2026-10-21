# `ops/` — the 2.5D and drilling operations

One module per operation, plus the depth-stepping helper they share. Each
module is reached through `compute::execute::execute_operation_annotated` (`pub(crate)`).

## Files

- `mod.rs`, `depth.rs`, `adaptive_shared.rs` — facade, depth stepping, shared
  adaptive helpers. `waterline.rs` — constant-Z contours.
- `pocket.rs`, `profile.rs` (`profile_ring` = the ONE cut ring; tabs measure on it, P5), `face.rs`, `zigzag.rs` — the 2.5D clearing family (a rest pass is a remaining-stock Pocket,
  R3). Pocket, Zigzag, Profile, Trace, Face give Z-free geometry + one level; `compute/execute/layered.rs` steps Z (W3). D3: no per-op public wrapper; a test reads `tests/common/kernels.rs` or `op_fixtures::generated_op`. `face::face_levels` = the ONE Face ladder. P6.0 (D-C): `polygon::offset_polygon` puts each arc join on points ON the arc (`FlattenPolicy::untoleranced`, 0.010 mm); no op takes a chord across a corner (sentry `an_offset_never_cuts_inside_its_corner_p6_0`).
- `drill.rs`, `drill_op.rs`, `drill_metrics.rs` — the drilling family.
- `vcarve.rs`, `inlay.rs`, `chamfer.rs`, `trace_path.rs`, `project_curve.rs` — the engraving
  and curve family. Chamfer: Z-free tip rings + one level from the tool angle (W3.5, Layered).

## Invariants

- A pinned Bottom Z is honoured only by `Adaptive3d`, `UnifiedFinish` and `Waterline`; Pocket, Profile,
  Zigzag, Adaptive 2D refuse it (P3.2). Read `capabilities().floor_owner` before you assume.
- An explicit-depth operation diagnoses a cut below the stock bottom. Another
  depth semantic abstains; it does not pass.
- Drill removal uses the supplied `StockCutDirection`, never the order of a hole's Z
  pair. The R-plane is per hole (`DrillHole::retract_z`, D8); emitter and metrics read it.
- The drill metrics live in `drill_metrics.rs` and reach the report as
  `drill_summaries`; the gates that read them live in `tool_load/`.
- Each generator picks its `LadderMode` at the call site; the registry field `ladder_includes_top` states it per op, so change both together. `depth.rs` holds the crate's ONE Z-ladder, `z_ladder` (`floor_ladder`, P3.2, is its 2.5D top-to-floor form), with two named arms
  (CUT-14): `EvenRedistributed` for 2.5D (equal passes,
  `total / ceil(total / per_pass)`, top excluded) and `ConstantStep` for
  finishing (constant step from the top, top included). Build no third.
- `ops/trace_path.rs` is the follow-path OPERATION (`trace/` holds the records). P1: on
  the line it cuts an open path with no chord; offset, closed loops only. P1b: leads on closed loops only.
- `inlay.rs` (R2): `Inlay` = pocket V walls only; `InlayPlug` = mirrored plug,
  own project, start depth `pocket_depth − glue_gap` (module doc derives it).

## Sentries

- `cargo test -p rs_cam_core -q --test drill_op_step3 --test drill_flip_removal_g_drillflip --test a_drill_hole_is_cut_once_g_drilltwice`
- `cargo test -p rs_cam_core -q --test project_curve_depth_sign --test chamfer_bevel_sits_on_the_edge_g_chamferside --test chamfer_inside_edge_bevels_into_the_material_g_chamferinside` (G-CHAMFERSIDE: the bevel on the sim stock; G-CHAMFERINSIDE, about 6 s: a pocket wall and a hole)
- `cargo test -p rs_cam_core -q --test depth_beyond_stock_core_g_depthstockcore --test waterline_auto_ladder_is_model_top_to_bottom_r3 --test waterline_respects_a_vertical_wall_r9`
- `cargo test -p rs_cam_core -q --test inlay_pocket_and_plug_fit_g_inlaysplit --test vcarve_cuts_nothing_outside_the_outline_g_vcarvelead` (G-INLAYSPLIT, about 7 s: walls only, mirror, fit at 60° and 90°, floor recipe; G-VCARVELEAD, about 7 s: no lead, nothing cut outside the outline)
