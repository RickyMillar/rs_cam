# Wave 3 consolidated gate — 2026-09-18 00:30

Run through `scripts/cargo_lane.sh` on master at `1cafe62b`, foreground, `-j 3/4`.

| step | result |
|---|---|
| `fmt --all -- --check` | clean |
| `clippy --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research,rs_cam_core/test-support -- -D warnings` | clean |
| `test -p rs_cam_core --lib -q` | 2505 passed, 0 failed, 12 ignored |
| `test -p rs_cam_viz -q` (all targets) | 730 passed, 2 failed |
| `test -p rs_cam_cli -q` | 48 passed, 0 failed |
| `test -p rs_cam_mcp -q` | 31 passed, 0 failed |

The two viz reds are the same `the_corridor_bounds_the_band_g_corridor`
arms as in `WAVE2_GATE.md`: the power session's T-16/T-17 deflection
change moved the `LONG_AND_THIN` ceiling off the chart. No wave 3 commit
touches `ui/feeds/`, `feeds/` or `tool/`. Wave 3 verdict: green on every
file it owns.
