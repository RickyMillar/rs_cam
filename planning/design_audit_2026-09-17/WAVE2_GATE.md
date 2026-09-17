# Wave 2 consolidated gate — 2026-09-17/18 night

Run through `scripts/cargo_lane.sh` on master at `5e10dc80`.

| step | result |
|---|---|
| `fmt --all -- --check` | clean |
| `clippy --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research,rs_cam_core/test-support -- -D warnings` | clean |
| `test -p rs_cam_core --lib -q` | 2508 passed, 0 failed, 12 ignored |
| `test -p rs_cam_viz -q` (all targets, `-j 3`) | 725 passed, 2 failed |
| `test -p rs_cam_cli -q` | 42 passed, 0 failed |
| `test -p rs_cam_mcp -q` | 29 passed, 0 failed |

The two viz reds are both arms of
`the_corridor_bounds_the_band_g_corridor` (the feeds nomogram ceiling on the
`LONG_AND_THIN` fixture). The last commits to their inputs are the power
session's `6aca2ffd` (T-16, 20:10) and `93dd145c` (T-17, 21:47), which
change the bending section the deflection ceiling rests on; the test's own
message says to re-derive `LONG_AND_THIN` if the ceiling leaves the chart.
No wave 2 commit touches `ui/feeds/`, `feeds/` or `tool/`. Handed to the
power session; the wave 3 viz-ui agent is told not to touch it.

The background gate run was killed twice by the low-memory watchdog during
the viz test link step (desktop applications hold ~30 GB); the viz half ran
in the foreground with `-j 3`. Wave 2 verdict: green on every file it owns.
