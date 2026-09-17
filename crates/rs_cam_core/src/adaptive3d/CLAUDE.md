# `adaptive3d/` — 3D adaptive clearing

Constant-engagement clearing on a mesh surface. The entry point is
`adaptive3d::adaptive_3d_toolpath` in `mod.rs`.

## Files

- `mod.rs` — the public facade and `ClearingStrategy3d`.
- `clearing.rs` — the Z-level clearing engine and its region detection.
- `path.rs` — the loop over Z levels and the segment linking.
- `search.rs` — direction search, engagement, entry-point finding.
- `tests.rs` — the unit tests.

## Invariants

- All three `ClearingStrategy3d` variants are live. `clear_z_level` is not
  dead code. Do not propose a deletion of the AgentSearch arm.
- This engine emits UNTAGGED vertical descents. A guard that reads the intent
  tag alone misses them; see `../dressup/CLAUDE.md`.

## Sentries

- `cargo test -p rs_cam_core -q --test adaptive3d_boundary_clear_parity`
- `cargo test -p rs_cam_core -q --test adaptive3d_keep_down_link_f038b`
- `cargo test -p rs_cam_core -q --test adaptive3d_entry_coalescing_f038`
- `cargo test -p rs_cam_core -q --test agent_search_coverage`
- `cargo test -p rs_cam_core -q --test adaptive3d_subtool_channel_gouge`
