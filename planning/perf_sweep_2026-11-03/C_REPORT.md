# P3 report: the scallop lifts its rings in parallel (agent C, perf-c)

Date: 2026-11-03. HEAD: `40cf411a7`. No commit.

## Paths touched

- `crates/rs_cam_core/src/finish/scallop/ring_generation.rs` (the change)
- `crates/rs_cam_core/tests/scallop_is_the_same_in_parallel_perf3.rs` (new sentry)
- `planning/perf_sweep_2026-11-03/C_REPORT.md` (this file)

`Cargo.toml` is not changed. The crate finds the test file by itself, and a
test build turns `test-support` on. `rayon` is already a dev-dependency.

## What changed

1. `ring_to_3d`: the per-point lift (`point_drop_cutter` plus
   `point_is_covered`) is one closure. The `parallel` build maps it with
   `par_iter` and an ordered `collect`. The serial build maps it with `iter`.
   The cascade branch also gets this speed-up. The cascade ring loop stays
   serial.
2. A new private `lift_rings(rings, ctx, cancel)` lifts independent rings
   in the input order. The `parallel` build cuts the ring list into chunks.
   It calls `check_cancel` on the calling thread before each chunk, and it
   runs `par_iter` inside the chunk. The serial build checks cancel before
   each ring.
3. The iso-field branch first extracts, filters (`len >= 3`) and decimates
   the rings serially, in the same order as before. Then `lift_rings` lifts
   them. The branch pushes each result with `len >= 3`, in the original
   order.
4. `refine_ring_chords`: the per-chord body is one closure. The `parallel`
   build runs `(0..n).into_par_iter().flat_map_iter(..)` with an ordered
   `collect`. The serial build keeps the old loop.
5. The doc comment of `generate_scallop_rings_with_cancel` now says that the
   iso-field branch polls cancel once per chunk.

No public signature changed. No drop-cutter probe is replaced by a lookup.

## Chunk size

The chunk size is `4 x rayon::current_num_threads()` rings
(`RINGS_PER_THREAD_PER_CHUNK = 4`).

- Cancel latency: one chunk takes about four ring lifts on each thread.
  Before the change, the latency was one serial ring lift. The ring lift
  itself now runs in parallel, so the wall-clock latency stays in the same
  order.
- Load balance: four rings for each thread let rayon move work from a long
  ring to the short ones. A chunk of one ring for each thread makes all
  threads wait for the longest ring at each cancel check.
- The chunk boundaries do not change the output. The sentry runs the pools
  with 1, 2 and 5 threads, so the chunks are 4, 8 and 20 rings. All give the
  same bits.

## `refine_ring_chords`: parallel, with the proof

I read `surface/chord_refine.rs` (`refine_chord`, `surface_at`,
`push_to_level`, `point_is_covered`).

- Inputs: chord `i` reads `ring[i]` and `ring[(i + 1) % n]`. These points
  come from the input ring, not from the output of an earlier chord.
- `refine_chord(a, b, ctx, depth, out)` reads `a`, `b`, `depth` and `ctx`
  through a shared reference. `ChordRefineCtx` has no interior mutability:
  three shared references and plain `f64` and `Copy` enum fields.
- `out` is only written: the function calls `out.push` and never reads,
  sorts or truncates `out`. So the earlier contents of `out` cannot change
  what chord `i` appends.
- The probes (`point_drop_cutter`, `point_is_over_mesh_xy`) are functions of
  `(x, y, mesh, index, cutter)` only.
- `refine_ring_chords` ignores the `bool` result of `refine_chord`.
- Thus the serial `out` is the concatenation over `i = 0..n` of
  `[ring[i]] ++ interior(i)`. Each chord builds its own piece, and
  `flat_map_iter(..).collect::<Vec<_>>()` keeps the order of `i`.

Caution for perf-a: this proof needs `point_drop_cutter` and
`point_is_over_mesh_xy` to stay pure functions of their arguments. A
per-thread scratch buffer is safe. A shared cache whose result depends on
the call order breaks the proof. perf-a confirms (2026-11-03): P1 adds only
per-thread scratch buffers that hold no state between calls, so the proof
holds with P1. After P1 lands, run this sentry again to confirm.

## `Sync`

`RingLiftCtx` is `ChordRefineCtx<'_>`. It holds `&TriangleMesh`,
`&SpatialIndex`, `&dyn MillingCutter` (`MillingCutter: Send + Sync`,
`tool/mod.rs:287`), `f64` fields and three `Copy` enums. It holds no debug or
trace handle. The compiler accepts the `par_iter` closures, so the type is
`Sync`. The `trace` argument and `cancel` stay on the calling thread.

## Tests

The sentry builds a bump height field (`common::meshes::height_field`,
`HALF = 12`, step 0.25 mm). It uses a region that runs 2 mm past the mesh
edge on -X, a ball of 2 mm, and a scallop height of 0.01 mm. It makes 36
(cascade) and 37 (iso field) ring annotations and 72 and 74 rapids, with 0
fed moves past the mesh edge. Each arm runs in 1, 2 and 5 thread pools. The
pins are `(moves, FNV-1a of Debug(moves))`.

| Command | Result |
|---|---|
| sentry, HEAD code, first run (floor of 40 rings too high) | `test result: FAILED. 0 passed; 2 failed` (floor only; fingerprints printed and pinned) |
| sentry, HEAD code, pinned, floor 24 | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.32s` |
| sentry, after P3 | `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.66s` |
| sentry, after P3, ring order reversed in each chunk (injected, reverted) | `test result: FAILED. 1 passed; 1 failed` (iso-field arm red) |
| `--lib finish::scallop` | `test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 2756 filtered out; finished in 1.92s` |
| `scallop_iso_field_config` | `test result: ok. 2 passed; 0 failed; 0 ignored` |
| `a_scallop_ring_connector_rides_the_surface_g_tierburial` | `test result: ok. 1 passed; 0 failed; 0 ignored` |
| `a_scallop_iso_field_link_clears_standing_stock_d1` | `test result: ok. 2 passed; 0 failed; 0 ignored` |
| `fed_chords_ride_the_surface_g_tierburial` | `test result: ok. 4 passed; 0 failed; 0 ignored` |
| `scallop_untouched_standing_h4` | `test result: ok. 4 passed; 0 failed; 0 ignored` |
| `scallop_solo_vs_unified_s1` | `test result: ok. 0 passed; 0 failed; 1 ignored` |
| `finish_resolution_policy_pr3` (pinned fingerprints) | `test result: ok. 10 passed; 0 failed; 0 ignored` |
| `ring_sample_bound_w14` | `test result: ok. 1 passed; 0 failed; 1 ignored` |
| `disconnected_finish_retract_structure_p0` | `test result: ok. 1 passed; 0 failed; 0 ignored` |
| `capability_link_moves_safety` | `test result: ok. 17 passed; 0 failed; 0 ignored` |
| `finish_surface_cache` | `test result: ok. 12 passed; 0 failed; 0 ignored` |
| `band_run_off_reproduction_d16_1` | `test result: ok. 5 passed; 0 failed; 0 ignored` |
| `shallow_band_stock_to_leave_exhibit_d16_2` | `test result: ok. 4 passed; 0 failed; 0 ignored` |
| `every_op_keeps_its_toolpath_g_opgolden` | `test result: FAILED. 0 passed; 1 failed; 1 ignored` (red at HEAD too, see below) |
| `cargo check -p rs_cam_core --no-default-features` | 7 errors, none in my file (red at HEAD, see below) |
| `cargo clippy -p rs_cam_core --all-targets --features heavy-tests,research,test-support -- -D warnings` | `Finished`, 0 warnings |
| `rustfmt --check` on the two files | clean |

I did not run the heavy binaries that name scallop (`scallop_candidates_m4`
143 s, `scallop_isofield_gouge_m4` 237 s, `scallop_oracle_validation_m4`
79 s, `checkpoint_b_resolution_ab`, `offset_candidates_m5`). The lead can ask
for them. `scallop_isofield_gouge_m4` is near the three-minute limit.

## Things the plan and the brief did not record

1. **The G-OPGOLDEN golden is red at HEAD.** The test reports
   `the generated toolpaths moved (46 changes)` over 8 ops (adaptive3d x2,
   pencil x2, scallop x2, steep_shallow, unified_finish). I ran it with the
   HEAD copy of `ring_generation.rs` (`git show HEAD:..`, then restored) and
   with P3. The 46 rows are identical in both runs, the scallop hashes
   included (`scallop/rest` gives `105b2fdb05d004bb`, `scallop/iso_field_rest`
   gives `d941c9d9c723f2b7` in both). So P3 does not move the golden. The
   golden is stale against HEAD (`tests/fixtures/op_golden_g_opgolden.toml`).
   I did not re-bless it.
2. **`--no-default-features` does not build at HEAD.** The `rayon` uses in
   `dexel_stock/band_batch.rs:49`, `dexel_stock/band.rs:38,185`,
   `dexel_stock/simulation.rs:33`, `dexel_stock/swept.rs:46`,
   `surface/pushcutter.rs:262` and `surface/slope.rs:186` have no
   `cfg(feature = "parallel")` gate. These files are unmodified against
   HEAD. The resolution errors stop the build before type check, so the
   compiler did not check my serial path. The serial path is a plain loop
   and a plain `iter().map`.
3. The timing baseline file `scratchpad/perf/base/exit.txt` reads
   `exit=137` (the process was killed). Its `time.txt` is empty. The lead
   must check the baseline before using it.
4. The sentry counts ring annotations, not rings. An annotation is one
   emitted ring run, so the floor (24) is a lower bound on chunks of work.
