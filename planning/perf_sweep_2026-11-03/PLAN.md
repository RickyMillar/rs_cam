# Performance sweep 2026-11-03 — round 1

Scope: three changes that make 3D generation faster. Each change must give
byte-identical toolpaths. The lead session decided the designs below. Agents
do not reopen them.

Source: the lead's review of a cheap-model audit (this session). Measured
input: `planning/rivmap_block_job/nz_wanaka_21_operator.toml`.

## Decisions

### P1 — `SpatialIndex::query` stops allocating a whole-mesh bitset (agent A)

- `SpatialIndex::query` allocates and zeroes `total_triangles / 64` words on
  every call, also at radius 0. On a 650 k-triangle mesh that is 81 KB per
  call. `query_rect_into` already has the single-cell fast path and the
  O(hits) clear.
- `query()` keeps its signature. It delegates to `query_rect_into` with a
  thread-local `QueryScratch`. The output is the same set in the same order.
- A new `SpatialIndex::with_query(cx, cy, radius, f)` gives `f` a `&[usize]`
  from a thread-local result buffer, so the caller allocates nothing.
- The thread-local buffers use take-and-put-back (`std::mem::take`), not a
  held `RefCell` borrow. A nested call then gets an empty buffer and never
  panics.
- `point_drop_cutter` and `point_is_over_mesh_xy` use `with_query`. No other
  caller changes in this round; they gain through `query()`.
- The false comment at `surface/dropcutter.rs:497` ("the single-cell fast
  path") is corrected.

### P2 — `project_curve` projects its points in parallel (agent B)

- The per-point projection in `project_polygon_rings_with_cancel` runs on
  rayon. The chain and emission loop after it stays serial and unchanged.
- Cancel: `&dyn CancelCheck` is not `Sync`. Use a serial loop over chunks,
  call `check_cancel` between chunks on the calling thread, and run
  `par_iter` inside a chunk. Do not change a public signature.
- Keep a serial path under `#[cfg(not(feature = "parallel"))]`.
- `point_over_triangle` is a copy of
  `surface::dropcutter::point_is_over_mesh_xy`. Delete it and call that.

### P3 — scallop lifts its rings in parallel (agent C)

- The iso-field branch of `generate_scallop_rings_with_cancel`
  (`finish/scallop/ring_generation.rs`, the `for ring in extract_rings(..)`
  loop) lifts independent rings one at a time. Lift them on rayon and keep
  the original ring order.
- `ring_to_3d`'s per-point lift map runs on rayon with an ordered collect.
  This also helps the offset-cascade branch. The cascade's ring loop stays
  serial: each ring depends on the one before.
- `refine_ring_chords` runs in parallel per chord only if the agent proves
  each chord is independent. Otherwise it stays serial, and the report says
  why.
- The cancel rule and the serial fallback are the same as P2.

### P4 — the simulation deviation pass (agent D, added after the profile)

- A profile of toolpath id 1 alone (with `--resolution 0.3`) took 300 s.
  Generation took about 14 s. `query_model_z_range` had 59 % self time and
  `SpatialIndex::query` had 23 %. The callers are `collect_column_deviations`
  and `compute_deviations` in `compute/simulate.rs`.
- Agent D measures why the candidate lists are large before it changes code.
  A frame mismatch is a correctness defect: D stops and reports it first.
- The deviation output must stay bit-identical.

## Measurements so far (baseline binary, HEAD `40cf411a7`)

- Input: `git show HEAD:planning/rivmap_block_job/nz_wanaka_21_operator.toml`,
  frozen in the scratchpad. The operator edits the live file in the GUI, so
  a live file cannot be a fixed input.
- Run 1 (0.2 mm, 18 GB scope cap): the cap stopped the run at 33.5 min,
  after the scallop. Peak RSS 18.8 GB. Generation start-to-start: tp1 about
  230 s, tp2 68 s, tp5–13 together 12 s (both project_curve ops), tp14
  297 s, tp15 460 s, tp16 741 s, tp17 scallop rings 60 s.
- G-code export refuses this project: the 18 mm tools do not cut at the
  centre and some moves descend too steeply. The identity check therefore
  compares the per-toolpath diagnostics and the agents' bit-exact sentries.

- Operator ruling 2026-11-03: leave "Front rest R1 (valleys)" (id 22) out of
  the measurement. It hangs in the GUI and ignores cancel (commit
  `91c6d6dff`). The before and after runs both use `--skip 22`.

## Result (2026-11-03)

End to end, `rs_cam_cli project` on the frozen input with `--skip 22`, 0.2 mm,
release build, same flags both sides:

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

- Identity: the 19 per-toolpath diagnostic files and the summary totals are
  equal once the timing fields are removed. G-OPGOLDEN prints the same 46
  rows as clean HEAD (that golden is stale at HEAD; not re-blessed).
- P4 gives most of the gain. The deviation pass also runs inside the
  generation of every op that reads the prior stock.
- Gate: clippy (workspace, all three features) is clean. `cargo fmt` is clean
  for the changed files; three files outside this sweep drift at HEAD. The
  demo job, the viz harness, `rs_cam_cli` and `rs_cam_mcp` pass. The core
  suite has 67 failing tests; the same 67 fail at clean HEAD. The viz suite
  has 17 failing tests; each is a source-scan, wire or UI check outside
  this sweep (not re-run at HEAD).
- `scallop_isofield_gouge_m4` passes on the merged tree (operator approval).
- `cargo check --no-default-features` fails at HEAD (rayon without a feature
  gate in `dexel_stock/`, `surface/pushcutter.rs`, `surface/slope.rs`), so
  no build checks the serial paths.

## Next candidates

- Move `ModelReach` into `mesh.rs` as one exact-candidates helper. The same
  low-side wrap still costs `point_is_over_mesh_xy`, `reach_map`
  `surface_z_at` and `gpu_upload/drawing.rs` (D_REPORT §7).
- Profile the 366 s closing phase.
- Tile sharing between checkpoints (peak RSS).

## Not in this round

- Tile sharing between simulation checkpoints (a design task).
- The per-frame rest-grid scan in `ui/properties/mod.rs:746`.
- The double carve in `compute/simulate.rs` (to be confirmed first).
- `Triangle` without `bbox`.

## Verification (lead)

- Baseline: the release CLI built from HEAD `40cf411a7`, copied before any
  edit. `rs_cam_cli project` on the operator file with `--emit-gcode`,
  `/usr/bin/time -v`.
- After: the same command on a build that has all three changes. The G-code
  must be byte-identical. The lead compares the per-op times and the peak
  RSS.
