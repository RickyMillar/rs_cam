# Phase 3: By Area cuts pocket by pocket from the pocket tree

Status: PLAN. Written 2026-09-25. Read `RESULTS.md` (same folder) first.
Start after the entry-descent work merges. That work changes `clearing.rs::plan_entry`, `path.rs` entry handling and `dressup/entry_descent.rs`. This plan does not use any of that entry code.

## 0. Summary

- The planner builds the pocket tree once per operation. The tree reads the planner's own tool-CL grid.
- A leaf pocket is a **valley** and is its own job. The rest of the material is one **rest** job: the internal bands, the root, and the cells that have no label.
- A per-cell ownership mask confines each job. The mask replaces the region bbox at one shared point in the code.
- Default order: the rest job first, then the valleys depth first, nearest next. Section 4 gives the reason. Phase 3 also measures the reverse order (rest last) and keeps the faster one.
- Two new dials: `pocket_min_depth_mm` = 2.0 and `pocket_min_area_mm2` = 400.
- Two existing defects must be fixed: the level filter and the material gate (section 3.4).

## 1. The grid and when to build it

1. The tree reads the **tool-CL drop-cutter grid**, `surface_hm` in `adaptive3d/path.rs::adaptive_3d_segments`. That grid is on `material_stock.z_grid`, with the same origin, cell and size. So `PocketTree.labels[i]` is planner cell `i`, and no resampling is needed.
2. Build the `FlowField` as follows:
   - `z[i] = max(surface_hm.z_or_bbox_floor_values()[i], z_floor)`. A pinned `bottom_z` gives a flat floor.
   - `nodata[i] = !covered[i] || outside the boundary polygon`. Use the same test as the boundary clear.
3. `top_z = level_top`, the output of `level_anchor_z`. Do not use `stock_top_z`. After the Face this value is 12.0. RESULTS.md used the same value.
4. Build the tree once. Do it after the border clear, the boundary clear and `level_anchor_z`, and before the first level. The CL surface does not change during the operation. Cost on rivmap100: 25 ms.
5. Do not build the tree from the remaining-stock grid. Its minima are piles of stock, not valleys. The topology is inverted.
6. Rest roughs (after a Face or an earlier rough): the tree stays geometry only. Cells with no remaining material do NOT become walls. So the pocket count does not change with prior stock. A job with no material skips its levels through the gate (3.4). Recorded risk: a thin rest rough can cut a valley job that has little material left in it.

## 2. What a region becomes

1. **Valley job**: each pocket with no children, also when it is a root. The pocket plate in `tests.rs` has two roots with no children, so it has two valleys and no rest.
2. **Rest job**: every planner cell that no valley owns at that level. This includes internal bands, the root, cells at or above `top_z`, and masked cells with no label. The masked cells include the F-027 stock beside the model. If the rest were defined as "root + band labels", those cells would never be cut.
3. Justification:
   - Finding 1: only leaves are valleys, and bands are thin. A band cut as its own job splits the rough into small pieces.
   - Finding 2: about half the material is root. By Area gains only on the leaves.
   - So the plan cuts the leaves pocket by pocket and cuts the rest as one Global-order job.
4. Invariant (sentry S3): at every level, the jobs partition the grid. Every cell gets the same Z-level sequence as in Global, top down. Only the order of the (job, level) pairs changes. So the axial DOC per cell is at most Depth/Pass, as in Global.

## 3. Confinement to the cells, not the bbox

### 3.1 The code points that read the region box today

- `adaptive3d/clearing.rs::build_material_bool_grid` (line 287). Filter: `row < r.row_min || row > r.row_max || col < ...`. Four callers:
  - line 785: the contour-parallel rings
  - line 961: the contour-parallel cleanup raster
  - line 1153: `clear_z_level_adaptive`
  - line 1594: `detect_and_order_regions` (AgentSearch and ContourSpiral)
- `adaptive3d/search.rs::material_remaining_in_region` (line 139). It loops over the row/col box. Callers: `clearing.rs` lines 770, 1143, 2554.
- `path.rs` `LevelSlot.region: Option<(usize, &MaterialRegion)>` and the ByArea branch (lines 895-1017).
- `clearing.rs` line 2564 (AgentSearch span label) reads `row_min..`.

### 3.2 Method: a per-cell mask, not a polygon clip

1. Replace `region: Option<&MaterialRegion>` with `Option<&AreaMask>`. `AreaMask` holds `owned: Vec<bool>` (one per planner cell), a row/col bound for the loops, and the job id.
2. Rebuild the mask per (job, level) from the tree labels. A rest-first valley owns `label == L && z < saddle_z(L)`. The rest owns the complement.
3. `build_material_bool_grid` skips cells the mask does not own. All four strategies get the mask from this one change.
4. Reject a polygon clip of the rings. It needs a contour and an offset, and it misses the cleanup raster. The mask is exact and costs nothing.

### 3.3 Behaviour at a pocket rim

1. The tool centre stays on owned cells. The tool radius can go past the rim into the cells of another job: up to `r - stepover/2` on a ring, and up to `r` on the cleanup raster.
2. This does not gouge. The grid is in tool-CL space, and every ring point sits at or above the CL of its own cell.
3. The planner stamps every cut into `material_stock`. The next job's bool grid reads that material as air, so no job cuts it again. This is how Global works today.
4. Engagement at the rim: the first job along a shared rim sees a wall of the other job's uncut material. The wall is at most one Depth/Pass high. Measure this (section 7) and do not design around it.

### 3.4 Two defects to fix in the same work package

1. **Material gate.** `material_remaining_in_region` counts only cells with `surf + stl <= z + 0.01`. Two failures follow:
   - A rest job has only high-ground cells above the level, so the gate skips it. On rivmap100 this skips every rest level, and the high ground is never cut.
   - The last draping level of a valley is below its floor, so the gate skips it too.
   - Fix: gate on the count of the masked bool grid (material above `max(CL + stl, z)`), which is the same test the rings use. Delete `material_remaining_in_region`.
2. **Level filter.** `path.rs` line 957 keeps `z >= surface_z_min + stl - 0.01`. This drops the first level below the floor of the region, so a floor between two levels never drapes.
   - Fix: a job's levels are the global `z_levels` below the job's upper Z. They continue down to and include the first level at or below `min_z + stl`.
   - The upper Z is the saddle for a rest-first valley, and `level_top` for a rest-last valley or the rest.

## 4. Cutting order

1. **Depth first per valley.** Cut all the levels of one valley, then go to the next valley.
2. **Sibling rule: nearest next.** After each job, pick the unvisited valley whose anchor is nearest to `last_pos`. Use `clearing.rs::nearest_neighbor_order` (line 1474) one step at a time. On a tie, take the lower `min_z`, then the lower id. Deepest first has no machining gain. Nearest next reduces rapid time, which the operator measures.
3. **Rest position: FIRST (default).** Reasons:
   - The rest cuts the high ground down to its drape. Each valley then starts from an open rim. The contour-parallel entry picks the ring point with the lowest stock top, and that point is in the cut rim strip. So each valley entry is shallow.
   - Rest last makes one full-depth entry per valley into virgin stock. On rivmap100 that is 3 deep entries instead of 1.
   - The rim wall is the same in both orders (at most one Depth/Pass, 3.3).
4. **Rest last (the measured alternative).** A valley owns its cells at all levels. The rest runs with no mask, which is Global on the remaining stock. The rest also runs the Global per-level waterline cleanup. `PocketTree::cut_order()` gives this order today (children first, deepest first).
5. Implement both orders behind a private `enum AreaOrder` with no config surface. WP5 measures both. WP6 deletes the losing order.
6. **Waterline cleanup.**
   - Rest first: run it once, at the bottom Z, after all jobs. Today's F9 limit stays, because `waterline_cleanup` has no mask.
   - Rest last: run it per level during the rest job, as Global does.
7. `PocketTree::cut_order()` is not the planner order. Replace it with `valleys()` (the leaf ids). The probe then prints the planner order from `AreaRegionMap`. Do not keep a shim.

## 5. The dials

| Config key | Unit | Default | Meaning (help text) |
|---|---|---|---|
| `pocket_min_depth_mm` | mm | 2.0 | A valley less deep than this below its rim joins its neighbour. |
| `pocket_min_area_mm2` | mm² | 400 | A valley smaller than this at its rim joins its neighbour. |

1. Do not scale by tool diameter. The CL grid already removes valleys that the tool cannot enter. The dials set only the job size, and the operator reads them in mm. A scaled value would show a derived number on the three surfaces.
2. Core: `adaptive3d::RegionOrdering::ByArea { pockets: MergeTreeParams }`.
   - The dials have no meaning under Global, and this keeps them in the linking group (CUT-04).
   - Drop `Eq` from `RegionOrdering` and keep `PartialEq`. No `==` use exists.
   - This form changes only the ByArea literals. A field on `Adaptive3dLinking` changes about 22 test files, because `Adaptive3dLinking` has no `Default`.
3. Files for the dials:
   - `crates/rs_cam_core/src/compute/operation_configs.rs`: two fields, serde default functions, `Default`.
   - `crates/rs_cam_core/src/compute/catalog/registry.rs`: `ADAPTIVE3D_PARAMS`, `required(..).with_help(..)`.
   - `crates/rs_cam_core/src/compute/execute/finish_3d.rs`: map to `ByArea { pockets }`.
   - `crates/rs_cam_core/src/adaptive3d/mod.rs`: the enum and its doc (the doc still says "flood fill").
   - `crates/rs_cam_cli/src/job.rs`: two `OperationDef` fields and `job_params_for` pushes.
   - `crates/rs_cam_cli/src/sweep.rs`: examine whether its key list needs the two names.
   - `crates/rs_cam_viz/src/ui/properties/operations/surface_3d.rs`: two drag rows under "Ordering". Show them only when By Area is set. Labels: "Pocket Min Depth", "Pocket Min Area". The help key is the param name.
4. MCP `set_toolpath_param` / `get_operation_schema` and CLI `--set` read the registry. They need no code change.

## 6. Overlay and MCP

1. Replace `AreaRegionMap::from_detection` with `from_plan(stock, &tree, &jobs_in_cut_order)`. Build it after the jobs run, because nearest next fixes the order only at run time.
   - Labels: valley cells get the valley's order. All other material cells get the rest order.
   - Rewrite `nearest_cell` to read the label grid.
2. Add these fields to `AreaRegion`: `kind` ("rest" or "valley"), `saddle_z: Option<f64>`, `depth_mm`, `area_mm2`. `bbox_xy` stays, as evidence only.
3. Delete the F4 doc lines in `region_map.rs`.
4. Replace the `note` string in `crates/rs_cam_viz/src/app/mcp/diagnostics.rs` (line 759). New note: jobs are cell-confined, the rest runs first or last, and the dials that were used.
5. `RegionStart` and `RegionZLevel` markers stay. `region_index` is the order.
6. Delete `MaterialRegion`, `detect_material_regions`, `detect_material_regions_labeled` and their unit tests (`adaptive3d/tests.rs` lines 640-815). Update the file list in `adaptive3d/CLAUDE.md` and the "Test door" paragraph in `merge_tree.rs`.

## 7. Measurement plan (WP5)

Project: `F=rivmap100_live_0925.toml`. Note that `arm.sh` defaults to the ladder demo. Rough = toolpath 1, finish (Scallop) = toolpath 2, `--resolution 0.5`, dpp 8. Build release on one commit, after the entry merge.

| Arm | `--set` |
|---|---|
| G | `1.region_ordering=global` |
| A | `1.region_ordering=by_area` (defaults, rest first) |
| A-last | same as A, with `AreaOrder::RestLast` (a private const in the WP5 build) |
| A-h1 | by_area, `1.pocket_min_depth_mm=1 1.pocket_min_area_mm2=100` (6 valleys) |
| A-h4 | by_area, `1.pocket_min_depth_mm=4` (2 valleys) |

1. Record for each arm and toolpath:
   - `total_runtime_s`, `cutting_runtime_s`
   - `runtime_by_intent.entry_s`, `runtime_by_intent.rapid_s`
   - `rapid_distance_mm`, `cutting_distance_mm`, the move counts
   - `average_engagement`, `whole_cycle_engagement`
   - `total_removed_volume_est_mm3`, the finding areas
   - planner ms (from the log), and the pocket count from `area_regions`.
2. Finish-equivalence check: score the rough and then the Scallop Finish in the same walk.
   - The finish removed volume and the finish `total_runtime_s` must be within 2 % of arm G.
   - The rough removed volume must be at least 99.5 % of arm G.
3. Phase 3 fails if at least one of these is true for arm A (or for the winning order):
   - F1: `total_runtime_s` is equal to or more than arm G.
   - F2: `whole_cycle_engagement` is lower than arm G.
   - F3: the finish-equivalence check fails.
   - F4: a new deflection "Exceeds" finding, or an axial DOC above dpp + 0.5 mm that arm G does not have.
   - F5: the pocket count is 10 or more at the defaults, or 0 valleys.
   - F6: GUI, MCP and CLI numbers differ.
   - F7: planner time is more than arm G + 20 %.
4. Known confound: Global runs the waterline cleanup per level, and rest-first By Area runs it once. Write this in RESULTS.md next to the numbers.
5. If F1 or F2 occurs, stop and report to the operator. Do not tune the dials to pass.

## 8. Sentries

**New**

- S1 `adaptive3d_by_area_flat_plane_is_global` (core tests). On a flat plane the tree has one root and no valley. By Area emits moves byte-identical to Global.
- S2 `adaptive3d_by_area_cells_confine_jobs`. A two-basin terrain has a ridge below the top, so it has a real rest. Every Cut point of a valley job has its centre cell labelled with that valley.
- S3 `adaptive3d_by_area_matches_global_stock`. The final `material_stock` of By Area equals Global's within 0.05 mm per cell. This also covers the level-filter and gate defects (3.4).
- S4 `adaptive3d_by_area_order`. The `RegionStart` sequence is rest then valleys in nearest-next order. The levels of each valley are contiguous.
- S5 In `merge_tree.rs` unit tests: `valleys()` returns the leaves, including a root with no children.
- S6 Extend the rough_score GUI/CLI parity test with `pocket_min_depth_mm`.

**At risk (update in the same work package)**

- `crates/rs_cam_core/src/adaptive3d/tests.rs`: `test_adaptive_3d_by_area_flat`, `test_adaptive_3d_by_area_hemisphere`, `by_area_exports_two_pocket_region_map` (its "larger first" assertion changes to nearest next), the `detect_material_regions` tests, `test_material_remaining_in_region`.
- `crates/rs_cam_core/tests/adaptive3d_emission_byte_parity.rs`: the contour_spiral case is ByArea. Re-bless with `ADAPTIVE3D_BYTE_PARITY_BLESS=1` and name the reason in the commit.
- `crates/rs_cam_core/tests/adaptive3d_entry_stock_aware.rs` (ByArea).
- `crates/rs_cam_core/src/compute/execute/tests.rs` (line 342) and `crates/rs_cam_viz/src/compute/worker/tests.rs` (line 111).
- `crates/rs_cam_core/src/compute/catalog/tests.rs` `param_defs_cover_every_config_field`; `crates/rs_cam_core/tests/set_param_refuses_absent_field_n5.rs`; `crates/rs_cam_core/tests/schema_enum_values_g_schemaenum.rs`.
- `crates/rs_cam_viz/tests/the_help_key_is_the_param_name_ui04.rs`, `crates/rs_cam_viz/tests/area_regions_overlay_by_area.rs`.
- `crates/rs_cam_viz/src/controller/tests/stale_cards_g_stalecards.rs`: its fixture `face_and_rest_rough.toml` is by_area.
- `crates/rs_cam_cli/src/job.rs` `an_unknown_order_by_is_refused`; `crates/rs_cam_cli/src/rough_score.rs` parity test.
- `crates/rs_cam_core/tests/pocket_merge_tree_census_by_area.rs` (reads `cut_order` and the old map).
- Planner-to-sim parity: `crates/rs_cam_core/tests/adaptive3d_planner_sim_dexel_parity.rs`.
- The adaptive3d/CLAUDE.md sentries: `adaptive3d_boundary_clear_parity`, `adaptive3d_keep_down_link_f038b`, `adaptive3d_entry_coalescing_f038`, `agent_search_coverage`, `adaptive3d_subtool_channel_gouge`.

## 9. Work packages (in order, one agent each)

**WP1: mask seam.** Replace `MaterialRegion` with `AreaMask` at the seams in 3.1. The old BFS detector emits a mask from its labels, so this step alone fixes finding 3. Also fix the gate and the level filter (3.4).
- Files: `adaptive3d/clearing.rs`, `adaptive3d/search.rs`, `adaptive3d/path.rs`, `adaptive3d/tests.rs`.
- Risk: the gate change can alter Global output. Global keeps `material_remaining_at_level`, so run the emission byte-parity test to prove Global is byte-identical. The ByArea parity case changes.

**WP2: the tree replaces the detector.** Add `adaptive3d/area_plan.rs`: FlowField build (section 1), jobs, ownership, `AreaOrder`, nearest next. Rewrite the ByArea branch of `path.rs`. Add `from_plan` in `region_map.rs`. Replace `cut_order` with `valleys()` in `merge_tree.rs`. Delete the old detector and `MaterialRegion`. Update the probe, the diagnostics note and the CLAUDE.md files. Add S1-S5.
- Default dials are a `const` in core until WP3.
- Risk: masked cells, F-027 cells and the rest complement. S3 guards them.

**WP3: dials.** Change the section 5 file list. Add S6. Update the at-risk registry sentries.
- Risk: the GUI row, the help key and the schema must agree. ui04 and the catalog tests guard this.

**WP4: sentry sweep.** Run every at-risk sentry in section 8. Re-bless byte parity with a reason. Update `area_regions_overlay_by_area.rs` to the new kinds. Run `/verify` for the core suite after the operator says go.

**WP5: measurement.** Run the section 7 arms. Append a "Phase 3 measurement" section to `RESULTS.md` with the table, the confound and a verdict against F1-F7. Get the operator's choice of dials.

**WP6: close-out.** Delete the losing `AreaOrder` arm and its const. Update `adaptive3d/CLAUDE.md` invariants: "By Area = pocket tree, cell-confined, rest first|last". Record the verdict in the planning index.

Risks across all work packages:
- (a) The rim wall engagement (3.3) can raise the tool load. F4 catches this.
- (b) Nearest next makes the order depend on `last_pos`, so the overlay order is fixed only after the run.
- (c) The entry merge can move the entry-time numbers. Score arm G again on the same commit and do not reuse old numbers.

### Critical Files for Implementation
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/adaptive3d/path.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/adaptive3d/clearing.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/adaptive3d/region_map.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/surface/merge_tree.rs
- /home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/compute/catalog/registry.rs
