# `session/` — `ProjectSession`, commands and effects

The one door for project state and compute is `ProjectSession::apply(Command)`.

## Files

- `mod.rs` — `ProjectSession` and the facade. `command.rs` — the registry and the `Effects` door. `dependencies.rs`, `generation_plan.rs` — edges, order.
- `mutation.rs`, `mutation/` (CRUD, S2 `stock_change.rs`); `compute.rs`, `compute/` (generation, simulation, export, diagnostics); `builder.rs`, `project_file.rs`, `save.rs`;
  `stock_change_report.rs` (S4 rows); `diagnostics_types.rs` (keep the `mod.rs` re-export).
- `eval_context.rs`, `cycle_time.rs`, `reach.rs`, `multitool.rs`, `pins.rs` (P6 pin inputs, P10 setup notes), `pin_report.rs` (P7, the one pin report); `shoe.rs` (H1, the one shoe plan); `rest_stock.rs` — the
  stored resolution; `load_report.rs` — the memo; `result_provenance.rs` — the feed stage (W2.5); `selection_status.rs` (P2.4: THE resolution, the `SelectionStatus` memo).

## Invariants

- A command returns `Effects`. `Effects.stale` is the stale set.
- An edit invalidates the result chain to fixpoint: `invalidate_output_dependents_of_set`;
  an S2 stock change, from its setup on: `drop_stock_change_dependents`.
- `dependencies::edges` (rules), `generation_plan` (order; a row that gains a result in the
  plan stales the snapshots below it, W1.5b): no surface re-derives either. `walk_output_dependents` is pure.
- `drop_simulation(cause)` is the one site that clears the simulation. It
  bumps `simulation_epoch` and records the cause. `AdoptResult` refuses a
  stale revision, `AdoptSimulation` a stale epoch. `store_simulation` is the one
  store; it numbers the run. W2.5: a slot is `Dressed` until modulation rewrites it (`Modulated
  { simulation_run, source }`, B5: current while `source` is); every export reads `feed_stage`.
- ONE stored `simulation_resolution` sets every cell. `try_with_effects`
  drops a rest result whose `SourceStock` no longer matches (G-RESTRES).
- There are no public `*_mut` hatches. `setups_mut` stays `#[cfg(test)]`.
  Read `ProjectSession::simulation_triage`, not a raw issue count.
- Generation passes `entry_feed_rate()`, not the raw ramp feed (G10 B4), and
  the STOCK top, never `heights.top_z`, as the descent ceiling (G-PECKSPLIT).
  Do not add a setter without a command row. The registry test fails on it.
  C1: the loader refuses each key in `project_file::RETIRED_KEYS` (a key that leaves the format gets a
  row); C2: one refusal names EVERY retired key in the file (`SessionError::RetiredKeys`).
- P2.4/P2.5: a model id and a tool id are EXACT (no first-item fallback); `SessionError::Selection` = generation = `selection_status`; `ToolpathConfig::models_read` drives `drop_results_for_model` (re-import AND `remove_model`, P2.6). P2.6: a click toggles through `pick_edit` (the memo); `face_pick_key`; `list_model_items`; the undo snapshot restores `boundary` (D11). P4.1: `SelectionTarget { Op, Boundary }` names a selection (`ToolpathConfig::selection_of`; the memo keys on it); an op with no slot takes its face picks and auto top from a `Selection` boundary (`boundary_face_top_z`). P4.2 (L-6): `pick_edit` gives `Replace` with the one pick while the op selection is `Selection::stock()`. P4.3: the `SelectionStatus` query answers the memo's `Arc` (no clone per frame); the GUI Selection section reaches it and `ListItems`. P4.4: the report gives `item_states` (index-aligned with `LoadedModel::items`) and `foreign`, from the same resolution and the `view` roles; a refused selection marks no item `Cut`.

## Sentries

`cargo test -p rs_cam_core -q --test <name>`, one per name (W2.5: viz `export_states_the_feed_stage_w2_5`): `command_registry_completeness`, `mutation_paths_invalidate_alike_p0`,
`adopt_result_rejects_stale_completion`, `stale_set_has_one_answer_wp28`, `dependency_edges_are_the_walker_dep1`, `generation_plan_is_the_edge_walk_w0b`, `a_late_simulation_does_not_refill_the_core_d7`, `rest_stock_identity_g_restres`,
`stock_changes_stale_from_their_setup_on_s2`, `a_plan_resimulates_after_an_upstream_change_w1_5b`, `fresh_generation_ignores_simulation_b19` (B19: `Fresh` reads no prior stock), `selection_status_is_memoised_p2_4`, P2.5 `a_reimport_stales_every_reader_of_the_model_p2_5`, `a_missing_tool_id_refuses_p2_5`, `a_retired_boundary_source_is_named_p2_5`, P2.6 `selection_round_trips_p2_6`, P2.7 `one_item_is_one_pick_p2_7`, P4.1 `a_3d_op_face_pick_sets_its_top_p4_1`, P4.2 `a_face_op_reads_its_selection_p4_2`, P4.4 `item_states_match_the_cut_p4_4`.
