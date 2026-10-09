# `session/` — `ProjectSession`, commands and effects

The one door for project state and compute is `ProjectSession::apply(Command)`.

## Files

- `mod.rs` — `ProjectSession` and the facade. `command.rs` — the registry and
  the `Effects` door. `dependencies.rs`, `generation_plan.rs` — edges, order.
- `mutation.rs`, `mutation/` (CRUD, S2 `stock_change.rs`); `compute.rs`, `compute/`
  (generation, simulation, export, diagnostics); `builder.rs`, `project_file.rs`, `save.rs`;
  `stock_change_report.rs` (S4 rows); `diagnostics_types.rs` (keep the `mod.rs` re-export).
- `eval_context.rs`, `cycle_time.rs`, `reach.rs`, `multitool.rs`; `rest_stock.rs` — the
  stored resolution; `load_report.rs` — the memo (`--lib session::load_report`).

## Invariants

- A command returns `Effects`. `Effects.stale` is the stale set.
- An edit invalidates the result chain to fixpoint: `invalidate_output_dependents_of_set`;
  an S2 stock change, from its setup on: `drop_stock_change_dependents`.
- `dependencies::edges` (rules) and `generation_plan` (order): no surface
  re-derives either. The pure `walk_output_dependents` touches no simulation.
- `drop_simulation(cause)` is the one site that clears the simulation. It
  bumps `simulation_epoch` and records the cause. `AdoptResult` refuses a
  stale revision, `AdoptSimulation` a stale epoch.
- ONE stored `simulation_resolution` sets every cell. `try_with_effects`
  drops a rest result whose `SourceStock` no longer matches (G-RESTRES).
- There are no public `*_mut` hatches. `setups_mut` stays `#[cfg(test)]`.
  Read `ProjectSession::simulation_triage`, not a raw issue count.
- Generation passes `entry_feed_rate()`, not the raw ramp feed (G10 B4), and
  the STOCK top, never `heights.top_z`, as the descent ceiling (G-PECKSPLIT).
- Do not add a setter without a command row. The registry test fails on it.

## Sentries

`cargo test -p rs_cam_core -q --test <name>`, one per name:
`command_registry_completeness`, `mutation_paths_invalidate_alike_p0`,
`adopt_result_rejects_stale_completion`, `stale_set_has_one_answer_wp28`,
`dependency_edges_are_the_walker_dep1`, `generation_plan_is_the_edge_walk_w0b`,
`a_late_simulation_does_not_refill_the_core_d7`, `rest_stock_identity_g_restres`,
`stock_changes_stale_from_their_setup_on_s2`.
