# Perf sweep round 2, W3: one collision sweep, on rayon (agent report)

Base: HEAD `bb179e8bd`. Not committed. The lead measures the wanaka run.

## Files changed

- `crates/rs_cam_core/src/session/compute/diagnostics.rs`: W3a doors.
- `crates/rs_cam_core/src/session/compute.rs`, `crates/rs_cam_core/src/session/mod.rs`:
  re-export `HolderCollisionSweepRow`.
- `crates/rs_cam_cli/src/project.rs`: the CLI runs the sweep once.
- `crates/rs_cam_core/src/stock/collision.rs`: W3b parallel walk.
- `crates/rs_cam_core/src/session/compute/tests.rs`: sentry `the_collision_sweep_runs_once_perf6` (unit test, because `results` is private).
- `crates/rs_cam_core/tests/the_collision_walk_is_the_same_in_parallel_perf7.rs`: new sentry.

## W3a: the CLI sweeps once

New core doors (`diagnostics.rs`):

- `ProjectSession::holder_collision_sweep(cancel) -> Vec<HolderCollisionSweepRow>`.
  A row is `(index, ToolpathId, Result<Option<CollisionCheckResult>, SessionError>)`.
  It holds the old `holder_collision_counts` body: `holder_collision_indices`, then
  `collision_check_with_index` per toolpath that has a result. The `warn!` for a failed
  check moved here.
- `ProjectSession::holder_collision_counts_of(&[row]) -> Vec<(ToolpathId, HolderCollisionCheck)>`:
  the one reading of a row (Workpiece-only count, `NotApplicable`, `Failed`).
- `holder_collision_counts(cancel)` is now `holder_collision_counts_of(&holder_collision_sweep(cancel))`.
  The CMP-14 doc on the three states stays on it.
- `diagnostics_with_holder_collisions(holder)` and `triage_with_holder_collisions(holder)`
  hold the old bodies. `diagnostics()` and `triage()` call them with
  `holder_collision_counts(&no_cancel)`. The `triage` early return for "no simulation" is in the
  same place. `diagnostics_with_holder_collisions` moves the vector into the evidence
  (a `match`), not two clones; the values are the same (clippy `needless_pass_by_value`).

CLI (`project.rs` ~935): `holder_collision_sweep(&cancel)` once, then
`holder_collision_counts_of(&sweep)`, then `into_iter()` over the rows fills
`collision_reports` and `collision_check_failed` with the same three arms as before.
`diagnostics()` -> `diagnostics_with_holder_collisions(holder_counts.clone())`;
`triage()` -> `triage_with_holder_collisions(holder_counts)`.

### Equality proof

1. Toolpath set. The old loop: `for idx in 0..toolpath_count()` with
   `result_at(idx).is_none() -> continue`. `toolpath_count()` is
   `toolpath_configs.len()` (`session/mod.rs:2232`) and `result_at` is
   `results.get(&index)` (`session/mod.rs:2180`). The sweep:
   `toolpath_configs.iter().enumerate().filter(results.contains_key(idx))`. Same set, same order.
2. Toolpath id. The old fallback `unwrap_or(ToolpathId::from_wire(idx))` cannot fire because
   `idx < toolpath_configs.len()`, so `toolpath_config_at(idx)` is always `Some`. The sweep reads
   `tc.id` of the same config.
3. Spatial index. `collision_check(idx)` passes `None`, so `run_collision_check`
   (`compute/collision_check.rs:108-115`) builds `SpatialIndex::build_auto(mesh)` on
   `model_by_id(tc.model_id).mesh`. `holder_collision_indices` builds `build_auto` on the same
   `mesh` of the same model id. `build_auto` and `build` (`mesh.rs:437-530`) are serial and read
   only the mesh (bbox, face count, face boxes): no hash order, no thread state. The CMP-24 unit
   test in `collision_check.rs` (`built_here` vs `handed_in`) already holds this. A model with no
   mesh has no index entry, and then `request.mesh` is `None`, so no index is read.
4. Cancel. The CLI `cancel` is `AtomicBool::new(false)` and nothing sets it; `diagnostics()` and
   `triage()` use `no_cancel = AtomicBool::new(false)`.
5. Arms. `Ok(Some)` with a report that is not clear -> `collision_reports`; `Ok(None)` -> nothing;
   `Err` -> `collision_check_failed`. Same inputs, same arms. The CLI `collision_count`
   (`collisions.len()`) still counts obstacle hits too, and the core count is still
   Workpiece-only. That difference is in HEAD; W3a keeps it.
6. Same session state. No `&mut session` use exists between the old loop and `triage()`
   (lines ~935-1125 read the session only), so the two old sweeps saw the same session as the loop.
7. Logs only: a failed check now logs the core warning once, not two times (one per old sweep).
   The CLI warning is unchanged. No output file changes.

GUI / MCP callers (`rg`): viz `holder_collision_counts_by_tp` is a viz-side function on
playback state, not the core sweep. The core callers of `diagnostics()` are
`session/compute/toolpath_diagnostics.rs:523` and tests; they call `diagnostics()` as before.
No caller of `triage()` or `holder_collision_counts` exists outside core and the CLI. The viz
worker calls `run_collision_check` directly; it gets the W3b walk.

## W3b: the interpolated walk runs on rayon

`check_collisions_interpolated_with_cancel` (`stock/collision.rs`):

- `#[cfg(feature = "parallel")]`: the walk goes over move indices in batches of
  `PARALLEL_MOVE_CHUNK = 1024`. Each batch is `(start..end).into_par_iter().map(move_collisions).collect()`.
  An indexed range keeps the order. `move_collisions` is the serial body for one move: it reads
  `moves[i - 1].target` (also across a batch edge) and `moves[i]`, and gives the events in sample
  order and the largest penetration of that move. The batches join the events in move order.
- `max_extra_stickout_needed`: each value that reaches `f64::max` passed
  `> BODY_STRIKE_THRESHOLD_MM`, so it is finite and positive (a NaN fails that test). `f64::max`
  over such values gives the same bits in any order. The code comment says this.
- Cancel rule: the calling thread polls `check_cancel` before each batch. The parallel body does
  not poll: a `&dyn CancelCheck` is not `Sync`, so the compiler refuses it in the closure. The
  cancel latency changes from one sample to one batch of 1024 moves. A cancel still gives
  `Err(Cancelled)`. An empty toolpath with a preset cancel gives `Ok` on both paths, as before.
- `#[cfg(not(feature = "parallel"))]`: the old serial body, unchanged except one comment (the old
  text "one collision per move per segment" was wrong; the `break` leaves the band loop for one
  sample). HANDOVER §6: `--no-default-features` does not build at HEAD, so no build checks this
  path.
- Not changed: the obstacle half `ObstacleToolView::check_toolpath` (`compute/obstacle.rs:1258`)
  has the same per-move shape, but it is analytic (polyline against grown boxes, no drop cutter)
  and the profile does not name it. `run_collision_check` has no other sampling loop.

## Sentries

| Sentry | Before the change | After |
|---|---|---|
| `--test the_collision_walk_is_the_same_in_parallel_perf7` (4 tests) | 4/0 | 4/0 |
| `--lib the_collision_sweep_runs_once_perf6` | 1/0 (the part on HEAD doors) | 1/0 (full) |

- perf7 keeps a verbatim copy of the HEAD serial body. Fixture: 20 mm hemisphere, more than 4000 moves
  (Linear, ArcCW, ArcCCW, Rapid), a shank and a holder that both strike, strikes past move 2048,
  step 1.0 and 0.0. It compares every event field (`to_bits` on each f64), the order and
  `min_safe_stickout`. Cases: a preset cancel (`Err`), a cancel on the second poll (`Err`), an
  empty toolpath with a preset cancel (`Ok`, same report). Defect check: a reversed batch join
  turned it red.
- perf6: five toolpaths, three on one meshed model that strike (`Measured(n > 0)`), one with no
  tool (`Failed`), one on a model with no mesh (`NotApplicable`). It compares each sweep row with
  `collision_check(idx)` (Debug of the report or the error text), `holder_collision_counts` with
  `holder_collision_counts_of(sweep)`, and the Debug of `diagnostics()` / `triage()` with the
  `_with_holder_collisions` forms. Defect checks, each restored after the run:
  - `Err -> NotApplicable` in `holder_collision_counts_of`: red (the fixture-shape assertion).
  - The sweep gives `Ok(None)` for index 2: red at `assert_eq!(from_sweep, per_toolpath)`. This
    is the assertion that carries the W3a claim.
  - `build(mesh, 40.0)` in place of `build_auto` in `holder_collision_indices`: still green. This
    is correct: the drop cutter tests every candidate the index gives, so the cell size changes
    the cost, not the answer. It also shows that the index identity in proof step 3 is a
    sufficient condition, not a necessary one.
  Limit: the fixture has no simulation, so `triage()` is the default triage in both forms.

## Gates (all after the last edit)

- Core `--lib`: 2775 passed, 0 failed, 13 ignored.
- Core sentries, all green: `rapid_check_catches_real_side_strikes_g_rapid6497` (7),
  `rapid_check_catches_shallow_plunges_g_rapidplungetol` (3),
  `a_failed_collision_check_is_not_a_clean_one_core_g_colfail` (3),
  `the_machine_head_guards_the_checks_h1` (4), `diagnostics_json_keys_ses01` (2),
  `project_diagnostics_null_without_trace_u2`, `a_manual_retract_rises_over_the_clamp_k1`,
  `kinematic_surfacing_p4`, `tier_band_overlap_g_overlapfill`,
  `a_rest_op_slower_than_fresh_is_told_r3`, `dowel_holes_have_one_source_p8`,
  `face_stock_top_frame_f028`, perf1, perf5, perf7: 15 binaries, 0 failed.
  The heavy or research binaries that name collisions were not run.
- `rs_cam_cli`: 105 passed, 0 failed.
- `rs_cam_mcp`: 39 passed, 0 failed.
- `rs_cam_viz --lib` (`-j 2`, 2 threads): 695 passed, 0 failed, 2 ignored. Viz is not changed.
- Clippy (workspace, all targets, three features, `-D warnings`): clean.
- rustfmt `--check` on each changed file: clean. Each `git diff` hunk is inside an edited region (no format drift).

## Not done

- No wanaka run and no timing (the lead measures).
- The serial `#[cfg(not(feature = "parallel"))]` path is not built by any gate (HANDOVER §6).
- The core full gate was not run (operator ruling).
