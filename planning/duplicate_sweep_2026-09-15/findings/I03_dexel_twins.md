# I03 — dexel twins (dexel_mesh.rs vs dexel_mesh_mc.rs)

Verdict: TRUE_DUP — but the `dexel_mesh.rs` copies are **dead legacy code**;
the merge is a deletion, not an extraction. (Note: "mc" = marching cubes
per commit 81cd86e2 / §6.J, not "midpoint-circle" as the plan item guesses.)

## Evidence
- Files: `crates/rs_cam_core/src/dexel_mesh.rs` (1451 L, facade + preview +
  side-grid + drill + legacy heightmap path) vs
  `crates/rs_cam_core/src/dexel_mesh_mc.rs` (775 L, live MC implementation).
- Pair 1 (0.9628): `find_matching_gap` — dexel_mesh.rs L635-649 vs
  dexel_mesh_mc.rs L550-563. Verified **byte-identical** body via diff;
  only difference is a doc comment on the dexel_mesh.rs copy.
- Pair 2 (0.9417): `cut_color` dexel_mesh.rs L791-801 vs `wood_color_at_z`
  dexel_mesh_mc.rs L565-574 — identical formula, renamed params. Wood
  constants `UNCUT_*/CUT_*` duplicated (mesh L16-21 vs mc L36-41); mc copy
  carries explicit comment "kept in sync with `dexel_mesh.rs`".
- Wider overlap (below threshold): `emit_z_grid_cavity_surfaces` +
  `emit_cavity_wall_edges`/`emit_cavity_wall_quad` (mesh L456-789) vs
  `emit_cavity_surfaces` (mc L445-543) — same 2x2-quad gap-match algorithm.
- Call-graph fact: `z_grid_to_solid_mesh` (mesh L173-175) **delegates** to
  `z_grid_marching_cubes`; mc has no other caller (lib.rs L42 only).
- The dexel_mesh.rs copies of `find_matching_gap`/`cut_color`/`emit_cavity_*`
  are reachable ONLY from `z_grid_to_solid_mesh_heightmap` (mesh L181), which
  is private + `#[allow(dead_code)]` with **zero callers repo-wide** (rg:
  definition + one stale doc mention in `rest_heatmap_mesh.rs:18`).
- No test exercises the legacy path either: its own doc claim ("kept ... for
  the cavity-emission tests") is stale — in-file tests (L1043+) and
  `tests/step5_marching_cubes.rs` all go through `dexel_stock_to_mesh` → MC.
  The dead code survives only because of its `#[allow(dead_code)]`.
- Live callers of the shared public API: `compute/simulate.rs` L1232/L1325,
  `viz.rs` L701/L1356, `fingerprint.rs` L683, viz `app/simulation.rs`
  L343-345, `benches/perf_suite.rs`. MC acts as the mesh engine behind them.
- Sentries pinning behavior: `tests/step5_marching_cubes.rs` (watertightness,
  z-bounds, density, drill seam), `tests/lateral_scrub_playback_stock_g_
  lateralscrub.rs` (closed-solid contract), in-file mod tests both files.

## Proposed cleanup
- home: `crates/rs_cam_core/src/dexel_mesh_mc.rs` (authoritative/live side).
- Unit: delete `z_grid_to_solid_mesh_heightmap` + `emit_z_grid_cavity_
  surfaces` + `emit_cavity_wall_edges` + `emit_cavity_wall_quad` +
  `find_matching_gap` + `cut_color` from dexel_mesh.rs (~430 L of dead code);
  fix stale doc ref in `rest_heatmap_mesh.rs:18`. Optionally export the wood
  constants from dexel_mesh.rs and have mc `use` them (dexel_mesh.rs is the
  canonical constants home per its own sync comment; they stay live there
  for preview/side-grid/drill at mesh L80/245/880/978...).
- risk: **low** (dead code; no API change; no wire-format/snapshot impact).
- proof test: `cargo test -p rs_cam_core -q --test step5_marching_cubes` +
  `cargo test -p rs_cam_core -q --test lateral_scrub_playback_stock_g_lateralscrub`
  + `cargo test -p rs_cam_core -q dexel_mesh` (in-file cavity/preview tests).
