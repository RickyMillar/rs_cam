# Wave 2 (SIM) — S2 tile early-out, S3 row-band stamping

Captured 2026-08-20 against `PERF_REVIEW.md` S2/S3 and the Phase 0 numbers in
`BASELINES.md`. Every cargo invocation serialized behind
`flock /tmp/rs_cam_cargo.lock`; other lanes were editing the same working tree
throughout.

Wave 1's parting calibration was the brief in one line — *"S7 is a rounding
error next to S2/S3; the loop's cost is three ray walks, a LUT probe and an
RMW, not two sqrts"* — and it held. S7 measured 1.04–1.07× on the arms it could
reach and nothing on the plunge fixture. S2 measures **1.46–2.07× on every arm
including both plunge arms**, on the same machine, in the same session.

Commits: `f9f26997` (S2), `5973b7cb` (S3).

**Do not read this file as a replacement for `BASELINES.md`.** It is one lane's
delta.

---

## THE HARD CONSTRAINT — held

`sim_metrics_match_golden` and `sim_metrics_3d_match_golden` — both arms,
including wave 1's 765-Arc / 4402-Helix 3D arm — are green and **unchanged**
after S2 and again after S3. `perf_golden_depth_level_geometry`, the 2335 lib
tests, and every dexel sentry named in the wave brief are green. **Nothing was
re-baselined.**

---

## 1. Bench numbers — a clean three-point pair, and why the first pair was thrown away

### Read this before reading the table

The first A/B of this wave, taken around 11:40, is **discarded**. An unrelated
repository's test suite was running at 171 % CPU with load average 22 on a
24-core box, and the *unmodified* tree read 18–134 % above wave 1's committed
numbers for identical code. The pair contradicted itself: `sim_e2e_small/res1`
"improved 74 %" while its sibling `res0.5` "regressed 20 %", both at p = 0.00.
Recording that because the failure mode is silent — criterion reports p-values
against contention just as confidently as against a real change.

Everything below was re-taken at **load average 3.8**, in one session, by
checking out three tree states in sequence and benching each:

| state | how |
|---|---|
| pre-S2 | `git checkout 25c6823e -- src/dexel_stock src/radial_profile.rs` |
| S2 | `git checkout f9f26997 -- …` |
| S2+S3 | working tree |

The pre-S2 column reproduces wave 1's committed numbers to within 1 % on every
arm, which is the check that the window was actually quiet.

### The table

`cargo bench -p rs_cam_core --bench hot_paths -- sim_`, criterion mean.
S2+S3 at `RAYON_NUM_THREADS=24`.

| Bench | pre-S2 | S2 | S2+S3 | S2 alone | S3 alone | total |
|---|---:|---:|---:|---:|---:|---:|
| `sim_kernel_lateral/flat6/cs0.25` | 15.000 ms | 10.241 ms | **9.224 ms** | 1.46× | 1.11× | **1.63×** |
| `sim_kernel_lateral/flat6/cs0.1` | 86.735 ms | 49.770 ms | **49.216 ms** | 1.74× | 1.01× | **1.76×** |
| `sim_kernel_lateral/flat12/cs0.25` | 51.527 ms | 28.706 ms | **28.238 ms** | 1.79× | 1.02× | **1.82×** |
| `sim_kernel_lateral/flat12/cs0.1` | 307.11 ms | 159.89 ms | **105.20 ms** | 1.92× | **1.52×** | **2.92×** |
| `sim_kernel_plunge/flat6_cs025/24` | 98.387 ms | 57.311 ms | **51.398 ms** | 1.72× | 1.11× | **1.91×** |
| `sim_kernel_plunge/flat6_cs025/60` | 235.25 ms | 125.99 ms | **115.75 ms** | 1.87× | 1.09× | **2.03×** |
| `sim_e2e_small/3op_2d/res1` | 17.725 ms | 10.606 ms | **11.227 ms** | 1.67× | 0.94× | **1.58×** |
| `sim_e2e_small/3op_2d/res0.5` | 67.865 ms | 32.834 ms | **31.940 ms** | 2.07× | 1.03× | **2.12×** |

Every S2 column is `p = 0.00`, "Performance has improved".

### The plunge arm is the informative row this time, as it was for S7

Wave 1 measured **nothing** on either plunge arm (p = 0.08 / 0.76) and read
that correctly: the plunge fixture drives the degenerate branch, which routes
through the already-sqrt-free `point_cell_coverage`, so S7 could not reach it.
S2 gets **1.72×/1.87×** there, and the reason is structural rather than
incidental: `plunge_pass` is plunge-*and-retract* cycles, and the retract is a
Linear feed from −5 mm back to +1 mm which `capture_cutting_segment` stamps
like any other cutting move. It is subdivided by the 0.02 mm `by_z` rule into
300 subsegments — **more than the descent's 250** — and every one of them is
pure air over ground the descent just cleared. All 300 are now whole-stamp
skips.

That is worth stating as a finding in its own right: **on plunge-heavy work
more than half the stamp budget was being spent retracting through a hole the
cutter had just made.** S1b proposes to make the descent analytic; the retract
half is gone already.

---

## 2. S2 — three things the one-line prescription got wrong

The review's S2 is one sentence: *"`tile_max_top ≤ depth_min` ⇒ skip tile,
exactly (h(r) ≥ 0)"*. The optimisation is real and the numbers above are it.
All three of the following would have shipped as a **silent metric change**.

### 2a. `h(r) ≥ 0` is a claim about the profile, not a fact — so it is measured

`RadialProfileLUT` now scans its own table at build time and publishes
`profile_is_nonneg_total()`: every sampled height inside the cutter radius is
finite, `>= 0`, **and non-decreasing**. Where it is false the skip turns itself
off rather than turning wrong.

Non-decreasing is load-bearing and is not implied by the other two. The LUT
interpolates `h0 + frac·(h1 − h0)`; with `h1 >= h0 >= 0` that is `>= h0 >= 0`
in `f64` as well as over the reals, but on a *descending* pair it can land a
few ULP below `h1`, and "`h >= 0` at the samples" would not give "`h >= 0` at
the query". `radial_profile_nonneg_total_holds_for_every_shipped_shape`
asserts all five cutter shapes across ten sizes satisfy it — including the
small-tip tapered ball, whose piecewise ball-then-cone profile was the one
plausible candidate for a float-noise dip at the junction — and re-checks the
property at the *query* surface, not just at the samples.

### 2b. `min(sd, ed)` is not the lowest tip the kernel evaluates

The per-cell tip height is `sd + t_center · seg_dd` with `t_center` **clamped**
to `[0, 1]`, so both endpoints are attainable. `seg_dd` is `fl(ed − sd)`, and
`fl(sd + fl(ed − sd))` need not be `ed`. Concretely: `sd = 1.0, ed = 1e-20`
rounds `seg_dd` to exactly `−1.0`, so the kernel evaluates a tip of **0.0** at
`t = 1` — strictly below `min(sd, ed)`. A skip that compared against
`min(sd, ed)` there would declare inert a stamp that cuts.

`segment_tip_low` returns the true attainable floor (both `fl(t·seg_dd)` and
`fl(sd + x)` are monotone, so the minimum is at whichever endpoint minimises
them). `segment_tip_low_is_the_true_floor_of_the_kernels_own_expression`
sweeps it against the kernel's own expression over ULP-neighbourhood pairs and
**asserts the naive form actually differs somewhere in the sweep**, so it
cannot go green on a fixture that never reaches the rounding case.

This is the same defect class as wave 1's S7 (`(r ± ext_diag)²` is not the
sqrt form's flip point in `f64`) and wave 1 GEN's G3 (`bbox.max.z <= cl.z` is
systematically wrong at equality). Three for three: **every "obviously exact"
bound in this review has been wrong at the last bit, and each was caught by a
bit-level oracle rather than a shape check.**

### 2c. "Skip the tile" is NOT bit-exact — the volume accumulators are a pair

This is the one that would have been hardest to see. `stamp_segment_with_metrics`
accumulates

```
pre_volume  += pre_len  · cell_area     // over covered cells, row-major
post_volume += post_len · cell_area
...
removed = (pre_volume − post_volume).max(0.0)
```

An **inert** cell has `post_len == pre_len` bit-for-bit, so it contributes the
*same* addend to both sums. Dropping it — the literal reading of the
prescription — does not cancel: `(a + X) − (b + X) != a − b` in `f64`. The
skip would move `removed_volume_est_mm3` in its last bits on exactly the
passes it was introduced to accelerate.

What landed instead:

* the **per-cell** early-out keeps the accumulator pair (one ray walk) and
  skips everything else — the LUT probe, `ray_material_length_above`, the
  blend, the second ray walk, the `cell_upper_bound_surface` sqrt, the
  `conservative_top` read-modify-write, and the whole engagement block;
* the **whole-stamp** early-out is exact for the opposite reason — with no cell
  visited both sums take identical (empty) addend sequences, so the difference
  is exactly `0.0`, `perp_max > perp_min` is false, and `max_penetration` never
  leaves `0.0`.

`the_air_skip_is_bit_exact_and_not_vacuous` diffs rays, `conservative_top` and
all four returned metrics **as bit patterns, with no tolerance anywhere**,
across 4 cutter shapes × 2 cell sizes × both cut directions.

### 2d. The mip source: `conservative_top`, not the ray tops

Two channels have to be shown inert, not one — the rays, and the sliver-safe
bound `check_rapid_collisions_against_stock` reads. Skipping on the ray tops
alone would leave `conservative_top` un-lowered on skipped cells, which is a
**safety** channel, not a metric. `conservative_top` is maintained as a
pointwise over-estimate of the ray top, so one mip over it dominates both:

```
ray_top ≤ conservative_top ≤ tile_bound ≤ tip_lo ≤ tip + h(d)
```

and `lower_conservative_top(idx, tip_hi + h(far))` is a monotone *min* with
`tip_hi + h(far) ≥ tip_lo ≥ conservative_top[idx]`, so it is a no-op too. The
`f32`/`f64` boundary does not open a gap: rounding to nearest is monotone and
`conservative_top` is already an `f32`, so
`f32(tip + h) >= f32(conservative_top as f64) = conservative_top`. Non-strict
`<=` throughout is deliberate — equality is the *common* case, not a corner
one: a flat end mill re-passing ground it already cut at the same Z hits it on
every cell.

### 2e. The refresh cadence — a silent-failure mode worth recording

`REFRESH_VISIT_MULTIPLIER` was first written as **32**, on the theory that a
mip rebuild is expensive relative to stamping. Measured consequence: on the
`sim_kernel_lateral` fixture (161 k cells, ~4.9 k cells per stamp, 960 stamps)
and on the exactness sentry alike, the budget was never exhausted, the mip
never left its build-time value of "stock top everywhere", and the whole-stamp
early-out fired **zero times in the entire suite**.

A stale mip is *sound* — that is the whole point of using a monotone-decreasing
field — which is exactly why the failure is silent. Nothing failed. The
optimisation was simply inert.

It is 1 (one grid-pass of stamped cell-visits buys one rebuild), and the
sentry now asserts non-vacuity, so the next person who tunes it in the cautious
direction gets a red test instead of a flat bench.

The real ratio is ~1:40 — a rebuild touches one `f32` per cell, a stamp visit
runs a projection, a coverage test, a LUT probe, three ray walks and an RMW —
so the maintenance overhead at 1 is a few percent and it is bought back many
times over.

### 2f. Where the per-cell skip fires, and where it does not

Measured on the sentry sweep: **~15 % of in-bbox cells overall, ~40 % on the
flat arm alone.** That gap is a finding, not a fixture artefact.

`conservative_top` is lowered to `tip + h(near_dist + cs·√2/2)`, so on a
round-profile cutter a re-pass over its *own* ground still reads a bound
strictly above the tip — **correctly**, because the rim cells of the old stamp
really do still hold material the new stamp will take. Overlap therefore only
becomes skippable when `h ≡ 0` across the cell. For ball, v-bit and tapered
tools the early-out fires on genuine air and not on overlap.

So S2's overlap half is a flat-tool win and its air half is universal. Anyone
sizing S2 for a 3D finishing workload should expect the air half only.

---

## 3. S3 — the decomposition is right, the dispatch granularity is the whole game

### 3a. "Bit-identical" does not survive contact

The review says row bands are bit-identical because per-cell mutation order is
preserved. **The mutation order half is true and the conclusion is not.**

Order is preserved for free: a cell belongs to exactly one band, so
`ray_blend_above`'s non-commutativity never gets a chance to bite, and no cell
is ever touched by two workers. Bit-identical are: the rays, `conservative_top`,
`axial_doc_mm` (a max), `radial_engagement` (a min and a max), the arc derived
from it, and the sample stream's order, `sample_index` numbering and timings.

**`removed_volume_est_mm3` is not.** It comes off the `pre_volume`/`post_volume`
pair of §2c, and splitting the rows splits both sums:
`(a₁+a₂)+(a₃+a₄) != ((a₁+a₂)+a₃)+a₄`. Banding reassociates it, deterministically
but not identically.

The golden absorbed this without moving because it was *designed* to — its own
docs say accumulated sums carry a `1e-3` relative tolerance "because float
addition is not associative and any change to iteration order — the thing S3
explicitly does — reassociates the sum". The wave brief's "bit-identical" is a
stronger claim than the net it was handed.

What **is** guaranteed, and is the property that matters: the band
decomposition is a function of the grid's row count and never of
`rayon::current_num_threads()`. `band_stamping_determinism_s3` pins bit-identity
of the grid *and* of the full sample stream at 1, 2, 4 and 8 threads, over a
fixture mixing overlapping raster lines, a ramp, a plunge, an arc and a re-pass.
Had the granularity been thread-derived, a golden captured on a 4-core box
would be un-reproducible on a 24-core one.

### 3b. The same asymmetry sank the first per-band S2 early-out

Worth recording because it is the exactness argument of §2c reappearing one
level down, and the sentry caught it within the hour: a band that takes the
whole-stamp skip for *its share* drops the same addend from both volume sums,
which is the `(a+X)−(b+X)` defect again. The mip is now asked about the stamp's
**global** bounding box, so every band reaches the same verdict — all of them
skip or none do, and then both sums stay at `0.0`.

### 3c. Thread scaling — it saturates at 4 and gets worse past 8

`sim_kernel_lateral/flat12/cs0.1`, 16 bands per stamp:

| threads | 1 | 2 | 4 | 8 | 24 |
|---|---:|---:|---:|---:|---:|
| ms | 165.1 | 102.3 | **78.5** | 78.6 | 108.6 |
| vs 1 thread | 1.00× | 1.61× | **2.10×** | 2.10× | 1.52× |

The review predicts 6–12×. Measured ceiling: **2.10×, at four threads, on the
largest-footprint arm in the suite.** Everything else does not dispatch at all.

The reason is structural, not tuning. A stamp is tens of microseconds of work —
`flat12/cs0.1` is 160 ms over 960 subsegments, i.e. 166 µs each, and split 16
ways that is 10 µs per task. The join tree deepens with the pool, and nothing
pins a band to a worker, so the same rows migrate between cores on *every
subsegment*. Coarsening the split with `with_min_len(4)` was tried and made
every thread count slower (105 ms at 4, 172 ms at 24) — it is not in the tree.

### 3d. The crossover is measured, and below it the parallel path LOSES

At `PARALLEL_MIN_BBOX_CELLS = 3000` the fine-cell Ø6 arm (~4.3 k cells per
stamp) was a **28 % regression** and `sim_e2e_small` was 25–40 % worse. The
constant is now **12 000**, sitting above the losing point:

| arm | est. bbox cells | S3/S2 at 3 000 | S3/S2 at 12 000 |
|---|---:|---:|---:|
| `flat6/cs0.1` | ~4.3 k | 0.72× | 1.01× |
| `flat12/cs0.1` | ~15.7 k | 1.48× | 1.52× |

### 3e. Two overheads that were not parallelism at all

Both were found by benching, not by reading:

* **`CoverageFastPath::new` sat above the bounding-box early-outs.** It costs a
  handful of `sqrt`s plus up to four bounded ULP walks (wave 1's S7 machinery),
  and under banding it ran **once per band** instead of once per stamp. Moving
  it below the early returns was worth ~20 % on the fine lateral arm and ~40 %
  end-to-end.
* **The fan-out visited every band in the grid**, not the stamp's own rows. On
  a 500-row grid with a Ø6 tool at 0.1 mm that is 63 bands per subsegment
  instead of 9. `stamp_row_span` bounds it; because an out-of-range band
  returns `StampPartial::empty()`, which is the identity of `merge`, the
  restriction is numerically free.

Together these are **1.02–1.11× on the arms that never dispatch** — i.e. most
of S3's measured benefit outside `flat12/cs0.1` is not parallelism.

### 3f. What would actually reach 6–12×, and why it was not attempted here

Amortise the dispatch over a **whole toolpath** (or a chunk of a few thousand
subsegments) rather than per stamp: one `par_bands` call, each band replaying
every stamp clipped to its own rows, partials reduced afterwards. Two things
change, and both are the ones that matter:

1. the join cost is paid once per thousands of stamps instead of once per
   stamp;
2. a band stays on one worker for the whole replay, so its rows stay in that
   core's cache instead of migrating every subsegment.

The cost is a two-phase restructure of `simulate_toolpath_with_lut_metrics_cancel`:
a pass that enumerates subsegments and emits samples with placeholder metrics,
a parallel pass over bands producing per-subsegment partials, and a serial pass
that reduces and patches. The sample stream's ordering, `sample_index` numbering
and timings must come out of the serial pass unchanged — the wave brief flagged
this as likely the hardest part of S3 and that assessment is correct. Memory
wants chunking: 64 bands × 70 k subsegments × 96 B is 430 MB unchunked.

`GridBand`, `StampPartial`/`merge`/`finish`, the row-span helper and the
determinism harness all landed here and are the substrate that work needs.

---

## 4. Correctness

* **Both goldens green and UNCHANGED**, after S2 and again after S3. No
  re-baseline.
* **`cargo test -p rs_cam_core --lib`: 2335 passed, 0 failed.**
* Sentries green: `dexel_stock_z_frame_f024`, `dexel_stock_z_frame_f026`,
  `sub_cell_stamping_fa`, `engagement_vector_step2`, `drill_metrics_pr2`,
  `descent_resolution_stability_am10`, `face_stock_top_frame_f028`,
  `sim_identity_setup_playback_frame`, `step5_marching_cubes`,
  `adaptive_feed_modulation_pipeline_f036b`, `lead_in_out_feed_rates_f040`,
  `capability_link_moves_safety`, `perf_golden_depth_level_geometry`.
  `kinematics_histogram` is ignored by its own gate.
* Clippy clean on `--lib --benches` plus this lane's tests, `-D warnings`.
  (An unscoped `--all-targets` run fails in `geom_cache.rs`, which is the G8
  lane's in-flight file — not touched, per the lane protocol.)
* `cargo fmt --check` clean for this lane's files; the one diff it reports is
  in `edge_distance.rs`, the GEN lane's.

### New nets

| Test | What it pins |
|---|---|
| `the_air_skip_is_bit_exact_and_not_vacuous` | S2 moves no bit of the rays, `conservative_top` or any returned metric — and actually fires |
| `segment_tip_low_is_the_true_floor_of_the_kernels_own_expression` | the tip floor is the kernel's own attainable minimum, and the naive `min(sd, ed)` really does differ in the sweep |
| `radial_profile_nonneg_total_holds_for_every_shipped_shape` | the S2 precondition is true for all five cutter shapes, checked at the query surface |
| `tile_bound_dominates_every_cell_it_covers` | the mip is an upper bound, exhaustively, on a ragged grid |
| `lowering_cells_without_a_refresh_keeps_the_bound_valid` | staleness only loosens |
| `the_refresh_cadence_is_amortised_but_not_asleep` | both halves — not per stamp, and not never |
| `band_stamping_is_bit_identical_across_thread_counts` | grid AND sample stream bit-identical at 1/2/4/8 threads |
| `bands_partition_every_cell_exactly_once` | the band split is a partition and `local()` agrees with the grid layout |
| `the_fixture_actually_spans_multiple_bands` | the determinism test is not four serial runs |

---

## 5. Files taken outside the stated ownership

* `src/radial_profile.rs` — unowned by any lane. Needed for
  `profile_is_nonneg_total`; `heights` is private, and probing the accessor
  cannot be exhaustive.
* `src/dexel_stock/{band,tile_mip}.rs` — new, inside owned territory.
* `compute/simulate.rs` was **not** taken; S4/S5 remain untouched.

## 6. Corrections owed to `PERF_REVIEW.md`

Applied in place and left **UNSTAGED** for consolidation with other lanes'
hunks, per the wave protocol:

1. **S2** — "skip tile, exactly" annotated with the volume-accumulator pair
   (§2c), the `h ≥ 0` measurement (§2a), the tip-floor ULP correction (§2b),
   and the flat-tool restriction on the overlap half (§2f).
2. **S3** — "bit-identical" corrected to "bit-identical except
   `removed_volume_est_mm3`, which reassociates deterministically" (§3a), and
   "6–12× desktop" corrected to the measured 2.10× ceiling at four threads with
   the reason (§3c) and the route past it (§3f).
