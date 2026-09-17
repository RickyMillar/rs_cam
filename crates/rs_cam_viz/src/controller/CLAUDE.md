# `controller/` — the bridge between the UI and the core

Takes a UI intent, applies it to `ProjectSession`, and accepts the worker
result. The entry point is `../controller.rs`.

## Files

- `../controller.rs` — the controller type and the apply path.
- `events/` — the event handlers by area: compute, model, toolpath,
  simulation, planner, undo.
- `generate_all.rs` — `generate_all` as a fixpoint over the rest-stock chain.
- `io.rs` — project open, save and import at the controller level.
- `tests/` — the controller test suites, one child module per theme;
  `tests/mod.rs` holds the fixtures they share. `workflow_tests.rs`,
  `results_parity_tests.rs` and the named `*_g_*.rs` sentries sit beside it.

## Invariants

- The view mirrors `Effects`. Use `Effects.stale`; do not compute a narrower
  stale answer in the UI or in an MCP reply.
- A toolpath edit clears the viewport simulation. A stale mesh beside a new
  toolpath is a wrong picture, not a cosmetic lag.
- `OptimizeToolpath` is a `Job` over a cloned session. It does not mutate the
  live session while it runs.
- `generate_all` is a fixpoint over the rest-stock chain. Do not replace it
  with a single pass over the toolpath list.
- `GuiState::post` is a CLONE of `session.post_config()` — one type,
  `gcode::PostConfig`. `io.rs`'s `refresh_post_mirror` rebuilds it. Never
  copy one post field by hand.

## Sentries

- `cargo test -p rs_cam_viz -q --test apply_contract_a3`
- `cargo test -p rs_cam_viz -q --test effects_are_stamped_wp19`
- `cargo test -p rs_cam_viz -q --test production_writes_go_through_apply_wp15a`
- `cargo test -p rs_cam_viz -q --test generate_all_fixpoint_parity`
- `cargo test -p rs_cam_viz -q --test feeds_apply_drops_result_n13`
