# `session/` — `ProjectSession`, commands and effects

The single entry point for project state and compute. The mutation door is
`ProjectSession::apply(Command)` in `command.rs`.

## Files

- `mod.rs` — the `ProjectSession` type and the facade.
- `command.rs` — the command registry and the one door that applies a
  command and returns `Effects`.
- `dependencies.rs`, `generation_plan.rs` — the dependency edges, and the
  ordered work that makes a scope current.
- `mutation.rs` and `mutation/` — the CRUD methods.
- `compute.rs` and `compute/` — generation, simulation, diagnostics, export
  and parameter mutation.
- `diagnostics_types.rs` — the diagnostic types. Keep the `mod.rs` re-export.
- `builder.rs`, `project_file.rs`, `save.rs` — construction, the project file.
- `eval_context.rs`, `cycle_time.rs`, `reach.rs`, `multitool.rs` — context,
  cycle time and its basis, reach, the planner action.

## Invariants

- A command returns `Effects`. `Effects.stale` is the stale set.
- A parameter, tool, model, stock or setup edit invalidates the affected
  cached result chain, to fixpoint. One walker answers:
  `invalidate_output_dependents_of_set`.
- `dependencies::edges` states that walker's rules, and `generation_plan`
  states the order; no surface re-derives either. The edit door clears the
  simulation; the pure `walk_output_dependents` does not.
- There are no public `*_mut` escape hatches. `setups_mut` is `#[cfg(test)]`
  and `pub(crate)`; keep it that way.
- Read `ProjectSession::simulation_triage`, not a raw issue count.
- Do not add a setter without a command row. The registry test fails on it.

## Sentries

`cargo test -p rs_cam_core -q --test <name>`, one per name:
`command_registry_completeness`, `mutation_paths_invalidate_alike_p0`,
`adopt_result_rejects_stale_completion`, `stale_set_has_one_answer_wp28`,
`dependency_edges_are_the_walker_dep1`, `generation_plan_is_the_edge_walk_w0b`.
