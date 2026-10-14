# `compute/` — dispatch, catalogue, configuration, simulation

## Files

- `execute.rs`/`execute/` — the dispatch (`execute_operation_annotated`), `prologue.rs` (W2.2), `findings.rs`
  (W2.3), `standing_material.rs` (G-RESTLEVELS), `layered.rs` (W3.0); `catalog.rs` — the registry, `catalog/capabilities.rs` (W2.4); `operation_configs.rs`, `config.rs`, `stock_config.rs`, `tool_config.rs`, `cutter.rs`, `transform.rs`.
- Simulation: `simulate.rs`, `sim_prefix.rs`, `collision_check.rs`, `source_stock.rs`, `stock_carry.rs`
  (S0), `stock_change.rs` (S2), `stock_change_apply.rs` (S3). Also `toolpath_stats.rs`, `stats.rs`, `annotate.rs`,
  `alignment_pins/` (keying, 2D placer, walls, setup map and flip axis, P4 set patterns, P8 hole sources, P10 setup notes), `spans.rs`, `validate.rs`, `generated_empty.rs`.

## Invariants

- One request, two builders (`ProjectSession::run_simulation`, the GUI). Five SHARED decisions in
  `simulate.rs`: `PhantomPriorStockScan`, `entry_tool_fields`, `group_stock_cut_direction`,
  `entry_metrics_not_applicable`, `group_stock_changes` (S2). Put a sixth there, not in one builder.
  Both builders carve every enabled result, an empty one too (G-RESTLEVELS: no phantom theft).
- A new operation needs a catalogue row and a config variant. The row's `generate` field IS the
  dispatch; add no second path. The prologue runs first: cancel, the row's `GeometryRequirement`,
  `OperationType::tool_refusal`. An adapter reads `Prepared`; it does not check them again.
- W2.4: a consumer reads `OperationConfig::capabilities()`, never a `match` on the op type. W2.3: a report reaches `GenerationFindings` only through `ctx.record`.
- W3: a Layered op (Pocket, Zigzag, Trace, Profile, Face, Chamfer) writes `prepare` + `emit_level`; hooks `outlines`, `ladder`, `spans`, `annotate`, `reach_band`, `off_grid`.
  `run_layered` owns the ladder, rest rule, one prepare per region, `region_ordering` (R4), cancel per level, spans. `LevelInfo::is_final`: final Z, spring passes too.
- W3.4: rest rule = `Capabilities::rest_use`. `LevelClip` clips the input; `LevelSkip` only skips (a clipped contour cuts a new wall; Chamfer band = `w + t`); Drill
  `StockColumn` lowers each hole top and R-plane (D8). 2D Adaptive `LevelMask`: declared only. Zigzag op and Pocket share `ZigzagRaster`.
- A dressup's parameters live INSIDE its `Option` on `DressupConfig` (CUT-13), with no value field
  beside its bool; wire: `DressupConfigWire`. G10: a helix radius is `helix_radius_for` (None =
  `HELIX_RADIUS_OVER_D` x D), capped at the flat bottom; an entry feed is `entry_feed_rate()`
  (<= the cut feed). `DefaultHelix` is read by `for_op` only (D3).
- G-SIMMEM: the loop holds the cut trace ONCE. The memo asks `admits` BEFORE it clones a snapshot; share the trace by `Arc`, never `clone()` it.
- A checkpoint keeps its mesh inputs; `build_mesh` builds on demand (M2). Checkpoint k's `mesh_stock` is prior k+1: one `Arc`, same group (M3).
  S0: a Z-axis group starts from the last Z-axis group's final stock. S3: its stock changes then apply, to it and (not lateral) to `global_stock`.

## Sentries

- `cargo test -p rs_cam_core -q --test retract_intent_move_type_census_w6` (and `capability_link_moves_safety`, `sim_prefix_memo_s5`; `sim_peak_memory_g_simmem` on Linux)
- `--test one_link_ceiling_for_every_op_g_linkceil` (G-LINKCEIL, R1): `execute/shared.rs::link_ceiling_for` is the ONE link-ceiling builder; each op states its `LinkCeilingNeed`
- `--test stock_carries_across_setups_s0`, `--test stock_changes_stale_from_their_setup_on_s2`, `--test stock_changes_apply_to_the_stock_s3`; `--lib cancellable_families_honour_a_preset_cancel_flag` (prologue order), `--lib every_generator_polls` (kernels), `--lib every_tool_refusal`
- `--test every_op_keeps_its_toolpath_g_opgolden` (G-OPGOLDEN, 2 s): the output of every op. Re-bless (`RS_CAM_BLESS_OPGOLDEN=1`) only the ops a change names. `--test every_op_states_its_capabilities_w2_4`: the frozen capability table
- `cargo test -p rs_cam_core -q --test set_param_refuses_absent_field_n5 --test generated_empty_refusal_g_entryempty`; `--test a_helix_leaves_no_core_and_a_ramp_never_outruns_the_feed_g10`, `--test a_rest_pocket_cuts_only_below_the_prior_floor_g_restlevels`, `--test layered_framework_owns_the_ladder_w3` + `--lib compute::execute::layered` (W3)
- Pins: `--test` `alignment_pin_keying_intrinsic_p1`, `alignment_pins_judge_the_full_setup_map_p2`, `alignment_pin_keying_g_pinauto`, `a_pin_set_edit_stales_only_the_pin_drill_p4`, `alignment_pin_walls_and_2d_placer_p6`, `dowel_holes_have_one_source_p8`, `pin_holes_are_drilled_by_size_r1`
