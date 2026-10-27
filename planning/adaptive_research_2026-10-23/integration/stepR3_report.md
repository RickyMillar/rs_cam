# Removal step R3: the 3D Adaptive planner leaves

Date: 2026-10-28. Branch `geodesic-integration`, base `c140de160` (R2).
The implementer did not commit. Design: `removal_design.md` §1.2, §5.

Patch: `stepR3_tree_vs_c140de160.patch`, md5
`cee2fb5c2db73213aab56af2a5db2ce4`. 13 file entries (12 tracked, +31/-932,
and one new golden). No planning file. `git apply --check -R` passes.

`removal_design.md` §6 has a new item 13 (planning file, not in the patch):
the D2 split has no live strategy test after R2; a fixture whose grown
inset splits is a later sentry, not built now.

## 1. What changed

| Place | Change |
|---|---|
| `adaptive3d/clearing.rs` | `clear_z_level_adaptive` (175 lines) and its import of the two grid helpers go. `ClearZLevelContext::target_frac` goes: its one reader was that planner (clippy `dead_code`) |
| `adaptive3d/path.rs` | the `Adaptive` dispatch arm, the import, and the `target_frac` computation and its `target_engagement_fraction` import |
| `adaptive3d/mod.rs` | `ClearingStrategy3d::Adaptive`, its docs, its arms in `effective_step_split` and `effective_keep_down_route` |
| `geometry/grid_field.rs` | `edt_curvature_field` and `smooth_grid` (102 lines; no other caller, no test); the module doc says they left with their caller |
| `finish/scallop_isofield.rs` | the module doc that cited `clear_z_level_adaptive` says it was removed in R3 |
| `adaptive3d/tests.rs` | the strategy list of the capped-split test and one comment |
| `tests/adaptive3d_global_gate_drapes_below_floor.rs`, `adaptive3d_by_area_cells_confine_jobs.rs`, `adaptive3d_by_area_matches_global_stock.rs` | `Adaptive` leaves the strategy lists |
| `tests/param_sweep.rs` (ignored harness) | the `adaptive` sweep value goes |
| `tests/adaptive3d_emission_byte_parity.rs` | §2 |

## 2. The byte-parity golden

The case `adaptive` (fixture `adaptive.txt`, 593 lines, deleted) also
carried the only parity coverage of the dials `detect_flat_areas` and
`z_blend`, which Contour Parallel still reads. A new case
`contour_parallel_flat_blend` (Contour Parallel with both dials on) takes
them, as a new pin blessed alone (`ADAPTIVE3D_BYTE_PARITY_BLESS=1`, that
test only): 1198 moves, against 433 for plain `contour_parallel`, so the
dials act. Every other fixture is unchanged (`git status`: only
`adaptive.txt` deleted and the new file added). The bless log in the test
doc records it.

## 3. Gates

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check`, clippy workspace all targets, full features | ok |
| `adaptive3d_emission_byte_parity` | 6/6 (`contour_parallel`, `geodesic_adaptive`, `agent_search`, `contour_spiral` unchanged; the new case) |
| `every_op_keeps_its_toolpath_g_opgolden` | 1/1, no bless (its 3D Adaptive row left in R2) |
| the adaptive3d sentries (global gate, By Area ×2, boundary clear, keep-down link, entry stock, coalescing, AgentSearch coverage, plan-sim gap, never-ahead, pieces), the s7_4 pins, the cap census, `folder_maps_name_real_files_f140` | 16 binaries ok |
| core `--lib` filters `adaptive3d`, `geometry::grid_field`, `finish::scallop_isofield` | 80 ok, 7 ignored |

## 4. Next

R4: AgentSearch and ContourSpiral leave the planner enum and the dispatch;
their tests are retargeted or deleted, with their goldens.
