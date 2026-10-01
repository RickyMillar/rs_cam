# `adaptive3d/` — 3D adaptive clearing

Constant-engagement clearing on a mesh; entry `adaptive_3d_toolpath` (`mod.rs`).

## Files

- `clearing.rs` — Z-level engine, regions, `AreaMask` (By Area: a job's
  cells). AgentSearch slice (CUT-09): `detect_and_order_regions`,
  `clear_one_region` into a `LevelSink` (new per-region work goes here),
  then `coalesce_level_entries` (a check).
- `path.rs` — Z levels, level loop, emitter. `search.rs` — clear-path test.
  `centre_clip.rs` — the session boundary clip, applied before each stamp.
- `mod.rs` — facade, `ClearingStrategy3d`; `region_map.rs` — By Area map.

## Invariants

- All `ClearingStrategy3d` variants are live (keep the AgentSearch arm). It
  emits UNTAGGED vertical descents; see `../dressup/CLAUDE.md`.
- `max_stay_down_distance_mm` is the ONE stay-down dial (CUT-05, unset 8 x D).
  Entries read floor and proof from the planner stock (`plan_entry`); no
  link feeds down into stock.
- `Adaptive3dParams` groups (CUT-04): `geometry`, `depth`, `linking`; a new dial joins its group.
- The planner stamps the path that ships: it applies the segment merge
  before the stamp and the dressups do not merge again (G-PLANSIMGAP). It
  stamps an entry only when a cut commits it; a replaced entry is never
  stamped, and cuts and links stamp from `PlannerCursor::tool_pos`
  (G-PHANTOMSTAMP). Never delete a segment after it is stamped. Every
  segment passes `centre_clip` before its push (G-BOUNDARYPHANTOM).
- The Z plan is ONE Depth/Pass (`path.rs::step_levels`, plus Detect Flat
  shelves); every level drapes. No step ladder (operator, 2026-09-24).

## Sentries: `cargo test -p rs_cam_core -q --test <name>`

`adaptive3d_boundary_clear_parity`, `adaptive3d_keep_down_link_f038b`,
`adaptive3d_entry_stock_aware`, `adaptive3d_entry_coalescing_f038`,
`agent_search_coverage`, `adaptive3d_subtool_channel_gouge`,
`adaptive3d_global_gate_drapes_below_floor`, `adaptive3d_plan_matches_merged_path_g_plansimgap`,
`adaptive3d_planner_never_ahead_of_emitted_path`,
`adaptive3d_boundary_clip_leaves_no_phantom_stock_g_boundaryphantom`; By Area:
`adaptive3d_by_area_cells_confine_jobs`, `..._by_area_matches_global_stock`.
