# Final consolidated gate — 2026-09-18 morning (after slot 9)

Run through `scripts/cargo_lane.sh` on master at `32f7ca82`, foreground.

| step | result |
|---|---|
| `fmt --all -- --check` | clean |
| `clippy --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research,rs_cam_core/test-support -- -D warnings` | clean |
| `test -p rs_cam_core --lib -q` | 2520 passed, 0 failed, 12 ignored |
| `test -p rs_cam_viz -q` (all targets) | 750 passed, 2 failed |
| `test -p rs_cam_cli -q` | 50 passed, 0 failed |
| `test -p rs_cam_mcp -q` | 31 passed, 0 failed |

The two viz reds are the same `the_corridor_bounds_the_band_g_corridor`
arms as in every gate since wave 2: the power session's T-16/T-17
deflection change moved the `LONG_AND_THIN` fixture's ceiling off the
chart. Not touched by this programme.

Not run: the core integration suite as a whole (each slot ran its folder
sentries; the operator's ruling reserves whole-suite runs), the twelve
`heavy-tests` binaries (CI), the wanaka harnesses. Every feature-gated
target compiles under its feature.
