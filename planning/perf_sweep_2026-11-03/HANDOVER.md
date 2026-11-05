# Perf sweep hand-over (2026-11-06, after round 2)

Read this file first, then `ROUND2_BRIEF.md` (round 2: brief, results,
next candidate) and `PLAN.md` (round 1). The agent reports `A_REPORT.md`
.. `F_REPORT.md` hold the detail for each change.

## 1. State

- Round 1 is committed: `98344a12f` (1.83x on the nz-wanaka-21 job).
- Round 2 is committed on `master`, not pushed:
  - `bb179e8bd` W1: radius-0 callers read no band off the low sides of
    the grid (`SpatialIndex::with_point_candidates`, `E_REPORT.md`).
  - `7c06e049d` W3: the holder collision sweep runs once and on rayon
    (`F_REPORT.md`). W2 was the profile that found it.
- Round 2 result, paired, same flags (`--skip 22 --resolution 0.3
  --memory-limit 15GiB --emit-gcode`, input frozen at `0c05e90e7`):
  base 5:20.7 -> W1 4:38.6 -> W3 2:56.3 (1.82x). Peak RSS 8.99 GB on
  all three. The G-code is byte-identical on all three; the 18 diag files
  are equal without the timing fields.
- Round 2 ran at 0.3 mm, not 0.2 mm: only 17-20 GB RAM was free (browser
  tabs, full swap). Its numbers do not compare with the round-1 table.
- G-code export now works on `nz_wanaka_21_operator.toml`, so byte-equal
  G-code is the identity check from now on (the §6 note is out of date).
- No perf work is open. No agent runs. No file holds an uncommitted perf
  edit.

## 2. What round 1 did

| Id | Change | Main file |
|---|---|---|
| P1 | `SpatialIndex::query` reuses a thread-local scratch; new `with_query` gives a `&[usize]` without an allocation | `rs_cam_core/src/mesh.rs` |
| P2 | `project_curve` projects its points on rayon, in chunks of 1024, with a cancel check between chunks; the copy `point_over_triangle` is deleted | `ops/project_curve.rs` |
| P3 | scallop lifts its rings and refines its chords on rayon, in ring order | `finish/scallop/ring_generation.rs` |
| P4 | the deviation pass asks `ModelZQuery`: the exact cell list, else a reach-box miss test, else the old query | `compute/simulate.rs` |

P4 gives most of the gain. The cause: `SpatialIndex` computes
`x1 as usize` for a point left of or below the grid. The cast wraps, so the
query scans a whole row or column of cells, or the whole mesh at a corner.
The deviation pass also runs inside each op that reads the prior stock.

Each change has a bit-exact sentry written before the change:

- `tests/a_spatial_query_reuses_its_scratch_perf1.rs`
- `tests/project_curve_is_the_same_in_parallel_perf2.rs`
- `tests/scallop_is_the_same_in_parallel_perf3.rs`
- `tests/column_deviation_is_the_same_and_fast_perf4.rs`
- 5 unit tests in `compute/simulate.rs` `model_z_query_tests`

## 3. Result (at `98344a12f`, release CLI, `--skip 22`, 0.2 mm)

| Section | Before | After |
|---|---|---|
| Back bulk rough (tp1) | 191 s | 43 s |
| Back rest + drills (tp2–4) | 69 s | 62 s |
| Rivers + 2D ops (tp5–13) | 13 s | 10 s |
| Front bulk rough (tp14) | 332 s | 153 s |
| Front 3D rough (tp15) | 335 s | 194 s |
| Front scallop finish | 61 s | 9.5 s |
| Final simulation + reports | 535 s | 366 s |
| Whole job | 25:41 | 14:02 |
| Peak RSS | 17.8 GB | 17.7 GB |

Identity proof: the 19 per-toolpath diagnostic JSON files and the summary
totals are equal after the timing fields are removed. G-OPGOLDEN prints the
same 46 rows as clean HEAD.

## 4. How to measure

1. Freeze the input. The operator edits the live file in the GUI, so never
   measure on the live file:
   `git show 40cf411a7:planning/rivmap_block_job/nz_wanaka_21_operator.toml > <dir>/input_head.toml`
   For a new round, freeze the HEAD version instead, and write the commit in
   `PLAN.md`.
2. Build a baseline release CLI from clean HEAD before any edit. Copy the
   binary out of `target/` so a later build does not replace it.
3. Run each binary with the same flags:
   `/usr/bin/time -v <cli> project <dir>/input_head.toml --output-dir <out> --skip 22 --resolution 0.2 --summary`
4. Compare the per-toolpath diag JSON files with the timing fields removed.
   G-code export cannot be the identity check: the project refuses export
   (see §6).
5. For a profile, build with symbols (`strip = "none"`, frame pointers on)
   and use `perf record -g`. The normal release binary is stripped.

Rules for the run:

- `--skip` takes toolpath **ids**, not list positions.
- Id 22 ("Front rest R1 (valleys)") stays out of every run (operator ruling
  2026-11-03). It hangs and ignores cancel.
- A full run takes about 15 minutes and 18 GB. Ask the operator first (the
  3-minute rule in `CLAUDE.md`). Run 1 at HEAD was killed by an 18 GB cap,
  so give the run a cap above that, or none.
- The scratchpad from the old session is on `/tmp`, a RAM-backed tmpfs.
  Do not rely on it after a reboot.

## 5. Next candidates

Round 2 did candidates 1 and 2 below (W1, W2 + W3). Open:

- The generation heightmap (`surface::slope` `from_mesh` ->
  `sample_grid_cell` -> `point_drop_cutter` + `query_rect_into`): about
  56 % of generation self time; already parallel. A design task (see
  `ROUND2_BRIEF.md` "Next candidate"). Watch the tie order of the
  contact record before you reuse candidate lists.
- Tile sharing between simulation checkpoints (peak RSS), unchanged.
- The flaky viz test `with_no_settings_the_budget_is_half_of_the_system_memory`
  hung once in a parallel `controller::tests:: compute::worker::tests::`
  run (2026-11-06) and passes alone. Not perf code.

Round-1 text, kept for reference:

Ask the operator which one to do. Recommended order:

1. **One exact-candidates helper in `mesh.rs`.** Move `ModelReach` and the
   cell-list read from `compute/simulate.rs` into `mesh.rs`, and use it from
   the other radius-0 callers. They still pay the wrap cost (D_REPORT §7):
   - `surface/dropcutter.rs` `point_is_over_mesh_xy`: millions of calls per
     3D finish.
   - `maps/reach_map.rs` `surface_z_at`: about 10^6 calls on a wanaka-size
     map; also called from `ops/project_curve.rs`, `finish/surface_link.rs`
     and `metrology/monge.rs`.
   - `rs_cam_viz/src/app/gpu_upload/drawing.rs`: one call per drawing vertex
     per upload.
   A simpler alternative: clamp the cell range in `SpatialIndex` itself, so
   a point off the grid gives an empty range, not a wrapped one. The output
   must stay bit-identical, so prove the clamp never drops a candidate
   (watch the edge margin and slivers; `ModelReach` has an `ill` list for
   them).
2. **Profile the 366 s closing phase** (final simulation and reports). No
   profile exists for it yet.
3. **Tile sharing between simulation checkpoints**, for the 17.7 GB peak.
   `push_checkpoint` clones the stock twice. This is a design task, not a
   local fix.

Out of this round (see `PLAN.md`): the per-frame rest-grid scan at
`ui/properties/mod.rs:746`, the possible double carve in
`compute/simulate.rs`, and `Triangle` without `bbox`.

Rejected ideas from the first audit (do not reopen):

- A heightmap probe in place of the drop cutter: it gouges.
- The `rest_regions` clone: it is an `Arc`, so it costs nothing.
- "Allocator lock contention" in `query`: those paths were serial.

## 6. Failures that exist at HEAD (not from this sweep)

- The core full gate: 67 tests fail; the same 67 fail at clean HEAD.
- The viz suite: 17 source-scan, wire or UI tests fail. They were not re-run
  at HEAD (the HEAD build died on `/tmp`); all are outside the sweep files.
- `simulation_issue_channel_m1` fails at HEAD.
- The G-OPGOLDEN golden is stale at HEAD. It was not re-blessed; that needs
  an operator ruling.
- `cargo fmt --check` drifts in three files outside the sweep.
- `cargo check --no-default-features` fails (rayon without a feature gate in
  `dexel_stock/`, `surface/pushcutter.rs`, `surface/slope.rs`), so no build
  checks the serial `#[cfg(not(feature = "parallel"))]` paths.
- G-code export refuses `nz_wanaka_21_operator.toml`: the 18 mm tools do not
  cut at the centre. Feeds-contract commit `b9bf55844` touched this area;
  check again before you rely on it.

## 7. Traps

- Disk: the shared `target/` grows past 130 GB. Check `df -h /` first. Free
  space with `cargo sweep --time N` (`--dry-run` first), never `cargo clean`.
  Guard a long gate with a `df` watcher.
- `/home/ricky/personal_repos/rs_cam_target_alt` is an old cargo target
  directory (34 GB, unused since 2026-09-10), not a worktree.
- Never put a `CARGO_TARGET_DIR` on `/tmp`: the tmpfs fills and the link
  dies with SIGBUS.
- Run cargo through `scripts/cargo_lane.sh`. Run viz tests with `-j 2`.
- Never run tree-wide git operations; other sessions share the tree. Commit
  by path.
- The lead decides and verifies. Agents implement and never commit.
- `Cancel` is not `Sync`: poll it on the calling thread between parallel
  chunks, as P2 and P3 do.
- Thread-local buffers use take-and-put-back (`Cell` + `std::mem::take`), so
  a nested call gets an empty buffer and never panics.
- Before you trust a GUI number, check that `rs_cam_gui` was built from HEAD.
