# SIM wave 5 — S1 swept-volume stamping: **decision package**

Branch `perf/s1-swept-volume`, off `tech-debt-3` at `c604c096`. **Nothing here is
landed on `tech-debt-3`.** S1 is metric-changing by design and the landing
decision is the user's; this document is the evidence for it.

The default is unchanged on this branch: `StampDispatch::Auto` still resolves to
`WholeToolpath`, `Auto` never selects a swept shape, and the `rs_cam_core`,
`rs_cam_viz`, `rs_cam_mcp` and `rs_cam_cli` suites are green in the default mode
(one pre-existing wall-clock flake aside — §6), with clippy at zero warnings.

---

## 0. What was actually built

Two new dispatch shapes, both opt-in, both behind the existing
`StampDispatch` enum (`crates/rs_cam_core/src/dexel_stock/whole_path.rs:83-134`):

| Mode | What it does | Metric-neutral? |
|---|---|---|
| `StampDispatch::Swept` | One stadium pass per **chunk of consecutive subsegments**; per-cell contributions smeared over the bins the cell's own footprint spans. | **No — by design.** |
| `StampDispatch::SweptPlungeOnly` | The same machinery, but a chunk may only grow where growing it is provably free: an exactly-vertical run, or a single bin. | **Yes — bit-identical, sentried.** |

New module: `crates/rs_cam_core/src/dexel_stock/swept.rs` (~950 lines incl. docs
and unit tests). Driver hook: `capture_cutting_segment_swept` /
`flush_swept_job` in `crates/rs_cam_core/src/dexel_stock/simulation.rs`.

**The old paths are untouched.** The swept scheduler is a *separate* `Option`
alongside the existing `Option<BandDispatch>`
(`simulation.rs`, `let mut swept = match self.stamp_dispatch { … }`), not a third
arm of the existing one, and `BandDispatch::for_grid` returns `None` for both
swept modes. So the A/B is same-binary, same-session, and "old mode unchanged"
is checkable by reading the diff rather than by trusting a claim.

A process-wide test hook, `RS_CAM_STAMP_DISPATCH`
(`auto` | `per_stamp` | `whole_path` | `swept` | `swept_plunge`), is parsed once
in a `OnceLock` and consulted only by `StampDispatch::default()`. Unset — the
shipped case — is `Auto`, unchanged. It exists so the whole suite, the goldens
and the wanaka harness can be forced onto one shape.

### Structural properties, all sentried

* **Deterministic across thread counts.** 1/2/4/8 threads, two cutters, two cell
  sizes: grid, `conservative_top` and every per-sample metric bit-identical, and
  the batch count itself does not move
  (`swept_dispatch_is_bit_identical_across_thread_counts`). The chunk
  decomposition is a function of the move's subdivision, the tool radius and the
  cell size — never of `current_num_threads()`.
* **Memory bounded.** Same caps as wave 4 (`MAX_PARTIALS_PER_BATCH = 262 144`,
  `MAX_JOBS_PER_BATCH = 8 192`), bounding the same product: a chunk of `k` bins
  touching `m` bands costs `k · m` partials, exactly as `k` separate stamps
  touching `m` bands each would. Swept dispatch does **not** enlarge the working
  set; it enlarges the unit of scheduling. Pinned by
  `swept_partial_memory_bound_holds` (≤ 26 MB against real `size_of`).
* **Cancellation latency comparable.** Identical contract to wave 4: the token is
  polled once per batch serially plus once per subsegment in the enumerator;
  batches close on the same visit budget. Not polled inside the parallel phase
  for the same reason (`CancelCheck` is not `Sync`).
* **Non-vacuous.** `swept_dispatch_is_not_vacuous` requires >1 band, >1 batch,
  >1 chunk per batch, and chunks spanning more than one band.

---

## 1. PERF — paired same-session A/B

`cargo bench -p rs_cam_core --bench hot_paths -- sim_dispatch_ab`. All four modes
sit adjacent in **one criterion session** per fixture and thread count; the
number that matters is the ratio, not either absolute (`BASELINES.md` cross-day
rule). Thread counts pinned with a rayon pool per arm, in-process.

Machine state: load average 1.62 at the start of the session, 4.87 at the end;
no cargo jobs alongside; 24 logical cores, 54 GiB RAM with > 15 GiB free
throughout. Absolute numbers sit ~9 % above an earlier session's on the same
tree (`per_stamp / flat12 / 1t` read 159.6 ms then and 173.8 ms here) — the
documented cross-day drift. **Only the ratios below are load-bearing.**

### Wall clock (median), ms

| fixture | threads | per_stamp | whole_path | swept_plunge | swept |
|---|---:|---:|---:|---:|---:|
| `flat12_cs0.1` | 1 | 173.82 | 161.06 | 112.88 | 39.93 |
| `flat12_cs0.1` | 2 | 110.86 | 100.26 | 69.86 | 26.66 |
| `flat12_cs0.1` | 4 | 92.76 | 71.64 | 52.41 | 17.22 |
| `flat12_cs0.1` | 8 | 92.97 | 61.31 | 43.51 | 13.75 |
| `flat12_cs0.1` | 24 | 135.37 | 57.26 | 41.63 | 12.30 |
| `flat6_cs0.1` | 1 | 55.08 | 50.75 | 36.65 | 15.42 |
| `flat6_cs0.1` | 2 | 54.65 | 34.23 | 23.97 | 11.97 |
| `flat6_cs0.1` | 4 | 58.82 | 26.10 | 18.00 | 7.82 |
| `flat6_cs0.1` | 8 | 58.07 | 22.74 | 15.76 | 6.73 |
| `flat6_cs0.1` | 24 | 58.61 | 23.88 | 16.64 | 6.51 |
| `flat6_cs0.25_plunge` | 1 | 65.05 | 60.26 | 17.75 | 17.36 |
| `flat6_cs0.25_plunge` | 2 | 66.08 | 42.59 | 11.02 | 9.40 |
| `flat6_cs0.25_plunge` | 4 | 56.84 | 34.11 | 7.02 | 7.07 |
| `flat6_cs0.25_plunge` | 8 | 56.91 | 33.70 | 6.85 | 6.94 |
| `flat6_cs0.25_plunge` | 24 | 57.11 | 36.18 | 7.22 | 7.08 |

### Ratios against `whole_path` (the shipped `Auto` resolution)

| fixture | threads | swept_plunge / whole_path | **swept / whole_path** | swept / per_stamp |
|---|---:|---:|---:|---:|
| `flat12_cs0.1` | 1 | 1.43x | **4.03x** | 4.35x |
| `flat12_cs0.1` | 2 | 1.44x | **3.76x** | 4.16x |
| `flat12_cs0.1` | 4 | 1.37x | **4.16x** | 5.39x |
| `flat12_cs0.1` | 8 | 1.41x | **4.46x** | 6.76x |
| `flat12_cs0.1` | 24 | 1.38x | **4.66x** | 11.01x |
| `flat6_cs0.1` | 1 | 1.38x | **3.29x** | 3.57x |
| `flat6_cs0.1` | 2 | 1.43x | **2.86x** | 4.57x |
| `flat6_cs0.1` | 4 | 1.45x | **3.34x** | 7.52x |
| `flat6_cs0.1` | 8 | 1.44x | **3.38x** | 8.63x |
| `flat6_cs0.1` | 24 | 1.44x | **3.67x** | 9.00x |
| `flat6_cs0.25_plunge` | 1 | 3.39x | **3.47x** | 3.75x |
| `flat6_cs0.25_plunge` | 2 | 3.86x | **4.53x** | 7.03x |
| `flat6_cs0.25_plunge` | 4 | 4.86x | **4.82x** | 8.04x |
| `flat6_cs0.25_plunge` | 8 | 4.92x | **4.86x** | 8.21x |
| `flat6_cs0.25_plunge` | 24 | 5.01x | **5.11x** | 8.07x |

Read the `swept_plunge` column carefully: it is **bit-identical to
`whole_path`** and still 1.37–1.45× faster on the raster fixtures and
3.4–5.0× on the plunge fixture. The raster gain is not a lateral-stamping
gain — `raster_pass` plunges into each pass, and those entries are what the
hoist collects. On a hypothetical toolpath with no vertical run at all it
would be a wash.

That column was **0.76–0.95× — a LOSS — before two fixes**, and the sequence is
worth recording because both were self-inflicted by the new code, not inherited:
(a) the per-bin midpoint and depth-bound tables were two `Vec` allocations per
(band, chunk), i.e. two allocations per stamp, against a kernel wave 4 had
already stripped of per-stamp allocation — now computed inline; (b) the smearing
weight cost a **division per cell**, which at one bin is always exactly `1.0` —
now short-circuited. Neither moved a bit; the bit-identity sentry was re-run
after each.

### Cell-visit counts

Not separately instrumented; the arithmetic is exact and does not need a
counter. Per move of length `L` subdivided into `n = L/s` subsegments, the
shipped kernel visits `n · (s + 2ρ)(2ρ)/cs²` cells where `ρ = R + 0.53·cs`; a
swept chunk visits its own bounding box once. For the `flat12/cs0.1` fixture
(Ø12, `s = 0.25`, axis-aligned 40 mm moves) that is a **≈ 49×** reduction in
cell visits against a measured **≈ 4.2×** in wall clock — the gap is the
per-cell cost that does not scale (the ray walks and the read-modify-write
survive; only the coverage evaluation, the LUT probe and the loop overhead are
amortised), plus the `SWEPT_MAX_BBOX_WASTE` chunker declining to grow chunks
whose bounding box outruns their stadium.

**This is the S1 finding's own arithmetic being over-optimistic in the same way
S3's was.** "≈ 26×, scales as `2R/s`" is the cell-visit ratio, and the review
quotes it as if it were the time ratio. It is not: measured **2.9–5.1×**.

### What does NOT scale: the end-to-end workload

See §4. On wanaka200 the *simulate* phase moves 14.8–16.0 % and the whole run
8.0 %. The stamp kernel is not the majority of a real `run_simulation` — S4
(per-toolpath full-grid clones + marching cubes) and S6 (trace clone + JSON) are
still in front of it. **A 4× kernel does not buy a 4× simulation.**

---

## 2. GOLDEN DIFF

`RS_CAM_STAMP_DISPATCH=swept cargo test -p rs_cam_core --release --test perf_golden_sim_metrics`.
The goldens were **not** regenerated. Both arms fail: 39 fields on the 2.5D
golden, 29 on the 3D one.

### The single most important line in this document

**Exactly one exact-compared field moved: `triage_action_count`, 3 → 1.**

Every other field the golden compares with **no tolerance** is unchanged:
`project_collision_count`, `project_rapid_collision_count`,
`holder_collision_total`, `triage_safety_count`, `total_sample_count`,
`resolution_mm`, `toolpath_count`, and per toolpath `name`, `op_kind`,
`sample_count`, `move_count`, `collision_count`, `rapid_collision_count`,
`metrics_not_applicable`, plus every per-kinematics `sample_count`. The sample
stream and every collision channel are bit-stable across the change. What moved
is exclusively *measured* quantities.

`triage_action_count` fell because two air-cut actions dropped below their
threshold — a consequence of (ii) below, not an independent movement.

### Classification of the 67 moved fields

Every moved field falls into one of three groups, and **none is unexplained**.

| # | Class | Fields | Cause |
|---|---|---|---|
| **(ii-a)** | Genuinely more accurate | `peak_axial_doc_mm` and its per-kinematics twins | Measured axial DOC now reaches the **commanded** DOC. Pocket 2.0625 → **3.0000**, Profile 2.6367 → **3.0000**, against a fixture `depth_per_pass` of exactly **3.0**. |
| **(ii-b)** | Grid-quantisation artifact removed | all `air_cut_*`, `average_engagement`, `low_engagement_time_s`, all `average_radial_woc_fraction` / `average_arc_radians` | The shipped air-cut reading is a function of the **simulation cell size**, not of the toolpath. |
| **(i)** | Sampling-density artifact removed | `total_removed_volume_est_mm3`, `average_mrr_mm3_s` (−0.15 % to −7.0 %) | The shipped kernel `f`-blends a partially covered cell once per subsegment, leaving `(1−f)^N`; it over-removes in proportion to sample density. |

Each classification is backed by a **falsifiable test comparing slopes**, not by
an argument that the new number looks better. All three are in
`crates/rs_cam_core/tests/swept_stamping_s1.rs` and all three pass:

**(ii-a) `swept_peak_axial_doc_reaches_the_commanded_depth_and_per_stamp_does_not`**
— one flat pass at a commanded 3.0 mm DOC into fresh stock:

| cell size | commanded | per-stamp | swept |
|---|---|---|---|
| 0.5 mm | 3.000 | **1.5820** | 3.0000 |
| 1.0 mm | 3.000 | **0.8926** | 3.0000 |

The shipped kernel under-reads a commanded 3 mm cut by up to **70 %**, and the
under-read grows with cell size. This is F-024's own property — measured axial
should equal commanded axial — and the shipped kernel does not have it. Cause:
a partially covered cell's removal is split across the subsegments that blend
it, so the per-subsegment maximum is a fraction of the column's real removal.

**(ii-b) `per_stamp_air_cut_pct_grows_with_cell_size_and_swept_does_not`** —
same toolpath, four cell sizes:

| cell size | per-stamp air % | swept air % |
|---|---|---|
| 0.25 mm | 0.31 | 5.95 |
| 0.50 mm | 33.66 | 7.84 |
| 1.00 mm | **89.61** | 11.04 |
| 2.00 mm | 95.57 | 95.57 |

The shipped air-cut criterion is `radial_woc_fraction < 0.02`
(`simulation_cut.rs:1195`) — *not* "removed nothing". At
`cell_size > sample_step` the first subsegment over a cell column takes all the
fresh material, and the rest find no cell with
`pre_fresh > FRESH_MATERIAL_THRESHOLD_MM` at all, so `perp_max` never exceeds
`perp_min`, radial engagement is exactly `0.0`, and the sample is booked as air.
The Phase 0 2.5D golden runs `cell = 1.0 mm` against `sample_step = 0.25 mm`, a
4:1 ratio — which is why its Pocket arm reports 60 % air on a pass that is
cutting throughout.

The `cs = 2.0` row is pinned deliberately and is **not** a swept win: above the
lateral-resolution limit `PERP_COVERAGE_GATE` admits too few cells for a width
to exist and both dispatches read the same 95.57 %. Swept does not rescue that
case, and it must not be sold as if it does — `sim_measurability`'s abstention
is what covers it.

**(i) `swept_removal_is_sample_density_independent_and_per_stamp_is_not`** —
same toolpath, `sample_step` 0.5 → 0.1 (a 5× change):

| cell size | per-stamp spread | swept spread |
|---|---|---|
| 0.2 mm | 0.34 % | **0.00 %** |
| 0.5 mm | 1.06 % | **0.00 %** |

Removed volume is a property of the stock and the cutter. The shipped kernel's
answer depends on how finely the driver chose to sample; swept's does not.

### (iii) unexplained: **none**

One movement was initially unexplained and was chased to a cause before being
written down: the air-cut collapse was first attributed to removal quantisation,
a hypothesis the test **falsified** (per-stamp air read 0 % at every cell size
under a "removed nothing" proxy). The real criterion is the engagement one, and
the corrected test reproduces the golden's behaviour exactly. That reversal is
recorded because it is the only reason the classification above is trustworthy.

---

## 3. SENTRY INVENTORY

Full `cargo test -p rs_cam_core --release --no-fail-fast`, mode forced by
`RS_CAM_STAMP_DISPATCH`. 185 test targets.

### `swept_plunge` — **fully green**

The only failure is `wanaka_scale_indexed_path_beats_linear_scan_and_matches_output`
(`tests/remap_interval_index_c9.rs`), a **wall-clock** assertion (≥ 5× speedup,
got 4.14×) with nothing to do with stamping. Verified as a pre-existing flake:
it also fails in the **default** mode on a warm machine (4.52×). Discounted.

So: **`SweptPlungeOnly` passes the entire regression net, including both perf
goldens, both `_litmatrix_` suites and every F-XXX sentry, with no
re-baselining of anything.**

### `swept` — 7 genuine failures, in three groups

| Test | File | Old → new | Is the assertion about a quantity S1 redefines? |
|---|---|---|---|
| `sim_metrics_match_golden` | `perf_golden_sim_metrics.rs` | 39 fields | **Yes.** §2. |
| `sim_metrics_3d_match_golden` | `perf_golden_sim_metrics.rs` | 29 fields | **Yes.** §2. |
| `as013_terrain_model_edge_axial_within_commanded_dpp_f027` | `adaptive3d_planner_stock_xy_f027.rs` | max axial 3.694 vs bar 3.500 (dpp 3.0 + 0.5 margin); **3 of 54 477** samples in the band | **Yes** — `axial_engagement_mm`. See below. |
| `as013_terrain_model_edge_band_outlier_count_zero_f027` | same | outliers 0 → **3** | **Yes**, same quantity. |
| `as013_terrain_whole_toolpath_axial_within_commanded_dpp_f031` | `adaptive3d_interior_cell_parity_f029.rs` | max axial 3.890 vs bar 3.500; **10 of 630 865** steady-state samples | **Yes**, same quantity. |
| `planner_sim_dexel_parity_agent_search` | `src/adaptive3d/mod.rs` (unit) | see below | **No — and this one is a surprise.** |
| `planner_sim_dexel_parity_contour_parallel` | same | see below | Same. |

#### The three F-XXX axial sentries

These are the direct cost of (ii-a). They assert
`axial_engagement_mm ≤ commanded depth_per_pass + 0.5 mm`. Under the shipped
kernel a cell's removal is split across the subsegments that blend it, so the
per-sample maximum sits *below* the column's real removal and the bar is
comfortably met. Under swept a cell is removed once and reports the whole
column, which on terrain can legitimately exceed `dpp` — a column at a model
edge or a steep face can hold more than one pass depth of material above the
tool surface, and the previous pass may have left more than `dpp` there.

**The toolpaths are unchanged** in these fixtures; only the measurement moved.
The exceedances are small (5.5 % and 11 % over a bar that already carries a
0.5 mm margin) and rare (3 in 54 k, 10 in 631 k).

**But state the cost plainly:** F-027 and F-031 exist to catch *real* over-cut,
and re-baselining them by widening the margin weakens a safety-adjacent net in
exchange for a measurement change. That is a decision, not a formality. The
honest alternative is to re-express those sentries against a quantity that is
still bounded by `dpp` under both kernels — the tool's *commanded* Z travel per
pass — rather than against the measured column removal, which under swept is
correctly allowed to exceed it.

#### The planner/simulator parity pair — swept **improves** agreement and still fails

| fixture | metric | default | swept |
|---|---|---|---|
| AgentSearch | cells differing > 0.53 mm | 2240 / 7921 | **1399 / 7921** |
| AgentSearch | **interior** cells differing | 704 / 5184 | **21 / 5184** |
| AgentSearch | `planner_higher` (sim removed more) | 880 | **39** |
| AgentSearch | `sim_higher` (planner removed more) | 1360 | 1360 |
| ContourParallel | cells differing > 0.53 mm | 1309 / 7921 | **576 / 7921** |
| ContourParallel | **interior** cells differing | 647 / 5184 | **8 / 5184** |
| ContourParallel | `planner_higher` | 741 | **8** |
| ContourParallel | `sim_higher` | 568 | 568 |

Interior disagreement falls **33×** and **81×**. The test fails anyway, because
it does not assert *magnitude* — it asserts **balance**:

> divergence is 71.00× lopsided (bar 2.50×) … Symmetric discretisation noise is
> balanced; a systematic skew means one side is applying a transformation the
> other is not.

Swept deletes one whole side of the disagreement. `planner_higher` — the cases
where the *simulator removed more than the planner claims* — collapses from
880/741 to 39/8, which is precisely the boundary-cell over-removal (i) predicts.
What survives, unchanged at 1360/568, is the planner claiming more removal than
its emitted path delivers: a **pre-existing, separate, systematic divergence
that S1 does not touch and that the balance bar was masking.**

This is the most interesting result in the wave. The bar's own reasoning is
sound and its conclusion is inverted here: removing the symmetric noise is what
makes the asymmetry visible. Re-baselining it is not "loosening a sentry", it is
un-masking a finding that should get its own item.

### Nothing else moved

No `_litmatrix_*` failure. No other F-XXX failure. No param-sweep fingerprint
failure. No `rs_cam_viz` failure (default mode; see §6).

---

## 4. WANAKA200 METRICS DIFF

Headless, `crates/rs_cam_core/tests/swept_wanaka_ab_s1.rs`, at **0.4 mm**, F.4
ladder driven by hand (the CLI has no fixpoint loop and would silently generate
4 of 8 toolpaths). Two adjacent processes on one machine, `Auto` (= the shipped
resolution, `WholeToolpath`) then `Swept`.

Machine state: load average 3.34 at the start of each run, 15.6 at the end (each
run's own rayon pool). No cargo jobs alongside. Wall clocks below carry that
caveat; the metric rows do not.

| Metric | 0C / old (`auto`) | new (`swept`) | Δ | Classification |
|---|---|---|---|---|
| **`rapid_collision_count`, project** | **0** | **0** | **0** | **No change — not a blocker** |
| **`rapid_collision_count`, every toolpath** | **0** | **0** | **0** | **No change** |
| **`collision_count`, project + every toolpath** | **0** | **0** | **0** | **No change** |
| `resolution_clamped` | 0 | 0 | 0 | No change |
| Verdict | `NOT MEASURED: air-cut % withheld for 3 toolpaths` | `WARNING: high air cutting … 51 % / 36 % / 56 %` | **changed kind** | (ii-b) — the metric became measurable |
| Measurability `not_measurable` rows | **9** (tp5, tp8, tp9 × 3) | **0** | −9 | (ii-b) |
| Measurability `degraded` rows | 3 (tp1) | 12 | +9 | (ii-b) |
| tp1 blind fraction | 37 % | 15 % | −22 pp | (ii-b) |
| tp5 blind fraction | 71 % (not measurable) | 27 % (degraded) | −44 pp | (ii-b) |
| tp8 blind fraction | 93 % (not measurable) | 31 % (degraded) | −62 pp | (ii-b) |
| tp9 blind fraction | 73 % (not measurable) | 46 % (degraded) | −27 pp | (ii-b) |
| `air_cut_pct_of_total_runtime`, project | 55.06 | 44.09 | −19.9 % | (ii-b) |
| `air_cut_pct_of_cutting_time`, project | 64.60 | 50.46 | −21.9 % | (ii-b) |
| tp8 `air_cut_pct_of_total_runtime` | 66.64 | 35.71 | −46.4 % | (ii-b) |
| `average_engagement`, project | 0.0515 | 0.0814 | +58.2 % | (ii-b) |
| tp8 `average_engagement` | 0.0249 | 0.0819 | +228.6 % | (ii-b) |
| `total_removed_volume_est_mm3`, project | 819 223.8 | 815 307.3 | −0.48 % | (i) |
| tp1 removal | 590 378.7 | 586 084.0 | −0.73 % | (i) |
| tp5 removal | 160 140.3 | 156 356.5 | −2.36 % | (i) |
| tp8 removal | 43 766.7 | 47 475.0 | **+8.47 %** | (i) + regenerated path, see below |
| tp9 removal | 9 281.7 | 9 173.9 | −1.16 % | (i) |
| `peak_axial_doc_mm`, project (= tp1) | 11.335 | 16.253 | **+43.4 %** | (ii-a) |
| tp3 `peak_axial_doc_mm` | 2.295 | 3.429 | +49.4 % | (ii-a) |
| tp4 `peak_axial_doc_mm` | 3.999 | 5.143 | +28.6 % | (ii-a) |
| `sample_count`, project | 5 696 467 | 5 646 202 | −0.88 % | **regenerated paths** |
| `total_moves`, project | 507 620 | 509 652 | +0.40 % | **regenerated paths** |
| tp8 `move_count` | 442 913 | 443 902 | +0.22 % | **regenerated paths** |
| tp8 `rapid_distance_mm` | 73 749.0 | 35 698.3 | **−51.6 %** | **regenerated paths** |
| tp9 `move_count` | 30 306 | 31 349 | +3.44 % | **regenerated paths** |

### The blast radius nobody predicted: S1 changes generated toolpaths

Four of wanaka200's eight toolpaths are `StockSource::FromRemainingStock`
(tp3, tp4, tp8, tp9). Their generators read the **simulated stock grid**. Swept
stamping changes that grid — it removes slightly less at partially covered
cells, by (i) — so the rest/finish ops **generate different motion**.

That is why `total_moves` moves at all, and it is much larger than a rounding
effect on tp8: its rapid distance halves (73 749 → 35 698 mm) and its runtime
falls 7.1 %. That is a *real* change to emitted G-code on a rest chain, and it
is not obviously an improvement or a regression — it is a different
decomposition of the remaining-stock region.

**This is the most important risk in the package.** S1 is described in
`PERF_REVIEW.md` as a metric change; on any project with a rest chain it is also
a **geometry** change. Every downstream consumer of a `FromRemainingStock` path
— G-code, cycle time, the acceptance corpus — is in scope.

### Wall clock

| Phase | old (`auto`) | new (`swept`) | Δ |
|---|---|---|---|
| load | 0.699 s | 0.676 s | −3.3 % |
| generate | 26.19 s | 25.59 s | −2.3 % |
| ladder 1 sim | 48.17 s | 46.88 s | −2.7 % |
| ladder 1 regen | 106.92 s | 103.31 s | −3.4 % |
| ladder 2 sim | 63.52 s | 53.39 s | **−16.0 %** |
| ladder 2 regen | 9.38 s | 8.76 s | −6.6 % |
| final sim | 65.34 s | 55.70 s | **−14.8 %** |
| diagnostics | 2.18 s | 2.24 s | +2.6 % |
| **total test** | **322.8 s** | **296.9 s** | **−8.0 %** |

These two runs predate `9e65d1a7`, which made the swept kernel faster without
moving a bit. The wall-clock column is therefore a **lower bound** on the gain;
the metric columns above it are unaffected, because bit-identity was re-verified
after that commit and the golden diff is unchanged at 39/29 fields.

**The 3.1–4.6× kernel speed-up becomes 15 % on a real simulation and 8 %
end-to-end.** The stamp kernel is no longer the majority of `run_simulation` on
this workload — S2, S3 and S5 already took the large multiples out of it, and
what is left in front of S1 is S4 (per-toolpath full-grid clones, duplicate LUT
rebuild, full marching-cubes mesh, checkpoint clone) and S6 (trace deep-clone +
pretty JSON). Note also that the two ladder-1 phases barely move while ladder 2
and the final sim move 15 % — ladder 1 is dominated by the *regeneration* of
four rest ops, which S1 does not touch.

---

## 5. WHAT THE S1 PRESCRIPTION GOT WRONG

Four corrections, in descending order of consequence. The ledger's running count
of refuted prescriptions goes from six to ten.

**(1) "Kill by_z analytically: the min of `z(t) + h(d(t))` over a cell's
in-reach interval is at `t_center` or the descending endpoint."** This is
**false for any round-tipped cutter**, and the fix does not need it.

*Proof it is false.* For a ball nose, `h(d) = R − √(R² − d²)` and
`d(t)² = d*² + x²` with `x = (t − t*)·L`. So
`f(t) = sd + t·seg_dd + R − √(R² − d*² − x²)`. The term `−√(c² − x²)` has
derivative `x/√(c² − x²)`, which is strictly increasing in `x` — the term is
**convex** — and a linear function plus a convex function is convex. A convex
function on an interval attains its minimum at an interior stationary point
whenever the derivative changes sign inside, which it does for any descending
ramp with `|seg_dd|/L < 1`: setting `f′ = 0` gives
`x = −c·(seg_dd/L)/√(1 − (seg_dd/L)²)`, an interior point whenever that lies in
the in-reach interval. Neither `t_center` (where `x = 0`) nor the descending
endpoint is that point. Evaluating "both candidates" would therefore return a
value **strictly above** the true minimum — an under-removal — with the error
growing with ramp slope.

*Bound, since the review's rule is quantify-don't-assume.* For a Ø6 ball nose on
a 30° descending ramp with the cell at `d* = 0`, the true minimum sits
`R(1 − cos θ) ≈ 0.40 mm` below the `t_center` value, against a subdivision error
the shipped `MAX_SUBSEGMENT_Z_DROP_MM = 0.02 mm` bounds at 0.02 mm. The
prescribed replacement is **20× worse** than the thing it replaces, on the
geometry it was aimed at.

*Why it does not matter.* The by_z redundancy is **loop order, not
subdivision.** On an exactly-vertical descent `(su, sv)` is literally the same
`f64` for every subsegment (`s + (e − s)·t` with `e − s` exactly `0.0`), so
`point_cell_coverage`, the cell's `dist_sq`, its LUT probe and the `sqrt` +
probe inside `cell_upper_bound_surface` are all **loop invariants** the shipped
kernel recomputes 250 times per cell. Inverting the loops — cells outside, bins
inside — hoists every one of them and changes nothing else: each cell still sees
its blends in ascending bin order, and for a fixed bin `removed_volume` still
accumulates over cells in row-major order. The result is **bit-identical**,
sentried over a full simulation
(`pure_vertical_chunks_are_bit_identical_to_per_stamp`), and it is worth
**3.4–5.0×** on the plunge fixture. No approximation, nothing to bound.

**(2) `floor(t_center · bins)` — the review's binning rule verbatim —
ALIASES, badly.** A bin is one `sample_step` wide; a cell is one `cell_size`
wide. Whenever `sample_step < cell_size` — which is every coarse-grid
simulation, wanaka at 0.4 mm included — whole bins contain no cell centre at
all. Measured before the fix, at `cs = 0.5`: **35 % of cutting samples reported
zero removal**, against 0 % on the shipped kernel, while their neighbours
reported double. That is not a change of measure; it is a sampling artifact of
the new estimator, and it would have been read downstream as a third of the cut
being air. The landed form smears a cell over the parameter interval its own
footprint occupies, which **tiles** the path (for an axis-aligned move the
spacing in `t` is `cs/L` and the interval width is exactly `cs/L`; at 45° both
are `cs√2/L`) so no bin between the first and last covered can be empty. The
artifact drops to 0.13 %.

**(3) "Stamp a whole move in one stadium pass" is a LOSS on diagonals.** A
swept pass costs its *bounding box*, not its stadium, and for an off-axis move
those diverge quadratically: a 100 mm move at 45° with a Ø6 cutter has a bbox of
~11 300 mm² around a stadium of ~630 mm². Stamping that in one pass is slower
than the per-subsegment kernel it replaces. Chunks exist for this reason, capped
by `SWEPT_MAX_BBOX_WASTE`, and it is why the measured speed-up is a function of
path direction and not only of `2R/s`.

**(4) "≈ 26×, scales as 2R/s" is a CELL-VISIT ratio quoted as a time ratio.**
Measured **2.9–5.1×** in wall clock across every fixture and thread count. Same
class of error as S3's "6–12× desktop", refuted twice already in this review.

**(5) S1b, "moves tagged `MoveIntent::Retract` may skip the removal math", is
ALREADY DONE** and has been since Step 1 (2026-05-19). `simulation.rs`'s metric
path routes a `Retract`-tagged `Linear` move — and every `Rapid` — through
`sample_segment_runtime`, which does time accounting and `is_cutting = false`
and **no grid mutation at all**. There is nothing left to skip.

*What still runs on a skipped retract, precisely.* Rapid-collision detection is
untouched and structurally cannot be affected:
`check_rapid_collisions_against_stock` (`collision.rs:450`) is called once per
toolpath from `compute/simulate.rs:929`, **before** that toolpath carves, against
a frozen snapshot of `group_stock.z_grid`. Its only inputs are the toolpath
geometry and that snapshot; it never reads `stamp_dispatch`, `BandDispatch`,
`SweptDispatch` or any per-stamp state. Holder/shank collision checks run from
`diagnostics()`, likewise off the toolpath and the stock, not off the stamp
stream. So the hard invariant holds: **collision detection is never disabled**,
and this wave did not have to do anything to keep it that way.

*One real gap S1b's text points at, not fixed here.* The **non-metric** replay
`simulate_toolpath_with_lut_cancel` — the second full pass
`compute/simulate.rs` runs against `global_stock` — stamps `MoveType::Linear`
**regardless of intent** (`simulation.rs:84`). So the metric grid and the
playback grid disagree about retracts. That is a pre-existing divergence, it is
not a swept-stamping question, and changing it would move `global_stock`. Logged
here; not touched.

---

## 6. VERIFICATION ON THIS BRANCH, DEFAULT MODE

With `RS_CAM_STAMP_DISPATCH` **unset** — i.e. the shipped `Auto`:

| Gate | Result |
|---|---|
| `cargo test -p rs_cam_core --release --no-fail-fast` | **1 failing target**, and it is not this change — see below |
| `cargo test -p rs_cam_viz --release --no-fail-fast` | 6 targets, **0 failures** |
| `cargo test -p rs_cam_mcp --release` | **ok** (13 tests) |
| `cargo test -p rs_cam_cli --release` | **ok** (16 tests) |
| `cargo clippy --workspace --all-targets -- -D warnings` | **zero warnings** |
| `cargo fmt --check` | 17 diffs, **all in six files this branch never touched** |

**The one `rs_cam_core` failure is
`wanaka_scale_indexed_path_beats_linear_scan_and_matches_output` and it is a
pre-existing wall-clock flake.** It asserts the C9 remap interval index is
≥ 5× faster than a linear scan and read 4.14× / 4.52× on a warm machine; it
touches no dexel code. It passed on an earlier, quieter run of the same tree in
the same mode. Noted, not attributed to S1.

**The `cargo fmt --check` diffs are pre-existing rustfmt drift, not this
branch's.** The six files — `src/{edge_distance,face,inlay,rest}.rs`,
`tests/{pushcutter_band_query_g1,sub_cell_stamping_fa}.rs` — are **byte-identical
to `tech-debt-3`** (`git diff tech-debt-3 -- <file>` is empty). Running
`cargo fmt -p rs_cam_core` reformats them as a cascade; per the standing rule
they were reverted after every format pass so this branch's diff contains only
its own files. `rustfmt --check` on every file this branch does touch is clean.

`Auto` is additionally pinned *behaviourally*: `auto_never_selects_swept_dispatch`
asserts an `Auto` run is bit-identical to a `WholeToolpath` run on the grid and
on the sample stream, so "the default did not move" is a test, not a claim.

---

## 7. RECOMMENDATION

**Split the landing. Two decisions, not one.**

### 7.1 `SweptPlungeOnly` — **land**, and it needs no re-baseline

It is bit-identical to the shipped dispatch, sentried at the whole-simulation
level over a fixture that mixes rasters, a ramp, a plunge, a re-pass and an arc,
across two cutters and two cell sizes, on the grid, on `conservative_top` and on
every published per-sample metric. The **entire** `rs_cam_core` suite passes
under it — both perf goldens, both `_litmatrix_` suites, every F-XXX sentry —
with the single exception of a pre-existing wall-clock flake that also fails in
the default mode.

It is worth **3.4–5.0×** on the plunge fixture and, after the two fixes in
`9e65d1a7`, **1.37–1.45×** on the raster fixtures as well — a strict improvement
on every arm at every thread count, for zero metric movement. Making `Auto`
select it is a one-line change with a bit-identity sentry behind it.

One honest caveat on the raster figure: that gain comes from the plunge entries
`raster_pass` makes into each pass, not from lateral stamping. A toolpath with no
vertical run at all would be a wash, not a win. It would not be a loss.

The only reason it is not already the default on this branch is that the brief
reserved the landing decision.

### 7.2 `Swept` — **do not land yet.** Land-with-re-baselines is available, but
the re-baseline list is longer than the review anticipated and one item on it is
not a re-baseline at all.

The case *for* is strong and, unusually, is made by falsifiable slope tests
rather than by preference:

* measured axial DOC reaches the commanded DOC (per-stamp under-reads by up to
  70 %, and the under-read grows with cell size);
* removed volume stops depending on the sampling rate;
* the air-cut percentage stops depending on the simulation resolution
  (0.31 % → 89.61 % across cell sizes today, on an unchanged toolpath);
* on wanaka, **nine `not_measurable` metrics become measurable** and the project
  stops abstaining;
* planner/simulator interior disagreement falls 33–81×;
* zero movement on any collision channel, on the sample stream, or on
  `metrics_not_applicable`.

The case *against* is that the change is larger than "metrics moved":

1. **It changes generated geometry** on every `FromRemainingStock` op. On
   wanaka200 tp8's rapid distance halves. That is emitted G-code, and no part of
   the review anticipated it. Before landing, the acceptance corpus and the
   56 param sweeps need a pass — I did not run those under the forced mode
   because a change in *generated* output is outside what a metric re-baseline
   is allowed to absorb quietly.
2. **Three F-XXX axial sentries need widening**, and they are safety-adjacent.
   The right move is probably to re-express them against commanded Z travel per
   pass rather than to widen the margin — that keeps the net's purpose while
   accommodating a measure that is now correctly allowed to exceed `dpp`.
3. **The planner/sim parity bar is not a re-baseline.** It fails because S1
   *fixed* one side of a two-sided disagreement, exposing a pre-existing
   systematic divergence (`sim_higher`, unchanged at 1360/568) that the balance
   test was masking. That divergence should become its own finding before the
   bar is touched.
4. **The wall-clock payoff is 8 % end-to-end**, not 4×. If speed is the reason
   to take the metric risk, the reason is weak: S4 and S6 are cheaper, are
   metric-neutral, and are still unclaimed.
5. Downstream consumers tuned against the *old* readings move: the CLI's 40 %
   air-cut verdict, the GUI's 20 % banner, every per-operation threshold in
   `OperationType::air_cut_high_threshold_pct`, and the wanaka verdict itself
   (which flips from an abstention to a WARNING). Those thresholds were tuned
   against a quantity that is about to change meaning; landing S1 without
   revisiting them ships a set of bars calibrated for the old instrument.

**Suggested sequencing** if the user wants `Swept` eventually:

1. Land `SweptPlungeOnly` now (free).
2. Take S4 and S6 — bigger end-to-end wins than S1, metric-neutral.
3. Run the acceptance corpus and the 56 param sweeps under `Swept` and read the
   *geometry* diff on rest chains. That, not the goldens, is the gate.
4. Re-express F-027/F-031/F-027b against commanded Z travel.
5. Open the planner/sim `sim_higher` divergence as its own item.
6. Re-tune the air-cut thresholds against the new measure — including
   `OperationType::air_cut_high_threshold_pct`, whose per-operation bands were
   fitted to a reading that is 3–90 % resolution artifact.
7. Only then flip `Auto`.

**The one thing I would not do is regenerate the goldens and land.** The golden
diff is the smallest part of this change; the geometry diff on rest chains is
the part with no net under it.

---

## 8. Commits on `perf/s1-swept-volume`

Branched from `tech-debt-3` at `c604c096`. **`tech-debt-3` is untouched.**

| Commit | What |
|---|---|
| `51497a19` | S1 swept-volume stamping behind `StampDispatch::Swept`; new `swept.rs`; three prescription corrections in the message |
| `5d02b7db` | `SweptPlungeOnly` — the bit-identical half, on its own switch; caught a 2-ULP reciprocal defect |
| `f1e837c9` | The three falsifiable claims (density independence, commanded axial DOC, air-cut vs cell size) |
| `f035349f` | Headless wanaka200 A/B harness |
| `9e65d1a7` | Made the bit-identical mode *faster* than the shipped one (per-chunk allocations + per-cell division) |
| *(this doc)* | The decision package |

Files touched, all under `crates/rs_cam_core`:

* `src/dexel_stock/swept.rs` — **new**, the kernel and its batching driver
* `src/dexel_stock/simulation.rs` — `capture_cutting_segment_swept`,
  `flush_swept_job`, `run_swept_batch_into_samples`, one extra `Option`
  threaded through `capture_cutting_segment`'s signature
* `src/dexel_stock/whole_path.rs` — two enum variants, the env override, one
  arm in `for_grid`
* `src/dexel_stock/stamping.rs` — visibility widening only, plus
  `cell_upper_bound_height` factored out of `cell_upper_bound_surface`
  (the latter is now defined in terms of the former, so they cannot drift)
* `src/dexel_stock/mod.rs` — one `mod swept;`
* `benches/hot_paths.rs` — two arms added to `sim_dispatch_ab`
* `tests/swept_stamping_s1.rs` — **new**, 8 sentries + 1 measurement
* `tests/swept_wanaka_ab_s1.rs` — **new**, the end-to-end A/B harness

## 9. How to reproduce every number here

```bash
# sentries
cargo test -p rs_cam_core --test swept_stamping_s1 -- --nocapture

# the three falsifiable claims, printed
cargo test -p rs_cam_core --release --test swept_stamping_s1 -- --nocapture

# paired four-mode A/B
cargo bench -p rs_cam_core --bench hot_paths -- sim_dispatch_ab

# golden diff (do NOT set UPDATE_PERF_GOLDENS)
RS_CAM_STAMP_DISPATCH=swept \
  cargo test -p rs_cam_core --release --test perf_golden_sim_metrics

# sentry inventory
RS_CAM_STAMP_DISPATCH=swept        cargo test -p rs_cam_core --release --no-fail-fast
RS_CAM_STAMP_DISPATCH=swept_plunge cargo test -p rs_cam_core --release --no-fail-fast

# wanaka200 A/B (one process per mode; the override is a OnceLock)
RS_CAM_STAMP_DISPATCH=auto  cargo test -p rs_cam_core --release \
  --test swept_wanaka_ab_s1 -- --ignored --nocapture 2>&1 | grep '^S1AB' | sort > /tmp/a.txt
RS_CAM_STAMP_DISPATCH=swept cargo test -p rs_cam_core --release \
  --test swept_wanaka_ab_s1 -- --ignored --nocapture 2>&1 | grep '^S1AB' | sort > /tmp/b.txt
diff /tmp/a.txt /tmp/b.txt
```
