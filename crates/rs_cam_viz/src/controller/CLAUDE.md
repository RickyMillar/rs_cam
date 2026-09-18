# `controller/` — the bridge between the UI and the core

Takes a UI intent, applies it to `ProjectSession`, accepts the worker result.

## Files

- `../controller.rs` — the type, the apply path, the plan progress accessor
  and `PlanResolutionConfirm`.
- `events/` — the handlers by area: compute, model, toolpath, simulation,
  planner, undo. `events/compute.rs` holds the plan driver.
- `generate_all.rs` — the plan's state: steps, outcomes, progress, sink. The
  ORDER is core's, at `session::generation_plan::plan`.
- `io.rs` — project open, save and import at the controller level.
- `tests/` — the suites, one child module per theme; `tests/mod.rs` holds the
  shared fixtures. The named `*_g_*.rs` sentries sit beside it.

## Invariants

- The view mirrors `Effects`. Use `Effects.stale`; never a narrower answer.
- A toolpath edit clears the viewport simulation. A stale mesh beside a new
  toolpath is a wrong picture, not a cosmetic lag.
- `OptimizeToolpath` is a `Job` over a cloned session. It does not mutate it.
- Generate is a PLAN over the dependency edges, and core owns its order. Do
  not replace it with a single pass, and do not re-derive the walk here.
- The plan advances by DIRECT CALL from `drain_compute_results`. A step that
  pushed an `AppEvent` would wait for a painted frame, which a parked
  compositor never gives (the 137 s stall).
- One plan at a time: `process_auto_regen` returns early while one runs, an
  edit cancels it, and a blocked submit never toasts (the row reads WAIT).
- `GuiState::post` is a CLONE of `session.post_config()` — one type,
  `gcode::PostConfig`. `io.rs`'s `refresh_post_mirror` rebuilds it. Never
  copy one post field by hand.

## Sentries

- `cargo test -p rs_cam_viz -q --lib generate_all_plan_g_genplan`
- `cargo test -p rs_cam_viz -q --test apply_contract_a3`
- `cargo test -p rs_cam_viz -q --test effects_are_stamped_wp19`
- `cargo test -p rs_cam_viz -q --test generate_all_fixpoint_parity`
- `cargo test -p rs_cam_viz -q --test feeds_apply_drops_result_n13`
