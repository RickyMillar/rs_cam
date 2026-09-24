# `adaptive3d/` — 3D adaptive clearing

Constant-engagement clearing on a mesh surface. The entry point is
`adaptive3d::adaptive_3d_toolpath` in `mod.rs`.

## Files

- `mod.rs` — the public facade and `ClearingStrategy3d`.
- `clearing.rs` — the Z-level clearing engine and its region detection. The
  AgentSearch slice runs three stages (CUT-09): `detect_and_order_regions`,
  `clear_one_region` per region over a `LevelEmission`, then
  `coalesce_level_entries`. Put new per-region work in the middle stage.
- `path.rs` — the Z levels, the level loop, the segment linking.
- `search.rs` — the material-remaining query, the clear-path test and two 3D
  path helpers. No direction search and no entry finding (those are in
  `clearing.rs` and `path.rs`); the sibling `adaptive/search.rs` has them.
- `region_map.rs` — the By Area map (overlay evidence); `tests.rs` — tests.

## Invariants

- All three `ClearingStrategy3d` variants are live. `clear_z_level` is not
  dead code. Do not propose a deletion of the AgentSearch arm.
- This engine emits UNTAGGED vertical descents. A guard that reads the intent
  tag alone misses them; see `../dressup/CLAUDE.md`.
- `max_stay_down_distance_mm` is the ONE stay-down distance dial (CUT-05). The
  unset case takes `path.rs::default_max_link_dist`. Add no second name.
- `Adaptive3dParams` has three groups (CUT-04): `geometry`, `depth` (the Z
  plan), `linking`. A new dial joins its group, not the top level.
- The Z plan is ONE Depth/Pass (`path.rs::step_levels` from
  `level_anchor_z`, plus Detect Flat shelves). Every level drapes. The step
  ladder was removed 2026-09-24 (operator ruling); do not add it back.

## Sentries

- `cargo test -p rs_cam_core -q --test adaptive3d_boundary_clear_parity`
- `cargo test -p rs_cam_core -q --test adaptive3d_keep_down_link_f038b`
- `cargo test -p rs_cam_core -q --test adaptive3d_entry_coalescing_f038`
- `cargo test -p rs_cam_core -q --test agent_search_coverage`
- `cargo test -p rs_cam_core -q --test adaptive3d_subtool_channel_gouge`
