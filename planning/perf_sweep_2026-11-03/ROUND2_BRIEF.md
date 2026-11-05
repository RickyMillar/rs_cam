# Perf sweep round 2 — lead brief (prepared 2026-11-06, not started)

Status: written while the feeds session holds the lane. No code is changed.
Start only after `rs-cam-96` sends its final HEAD.

## Start checklist (lead)

1. `git status --short` is clean for the paths below; record HEAD here.
2. `df -h /` — free disk with `cargo sweep --time N --dry-run`, then real.
3. `free -g` — a full wanaka run needs about 18 GB.
4. Freeze `planning/rivmap_block_job/nz_wanaka_21_operator.toml` at HEAD
   into the scratchpad (not `/tmp` for the target dir).
5. Build the baseline release `rs_cam_cli` from clean HEAD; copy it out.
6. Check if G-code export now works on the frozen input (`--emit-gcode`).
   If yes, byte-identical G-code is the identity check; else the diag JSON.

## W1 — the low-side wrap in `SpatialIndex` (one Opus agent)

Facts the lead read (HEAD of 2026-11-05):

- `SpatialIndex::build` sets `origin = mesh.bbox.min`
  (`crates/rs_cam_core/src/mesh.rs:462`).
- `query_rect_into` (`mesh.rs:720`) computes `x1 as usize` on a negative
  `isize`. The cast wraps to a large value, `.min(ncx-1)` gives the last
  column, and the query reads columns `0..=ncx-1`. Same for Y. A point
  left of or below the grid therefore reads a whole row, column or mesh.
- The high side already returns empty (`x0 > x1`). So the current code is
  asymmetric: low side over-includes, high side gives nothing.
- `build` uses the same arithmetic (`mesh.rs:503-506`), but there a face
  bbox is never left of `origin`, so the wrap cannot fire in `build`.
- 44 non-test call sites read `query`, `with_query`, `query_into` or
  `query_rect_into`.

The trap: `Triangle::contains_point_xy` accepts points a small distance
outside the triangle (barycentric `>= -1e-8`). `ModelReach`
(`compute/simulate.rs:3098`) bounds that distance by `1e-5 * W` plus a
size term, and lists ill-conditioned triangles in `ill`. A point just left
of `origin` can be accepted by a triangle in column 0, or (a large sliver)
in a later column. Today the wrap finds it. A naive "empty when `x1 < 0`"
clamp drops it and changes output.

Agent task:

1. Write a bit-exact sentry first: for every radius-0 caller class
   (`point_is_over_mesh_xy`, `reach_map::surface_z_at`, `ModelZQuery`,
   `query`) the result on points off each low side, at each corner, and
   within the acceptance margin, on a terrain mesh and on a mesh with
   slivers, equals the result from the pre-change code.
2. Design options, in order of preference:
   a. Fix the cell range in `query_rect_into` for the off-grid low side so
      that it reads only the cells that can hold an accepting triangle,
      with a proof from the `ModelReach` bound. If a triangle's band can
      reach past column 0 (large W), the query must still find it.
   b. Else move `ModelReach` from `compute/simulate.rs` into `mesh.rs` as
      one exact-candidates helper on `SpatialIndex`, and use it from
      `point_is_over_mesh_xy`, `surface_z_at`, `ModelZQuery` and
      `gpu_upload/drawing.rs`.
3. Update the doc comment at `mesh.rs:711-715` ("must not fix it").
4. Report the proof, the sentry names, and per-caller call counts before
   and after on a wanaka-size probe.

Rules for the agent: no commit; `scripts/cargo_lane.sh` for every cargo
command; `timeout` on every test run; format only its own files; read
`crates/rs_cam_core/CLAUDE.md`, `src/surface/CLAUDE.md`,
`src/maps/CLAUDE.md`, `src/compute/CLAUDE.md` first.

Lead gates after W1: the 4 round-1 perf sentries, the new sentry,
`--test every_op_keeps_its_toolpath_g_opgolden` (diff against
`planning/feeds_contract_2026-11-03/opgolden_known_46_rows.txt`),
`--lib` for core, viz `--lib` (`-j 2`), `rs_cam_mcp`, clippy (workspace,
three features). Then the full wanaka run, before and after.

## W2 — profile the closing phase (366 s)

Symbol build (`strip = "none"`, frame pointers), `perf record -g` on the
final simulation and reports. Lead reads the profile and decides a fix.

## W1 result (2026-11-06)

- The agent chose option (b): `SpatialIndex::with_point_candidates` in
  `mesh.rs` holds the P4 box (`ModelReach` renamed `PointReach`). Option (a)
  was not provable: a near-collinear sliver accepts a point 3.39 mm left of
  its own box (sentry `an_off_grid_query_reads_no_band_perf5`). Detail:
  `E_REPORT.md`.
- Callers moved: `point_is_over_mesh_xy`, `reach_map::surface_z_at`,
  `ModelZQuery`, viz `gpu_upload/drawing.rs`.
- Measurement: HEAD `0c05e90e7`, input frozen at that HEAD, release CLI,
  `--skip 22 --resolution 0.3 --memory-limit 15GiB --emit-gcode`. The
  resolution is 0.3, not 0.2: only 17 GB RAM was free and the swap was
  full, so these numbers do not compare with round 1.

| Section | Base | W1 |
|---|---|---|
| Back rest 6 mm (tp2) generation | 36.7 s | 30.2 s |
| Front 3D rough (tp21) generation | 111.4 s | 79.7 s |
| Whole job (wall) | 5:20.7 | 4:38.6 |
| Peak RSS | 8.99 GB | 8.99 GB |

- Identity: the G-code (6.0 MB) is byte-identical, and the 18 diag JSON
  files are equal once the timing fields (`*_us`, `*_s`, `*time*`) are
  removed. G-code export now works on the operator file.
- Gates: G-OPGOLDEN prints exactly the 46 known rows; core `--lib`
  2774/0; perf1-perf5 sentries green; viz `--lib` 695/0 (agent);
  `rs_cam_mcp` 39/0; `rs_cam_cli` 105/0; clippy clean; demo job passes.
  The core full gate was not run (operator ruling 2026-09-11).

## W2 result — the closing-phase profile (2026-11-06)

Symbol build (`release-fast` with fat LTO, one codegen unit, `strip = none`,
frame pointers), `perf record -F 99 -g`, same input and flags as W1.
The closing phase (after "generation plan finished") took 113 s: the final
simulation part 75 s, the reports part 38 s. About 99 % of it was
`run_collision_check` -> `check_collisions_interpolated_with_cancel` ->
`point_drop_cutter` -> `query_rect_into`, on ONE thread (11 329 samples at
99 Hz). The CLI ran the same holder sweep three times: its own loop,
`session.diagnostics()` and `session.triage()`.

## W3 result (2026-11-06)

- W3a: `ProjectSession::holder_collision_sweep` runs the sweep once;
  `holder_collision_counts_of`, `diagnostics_with_holder_collisions` and
  `triage_with_holder_collisions` read it. The CLI calls it once.
- W3b: `check_collisions_interpolated_with_cancel` checks moves on rayon in
  batches of 1024, cancel polled between batches, events joined in move
  order. Serial path kept under `cfg(not(feature = "parallel"))`.
- Sentries: `the_collision_walk_is_the_same_in_parallel_perf7` (4),
  `--lib the_collision_sweep_runs_once_perf6` (1). Detail: `F_REPORT.md`.

| Run (0.3 mm, same flags) | W1 | W3 |
|---|---|---|
| Whole job (wall) | 4:39.0 | 2:56.3 |
| Peak RSS | 8.99 GB | 8.98 GB |

- Identity: the G-code is byte-identical to W1 and to the round-2 base;
  the 18 diag files are equal without the timing fields; the printed
  report (133 lines, triage included) is identical.
- Round 2 so far, base -> W3: 5:20.7 -> 2:56.3 (1.82x), same output.
- Gates (agent, after its last edit): core `--lib` 2775/0; 15 core
  sentry binaries 0 failed; `rs_cam_cli` 105/0; `rs_cam_mcp` 39/0; viz
  `--lib` 695/0; clippy clean. The lead re-ran perf6 and perf7.

## Next candidate (not started)

The generation profile (0-166 s): `surface::slope` `from_mesh` ->
`sample_grid_cell` -> `point_drop_cutter` + `query_rect_into` is about 56 %
of self time (the "Surface heightmap" step, 26 s of tp21's 54 s). It is
already parallel; a gain needs an algorithm change (reuse of the candidate
list between neighbour grid points). A design task.
