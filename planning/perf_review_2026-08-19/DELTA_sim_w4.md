# Wave 4 (SIM) — S3 whole-toolpath band dispatch, and one defect

Captured 2026-08-20 against `PERF_REVIEW.md` S3, `DELTA_sim_w2.md` §3f (the
spec) and `DELTA_sim_w3.md` §6 (the defect). Every cargo invocation serialized
behind `flock /tmp/rs_cam_cargo.lock` with a ≥20 GB available-memory gate.

**Do not read this file as a replacement for `BASELINES.md`.** It is one
lane's delta.

Commits: `c652ee52` (defect), `38b8e4a9` (restructure), `d1d9a0a4` (sentries +
bench arm), `ee8b9da4` (memory-bound test tightened).

---

## THE HARD CONSTRAINT — held, and this wave holds a stronger one

`sim_metrics_match_golden`, `sim_metrics_3d_match_golden` and
`perf_golden_depth_level_geometry` are green and **unchanged**. The wave-3 S5
sentries (`sim_prefix_memo_s5`, whose snapshot captures this kernel's outputs)
are green and unchanged. **Nothing was re-baselined.**

Beyond that: whole-toolpath dispatch is **bit-identical to per-stamp dispatch**,
on the grid, on `conservative_top` and on every metric field of every sample —
including `removed_volume_est_mm3`, which wave 2 could only promise reassociates
*deterministically*. The brief explicitly offered this wave the same latitude
and it was not used; see §2d.

---

## 1. The defect — `DELTA_sim_w3.md` §6, and it was in six places

`stamping.rs:504` computed

```rust
let bbox_cells = (row_hi + 1 - row_lo).saturating_mul(col_hi + 1 - col_lo);
```

over clamps that can leave `row_lo > row_hi`. Debug panicked with "attempt to
subtract with overflow", release wrapped — so the two builds disagreed, which
is the same class as the `debug_assert!`-in-a-dependency note `CLAUDE.md`
already carries for the offset library. §6's reproduction is a raster pass at
`y ∈ [31, 35]` over a stock whose Y extent is `[0, 24]`, and it is reachable
from an ordinary project: profile lead-ins, edge drills and any op whose
boundary extends past the blank all leave the stock.

§6 also flagged the second half, on the opposite side: a **negative** `col_max`
cast through `usize` wraps to a huge value that `.min(cols - 1)` clamps back to
the last column, so a stamp entirely to the LEFT of the grid walked the whole
column range. Every one of those cells was rejected on coverage, so the results
were right and the work was not.

### The finding §6 could not have made from the one line it saw

The prescription is "fix it at `stamping.rs:504`". The pattern is at **six**
sites and one of them is the reported line:

| Site | Underflow | Wrapping cast | Hot path? |
|---|---|---|---|
| `stamp_point_on_grid` (the reported line) | yes | yes | playback |
| `stamp_segment_on_grid` | yes | — (float→int casts saturate) | playback |
| `stamp_segment_with_metrics`, swept branch | guarded | **yes** | **metric** |
| `stamp_segment_with_metrics`, degenerate branch | guarded | **yes** | **metric** |
| `band::stamp_row_span` | — | **yes**, one level up | **metric** |
| `local_material_sum` / `max_top_z_in_disc` (`mod.rs`) | — | **yes** | queries |

The metric kernel is the interesting row. It already guarded the underflow
(`if row_lo > row_hi || col_lo > col_hi { return out; }`) so it never panicked
— and it still folded an off-grid footprint onto row/column 0 and walked it.
`stamp_row_span` did the same one level up: it clamped a below-the-grid span to
`(0, 0)` and handed **band 0** a stamp with no cells in it. Which is to say the
defect was quietly costing work on the hot path, not only crashing debug builds
on the cold one, and the reported site was the least consequential of the six.

All six now go through one helper, `clamped_cell_bbox`, which returns `None`
for a footprint with no cells — the answer the cell loop would have computed
anyway. `GridBand` gains `grid_rows`, because the global-bbox clamp is a
question about the grid rather than about the band.

**No result moves.** The mip sees the same tiles either way (`max_over` clamps
its own arguments), and a walked-but-rejected cell contributes nothing to any
accumulator.

### The nets, and the one that makes the others mean something

`stamps_entirely_outside_the_grid_are_a_clean_skip` (both non-metric kernels,
mip on and off, all four sides **and all four corners** — the underflow needs
one axis outside and the wrapping cast needs the other, so a sides-only fixture
proves less than it looks); `out_of_grid_metrics_equal_not_stamping_at_all`
(the metric kernel's four published numbers, bit for bit, against never calling
it, plus `bbox_cells == 0`); `a_toolpath_that_leaves_the_stock_does_not_panic`
(§6's shape through the public simulator, metric and playback paths);
`off_grid_disc_queries_are_empty_not_full_grid`.

And `the_pre_fix_clamp_really_did_underflow_and_over_walk`, which keeps the
pre-fix expressions verbatim as an oracle — the technique `legacy_fast_path`
uses for S7 — and asserts that the fixture's own offsets reach **both** defect
classes. Without it the four tests above could go green on a fixture that never
gets near the bug, which is the failure mode wave 2 §2e recorded for the mip
refresh cadence.

---

## 2. The restructure — §3f implemented, with four corrections

New module `crates/rs_cam_core/src/dexel_stock/whole_path.rs`; the enumerating
loop and the two shared helpers are in `dexel_stock/simulation.rs`. The shape is
`StampDispatch::{Auto, PerStamp, WholeToolpath}` on `TriDexelStock`, so both
dispatch shapes are live in one binary and can be A/B'd in one process.

### 2a. The three phases

1. **Enumerate (serial)** — `simulation.rs::capture_cutting_segment`. Unchanged
   move loop, unchanged subdivision, unchanged cancel poll per subsegment. It
   pushes the sample through `push_cutting_sample` and records a `StampJob`
   carrying the sample's slot.
2. **Replay (parallel)** — `whole_path.rs::BandDispatch::run_batch`. One
   `par_bands` over the batch's own union row span (§3d — dispatching every
   band in the grid was measurably wrong), zipped with per-band job buckets;
   each band replays its own jobs in job order, clipped to its own rows,
   through the *unmodified* `stamp_segment_with_metrics`.
3. **Reduce and patch (serial)** — same function. Per job, merge its bands in
   ascending band order into a `StampPartial`, apply the driver-side degenerate
   fix, `absorb` into the mip, `finish`, and hand the four numbers to
   `apply_subsegment_metrics`, which writes them onto the already-emitted
   sample.

### 2b. CORRECTION — the way to keep the sample stream identical is not to
### reproduce it

§3f: *"a pass that enumerates subsegments and emits samples with placeholder
metrics … the sample stream's ordering, `sample_index` numbering and timings
must come out of the serial pass unchanged — the wave brief flagged this as
likely the hardest part of S3 and that assessment is correct."*

The assessment is correct and the framing invites the wrong implementation. A
"phase 1 that emits samples" alongside a per-stamp path that also emits samples
is **two enumerators that must be kept in step by hand**, and there is no test
that can prove two loops will stay identical under future edits — only that
they are identical today.

What landed instead has **one** enumerator and **one** metric applicator, both
shared:

* `push_cutting_sample` is the only place in the file that pushes a
  `SimulationCutSample` for a cutting subsegment. Both dispatch shapes call it.
* `apply_subsegment_metrics` is the only place that turns the kernel's four
  numbers into the eight sample fields derived from them. Both shapes call it.

The dispatch mode therefore selects *what happens to the geometry*, never what
happens to the stream. Ordering, numbering and timings are identical by
construction; a divergence would have to be a divergence in the stamp kernel,
which is what the bit-identity sentries test.

One ordering detail was moved and is worth stating: `cumulative_time_s` is now
accumulated **before** the stamp instead of after. The stamp does not feed the
clock, so this is not observable — and the sentry compares
`cumulative_time_s` as a bit pattern across both shapes, which is the check that
turns "not observable" from an argument into a measurement.

### 2c. CORRECTION — the batch size is set by the S2 mip, not by memory

§3f: *"Memory wants chunking: 64 bands × 70 k subsegments × 96 B is 430 MB
unchunked."* True, and it is the second-order constraint. §3f does not mention
the S2 air-skip mip, and the mip is what actually decides the batch size.

`TileMaxTop` is rebuilt by reading `conservative_top` across the **whole grid**.
Inside the parallel phase that is a data race against the bands' own writes, so
a rebuild can only happen at a batch boundary. A literal "one `par_bands` per
toolpath" would therefore refresh the mip **never**: it would sit at its
build-time value of "stock top everywhere" for the entire replay and the
whole-stamp early-out would fire zero times — which is precisely wave 2 §2e's
silent no-op, re-created on purpose. On plunge-heavy work that is not a rounding
error: §1 of `DELTA_sim_w2.md` measured the retract half of every
plunge-and-retract cycle as **more than half the stamp budget**, and it is all
whole-stamp skips.

So a batch closes on whichever of these comes first:

| Bound | Value | What it protects |
|---|---:|---|
| `BATCH_VISIT_BUDGET_PASSES` | 4 grid-passes of estimated stamped cell-visits | **mip freshness** — `REFRESH_VISIT_MULTIPLIER` is 1, so the mip is at most 4 passes stale |
| `MAX_PARTIALS_PER_BATCH` | 262 144 `(band, stamp)` partials | memory |
| `MAX_JOBS_PER_BATCH` | 8 192 stamps | memory, when stamps touch few bands |
| `MIN_JOBS_PER_BATCH` | 64 (floor, not a cap) | stops a grid one stamp wide from dispatching per stamp |

**Memory bound, stated.** `size_of::<StampPartial>()` is 80 B and
`size_of::<StampJob>()` is 72 B, so the per-batch working set is
`262 144 × (80 + 4) + 8 192 × (72 + 80) ≈ 23 MB`, **independent of toolpath
length, grid size and cutter diameter**. That is the whole point of batching, and
`partial_memory_bound_holds_at_the_documented_size` pins the arithmetic against
the real `size_of` so a later field addition is a red test rather than a silent
1.5× on a number nobody re-derives.

Staleness is only ever a *speed* cost, never a correctness one, and the argument
matters because it is what lets the dial be tuned at all:
`conservative_top` is monotone decreasing, so an older mip is still an upper
bound and fires the early-out **less** often. A stamp that is not skipped but
whose cells are all inert contributes the identical addend to `pre_volume` and
`post_volume` in the identical order, so its removed volume is exactly `0.0`
either way.

### 2d. CORRECTION — `removed_volume_est_mm3` does NOT reassociate here

The brief allowed this wave the same latitude wave 2 took: *"`removed_volume_est_mm3`
was allowed to reassociate deterministically … if your reduction has the same
property, document it the same way."*

It does not have that property, and the latitude was not used. Wave 2
reassociated the volume sum because banding **split** it. Wave 4 does not split
it again: a job's bucket is filled from exactly the `band_span(row_lo, row_hi)`
range the per-stamp path iterates, and the reduce merges those bands in the same
ascending order. So the reassociation is the one the per-stamp path already
performs, and `whole_path_dispatch_matches_per_stamp_bit_for_bit` compares
`removed_volume_est_mm3` — along with the grid, `conservative_top`, and every
other metric field — as bit patterns with **no tolerance anywhere**, at 1, 2, 4
and 8 threads, on two cutters and two cell sizes.

That is a stronger guarantee than wave 2 shipped, and it is worth having
explicitly: it means the goldens' `1e-3` accumulated-sum tolerance is not being
leaned on by this change at all.

### 2e. CORRECTION — "a band stays on one worker" is not something rayon promises

§3f's second claimed win: *"a band stays on one worker for the whole replay, so
its rows stay in that core's cache instead of migrating every subsegment."*

Rayon offers no affinity guarantee, and `par_chunks_mut` + work stealing
explicitly may move a band between batches. What is actually true — and is
enough — is **intra-task locality**: within one `for_each` body a band walks the
same rows for every stamp in the batch, so the batch's whole working set for
those rows is loaded once instead of once per subsegment. Across batches the
placement is whatever rayon's recursive split happens to give, which is stable
in practice and guaranteed by nothing.

The honest statement of the win is therefore: **one join per batch instead of
one per stamp, and one pass over a band's rows per batch instead of one per
stamp.** The measured effect is §3.

### 2f. Cancellation granularity, stated

Phase 1 polls the cancel token once per subsegment, exactly as before. Phase 2
polls it once per batch, serially, at the top of `run_batch`, and **not** inside
the parallel region: `CancelCheck` carries no `Sync` bound, so reaching it from
a rayon worker means widening a signature the whole crate depends on.

What that bounds is not "once per 70 k subsegments" — a batch closes at 4
grid-passes of estimated stamped cell-visits or 8 192 stamps, so the latency
scales with the *grid*, not the toolpath. On the shipped 0.4 mm wanaka grid it
is tens of milliseconds; on a 4 M-cell grid, the largest this simulator is asked
for, it is ~16 M cell-visits, order half a second. Small fixtures close on the
job cap, which is tighter still. A `Sync` cancel token would let the poll move
inside the band loop; that is a crate-wide change and is not this wave's.

## 3. Numbers — and §3f's own prediction is refuted too

### Read this before reading the tables

`sim_dispatch_ab` is a **paired, same-session, same-process** group: for every
fixture and every thread count, `per_stamp` and `whole_path` run adjacent in one
criterion invocation, with the thread count pinned by a rayon pool per arm
rather than by `RAYON_NUM_THREADS`. The number that matters is the ratio between
two arms of the same run, never an absolute against another day —
`DELTA_sim_w2.md` §1 threw a whole A/B away because an unmodified tree read
18–134 % above its own committed numbers under contention.

Machine state at the run of record: **load average 3.36 rising to 4.39** over the
7-minute run, 22 GB available, `pgrep -x cargo` / `pgrep -x rustc` both clear at
launch, one `rs_cam_gui --mcp` idle and a browser at the usual desktop load.
Comparable to wave 2's 3.8. The cross-check that the window is comparable at all:
`flat12_cs0.1/per_stamp/1` reads **175.5 ms** against wave 2's 165.1 ms for the
same code and fixture — 6 %, inside the noise this file's own rules allow, and
the *shape* of per-stamp's thread curve reproduces exactly (peak at 4–8, worse
at 24).

**One earlier pair is discarded** and should not be quoted: the first run of this
group had `flat12_cs0.1/whole_path/2` at 239 ms with a confidence interval of
[184.6, 311.0] — a 68 % spread on a 10-sample arm — while its neighbours were
tight. Contention, on the same evidence wave 2 used.

### 3a. The A/B, at the run of record

`cargo bench -p rs_cam_core --bench hot_paths -- sim_dispatch_ab`, criterion
mean. `wp/ps` is whole-path against per-stamp **at the same thread count**;
`scaling` is whole-path against its own one-thread arm.

**`flat12_cs0.1`** — the arm wave 2's ceiling was measured on (16 bands per
stamp, 15.7 k bbox cells, so per-stamp *does* dispatch here):

| threads | per_stamp | whole_path | wp/ps | wp scaling | ps scaling |
|---|---:|---:|---:|---:|---:|
| 1 | 175.54 ms | 163.90 ms | 1.07× | 1.00× | 1.00× |
| 2 | 105.31 ms | 93.95 ms | 1.12× | 1.74× | 1.67× |
| 4 | 79.26 ms | 62.81 ms | 1.26× | **2.61×** | 2.21× |
| 8 | 76.39 ms | 52.04 ms | 1.47× | **3.15×** | 2.30× |
| 24 | 101.52 ms | 47.81 ms | **2.12×** | **3.43×** | 1.73× |

**`flat6_cs0.1`** — 4.3 k bbox cells, i.e. **below** `PARALLEL_MIN_BBOX_CELLS`,
so per-stamp dispatch is gated OFF entirely and its row is flat by construction:

| threads | per_stamp | whole_path | wp/ps | wp scaling |
|---|---:|---:|---:|---:|
| 1 | 52.61 ms | 48.60 ms | 1.08× | 1.00× |
| 2 | 57.76 ms | 29.91 ms | 1.93× | 1.62× |
| 4 | 52.64 ms | 22.31 ms | 2.36× | 2.18× |
| 8 | 51.79 ms | 18.78 ms | **2.76×** | 2.59× |
| 24 | 52.16 ms | 19.40 ms | 2.69× | 2.51× |

**`flat6_cs0.25_plunge`** — the plunge-and-retract fixture, 12 bands total:

| threads | per_stamp | whole_path | wp/ps | wp scaling |
|---|---:|---:|---:|---:|
| 1 | 57.56 ms | 52.11 ms | 1.10× | 1.00× |
| 2 | 57.04 ms | 34.57 ms | 1.65× | 1.51× |
| 4 | 57.44 ms | 31.86 ms | **1.80×** | 1.64× |
| 8 | 57.01 ms | 33.98 ms | 1.68× | 1.53× |
| 24 | 57.38 ms | 36.27 ms | 1.58× | 1.44× |

### 3b. Was the 2.10× ceiling beaten? Yes. Was 6–12× reached? No.

The bar the brief set is "beat ~2.10× on the same fixture and thread count".

* **On the same fixture and thread count** (`flat12/cs0.1`, 4 threads):
  per-stamp scales **2.21×** in this session, whole-path **2.61×**.
* The ceiling itself is gone rather than raised. Per-stamp *saturates*: 2.21× at
  4, 2.30× at 8, and then **regresses to 1.73× at 24** — which reproduces wave
  2's finding on a different day. Whole-path is still climbing at 24 threads:
  2.61 → 3.15 → **3.43×**.
* The largest single paired win is **2.76×** (`flat6/cs0.1`, 8 threads), on an
  arm per-stamp is not allowed to dispatch at all.

**6–12× is not reached, and this is the sixth prescription in this review to be
measured wrong.** §3f did not merely say the restructure was the route past
2.10× — it said it was what "would actually reach 6–12×". Measured ceiling:
**3.43×**, at 24 threads on a 24-core box, on the largest-footprint arm in the
suite. The remaining gap is structural and visible in the tables: on
`flat12/cs0.1` a stamp's footprint covers **16 of the grid's 40 bands**, so a
batch of consecutive subsegments can occupy at most 16 workers no matter how
many the pool has. `BAND_ROWS` is the dial that would change that, and it is
deliberately a constant (it fixes the volume-sum reassociation), so moving it
re-baselines a golden. That is a different piece of work, and it is honest to
say the prediction was for a machine-shaped speedup that the band geometry does
not permit.

### 3c. The finding that is worth more than the ratio: the crossover is gone

`DELTA_sim_w2.md` §3d recorded a **crossover** below which the parallel path was
a net loss — 28 % at 4.3 k bbox cells — and set `PARALLEL_MIN_BBOX_CELLS` to
12 000 to gate it off. The `flat6_cs0.1` table above is that regime, and
whole-path dispatch turns a **refusal to parallelise** into **2.76×**.

So the crossover was never a property of the *work size*. It was a property of
the *dispatch granularity*: at 4.3 k cells a stamp is ~50 µs, a rayon join per
stamp is a large fraction of that, and the only fix available per-stamp was to
stop dispatching. Amortised over a batch, the same work parallelises fine. The
same reading explains the plunge arm, which per-stamp also never dispatches
(flat 57 ms at every thread count) and which whole-path takes to 1.80×.

### 3d. One correction found by measurement, not by reading — the fan-out again

The first post-implementation A/B had whole-path **losing** at two threads
(0.92× on `flat12`, 0.98× on `flat6_cs0.1`) while winning everywhere else. The
cause is the exact defect wave 2 §3e found one level down, in a form that is
worse here:

`par_bands(0, rows - 1)` dispatched **every band in the grid**. Rayon splits an
indexed parallel iterator by index range, and a batch's stamps are *consecutive
subsegments*, so its active bands are a **contiguous run** — 16 of 40 on
`flat12`, all of them inside one half of the grid. At two threads the split at
band 20 handed one worker every active band and the other worker twenty empty
ones.

Restricting the dispatch to the batch's own union row span is numerically free
(an excluded band has an empty bucket and contributes nothing) and worth, on the
same fixtures in the following session:

| arm | before | after | |
|---|---:|---:|---:|
| `flat12_cs0.1` @ 2 | 118.37 ms | 93.95 ms | 1.26× |
| `flat6_cs0.1` @ 2 | 56.58 ms | 29.91 ms | **1.89×** |
| `flat6_cs0.25_plunge` @ 2 | 45.53 ms | 34.57 ms | 1.32× |
| `flat6_cs0.25_plunge` @ 8 | 37.63 ms | 33.98 ms | 1.11× |

Recording it because the shape of the mistake is the point: **the same fan-out
error was made twice, at two levels, by two waves, and both times it presented
as "parallelism does not help here" rather than as a bug.**

### 3e. `Auto` takes whole-path at ONE thread too, and that is measured

`Auto` was first written to require `current_num_threads() > 1`. The A/B says
otherwise: at a one-thread pool whole-path is **1.07× / 1.08× / 1.10×** faster
than per-stamp on the three arms, consistently, in both sessions. Per-stamp pays
a rayon bridge per stamp above its bbox cutoff and a fresh band iterator per
stamp below it; batching amortises both whether or not there is anyone to hand
the work to. The condition is gone and the reason is at the constant.

## 4. Correctness

* **All three goldens green and UNCHANGED** (`sim_metrics_match_golden`,
  `sim_metrics_3d_match_golden`, `perf_golden_depth_level_geometry`). No
  re-baseline.
* **`cargo test -p rs_cam_core`: whole package green**, including the wave-3
  `sim_prefix_memo_s5` sentries whose snapshot captures this kernel's outputs.
* `cargo test -p rs_cam_viz -q`: green.
* `cargo clippy --workspace --all-targets -- -D warnings`: zero warnings.
* `cargo fmt --check`: clean for this lane's files.

### 4a. New nets

| Test | What it pins |
|---|---|
| `whole_path_dispatch_matches_per_stamp_bit_for_bit` | grid, `conservative_top` and every sample metric field identical between the two dispatch shapes, **no tolerance**, at 1/2/4/8 threads × 2 cutters × 2 cell sizes |
| `whole_path_dispatch_is_bit_identical_across_thread_counts` | the batch boundaries are a function of the grid, not of `current_num_threads()` — and the *batch count* is asserted equal too |
| `assert_dispatch_is_not_vacuous` (helper, called on every arm above) | > 1 band, > 1 batch, > 1 stamp per batch, and more partials than stamps — i.e. at least one stamp really was split across bands |
| `the_whole_path_fixture_spans_multiple_bands_and_multiple_batches` | the coarsest arm still has margin, so a later shrink of `mixed_pass` is a red test rather than a quiet loss of coverage |
| `band_stamping_is_bit_identical_across_thread_counts` (extended) | now also asserts `Auto` actually selects whole-path on the fixture |
| `partial_memory_bound_holds_at_the_documented_size` | the ~23 MB per-batch bound, computed from the real `size_of`s |
| `auto_declines_a_grid_with_too_few_bands` | `Auto`'s gate, and that an explicit request overrides it |
| `stamps_entirely_outside_the_grid_are_a_clean_skip` | §1 — no panic in debug, zero cells touched, four sides and four corners |
| `out_of_grid_metrics_equal_not_stamping_at_all` | §1 — the metric kernel's four numbers, bit for bit, against never calling it |
| `a_toolpath_that_leaves_the_stock_does_not_panic` | §1 — the §6 reproduction through the public simulator |
| `off_grid_disc_queries_are_empty_not_full_grid` | §1 — the same clamp in `mod.rs`'s three disc queries |
| `the_pre_fix_clamp_really_did_underflow_and_over_walk` | §1 — the pre-fix expressions as an oracle, asserting BOTH defect classes are reached |

**The non-vacuity net is the one that earns its keep here.** A whole-toolpath
dispatch that degenerates to one band, or to one batch, passes every bit-identity
check above *for free* while exercising neither of §3f's two claims. Wave 2 §2e
recorded exactly that failure mode for the mip refresh cadence — nothing failed,
the optimisation was simply inert — so the counters exist and are asserted.

### 4b. Where things landed

| What | Where |
|---|---|
| `StampDispatch`, `StampDispatchStats` | `dexel_stock/whole_path.rs:75`, `:97` |
| Batch bounds (mip freshness, memory, floor) | `whole_path.rs:112,116,129,134,148` |
| `StampJob` / `BandDispatch` | `whole_path.rs:157`, `:184` |
| Phase 2 + 3 (`run_batch`) | `whole_path.rs:341` |
| Bucketing against the grid about to be stamped | `whole_path.rs:269` |
| Phase 1 — the single enumerator | `dexel_stock/simulation.rs:687` (`push_cutting_sample`), driven from `:544` |
| The single metric applicator | `simulation.rs:735` (`apply_subsegment_metrics`) |
| Batch flush + patch | `simulation.rs:801` (`run_batch_into_samples`) |
| §1's shared clamp | `dexel_stock/stamping.rs:461` (`clamped_cell_bbox`) |
| §1's row-span `Option` | `dexel_stock/band.rs:217` (`stamp_row_span`) |
| Paired A/B bench group | `benches/hot_paths.rs`, `sim_dispatch_ab` |

## 5. Files taken outside the stated ownership

* `src/adaptive3d/mod.rs` — **one line in a `#[cfg(test)]` helper.**
  `TriDexelStock` gained two fields, and that file is the only place in the
  workspace that builds one with a struct literal instead of `from_bounds`.
  Nothing else in it was touched.
* `src/dexel_stock/whole_path.rs` — new, inside owned territory.
* `compute/simulate.rs`, `compute/sim_prefix.rs`, `session/` and all of
  `rs_cam_viz` were **not** taken; the S5 lane's wave-3 landing is untouched.

## 6. Deferred, and the obvious next lever

* **The non-metric playback replay is still 100 % serial.**
  `simulate_toolpath_with_lut_cancel` → `stamp_segment_on_grid` has no band
  decomposition at all, and `compute/simulate.rs` runs it against `global_stock`
  for **every** toolpath — i.e. it is a second full replay of the whole project,
  and wave 2's S2 notes say so at the call site. Banding it needs a `GridBand`
  variant of `stamp_segment_on_grid` (the metric kernel cannot be reused: it is
  ~25× the per-cell cost by `perf_suite`'s own measurement). Same three-phase
  shape, and strictly simpler because there are no metrics to reduce — the
  partial is empty and the reduce disappears. This is the largest remaining
  piece of S3 and it is not in this wave.
* **A `Sync` cancel token** would let the poll move inside the band loop
  (§2f). Crate-wide signature change.
* **Batch-to-worker affinity.** §2e — rayon promises none, and nothing here
  asks for any. If the numbers ever justify it, `ThreadPool::install` with a
  fixed band→worker map would make the claim true instead of incidental.
* **`BAND_ROWS` is the remaining parallelism dial, and it is not free.** §3b:
  the 3.43× ceiling is set by a stamp footprint covering 16 of 40 bands, so a
  batch cannot occupy more than 16 workers however large the pool. Halving
  `BAND_ROWS` to 4 doubles the available width — and changes the band
  decomposition, which changes how `pre_volume`/`post_volume` reassociate, which
  moves `removed_volume_est_mm3` in its last bits and **re-baselines a golden**.
  That is a metric-changing fix and belongs with S1, not here.
