# Removal step R6: the rename, the load-only fields and the docs

Date: 2026-10-28. Branch `geodesic-integration`, base `21f3b3f37` (R5).
The implementer did not commit. Design: `removal_design.md` §5.

Patch: `stepR6_tree_vs_21f3b3f37.patch`, md5
`b7905a6e5a8962aa55297bd73bc8ab0c`. 17 files (+183/-133). No planning file.
`git apply --check -R` passes.

## 1. The rename

`clear_z_level_agent_2d_slice` is now `clear_z_level_slice` in every crate
file: `clearing.rs`, `path.rs`, `tests.rs` and three test files
(`adaptive3d_entry_coalescing_f038`, `adaptive3d_emission_byte_parity`,
`agent_search_coverage`). The sentry file name `agent_search_coverage` stays:
it is a file name, and the lead did not rule a rename. `removal_design.md`
still names the old function at lines 132 and 145; it is a design record.

## 2. The load-only fields

`Adaptive3dConfig::trochoid_cap_mult` and `engagement_measure` now carry the
param help "Not read by 3D Rough: the strategy that read it is removed. Kept
so saved projects load unchanged." This follows the 2D precedent
(`AdaptiveConfig::engagement_measure`). The docs on `Adaptive3dParams`, on
the slice context and on `default_trochoid_cap_mult` say the same.

Why no reader is left:

- `trochoid_cap_mult` is read only on the `PathStrategy2d::ContourSpiral`
  path (`adaptive/path.rs`, `spiral_passes`). The slice sets
  `PathStrategy2d::Agent`.
- `engagement_measure` is read only by `pass_load_for` under
  `KeepDownLinks::RetractedByCaller`. The slice sets `WithinPassLoad`
  (R5), and the geodesic door passes it through unchanged.
- Contour Parallel builds no 2D params.

The values still pass through to `params_2d`. The patch does not delete
that pass-through: a delete needs a new typed value in its place. It is a
small follow-up if the lead wants it.

New test: `--lib adaptive3d::tests::the_load_only_fields_do_not_change_the_toolpath_r6`.
On the hemisphere, for Contour Parallel and Geodesic Adaptive, the cap ×2
and `LeadingArc` each give the same move list as the defaults. Planted red:
the slice stepover × `trochoid_cap_mult / 1.6` makes it fail with
"GeodesicAdaptive: `trochoid_cap_mult` changed the toolpath". The file was
restored (cmp-checked).

## 3. The docs

- Code comments in `clearing.rs`, `mod.rs`, `path.rs`, `intended_floor.rs`
  and `rough_3d.rs` no longer call the slice the AgentSearch or
  ContourSpiral path. The notes that name a removal step stay.
- `adaptive3d/CLAUDE.md` (40 lines): the slice is `clear_z_level_slice`;
  the strategies are CP and Geodesic Adaptive; the F-038 forecast keeps the
  2D agent planner; the two fields are load-only.
- MCP: the `recommend_clearing_strategy` description named "ContourParallel
  vs ContourSpiral" and a parallel-vs-spiral verdict. It now names
  GeodesicAdaptive and says that the advisor ranks by wall-clock time at
  the machine acceleration. The debug-trace tool no longer names
  AgentSearch. The `exit_reason` filter doc listed values of the removed
  3D search. It now lists the values that the 2D planner writes ("idle",
  "over pass load", "no direction", "no entry"; `adaptive/path.rs`).
- The wire snapshot `mcp_wire_surface.json` changes in that one row only
  (written with `RS_CAM_UPDATE_WIRE_SNAPSHOT=1`). Tool descriptions are not
  in the snapshot.
- `FEATURE_CATALOG.md`: one sentence on the load-only fields and the sentry.

## 4. Boundary parity B2 is strategy-neutral (D67)

Lead's item, added to R6. D67 (P15) closed G-GEOWATERLEAK: the old B2
counted the PRE-CLIP endpoints of the boundary-unaware `waterline_cleanup`,
with an empty `centre_boundary`. Geodesic Adaptive's finer chord tolerance
cuts the same contour arcs into more moves, so the count was per strategy
(CP 135, Geodesic 312).

Change (`tests/adaptive3d_boundary_clear_parity.rs`):

- `measure` plans the fixture a second time with `centre_boundary` set to
  the effective containment, as `compute/execute/rough_3d.rs` sets it. B2
  counts the cutting endpoints of that plan fully outside the containment.
- B2 asserts 0 on both strategies, and as non-vacuity it asserts that the
  pre-clip leak (the plan without `centre_boundary`) is greater than 0. So
  the bar proves that the centre clip removes a leak that exists.
- The two pins `WATERLINE_LEAK_CUT_MOVES` (135) and `_GEODESIC` (312) are
  deleted. The B2 comment and the bar table cite D67. The D4 arm keeps its
  inverted pre-clip pin, and its F-B doc now names the pre-clip leak.
- The main plan, and with it B1, B3, B4, P1 and P2, is unchanged: the
  second plan feeds B2 only. The unused `Measured::strategy` field is
  removed.

Measured: B2 = 0 on both; pre-clip 135 (CP) and 312 (Geodesic), as D67.
Test time 12 s to 13 s for the binary.

Planted red: `path.rs` with the centre clip turned off
(`centre_clip.filter(|_| false)`). Both arms fail: "B2: 135 …" (CP) and
"B2: 312 …" (Geodesic). The file was restored (cmp-checked).

## 5. Gates

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check`, clippy workspace all targets, full features | ok |
| `adaptive3d_emission_byte_parity` | 6/6, no bless |
| `every_op_keeps_its_toolpath_g_opgolden` | 1/1, no bless |
| `folder_maps_name_real_files_f140`, `a_retired_clearing_strategy_is_named_r2`, `agent_search_coverage` | ok |
| core `--lib adaptive3d` | 75 ok, 7 ignored (74 + the new test) |
| core `--lib operation_configs` | 4 ok |
| viz `mcp_authoring_surface`, `command_surface_completeness`, `mcp_wire_surface_pin` | 16, 7, 2 ok |
| `rs_cam_mcp` | 36 ok |
| `adaptive3d_boundary_clear_parity` (after §4) | 4 ok; fmt and full clippy run again, ok |

The slow geodesic binaries (F-038 coalescing, keep-down, never-ahead) did
not run again: R6 changes only comments, docs and one test in the code
that they read.
