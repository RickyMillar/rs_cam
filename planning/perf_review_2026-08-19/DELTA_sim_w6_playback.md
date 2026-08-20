# Wave 6 (SIM) — banding the non-metric playback replay

Captured 2026-08-21 against `DELTA_sim_w4.md` §6 (which named this as the
largest remaining piece of S3) and `DELTA_sim_w5b_landing.md` §9 row W5B-F5.
Every cargo invocation serialized behind `flock /tmp/rs_cam_cargo.lock` with a
≥20 GB available-memory gate; the A/B run held that lock for its whole duration.

**Do not read this file as a replacement for `BASELINES.md`.** It is one lane's
delta.

Commits: `6a04f775` (kernel + dispatch), `4cfd0e07` (sentries), `e3e09c49`
(paired A/B bench arm).

---

## THE HARD CONSTRAINT — bit-identity, and it is stricter here than on the
## metric side

`global_stock` and a metrics-off `group_stock` are both built by this kernel,
and `group_stock` is what `prior_stocks` snapshots — which is what
`StockSource::FromRemainingStock` generation reads. So a last-bit difference in
this grid is a difference in generated G-code, not a difference in a reported
number. The brief allowed no reassociation latitude and none was needed.

**Nothing was re-baselined. No golden moved.**

---

## 1. What landed

New module `crates/rs_cam_core/src/dexel_stock/playback.rs`, plus one new kernel
in `stamping.rs`. Shape is `PlaybackDispatch::{Auto, Serial, Banded}` on
`TriDexelStock`, with a process-wide `RS_CAM_PLAYBACK_DISPATCH` override in the
style of `RS_CAM_STAMP_DISPATCH`, so both shapes are live in one binary and can
be A/B'd in one process. `Auto` resolves to `Banded`.

| What | Where |
|---|---|
| `PlaybackDispatch`, `PlaybackDispatchStats` | `playback.rs:75`, `:132` |
| `RS_CAM_PLAYBACK_DISPATCH` parse | `playback.rs:90` |
| Batch bounds (mip freshness, memory, floor) | `playback.rs:156,159,172,186,203` |
| `PlaybackJob` / `PlaybackBandDispatch` | `playback.rs:212`, `:222` |
| Phase 2 + 3 (`run_batch`) | `playback.rs:344` |
| The band-clipped kernel | `stamping.rs:794` (`stamp_segment_on_band`) |
| `PlaybackPartial` | `stamping.rs:753` |
| Mip fold for the playback path | `tile_mip.rs:177` (`absorb_playback`) |
| The single playback enumerator | `simulation.rs:80` (`replay_moves`) |
| Paired A/B bench group | `benches/hot_paths.rs`, `sim_playback_ab` |

### 1a. The three phases, and the third one really does collapse

1. **Enumerate (serial).** `simulation.rs::replay_moves`. Unchanged move loop,
   unchanged arc linearisation, unchanged cancel poll per move and per arc
   window. It records a `PlaybackJob` instead of stamping.
2. **Replay (parallel).** One `par_bands` per batch over the batch's own union
   row span, zipped with per-band job buckets; each band replays its own jobs in
   job order through `stamp_segment_on_band`.
3. **Reduce (serial).** Nothing that reaches a result passes through it. The
   only thing coming back out of a band is `PlaybackPartial { bbox_cells,
   stamp_skipped }` — the S2 mip's own bookkeeping, which sets refresh cadence
   and diagnostics.

### 1b. "Strictly simpler than S3" — CONFIRMED, and the reason is worth stating

`DELTA_sim_w4.md` §6 predicted this would be "strictly simpler because there are
no metrics to reduce — the partial is empty and the reduce disappears." That
held, and the consequence is a *stronger correctness claim*, not just less code.

`whole_path.rs` has to argue that a job's bands are merged in the same ascending
order the per-stamp path merged them, because `pre_volume` and `post_volume` are
running sums and banding would otherwise reassociate them. `stamp_segment_on_band`
has **no accumulators at all**. It writes to cells; cells are partitioned by the
bands; a band replays its jobs in the original sequence, which is what
`ray_blend_above`'s non-commutativity at `f < 1` requires. There is no
reassociation available to get wrong, so the bit-identity is *structural* rather
than order-dependent.

`PlaybackPartial` is 16 B against `StampPartial`'s 80 B, and both of its channels
are order-independent (a sum of disjoint counts and a boolean `and`).

### 1c. Where the prediction was INCOMPLETE — the reduce does not disappear, the
### mip bookkeeping does not

§6's "the partial is empty and the reduce disappears" is right about the metric
channels and wrong about the S2 mip. The serial kernel calls `charge`,
`note_stamp_skipped` and `note_stamp_run` from inside itself, and those need
`&mut TileMaxTop` — which cannot cross into the parallel region. So a partial and
a per-job fold still exist; they just carry three numbers instead of nine.

That is a small correction, but it is the difference between "delete the reduce"
and "keep the reduce and shrink it", and the second is what the code had to do.

### 1d. CORRECTION — `MIN_JOBS_PER_BATCH` is a UNIT change, not a tuning
### preference

`whole_path.rs` sets it to 64. Copying that number here would have been wrong,
and the reason is that the two dispatchers count different things:

* A **metric** job is one *subsegment* — a `sample_step_mm` slice of a move, and
  a plunge is cut into 250 of them by the `by_z` rule.
* A **playback** job is a whole *move*.

On the shipped raster fixture one playback move is ~6.6 k estimated cell-visits
against a metric subsegment's few hundred. A 64-job floor would hold a batch open
for roughly **20 grid-passes** of stamping, which means
`BATCH_VISIT_BUDGET_PASSES = 4` would never get to close a batch at all — the
mip would sit five times staler than its own dial says it may, on exactly the
plunge-heavy workload where the whole-stamp early-out is worth the most
(`DELTA_sim_w2.md` §1 measured the retract half of a plunge cycle as more than
half the stamp budget). It is 16 here.

Staleness is a **speed** cost and never a correctness one, for the same reason as
wave 4 — `conservative_top` is monotone decreasing, so an older mip is still an
upper bound and fires the early-out *less* often — and in this kernel the second
half of wave 4's argument is not even needed: a stamp that is not skipped but
whose cells are all inert writes nothing at all, because there are no volume
accumulators to keep in step.

### 1e. The fan-out mistake was NOT made a third time

`par_bands` is restricted to the batch's own union row span, not to the whole
grid. `DELTA_sim_w2.md` §3e and `DELTA_sim_w4.md` §3d each found this defect
once, at different levels, and both times it presented as "parallelism does not
help here" rather than as a bug. The condition is the same here — a batch's
stamps are consecutive moves, so its active bands are a **contiguous run**, and
rayon splits an indexed parallel iterator by index range.

### 1f. Out-of-grid clamping — no seventh site

Every cell-bbox computation in `stamp_segment_on_band` goes through
`clamped_cell_bbox` (`stamping.rs:479`), including the degenerate (point) branch,
which is inlined from `stamp_point_on_grid` so the band clip can apply to it.
`a_banded_replay_that_leaves_the_stock_matches_serial` drives five off-grid
positions (four sides plus a corner), each with a lateral pair and a plunge, and
compares against serial.

### 1g. `replay_moves` — two enumerators became one

`simulate_toolpath_with_lut_cancel` and `simulate_toolpath_range_with_lut` were
two hand-maintained copies of the same `match`, and they had already drifted: the
full one polls the cancel token per arc window, the range one had no token at
all. Both now walk `replay_moves`. This is `DELTA_sim_w4.md` §2b's argument
applied to the playback side — two loops that must stay in step by hand cannot be
proved identical by any test, only observed identical today.

### 1h. The per-band fixed cost — found by measurement, and it was a 2.4× LOSS

The first A/B (§2a) had banded dispatch at **0.42×** — a 2.4× loss — at one
thread on `flat6_cs0.5_dense`, the coarse-cell arm where a stamp is few cells.
Reading the diff would not have found it; the cause is a cost that is invisible
on the metric kernel:

`CoverageFastPath::new` is several `sqrt`s plus up to four bounded ULP walks,
each with its own `sqrt`. `stamp_segment_on_grid` pays it **once per stamp**; the
naive banding paid it **once per (band, stamp)**, so a stamp spread over `k`
bands paid it `k` times. Against the metric kernel's ~11 ns/cell inner loop that
is a rounding error, which is why wave 4 could leave it inside the kernel and
even measured a win from doing so. Against a kernel at roughly a **25th** of that
per-cell cost it is the entire crossover.

Its two arguments — `lut.radius_sq()` and `grid.cell_size` — do not move inside a
replay, so the driver now computes it **once per batch** and hands the same value
to every band (`da1aac06`). Bit-identical by construction: same pure function,
same two inputs, and `playback_band_dispatch_s6` is green unchanged across the
change.

**The general lesson, stated because this wave nearly inherited it:** a per-band
fixed cost that is negligible on the metric kernel is not automatically
negligible on a kernel 25× cheaper per cell. Anything hoisted "once per stamp" in
`stamp_segment_with_metrics` deserves re-examination before being copied into a
banded playback path.

### 1i. Cancellation granularity, stated

Phase 1 polls the token once per move and once per linearised arc window, exactly
as before. Phase 2 polls it once per batch, serially, at the top of `run_batch`,
and **not** inside the parallel region: `CancelCheck` carries no `Sync` bound.
Same tail item as `DELTA_sim_w4.md` §2f; this wave did not take it either. What
it bounds is the *grid*, not the toolpath — and because a playback stamp is
roughly a 25th of a metric stamp's per-cell cost, the same budget in cell-visits
is a **shorter** wall-clock latency here than the one wave 4 accepted.

---

## 2. Numbers

### Read this before reading the tables

`sim_playback_ab` is a **paired, same-session, same-process** group: for every
fixture and every thread count, `serial` and `banded` run adjacent in one
criterion invocation, with the thread count pinned by a rayon pool per arm rather
than by `RAYON_NUM_THREADS`. The number that matters is the ratio between two
arms of the same run, never an absolute against another day
(`DELTA_sim_w2.md` §1). The lane held `flock /tmp/rs_cam_cargo.lock` for the whole
duration of each measurement run.

**Both arms are bit-identical**, so — unlike `sim_dispatch_ab`'s `swept` arm —
nothing in these tables can be faster by measuring something else.

Absolute scale, worth stating once: the whole playback replay of the
`flat12_cs0.1` raster is **4.7 ms serial**, against **175 ms** for the metric
simulation of the same fixture (`DELTA_sim_w4.md` §3a). That 37× is the
independent confirmation of `perf_suite`'s ~25× per-cell claim, and it is why the
per-band fixed cost of §1h mattered here and not there.

### 2a. Run 1 (pre-hoist, `6a04f775`) — the run that found the defect

Machine state: **load 2.51 at start, 11.63 at end** — another lane started up
during the run. The first two fixtures are usable; **the last two are
contaminated and their ratios are not quoted as results.** The tell is inside the
data: `flat6_cs0.25_plunge/serial` reads 0.378 ms at 1 thread, 0.904 ms at 4, and
0.394 ms at 24 — a 2.4× excursion on the arm that is *supposed* to be flat.

| fixture | threads | serial | banded | banded/serial |
|---|---:|---:|---:|---:|
| `flat12_cs0.1` | 1 | 4.707 ms | 4.460 ms | 1.06× |
| `flat12_cs0.1` | 4 | 4.678 ms | 2.973 ms | 1.57× |
| `flat12_cs0.1` | 24 | 4.828 ms | 2.705 ms | **1.78×** |
| `flat6_cs0.1` | 1 | 3.634 ms | 3.505 ms | 1.04× |
| `flat6_cs0.1` | 4 | 3.989 ms | 2.531 ms | 1.58× |
| `flat6_cs0.1` | 24 | 3.414 ms | 2.501 ms | 1.37× |
| `flat6_cs0.5_dense` | 1 | 0.500 ms | 1.198 ms | **0.42×** |
| `flat6_cs0.5_dense` | 4 | 0.475 ms | 0.362 ms | 1.31× |
| `flat6_cs0.5_dense` | 24 | 0.472 ms | 0.456 ms | 1.03× |
| `flat6_cs0.25_plunge` | 1 | 0.378 ms | 0.402 ms | 0.94× |
| `flat6_cs0.25_plunge` | 4 | *0.904 ms* | 0.598 ms | *(contended)* |
| `flat6_cs0.25_plunge` | 24 | 0.394 ms | 0.418 ms | 0.94× |

The **0.42×** is the finding, and it is not a contention artefact: it is a
*one-thread* reading, where there is no scheduling to be perturbed, and it is
2.4× — an order out of the range contention explains on the neighbouring arms.
§1h is its diagnosis and `da1aac06` is the fix.

### 2b. Run 2 (post-hoist, `da1aac06`)

Measured 2026-08-21 ~11:15 by the consolidator (the lane itself was killed by
`systemd-oomd` at 10:57 — see §6). Same paired group, same pinned-pool method,
`flock` held for the duration, load 3.2 and falling at launch, no other cargo
job. Ratios are serial/banded on criterion midpoints:

| fixture | threads | serial | banded | banded/serial |
|---|---:|---:|---:|---:|
| `flat12_cs0.1` | 1 | 5.153 ms | 4.975 ms | 1.04× |
| `flat12_cs0.1` | 4 | 4.914 ms | 2.580 ms | 1.90× |
| `flat12_cs0.1` | 24 | 5.087 ms | 2.259 ms | **2.25×** |
| `flat6_cs0.1` | 1 | 3.669 ms | 3.283 ms | 1.12× |
| `flat6_cs0.1` | 4 | 3.462 ms | 2.141 ms | 1.62× |
| `flat6_cs0.1` | 24 | 3.444 ms | 2.137 ms | 1.61× |
| `flat6_cs0.5_dense` | 1 | 494.3 µs | 482.8 µs | **1.02×** |
| `flat6_cs0.5_dense` | 4 | 499.3 µs | 332.9 µs | 1.50× |
| `flat6_cs0.5_dense` | 24 | 498.2 µs | 436.6 µs | 1.14× |
| `flat6_cs0.25_plunge` | 1 | 355.7 µs | 347.6 µs | 1.02× |
| `flat6_cs0.25_plunge` | 4 | 369.0 µs | 296.2 µs | 1.25× |
| `flat6_cs0.25_plunge` | 24 | 369.2 µs | 365.7 µs | 1.01× |

The §1h defect is confirmed fixed *by measurement, on the arm that found it*:
`flat6_cs0.5_dense` at one thread goes **0.42× → 1.02×**. Banding now loses
nowhere — the worst cell in the table is parity — and the fine-cell ceiling
moved 1.78× → 2.25×. The no-size-gate decision in §5 stands on this table.

### 2c. What the shape of the win is, and where the ceiling comes from

Two regimes:

* **Fine cells** (`cs0.1`): the win scales with threads until the serial
  enumerate phase and per-batch bookkeeping dominate — 2.25× at 24 threads on
  the larger grid, 1.61× on the smaller one, which is an Amdahl ceiling, not a
  contention artifact (the serial arm is flat across thread counts, as it
  should be).
* **Coarse cells / plunge**: the absolute cost is 350–500 µs and a stamp is a
  handful of cells, so the parallel section is thin; the win is 1.0–1.5× and
  saturates immediately. Nothing to chase here — the numbers are microseconds.

**The honest framing of the whole wave:** the intake called this "the largest
remaining perf lever", and in *cell-visit* terms it was the largest untouched
serial loop. In *wall-clock* terms the playback replay was already cheap —
4.7 ms serial on the fixture whose metric simulation costs 175 ms (§2's 37×) —
so banding it buys ~2 ms per replay on bench fixtures. Its end-to-end weight on
a wanaka-class project (huge grid, 509 k moves, replays per toolpath per
fixpoint round) is plausibly larger but is **not measured here**, and nobody
should quote this wave as an end-to-end speed-up until a paired wanaka A/B says
so. The structural wins that are real regardless: the replay no longer
serializes a 24-core box, the two hand-copied enumerators are now one
(§1g — the range copy had drifted on cancellation), and the per-band fixed-cost
lesson (§1h) is recorded before someone copies a heavier hoist into a cheaper
kernel.

---

## 3. Correctness

### 3a. The nets

All in `crates/rs_cam_core/tests/playback_band_dispatch_s6.rs` unless noted.
Fixtures: a 2.5D multi-op job (raster clear at three depths, a contour with arcs
and a lead-in that leaves the stock, 24 drill plunges), a 3D drape (helical
entry, a ramp on every one of 144 segments), a four-toolpath cascade into ONE
stock at 0.2 mm, and a `FromBack` side-grid arm. Thread counts 1, 4 and the box's
`available_parallelism`.

| Test | What it pins |
|---|---|
| `playback_banded_matches_serial_bit_for_bit` | every dexel span and `conservative_top` on every live grid, plus the remaining-stock probes, **no tolerance**, at 1/4/max threads on all three fixtures |
| `playback_banded_is_bit_identical_across_thread_counts` | the band decomposition and batch boundaries are functions of the grid, not of `current_num_threads()`; batch **and** band counts asserted equal too |
| `playback_banded_is_deterministic_run_to_run` | four banded runs at max threads, same fingerprint — the check that would catch a race inside one pool configuration, which the thread-count sweep would not |
| `auto_selects_banded_and_agrees_with_both_explicit_arms` | the shipped default is the banded path, and it agrees with both forced arms |
| `playback_banded_matches_serial_on_a_side_grid` | `FromBack`, i.e. the lazily-created Y grid, whose rows are a different axis |
| `a_banded_replay_that_leaves_the_stock_matches_serial` | `DELTA_sim_w3.md` §6's shape through the banded path — five off-grid positions, each with a lateral pair and a plunge |
| `the_playback_fixtures_span_multiple_bands_and_batches` | the coarsest arm keeps margin, so a later fixture shrink is a red test |
| `playback_batch_memory_bound_holds_at_the_documented_size` (unit) | the ~6 MB per-batch bound against the real `size_of`s |
| `a_grid_with_too_few_bands_declines_banded_playback`, `auto_resolves_to_banded` (unit) | the band floor and the single `Auto` mapping |

### 3b. What "bit-identical" is checked ON, and why the probes are there

Two fingerprints, both tolerance-free:

1. **The grid.** `enter`/`exit` bit patterns of every dexel span on every ray, and
   `conservative_top` alongside, on **every live grid** — a side-cut direction
   creates the Y or X grid lazily and that is the one the stamps landed in, so
   checking only `z_grid` would pass vacuously on the `FromBack` arm.
2. **The downstream-geometry queries.** `max_top_z_in_disc`,
   `max_conservative_top_z_in_disc` and `local_material_sum` over a 300-point
   lattice at two radii. This is not a proxy for the risk — it is the risk:
   `FromRemainingStock` generation does not read the grid, it asks the stock what
   material is left near a point, and these are those questions.

Both halves refuse to pass vacuously: the grid check asserts the fixture left
dexel spans at all, and the probe check asserts **> 20 probes see material below
the stock top**, so a lattice sampling untouched blank is a red test rather than
a free pass.

### 3c. Non-vacuity, including one counter that cannot be faked

`assert_playback_dispatch_is_not_vacuous` requires, on every arm: `bands > 1`,
`batches > 1`, `max_jobs_in_a_batch > 1`, `max_partials_in_a_batch >
max_jobs_in_a_batch` (i.e. at least one stamp really was split across bands), and
**`band_tasks_run > batches`**.

The last one is the point. The other four are recorded on the serial side and
would keep counting if the `par_bands` call itself were reduced to nothing — an
empty dispatch range, a truncating `zip`, a slice that came back `None`.
`band_tasks_run` is incremented by the closure that does the stamping, from
inside the parallel region. A stale gate that fires zero times is silently sound;
that is this programme's most-repeated trap and it has a counter now.

### 3d. Mutation-checked

The bit-identity claim rests entirely on `ray_blend_above` being replayed per
cell in the original stamp order. Reversing the per-band job iteration
(`bucket.iter()` → `.rev()`) turns **four of the seven** sentries red, on the
first ray span of the 2.5D fixture at **one thread**. Reverted immediately; the
check is recorded because a sentry whose failure mode has never been observed is
a sentry whose teeth are hypothetical.

### 3e. Gates

Run by the consolidator 2026-08-21 post-OOM (the lane was killed before this
section — §6), all under `flock`, all at `-j 8`:

| Gate | Result |
|---|---|
| `cargo test -p rs_cam_core --release --no-fail-fast` | **3129 passed, 1 failed** — the failure is the known C9 wall-clock flake (below) |
| `cargo test -p rs_cam_viz --release` | clean (incl. the 276-test target) |
| `cargo test -p rs_cam_cli --release` | clean |
| `cargo test -p rs_cam_mcp --release` | clean (13 tests) |
| `playback_band_dispatch_s6` | **7/7** |
| `cargo clippy --workspace --all-targets -- -D warnings` | zero warnings |
| `cargo fmt --check` | clean after `1a0a4ee0` (pre-existing drift in six G1/G9-era files — none of them this wave's) |

The one failure, `wanaka_scale_indexed_path_beats_linear_scan_and_matches_output`
(`remap_interval_index_c9.rs:539`), read 4.66× against its ≥5× bar, then
**5.3× on immediate re-run, passing** — and the movement between the two runs
was entirely in the *linear* arm (158.9 → 190.5 ms) while the indexed arm held
at ~35 ms. The bar has effectively zero margin and its noise is in the
reference arm; this is the third session to record the same flake. Candidate
fix (a user decision, not taken here): lower the bar to 4× or take the median
of 3 reps.

## 6. How this wave ended — the lane was OOM-killed, the work was not lost

At 10:57:39 on 2026-08-21 `systemd-oomd` killed the terminal scope this lane
(and the whole orchestrating session) ran in — user-slice memory pressure held
above 50% for 20 s during the full-suite release build, stacked on ambient
desktop apps. The machine itself did not reboot. All four code commits and this
document's §1/§3 content were already on disk; the consolidator re-verified the
sentries, ran §2b and §3e, and filled in the TBDs. Process change adopted:
heavy cargo now runs with `-j 8` everywhere, and the ≥20 GB gate fails closed
(it had a shell-portability hole — `free -g` prints `31Gi` on this box and the
unquoted integer compare errored).

---

## 4. W5B-F5 — the retract measurement, and the answer is **0.0 %**

The brief asked, explicitly as evidence for a later decision and explicitly
without changing anything: *what fraction of playback stamp time is retract
moves?*

**Zero. No shipped generator emits a `MoveIntent::Retract` move that the playback
kernel stamps at all.**

### 4a. Method, stated so the claim can be checked or overturned

`Move::intent` can only be set through `Toolpath::{feed_to, rapid_to, arc_cw_to,
arc_ccw_to}_with_intent` or a `Move { .. }` struct literal. A script parsed every
one of those call sites in `crates/rs_cam_core/src` (production halves only —
everything from the file's `#[cfg(test)]` marker onward was excluded) and read the
`MoveIntent::` argument out of each balanced call:

| call | intent | count |
|---|---|---:|
| `rapid_to_with_intent` | `Retract` | **26** |
| `feed_to_with_intent` | `Retract` | **0** |
| `arc_cw_to_with_intent` / `arc_ccw_to_with_intent` | `Retract` | **0** |

The five `feed_to_with_intent` sites whose intent is a *variable* are all
`body_intent` on `Toolpath::emit_*_with_intent`, and every caller across
`rs_cam_core` and `rs_cam_viz` passes `ClearingCut`, `FinishingCut`, `Linking`,
`EntryPlunge` or `Unknown` — never `Retract`. The only `Move { intent:
MoveIntent::Retract }` literal in the workspace is in
`measurement.rs`'s test module. `rs_cam_viz` and `rs_cam_cli` set no intents at
all, and nothing anywhere re-types a `Rapid` into a `Linear` while preserving its
intent (`with_feed_rate` is the only `move_type` rewrite, in `feed_modulation.rs`
and `feedopt.rs`).

**Static, and I am labelling it static.** I did not run a dynamic census across
the 56-family sweep. The census is exhaustive over the *only* ways an intent can
be attached, which is why I am willing to state it as a number rather than as an
impression — but a generator added tomorrow could falsify it, and there is no
sentry pinning it.

### 4b. What that means for W5B-F5, and it is not what the row implies

The row reads: *"the non-metric replay stamps `MoveType::Linear` regardless of
intent, so the metric grid and the playback grid disagree about retracts."* The
code says exactly that and it is a real latent divergence. But the input that
would trigger it is **not produced by anything that ships**:

* Every production `Retract` is on a **rapid**, and the playback loop has always
  skipped rapids (`MoveType::Rapid => {}`).
* So the divergence is currently unreachable from generation, the retract share
  of playback stamp time is 0.0 %, and **implementing W5B-F5 today would change
  no grid and save no time.**

### 4c. The corollary, which is the more interesting half

The *metric* path's `is_retract_feed` branch (`simulation.rs`, the
`MoveType::Linear { feed_rate } if is_retract_feed` arm) is reached by the same
input class — and by the same census it is **also unreachable from shipped
generators today**. `CLAUDE.md` describes it as live: *"Plunge-and-retract-loop
ops (project_curve, v_carve, drill) no longer inflate either reading from retract
feeds — retracts are now tagged `MoveIntent::Retract` and excluded from cutting
metrics (Step 1, 2026-05-19)."* `drill.rs`'s retracts are
`rapid_to_with_intent(Retract)`, and rapids were already excluded from cutting
metrics by being rapids.

So either the tagging moved from feed to rapid after Step 1 landed, or the
exclusion was never the thing doing the work. **This is a claim in `CLAUDE.md`
that the code no longer supports**, and it is not this lane's to fix — flagged
here because the same census that answers the brief's question happens to answer
this one, and because it is exactly the "instrument integrity" failure mode the
repo's own notes warn about: a docstring that describes a mechanism which stopped
firing.

---

## 5. Deferred

* **A `Sync` cancel token** would let the poll move inside the band loop (§1i).
  Crate-wide signature change; wave 4 declined it and so does this one. It is now
  wanted by two dispatchers instead of one.
* **A minimum-size gate is NOT shipped, and §2c says why.** Both paths are
  bit-identical, so a size gate here would be *purely* a speed dial and could
  never select a different measure — which is the opposite of the `MIN_BANDS`
  situation in the S1 flip, where the gate chose between two kernels that
  measured different things. It is not shipped because after the
  `CoverageFastPath` hoist the measured loss regime no longer justifies one; if
  it comes back, `pending_visits / pending_partials` (estimated cells per
  `(band, stamp)` partial) is the quantity to gate on and both counters already
  exist in the driver.
* **The mip's whole-stamp query still runs once per (band, stamp).** It could be
  hoisted into `build_buckets` — one query per job, serially, with skipped jobs
  simply not bucketed — which would also let the band kernel drop its
  `Option<&TileMaxTop>` for a plain `air_skip: bool`. Not taken: it duplicates the
  global-bbox and tip arithmetic on the driver side, and a divergence between the
  driver's copy and the kernel's is a correctness risk in exchange for a cost
  measured in tile reads.
* **W5B-F5 itself** — see §4. It is a smaller item than the row suggests, and
  the useful move now is a sentry pinning "no generator emits a Retract-tagged
  Linear move", not the skip.
