# `compute/` — dispatch, catalogue, configuration, simulation

One entry point for every toolpath operation, the operation catalogue, the
shared configuration types and the simulation orchestration. The entry point
is `compute::execute_operation_annotated`.

## Files

- `execute.rs` and its `execute/` children — the dispatch for all operations,
  grouped by family (2.5D clearing, drilling, curve and V-bit engraving, 3D
  finishing, raster finishing, the dressup pipeline, the findings writers).
- `catalog.rs` and its `catalog/` children — `OperationType`,
  `OperationConfig`, the parameter registry and its schema.
- `operation_configs.rs`, `config.rs`, `stock_config.rs`, `tool_config.rs`,
  `cutter.rs`, `transform.rs` — the configuration model.
- `simulate.rs`, `sim_prefix.rs`, `collision_check.rs` — simulation
  orchestration, the `generate_all` prefix memo, the collision wrapper.
- `annotate.rs`, `spans.rs`, `stats.rs` — runtime annotation and spans.
- `validate.rs`, `generated_empty.rs` — the stale-default validator and the
  empty-generation refusal.

## Invariants

- A rest chain needs simulated upstream stock. The core builder and the GUI
  builder share a phantom prior-stock rule for the first pending rest
  operation. Keep that parity when you change the admission logic.
- A new operation needs a catalogue row, a config variant and a dispatch arm.
  Add all three; the registry test fails on a missing row.

## Sentries

- `cargo test -p rs_cam_core -q --test retract_intent_move_type_census_w6`
- `cargo test -p rs_cam_core -q --test capability_link_moves_safety`
- `cargo test -p rs_cam_core -q --test sim_prefix_memo_s5`
- `cargo test -p rs_cam_core -q --test set_param_refuses_absent_field_n5`
- `cargo test -p rs_cam_core -q --test generated_empty_refusal_g_entryempty`

## Do not

- Do not add a second dispatch path beside `execute`.
