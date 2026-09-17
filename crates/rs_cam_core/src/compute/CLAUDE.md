# `compute/` — dispatch, catalogue, configuration, simulation

Operation dispatch, the catalogue, the configuration types and the simulation
orchestration. The entry is `compute::execute_operation_annotated`.

## Files

- `execute.rs` and its `execute/` children — the dispatch for all operations,
  grouped by family (2.5D clearing, drilling, curve and V-bit engraving, 3D
  finishing, raster finishing, the dressup pipeline, the findings writers).
- `catalog.rs` and its `catalog/` children — `OperationType`,
  `OperationConfig`, the parameter registry and its schema.
- `operation_configs.rs`, `config.rs`, `stock_config.rs`, `tool_config.rs`,
  `cutter.rs`, `transform.rs` — the configuration model.
- `toolpath_stats.rs` — `ToolpathStats` and its finding types.
  `alignment_pins.rs` — the keyed-pin construction and the flip audit.
- `simulate.rs`, `sim_prefix.rs`, `collision_check.rs` — simulation
  orchestration, the `generate_all` prefix memo, the collision wrapper.
- `annotate.rs`, `spans.rs`, `stats.rs` — runtime annotation and spans.
  `validate.rs`, `generated_empty.rs` — stale defaults, the empty refusal.

## Invariants

- One simulation request, two builders: `ProjectSession::run_simulation` and
  the GUI controller. Four decisions are SHARED functions in `simulate.rs`,
  not parity to keep by hand — `PhantomPriorStockScan` (a rest chain needs
  simulated upstream stock), `group_stock_cut_direction`,
  `entry_tool_fields` and `entry_metrics_not_applicable`. Put a fifth there
  rather than in one builder.
- A new operation needs a catalogue row and a config variant. The row's
  `generate` field IS the dispatch; `execute.rs` has no second arm to add.

## Sentries

- `cargo test -p rs_cam_core -q --test retract_intent_move_type_census_w6`
- `cargo test -p rs_cam_core -q --test capability_link_moves_safety`
- `cargo test -p rs_cam_core -q --test sim_prefix_memo_s5`
- `cargo test -p rs_cam_core -q --test set_param_refuses_absent_field_n5`
- `cargo test -p rs_cam_core -q --test generated_empty_refusal_g_entryempty`

## Do not

- Do not add a second dispatch path beside `execute`.
