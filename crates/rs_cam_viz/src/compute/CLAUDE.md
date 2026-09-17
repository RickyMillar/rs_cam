# viz `compute/` — the worker lanes

The background workers that run generation, simulation and optimisation off
the UI thread. The entry point is `compute::worker`.

## Files

- `mod.rs` — the lane facade and the request and result types.
- `worker.rs` and `worker/` — the worker loop, the per-operation execute
  helpers and the test fixture builder.

## Invariants

- The worker calls the core door. It does not re-implement generation. A
  worker result must agree with `ProjectSession` for the same inputs.
- A result arrives with the revision it was computed for. The controller
  rejects a result whose revision no longer matches.
- A generation-input change drops the affected result. It does not paint a
  stale label over a live one.

## Sentries

- `cargo test -p rs_cam_viz -q --test generate_all_fixpoint_parity`
- `cargo test -p rs_cam_viz -q --test export_parity_core_vs_gui_p0`
- `cargo test -p rs_cam_viz -q --test optimize_runs_on_the_job_lane_wp14b`
- `cargo test -p rs_cam_viz compute::worker::tests::`

## Do not

- Do not run a heavy job on the UI thread. Do not start a second cargo build
  beside a running one.
