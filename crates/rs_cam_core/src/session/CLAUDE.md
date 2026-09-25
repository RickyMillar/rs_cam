# `session/` — `ProjectSession`, commands and effects

The one door for project state and compute is `ProjectSession::apply(Command)`.

## Files

- `mod.rs` — the `ProjectSession` type and the facade.
- `command.rs` — the command registry, and the door that returns `Effects`.
- `dependencies.rs`, `generation_plan.rs` — the edges, and the plan order.
- `mutation.rs`, `mutation/` (CRUD); `compute.rs`, `compute/` (generation,
  simulation, export, diagnostics); `builder.rs`, `project_file.rs`, `save.rs`.
- `diagnostics_types.rs` — the diagnostic types. Keep the `mod.rs` re-export.
- `eval_context.rs`, `cycle_time.rs`, `reach.rs`, `multitool.rs` — context,
  cycle time, reach, the planner. `rest_stock.rs` — the stored resolution.

## Invariants

- A command returns `Effects`. `Effects.stale` is the stale set.
- A parameter, tool, model, stock or setup edit invalidates the affected
  cached result chain, to fixpoint: `invalidate_output_dependents_of_set`.
- `dependencies::edges` states that walker's rules and `generation_plan` the
  order; no surface re-derives either. The pure `walk_output_dependents`
  touches no simulation.
- `drop_simulation` is the one site that clears the simulation, and it bumps
  `simulation_epoch`. `AdoptResult` refuses a stale revision,
  `AdoptSimulation` a stale epoch.
- ONE stored `simulation_resolution` sets every cell. `try_with_effects`
  drops a rest result whose `SourceStock` no longer matches (G-RESTRES).
- There are no public `*_mut` hatches. `setups_mut` stays `#[cfg(test)]`.
  Read `ProjectSession::simulation_triage`, not a raw issue count.
- Generation passes `entry_feed_rate()`, not the raw ramp feed (G10 B4).
- Do not add a setter without a command row. The registry test fails on it.

## Sentries

`cargo test -p rs_cam_core -q --test <name>`, one per name:
`command_registry_completeness`, `mutation_paths_invalidate_alike_p0`,
`adopt_result_rejects_stale_completion`, `stale_set_has_one_answer_wp28`,
`dependency_edges_are_the_walker_dep1`, `generation_plan_is_the_edge_walk_w0b`,
`a_late_simulation_does_not_refill_the_core_d7`, `rest_stock_identity_g_restres`.
