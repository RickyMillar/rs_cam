# `adaptive3d/` — 3D adaptive clearing

Constant-engagement clearing on a mesh surface. The entry point is
`adaptive3d::adaptive_3d_toolpath` in `mod.rs`.

## Files

- `mod.rs` — the public facade and `ClearingStrategy3d`.
- `clearing.rs` — the Z-level clearing engine and its region detection. The
  AgentSearch slice runs three stages (CUT-09): `detect_and_order_regions`,
  `clear_one_region` per region over a `LevelEmission`, then
  `coalesce_level_entries`. Put new per-region work in the middle stage.
- `path.rs` — the loop over Z levels and the segment linking.
- `search.rs` — the material-remaining query, the clear-path test and two
  3D path helpers. It holds no direction search and no entry-point finding;
  those live in `clearing.rs` and `path.rs`. The sibling `adaptive/search.rs`
  does hold them, so the two file names do different jobs.
- `tests.rs` — the unit tests.

## Invariants

- All three `ClearingStrategy3d` variants are live. `clear_z_level` is not
  dead code. Do not propose a deletion of the AgentSearch arm.
- This engine emits UNTAGGED vertical descents. A guard that reads the intent
  tag alone misses them; see `../dressup/CLAUDE.md`.
- `max_stay_down_distance_mm` is the ONE stay-down distance dial (CUT-05). The
  unset case takes `path.rs::default_max_link_dist`. Add no second name.
- `Adaptive3dParams` carries three groups (CUT-04): `geometry` (the cutter and
  the XY frame), `depth` (the Z plan) and `linking` (order, entry floor,
  stay-down). A new dial joins the group it belongs to, not the top level.
  The shallow tier is ONE `Option<ShallowTier>`; do not split it again.

## Sentries

- `cargo test -p rs_cam_core -q --test adaptive3d_boundary_clear_parity`
- `cargo test -p rs_cam_core -q --test adaptive3d_keep_down_link_f038b`
- `cargo test -p rs_cam_core -q --test adaptive3d_entry_coalescing_f038`
- `cargo test -p rs_cam_core -q --test agent_search_coverage`
- `cargo test -p rs_cam_core -q --test adaptive3d_subtool_channel_gouge`
