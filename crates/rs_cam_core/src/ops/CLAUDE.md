# `ops/` — the 2.5D and drilling operations

One module per operation, plus the depth-stepping helper they share. Each
module is reached through `compute::execute::execute_operation_annotated` (`pub(crate)`).

## Files

- `mod.rs`, `depth.rs`, `adaptive_shared.rs` — facade, depth stepping, shared
  adaptive helpers. `waterline.rs` — constant-Z contours; `waterline_level_rings` (the rings, rest and boundary split, W4 `RunFilter::outer_floor_z`: no outer ring at or below the web of a through cut), `helical_waterline` (W1: `chain_links`, the ONE chain rule; `emit_chain` descends along the LOWER ring; `ChainLinkProbe` = the gouge guard's reduced tool; W2 `WallStep`/`thin_wall_chain`; P22-0c-2: `lap_count` holds `CoreZone::max_ring_slope` of a non-centre tool; P29-8: the ring descent is `WATERLINE_RING_DESCENT_DEG`, not an entry angle). The op adapter is `compute/execute/finish_raster.rs`.
- `pocket.rs` (P23d: `pocket_contours_to_sided_toolpath_slot_first` feeds the first ring of each piece, a full slot in fresh stock, at the unlifted feed and holds it as `Move::feed_cap`; the geometry does not move; a rest Pocket's first ring takes the lower feed too, the safe side (lead ruling 2026-11-10), although it is no slot by construction; sentry `a_pocket_feeds_its_slot_first_ring_unlifted_p23d`), `profile.rs` (`profile_ring` = the ONE cut ring; tabs measure on it, P5; P6.3: side `On` reads `Curves` and cuts on the line, `on_line_contours`: every closed ring, and an open path in its drawn direction through `profile_path_open_to_toolpath`, no chord, no climb reversal, no tab (one `TabNote::OpenPath`), no lead (`TRANSFORM_PROFILE`); sentry `profile_on_cuts_on_the_line_p6_3`; P35: `stock_to_leave_radial` moves the tool centre in `profile_path_reported` and `on_line_contours` (Outside out, Inside in, On out of the ring's own area; positive leaves material), a loop that collapses or an open path with a value refuses (`RadialLeaveRefusal`), P35 D: a loop that splits cuts each ring (`ProfileRings`) and warns (`RadialLeaveSplit` → `TabNote::SplitLoop` → `geom.profile_loop_split`); sentry `a_profile_leaves_radial_stock_p35`), `face.rs`, `zigzag.rs` — the 2.5D clearing family (a rest pass is a remaining-stock Pocket,
  R3). Pocket, Zigzag, Profile, Trace, Face give Z-free geometry + one level; `compute/execute/layered.rs` steps Z (W3). D3: no per-op public wrapper; a test reads `tests/common/kernels.rs` or `op_fixtures::generated_op`. `face::face_levels` = the ONE Face ladder. P6.0 (D-C): `polygon::offset_polygon` puts each arc join on points ON the arc (`FlattenPolicy::untoleranced`, 0.010 mm); no op takes a chord across a corner (sentry `an_offset_never_cuts_inside_its_corner_p6_0`).
- `drill.rs`, `drill_op.rs`, `drill_metrics.rs` — the drilling family. `bore.rs` (P6.6) — the Bore helix (a milling op, L-11): the tool centre on `r = (d - D) / 2`, `ceil(depth / pitch)` whole turns, CCW for climb, one floor turn; `finish_allowance > 0` adds a turn and a helix up at `r`; cut intents, never `EntryHelix` (L-15). Lib `ops::bore`; sentry `a_bore_cuts_the_stated_size_p6_6`.
- `vcarve.rs`, `inlay.rs`, `chamfer.rs`, `trace_path.rs`, `project_curve.rs` — the engraving
  and curve family. Chamfer: Z-free tip rings + one level from the tool angle (W3.5, Layered); R-C: `chamfer_edge_reported` / `chamfer_open_at_z` cut a STEP edge on the air side its model states (no chord).

## Invariants

- A pinned Bottom Z is honoured only by `Adaptive3d`, `UnifiedFinish` and `Waterline`; Pocket, Profile,
  Zigzag, Adaptive 2D and Bore (P6.6) refuse it (P3.2). Read `capabilities().floor_owner` before you assume.
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
- P33 (operator ruling 2026-11-15): a Project Curve with `depth_per_pass` cuts in passes inside one toolpath, `project_curve::ladder_passes`: `z_ladder` (`EvenRedistributed`) from the op top Z to the lowest tip Z, each pass the planned curve moved up, pass-major, the deepest last, no sample of an earlier pass at or above the op top; no key = one pass. The stated cut is then `Axial::PerPass`; no feeds door starts a ladder (the feeds setter writes only a stated key). Lib `p33`; sentry `a_project_curve_ladder_holds_through_the_session_p33`. P35: from remaining stock the ladder top is the stock top under the curve (`compute/execute/curve.rs` `ladder_top_z`), not the op top Z; sentry `project_curve_passes_read_the_remaining_stock_p35`. P35 F: `lateral_offset` moves the curve in XY before the projection (`project_curve::lateral_curves`: an open path left for a positive value through `polygon::offset_open_path_reported`, a ring out of its own area through `profile::rings_moved_out`, THE ring rule; pieces and a `TabNote::OffsetSplit` when it is not one piece); sentry `project_curve_takes_a_lateral_offset_p35f`.
- `ops/trace_path.rs` is the follow-path OPERATION (`trace/` holds the records). P1: on
  the line it cuts an open path with no chord. P6.2: `Left`/`Right` put the tool on that side of travel (O-1, as G41/G42), on a loop and on an open path (`polygon::offset_open_path_reported`; the drawn direction, held in the drawing frame on a Bottom setup; sentry `trace_cuts_beside_an_open_path_p6_2`). P1b: leads on closed loops only.
- `inlay.rs` (R2): `Inlay` = pocket V walls only; `InlayPlug` = mirrored plug,
  own project, start depth `pocket_depth − glue_gap` (module doc derives it). F195: the folder denies `cast_possible_truncation` and `cast_sign_loss`; a count from a user length goes through `util::steps`, or its `#[expect]` reason names the bound.

## Sentries

- `cargo test -p rs_cam_core -q --test drill_op_step3 --test drill_flip_removal_g_drillflip --test a_drill_hole_is_cut_once_g_drilltwice`
- `cargo test -p rs_cam_core -q --test project_curve_depth_sign --test chamfer_bevel_sits_on_the_edge_g_chamferside --test chamfer_inside_edge_bevels_into_the_material_g_chamferinside` (G-CHAMFERSIDE: the bevel on the sim stock; G-CHAMFERINSIDE, about 6 s: a pocket wall and a hole)
- `cargo test -p rs_cam_core -q --test depth_beyond_stock_core_g_depthstockcore --test waterline_auto_ladder_is_model_top_to_bottom_r3 --test waterline_respects_a_vertical_wall_r9` + Waterline D69: `--test a_helical_waterline_cuts_a_stack_with_one_entry_d69 --test an_old_waterline_loads_helical_w1b --test a_waterline_widens_its_step_on_a_vertical_wall_w2 --test a_rest_waterline_after_a_rough_widens_its_wall_step_w2 --test a_waterline_stops_at_the_web_of_a_through_cut_w4`
- `cargo test -p rs_cam_core -q --test inlay_pocket_and_plug_fit_g_inlaysplit --test vcarve_cuts_nothing_outside_the_outline_g_vcarvelead` (G-INLAYSPLIT, about 7 s: walls only, mirror, fit at 60° and 90°, floor recipe; G-VCARVELEAD, about 7 s: no lead, nothing cut outside the outline)
