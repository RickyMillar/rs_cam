# `controller/` — the bridge between the UI and the core

Takes a UI intent, applies it to `ProjectSession`, accepts the worker result.

## Files

- `../controller.rs` — the type, the apply path, the plan progress accessor
  and `PlanResolutionConfirm`. `events/` — the handlers by area: compute (the
  plan driver), model, toolpath, simulation, planner, preferences, undo, pins
  (the one pin-edit door for GUI and MCP, P5). P4.6 `EditTabs`: one `SetToolpathParam` and one undo step per act; the panel snapshot moves past the act. P4.D: `TogglePick` asks core `click_edit`; a `BoundaryToSelection` is one `SetBoundaryConfig` (`apply_boundary_switch`) with a notice.
- `generate_all.rs` — the plan's state: steps, outcomes, progress, sink. The
  ORDER is core's, at `session::generation_plan::plan`.
- `io.rs` — project open, save and import at the controller level. P4.8: a DXF or SVG opens the import dialog (`open_import_dialog`, `edit_import_dialog`, `confirm_import_dialog`); `set_model_import_options` is the fourth refresh door. P4.9 `edit_drawing_ledger`: one `EditModelDrawing` and one `ModelDrawingEdits` undo step per act (undo writes `LedgerEdit::Replace`).
- `tests/` — the suites, one child module per theme; `tests/mod.rs` holds the
  shared fixtures. The named `*_g_*.rs` sentries sit beside it.

## Invariants

- The view mirrors `Effects`. Use `Effects.stale`; never a narrower answer.
- A toolpath edit clears the viewport simulation. A stale mesh beside a new
  toolpath is a wrong picture, not a cosmetic lag.
- A GUI `Job` (tier-map preview; P4.7 STL faces, `events/model_faces.rs` `process_model_faces`, one submit per cold model) sets `AppState::job_run`; only `deliver_gui_job` clears it.
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

- `cargo test -p rs_cam_viz -q --lib` with `g_genplan`, `g_rapidframe`, `g_cachekeys`, `pin_edit_one_door_p5`, `d11`, `p4_7`, `p4_8`, `p4_9`; `--test model_refresh_invalidates_results_g_rescalestale` (P4.8 arms)
- `cargo test -p rs_cam_viz -q --test apply_contract_a3`
- `cargo test -p rs_cam_viz -q --test effects_are_stamped_wp19`
- `cargo test -p rs_cam_viz -q --test generate_all_fixpoint_parity`
- `cargo test -p rs_cam_viz -q --test feeds_apply_drops_result_n13`, `--test a_no_op_feeds_apply_stales_nothing_b19` (B19: stamp only `Effects.stale`)
