# Phase 0 baselines — PERF_REVIEW 2026-08-19

Baseline numbers for `PERF_REVIEW.md`. **These land before any fix**, which is
the only order in which a baseline means anything: a number captured after a
refactor proves the refactor is self-consistent, not that it changed anything.

| | |
|---|---|
| Bench target | `crates/rs_cam_core/benches/hot_paths.rs` (criterion, `harness = false`) |
| Goldens | `crates/rs_cam_core/tests/perf_golden_sim_metrics.rs`, `crates/rs_cam_core/tests/perf_golden_depth_level_geometry.rs` |
| Golden data | `crates/rs_cam_core/tests/fixtures/perf_golden_sim_metrics.json`, `.../perf_golden_depth_fingerprints.json` |
| Captured | 2026-08-19 |
| Scope | 0A (criterion benches) + 0B (correctness goldens). **0C (wanaka wall clock) is NOT in this file** — it is a manual protocol on an idle machine and is captured separately. |

## Machine state at capture

```text
2026-08-19 16:41:24 NZST  (run ended 16:47:55 — 6 min 31 s including the release compile)
Mem: 54 Gi total / 18 Gi available / 14 Gi buff-cache
cargo processes: 0     rustc processes: 0
```

Build lane was genuinely idle at launch (verified with `pgrep -x cargo` /
`pgrep -x rustc`, both zero). Available memory read 18 Gi rather than the 20 Gi
house rule, with 14 Gi of that in reclaimable page cache and no competing
compile; the run completed without pressure. Two unrelated background jobs — an
editor-driven `cargo check --workspace` and another repository's test binary —
had held the lane earlier and were clear by launch time.

Criterion always builds `--release`, so every number below is a release number.

Note on the idle check: the documented `pgrep -f "carg[o]"` gate **self-matches**
in this harness. The bracket trick stops `pgrep` matching its own process, but
the parent `zsh -c '…cargo bench…'` still carries the word in its argv, so the
gate reported BUSY continuously against nothing. The reliable form is
`pgrep -x cargo` / `pgrep -x rustc`, which match on process *name*. Worth
knowing before the next person waits half an hour for an empty lane.

## 0A — bench results

`cargo bench -p rs_cam_core --bench hot_paths`. Criterion mean, with the
[lower upper] confidence bounds. Nine benches specified, **nine implemented,
none skipped.** 25 measured rows; whole suite 6.5 min including the compile.

### Simulation kernel

| Bench | Mean | Bounds |
|---|---:|---|
| `sim_kernel_lateral/flat6/cs0.25` | **19.483 ms** | [19.096, 19.878] |
| `sim_kernel_lateral/flat6/cs0.1` | **101.74 ms** | [100.42, 103.79] |
| `sim_kernel_lateral/flat12/cs0.25` | **59.404 ms** | [58.610, 60.425] |
| `sim_kernel_lateral/flat12/cs0.1` | **329.18 ms** | [325.86, 333.04] |
| `sim_kernel_plunge/flat6_cs025/24` | **126.61 ms** | [111.78, 133.74] |
| `sim_kernel_plunge/flat6_cs025/60` | **283.48 ms** | [277.86, 290.40] |

Fixture: 6 raster passes × 40 mm at `sample_step` 0.25 mm (960 subsegments)
over a 50 × 32 mm stock; plunge arm is 24/60 plunge-retract cycles 5 mm deep.

Scaling, which is the part S1a/S7/S8 predict:

- **cell size**: ×2.5 finer (0.25 → 0.1) costs **5.22×** on Ø6 — the 6.25×
  cell-count increase minus fixed per-subsegment overhead.
- **tool radius**: Ø6 → Ø12 at cs 0.1 costs **3.24×**. S1a's redundancy scales
  as 2R/s, and the footprint area as R², so the sub-4× reading is the fixed
  cost per subsegment showing through.
- **plunge**: 2.5× the holes costs 2.24×, i.e. ≈ 19 µs per `by_z` stamp at
  15,000 stamps for the 60-hole arm. This is S1b's cost, isolated.

### Simulation end-to-end (S4, S6-partial)

| Bench | Mean | Bounds |
|---|---:|---|
| `sim_e2e_small/3op_2d/res1` | **20.318 ms** | [20.050, 20.571] |
| `sim_e2e_small/3op_2d/res0.5` | **79.697 ms** | [78.881, 80.634] |

Halving the cell costs **3.92×** against a 4× cell count — near-perfectly
linear in grid cells, which is what S4's "O(k·cells) of clone + mesh +
checkpoint" predicts and what a fix should bend.

### Generation

| Bench | Mean | Bounds |
|---|---:|---|
| `gen_depth/pocket/L1` | **33.310 ms** | [32.862, 34.237] |
| `gen_depth/pocket/L20` | **738.19 ms** | [697.80, 785.67] |
| `gen_depth/profile/L1` | **2.0163 ms** | [1.9341, 2.1598] |
| `gen_depth/profile/L20` | **45.429 ms** | [43.827, 47.085] |
| `gen_depth/zigzag/L1` | **1.9364 ms** | [1.8389, 2.0135] |
| `gen_depth/zigzag/L20` | **39.448 ms** | [38.673, 40.177] |
| `gen_waterline/rolling61_ball6/L1` | **407.18 µs** | [398.83, 418.22] |
| `gen_waterline/rolling61_ball6/L20` | **97.842 ms** | [96.736, 99.074] |
| `gen_contains_point/polygon_1400v_3holes_10k` | **20.966 ms** | [20.708, 21.265] |
| `gen_contains_point/regionset_8x350v_10k` | **38.928 ms** | [38.325, 39.639] |
| `gen_rapid_order/nn_seed/5000` | **24.343 ms** | [24.118, 24.617] |
| `gen_rapid_order/nn_seed/20000` | **408.32 ms** | [405.09, 412.04] |
| `gen_vcarve_field/frame60_9holes_tol005` | **193.67 ms** | [191.02, 196.56] |

#### G2 — the number that matters is the ratio

| Operation | L20 / L1 |
|---|---:|
| pocket | **22.2×** |
| profile | **22.5×** |
| zigzag | **20.4×** |

At L = 20 the review predicts ≈ 20×; measured 20.4–22.5×, the excess being the
inter-level retract and the larger output vector. **After the G2 hoist these
must fall towards ~1** plus per-level emission. This is the single clearest
"before" number in Phase 0, and the golden in 0B pins that the hoist may not
change a single emitted move while doing it.

#### G5 — the uncapped quadratic NN seed

4× the segments (5k → 20k) costs **16.8×**. That is the O(n²) the review
names, measured: `MAX_2OPT_SEGMENTS = 500` caps the 2-opt refinement, but
nothing caps the seed.

#### G1 — read this ratio with care

`gen_waterline` L20/L1 reads **240×**, and that is **not** a repetition factor.
The L1 arm sits at the mesh's maximum Z, where almost nothing is in section, so
it is cheap for a reason that has nothing to do with level count: the ratio
conflates *how many* levels with *how much material each level intersects*.
**Use the L20 absolute (97.8 ms) as the G1 baseline** and compare like-for-like
after the row-query fix; do not quote the 240×. A cleaner arm would hold the Z
band fixed and vary only the step, and is worth adding when G1 is actually
worked.

### Viewport, core side (V1)

| Bench | Mean | Bounds |
|---|---:|---|
| `viz_triage_build/measurability/100000` | **1.3103 ms** | [1.2718, 1.3695] |
| `viz_triage_build/triage/100000` | **1.8322 ms** | [1.7404, 1.9094] |
| `viz_triage_build/measurability/600000` | **14.323 ms** | [13.907, 14.768] |
| `viz_triage_build/triage/600000` | **17.519 ms** | [17.354, 17.866] |

Both scale linearly in samples. At the wanaka-scale 600k-sample trace the pair
costs **31.8 ms per rebuild** — and V1 is that they are rebuilt *every frame*,
which is a 60 fps budget blown by a single strip on its own, before the other
five uncached per-frame derivations in Tier 1. The fix is the existing
`(Arc::as_ptr(trace), edit_counter)` cache; this bench is what proves the
cache-hit path is free rather than merely cheaper.

### Bench-suite health

Criterion emitted four benign "unable to complete 10 samples in 5.0 s" notices
(pocket/L20, waterline/L20, rapid_order/20000, vcarve) and extended those
targets itself. No row was skipped, no sizing changed.

## 0B — correctness goldens

Both goldens regenerate with an env-var opt-in and assert otherwise:

```text
cargo test -p rs_cam_core --test perf_golden_sim_metrics
cargo test -p rs_cam_core --test perf_golden_depth_level_geometry
UPDATE_PERF_GOLDENS=1 cargo test -p rs_cam_core --test perf_golden_sim_metrics
```

### `perf_golden_sim_metrics` — the metric-neutrality net

A three-operation 2.5D project (pocket + zigzag + profile, one Ø6 flat end mill,
100×80×12 mm stock, sim resolution pinned at 1.0 mm) built through the real
`ProjectSession::add_*` entry points, generated, simulated, and reduced to
per-toolpath aggregates: sample/move/collision counts, both air-cut
denominators, engagement, removal volume, runtime split, and the peak
reductions.

Tolerances: counts **exact**; per-sample maxima at `1e-9` relative (a max is
order-independent, so a reassociation cannot move it); accumulated sums at
`1e-3` relative, because float addition is not associative and S3's row-band
decomposition reassociates every sum in the trace by design.

This is what lets **S2** (tile max-top early-out — an exact skip, since
`h(r) ≥ 0`) and **S3** (row-band parallel stamping — claimed bit-identical)
prove they are metric-neutral. It is also the thing that gets **deliberately
re-baselined for S1** (swept-volume stamping changes `pre_fresh` per sample),
in its own commit, with the reason in the message.

A companion test, `golden_fixture_is_not_vacuous`, asserts the fixture actually
cuts (>500 samples, >100 mm³ removed, every toolpath non-empty). A golden over
a project that removed nothing would pin a page of zeros and pass forever —
the vacuity failure `gate_population_vacuity_xvac` exists for, applied to a
golden instead of a gate.

Captured values (project level):

| | |
|---|---|
| toolpaths | 3 |
| total cut samples | 17,720 |
| removed volume | 31,010.108 mm³ |
| air cut, % of total runtime | 74.806 |
| air cut, % of cutting time | 77.385 |
| average engagement | 0.1038 |
| collisions / rapid collisions / holder | 0 / 0 / 0 |
| triage safety / action findings | 0 / 3 |

### `perf_golden_depth_level_geometry` — the G2 invariant

Two independent nets over pocket / profile / zigzag on a 1400-vertex jittered
ring at L = 1 and L = 8:

1. **`per_level_xy_is_invariant`** — structural, no pinned constant. Asserts
   every Z level's ordered XY sequence is *bit-identical* (compared as `u64`
   bit patterns, so a last-ULP divergence cannot hide and `-0.0 == 0.0` cannot
   mask one) to level 1's. This is G2's invariant stated as a property: it
   cannot rot and never needs re-baselining.
2. **`depth_stepped_fingerprints_match_golden`** — byte-level FNV-1a over the
   move list (the repo's one canonical fingerprint, included from
   `tests/common/fingerprint.rs` rather than re-rolled). G2's hoist claims
   "identical by construction"; if this hash moves, the hoist changed emission,
   not just scheduling.

Captured fingerprints:

| arm | moves | FNV-1a |
|---|---:|---|
| `pocket_L1` | 11,989 | `59cc2182761d9087` |
| `pocket_L8` | 95,912 | `4ba05b5102a51bff` |
| `profile_L1` | 2,249 | `ab093c1debebbb8f` |
| `profile_L8` | 17,992 | `fce9f14353bc184f` |
| `zigzag_L1` | 76 | `1e1c37ac23d56b91` |
| `zigzag_L8` | 608 | `6c1e2d61c8a92387` |

## What the goldens already showed

Three things surfaced while building the nets that the review did not name.

**1. G2 is visible in the golden's move counts, not just in the timing.**
`pocket_L8` is **95,912** moves against `pocket_L1`'s **11,989** — exactly
8×, to the move. Same for profile (17,992 = 8 × 2,249) and zigzag
(608 = 8 × 76). The per-level output is not merely *similar*, it is the same
move list eight times with a different Z stamp, which is the strongest possible
form of the review's "identical by construction" claim and means the hoist has
no emission-order subtleties to preserve.

**2. `peak_axial_doc_mm` reads a hard 0.0 on an op that removed 582 mm³.**
The Zigzag toolpath in the sim-metrics golden records
`peak_axial_doc_mm: 0.0`, `average_engagement: 0.0063` and
`air_cut_pct_of_total_runtime: 94.3` while `total_removed_volume_est_mm3` is
**582.689**. This is the `sim_measurability` failure mode CLAUDE.md documents
(a metric that reads a clean zero over a real cut) appearing unprompted on a
shipped operation at a shipped resolution — 1.0 mm cells against a 3 mm-deep
single-level pass. It is not caused by anything in this phase, and no threshold
was moved to accommodate it; it is recorded here because **any S1/S2/S3 fix
evaluated against this golden must not read that 0.0 as "measured and clean"**.

**3. The core simulation writes nothing to disk, so half of S6 is not
core-benchable.** S6 attributes cost to `to_vec_pretty` JSON written
synchronously on the analysis lane. That write has exactly one production
caller and it lives in `rs_cam_viz`
(`compute/worker/execute/mod.rs`, commented "viz-only filesystem concern");
`ProjectSession::run_simulation` never touches the filesystem. `sim_e2e_small`
therefore measures S4 (per-toolpath grid clone, LUT rebuild, marching-cubes
mesh, mesh transform, checkpoint clone) and S6's per-sample allocation half
only. The JSON-dump half needs a viz-side instrument, which is out of Phase 0
scope as written.

## Fix-verification loop

Per `PERF_REVIEW.md`: baseline number from this file → implement fix →
goldens + F-XXX sentries + targeted `cargo test -p rs_cam_core` green →
re-run the finding's bench and record the delta here → for structural fixes,
re-run the 0C wanaka wall clock.
