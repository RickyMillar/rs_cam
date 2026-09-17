# Wave 1 consolidated gate — 2026-09-17 late evening

Run through `scripts/cargo_lane.sh` on master at `a679eec6` with the power
session's uncommitted edit to `crates/rs_cam_core/src/tool/mod.rs` in the
tree.

| step | result |
|---|---|
| `fmt --all -- --check` | clean |
| `clippy --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research -- -D warnings` | clean |
| `test -p rs_cam_core --lib -q` | 2498 passed, 5 failed, 12 ignored |
| `test -p rs_cam_viz -q` (all targets) | all green, 0 failed |
| `test -p rs_cam_cli -q` | 42 passed, 0 failed |
| `test -p rs_cam_mcp -q` | 29 passed, 0 failed |

The five core failures are all `tool::tests::tip_deflection_*`. On a clean
worktree at HEAD (own target dir) the same filter gives 4 passed, 1 failed:
`tip_deflection_tapered_ball_lies_between_shank_and_tip_limits`, which
`6aca2ffd` (the power session's T-16 fix, 20:10) introduced. The other four
come from that session's uncommitted `tool/mod.rs` edit. No wave 1 commit
touches `tool/`. The operator confirmed the folder is the power session's.

Wave 1 verdict: green on every file it owns.
