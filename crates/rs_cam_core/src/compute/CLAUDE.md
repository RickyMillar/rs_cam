# `compute/` — dispatch, catalogue, configuration, simulation

The title names the scope. The entry is `compute::execute_operation_annotated`.

## Files

- `execute.rs` + `execute/` — the dispatch for all operations, by family
  (2.5D, drilling, engraving, 3D and raster finishing, dressups, findings).
- `catalog.rs` + `catalog/` — `OperationType`, `OperationConfig`, registry.
- `operation_configs.rs`, `config.rs`, `stock_config.rs`, `tool_config.rs`,
  `cutter.rs`, `transform.rs` — the configuration model.
- `simulate.rs`, `sim_prefix.rs`, `collision_check.rs`, `source_stock.rs` —
  simulation, the prefix memo, collisions, the rest snapshot record.
- `toolpath_stats.rs`, `alignment_pins.rs`, `annotate.rs`, `spans.rs`,
  `stats.rs`, `validate.rs`, `generated_empty.rs`.

## Invariants

- One simulation request, two builders: `ProjectSession::run_simulation` and
  the GUI controller. Four decisions are SHARED functions in `simulate.rs`,
  not parity kept by hand: `PhantomPriorStockScan` (a rest chain needs
  simulated upstream stock), `group_stock_cut_direction`, `entry_tool_fields`,
  `entry_metrics_not_applicable`. Put a fifth there, not in one builder.
- A new operation needs a catalogue row and a config variant. The row's
  `generate` field IS the dispatch; add no second path beside `execute.rs`.
- A dressup's parameters live INSIDE its `Option` on `DressupConfig`
  (CUT-13). Add no value field beside an enable bool. `DressupConfigWire`
  holds the flat wire keys; the project file and MCP read that, not the type.
- G10: a helix radius is `helix_radius_for` (None = `HELIX_RADIUS_OVER_D`
  x D), capped at the flat bottom; an entry feed is `entry_feed_rate()`
  (<= the cut feed). `DefaultHelix` is read by `for_op` only (D3).

## Sentries

- `cargo test -p rs_cam_core -q --test retract_intent_move_type_census_w6`
- `cargo test -p rs_cam_core -q --test capability_link_moves_safety`
- `cargo test -p rs_cam_core -q --test sim_prefix_memo_s5`
- `cargo test -p rs_cam_core -q --test set_param_refuses_absent_field_n5`
- `cargo test -p rs_cam_core -q --test generated_empty_refusal_g_entryempty`
- `cargo test -p rs_cam_core -q --test a_helix_leaves_no_core_and_a_ramp_never_outruns_the_feed_g10`
