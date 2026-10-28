# C4b, "Contour each level" in By Area order (the F9 fix)

Date: 2026-10-28. Built in an isolated worktree at `0684b89f8` (own lane
script, own target directory), then moved onto `d131559dd` (a docs-only
commit; one catalog conflict resolved: the 3D Adaptive row is mine, the
Waterline row is the branch's). The implementer did not commit.

Patch: `c4b_by_area_walls_tree_vs_d131559dd.patch`, md5
`0df2281f8abda4a9689ce903e17a5a26`. 6 files (5 changed, 1 new).
`git apply --check` passes on the shared HEAD `d131559dd`.
Pre-registration: `c4b_PREREG.md` (md5 `42fcb51dd9a4870df8c5a94b5dbc331e`),
approved.

## 1. Change

- `clearing.rs::waterline_cleanup` gains `keep: Option<CleanupKeep<'_>>`
  (`type CleanupKeep<'a> = &'a dyn Fn(&TriDexelStock, P3) -> bool`).
  `None` (Global, the By Area bottom run, the option off) pushes the same
  segments in the same order as before. `Some` splits each traced contour
  into kept runs (`kept_runs`; a fully kept contour is the closed contour
  as before; a run across the closing point is joined), each one Rapid
  and one Cut through `push_segment_with_stamp`.
- `path.rs`, By Area loop: with `level_contour` on, after each
  `clear_planned_level` of a region, a WaterlineCleanup marker and a
  masked `waterline_cleanup` at that level. The bottom run stays as
  before. The F9 comment now says the region-level runs are masked.
- `level_contour.rs::keeps_to_region`: a point is kept when no planner
  cell within R + cs/2 holds material above max(level, its own floor
  (surface + stock to leave)) + `MATERIAL_MARGIN_MM` (the anchor's 0.01,
  now `pub(super)`) outside the region's `AreaMask`.

**One clarification of the pre-registration's wording:** "holds stock above
z" must read the cell's own floor too. The planner stock holds the part,
so a wall cell (the part, surface at the top) read as "stock above z" and
the mask rejected every point (first run: 5 cleanup spans, 0 traced). With
the floor, it is the test C4's `planner_top_in_reach` uses.

## 2. Sentries

| Sentry | Result |
|---|---|
| `by_area_walls_cut_each_region_level_c4b` (two pockets; pocket B has a shallow floor at 7 and a hole to 1) | off: 1 cleanup, 153 moves, max axial 8.334; on: 7 cleanups, 269 moves, max axial 3.000 (DPP 3, sim cell 0.5) |
| `by_area_contour_each_level_is_deterministic_c4b` | ok |
| `--lib the_wall_mask_keeps_off_unroughed_regions_c4b` | other region standing above the level: rejected; own region: kept; other region cut to the level: kept |
| planted red: `keep` accepts every point | max axial 3.625 > 3.5: fails, as it must (286 moves) |

- The planted red fails by less than my prediction (6 mm): pocket A's
  level-7 run also steps pocket B's walls down first. My first fixture
  (two plain pockets) could not show F9 at all, for the same reason, so I
  added the hole, whose wall contour first appears at Z 4.
- With the option on, the fixture's bottom-only over-cut (8.334 with the
  option off) also goes to 3.000.

## 3. Gates

| Gate | Result |
|---|---|
| byte parity (incl. `contour_parallel_by_area`), op golden (incl. `by_area_helix_cp`), s7_4 | 6/6, 1/1, 1/1: option off unchanged |
| `adaptive3d_by_area_cells_confine_jobs`, `adaptive3d_by_area_matches_global_stock` | 2/2, 1/1 |
| C4 sentries (`a_rest_op_holds_one_depth_at_the_band_c4`, `contour_each_level_is_off_by_default_c4`) | 5/5, 2/2 |
| `rs_cam_core --lib` | 2729 ok |
| `rs_cam_cli` (every test) | all ok |
| clippy `rs_cam_core --all-targets`, full features; `cargo fmt --check` | clean |

Not run: the core full gate.

## 4. Re-bless list

None. Docs: `FEATURE_CATALOG.md` (the C4 sentence now says "In Global
order a fresh rough is unchanged", plus the By Area behaviour and the
sentry), `adaptive3d/CLAUDE.md` (file map and sentry; 40 lines).
