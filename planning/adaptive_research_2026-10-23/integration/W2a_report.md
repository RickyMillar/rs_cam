# W2a report: the core part of the slope-aware Waterline ladder (D69 §3)

Patch: `W2a.patch` = `git diff 3811e82d0 --` 5 tracked paths + 1 new test file, md5
`6f15fb6b49f03d832a0f932b06aa3da4`. `git apply --check` passes on a clean `3811e82d0` checkout. Nothing is committed.
Pre-registration: `W2_PREREG.md` (approved by the lead after the fact).

## Paths

- `crates/rs_cam_core/src/compute/execute.rs`: `ExecutionContext::material` and `rest_axial_step_mm`, `None` in
  `ExecutionContext::new`.
- `crates/rs_cam_core/src/compute/execute/standing_material.rs`: one cell walk `cells_in_band` for `keeps` (same
  answer) and the new `radial_skin_mm`.
- `crates/rs_cam_core/src/ops/waterline.rs`: `WallStep`, `thin_wall_chain`; `helical_waterline` takes
  `Option<&WallStep>`.
- `crates/rs_cam_core/src/compute/execute/finish_raster.rs`: `WallInputs`, `wall_step_admits` (the deflection check)
  and its lib test `the_wall_step_reads_the_deflection_model_w2`.
- `crates/rs_cam_core/tests/a_helical_waterline_cuts_a_stack_with_one_entry_d69.rs`: the new `None` argument.
- New: `crates/rs_cam_core/tests/a_waterline_widens_its_step_on_a_vertical_wall_w2.rs`.

## Output

No output changes: no caller fills the two fields yet (W2b, after G-C). G-OPGOLDEN, W4, the restlevels sentry and
`steep_shallow_cuts_only_the_rest_w6_4_6` are green with no re-bless.

## Gates

fmt clean; `clippy -p rs_cam_core --all-targets --features heavy-tests,research,test-support -D warnings` clean; the
W2 sentry 4 passed (red check: with the coincidence test removed, `a_drafted_wall_keeps_every_ring` fails); the W1
sentry 3 passed; the lib test passed.

## W2b (waits for G-C)

The session fill in `session/compute.rs` and `session/compute/generation.rs`, and one session-level sentry: a rest
Waterline after a 3D Rough, with material and rpm set, keeps fewer rings than W1, and its maximum simulated axial
engagement is at most the rough DPP.
