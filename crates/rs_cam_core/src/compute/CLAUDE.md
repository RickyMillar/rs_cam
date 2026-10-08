# `compute/` — dispatch, catalogue, configuration, simulation

## Files

- `execute.rs`/`execute/` — the dispatch (`execute_operation_annotated`); `catalog.rs` — the registry;
  `operation_configs.rs`, `config.rs`, `stock_config.rs`, `tool_config.rs`, `cutter.rs`, `transform.rs`.
- Simulation: `simulate.rs`, `sim_prefix.rs`, `collision_check.rs`, `source_stock.rs`,
  `stock_carry.rs` (S0), `stock_change.rs` (S2). Also `toolpath_stats.rs`, `stats.rs`,
  `alignment_pins.rs`, `annotate.rs`, `spans.rs`, `validate.rs`, `generated_empty.rs`.

## Invariants

- One request, two builders (`ProjectSession::run_simulation`, the GUI). Five
  SHARED decisions in `simulate.rs`: `PhantomPriorStockScan`, `entry_tool_fields`,
  `group_stock_cut_direction`, `entry_metrics_not_applicable`,
  `group_stock_changes` (S2). Put a sixth there, not in one builder.
- A new operation needs a catalogue row and a config variant. The row's
  `generate` field IS the dispatch; add no second path beside `execute.rs`.
- A dressup's parameters live INSIDE its `Option` on `DressupConfig`
  (CUT-13), with no value field beside its bool; wire: `DressupConfigWire`.
- G10: a helix radius is `helix_radius_for` (None = `HELIX_RADIUS_OVER_D`
  x D), capped at the flat bottom; an entry feed is `entry_feed_rate()`
  (<= the cut feed). `DefaultHelix` is read by `for_op` only (D3).
- G-SIMMEM: the loop holds the cut trace ONCE. The memo asks `admits` BEFORE
  it clones a snapshot; share the trace by `Arc`, never `clone()` it.
- A checkpoint keeps its mesh inputs; `build_mesh` builds on demand (M2).
  Checkpoint k's `mesh_stock` is prior k+1: one `Arc`, same group (M3). S0:
  a Z-axis group starts from the last Z-axis group's final stock.

## Sentries

- `cargo test -p rs_cam_core -q --test retract_intent_move_type_census_w6`
- `cargo test -p rs_cam_core -q --test capability_link_moves_safety`
- `cargo test -p rs_cam_core -q --test sim_prefix_memo_s5`
- `cargo test -p rs_cam_core -q --test stock_carries_across_setups_s0`
- `cargo test -p rs_cam_core -q --test stock_changes_stale_from_their_setup_on_s2`
- `cargo test -p rs_cam_core -q --test set_param_refuses_absent_field_n5`
- `cargo test -p rs_cam_core -q --test generated_empty_refusal_g_entryempty`
- `cargo test -p rs_cam_core -q --test sim_peak_memory_g_simmem` (Linux)
- `cargo test -p rs_cam_core -q --test a_helix_leaves_no_core_and_a_ramp_never_outruns_the_feed_g10`
