# `adaptive3d/` — 3D adaptive clearing

Constant-engagement clearing on a mesh; entry `adaptive_3d_toolpath` (`mod.rs`).

## Files

- `clearing.rs` — the Z-level clearing engine, its region detection and
  `AreaMask` (By Area confines a job to its cells, not its box). The
  AgentSearch slice runs three stages (CUT-09): `detect_and_order_regions`,
  `clear_one_region` per region over a `LevelEmission`, then
  `coalesce_level_entries`. Put new per-region work in the middle stage.
- `path.rs` — the Z levels, level loop, segment linking. `search.rs` — the
  floor-cell diagnostic, clear-path test, two path helpers (no entry finding).
- `mod.rs` — the facade and `ClearingStrategy3d`; `region_map.rs` — the By
  Area map (overlay evidence); `tests.rs` — tests.

## Invariants

- All three `ClearingStrategy3d` variants are live. `clear_z_level` is not
  dead code. Do not propose a deletion of the AgentSearch arm.
- This engine emits UNTAGGED vertical descents; see `../dressup/CLAUDE.md`.
- `max_stay_down_distance_mm` is the ONE stay-down dial (CUT-05, unset 8 x D).
  Add no second name. Every entry reads its floor and keep-down proof from the
  planner stock (`clearing.rs::plan_entry`). No link feeds down into stock.
- `Adaptive3dParams` has three groups (CUT-04): `geometry`, `depth` (the Z
  plan), `linking`. A new dial joins its group, not the top level.
- The planner stamps the path that ships: it applies the segment merge
  before the stamp and the dressups do not merge again (G-PLANSIMGAP).
- The Z plan is ONE Depth/Pass (`path.rs::step_levels` from
  `level_anchor_z`, plus Detect Flat shelves). Every level drapes. The step
  ladder was removed 2026-09-24 (operator ruling); do not add it back.

## Sentries: `cargo test -p rs_cam_core -q --test <name>`

`adaptive3d_boundary_clear_parity`, `adaptive3d_keep_down_link_f038b`,
`adaptive3d_entry_stock_aware`, `adaptive3d_entry_coalescing_f038`,
`agent_search_coverage`, `adaptive3d_subtool_channel_gouge`,
`adaptive3d_global_gate_drapes_below_floor`,
`adaptive3d_plan_matches_merged_path_g_plansimgap`; By Area:
`adaptive3d_by_area_cells_confine_jobs`, `adaptive3d_by_area_matches_global_stock`.
