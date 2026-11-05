# E report — round 2, package W1 (the low-side wrap in `SpatialIndex`)

Implementer report, 2026-11-06, on HEAD 0c05e90e7. Nothing is committed.

## 1. The option chosen: (b)

I chose option (b). `query_rect_into` is unchanged. The `ModelReach` box of
`compute/simulate.rs` moved into `mesh.rs` as the private `PointReach`. The
index builds it in `SpatialIndex::build` and keeps it in a new field. One new
door reads it: `SpatialIndex::with_point_candidates(mesh, x, y, f)`.

### Why not (a)

Option (a) needs a cell bound that keeps every triangle that any caller can
accept. Two facts break a plain bound:

1. An ill-conditioned sliver accepts points far outside its own box. A random
   search over doubles found one. The sentry holds it as `SLIVER`:
   `contains_point_xy` accepts `SLIVER_POINT`, which is 3.39 mm left of the
   sliver's min X. Its `1e-5 * W` margin is 2.2e-3 mm. No cell bound from the
   `ModelReach` margin keeps it. A narrowed primitive must carry the ill list
   and merge the ill triangles back into the old cell-major order.
2. Each caller has its own slack past its rectangle.
   `drop_cutter_can_contact` accepts a triangle up to `DROP_CONTACT_SLACK_MM`
   (1e-4 mm) past the cutter radius. `classify_probe` calls `drop_cutter` on
   every candidate with no early-out. `stock_edit::vertical_hit` uses its own
   edge function, not `contains_point_xy`. A bound in the primitive must
   cover the largest slack of every present caller and every future caller.

A design of (a) exists: cap the column range at
`floor((x_max + S - origin) / cs)` with
`S = 2 * (max safe margin + DROP_CONTACT_SLACK_MM)`, and append the ill
triangles in old order. It keeps the kept triangles as an ordered subsequence
of the old list. But it couples `mesh.rs` to the slack constants of its
callers. It also changes the raw vector, so `a_spatial_query_reuses_its_scratch_perf1`
(`every_query_path_returns_the_reference_vector` and
`the_scratch_stays_clean_across_meshes_of_different_size`) goes red and needs
an edit. Option (b) changes no raw query result, needs no gate edit, and
reuses a proof that round 1 reviewed. So I did (b).

### The proof of (b)

`with_point_candidates` has three paths. These are the P4 paths of
`ModelZQuery`, now shared:

1. The point is in an index cell that holds triangles. The door passes
   `cell_triangles_at(x, y)`. A point in the grid makes the radius-0 query
   take its single-cell path (`x0 == x1`, `y0 == y1`), so the list is the
   same list, element for element.
2. The point is outside the `PointReach` box and no `ill` triangle accepts
   it. The box proof (doc on `PointReach`, copied from `ModelReach`): a safe
   triangle (`1e-9 * W^2 <= |D|`) accepts only points in its box widened by
   `3.4e-6 * W`; the margin is `1e-5 * W + 1e-12 * size`. A triangle with
   `|D| < 1e-15` accepts no point. Every other triangle (NaN too) is in
   `ill`, and the door tests each one. So no triangle of the mesh accepts
   the point. The door passes an empty list. Every caller filters by
   `contains_point_xy`, so over the old list it also kept nothing.
3. Any other point: the door calls `with_query(x, y, 0.0, f)`, the old path.

The contract, in the doc: after the `contains_point_xy` filter the result is
the radius-0 query's result, triangle for triangle, in the same order. Path 1
gives the same raw list. Path 2 gives an empty list where the old filtered
list was empty. Path 3 is the old list. So a fold, a first hit, an `any` and
a max or min give the same bits.

`excludes` now treats an `ill` index that the passed mesh does not hold as
"cannot prove" (it falls back to the query). With the right mesh this branch
never runs.

## 2. Files changed

- `crates/rs_cam_core/src/mesh.rs` — `PointReach` (moved from simulate),
  the `point_reach` field built in `build`, `with_point_candidates`, the new
  wrap docs on `build` and `query_rect_into`, two unit tests moved from
  simulate (`a_point_far_off_the_low_sides_is_excluded_p4`,
  `slivers_and_bad_triangles_go_to_ill_p4`).
- `crates/rs_cam_core/src/compute/simulate.rs` — `ModelZQuery` reads the
  door; `ModelReach` deleted; its two box tests moved to `mesh.rs`. The P4
  bit-identity tests stay and pass.
- `crates/rs_cam_core/src/surface/dropcutter.rs` — `point_is_over_mesh_xy`
  reads the door; its cost doc updated.
- `crates/rs_cam_core/src/maps/reach_map.rs` — `surface_z_at` reads the door
  (it also no longer allocates a `Vec` per call).
- `crates/rs_cam_viz/src/app/gpu_upload/drawing.rs` — the Project Curve
  `point_z` closure reads the door.
- `crates/rs_cam_core/tests/an_off_grid_query_reads_no_band_perf5.rs` — new
  sentry.
- `crates/rs_cam_core/src/mesh/CLAUDE.md`, `src/surface/CLAUDE.md`,
  `src/maps/CLAUDE.md` — name the new sentry (each file stays at 40 lines).

## 3. The sentry

`crates/rs_cam_core/tests/an_off_grid_query_reads_no_band_perf5.rs`. The
reference is the pre-W1 query body, copied over the public API. Meshes: a
wavy field at an offset origin, the field plus a long safe triangle
(`W` about 2000) at the low edge, and a field plus the ill sliver. Each mesh
at `build_auto` and one explicit cell size. Points: a lattice past each side,
each cell edge, each low and high edge with float neighbours, the band
`min - {1e-12, 1e-9, 1e-7, 1e-6, 1e-5, 1e-4, 2e-2, cell/2}`, far values,
NaN, the infinities and `±1e300`, all
combined on X and Y (so each low side and each low corner), plus
`SLIVER_POINT`.

| Test | Before the change | After |
|---|---|---|
| `the_sliver_accepts_a_point_far_left_of_its_box` | pass | pass |
| `the_query_is_the_reference_vector` (radius 0, 0.1, cell, 2 x extent) | pass | pass |
| `the_radius_zero_callers_give_the_reference_result` (`point_is_over_mesh_xy`, `surface_z_at`) | pass | pass |
| `the_drop_cutter_gives_the_reference_cl_point` (ball 6, flat 3) | pass | pass |
| `the_exact_door_gives_the_reference_result_under_each_predicate` | not present (door absent) | pass |
| `a_point_far_off_the_low_sides_reads_no_band` (fast) | not present (door absent) | pass |

Before: 4 passed, 0 failed. After: 6 passed, 0 failed.

Defect injection: I made `excludes` skip the `ill` list. Two tests went red
(`the_radius_zero_callers_give_the_reference_result`,
`the_exact_door_gives_the_reference_result_under_each_predicate`), on the
sliver mesh. I restored the code.

## 4. Per caller class

All non-test callers of `query`, `with_query`, `query_into` and
`query_rect_into` (`rg`, multi-line calls included; the `session.query`
hits are a different `query`):

| Class | Callers | Why the output is unchanged |
|---|---|---|
| A. Radius 0, `contains_point_xy` filter, moved to the door | `dropcutter::point_is_over_mesh_xy`, `reach_map::surface_z_at`, `simulate::ModelZQuery::z_range` (both deviation passes), viz `gpu_upload/drawing.rs` `point_z` | The proof in §1. The sentry pins each predicate shape (any, max plane Z, min/max fold, first hit, the accepting list). |

The door needs the mesh that the index was built from. I checked the 15
call sites of `point_is_over_mesh_xy` and `surface_z_at` (`rg`): each one
passes the `(mesh, index)` pair that its function also gives to the drop
cutter or to the radius-0 query, so a wrong pair already gave wrong output
before W1. `ModelZQuery` builds or receives the index of its own mesh. The
GUI builds the index from the same transformed mesh in the same closure.
| B. Radius 0, not moved | `finish/conformal_spiral.rs:2064` `locate_exact` (research arm), `finish/classify_probe.rs:355` (study arm 0b) | Code unchanged, so output unchanged. Both filter by `contains_point_xy`, so both can move to the door later. |
| C. Radius 0, other predicate | `dexel_stock/stock_edit.rs:270` (`vertical_hit` edge function) | Code unchanged. The box proof covers `contains_point_xy` only, so this caller must keep `query_into`. Its node box starts up to one dexel cell left of the mesh box, so it does hit the wrap for the first column and row of nodes. |
| D. Radius > 0 or a rectangle | `point_drop_cutter` (`dropcutter.rs:32`), `tier_map` x3, `horizontal_finish:352`, `classify_probe:342,597`, `pushcutter:148`, `metrology/monge.rs:329`, benches | The primitive is unchanged, so the raw list is unchanged. The wrap fires for these only when the whole rectangle is off the low side. |

## 5. Gate results

All through `scripts/cargo_lane.sh` with `timeout`:

- New sentry: 6 passed, 0 failed.
- Round-1 sentries (`a_spatial_query_reuses_its_scratch_perf1`,
  `project_curve_is_the_same_in_parallel_perf2`,
  `scallop_is_the_same_in_parallel_perf3`,
  `column_deviation_is_the_same_and_fast_perf4`) with the new one in one run:
  binaries 4 + 6 + 2 + 2 + 2 passed, 0 failed.
- `cargo test -p rs_cam_core -q --lib -- mesh surface maps compute::simulate`:
  296 passed, 0 failed, 3 ignored.
- Folder sentries: `drop_cutter_off_mesh` 5/0, `reach_map_p5` 1/0 (3
  ignored), `reach_map_residual_p5_1` 7/0 (1 ignored), `tier_map_walk_t1`
  16/0 (1 ignored), `geometry_cache_g8` 7/0 (1 ignored),
  `band_run_off_reproduction_d16_1` 9/0.
- `cargo test -p rs_cam_viz -q --lib -j 2 -- --test-threads 2`: 695 passed,
  0 failed, 2 ignored.
- `cargo clippy --workspace --all-targets --features rs_cam_core/heavy-tests,rs_cam_core/research,rs_cam_core/test-support -- -D warnings`:
  clean (exit 0). The only output is the old future-incompat note for
  `nom` and `quick-xml`.
- `cargo test -p rs_cam_mcp -q`: 39 passed, 0 failed (plus an empty doc
  binary).
- Format: `rustfmt --edition 2024` on my six Rust files only. It changed
  only my hunks.

## 6. Not done, and notes for the lead

- I read `HANDOVER.md` §5 and `D_REPORT.md` §7-8 after the sentry and the
  first draft of the code. They name the same helper and the same four
  callers; nothing changed. D_REPORT §7 gives the call-count estimates
  (millions of `point_is_over_mesh_xy` calls per 3D finish, about 10^6
  `surface_z_at` calls on a wanaka-size map).
- No wanaka-size call-count probe (brief step 4). I built no
  instrumentation. The lead's full wanaka run before and after is the
  measure.
- I did not run the lead gate `every_op_keeps_its_toolpath_g_opgolden` or
  the core `--lib` in full. The op-golden gate is the check that catches a
  divergence that the sentry cannot construct.
- `SpatialIndex::build` now makes one more pass over the faces (about 20
  flops per triangle) to build `PointReach`. `ModelZQuery::new` no longer
  makes that pass, so the deviation passes do not pay it twice. The
  `spatial_index/build_terrain` bench may move a little.
- Open item, not mine, and not a defect claim (a worst-case bound, no
  measured miss): the G3 proof of `drop_cutter_can_contact` says a
  facet contact lies within `radius()` of `cl`. `facet_drop` tests the CC
  point with `contains_point_xy`, which accepts up to `3.4e-6 * W` outside a
  safe triangle. For `W` above about 29 mm that is more than
  `DROP_CONTACT_SLACK_MM`. The bound is a worst case, not a measured miss.
  The lead can decide if it needs a check.
- Follow-up candidates: move class B callers to the door. If the lead later
  wants class D narrowed too, §1 "Why not (a)" gives the design and its
  coupling cost.
