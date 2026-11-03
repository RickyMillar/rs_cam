# P2 report: `project_curve` projects its points in parallel (agent perf-b)

## What changed

- `project_polygon_rings_with_cancel` now calls `project_tip_zs` once for each ring.
  This function returns the `Option<f64>` tip Z of every resampled point, in order.
  The chain and emission loop then runs serially over that vector. The loop is unchanged.
- `surface_tip_z` holds the per-point body of the old loop (Cutter and Point arms).
  The code is the same as before.
- With `parallel` (the default), `project_tip_zs` walks the points in chunks of
  `PARALLEL_CHUNK = 1024`. It calls `check_cancel(cancel)?` on the calling
  thread before each chunk. It then runs `chunk.par_iter().map(..)` into
  `out.par_extend(..)`. An indexed `par_extend` keeps the order.
- Without `parallel`, `project_tip_zs` is the old serial loop. It polls every
  `CANCEL_POLL_STRIDE = 64` points. Both constants are `cfg`-gated, so each
  build has no dead constant.
- `point_over_triangle` is deleted. The code now calls
  `crate::surface::dropcutter::point_is_over_mesh_xy`. At HEAD both functions are
  `index.query(x, y, 0.0)` followed by "any face with `contains_point_xy(x, y)`". This is
  the same predicate with the same short-circuit. The sentry hash did not change.
- `Sync` check: `TriangleMesh` and `SpatialIndex` (plain `Vec`s, no `Cell`) and
  `&dyn MillingCutter` (`MillingCutter: Send + Sync`) enter the closure. The
  compiler accepts it. `cancel` does not enter the closure. No public signature changed.

## Chunk size: 1024

The old code polled every 64 points on one thread. 1024 is 16 × 64. On 16 or more
threads, one chunk takes about the wall time of 64 serial points. Thus the cancel latency
does not change on the operator PC (24 cores). On one thread, the latency is 16 × longer.
That is about 1024 drop-cutter calls, which takes milliseconds.
A chunk of 1024 also gives each of 24 threads about 40 points. Thus the fork-join cost is
small against the drop-cutter work. A smaller chunk (for example 64) leaves most threads idle.

## Paths touched

- `crates/rs_cam_core/src/ops/project_curve.rs`
- `crates/rs_cam_core/tests/project_curve_is_the_same_in_parallel_perf2.rs` (new)
- `crates/rs_cam_core/Cargo.toml` is not changed. Tests auto-register, and
  the self dev-dependency turns on `test-support`.

## The sentry

`project_curve_is_the_same_in_parallel_perf2` builds a 60 × 60 mm wavy heightfield
with a 10 × 10 mm hole. It generates five cases with spacing 0.02 mm, so each
ring has about 3 000 to 8 000 points (3 to 8 chunks):

- a closed ring with a hole ring, `Cutter`
- the same ring with a hole ring, `Point`
- the same ring with a hole ring, `Cutter`, `FromBelow`, `Outside`
- an open zigzag, `Cutter`
- the same open zigzag, `Point`

Every ring crosses the mesh hole or the mesh edge, and the test asserts that some
points fall off the surface. It pins (move count, rapid count, FNV-1a hash). The hash covers
each move's target bits, move type with its feed and arc bits, and intent. A second test
uses a non-`Sync` `Cell` cancel that answers true at poll 3 (inside the first ring).
The test asserts that the op returns `Err` and stops at poll 3.

Pinned values (from the code before P2):
`closed_cutter_center (8692, 8, 0x14dfa40567f9ff42)`,
`closed_point_center (8750, 8, 0x6d358162073ba45d)`,
`closed_cutter_outside_from_below (10470, 10, 0x7bd170e66e2ae681)`,
`open_cutter (7414, 8, 0x614e1d1fac397705)`, `open_point (7414, 8, 0x62e4e1e50675eb41)`.

- Before (current code, `project_curve.rs` equal to HEAD, values pinned):
  `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.92s`
- After:
  `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s`
- Defect check: I changed `par_iter()` temporarily to `par_iter().rev()`. All five hashes
  changed, and the result was `test result: FAILED. 1 passed; 1 failed; ...`. I then restored
  the file.

The "before" run used the shared tree, which already held the perf-a edits to `mesh.rs` and
`dropcutter.rs`. The pins thus include perf-a's `point_drop_cutter`. They do not
prove that perf-a matches HEAD.

## Test commands (all through `scripts/cargo_lane.sh`)

| Command | `test result:` |
|---|---|
| `test -p rs_cam_core -q --lib ops::project_curve` | `ok. 10 passed; 0 failed; 0 ignored; 0 measured; 2784 filtered out` |
| `--test project_curve_depth_sign` | `ok. 4 passed; 0 failed` |
| `--test project_curve_from_below_on_a_flipped_setup_w6_5` | `ok. 3 passed; 0 failed` |
| `--test a_buried_project_curve_is_named_w6_5` | `ok. 2 passed; 0 failed` |
| `--test project_curve_deviation` | `ok. 1 passed; 0 failed; 1 ignored` |
| `--test op_precondition_static_validation_f015` | `ok. 6 passed; 0 failed` |
| `--test every_op_states_its_capabilities_w2_4` | `ok. 21 passed; 0 failed` |
| `--test schema_enum_values_g_schemaenum` | `ok. 2 passed; 0 failed` |
| `--test one_link_ceiling_for_every_op_g_linkceil` | `ok. 4 passed; 0 failed` |
| `--test capability_link_moves_safety` | `ok. 17 passed; 0 failed` |
| `--test retract_intent_move_type_census_w6` | `ok. 2 passed; 0 failed` |
| `--test lookup_parity` | `ok. 3 passed; 0 failed` |
| `--test param_sweep` | `ok. 0 passed; 0 failed; 52 ignored` |
| `--test a_vbit_row_is_read_at_its_printed_key_b4` | `ok. 9 passed; 0 failed` |
| `--test every_op_keeps_its_toolpath_g_opgolden` | `FAILED. 0 passed; 1 failed; 1 ignored` (see below) |
| `--test simulation_issue_channel_m1` | `FAILED. 0 passed; 2 failed; 1 ignored` (see below) |
| `clippy -p rs_cam_core --all-targets --features heavy-tests,research,test-support -- -D warnings` | `Finished`, no warning |
| `check -p rs_cam_core --no-default-features` | 7 errors, none in `project_curve.rs` (see below) |
| `fmt -p rs_cam_core -- --check` | clean |

### The red results are not from P2

- `every_op_keeps_its_toolpath_g_opgolden`: all seven `project_curve/*` rows print `ok`.
  The 46 changes are in `adaptive3d/by_area_helix_cp`, `adaptive3d/fresh`, `pencil/rest`,
  `pencil/rest_depth_rest`, `scallop/iso_field_rest`, `scallop/rest`,
  `steep_shallow/rest_regions` and `unified_finish/claims_rest_clip`. Example:
  `adaptive3d/fresh move_count 368 -> 372`. None of these ops calls `project_curve`. The
  dirty tree also holds edits to `mesh.rs`, `dropcutter.rs`, `ring_generation.rs` and
  `compute/simulate.rs`. **The lead must find which one moves these toolpaths. The plan
  requires byte-identical output.**
- `simulation_issue_channel_m1`: `generate pocket-real: Operation(NotSet(NotSet { op:
  "Pocket", kind: Feeds, fields: ["feed_rate", "plunge_rate", "spindle_rpm"] }))`. This is
  a Pocket feeds precondition. It is not related to project_curve.
- `--no-default-features`: the errors are `cannot find module or crate rayon` in
  `dexel_stock/band.rs`, `band_batch.rs`, `simulation.rs`, `swept.rs`,
  `surface/pushcutter.rs` and `surface/slope.rs`. These files are not changed against HEAD,
  so this build fails at HEAD too. Thus the compiler did not check my serial
  `#[cfg(not(feature = "parallel"))]` arm. That arm is the old loop, moved without change.

### Not run

- `air_cut_family_calibration_w5bf4`: it requires `heavy-tests`. I did not ask for a heavy run.
- `direction_field_wanaka_f1` (requires `research`), `wanaka_defaults_validation` and
  `wanaka_suggest_integration`: these files name `project_curve` only in comments and feeds
  tables. They do not generate it.
- `rs_cam_viz` and `rs_cam_cli`: `rg -l project_curve` finds only
  `crates/rs_cam_viz/tests/snapshots/mcp_wire_surface.json`, a wire snapshot. No test binary
  there names it.

## What the plan got wrong

- The plan says "keep a serial path under `#[cfg(not(feature = "parallel"))]`".
  `--no-default-features` does not compile at HEAD, so no build can check that path today.
- The plan gives no chunk size. I chose 1024 for the reasons above.
- Nothing else. `point_over_triangle` is the same predicate as `point_is_over_mesh_xy`, as the plan says.
