# `session/` — `ProjectSession`, commands and effects

The single entry point for project state and compute. The mutation door is
`ProjectSession::apply(Command)` in `command.rs`.

## Files

- `mod.rs` — the `ProjectSession` type and the facade.
- `diagnostics_types.rs` — the diagnostic result types. Keep the `mod.rs`
  re-export; it is the path every caller names.
- `command.rs` — the command registry and the one door that applies a
  command and returns `Effects`.
- `mutation.rs` and `mutation/` — the CRUD methods: entities, project-wide
  configuration, toolpath order and per-toolpath configuration.
- `compute.rs` and `compute/` — generation, simulation, diagnostics, export
  and parameter mutation.
- `builder.rs`, `project_file.rs`, `save.rs` — construction, the project file.
- `eval_context.rs`, `cycle_time.rs`, `reach.rs`, `multitool.rs` — the
  per-setup context, cycle time and its basis, reach, the planner action.

## Invariants

- A command returns `Effects`. `Effects.stale` is the stale set.
- A parameter, tool, model, stock or setup edit invalidates the affected
  cached result chain, to fixpoint. One walker answers:
  `invalidate_output_dependents_of_set`.
- There are no public `*_mut` escape hatches. `setups_mut` is `#[cfg(test)]`
  and `pub(crate)`; keep it that way.
- Read `ProjectSession::simulation_triage`, not a raw issue count.

## Sentries

- `cargo test -p rs_cam_core -q --test command_registry_completeness`
- `cargo test -p rs_cam_core -q --test mutation_paths_invalidate_alike_p0`
- `cargo test -p rs_cam_core -q --test adopt_result_rejects_stale_completion`
- `cargo test -p rs_cam_core -q --test stale_set_has_one_answer_wp28`

## Do not

- Do not add a setter without a command row. The registry test fails on it.
