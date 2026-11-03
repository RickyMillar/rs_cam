# P4 report — the model Z query of the deviation passes

Agent: perf-d. Date: 2026-11-03. Base: HEAD 40cf411a7 plus the working
tree of the other agents (perf-a had `mesh.rs` in progress during every
build below). Nothing is committed.

## 1. The measured cause

The cause is (a): query points left of or below the model's index grid.
There is no frame mismatch (b).

### Instrument

A temporary probe in `compute/simulate.rs` counted the candidates of each
`index.query(x, y, 0.0)` call in `query_model_z_range`. It also evaluated
`contains_point_xy` on every off-grid candidate, and it printed the XY range
of the query points `q` of each group against the model box. I removed the
probe before the hand-off (`grep -c P4PROBE` gives 0).

Run: the release CLI, `project` on `perf/input_head.toml`, toolpath 1 only
(`--skip` of all others), `--resolution 0.2`. The working tree refuses 0.3
mm for this project ("coarser than the 0.2 mm the rest ... needs"), so the
run used 0.2 mm. One CLI run holds three simulations. Each simulation runs
the column pass for two groups and the vertex pass once.

### Frame check (b): no mismatch

| Item | x range | y range |
|---|---|---|
| Query points `q`, group 0 and group 1 (both identity) | 0.000 .. 395.000 | 0.000 .. 460.000 |
| Model mesh (stock-relative frame) | 18.500 .. 376.500 | 8.500 .. 451.500 |
| Stock origin (`stock_min`) | -18.5 | -8.5 |

The query points span the stock-relative stock box, and the model sits
inside it at the world model box less the stock origin. The frames agree.

### Candidate counts (one group, one simulation)

| Pass | Queries | Candidates | Mean | Max |
|---|---|---|---|---|
| Columns | 4 546 776 | 3 502 161 892 | 770.3 | 630 957 |
| Vertices | 9 102 108 | 7 004 235 612 | 769.5 | 630 957 |

| Class (columns pass) | Queries | Share | Candidates | Mean |
|---|---|---|---|---|
| Off-grid low (x or y below the grid) | 294 962 | 6.49 % | 3 398 786 229 (97.0 %) | 11 522.8 |
| — of these, off both low sides (whole mesh) | 3 999 | 0.09 % | 2 523 m (est. 3 999 x 630 957) | 630 957 |
| Off-grid high | 242 788 | 5.34 % | 0 | 0 |
| In the grid | 4 009 026 | 88.2 % | 103 375 663 | 25.79 |

The vertex pass has the same shape: 590 196 off-grid-low queries (6.48 %)
read 6 797 327 694 candidates (97.0 %). No off-grid-low point was accepted
by any candidate (`off_neg_hits = 0` in both passes).

The stock is 18.5 mm wider than the model on the low X side and 8.5 mm on
the low Y side. `SpatialIndex::query` has the `x1 as usize` wrap, so each of
these columns reads a whole row or column band of the grid, and a corner
column reads the whole mesh (630 957 triangles).

## 2. The change

Path: `crates/rs_cam_core/src/compute/simulate.rs`.

`query_model_z_range` is replaced by `ModelZQuery` (one per pass) and the
helpers `fold_model_z_range` and `ModelReach`. `ModelZQuery::z_range(x, y)`
takes one of three paths:

1. The cell of `(x, y)` holds triangles. The candidates are
   `SpatialIndex::cell_triangles_at(x, y)`. No allocation.
2. The cell is empty, `(x, y)` is outside the `ModelReach` box, and no
   ill-conditioned triangle accepts the point. The result is `None`.
3. Any other point reads `index.query(x, y, 0.0)`, as before.

`ModelReach::of(mesh)` makes one pass over the faces (3 to 4 ms on the
630 957-triangle model). It gives a box outside which no triangle with a
safe condition can accept a point, and the list `ill` of the other
triangles. Both deviation passes keep their structure, their parallel
branches and their output order. No other file changes. `Cargo.toml` does
not change. No test door is added.

### Measured effect (same run, release, 0.2 mm, toolpath 1)

| Phase (per call) | Before (probe build) | After |
|---|---|---|
| Column pass, one group | 60.7 .. 64.4 s | 0.15 .. 0.21 s |
| Vertex pass | 128.6 .. 136.7 s | 0.24 .. 0.26 s |
| Whole CLI run | not completed (stopped after 2 of 3 simulations) | 48.25 s wall |

The "before" times include the probe's extra `contains_point_xy` pass over
the off-grid candidates, so they are high by up to about 2x. The lead's
profile of HEAD at 0.3 mm took 300 s for the whole run.

Path counts after the change (one group, columns): path 1 = 3 979 164,
path 2 = 565 890, path 3 = 1 722, and the path 3 queries read 0 candidates.
The vertex pass: 7 961 944 / 1 136 748 / 3 416, 0 candidates. On this model
`ModelReach` is x 18.494227499 .. 376.505773140, y 8.494226860 ..
451.505773140, and `ill` is empty.

## 3. The identity proof

The old result is the min and max of `z_at_xy` over the candidates of
`query(x, y, 0.0)` that `contains_point_xy` accepts, in candidate order.

### Path 1: the same list in the same order

At radius 0 the query computes `floor((x - 0.0 - origin_x) / cs)` and
`floor((x + 0.0 - origin_x) / cs)`. Both are the value of
`cell_triangles_at`'s `floor((x - origin_x) / cs)`. `x - 0.0` is `x` bit for
bit. `x + 0.0` differs only for `x = -0.0`, and both signs of zero give
cell 0. A non-empty `cell_triangles_at` slice means that both indices are in
`0..count` (a NaN gives cell 0 in both functions). Then the query walks that
one cell. The cell list is free of duplicates (`build` writes a triangle
once per cell of its span) and ascending, so the dedup keeps it unchanged.
The fold reads the same list in the same order, so the result is the same
bits, signed zeros and NaN included.

### Path 2: no candidate can accept the point

This path does not use the grid origin. It needs only that no triangle of
the mesh accepts the point, because the query candidates are a subset of the
mesh. Then the old fold sees no accepted triangle and gives `None`.

`contains_point_xy` accepts computed barycentric coordinates down to
`-1e-8`, so it accepts points a small distance outside a triangle. The bound
(written out in the `ModelReach` doc comment) holds for the computed
predicate with its rounding:

- Notation: `A = y1-y2`, `B = x2-x1`, `C = y2-y0`, `E = x0-x2` (all
  computed), the computed `D` (the same expression as the predicate),
  `W = |A|+|B|+|C|+|E|`, `u = 2^-53`.
- A triangle with `|D| < 1e-15` never accepts. `ModelReach` leaves it out.
- A triangle has a safe condition when `1e-9 * W^2 <= |D|`. Then
  `|D - det| <= 2u W^2` and `q = 2u W^2 / |det| <= 2.3e-7`.
- An accepted point has computed `a, b >= -1e-8`, and `c >= -1e-8` gives
  `a + b <= 1 + 1e-8 + 2u`. So `alpha = N_a/det` and `beta = N_b/det` lie
  in the triangle `alpha, beta >= -L`, `alpha + beta <= 1 + L`, `L = 1e-6`.
- `dx = alpha E + beta (x1 - x2) - err`, with `|err| <= q * 1.01 W`.
  The minimum of the linear part over that triangle is at a corner, so the
  point x is at least `min(x0, x1, x2) - 3.4e-6 W`. The maximum and the
  y axis are the same by symmetry.
- `ModelReach` uses the margin `1e-5 W + 1e-12 * (sum of |vertex x, y|)`.
  The second term covers the rounding of the margin arithmetic.
- An infinite point coordinate gives an infinite or NaN `a` or `b`, and the
  predicate then refuses. An overflow gives infinity, never a wrong finite
  value.
- A triangle that fails the safe test (a sliver, NaN, very large values)
  goes to `ill`. Path 2 evaluates `contains_point_xy` on each `ill` triangle
  and takes path 3 if any accepts.
- A NaN point coordinate is never "outside" the box, so it takes path 3.

### The class that is not the same without care

A point left of or below the grid inside the tolerance band IS accepted by a
triangle (for example `x = origin_x - 1e-12`, and also
`origin_x.next_down()`). `cell_triangles_at` alone gives `None` for it. The
old query gives a hit through the row band. `ModelReach` does not exclude
these points (they are inside its margin), so they take path 3 and keep the
old result. The unit test
`a_point_in_the_tolerance_band_left_of_the_grid_still_hits_p4` holds this
class on the left, the bottom and the corner.

### Evidence

- Unit tests (`compute::simulate::model_z_query_tests`) compare `z_range`
  with the old fold over `index.query(x, y, 0.0)`, bit for bit, on a
  lattice past each side of the grid, every exact cell edge, the exact max
  edge of the grid (`origin + cs * count`), the mesh box edges and their
  float neighbours, points `1e-12`, `1e-9` and `1e-6` outside, NaN, plus and
  minus infinity, `-0.0` and `1e300`. Meshes: an offset height field, a
  height field at negative coordinates, and a mesh with a needle, a
  near-collinear sliver, an exactly collinear triangle and a NaN vertex at
  the low edges.
- I injected a zero margin once. All five unit tests went red (for example
  "differs at (2.060000000000001e0, 9.999999999999999e-1)"). I then put the
  margin back.
- On the real job, a temporary build computed both the new and the old
  result for every query and compared the bits: 59 133 756 queries (three
  simulations: column and vertex passes), 0 mismatches. I removed that code.

## 4. Paths touched

- `crates/rs_cam_core/src/compute/simulate.rs` (the change and the unit
  tests).
- `crates/rs_cam_core/tests/column_deviation_is_the_same_and_fast_perf4.rs`
  (new sentry).
- `planning/perf_sweep_2026-11-03/D_REPORT.md` (this file).
- `cargo fmt -p rs_cam_core` ran as instructed. It formats the whole crate.

## 5. The sentry

`column_deviation_is_the_same_and_fast_perf4.rs` runs `run_simulation` on an
identity setup with the stock origin at (10, 20, -8). The stock-relative
stock is 40 x 28 mm. The model (a height field) covers 6..32 x 5..22, so the
stock is larger on all four sides, and 0.2 mm gives 200 x 140 columns (the
parallel branch). It pins the column count, the vertex count, and an FNV-1a
digest of every `ColumnDeviation` field and every per-vertex deviation, bit
for bit. The probe confirmed that this fixture reaches every class: 30 % of
its column queries were off-grid low, 750 in the corner, 7 686 off-grid
high. A second test runs the simulation twice and compares the digests.

The pins came from the code before the change (simulate.rs = HEAD content;
the temporary probe does not change any value).

The sentry needs no door and no `Cargo.toml` row (it uses only the public
`run_simulation`).

## 6. Test commands and results

All through `scripts/cargo_lane.sh`.

| Command | Result |
|---|---|
| `test -p rs_cam_core -q --test column_deviation_is_the_same_and_fast_perf4`, BEFORE the change | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.38s` |
| same, AFTER the change (final code) | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s` |
| `test -p rs_cam_core -q --lib compute::simulate` (final code) | `test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 2779 filtered out; finished in 0.29s` |
| `test -p rs_cam_core -q --lib model_z_query` | `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 2789 filtered out; finished in 0.07s` |
| `check -p rs_cam_core --no-default-features` | FAILS, not in my files: `cannot find module or crate rayon` in `dexel_stock/band_batch.rs:49`, `dexel_stock/band.rs:38,185`, `dexel_stock/simulation.rs:33`, `dexel_stock/swept.rs:46`, `surface/pushcutter.rs:262`, `surface/slope.rs:186`. No error names `simulate.rs`. |
| `clippy -p rs_cam_core --all-targets --features heavy-tests,research,test-support -- -D warnings` | `Finished`, no warning |

The `rg -l "column_deviation|ColumnDeviation|compute_deviations|deviation" crates/*/tests`
list: I ran every core binary on it that is not behind `heavy-tests` or
`research` (32 binaries, `-j 4 --no-fail-fast`). 31 gave `test result: ok`.
One failed:

- `tier_arcfit_burial_f4`: `test result: FAILED. 0 passed; 1 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.22s`.
  The panic is at generation, before any simulation:
  `generates: Operation(NotSet(NotSet { op: "Unified Finish", kind: Feeds, fields: ["feed_rate", "plunge_rate", "spindle_rpm"] }))`.
  It does not reach the deviation code. I did not run it at HEAD.

The viz binaries on the list (`-j 2`): `an_arc_move_draws_on_its_arc_r_arc`,
`cycle_time_basis_g_timeest`, `the_viewport_dock_reaches_every_option_g_vpdock`
and `component_contracts_up2` gave `test result: ok` (2, 23, 12 and 11
passed). `mcp_authoring_surface` gave
`test result: FAILED. 8 passed; 1 failed; ...`:
`set_tool_param's description omits the accepted param non_centre_cutting`.
That is MCP description text, not the deviation code.

I did not run the `heavy-tests` and `research` binaries on the list
(`feed_modulation_cycle_time_f036c`, `checkpoint_b_resolution_ab`,
`machine_kinematics_cycle_time_f034`, `scallop_isofield_gouge_m4` and the
research harnesses).

## 7. The other radius-0 `index.query` callers (not edited)

Each one has the same band cost for a point left of or below the grid. Each
one that only tests for a hit (`any`) still reads the whole band on a miss.

| Caller | Use | Call count (estimate) |
|---|---|---|
| `surface/dropcutter.rs:499` `point_is_over_mesh_xy` (perf-a, `with_query` in P1) | "is the point over the mesh" | one per covered drop-cutter point, per chord-refine probe, per unified-finish route point: millions per 3D finish |
| `maps/reach_map.rs:1015` `surface_z_at` | the zero-radius surface Z | one per reach-map grid cell (`:707`) and per vertex (`:754`, `:1517`, `:1846`); of the order of the map grid, about 10^6 on a wanaka-size map; also `ops/project_curve.rs:364`, `finish/surface_link.rs:355,360`, `metrology/monge.rs:156` |
| `ops/project_curve.rs:135` `point_over_triangle` (perf-b deletes it in P2) | the hole test | one per projected point; 10^4 .. 10^5 per Project Curve op |
| `rs_cam_viz/src/app/gpu_upload/drawing.rs:185` | the Z of a drawn curve point on the surface | one per drawing vertex per upload; 10^3 .. 10^5 |

`ModelZQuery` / `ModelReach` could serve the hit test and the max-Z read of
these callers, because they ask the same predicate. That needs one shared
home (probably `mesh.rs` or `surface/`), which is outside my ownership.

## 8. Notes for the lead

- `ModelZQuery` path 1 depends on `cell_triangles_at` and the radius-0
  query reading the same cell. A `SpatialIndex::origin()` accessor is not
  needed: path 2 does not use the origin, and path 3 is the old call.
- If perf-a changes the radius-0 query output (it must not), path 1 still
  matches the pre-P4 behaviour and the unit tests go red.
- The integration sentry pins the output only. The "fast" claim is held by
  the unit test `a_point_far_off_the_low_sides_is_excluded_p4` and by the
  measured path counts above.
