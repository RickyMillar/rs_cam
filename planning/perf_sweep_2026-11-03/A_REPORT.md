# P1 report (agent perf-a): `SpatialIndex::query` reuses its scratch

## What changed

- `SpatialIndex::query` keeps its signature. It now calls `query_into` with a
  thread-local `QueryScratch`. It allocates only the returned `Vec`. It does
  not allocate the whole-mesh bitset.
- The new `SpatialIndex::with_query(cx, cy, radius, f)` gives `f` a
  `&[usize]`. The result buffer and the scratch are thread-local. In steady
  state the caller allocates nothing.
- The two thread-local slots are `Cell`s. The code takes each buffer out with
  `Cell::take` and puts it back with `Cell::set`. No borrow is held while
  `f` runs. The scratch goes back before `f` runs, so a nested call reuses
  it. A nested call gets an empty result buffer of its own.
- The helpers use `LocalKey::try_with`. During thread teardown they use a
  fresh buffer and do not panic.
- `point_drop_cutter` and `point_is_over_mesh_xy` use `with_query`. No other
  caller changed.
- The doc comment of `point_is_over_mesh_xy` is now true, and it says why.
  For a point inside the grid, both ends of the radius-0 rectangle fall in
  one cell, so `query_rect_into` takes its single-cell fast path.
- The doc comment of `query_into` said that `query` allocates a whole-mesh
  bitset. It now says that this was the state before P1.

## Paths touched

- `crates/rs_cam_core/src/mesh.rs`
- `crates/rs_cam_core/src/surface/dropcutter.rs`
- `crates/rs_cam_core/tests/a_spatial_query_reuses_its_scratch_perf1.rs` (new)
- `planning/perf_sweep_2026-11-03/A_REPORT.md` (this file)

`Cargo.toml` did not change. The crate discovers tests automatically. The
self dev-dependency turns `test-support` on, so `make_test_hemisphere` needs
no feature flag.

No accessor was added. The grid origin is `mesh.bbox.min` by the definition
in `SpatialIndex::build`, so the reference reads the origin from the mesh.

## The sentry

`reference_query` in the test is a copy of the old `query` body over the
public API. The test holds these claims:

1. `query`, `query_into` and `with_query` return the reference vector, index
   for index. The probes are a 61-value lattice per axis from 2.5 cells
   before the grid to 2.5 cells past it. They also include the min edge, the
   mesh max edge and the grid max edge, each at ±1e-9 and ±0.5 cell, and
   ±1e3, ±1e6 and 0. The radii are 0, 0.1, the cell size and twice the mesh
   extent. The meshes are the hemisphere and a non-square 40 × 9 mm strip
   with an origin off zero, each with `build_auto` and one fixed cell size.
   The test asserts that the probe set gives both empty and non-empty results.
2. `point_drop_cutter` (ball 6 mm and flat 3 mm) gives the same `x`, `y`, `z`
   bits and `contacted` as the old loop over `reference_query`.
   `point_is_over_mesh_xy` gives the same answer as the old predicate.
3. Nested case: `with_query` inside `with_query`, with `query` and a third
   `with_query` inside that. Each level gets the right result. The outer
   slice does not change. The buffers give correct results after the nest.
4. One thread queries a large mesh and a small mesh in turn, three times.
   A bit left set in the scratch would drop a triangle.

Claims 1 and 2 ran against the code before the change. The `with_query`
parts of claim 1 and claims 3 and 4 need the new function, so they were
added after the change.

## Test commands and results

All commands ran through `scripts/cargo_lane.sh` from the repo root.

| Command | `test result:` line |
|---|---|
| `test -p rs_cam_core -q --test a_spatial_query_reuses_its_scratch_perf1` (BEFORE the change, claims 1–2) | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.14s` |
| same (AFTER the change, claims 1–4) | `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.95s` |
| same, with a defect injected in `with_query` (`f` gets the list without its last entry), then the file restored | `test result: FAILED. 0 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s` |
| `test -p rs_cam_core -q --lib mesh` | `test result: ok. 111 passed; 0 failed; 2 ignored; 0 measured; 2676 filtered out; finished in 0.44s` |
| `test -p rs_cam_core -q --lib surface::dropcutter` | `test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 2766 filtered out; finished in 0.01s` |
| `test -p rs_cam_core -q --test drop_cutter_off_mesh` | `test result: ok. 1 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s` |
| `test -p rs_cam_core -q --test finish_resolution_policy_pr3 --test strategy_comparison_h4 --test band_run_off_reproduction_d16_1 --test tier_fine_burial_sources_g_tierburial` (the `rg -l` hits) | four lines: `ok. 5 passed; 0 failed; 0 ignored`, `ok. 10 passed; 0 failed; 0 ignored`, `ok. 1 passed; 0 failed; 4 ignored`, `ok. 0 passed; 0 failed; 1 ignored` |
| `test -p rs_cam_core -q --lib` | `test result: ok. 2776 passed; 0 failed; 13 ignored; 0 measured; 0 filtered out; finished in 68.98s` |
| `clippy -p rs_cam_core --all-targets --features heavy-tests,research,test-support -- -D warnings` | `Finished`, no warning |
| `check -p rs_cam_core --no-default-features` | FAILS, 7 errors, none in my files (see below) |

The `--test drop_cutter_off_mesh` run ignores 3 heavy tests; I did not run
them with `heavy-tests`.

`rustfmt --check` passes on the three files I touched.
`cargo fmt --all -- --check` fails on files of other agents
(`compute/simulate.rs`, `rs_cam_mcp/src/server.rs`, two viz files).

## Things the plan or the brief got wrong

- `cargo check -p rs_cam_core --no-default-features` does not compile at
  HEAD. The errors are `cannot find module or crate rayon` and
  `cannot find trait IndexedParallelIterator` in
  `dexel_stock/band_batch.rs:49`, `dexel_stock/band.rs:38`, `:185`,
  `dexel_stock/simulation.rs:33`, `dexel_stock/swept.rs:46`,
  `surface/pushcutter.rs:262` and `surface/slope.rs:186`. These files have
  no diff against HEAD. The P1 code uses no rayon and has no `cfg` on
  `parallel`.
- The old comment at `dropcutter.rs:497` was false for two reasons. The old
  `query` had no fast path at all. Also, a radius-0 query is single-cell
  only for a point inside the grid. A point at negative X or Y spans a whole
  row or column, because `x1 as usize` wraps and then clamps to the last
  cell. A point past the max edge returns an empty list. The new comment
  states all three cases.
- `query()` still allocates its returned `Vec`. Only `with_query` is free of
  allocation in steady state. The other callers gain only the bitset saving.
- Each thread keeps one scratch of `total_triangles / 8` bytes for the
  largest mesh that it queried (about 81 KB at 650 k triangles). The scratch
  does not shrink. On rayon this is one scratch per worker thread.
