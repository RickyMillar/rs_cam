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

S3's whole-toolpath dispatch (the remaining 6–12× lever), G5/G6 (shared NN
orderer + surface_link provenance inversion), G8 (per-setup index/silhouette/
mesh caching). Not yet started: S1, S5, S8, G7, G10-G12, V2, V5-V7, V9, V11,
V14, V15, and the 0C wanaka wall-clock protocol.

## 0C — wanaka200 end-to-end wall clock (2026-08-20, post metric-neutral tier)

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
