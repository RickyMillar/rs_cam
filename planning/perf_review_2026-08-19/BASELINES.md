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

---

# Wave 1 (GEN) — G4 then G3

Captured 2026-08-19, same machine, criterion release builds, every cargo
invocation serialized behind `flock /tmp/rs_cam_cargo.lock`.

**Machine state.** Not an idle box, and honesty about that is part of the
number. An editor-driven `cargo check --workspace` (rust-analyzer) runs
continuously in this repo and a second agent's `cargo test -p rs_cam_core`
plus an unrelated repository's `cargo test -p sysml-spec-tests` were live for
much of the window; available memory sat at **15–18 Gi**, i.e. the same
condition the Phase 0 capture recorded (18 Gi), not the 20 Gi house rule.
Every G3 arm below is a **before/after pair measured minutes apart under the
same contention**, so the ratio is sound even where an absolute would not be.
The G4 comparison is against the committed Phase 0 absolute; its effect sizes
(3.9× and 51×) are far outside any plausible contention noise.

Note on the idle gate, extending the Phase 0 note: `pgrep -x cargo` is the
right check, but waiting for it to read **zero** in this repo is waiting for
something that does not happen — rust-analyzer keeps a `cargo check` in
flight. Gate on the flock plus a memory floor, not on an empty process table.

## G4 — `Polygon2` cached exterior AABB + `RegionSet` pre-filter

`cargo bench -p rs_cam_core --bench hot_paths -- gen_contains_point`.
Before = the 0A table above (same bench, same fixture, unchanged).

| Bench | Before | After | Change | Speed-up |
|---|---:|---:|---:|---:|
| `gen_contains_point/polygon_1400v_3holes_10k` | 20.966 ms | **5.3794 ms** | −74.34% | **3.90×** |
| `gen_contains_point/regionset_8x350v_10k` | 38.928 ms | **762.54 µs** | −98.04% | **51.0×** |

Criterion's own verdict on both: `Performance has improved`, p = 0.00.

The gap between the two arms is the finding's shape, not a fluke. The single
polygon pays one O(1) box test and then still ray-casts every query point that
lands inside its box, so 3.9× is bounded by whatever share of this fixture's
sample the box can refuse — a fixture property, not a general figure, and the
one number here that should not be quoted out of context. The `RegionSet` arm
is the `.any()` layer the review named: eight
disjoint 350-vertex regions, of which a query point can be inside **at most
one**, so seven of eight ray casts were pure waste on every single call. The
`m` factor collapses and 51× is what is left. That ratio is the direct
argument for the twelve inner loops in G4's list — the ones that call this
per move, per sample, per grid cell.

## G3 — `drop_cutter` monotone-Z + XY-AABB early-outs

Neither `perf_suite`'s drop groups nor `classification` were in Phase 0, and
the only stored criterion data for them was **17 days old** (2026-08-02) at an
unrelated commit — worthless as a baseline. So both arms were measured fresh
in this session: `git stash push` of *only* `tool/mod.rs` + `dropcutter.rs`
→ bench (before) → `git stash pop` → bench (after). Nothing else in the tree
moved between the two.

`cargo bench -p rs_cam_core --bench perf_suite -- drop_cutter`:

| Bench | Before | After | Change | Speed-up |
|---|---:|---:|---:|---:|
| `batch_drop_cutter/hemisphere_ball_6mm` | 5.9134 ms | **1.9384 ms** | −67.26% | **3.05×** |
| `batch_drop_cutter/terrain_ball_6mm_step1` | 79.099 ms | **13.001 ms** | −83.61% | **6.08×** |
| `batch_drop_cutter/terrain_flat_6mm_step1` | 62.200 ms | **9.5539 ms** | −84.72% | **6.51×** |
| `point_drop_cutter/terrain_center_ball` | 6.3921 µs | **1.9588 µs** | −69.83% | **3.26×** |

`cargo bench -p rs_cam_core --bench classification`:

| Bench | Before | After | Change | Speed-up |
|---|---:|---:|---:|---:|
| `classification/current/synthetic/64` | 510.79 µs | **345.05 µs** | −34.72% | 1.48× |
| `classification/current/synthetic/143` | 4.2849 ms | **3.5431 ms** | −17.31% | 1.21× |
| `classification/current/query_only/143` | 8.5887 ms | 8.7383 ms | +1.74% (p = 0.11) | — |

**`query_only` is the control and it is the most informative row.** It runs
the spatial-index queries with none of the contact math, and it did not move
(p = 0.11, i.e. not distinguishable from noise). So the drop-cutter gains
above are the contact math being skipped, not the harness getting faster —
and it confirms `CLASSIFICATION_PERF_STUDY.md:91`'s "contact math = 4/5 of
classify cost" from the other direction. It also bounds what G3 can ever do
for classification: with the query half untouched at 8.6 ms, the synthetic
rows' 1.2–1.5× is near the ceiling, and the next classification lever is the
query, not the drop.

The terrain rows are the ones to quote for the wanaka workload — a 6× on a
661k-triangle mesh is the regime the review's reference workload lives in.
`classification/current/terrain` stayed skipped (needs `RS_CAM_M3_HEAVY=1`,
minutes per row at 849²).

### The early-out that was not exact — and what caught it

The first G3 implementation used the review's literal formulation,
`if tri.bbox.max.z <= cl.z { return }`. It made
`finish_resolution_policy_pr3::steep_shallow_fingerprint` fail with an
**identical move count (913) and a different hash** — the exact signature of a
last-ULP divergence, and precisely the thing a fingerprint golden exists to
catch.

The cause: every `edge_drop` implementation accepts an edge parameter in
`-1e-8..=1.0 + 1e-8` and then *evaluates the edge at it*, so a contact can land
up to `1e-8 × edge_length` outside the triangle's own bounding box — including
in Z. That is not a rare coincidence on a real mesh: a flat-tipped cutter's
`vertex_drop` returns `vertex.z - 0.0`, so `cl.z` becomes **exactly** a vertex
height, and every triangle sharing that vertex then satisfies
`bbox.max.z == cl.z`. The unpadded test skipped them; the old code ran
`edge_drop` and let the overshoot nudge `cl.z` up.

Both rejects are now padded by `DROP_CONTACT_SLACK_MM = 1e-4` (covers edges to
10 km; four orders past any mesh here). `steep_shallow_fingerprint` returned to
its pre-change hash, and the pad costs nothing measurable — the numbers in the
tables above are the **padded** implementation.

Two things worth carrying forward from that:

1. **The review's G3 text states the `<=` form as "provably sound".** It is
   not, at the ULP level, and the proof it offers is a proof about exact
   arithmetic applied to code with a tolerance in it. The invariant that
   actually holds is "≤ `bbox.max.z` **plus the edge-parameter overshoot**".
2. **A fingerprint golden with an unchanged count is worth more than one with
   a changed count.** Nothing about the toolpath's shape moved; only its last
   bits did. Had the net been "same number of moves", this would have shipped.

### Devirtualization: partial, deliberately

Full devirtualization of the `Box<dyn MillingCutter>` per-triangle call was
**not** attempted — `MillingCutter` carries no `Any` bound, so a match on the
concrete type from a `&dyn MillingCutter` would need a downcast facility added
to a public trait implemented by five shapes plus the `ToolDefinition`
wrapper. That spiders, exactly as the wave brief anticipated.

What landed instead gets most of the effect for two lines: `point_drop_cutter`
hoists `cutter.radius()` out of the triangle loop (on the `ToolDefinition`
wrapper that accessor is *itself* an indirect call, once per triangle) and
calls `drop_cutter_can_contact` **before** `drop_cutter`, so a rejected
triangle costs no virtual dispatch at all. The trait's default `drop_cutter`
keeps its own copy of the check as the safety net for every other caller
(`classify_probe`, `slope`, the tool-module tests). Accepted triangles pay the
four-compare check twice; that is inside the noise of a facet + 3 vertex + 3
edge drop.

Full devirtualization stays on the table as follow-up, as does the review's
cell-sorting-by-max-Z idea, which was explicitly out of scope for this wave.

### Correctness

- `cargo test -p rs_cam_core` green apart from two failures in
  `arc_fit_disposition_a5` and `wanaka_suggest_integration`, both asserting
  **Suggest feed values** and both inside another agent's in-flight
  `feeds/suggest.rs` work — nothing in this wave can move a feed rate.
- `cargo clippy -p rs_cam_core --all-targets -- -D warnings`: clean.
- New nets: `polygon::tests::{bbox_reject_agrees_with_the_ray_cast_everywhere,
  invalidate_bbox_clears_a_populated_cache,
  nan_vertex_disables_the_reject_rather_than_changing_an_answer,
  degenerate_rings_reject_without_changing_the_answer}` and
  `tool::tests::{drop_cutter_early_outs_are_bit_identical,
  drop_cutter_can_contact_rejects_only_unreachable_triangles}`. The first and
  the fifth compare against a verbatim copy of the pre-change body — bit
  patterns for the drop, exhaustive lattice for the containment — rather than
  against a pinned constant, so they cannot rot.

---

# Campaign scoreboard — waves 1-3 landed

Consolidated by the orchestrator from the per-lane `DELTA_*.md` files, which
remain the primary record (method, sentries, declined levers). This table is a
summary, not a substitute: read the delta doc before citing a number.

## Measurement discipline — read before comparing anything here

**Cross-day absolutes on this box are NOT comparable.** The SIM lane measured
the *unmodified* tree at 98.4 ms on `sim_kernel_plunge/24` against Phase 0's
126.61 ms — a 22% swing with no code change. Only **paired same-session A/B**
(measure the tree as found, then the change, in one session) is load-bearing.
Later waves quote paired numbers; the Phase 0 column below is the original
capture and is included for shape, not for arithmetic.

Criterion's stored state under `target/criterion/` has been overwritten several
times by lane runs. The numbers written down here and in the delta docs are the
record.

## Landed

| Finding | Commit | Measured | Delta doc |
|---|---|---|---|
| **G9** v-carve/inlay distance field | `7d9557db` | `gen_vcarve_field` 193.67 ms → **15.869 ms** (**12.2×**) | `DELTA_gen_w3.md` |
| **G1** push-cutter band query | `932a9719` | `push_cutter_batch/terrain` **4.51×**; `gen_waterline/L20` 97.84 → **31.83 ms** (3.07×) | `DELTA_gen_w2b.md` |
| **G4** Polygon2 cached AABB | `1e3c5d8c` | `contains_point` **3.90×**; RegionSet **51×** | (in-file, above) |
| **G2** 2.5D depth hoist | `473c3d1f` | L20/L1 **22.2× → 1.04×**; pocket L20 738.19 → **33.78 ms** | `DELTA_gen_w2.md` |
| **face.rs** (G2 sibling) | `7d9557db` | `gen_face_levels` 264.89 → **85.18 µs** (3.11×) | `DELTA_gen_w3.md` |
| **G3** drop-cutter early-outs | `ca92d767` | see in-file section above | (in-file, above) |
| **S4a/S7/S6** | `5099db9e`, `29823980` | `flat6/cs0.25` **−7.10%**, `e2e/res1` **−7.93%**; plunge arms **no change** | `DELTA_sim_w1.md` |
| **V8/V13** content-keyed uploads | `08345ee4` | 8-op generate: 36 → **8** toolpath builds, 8 → **0** mesh builds | `DELTA_viz_w2.md` |
| **V1/V3/V4** viz caching | `42ed4774` | per-frame triage/deflection/issues rebuilds removed | — |
| **S2/S3** stamp early-out + row bands | `f9f26997`, `5973b7cb` | 1.46–2.07× (S2) on every arm; S3 ceiling 2.10× at 4 threads | `DELTA_sim_w2.md` |
| **S5** fixpoint prefix memo | `2ed9df04`, `55847d4f` | `sim_fixpoint_ladder` 180.37 → **100.66 ms** (**1.79×**, ceiling 2.00×) | `DELTA_sim_w3.md` |
| **S1** `SweptPlungeOnly` — bit-identical | `5d02b7db`, `9e65d1a7` (merged `d671b049`) | raster arms **1.37–1.45×**, plunge arm **3.4–5.0×**, **zero metric movement** | `DELTA_sim_w5_s1_DECISION.md` |
| **S1** full `Swept` — **metric-changing**, now the default | `51497a19` + flip `a4ff2a8c` | kernel **2.9–5.1×**; wanaka simulate phases **−14.8/−16.0%**, whole run **−8.0%** | `DELTA_sim_w5b_landing.md` |

Two rows for S1 because they are two decisions. `SweptPlungeOnly` is free —
bit-identical to the shipped kernel, sentried at whole-simulation level, and it
needed no re-baseline of anything. Full `Swept` **changes what the simulator
measures** and, on any project with a rest chain, **what it generates**; it was
landed by explicit user decision, with the re-baselines and the accepted risks
enumerated in `DELTA_sim_w5b_landing.md`. Read the ratio against its own
context: a 3–5× stamp kernel buys 8% end-to-end, because S4 and S6 are now in
front of it.

Phase 0 instruments: `bfe4e251` (benches + goldens), `b2a5e661` (3D golden arm).
Gate repair: `e4379dd5`.

## Corrections to PERF_REVIEW — the campaign's other output

Every wave refuted something. These are the corrections, all consolidated into
`PERF_REVIEW.md` in place:

| Finding | What the review said | What was true |
|---|---|---|
| **S7** | fast paths "exact in squared space vs hoisted `(r±ext_diag)²`" | True over the reals, **false in f64** — 32 disagreements in 302,900 probes, both directions; no comparison operator makes it exact. Fixed by *solving* for the flip point (exact by construction), not by asserting the identity. |
| **S4a** | `coverage_max` "has zero consumers" | Read by the **public** `DexelGrid::coverage_at`, six times, from a sentry. True claim is the narrower one its docstring made: no *production* reader. |
| **G3** | `tri.bbox.max.z <= cl.z` "provably sound" | **Unsound.** `edge_drop`'s ±1e-8 slack plus flat-tip `vertex_drop` make `bbox.max.z == cl.z` systematic. Landed form pads by 1e-4. |
| **G9** | index is the fix; `par_iter()` "mechanical" | **Inverted.** Parallelism **4.10×**, index **2.98×**. |
| **G9** | three call sites share a distance field | **Two.** `rest.rs:126` is `contains_point` — a containment query, not a distance field, and it already has a bbox early-out. |
| **G2** | `offset_library_failures` over-counts by L | True and self-correcting — but **`truncated_core_mm2` was also ×L**, an unnamed *quantitative* defect on a surface reaching narration/diagnostics/MCP. Fixed. |
| **V3** | hoist the deflection guard | Would **silently stale** the 2D tool overlay — the pre-guard block publishes six playback fields it reads every frame. |
| **V4** | return `&[..]` | Does not compile — two call sites hold the list across a later `&mut sim`. `Arc<[SimulationIssue]>` does. |
| **V8** | per-resource dirty bits | Substituted **content keys**: dirty bits put the burden on ~40 setters, and a setter naming too few is *exactly what V13 is*. |
| **V13** | highlight "one frame stale" | Reduced to a one-*repaint* lag, not eliminated; killing it would reorder the frame for all ~40 upload sites. |
| **S5** | "provenance hashes already exist" ⇒ use them as the cache key | They exist and they **do not close the input set**. The tool hash carries **no cutter shape** (Ø6 flat ≡ Ø6 ball), `hash_toolpath` omits spans and `MoveIntent`, and none of the three covers resolution, stock, metric options, the model mesh or the group transform. Keyed on them, a tool swap inside the prefix resumes onto a grid carved by the wrong cutter — silently. |
| **S5** | "cache prefix-hash → post-carve grid" | The grid is **one of fifteen** loop-carried accumulators. `DELTA_sim_w3.md` §2.1 enumerates all of them with a reuse-or-prove disposition. |
| **S1** | "Kill by_z analytically: the min of `z(t)+h(d(t))` over a cell's in-reach interval is at `t_center` or the descending endpoint" | **False for any round-tipped cutter.** `f` is linear + convex, so the minimum is at an *interior* stationary point; evaluating "both candidates" returns a value strictly above it. On a Ø6 ball on a 30° ramp the prescribed rule errs by ≈0.40 mm against the 0.02 mm the shipped subdivision bounds — **20× worse than the thing it replaces**. The redundancy is **loop order, not subdivision**: on an exactly-vertical descent `(su, sv)` is the same `f64` for every subsegment, so inverting the loops hoists every invariant, is **bit-identical**, and is worth 3.4–5.0×. |
| **S1** | `floor(t_center · bins)` as the binning rule, verbatim | **Aliases badly.** A bin is one `sample_step` wide, a cell one `cell_size`; whenever `sample_step < cell_size` — every coarse simulation, wanaka at 0.4 mm included — whole bins contain no cell centre. Measured at `cs = 0.5`: **35% of cutting samples reported zero removal** (0% on the shipped kernel) while their neighbours reported double. The landed form smears a cell over the parameter interval its own footprint occupies, which *tiles*; the artifact drops to **0.13%**. |
| **S1** | "Stamp a whole move/chunk in one stadium pass" | **A loss on diagonals.** A swept pass costs its *bounding box*, not its stadium, and those diverge quadratically off-axis: a 100 mm move at 45° with a Ø6 cutter has an ~11 300 mm² bbox around a ~630 mm² stadium. Chunks exist for exactly this, capped by `SWEPT_MAX_BBOX_WASTE`, which is why the measured speed-up is a function of path direction and not only of `2R/s`. |
| **S1** | "≈ 26×, scales as `2R/s`" | A **cell-visit** ratio quoted as a **time** ratio. Measured **2.9–5.1×** in wall clock across every fixture and thread count. Third instance of this error class in the review (after S3's "6–12× desktop"). |
| **S1b** | "moves tagged `MoveIntent::Retract` may skip the removal math" | **Already done** since Step 1 (2026-05-19): the metric path routes a `Retract`-tagged `Linear` and every `Rapid` through `sample_segment_runtime` with no grid mutation at all. Nothing left to skip. |
| **S1** | "F-XXX / litmatrix sentries need deliberate re-baselining" — i.e. the risk is a metric re-baseline | **The risk is bigger and differently shaped.** No `_litmatrix_*` sentry moved. What moved: two goldens, three F-XXX axial bars — and one item that is **not a re-baseline at all** (the planner/sim parity bar failed because S1 *fixed* one side of a two-sided disagreement, un-masking pre-existing finding W5B-F1). And the un-anticipated one: S1 **changes generated geometry** on every `StockSource::FromRemainingStock` op, because those generators read the simulated stock grid. On wanaka200 tp8's rapid distance **halves**. That is emitted G-code, not a metric. |
| **S5** | (unstated) `phantom_prior_stock` | Must be **neither keyed nor restored**. Keyed ⇒ the memo hits zero times (S2 §2e's silent no-op). Restored ⇒ the previous round's phantom id survives into `prior_stocks`, a key a full replay never produces, on the map rest generators read. Re-derived from the live request instead. |

## Levers measured and declined

Recorded so they are not re-proposed:

- **G1 lever 2** (reuse fiber candidates across Z levels): query is **3.0%** of a
  level's cost, so the ceiling across 20 levels is **2.9%**. Not taken.
- **S7's `cell_upper_bound_surface` tail**: prescribed formula dimensionally
  wrong as written, and that sqrt feeds `conservative_top` → rapid-collision
  detection, a **safety** channel. Declined.
- **S6 `span_path` interning**: neither `Arc<[SpanId]>` nor `SmallVec`
  serialises without a workspace-manifest edit, and sharing does not survive a
  round-trip. Deferred; sized at single-digit percent.
- **STEP/enriched mesh indexing** (V13 adjacent): structural, not contained —
  that path is flat-shaded *and* per-face-group coloured, so a shared vertex
  needs agreement on both. The STL path is indexed because it *is* smooth-shaded.

## Calibration for the remaining sim work

The SIM lane's parting note, which sets expectations for S2/S3: **"S7 is a
rounding error next to S2/S3. The loop's cost is three ray walks, a LUT probe
and an RMW, not two sqrts."** The plunge arms showing *no change* under S7
(p=0.08, p=0.76) are the evidence — that fixture routes through the already
sqrt-free `point_cell_coverage`.

---

# Wave 2 (SIM) — S2 then S3

Captured 2026-08-20. Full write-up, corrections and sentry inventory in
`DELTA_sim_w2.md`; this is the numbers table only.

**Three tree states benched back to back in one session**, at load average 3.8
on a 24-core box, by checking out each state of the lane's own files:
pre-S2 (`25c6823e`), S2 (`f9f26997`), S2+S3 (working tree, later `5973b7cb`).
The pre-S2 column reproduces wave 1's committed numbers to within 1 % on every
arm, which is the check that the window was quiet.

**An earlier pair from the same day is discarded** and should not be quoted: an
unrelated repository's test suite was at 171 % CPU with load average 22, the
*unmodified* tree read 18–134 % above wave 1's numbers for identical code, and
the pair contradicted itself (`sim_e2e_small/res1` "improved 74 %" while
`res0.5` "regressed 20 %", both at p = 0.00). Criterion reports p-values against
contention as confidently as against a real change.

| Bench | pre-S2 | S2 | S2+S3 (24 thr) | S2 alone | S3 alone | total |
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

## S3 thread scaling — the number that bounds the finding

`sim_kernel_lateral/flat12/cs0.1`, 16 bands per stamp:

| threads | 1 | 2 | 4 | 8 | 24 |
|---|---:|---:|---:|---:|---:|
| ms | 165.1 | 102.3 | **78.5** | 78.6 | 108.6 |
| vs 1 thread | 1.00× | 1.61× | **2.10×** | 2.10× | 1.52× |

The review predicts 6–12×. Per-*stamp* dispatch saturates at **2.10× on four
threads** and gets *worse* past eight: a stamp is tens of microseconds, the join
tree deepens with the pool, and nothing pins a band to a worker so the rows
migrate between cores on every subsegment. Below ~12 k bbox cells the parallel
path is a net loss (28 % at 4.3 k) and is gated off. Amortising the dispatch
over a whole toolpath is the remaining lever — see `DELTA_sim_w2.md` §3f.

## Two things the plunge arm says

1. **S2 reaches it and S7 could not.** Wave 1 measured p = 0.08 / 0.76 there;
   S2 gets 1.72×/1.87×. `plunge_pass`'s *retract* is a Linear feed the kernel
   stamps like any other, subdivided by the 0.02 mm `by_z` rule into **300
   subsegments — more than the descent's 250** — every one of them pure air over
   ground the descent just cleared. More than half the stamp budget on
   plunge-heavy work was being spent retracting through a hole the cutter had
   just made.
2. That half is gone already, which changes what S1b is worth.

## In flight

G5/G6 (shared NN orderer + surface_link provenance inversion), G8 (per-setup
index/silhouette/mesh caching). Not yet started: S1, S8, G7, G10-G12, V2,
V5-V7, V9, V11, V14, V15. S5 landed in wave 3 SIM and S3's whole-toolpath
dispatch in wave 4 SIM — see the sections at the end of this file.

## 0C — wanaka200 end-to-end wall clock (2026-08-20, post metric-neutral tier) — **SUPERSEDED BY 0D**

> **Superseded 2026-08-21 by 0D (below).** After the S1 landing
> (`Auto` → `Swept`, `a4ff2a8c`) this section's *result-consistency* checks are
> no longer the right comparison: nine metrics that read `not_measurable` here
> became measurable, the project verdict changed kind (abstention → WARNING),
> and one `FromRemainingStock` toolpath's geometry changed. The **wall-clock**
> figures below remain valid as the post-metric-neutral-tier reading and are
> what the ≈6× against the pre-campaign reference is stated against; the
> metric rows are historical.

Binary: release, built at tip 9dee1889 (all waves through SIM w2 / GEN w4).
Protocol: fresh MCP GUI instance, load wanaka200.toml, timed `generate_all`
(fixpoint: true, simulation_resolution_mm: 0.4), then timed `run_simulation` (0.4).

| Stage | This run | Reference (2026-08-19, pre-campaign) |
|---|---|---|
| generate_all fixpoint, 8 ops | **394 s (6 min 34 s)** | ~40 min (approximate — anecdotal wall clock, not a paired measurement) |
| rounds / simulations | 3 / 2 (identical) | 3 / 2 |
| run_simulation standalone | **84 s** | not separately recorded |

**≈6× end-to-end**, with the caveat that the reference is approximate and
unpaired (see the cross-day comparability rule above — this is the one place we
accept it, because the gap dwarfs the observed 22% cross-day noise).

Result-consistency check (metric-neutrality on the real project): zero rapid
collisions, verdict unchanged (WARNING >20% air), measurability abstentions
identical to the 2026-08-19 RUN_LOG (tp5/tp8/tp9 not_measurable, tp1 degraded,
cell_too_coarse_for_tip_contact at 0.4 mm), fixpoint converged in the same
3 rounds / 2 simulations.

Machine state: load 4.05 at gen start, 11.55 at gen end (parallel stamping +
rayon working), 8.27 at sim end; one idle stale-binary GUI from another session
resident throughout; no cargo jobs during the run.

---

# Wave 3 (SIM) — S5 fixpoint prefix memoization

Captured 2026-08-20. Full write-up, corrections, output enumeration and sentry
inventory in `DELTA_sim_w3.md`; this is the numbers table only.

Commits: `2ed9df04` (implementation), `55847d4f` (sentries + bench arm).

## The number

New bench group, built as a **paired same-session A/B inside one criterion
invocation**: both arms run the identical three-round fixpoint ladder
(2 → 4 → 6 toolpaths over a 100 × 60 × 8 mm stock at **0.4 mm**, the wanaka
reference resolution) and differ only in whether each round leaves a prefix
snapshot for the next. `memo_off` is the pre-S5 behaviour exactly — three full
replays — so no stored "before" number is involved and the cross-day
comparability rule above does not bite.

Machine state: load average 6.09, 24 GiB available, `pgrep -x cargo` **0** at
launch.

| Bench | memo_off | memo_on | Speed-up |
|---|---:|---:|---:|
| `sim_fixpoint_ladder/3round_6op_res0.4` | **180.37 ms** [177.32, 184.11] | **100.66 ms** [98.605, 103.63] | **1.79×** |

### Read the ratio against its ceiling, not on its own

A `k`-round ladder runs `Σ nᵢ` op-simulations without the memo and only the
newly generated ones with it. For 2/4/6 that is **12 → 6**, i.e. an arithmetic
ceiling of **2.00×** — measured 1.79× is **90 % of the achievable**, and the
residual is the three rounds' unavoidable end-of-run work (trace assembly,
marching cubes, composite mesh) plus the snapshot copy.

So **1.79× is not a general figure.** The ceiling for a `k`-round ladder is
`(k+1)/2`: a 5-round rest cascade tops out near 3×, and a project with **no**
rest ops takes `FixpointPlan::single_pass` and never simulates at all, so S5
does nothing for it. The review's "HIGH on rest chains" is the right severity
shape.

## Memory, measured

`the_snapshot_shares_checkpoints_rather_than_copying_them`, on the 3-op /
0.5 mm sentry fixture:

| | bytes |
|---|---:|
| snapshot's own footprint (`held_bytes`) | **786,048** |
| checkpoint bytes it **shares** rather than copies | **1,561,680** |

`SimulationResult::checkpoints` is now `Vec<Arc<SimCheckpointMesh>>` and the
GUI's `SimCheckpoint` shares it, so the snapshot adds a refcount rather than a
second copy of the heaviest per-toolpath artifact — a marching-cubes mesh plus
a full grid clone, per toolpath. `Arc::strong_count` is **2** while held and
**1** after `clear()`, asserted rather than argued. The change also removes a
deep copy that predates S5 on the controller's result path.

Bound: at most one snapshot; lookup **takes** (a stale snapshot is freed at the
next simulation, not held); only the fixpoint ladder asks to store; a hard
`max_bytes` ceiling that refuses and counts; and an explicit `clear()` when the
ladder settles.

## Correctness

Both perf goldens green and **unchanged** — no re-baseline; S5 decides
*whether* a toolpath is simulated again, never *how*.
`cargo test -p rs_cam_core` exit 0 over 184 test binaries, `cargo test
-p rs_cam_viz -q` exit 0, `cargo clippy --workspace --all-targets -- -D warnings`
clean, `cargo fmt --check` clean for this lane's files.

## An incidental defect, reported not fixed

`dexel_stock/stamping.rs:504` panics in **debug** with "attempt to subtract
with overflow" when a stamp's bbox falls entirely outside the grid
(`row_hi + 1 - row_lo` underflows once the clamps give `row_lo > row_hi`).
Reproduced with a raster pass at `y ∈ [31, 35]` over a stock whose Y extent is
`[0, 24]`. **Pre-existing** — that line is S2's mip block (`f9f26997`) and
nothing in S5 touches `dexel_stock/` — and reachable from ordinary projects,
since toolpaths legitimately leave the stock (profile lead-ins, edge drills,
any op whose boundary extends past the blank). Release wraps instead of
panicking, so debug and release disagree. Left for the S3 lane, which owns that
file; details and two adjacent issues in the same expression are in
`DELTA_sim_w3.md` §6.

## Deferred with a reason

`session/compute.rs::simulate_candidate_isolated` still runs a **full**
`run_simulation` to harvest one cut trace. A metrics-only mode is a second
orthogonal switch inside the loop S5 just restructured **and it would have to
enter the prefix cache key** — a prefix carved with checkpoints suppressed is
not interchangeable with one carved with them on. Sketch in `DELTA_sim_w3.md`
§8.


---

# Wave 4 (SIM) — S3 whole-toolpath band dispatch

Captured 2026-08-20. Full write-up, corrections and sentry inventory in
`DELTA_sim_w4.md`; this is the numbers table only. Commits: `c652ee52` (the
out-of-grid clamp defect), `38b8e4a9` (restructure), `d1d9a0a4` (sentries +
bench arm), `ee8b9da4`.

**Paired, same-session, same-process.** The new bench group `sim_dispatch_ab`
runs `per_stamp` and `whole_path` adjacent for every fixture and thread count in
one criterion invocation, with the thread count pinned by a rayon pool per arm
rather than by `RAYON_NUM_THREADS`. The number quoted is always the ratio
between two arms of the same run.

Machine state: load average **3.36 → 4.39** over the 7-minute run, 22 GB
available, `pgrep -x cargo` / `pgrep -x rustc` clear at launch. Comparability
cross-check: `flat12_cs0.1/per_stamp/1` reads 175.5 ms against wave 2's
165.1 ms for the same code and fixture (6 %, inside the noise band this file
allows), and the *shape* of per-stamp's thread curve reproduces exactly.

**One earlier pair is discarded**: `flat12_cs0.1/whole_path/2` came back at
239 ms with a CI of [184.6, 311.0] — a 68 % spread on a 10-sample arm — while
its neighbours were tight.

## The A/B — `wp/ps` at equal thread count, `scaling` against the same shape's own 1 thread

| Fixture | thr | per_stamp | whole_path | wp/ps | wp scaling | ps scaling |
|---|---:|---:|---:|---:|---:|---:|
| `flat12_cs0.1` | 1 | 175.54 ms | 163.90 ms | 1.07× | 1.00× | 1.00× |
| | 2 | 105.31 ms | 93.95 ms | 1.12× | 1.74× | 1.67× |
| | 4 | 79.26 ms | 62.81 ms | 1.26× | **2.61×** | 2.21× |
| | 8 | 76.39 ms | 52.04 ms | 1.47× | **3.15×** | 2.30× |
| | 24 | 101.52 ms | 47.81 ms | **2.12×** | **3.43×** | 1.73× |
| `flat6_cs0.1` | 1 | 52.61 ms | 48.60 ms | 1.08× | 1.00× | — |
| | 2 | 57.76 ms | 29.91 ms | 1.93× | 1.62× | — |
| | 4 | 52.64 ms | 22.31 ms | 2.36× | 2.18× | — |
| | 8 | 51.79 ms | 18.78 ms | **2.76×** | 2.59× | — |
| | 24 | 52.16 ms | 19.40 ms | 2.69× | 2.51× | — |
| `flat6_cs0.25_plunge` | 1 | 57.56 ms | 52.11 ms | 1.10× | 1.00× | — |
| | 2 | 57.04 ms | 34.57 ms | 1.65× | 1.51× | — |
| | 4 | 57.44 ms | 31.86 ms | **1.80×** | 1.64× | — |
| | 8 | 57.01 ms | 33.98 ms | 1.68× | 1.53× | — |
| | 24 | 57.38 ms | 36.27 ms | 1.58× | 1.44× | — |

`ps scaling` is blank on the two lower rows because per-stamp dispatch is gated
**off** there by `PARALLEL_MIN_BBOX_CELLS = 12 000` — those arms are flat by
construction.

## The three findings

1. **The 2.10× ceiling is beaten and, more usefully, removed.** On the same
   fixture and thread count wave 2 measured it on (`flat12/cs0.1`, 4 threads),
   per-stamp scales 2.21× in this session and whole-path 2.61×. Per-stamp then
   *saturates and regresses* — 2.30× at 8, **1.73× at 24** — which reproduces
   wave 2's shape on a different day. Whole-path is still climbing at 24:
   2.61 → 3.15 → **3.43×**.
2. **6–12× is NOT reached.** `DELTA_sim_w2.md` §3f said whole-toolpath dispatch
   was what "would actually reach 6–12×". Measured ceiling **3.43×**, at 24
   threads on a 24-core box. The limit is band geometry, not dispatch: a
   `flat12/cs0.1` footprint covers 16 of the grid's 40 bands, so a batch of
   consecutive subsegments can occupy at most 16 workers. `BAND_ROWS` is the
   dial and it is deliberately a constant. **Sixth refuted prescription in this
   review.**
3. **The crossover was never about work size.** §3d gated per-stamp dispatch off
   below 12 k bbox cells because it was a 28 % *loss* there. `flat6_cs0.1` is
   that regime, and whole-path turns the refusal into **2.76×**. Same for the
   plunge arm, which per-stamp also never dispatches: 1.80×. The crossover was a
   property of the dispatch granularity.

## Bit-identity, stronger than wave 2 could claim

Wave 2 had to concede that `removed_volume_est_mm3` reassociates under banding.
Wave 4 does **not** re-split those sums — a job's bucket is filled from exactly
the `band_span` range the per-stamp path iterates, merged in the same ascending
order — so whole-path is bit-identical to per-stamp on the grid,
`conservative_top` and every metric field of every sample, with no tolerance
anywhere, at 1/2/4/8 threads
(`whole_path_dispatch_matches_per_stamp_bit_for_bit`). Goldens unchanged; no
re-baseline.

## The defect this wave also closed

`DELTA_sim_w3.md` §6's debug-only "attempt to subtract with overflow" in
`stamping.rs`. It was at **six** sites, not one, and four of them carried a
second defect §6 flagged but could not locate: a negative `col_max` cast through
`usize` clamps back to `cols - 1`, so a stamp entirely LEFT of the grid walked
every column. Two of those four are on the hot metric path. Fixed at all six
behind one helper (`clamped_cell_bbox`); no result moves; five regression tests
including an oracle that asserts both defect classes are actually reached.
---

# Wave 5 / 5b (SIM) — S1 swept-volume stamping

Wave 5 built and evidenced it on `perf/s1-swept-volume`
(`DELTA_sim_w5_s1_DECISION.md`, the decision package). Wave 5b **landed** it on
`tech-debt-3` by explicit user decision, with the default flipped:
`DELTA_sim_w5b_landing.md`. Both delta docs are the primary record; this is the
numbers table only.

Merge `d671b049`; flip `a4ff2a8c`; re-baselines `9b4505be` (goldens),
`a4452a59` (F-XXX axial), `b5b6db3c` (parity bar); `dbb9c8fa` (S5 key
argument); `217be7a2` (clippy).

## The kernel A/B — four modes, one criterion session per fixture and thread count

Ratios against `whole_path`, the shape `Auto` resolved to before the flip. Only
the ratios are load-bearing (cross-day rule above).

| fixture | threads | `swept_plunge` (bit-identical) | **`swept`** (the new default) |
|---|---:|---:|---:|
| `flat12_cs0.1` | 1 / 4 / 24 | 1.43× / 1.37× / 1.38× | **4.03× / 4.16× / 4.66×** |
| `flat6_cs0.1` | 1 / 4 / 24 | 1.38× / 1.45× / 1.44× | **3.29× / 3.34× / 3.67×** |
| `flat6_cs0.25_plunge` | 1 / 4 / 24 | 3.39× / 4.86× / 5.01× | **3.47× / 4.82× / 5.11×** |

Full 15-row table with absolutes in `DELTA_sim_w5_s1_DECISION.md` §1.

**Read the `swept_plunge` column first.** It is bit-identical to the shipped
kernel and still 1.37–1.45× on the raster fixtures and 3.4–5.0× on the plunge
one — the raster gain is the plunge entries `raster_pass` makes into each pass,
not lateral stamping, so a toolpath with no vertical run would be a wash rather
than a win.

**A 3–5× kernel does not buy a 3–5× simulation.** On wanaka200 the simulate
phases move 14.8–16.0% and the whole run 8.0%. S2, S3 and S5 already took the
large multiples out, and what is left in front of S1 is S4 (per-toolpath
full-grid clones + marching cubes) and S6 (trace clone + JSON).

## What the default flip cost, in one table

| | |
|---|---|
| Perf goldens re-baselined | 39 fields (2.5D) + 29 (3D). **Exactly one exact-compared field moved**: `triage_action_count` 3 → 1, itself downstream of the air-cut change. Every collision channel and the whole sample stream bit-stable. Non-vacuity guards unchanged; no tolerance widened. |
| F-XXX axial sentries re-baselined | 3 — F-027 (×2) and F-031. **Split, not widened**: absolute ceiling `dpp + 1.0`, plus the original `dpp + 0.5` kept as a 0.05% *population* bar. The ceiling catches both original defects (30–47 mm and 44.8 mm) by 7–12×. The population bar catches F-027's (0.55% vs the 0.05% cap, 11×) but **not** F-031's — those 282 samples were transit-class, which the test filters out, and 0.045% would sit under the cap regardless. Recorded because the tidy version of that sentence is false. |
| Planner/sim parity pair | **Not a re-baseline — a tightening.** See below. |
| `_litmatrix_*` (7 binaries), collision channels | **nothing moved** |
| 56 param sweeps | **NOT RUN.** They are `#[ignore]`d by default, so a green suite says nothing about them. Follow-up W5B-F3 — the one gate this landing did not clear. |
| Generated geometry | **CHANGES on every `FromRemainingStock` op.** wanaka200 tp8's rapid distance halves. Accepted risk; acceptance corpus + 56 sweeps under the new default are follow-up W5B-F3. |

## The most interesting result in the wave: the parity bar was passing on a cancellation

Measured today, paired, same binary, both dispatches:

| fixture / mode | interior `planner_higher` | interior `sim_higher` | boundary `sim_higher` | whole-grid skew |
|---|---:|---:|---:|---:|
| AgentSearch, `whole_path` | 704 | **0** | 1360 | 1.55× — *passed* |
| AgentSearch, `swept` | 21 | **0** | 1360 | 34.87× — failed |
| ContourParallel, `whole_path` | 647 | **0** | 568 | 1.30× — *passed* |
| ContourParallel, `swept` | 8 | **0** | 568 | 71.00× — failed |

Interior `sim_higher` is **exactly zero in all four rows**, so the whole-grid
bar was never reading one population with two directions. It was reading two
*separate*, each perfectly one-sided populations — an interior one (the
simulator over-removing; the old kernel's `f`-blend artifact, which swept fixed
33×/81×) and a boundary one (the planner over-claiming; **bit-for-bit invariant
under the dispatch**) — that nearly cancelled when summed. 1.55×/1.30× is what
that cancellation looked like from outside.

The bar is now stated over the interior population, which makes it **stricter**:
under `whole_path` that skew reads 704× and 647× against the same 2.5× bar, and
`RS_CAM_STAMP_DISPATCH=whole_path` no longer runs this pair green. The boundary
divergence is pinned as named finding **W5B-F1**, not buried.

## S5 interaction: the dispatch shape does not belong in the prefix key

Checked because the flip made it a live question — a prefix carved under
`whole_path` and resumed under `swept` would be a key-closure defect. It cannot
happen: the mode is a `OnceLock`-backed **process constant** (only
`TriDexelStock::from_bounds` constructs, and it always stamps
`StampDispatch::default()`; no production code assigns the field), and the
cache is in-process and single-slot. All 15 `sim_prefix_memo_s5` sentries green
at the new default with no re-baseline, including the four whole-result
bit-pattern fingerprints. The precondition is now pinned by a test and the
argument written into `sim_prefix.rs`'s key table.

## 0D — wanaka200 reference at the new default (2026-08-21)

**Supersedes 0C's metric rows.** Headless harness
(`crates/rs_cam_core/tests/swept_wanaka_ab_s1.rs`), one process,
`RS_CAM_STAMP_DISPATCH` unset (= the shipped `Auto` = `Swept`), 0.4 mm, F.4
ladder driven by hand.

Machine state: load average **2.12 at start, 17.43 at end** (the run's own rayon
pool), 21 GiB available, `pgrep -x cargo` zero at launch.

### Safety — unchanged, and structurally so

| Metric | 0C | **0D** |
|---|---:|---:|
| `rapid_collision_count`, project and all 8 toolpaths | 0 | **0** |
| `collision_count`, project and all 8 toolpaths | 0 | **0** |
| `resolution_clamped` / `effective_cell_mm` | 0 / 0.400 | **0 / 0.400** |
| ladder rounds / pending after | 2 / 0 | **2 / 0** |

Collision detection runs off the toolpath and a frozen pre-carve stock
snapshot, never off the stamp stream, so no dispatch change can reach it.

### Verdict, measurability and project metrics

| | 0C | **0D** |
|---|---|---|
| Verdict | `NOT MEASURED: air-cut % withheld for 3 toolpaths` | `WARNING: high air cutting` — tp5 51%, tp8 36%, tp9 56% |
| `not_measurable` rows | **9** | **0** |
| `degraded` rows | 3 | **12** |
| blind fraction tp1 / tp5 / tp8 / tp9 | 37 / 71 / 93 / 73 % | **15 / 27 / 31 / 46 %** |
| `air_cut_pct_of_total_runtime` | 55.06 | **44.094** |
| `air_cut_pct_of_cutting_time` | 64.60 | **50.458** |
| `average_engagement` | 0.0515 | **0.08138** |
| `total_removed_volume_est_mm3` | 819 223.8 | **815 307.3** |
| `peak_axial_doc_mm` | 11.335 | **16.253** |
| `sample_count` / `total_moves` | 5 696 467 / 507 620 | **5 646 202 / 509 652** |

### Per toolpath

| # | name | op_kind | removal mm³ | air % | avg eng | peak axial | moves | rapid mm |
|---|---|---|---:|---:|---:|---:|---:|---:|
| tp0 | 1 Pin Drill | `alignment_pin_drill` | — | — | — | — | 66 | 733.5 |
| tp1 | 2 Back Rough | `adaptive3d` | 586 084.0 | 16.73 | 0.1592 | 16.253 | 11 761 | 19 944.6 |
| tp2 | 3 Holes (6mm pilot) | `drill` | — | — | — | — | 234 | 1 103.7 |
| tp3 | 4 Rivers (back, V-bit) | `project_curve` | 8 659.6 | 15.97 | 0.3464 | 3.429 | 4 365 | 13 045.1 |
| tp4 | 5 Lakes (back) | `project_curve` | 7 558.2 | 10.90 | 0.4789 | 5.143 | 1 184 | 1 535.9 |
| tp5 | 6 3D Rough (front) | `adaptive3d` | 156 356.5 | 50.77 | 0.1108 | 4.200 | 16 791 | 42 667.2 |
| tp8 | 7 3D Finish (R1.5) | `drop_cutter` | 47 475.0 | 35.71 | 0.0819 | 2.805 | 443 902 | **35 698.3** |
| tp9 | 8 Pencil detail (R0.5) | `pencil` | 9 173.9 | 55.83 | 0.0461 | 1.666 | 31 349 | 127 008.3 |

tp8's rapid distance was **73 749.0 mm** at 0C. It halves because tp8 is a
`FromRemainingStock` op whose generator reads the simulated grid, and swept
changes that grid. That is emitted G-code.

### Wall clock — recorded, NOT load-bearing

Total **309.9 s** (load 0.576 / generate 26.90 / ladder1 sim 51.08 / ladder1
regen 104.73 / ladder2 sim 57.58 / ladder2 regen 8.72 / final sim 57.69 /
diagnostics 2.28). This is a single unpaired cross-day absolute and the rule at
the top of this file forbids comparing it to the 2026-08-20 session. The paired
figure from that session stands and is the one to cite: **−8.0% end-to-end,
−14.8/−16.0% on the simulate phases**, itself a lower bound because both of
those runs predate `9e65d1a7`.

## Verification at the new default

185 `rs_cam_core` test targets ok; `rs_cam_viz` / `rs_cam_cli` / `rs_cam_mcp`
clean; `cargo clippy --workspace --all-targets -- -D warnings` zero warnings;
`rustfmt --check` clean on every file the lane touches.

One `rs_cam_core` failure:
`wanaka_scale_indexed_path_beats_linear_scan_and_matches_output` read 4.4×
against a ≥5× wall-clock bar while 180 other test binaries ran alongside it, and
**passes in isolation on an idle lane**. Pre-existing, touches no dexel code,
recorded as the same flake by the decision package.

One claim from the decision package **did not reproduce**: clippy was not clean
on the merged lane (4 `needless_range_loop` in `swept.rs`, 6 `print_stdout` in
`tests/swept_stamping_s1.rs`, all pre-existing on the lane's own files). Fixed
in `217be7a2`. Every other number in the package reproduced exactly, including
all 39 + 29 golden fields, all three F-XXX readings, both parity splits and
every wanaka metric row.
