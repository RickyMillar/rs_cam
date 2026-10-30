# `controller/` — the bridge: a UI intent to `ProjectSession`, and the worker result back

## Files

- `../controller.rs` — the type, the apply path, the plan progress accessor and `PlanResolutionConfirm`.
- `events/` — the handlers by area: compute (the plan driver), model, toolpath, simulation, planner,
  preferences, undo, pins (the one pin-edit door for GUI and MCP, P5), `model_faces.rs` (STL faces), `model_pick_index.rs` (the SEL-2 pick BVH). S11: `handle_add_toolpath` (= `add_toolpath_with_model`) builds the op with core `draft_toolpath` (the MCP and CLI door); no op is built here. Q7: it passes the selected model (`Selection::Model`) and no faces (an op takes faces from its own selection); R2: no fitting tool adds the op with no tool and one Warning toast (the Bore add too).
  E4 `push_act_step`: `EditTabs`, `EditSelection` and the boundary switch push one undo step per act; the panel snapshot moves past the act.
  `TogglePick` asks core `click_edit`; a `BoundaryToSelection` is one `SetBoundaryConfig` (`apply_boundary_switch`) with a notice.
  `AddPick` (P3) asks the same, and a `RemovePick` answer is no edit.
- `generate_all.rs` — the plan's state: steps, outcomes, progress, sink. The ORDER is core's, at
  `session::generation_plan::plan`.
- `io.rs` — project open, save and import. A DXF or SVG opens the import dialog (`open_import_dialog`,
  `edit_import_dialog`, `confirm_import_dialog`); `set_model_import_options` is the fourth refresh door. No door here fits the stock: the core row does (F31).
  `edit_drawing_ledger`: one `EditModelDrawing` and one `ModelDrawingEdits` undo step per act (the undo
  writes `LedgerEdit::Replace`). F110: only `open_job_from_path` builds `VizError::ProjectLoad`; every other core refusal is `VizError::Session`.
- `tests/` — the suites, one child module per theme; `tests/mod.rs` holds the shared fixtures. The named `*_g_*.rs` sentries, `results_parity_tests.rs` (G-RESULTS) and `workflow_tests.rs` (end to end) sit beside it.

## Invariants

- The view mirrors `Effects`. Use `Effects.stale`; never a narrower answer. F040: `apply_multitool_plan` applies `PlanMultitoolFinishing`; F115: the sim drain re-points the run result for `Effects::modulated`.
  F096: every controller and MCP write takes `AppController::apply` (the state door, then the view sim clears at once).
  F2: a focus write takes `AppController::set_focus`; a change of focus or of its setup raises `pending_upload`. R7b: `discharge_panel_side_effects` shows each `PanelSideEffects.notes` entry as an Info notice (lib `a_model_change_in_the_panel_drops_the_lost_picks_r7b`).
- A toolpath edit clears the viewport simulation. A stale mesh beside a new toolpath is a wrong picture.
- A GUI `Job` (the tier-map preview; the STL faces: `process_model_faces`, one submit per cold model; the pick BVH:
  `process_model_pick_index`, every mesh model, a failed one never again) sets `AppState::job_run`; only `deliver_gui_job` clears it.
- Generate is a PLAN over the dependency edges; core owns its order. Do not make it one pass or re-derive the walk.
- The plan advances by DIRECT CALL from `drain_compute_results`. A step that pushed an `AppEvent` would
  wait for a painted frame, which a parked compositor never gives (the 137 s stall).
- One plan at a time: `process_auto_regen` returns early while one runs, an edit cancels it, and a
  blocked submit never toasts (the row reads WAIT). W2c: an AUTOMATIC submit (the sweep, a plan step that is not `plan.target`) skips a disabled or not-ready op through `automatic_submit_skip` (core `skips_automatic_generate`): no toast, no refusal, the plan counts `not_ready`. The explicit target refuses.
- F072: the session holds the only post config. Every reader takes `session.post_config()`; every
  write is `Command::SetPostConfig`. The Post panel edits a one-frame copy (`properties::commit_post_edit`).

## Sentries

- `cargo test -p rs_cam_viz -q --lib` with `g_genplan`, `g_rapidframe`, `g_cachekeys`, `pin_edit_one_door_p5`,
  `d11`, `p4_7`, `p4_8`, `p4_9`, `p4_d`, `f096`, `g_roughcov` (A2: the 3D Rough coverage is read at adopt from the VIEW's checkpoints; the session copy keeps none, W2-E); `--test model_refresh_invalidates_results_g_rescalestale` (P4.8 arms)
- `cargo test -p rs_cam_viz -q --test apply_contract_a3 --test effects_are_stamped_wp19 --test generate_all_fixpoint_parity`
- `cargo test -p rs_cam_viz -q --test feeds_apply_drops_result_n13`, `--test a_no_op_feeds_apply_stales_nothing_b19` (B19: stamp only `Effects.stale`), `--test one_apply_feeds_funnel_for_every_surface_f114` (the funnel applies `Command::ApplyFeeds`)
