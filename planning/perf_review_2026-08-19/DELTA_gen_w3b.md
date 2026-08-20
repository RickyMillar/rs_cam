# DELTA — gen-w3b (path-ordering lane), findings G5 and G6

Lane: **PATH-ORDERING**. Owned files: `crates/rs_cam_core/src/tsp.rs`,
`surface_link.rs`, `pencil.rs`, `adaptive3d/clearing.rs`, `ramp_finish.rs`,
`unified_finish.rs`, plus a new shared module
`crates/rs_cam_core/src/nn_order.rs` and its one-line declaration in
`lib.rs`.
Baseline: `BASELINES.md` (Phase 0, captured 2026-08-19).

**Result in one line: `gen_rapid_order/nn_seed/20000` 408.32 ms → 8.4260 ms
(−97.95%, 48.5×) and `/5000` 24.343 ms → 1.5858 ms (−93.03%, 15.4×), with the
scaling exponent moving 2.035 → 1.205 — and the emitted toolpath
bit-identical at every site, pinned against a pre-change A/B fingerprint.**

---

## G5 — what was actually there, and what the review got right

The review named six sites. Five of them really are the same loop; the sixth
is a different animal and is declined below with a measurement.

| Site | Query point | Candidates | Metric | Sentinel | Fallback when nothing clears it |
|---|---|---|---|---|---|
| `tsp.rs:309` (`run_tsp` seed) | previous segment's **end** | every segment's **start** | `sqrt` | `f64::INFINITY` | index `0` (re-pushed even if visited) |
| `surface_link.rs:249` | previous fragment's **exit** | every fragment's **entry** | `sqrt` | `f64::INFINITY` | `usize::MAX` → `break` |
| `pencil.rs:579` | previous chain's **last point** | **both endpoints** of every chain, `min` of the two | **squared** | `f64::MAX` | index `0` |
| `adaptive3d/clearing.rs:1368` | running cursor | region anchors | **squared** | `f64::INFINITY` | none — emit nothing, leave the cursor |
| `ramp_finish.rs:320` | each upper contour's centroid | lower contours' centroids | **squared** | `f64::INFINITY` | none — skip this upper contour |
| `unified_finish.rs:2434` | previous region's exit | every region | **not a distance** — see below | n/a | `break` |

Two things in that table matter more than the review's summary suggests, and
both are correctness constraints on any shared implementation:

1. **The four "sentinel + fallback" columns are all different.** Reproducing
   the tour is not just reproducing the argmin; a site that finds nothing has
   site-specific behaviour, and three of the five do something other than
   stop. `NearestPicker::nearest` therefore returns `Option<(owner, value)>`
   and applies **no threshold of its own** — each call site keeps its own
   `d < f64::INFINITY` / `d < f64::MAX` test and its own fallback, verbatim.
2. **`pencil` and `clearing` compare squared distances; `tsp` and
   `surface_link` compare the square root.** That is not interchangeable at
   the tie-break. `sqrt` is monotone but **not injective**: two different
   squared distances can compare *equal* after it, and then the index
   tie-break decides where a squared comparison would have picked a strict
   winner. So the shared type is parameterised by `Metric`, and the
   parameter is load-bearing rather than cosmetic.

### The tie-break, stated precisely

Every one of the five loops scans candidates in ascending index order and
keeps the **first strict improvement** (`if d < best`). That is exactly the
lexicographic minimum of `(distance, index)` — the *lowest index* wins an
exact tie. `NearestPicker::nearest` returns that lexicographic minimum
exactly. The grid is a pruning device and never a tie-break.

Three properties make that hold, and all three are pinned by tests:

- The ring early-out is **strict** (`lower_bound > best`, never `>=`).
  Stopping on `>=` would be sound for distance alone but not for the
  tie-break: equal-distance candidates genuinely can sit in *different*
  Chebyshev rings (a candidate at Euclidean distance `d` along an axis is one
  ring further out than one at the same `d` on the diagonal), so an unscanned
  candidate could tie the incumbent and win on index. Under `>` no unscanned
  candidate can even tie.
- The per-candidate value is computed with the same expression the
  hand-rolled loop used, selected by `Metric`.
- NaN is ordered **after** every real value and never prunes, reproducing
  `d < best` being false for NaN — including the all-NaN case, where the
  picker returns a NaN value that fails every call site's own sentinel and
  drops it into that site's fallback.

### The evidence, and its non-vacuity

`nn_order::tests` carries **verbatim copies of all five replaced loops** —
copied from the pre-G5 bodies with only the segment/fragment/path plumbing
reduced to raw coordinates — and asserts the **identical permutation**, not
an equal-length tour and not an equal total distance. Because the reference
implementations live in the test module rather than being re-derived from the
shipped code, they cannot rot into agreement.

Adversarial fixtures (16 of them, every one run against all five shapes):
empty, single, two identical points, a 40-point coincident cluster, collinear
in both directions, `9×9` / `20×20` / `1×400` lattices, signed zeros
(`+0.0`/`-0.0` in all four sign combinations), 64 points on a circle at
identical radius, a lattice plus a `1e7` outlier, NaN and ±∞ coordinates, a
600-point cloud quantised to a 0.5 mm grid, a 1200-point random scatter, and
the `gen_rapid_order` bench's own scatter. Plus `pencil`'s per-step chain
**reversal** decision, and `ramp_finish`'s extent test swept from "reject
everything" to "accept everything".

`reported_value_matches_the_hand_rolled_expression_bit_for_bit` compares the
returned value as `f64::to_bits`, never `==`, so neither a last-ULP
divergence nor `-0.0 == 0.0` can mask one.

**The net was checked for vacuity by mutation, not assumed.** Two mutants:

| Mutant | Result |
|---|---|
| Tie-break disabled (`owner < bo` → `false`) | **caught at all five sites**, first failure on the `9×9` lattice |
| Prune bound halved (`lb > bv` → `lb > bv * 0.5`) | **caught at all five sites plus** `ring_search_agrees_with_exhaustive_scan_under_deletion` |

A third mutant was considered and **not** run because it is unkillable by
construction and saying so is more useful than a green tick: changing the
early-out to `>=` cannot be distinguished from `>` on any realistic fixture,
because the bound already has a `cs · 1e-9` slack subtracted from it and
`lb == bv` is therefore a measure-zero coincidence. `>` is kept because it is
*provably* correct, not because a test proved it necessary.

### The algorithm

Uniform grid, CSR-packed (count → prefix-sum → fill; no `Vec<Vec<_>>` — the
pattern `PERF_REVIEW` G8 complains about in `mesh.rs` and the one
`chain_segments` already uses). Cell area targets one point per cell. A query
scans expanding Chebyshev rings from the query cell and stops at the first
ring whose geometric lower bound strictly beats the incumbent.

Three details worth recording because they are where this kind of structure
usually goes wrong:

- **Deletion.** Removing an owner marks it dead; the grid is **rebuilt from
  the survivors whenever the live count halves**. Without that, the endgame
  of a tour degenerates into scanning a mostly-dead grid — the classic
  failure mode that makes a "fixed" NN still quadratic. Rebuild cost
  telescopes to O(n).
- **Out-of-cloud queries.** The ring bound counts only the sides that
  actually have cells beyond them. Without that, a query outside the point
  cloud clamps to an edge cell and the bound goes non-positive, degenerating
  into a full scan on every step.
- **Non-finite coordinates** cannot be gridded, so they go in a separate
  stray list scanned unconditionally. Normally empty; it is what makes the
  NaN fixtures behave.

Below 32 live candidates the picker scans linearly, which is both the small-n
path and what keeps the tail of a tour cheap.

---

## G6 — `surface_link` provenance inversion

`surface_link.rs:391` sat inside the per-fragment emit loop and scanned all
`n_in` moves to find the ones whose owner was this fragment. The information
was already known: the pass at `:214` had just walked every move computing
`owner_of_rapid[i]`. Inverted to `rapids_of_frag[fi] -> Vec<rapid index>`,
built by the **same single O(n_in) walk** — only the storage shape changed.
The emit loop now iterates its own fragment's rapids.

Cost, counted rather than argued. Each rapid index has exactly one owner in
both formulations and every write assigns the same range, so the two are
identical by construction, but the iteration counts are not:

| | pre-G6 | post-G6 |
|---|---:|---:|
| iterations | `fragments × n_in` | `total rapids` |
| harness fixture (6,000 fragments, `n_in` = 41,999) | **251,994,000** | **11,999** |
| wanaka figure the review quotes (12,780 fragments, ~10⁵ moves) | **~1.28 × 10⁹** | **~2.6 × 10⁴** |

G6 has no output-ordering risk and landed as its own commit ahead of G5.

---

## Declined: `unified_finish::route_greedy`, with the measurement

The sixth site in the review's list is **not** a nearest-neighbour orderer and
cannot adopt a geometric picker without changing its output. Its per-candidate
cost is `choose_link` (`unified_finish.rs:2483`): a drop-cutter-sampled
surface drape, gouge-checked against the mesh, clipped to the machining
boundary, then costed through `machine_kinematics::compute_cycle_time` — an
accel-limited trapezoidal integration — and taken against the retract
alternative. That is a **link time in seconds**, not a distance, and it is not
monotone in XY distance. Ordering candidates by XY would produce a different
route.

And the quadratic it is part of is already bounded, by an existing regression
net rather than by an argument:
`tests/finish_planner_wanaka_decompose.rs:213` asserts
`planned.stats.region_count <= 24` on the wanaka reference workload, and
`:454` asserts `<= 100` across the whole conditioning dial sweep ("an O(100)
storm fails the sweep"). `route_greedy` runs over one `RegionPath` per planned
region, so `n ≤ 100` is a *tested* bound, not an estimate — `unified_finish.rs:1720`
says the same thing in prose ("O(1) regions per band on wanaka today"). This
is not the uncapped-quadratic class G5 names; the review's own framing
("removes the need for caps") does not apply to a site that already has one.

There **is** a sound fix here, and it is a different fix from G5's: prune
`choose_link` calls with the admissible bound `cost_s ≥ xy_distance / v_max`
(every candidate path traverses at least the straight-line XY gap, at no more
than the larger of `max_feed` and `rapid_feed`), enumerating candidates in
increasing XY distance and stopping when the bound strictly exceeds the
incumbent — with the incumbent compared as lexicographic `(cost_s, index)`
so the existing tie-break survives. That needs a distance-**ordered
enumerator**, not a single-nearest query, and it is worth doing only if
`choose_link` shows up in a profile at `n ≤ 100`. Recorded as follow-up, not
taken.

`ramp_finish::match_contours` **was** adopted, but it is worth flagging that
it is also not a tour: it is a greedy bipartite match whose nearest candidate
may then be **rejected** by the extent test and must stay available for the
next query. The picker is used there as a pure query with deletion driven by
acceptance rather than by selection — which the shared type supports because
`nearest` and `remove` are separate operations.

---

## Numbers

### The scaling exponent — the actual deliverable

`cargo test --release -p rs_cam_core --test nn_order_scaling_g5_g6 -- --nocapture`,
measured as a **same-session A/B**: `git stash push` of *only* `tsp.rs` +
`surface_link.rs` → run (before) → `git stash pop` → run (after). Nothing else
in the tree moved between the two, and neither run touched `lib.rs` (which
carries another lane's `pub mod geom_cache;` hunk — see "Files" below).

The harness reports the empirical exponent `log(t₂/t₁) / log(n₂/n₁)`.

| Arm | n₁ → n₂ | Before | After | Exponent before | Exponent after |
|---|---|---:|---:|---:|---:|
| `tsp::optimize_rapid_order` | 5,000 → 20,000 segments | 25.586 → 425.760 ms | 2.947 → 13.059 ms | **2.028** | **1.074** |
| `surface_link::relink_fragments` (reorder on) | 1,500 → 6,000 fragments | 15.421 → 130.466 ms | 1.901 → 10.421 ms | **1.540** | **1.227** |

Per-point speed-ups: tsp **8.7× at 5k** and **32.6× at 20k**; relink **8.1× at
1.5k fragments** and **12.5× at 6k**.

**The exponent is the result, not the absolutes.** `tsp` reading 2.028 before
and 1.074 after is the quadratic being removed, measured on the same machine
minutes apart. A single absolute would not have shown that, and on this
contended box the absolutes wander by ±40% between runs (the "after" tsp/5000
arm read 4.601 ms on one run and 2.947 ms on the next) while the exponent
held.

Two sanity checks on the harness itself:

- Its **before** numbers reproduce the Phase 0 criterion baseline almost
  exactly — 25.586 / 425.760 ms against `BASELINES.md`'s 24.343 / 408.32 ms —
  despite being a plain `Instant` loop rather than criterion. So the harness
  is measuring the same thing the bench is.
- `BASELINES.md` records the before ratio as **16.8× for 4× the segments**.
  This harness measured **16.6×** on the same fixture. After the fix it is
  **4.43×** against a 4× size increase, i.e. essentially linear.

`relink`'s before-exponent is 1.540 rather than 2.0 because its cost is a
*mix*: the quadratic ordering scan and the quadratic provenance rescan sit on
top of genuinely linear emission and surface-link work, and over this size
range the linear part still dilutes the measurement. That is a property of
the fixture, not a weaker finding — the same mix at 12,780 fragments is where
the review's ~10⁹ figure comes from.

### Criterion, `gen_rapid_order`

`cargo bench -p rs_cam_core --bench hot_paths -- gen_rapid_order`, against the
Phase 0 absolutes in `BASELINES.md` (same bench, same fixture, unchanged).

| Bench | Before (Phase 0) | After | Change | Speed-up |
|---|---:|---:|---:|---:|
| `gen_rapid_order/nn_seed/5000` | 24.343 ms | **1.5858 ms** [1.5147, 1.7538] | −93.03% | **15.4×** |
| `gen_rapid_order/nn_seed/20000` | 408.32 ms | **8.4260 ms** [8.3200, 8.5045] | −97.95% | **48.5×** |

Criterion's own verdict on both: `Performance has improved`, p = 0.00. Its
stored baseline was the Phase 0 capture (24.418 / 408.32 ms), so the
percentages are a like-for-like comparison and not a re-baselined number.

**Scaling — the row that carries the finding:**

| | 4× the segments (5k → 20k) costs | implied exponent |
|---|---:|---:|
| before (`BASELINES.md`) | **16.8×** | **2.035** |
| after | **5.31×** | **1.205** |

That is the quadratic gone, measured on the bench the review nominated. The
residual 1.2 is the linear work the bench also carries — the `tp.clone()` per
iteration, `split_into_segments`, and the rebuild/remap — plus the 2-opt
refinement, which is untouched and still runs at n ≤ 500.

Note the criterion "after" absolutes (1.59 / 8.43 ms) are faster than the
scaling harness's (2.95 / 13.06 ms) because criterion reports a steady-state
mean over thousands of warm iterations while the harness times a single run.
The two agree on direction and exponent, which is what is being claimed.

### G6, counted rather than timed

The `fragments × n_in` → `total rapids` inversion on the harness's own
fixtures:

| fragments | `n_in` | pre-G6 iterations | post-G6 iterations | ratio |
|---:|---:|---:|---:|---:|
| 1,500 | 10,499 | 15,748,500 | 2,999 | 5,251× |
| 6,000 | 41,999 | 251,994,000 | 11,999 | 21,001× |
| 12,780 (the review's wanaka figure) | ~10⁵ | ~1.28 × 10⁹ | ~2.6 × 10⁴ | ~5 × 10⁴× |

These are exact counts derived from the loop bounds, not estimates: the old
loop ran `old_to_new.len()` iterations per fragment; the new one runs one per
rapid the fragment owns.

## Correctness

**Every fingerprint held. The emitted toolpath did not move, at any site.**

| Net | Result |
|---|---|
| `perf_golden_sim_metrics` (5 tests) | green |
| `perf_golden_depth_level_geometry` (2 tests, incl. the FNV move-list golden) | green |
| `finish_resolution_policy_pr3` (10, incl. `steep_shallow_fingerprint`, `ramp_finish_fingerprint`, `scallop_fingerprint`) | green |
| `crease_own_region_pr6b` (3, incl. `production_unified_finish_output_is_byte_identical`) | green |
| `checkpoint_b_resolution_ab` (8) | green |
| `transform_provenance_fingerprints` (3) | green |
| `cargo test -p rs_cam_core --lib` | **2,331 passed, 0 failed** |
| `nn_order::tests` (8 permutation-equality tests) | green |
| `nn_order_scaling_g5_g6` (3) | green |

`finish_resolution_policy_pr3::ramp_finish_fingerprint` is worth naming
separately: it is the sentry that would fail if the `match_contours` rewrite
had changed a single contour pairing, and `crease_own_region_pr6b::production_unified_finish_output_is_byte_identical`
covers the pencil and scallop paths end to end.

The strongest single piece of evidence is the A/B fingerprint pinned in
`nn_order_scaling_g5_g6::g5_g6_relink_output_and_provenance_fingerprint`. Its
constants were **captured from the pre-change tree during the stash A/B**, not
blessed from this change's own output:

| arm | moves | FNV-1a over `(moves, provenance)` |
|---|---:|---|
| `reorder = false` | 840 | `fd019db23752f9ca` |
| `reorder = true` | 837 | `836314d3c7a91e97` |

The `reorder = true` arm is the one that matters — it is where G5's tour
lives — and it came back **byte-identical**, provenance included. Note the
fingerprint covers the `MoveProvenance` as well as the moves, so G6's
inversion is pinned on the channel it actually rewrote rather than only on
the emitted geometry.

Because nothing moved, the "if the fingerprint moves, show the total rapid
distance is equal or better" obligation does not arise. Had it arisen the
honest answer would have been to re-baseline deliberately rather than absorb
it here.

`cargo clippy -p rs_cam_core --benches --tests -- -D warnings`: **clean for
every file in this lane.** Two `int_plus_one` findings in `nn_order.rs` were
fixed (`cx >= r + 1` → `cx > r`). The run does not exit zero overall, but the
three remaining errors are all in another lane's in-flight `geom_cache.rs`
(the G8 lane), not in anything this wave touched.

`rustfmt --edition 2024 --config skip_children=true` on each owned file.
`skip_children` matters: `tests/nn_order_scaling_g5_g6.rs` `#[path]`-includes
`tests/common/fingerprint.rs`, and formatting children would have rewritten a
file shared with thirteen other sentries.

## Files

Owned and changed:

- `crates/rs_cam_core/src/nn_order.rs` (new, shared)
- `crates/rs_cam_core/src/tsp.rs`
- `crates/rs_cam_core/src/surface_link.rs`
- `crates/rs_cam_core/src/pencil.rs`
- `crates/rs_cam_core/src/adaptive3d/clearing.rs`
- `crates/rs_cam_core/src/ramp_finish.rs`
- `crates/rs_cam_core/tests/nn_order_scaling_g5_g6.rs` (new)

Not owned, one line touched: `crates/rs_cam_core/src/lib.rs` gains
`pub(crate) mod nn_order;`. That file **also** carries another lane's
`pub mod geom_cache;` hunk in the same working tree, so it was staged
hunk-by-hunk (`git apply --cached` on a single-hunk patch) rather than with
`git add`, and the other lane's line is left unstaged.

`crates/rs_cam_viz` needed no change: `nn_order` is `pub(crate)` and every
call site is inside `rs_cam_core`.

## What this lane did NOT do

- **Did not touch `benches/hot_paths.rs`** (SIM lane owns it). No new bench
  arm was needed — `gen_rapid_order` already covers the headline site, and the
  `surface_link` arm the review asked about does not exist there, which is why
  the scaling harness above is a `tests/` file instead.
- **Did not adopt `unified_finish::route_greedy`** — declined above, with the
  `region_count <= 24 / <= 100` bound as the measurement.
- **Did not remove `MAX_2OPT_SEGMENTS`.** The review says the shared orderer
  "removes the need for the caps", and that is true of the *seed* — but the
  cap being discussed guards the **2-opt refinement**, which is O(N³) and
  untouched by anything here. Removing it would reintroduce the hang it was
  added to prevent. The seed never had a cap and still does not need one.
- **Did not edit `compute/execute.rs` or `compute/config.rs`.** Both were read
  as context (the reachability branch at `execute.rs:3183`, the default at
  `config.rs:1703`) and confirm the review's account of how DropCutter arrives
  as a single group; neither needed changing.

## Correction to `PERF_REVIEW.md`

One claim in G5 as written is wrong and has been corrected in place (edit left
**unstaged**, per the wave protocol — other lanes have hunks in that file):

> `ramp_finish.rs:320`, unified_finish routing … one shared
> `nearest_neighbour_order` … six sites collapse to one.

`ramp_finish.rs:320` is a bipartite match, not a tour (it adopted the shared
type but with acceptance-driven deletion), and `unified_finish.rs:2434` is not
a distance-ordered problem at all. The honest count is **five sites collapse
to one, and the sixth is a different finding**.

