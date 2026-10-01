# `adaptive3d/` — 3D adaptive clearing

Constant-engagement clearing on a mesh; entry `adaptive_3d_toolpath` (`mod.rs`).

## Files

- `clearing.rs` — Z-level engine, `AreaMask` (a job's cells). AgentSearch
  slice (CUT-09): `detect_and_order_regions`, `clear_one_region` into a
  `LevelSink` (new per-region work goes here), `coalesce_level_entries`.
- `area_plan.rs` — By Area: pocket tree (`surface::merge_tree`) on the
  planner's CL grid; valley jobs + one rest job, per-level ownership,
  private `AreaOrder`, nearest next. Dials: 1/3 x D, 14.147 tool discs.
- `path.rs` — Z levels, level loop, emitter. `search.rs` — clear-path test.
  `centre_clip.rs` — boundary clip before each stamp. `mod.rs` — facade;
  `region_map.rs` — By Area map (`from_plan`).

## Invariants

- All `ClearingStrategy3d` variants are live (keep AgentSearch); UNTAGGED
  vertical descents, see `../dressup/CLAUDE.md`.
- `max_stay_down_distance_mm` is the ONE stay-down dial (CUT-05, unset 8 x D).
  Entries read the planner stock (`plan_entry`); no link feeds into stock.
- `Adaptive3dParams` groups (CUT-04): `geometry`, `depth`, `linking`.
- The planner stamps the path that ships: segment merge before the stamp
  (G-PLANSIMGAP); an entry only when a cut commits it, cuts and links from
  `PlannerCursor::tool_pos` (G-PHANTOMSTAMP); never delete a stamped
  segment; `centre_clip` before every push (G-BOUNDARYPHANTOM).
- ONE Depth/Pass Z plan (`step_levels`, shelves); levels drape; no ladder (operator, 2026-09-24).
- By Area jobs partition the grid per level (Global's levels per cell; only
  the order moves). One pocket = no valley = Global's moves.

## Sentries: `cargo test -p rs_cam_core -q --test <name>`

`adaptive3d_boundary_clear_parity`, `adaptive3d_keep_down_link_f038b`,
`adaptive3d_entry_stock_aware`, `adaptive3d_entry_coalescing_f038`,
`agent_search_coverage`, `adaptive3d_subtool_channel_gouge`,
`adaptive3d_global_gate_drapes_below_floor`, `adaptive3d_plan_matches_merged_path_g_plansimgap`,
`adaptive3d_planner_never_ahead_of_emitted_path`, `..._g_boundaryphantom`;
By Area `adaptive3d_by_area_` + `flat_plane_is_global` / `cells_confine_jobs`
/ `matches_global_stock` / `order`.
