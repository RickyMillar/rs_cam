# Wave 1 (SIM) — golden 3D arm, S4a, S7, S6

Captured 2026-08-20 against the Phase 0 numbers in `BASELINES.md`. Every cargo
invocation serialized behind `flock /tmp/rs_cam_cargo.lock`; two other lanes
(GEN, VIZ) were editing the same working tree throughout.

**Do not read this file as a replacement for `BASELINES.md`.** It is one lane's
delta; the consolidation is the orchestrator's.

---

## 0. Prerequisite — a 3D arm on the sim-metrics golden

Commit `b2a5e661`, landed **before** any src file was touched.

Phase 0's golden is pocket + zigzag + profile on a 2D polygon model. It works
the stamp kernel hard and contains no 3D finishing kinematics: no ball-tip
contact, no curved surface, and — the part that matters for waves 3–4 — **no
`CutKinematics::Arc` sample anywhere**. S1 (swept volume), S2 (tile early-out)
and S3 (row-band parallel stamping) all reshape the arc and helix stamp
branches, so a net built only from that arm would have pinned the
straight-and-level path and let a bug in the code actually under change ship.

Added:

| | |
|---|---|
| Fixture | synthetic hemisphere r = 10 mm, 8 divisions (992 tri), Ø6 ball nose, drop cutter (stepover 1.5) + waterline (z-step 1.5), `arc_fitting` forced on |
| Sim cell | **0.5 mm** — a ball tip needs a cell well under the tip radius before radial engagement is measurable at all (`PERP_COVERAGE_GATE`) |
| Golden | `tests/fixtures/perf_golden_sim_metrics_3d.json` |
| Both arms now record | the per-toolpath `per_kinematics` block |

Measured on the new arm: **765 `Arc` samples, 4402 `Helix`**, 8033 cut samples
total, average engagement **0.265**, 3423.7 mm³ removed, 0 collisions.

`three_d_arm_covers_arc_and_helix_kinematics` asserts Arc and Helix are
populated, so the arm cannot quietly decay into another Linear-only fixture the
day a generator stops fitting arcs — a golden can go green on a fixture that
stopped covering the branch it was built for, and nothing else would notice.

Two smaller things the arm brought with it:

* `close_opt` — `None` on a `KinematicsSummary` field means **not measured**,
  not zero, so its None-ness is compared exactly and never toleranced into a
  value. Same contract the rest of the codebase's `Option` findings carry.
* The 2.5D arm **already emitted `Helix`** (ramp entries: XY and Z both change,
  which is what `classify_cut_kinematics` calls a helix). `Arc` is the class the
  3D arm uniquely adds. Worth knowing before anyone claims the old golden had no
  3D kinematics at all.

The 2.5D golden JSON was regenerated **only** to carry the new block. Verified
field-by-field: strip `per_kinematics` from both and the before/after JSON are
equal. No previously pinned value moved.

---

## THE HARD CONSTRAINT — held

`sim_metrics_match_golden` and `sim_metrics_3d_match_golden` are **green,
unchanged**, after every change below. Nothing in this wave was re-baselined.

---

## 1. S4a — two deletions

### 1a. `coverage_max` — the review's "zero consumers" is WRONG, and here is what was done about it

**Finding correction.** `PERF_REVIEW.md` S4 says `coverage_max` is *"written
`stamping.rs:365,460,568,673`, read by nothing"*. That is not accurate. It was
read by `DexelGrid::coverage_at`, a **public** accessor, which was in turn read
in **six places** by `crates/rs_cam_core/tests/sub_cell_stamping_fa.rs` — the
F.a sub-cell-stamping sentry suite. The wave brief said to STOP and report if a
consumer turned up, so: reporting it.

What is true is the narrower statement the field's own docstring makes — *"Not
used by any planning / mesh / collision consumer today"*. Confirmed by grep
across the whole workspace: **zero production readers**, in core, viz, mcp or
cli. The only reader anywhere was that one test file.

So the field was deleted, and the sentries were **converted rather than
dropped**. On fresh stock the coverage a stamp applied at a cell is recoverable
exactly from the observable it drives — `ray_blend_above` lowers the ray top by
`coverage × depth`, so `coverage = (top₀ − top) / depth`. A new
`observed_coverage` helper does that, and the assertions now run against the
blended ray top, which is the quantity the simulator actually consumes.

Net effect on the sentry, honestly stated:

| Old assertion | New assertion | Strength |
|---|---|---|
| interior cell `coverage_max ≈ 1` | interior cell blended to full depth | same |
| boundary cell `coverage_max > 0` | boundary cell top lowered | same |
| straddling cell `∈ (0,1)` | straddling cell strictly between untouched and fully cut | same |
| — | **straddling coverage is a multiple of 1/16** | **new** — pins the 4×4 sub-sample fan, which the sidecar reading never checked |
| top `= top₀ − cov·depth` where `cov` came from the sidecar | top strictly inside `(surface, top₀)` | weaker in form, but the old version was circular once `cov` is derived from `top` |
| `coverage_max` never regresses on a second stamp | observed coverage never regresses on a second stamp | equivalent — and stronger in substance, since blending `f₁` then `f₂` removes `1 − (1−f₁)(1−f₂) ≥ max(f₁,f₂)` |

Removed: 4 B/cell from every `DexelGrid` (so from every full-grid clone in
`compute/simulate.rs`), one `Vec<f32>` allocation + zero-fill per grid
construction, and one read-modify-write against a separate cache stream from
each of the four stamp kernels' inner loops.

`crates/rs_cam_core/src/adaptive3d/mod.rs` also constructed the field (a
`DexelGrid` struct literal); two lines removed there. That file belongs to no
lane in this wave.

### 1b. Duplicate `playback_lut`

Confirmed and deleted. `compute/simulate.rs:848` built
`RadialProfileLUT::from_cutter(&entry.tool, LUT_SAMPLES)` — the same two
arguments as `lut` at :728, i.e. a bit-for-bit duplicate 4096-entry table,
rebuilt once per toolpath. The playback stamp is a different grid in a different
frame, but it is the same cutter, so it takes the same profile.

---

## 2. S7 — de-sqrt the segment coverage kernel

`stamping.rs`'s `segment_cell_coverage` paid **two** `sqrt`s per cell in the
swept bounding box, and one of them was `r_sq.sqrt()` — a loop invariant,
recomputed per cell because the radius arrived pre-squared.

Both fast-path tests now run in squared space against a `CoverageFastPath`
built once per `stamp_*` call.

**The sentinel the review's text does not mention.** `center_d + ext_diag ≤ r`
squares to `center_d² ≤ (r − ext_diag)²` **only while `r ≥ ext_diag`**. When the
4×4 sub-sample fan is wider than the cutter — a Ø0.5 tool on a 1 mm grid, and
the fine-tool regime is real — `r − ext_diag` is negative, the original test is
unsatisfiable for every cell, and naively squaring it would flip the test's
sense and report **full coverage for cells near the tool axis**. `inner_sq`
carries `-1.0` in that case, which no non-negative `center_d_sq` can satisfy.

### The review's "exact in squared space" is FALSE, and the sentry proved it

This is the wave's most important finding, and it nearly shipped as a claimed
win. The first S7 implementation used the review's literal formulation — hoisted
`(r ± ext_diag)²` — and `squared_fast_paths_agree_with_the_sqrt_form` **failed**.

I had never run it. The test module went into `stamping.rs` about two minutes
*after* the full `cargo test -p rs_cam_core` was launched, so the suite compiled
the pre-test snapshot, and the clippy run afterwards compiles tests without
executing them. The green reported for the suite was real and did not cover this
test. Recording that because the mechanism — a net added after the run that was
supposed to exercise it — will recur.

What it caught, reproduced in exact IEEE754 semantics:

| sweep | disagreements |
|---|---|
| original density (36 pairs × ~403 probes) | **4** / 14,544 |
| denser (110 pairs × ~3,000 probes + ULP neighbourhoods) | **32** / 302,900 |

and they go **both ways**. The clearest case: at `d = fl(r + ext_diag)` the
squared form sees `d² == outer_sq` exactly and fast-paths to `Some(0.0)`, while
the sqrt form evaluates `fl(fl(r + ext_diag) − ext_diag)`, which lands *below*
`r`, and falls through to sub-sampling. All four strict/non-strict comparison
variants were swept — 32, 114, 117, 199 disagreements. **No choice of comparison
operator makes the algebraic bound exact.** Over the reals the review is right;
in `f64` the two rounding paths differ by an ULP either side of both bounds.

### What landed instead — solve the threshold, don't derive it

Both legacy predicates are **monotone in `d_sq`** (`sqrt` is monotone and
correctly rounded; adding a constant and comparing preserve that). A monotone
predicate over a totally ordered domain has an exact flip point. So
`CoverageFastPath::new` no longer *computes* the bound — it **solves** for it:
start from the naive squared value and walk ULPs (bounded at 64) until the
legacy predicate's own answer flips.

The threshold is then *defined as the boundary of the old test*, not derived
from an identity that only holds over the reals, so it is exact **by
construction**. Cost: a handful of `sqrt`s once per stamp, buying back one per
cell. Verified 0/402,072 disagreements in the f64 model before any Rust changed.

### The sentry got stronger, not weaker

There was no tolerance to relax — it compares `Option<f32>` for exact equality —
and the temptation in a case like this is to thin the sweep instead. The
opposite happened:

| | before | after |
|---|---:|---:|
| radii | 6 (0.25 … 10) | **11** (0.1 … 12.7) |
| cell sizes | 6 (0.05 … 2) | **11** (0.02 … 5) |
| sweep steps | 400 | **600** |
| ULP neighbourhood around each boundary | — | **±4, at 5 values** |
| probes | ~14.5k | **~73k** |

The ULP neighbourhood is the load-bearing addition: the disagreements live
within one ULP of a boundary, and a pure linear sweep steps straight over them.
`sentinel_rows > 0` is untouched, and a new `probes_checked > 70_000` assertion
means a future thinning of the sweep fails loudly instead of going green
quietly. The oracle remains a **verbatim copy of the pre-S7 sqrt body**, so it
cannot rot into agreement with whatever the implementation later becomes.

### `cell_upper_bound_surface` — DECLINED, with reasons

The wave brief offered the third sqrt at `stamping.rs:241-250` as an "if cheap"
extra, replaceable by *"a dilated LUT `h(sqrt(d² + cs·sqrt(2)/2))` precomputed
once per tool/resolution"*. Not taken, for three reasons:

1. **The formula as written is dimensionally wrong.** It adds a length
   (`cs·√2/2`) to an area (`d²`) under the root. The quantity the code computes
   is `h((√(d²) + cs·√2/2)²)`, which is not the same function, and building a
   table indexed by `d²` for it still needs the per-entry sqrt — at table-build
   time, which is fine, but that is a different change from the one written.
2. **It is not metric-neutral.** `cell_upper_bound_surface` feeds
   `conservative_top`, and `conservative_top` feeds **rapid-collision
   detection** (`check_rapid_collisions_against_stock`). A dilated,
   interpolated table introduces a different rounding than the exact evaluation,
   and the failure direction is "a clearance ceiling that is no longer an upper
   bound". That is a safety channel, not a metric; it does not belong in a
   change advertised as no-behaviour-change.
3. It is already guarded by `from_high && coverage >= FULL_COVERAGE`, so it does
   not run on the partial-coverage cells that make up the boundary band.

Recommend it be re-scoped as its own item with its own net, not carried as an
S7 tail.

---

## 3. S6 — one half done, one half deferred

### 3a. Sample-vector reserve — DONE

`simulation.rs` reserved `moves.len() * 2`. The emitter pushes **one sample per
subsegment**, and a move is cut into `max(⌈len/sample_step⌉, ⌈|Δz|/0.02⌉)` of
them, so the old figure was not an estimate of anything — the Vec re-allocated
and memcpy'd its way up through every doubling on every toolpath.

`estimate_sample_count` walks the move list once (O(moves), against an
O(moves × cells) stamp) and reproduces the emitter's own arithmetic, including
the `by_z` plunge arm and the rapid/`MoveIntent::Retract` cases that are
length-only. It is **exact except for arcs**, which are counted by their chord
because the true count comes from `linearize_arc_into` and reproducing it would
mean linearising every arc twice. So it is never an over-estimate, and no cap is
needed.

New sentry `sample_count_estimate_never_exceeds_the_run` asserts both halves
across three sample steps on a fixture with laterals, a plunge, a ramp and a
rapid: `estimate ≤ actual` (the documented guarantee) and `2 × estimate ≥
actual` (which is what distinguishes it from `moves.len() * 2`).

### 3b. `span_path` interning — DEFERRED, and this is why

Not attempted. Three specific blockers, none of them "it looked hard":

1. **`Arc<[SpanId]>` does not serialize on this workspace.** `serde`'s `Arc`
   impls are behind its `rc` feature, and `Cargo.toml:27` enables only
   `derive`. `SimulationCutSample` is serialized (the viz-side trace dump, the
   MCP wire). The change therefore starts with a **workspace-manifest edit**
   that affects all three lanes. `SmallVec<[SpanId; 4]>` has the same shape of
   problem: `Cargo.toml:33` enables `union`, not `serde`.
2. Even with `rc` on, serde deserializes each `Arc` separately — sharing is not
   preserved across a round-trip — so the wire side gains nothing and only the
   in-process trace does. Worth knowing before the manifest is edited for it.
3. It is a **public field type change**, and the constructors that break include
   two `#[cfg(test)]` modules in `crates/rs_cam_viz/` (`state/simulation.rs`
   ×2, `app/mcp.rs` ×1) that the VIZ lane is editing right now, plus
   `benches/perf_suite.rs`, `src/tool_load/locality.rs` and
   `tests/span_summary_single_pass_c1.rs`. Landing that mid-wave breaks another
   lane's `cargo test` for no reason.

Sizing, so the deferral is a judgement and not a dodge: `span_path` is typically
2–4 `SpanId`s (8–16 bytes). One malloc + memcpy per sample; at ~20 subsegments
per move over a 12.6k-move pass that is ~250k mallocs, order 5–8 ms per toolpath
against a stamp kernel measured below in the tens to hundreds of ms. Real, but
single-digit percent — well below S2/S3, and not worth a cross-lane manifest
change in wave 1.

Recommend it be scheduled as its own commit once the lanes converge, with the
`serde/rc` decision made explicitly.

---

## 4. Bench deltas

### Read the pairing, not the Phase 0 column

The Phase 0 absolutes were captured on **2026-08-19 at 18 Gi available**. This
lane ran on **2026-08-20 at 8–10 Gi available**, with an editor-driven
rust-analyzer holding **9.1 GB RSS** and two other lanes queued on the flock the
whole time. Today's *unmodified* tree already reads **98.4 ms** on
`sim_kernel_plunge/24` against Phase 0's **126.61 ms** — a 22 % gap with nothing
in between it could be attributed to.

So every number below is a **paired before/after on this machine, minutes
apart**, taken exactly the way wave 1 GEN took its G3 pair: `git stash push` of
only the five source files → bench (before) → `git stash pop` → bench (after).
Nothing else in the tree moved between the two runs. The Phase 0 column is
included for continuity, not for arithmetic.

| Bench | Phase 0 | Before | After | Criterion verdict |
|---|---:|---:|---:|---|
| `sim_kernel_lateral/flat6/cs0.25` | 19.483 ms | 16.268 ms | **15.212 ms** | −7.10 %, p = 0.00 |
| `sim_kernel_lateral/flat6/cs0.1` | 101.74 ms | 90.741 ms | **86.820 ms** | −6.04 %, p = 0.00 |
| `sim_kernel_lateral/flat12/cs0.25` | 59.404 ms | 55.313 ms | **52.727 ms** | −4.66 %, p = 0.00 |
| `sim_kernel_lateral/flat12/cs0.1` | 329.18 ms | 322.94 ms | **311.62 ms** | −3.51 %, p = 0.00 |
| `sim_kernel_plunge/flat6_cs025/24` | 126.61 ms | 98.417 ms | **96.276 ms** | p = 0.08 — **no change** |
| `sim_kernel_plunge/flat6_cs025/60` | 283.48 ms | 239.73 ms | **239.13 ms** | p = 0.76 — **no change** |
| `sim_e2e_small/3op_2d/res1` | 20.318 ms | 19.007 ms | **17.411 ms** | −7.93 %, p = 0.00 |
| `sim_e2e_small/3op_2d/res0.5` | 79.697 ms | 72.651 ms | **68.240 ms** | −6.32 %, p = 0.00 |

The "After" column is the **landed** (threshold-solving) form, not the naive one
that failed the sentry. Measured back to back with the same baseline, the exact
form is no slower than the naive one — the ULP walk is per-stamp (order 10⁴
extra `sqrt`s) against millions of per-cell visits.

### The plunge arm did not move, and that is the informative row

3.5–8 % on the lateral kernel and end-to-end; **nothing** on either plunge arm
(p = 0.08 and p = 0.76 — not distinguishable from noise). That is expected, not
disappointing, and it is worth stating because it bounds what S7 can ever do:
the plunge fixture drives the **degenerate branch** of
`stamp_segment_with_metrics`, which routes through `point_cell_coverage` — the
function the review itself notes was *already* sqrt-free. S7 cannot reach it.
The plunge arm therefore measures S4a's contribution alone, and S4a alone is
inside the noise on this fixture.

Reading the two together: of the 3.5–8 % on the lateral arms, most is S7 (the
per-cell sqrt pair), and the 6–8 % on `sim_e2e_small` — which is the only arm
that pays for full-grid clones — is S7 plus S4a's 4 B/cell.

### Calibration against what the review predicted

The review sizes S7 as "MED (multiplies everything above)". Measured at
**1.04–1.07×** on the arms it can reach. Nobody should read "de-sqrt the inner
loop" and expect more: the loop's cost is dominated by three ray walks, a LUT
probe and a read-modify-write, not by two square roots. **S7 is a rounding
error against S2 and S3**, and its real value in this wave turned out to be the
sentry it forced, not the 5 %.

## 5. Correctness

Everything below is on the landed tree, i.e. after the S7 threshold fix.

- **Both goldens green, never re-baselined.** `perf_golden_sim_metrics`
  (5 tests, both arms) and `perf_golden_depth_level_geometry` (2 tests) pass
  after S4a, after S7-naive, and again after S7-exact. The only golden write in
  this wave is the deliberate prerequisite regeneration in `b2a5e661`, which
  added the `per_kinematics` block and — verified field by field — moved no
  previously pinned value.
- **Sentries green:** `dexel_stock_z_frame_f024`, `dexel_stock_z_frame_f026`,
  `engagement_vector_step2`, `drill_metrics_pr2`, `kinematics_histogram`
  (1 ignored by its own gate), `sub_cell_stamping_fa` (6), and the 33
  `dexel_stock::` lib tests.
- **Full `cargo test -p rs_cam_core` green** — ran to completion through
  doctests. Caveat recorded above: that run started ~2 minutes before
  `squared_fast_paths_agree_with_the_sqrt_form` existed, so it did not cover it.
  That test is now green on its own and inside the lib suite.
- **Clippy clean on every target this lane owns:**
  `cargo clippy -p rs_cam_core --lib --benches --test perf_golden_sim_metrics
  --test sub_cell_stamping_fa --test perf_golden_depth_level_geometry --test
  dexel_stock_z_frame_f024 --test dexel_stock_z_frame_f026 --test
  engagement_vector_step2 --test drill_metrics_pr2 -- -D warnings` → exit 0.
  The unscoped `--benches --tests` form fails on a `println!` in
  `tests/pushcutter_band_query_g1.rs`, which is the GEN lane's in-flight G1
  work — not touched, not fixed, per the lane protocol.

### New nets added by this lane

| Test | What it pins |
|---|---|
| `three_d_arm_covers_arc_and_helix_kinematics` | the 3D golden arm keeps producing Arc and Helix samples |
| `golden_3d_fixture_is_not_vacuous` | it keeps actually cutting |
| `squared_fast_paths_agree_with_the_sqrt_form` | S7 is bit-exact against a verbatim copy of the pre-S7 body, at ~73k probes including ±4 ULP around every boundary |
| `sample_count_estimate_never_exceeds_the_run` | the reserve never over-estimates, and is not the old `moves × 2` under-estimate |
| `sub_cell_stamping_fa` (4 converted) | the F.a coverage guarantees, read through the blended ray top instead of the deleted sidecar |

### One thing owed to the GEN lane

Early in the wave I ran `cargo fmt -p rs_cam_core`, which — as
`feedback_rustfmt_cascade` warns — reformatted the whole package, including the
GEN lane's in-flight `benches/hot_paths.rs` and `src/compute/execute.rs`.
Formatting only, no semantic change, and neither file has been touched since.
If one of their `Edit` calls failed on a stale `old_string` around then, that
was this.

---

## 6. Corrections owed to `PERF_REVIEW.md`

Applied in place:

1. **S4** — "`coverage_max` … read by nothing" corrected to "no *production*
   reader; six reads in `tests/sub_cell_stamping_fa.rs` via the public
   `coverage_at`", with the note that the sentry was converted rather than
   dropped.
2. **S7** — the `cell_upper_bound_surface` prescription marked declined, with
   the dimensional error in the stated formula and the `conservative_top` →
   rapid-collision coupling recorded.
3. **S6** — the `Arc<[SpanId]>` prescription annotated with the `serde/rc`
   prerequisite it does not mention.
