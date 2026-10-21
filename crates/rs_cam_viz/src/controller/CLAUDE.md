# `controller/` — the bridge: a UI intent to `ProjectSession`, and the worker result back

## Files

- `../controller.rs` — the type, the apply path, the plan progress accessor and `PlanResolutionConfirm`.
- `events/` — the handlers by area: compute (the plan driver), model, toolpath, simulation, planner,
  preferences, undo, pins (the one pin-edit door for GUI and MCP, P5), `model_faces.rs` (STL faces).
  `EditTabs`: one `SetToolpathParam` and one undo step per act; the panel snapshot moves past the act.
  `TogglePick` asks core `click_edit`; a `BoundaryToSelection` is one `SetBoundaryConfig`
  (`apply_boundary_switch`) with a notice.
- `generate_all.rs` — the plan's state: steps, outcomes, progress, sink. The ORDER is core's, at
  `session::generation_plan::plan`.
- `io.rs` — project open, save and import. A DXF or SVG opens the import dialog (`open_import_dialog`,
  `edit_import_dialog`, `confirm_import_dialog`); `set_model_import_options` is the fourth refresh door.
  `edit_drawing_ledger`: one `EditModelDrawing` and one `ModelDrawingEdits` undo step per act (the undo
  writes `LedgerEdit::Replace`). F110: only `open_job_from_path` builds `VizError::ProjectLoad`; every other core refusal is `VizError::Session`.
- `tests/` — the suites, one child module per theme; `tests/mod.rs` holds the shared fixtures. The named `*_g_*.rs` sentries, `results_parity_tests.rs` (G-RESULTS) and `workflow_tests.rs` (end to end) sit beside it.

## Invariants

- The view mirrors `Effects`. Use `Effects.stale`; never a narrower answer. F040: `apply_multitool_plan` applies `PlanMultitoolFinishing`; F115: the sim drain re-points the run result for `Effects::modulated`.
  F096: every controller and MCP write takes `AppController::apply` (the state door, then the view sim clears at once).
- A toolpath edit clears the viewport simulation. A stale mesh beside a new toolpath is a wrong picture.
- A GUI `Job` (the tier-map preview; the STL faces: `process_model_faces`, one submit per cold model)
  sets `AppState::job_run`; only `deliver_gui_job` clears it.
- Generate is a PLAN over the dependency edges; core owns its order. Do not make it one pass or re-derive the walk.
- The plan advances by DIRECT CALL from `drain_compute_results`. A step that pushed an `AppEvent` would
  wait for a painted frame, which a parked compositor never gives (the 137 s stall).
- One plan at a time: `process_auto_regen` returns early while one runs, an edit cancels it, and a
  blocked submit never toasts (the row reads WAIT).
- F072: the session holds the only post config. Every reader takes `session.post_config()`; every
  write is `Command::SetPostConfig`. The Post panel edits a one-frame copy (`properties::commit_post_edit`).

## Sentries

- `cargo test -p rs_cam_viz -q --lib` with `g_genplan`, `g_rapidframe`, `g_cachekeys`, `pin_edit_one_door_p5`,
  `d11`, `p4_7`, `p4_8`, `p4_9`, `p4_d`, `f096`; `--test model_refresh_invalidates_results_g_rescalestale` (P4.8 arms)
- `cargo test -p rs_cam_viz -q --test apply_contract_a3 --test effects_are_stamped_wp19 --test generate_all_fixpoint_parity`
- `cargo test -p rs_cam_viz -q --test feeds_apply_drops_result_n13`, `--test a_no_op_feeds_apply_stales_nothing_b19` (B19: stamp only `Effects.stale`), `--test one_apply_feeds_funnel_for_every_surface_f114` (the funnel applies `Command::ApplyFeeds`)
