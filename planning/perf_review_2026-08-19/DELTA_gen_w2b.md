# DELTA — gen-w2b (push-cutter lane), finding G1

Lane: **PUSH-CUTTER**. Owned files: `crates/rs_cam_core/src/pushcutter.rs`,
`crates/rs_cam_core/src/waterline.rs`, `crates/rs_cam_core/src/mesh.rs`.
Baseline: `BASELINES.md` (Phase 0, captured 2026-08-19).

**Result in one line: `gen_waterline/L20` 90.187 ms → 31.828 ms (−64.7%, 2.83×)
in a same-session A/B, and 97.842 ms → 31.828 ms (−67.5%, 3.07×) against the
Phase-0 baseline. `push_cutter_batch/terrain_200fibers` 75.326 ms → 16.689 ms
(−77.9%, 4.51×). Output bit-identical.**

## What G1 said, and what was actually there

`pushcutter.rs:43` sized its spatial-index query by the fiber's **length**:

```rust
let half_len = fiber.length() / 2.0;
let query_r = half_len + r;
let candidates = index.query(cx, cy, query_r);
```

That is a *square* of half-side `fiber_length/2 + r` centred on the fiber's
midpoint. Waterline fibers are built from the mesh bbox
(`waterline.rs:282-301`), so the square covers the entire index and the query
returns **every triangle in the mesh** — the index pruned literally nothing.
Confirmed as a measured assertion, not an inference:
`band_query_actually_prunes` asserts `old.len() == mesh.faces.len()` on the
bench terrain, so if that ever stops being true the sentry says so.

The consequence the review names is the transient allocation (a 5.3 MB `Vec`
and an 82 KB bitset per fiber on the 661 k-triangle terrain). That is real, but
it is **not the expensive half**. The expensive half is that every triangle the
query returned then went through the per-triangle Z reject, and every triangle
that survived the Z reject went through the full contact math — three vertex
pushes, a facet push, and three edge pushes, each edge push costing 9 coarse
evaluations plus up to 10 bisection evaluations. On a height field at
mid-section roughly half the mesh sits above the plane and passes the Z reject,
so the contact math ran over half the mesh *per fiber*, at every Z level.

This matters for reading the result: `CLASSIFICATION_PERF_STUDY.md` measured
the "per-query allocation" hypothesis at **0.99×** and the lane brief warns not
to spend the wave on allocator tuning. That warning is correct and was
followed — the win claimed here is pruning of contact math, and the buffer
reuse is a by-product that is not credited with any of it.

## The fix

Three changes, all in owned files.

**1. `mesh.rs` — `SpatialIndex::query_rect_into`.** The existing
`query`/`query_into` take a *square* window around a centre, which is the right
shape for a cutter at one XY point and the wrong shape for a long thin fiber.
`query_rect_into` states the two axes independently. `query_into` now delegates
to it with `(cx±r, cy±r)`, so the three query paths share one implementation and
cannot drift — including the pre-existing `x1 as usize` wrap on a negative
index, which is reproduced verbatim rather than "fixed" (it is over-inclusive,
not wrong, and three callers depend on the same answer). `rect_query_matches_square_query`
pins rect ≡ square ≡ allocating-`query` across on-grid, off-grid and
zero-radius windows.

**2. `pushcutter.rs` — the band query.** The query window is now the fiber's own
XY segment bbox inflated by `fiber_lateral_reach_mm(cutter)`. For an X-fiber at
row `y` that is the band `y ± reach`; the X extent is unchanged, because
`[x_min - reach, x_max + reach]` is exactly what the old square already gave
*along* the fiber. Only the perpendicular axis tightens — which is the axis the
old formula got wrong.

`fiber_lateral_reach_mm` is `max(envelope_radius_mm, xy_normal_length +
normal_length) + PUSH_QUERY_SLACK_MM`. The first term bounds
`width_at_height`, which is what `vertex_push` and `edge_push_single` compare
their perpendicular distance against; the second bounds the facet contact's XY
offset `r1·n̂_xy + r2·n`. The two are equal for every shipped shape (flat `R+0`,
ball `0+R`, bullnose `r1+r2 = R`, V-bit `R+0`, tapered ball `0 + r_ball ≤ R`)
but the `max` is taken rather than assumed, and a `debug_assert` samples the
profile to check the envelope contract instead of trusting it.

**3. `pushcutter.rs` — an exact XY reject per candidate.** The index query is
quantised to whole cells, so it admits up to one cell row of slop either side
of the band. `Triangle::bbox` is already computed at construction, so a
four-compare exact reject removes that slop before the contact math. Same bound
as the query window, so it can only drop candidates the contact tests would
have rejected anyway.

### Why `PUSH_QUERY_SLACK_MM = 1e-3` and not zero

The band bound is exact in real arithmetic. It is **not** exact in the
arithmetic the contact tests actually use, and a bound tight to the last ULP
would have turned a pruning change into a different answer:

| site | its own slack | what a tight band would have changed |
|---|---|---|
| `vertex_push`, `edge_push_single` | rejects at `perp_dist > w + 1e-10` | a vertex exactly `R` away is **accepted** and contributes a zero-width interval |
| `facet_push` | accepts `t ∈ -1e-8 ..= 1+1e-8` | contact can land `1e-8 · fiber_length` beyond an endpoint |
| `Triangle::contains_point_xy` | barycentric `≥ -1e-8` | accepts a point `1e-8 · triangle_scale` outside the triangle |

1e-3 mm clears all three by orders of magnitude and costs nothing measurable:
the band is `2R` wide, so a Ø6 cutter's band grows 0.03%, and the index
quantises to cells ≥ 0.1 mm, so on almost every query the slack does not move
the cell range at all.

This is the wave-1 G3 lesson applied in advance. G3 shipped `bbox.max.z <= cl.z`
as a "provably sound" reject; it was systematically wrong for every triangle
sharing a flat-tip contact vertex, and the only thing that caught it was a
fingerprint sentry showing an identical move count and a different hash.

## Correctness evidence

There is no golden pinning waterline output, so the lane built its own:
`crates/rs_cam_core/tests/pushcutter_band_query_g1.rs`. It re-implements the
**pre-G1 unbounded query verbatim** and asserts the new path matches it
**bit-for-bit** — interval endpoints compared as `u64` bit patterns (so a
last-ULP divergence cannot hide behind a tolerance and `-0.0 == 0.0` cannot
mask one), then the woven contours compared the same way, then an FNV-1a
fingerprint over the whole contour set on top.

| test | what it covers |
|---|---|
| `band_query_matches_unbounded_on_hemisphere_every_shape` | all five shipped cutter shapes (flat, ball, bullnose, V-bit, tapered ball) × 15 Z levels |
| `band_query_matches_unbounded_on_rolling_terrain` | the `gen_waterline` bench fixture's own mesh class |
| `band_query_matches_unbounded_on_exact_cell_boundaries` | grid-aligned mesh, cell size dividing the vertex pitch, fibers **exactly on cell edges**, reach an exact cell multiple |
| `band_query_matches_unbounded_on_half_cell_offset_fibers` | the complement — every band edge falls mid-cell |
| `band_query_matches_unbounded_on_long_thin_mesh` | a mesh 25× longer than it is wide; catches a band that kept the fiber-length term on the perpendicular axis |
| `band_query_actually_prunes` | non-vacuity: the old query really did return the whole mesh, the new one returns a small fraction, **and** every triangle within a cutter radius of the fiber row survives |
| `rect_query_matches_square_query` | `query_rect_into` ≡ `query_into` ≡ `query` on square windows, on- and off-grid |

Every Z ladder includes the degenerate ends: exactly `bbox.max.z`, exactly
`bbox.min.z`, ±1 ULP-ish either side of each, and planes a full mesh height
above and below.

### Sentries run

`cargo clippy -p rs_cam_core --benches --tests -- -D warnings` clean.

17 integration targets, **81 passed / 0 failed**: the two Phase-0 goldens
(`perf_golden_sim_metrics`, `perf_golden_depth_level_geometry`), the new
`pushcutter_band_query_g1`, and every waterline / steep-shallow / unified-finish
/ scallop harness this lane could find —
`waterline_shared_finish_setup_c3`, `steep_shallow_min_segment_pr8d`,
`finish_resolution_policy_pr3` (**three pinned `(moves, hash)` constants** —
this is the sentry class that caught wave-1's G3),
`checkpoint_b_resolution_ab`, `crease_own_region_pr6b`,
`unified_finish_{dropped_band_finding_d1, partial_clip_finding_c8,
semantic_regions, tapered_end_to_end_m21}`,
`scallop_{isofield_gouge_m4, candidates_m4}`,
`shallow_band_stock_to_leave_exhibit_d16_2`,
`band_run_off_reproduction_d16_1`, `adaptive3d_interior_cell_parity_f029`.

`band_query_actually_prunes` is the arm that matters most for a *perf* change:
without it, a band that silently degenerated back to the full grid would still
pass every equivalence test above. It is the `gate_population_vacuity_xvac`
lesson — a bar written as a comparison is vacuous until you have checked its
population — applied to a golden instead of a gate.

## Numbers

### Method

The Phase-0 baselines were captured on 2026-08-19, and three other lanes have
landed changes to this crate since (`ca92d767` G3, `473c3d1f` G2, `08345ee4`
V8/V13), so a straight comparison against `BASELINES.md` cannot separate this
lane's effect from theirs or from a different machine load. The primary number
below is therefore a **same-session A/B**: within one hold of the build lane,
`git checkout HEAD --` on *only* the two files this lane owns
(`pushcutter.rs`, `mesh.rs`), bench, restore, bench again. Nothing else in the
tree moved between the two arms, and the two runs are six minutes apart.

The `BASELINES.md` comparison is reported alongside for continuity, and the
gap between the two "before" readings (90.187 vs 97.842 ms, 8%) is itself worth
knowing: it is what machine state plus three unrelated lanes is worth on this
bench, which is roughly the size of a small win. Read the A/B column.

### Same-session A/B — `hot_paths`, `gen_waterline`

| arm | before (HEAD) | after (G1) | change | speedup |
|---|---:|---:|---:|---:|
| `rolling61_ball6/L1` | 303.84 µs | **88.075 µs** | −69.7% | **3.45×** |
| `rolling61_ball6/L20` | 90.187 ms | **31.828 ms** | −64.7% | **2.83×** |

Criterion `p = 0.00` on both, "Performance has improved."

### Same-session A/B — `perf_suite`, `push_cutter_batch`

| arm | before (HEAD) | after (G1) | change | speedup |
|---|---:|---:|---:|---:|
| `hemisphere_200fibers` | 12.769 ms | **6.4894 ms** | −58.9% | **1.97×** |
| `terrain_200fibers` | 75.326 ms | **16.689 ms** | −77.9% | **4.51×** |

Two caveats on this group, both of which make it the *pessimistic* read:

- The hemisphere arm is **noisy** — the after estimate is
  [4.4126, 9.2091] ms with one high-severe outlier among ten samples, so the
  honest span is 1.4×–2.9× rather than a point 1.97×. The before arm was tight
  ([12.509, 13.057]). The terrain arm is tight on both sides.
- Both arms build their index with `SpatialIndex::build(&mesh, 10.0)` — a
  hand-picked 10 mm cell, clamped down to `max_extent / 4` — not the
  density-aware `build_auto` that the `gen_waterline` arm and the shipped
  generators use. A coarse grid is exactly the condition under which a band
  query has the least to prune, because the band cannot be narrower than one
  cell row. The 4.51× on the terrain arm is what the band achieves *despite*
  that.

### Against `BASELINES.md` Phase 0 (2026-08-19)

| arm | Phase 0 | now | change | speedup |
|---|---:|---:|---:|---:|
| `gen_waterline/rolling61_ball6/L1` | 407.18 µs | 88.075 µs | −78.4% | 4.62× |
| `gen_waterline/rolling61_ball6/L20` | 97.842 ms | 31.828 ms | −67.5% | **3.07×** |

`push_cutter_batch` had no stored Phase-0 estimate in `target/criterion` when
this lane started (the directory holds only the `hot_paths` groups), which is
why the A/B above captures its own before arm rather than citing one.

### The L20/L1 ratio went UP, and that is the correct outcome

| | before | after |
|---|---:|---:|
| `L20 / L1` | 297× | **361×** |

`BASELINES.md` warns that this ratio is not a repetition factor, and this is
the demonstration. G1 is a **pruning** fix: it makes each level's fiber batch
cheaper in proportion to how much mesh the band excludes. The L1 arm sits at
the mesh's maximum Z, where almost nothing is in section and almost nothing
survives the per-triangle Z reject — so the *old* code was already doing
little contact math there and the band's saving is mostly the index walk,
while at L20 the band also removes contact math. Both arms got much faster;
L1 got faster by more, so the ratio rose.

Anyone quoting "the ratio didn't fall, so G1 didn't work" would have it exactly
backwards. A fix that moved *that* ratio would be a per-level hoist (G2's
shape), which is the other lever discussed below and was not taken.

## What was NOT done, and why

**Lever 2 — "compute per-row candidates once and reuse them across Z levels" —
was not implemented.** The review names it alongside the band query, and it is
sound; it is just second-order once the band is in, and the restructure is not
free.

Reuse can remove only the **query** half of a level's cost. The contact math is
Z-dependent by construction (the whole point of a Z level is that a different
part of the mesh is in section) and has to run at every level regardless. So
the ceiling on lever 2 is `query_share × (1 − 1/levels)`. That is measurable,
and it was measured rather than argued —
`measure_query_vs_contact_split` in the test file (`#[ignore]`d; run it with
`cargo test -p rs_cam_core --release --test pushcutter_band_query_g1 --
--ignored --nocapture`), on the `gen_waterline` fixture, 20 levels:

```text
G1 lever-2 headroom: query 11.611173ms, full 386.776001ms, query_share 3.0%
(candidates 1248000, intervals 988); ceiling on reuse across 20 levels = 2.9%
```

**Lever 2 is worth at most 2.9% now that lever 1 has landed**, and only 2.9%
because 20 levels is near the asymptote — the `(1 − 1/L)` factor is already
0.95. That is the whole case for not taking it: it is a real optimisation of a
term that lever 1 shrank to a rounding error.

The same run gives the pruning ratio directly: **1,248,000 candidates** over
20 levels × 48 fibers is **1,300 per fiber**, against 7,200 — the whole mesh —
before. 5.5× fewer, which is the shape of the 2.83× wall-clock (the survivors
are disproportionately the ones that go on to do contact math).

Two further costs that would argue against lever 2 even if the 2.9% were
tempting:

- It needs a multi-Z entry point (`waterline_contours` is per-Z and is called
  from `steep_shallow.rs:218`, `ramp_finish.rs:622` and
  `adaptive3d/clearing.rs:1236` — three files this lane does not own), so the
  API churn escapes the lane boundary.
- Cancellation granularity gets **coarser**, not finer:
  `waterline_toolpath_with_cancel` currently checks cancel once per Z level, and
  a fiber-outer/Z-inner loop cannot. That is a real user-visible regression on
  a long generate, and it would have to be bought back with per-row-chunk
  checks.

`waterline.rs` is therefore unchanged by this lane.

**Allocator tuning was not attempted**, per the brief. The per-worker buffer
reuse in `batch_push_cutter_with_cancel` is a by-product of adopting
`query_into`/`QueryScratch` (which the review lists under "Already good — gap
is adoption") and is **not credited with any of the measured win**;
`CLASSIFICATION_PERF_STUDY.md` measured that hypothesis at 0.99× and this lane
did not re-test it.

## Corrections to `PERF_REVIEW.md`

None. G1's diagnosis was accurate in every particular that this lane could
check, including the claim that the index prunes *nothing* — which is now an
assertion in `band_query_actually_prunes` (`old.len() == mesh.faces.len()`),
not a reading.

One refinement worth carrying forward, which is an addition rather than a
correction: the review frames the cost as "~26e9 triangle tests and ~200 GB of
transient allocation", i.e. it leads with the allocation. The allocation is
real but it is the cheaper half — `CLASSIFICATION_PERF_STUDY.md` already
measured that hypothesis at 0.99×. The expensive half is that every triangle
surviving the per-triangle Z reject then ran the full contact math (three
vertex pushes, a facet push, three edge pushes, each edge push 9 coarse
evaluations plus up to 10 bisections). On a height field at mid-section that
is roughly half the mesh, per fiber, per level. That is why the win is 2.8–4.5×
rather than the few percent an allocation fix would have bought.

**This lane made no edit to `PERF_REVIEW.md`.**

## Proposed bench arm (for the orchestrator to land)

`benches/hot_paths.rs` is held by the SIM lane, so this lane did not touch it.
BASELINES.md asks for an arm that holds the Z band fixed and varies only the
step; here it is, ready to paste into `bench_gen_waterline` after the existing
`for levels in [1_usize, 20]` loop:

```rust
    // ── G1's clean arm: fixed Z band, varying step ──────────────────────
    //
    // The `L1`/`L20` pair above cannot be read as a repetition factor. Its
    // L1 arm sits at the mesh's maximum Z, where almost nothing is in
    // section, so the 240× conflates *how many* levels with *how much
    // material each level intersects* (BASELINES.md, "G1 — read this ratio
    // with care"). These arms hold the band fixed at the middle 50% of the
    // mesh — where every level cuts a comparable amount of section — and
    // vary only the step, so `steps16 / steps2` IS a repetition factor and
    // should read ≈ 8 both before and after any pruning fix. A pruning fix
    // moves the ABSOLUTES and leaves the ratio alone; a per-level-hoist fix
    // (G2's shape) would do the opposite. Having both arms makes the two
    // kinds of win distinguishable, which the L1/L20 pair cannot do.
    let band_top = bottom + span * 0.75;
    let band_bottom = bottom + span * 0.25;
    for levels in [2_usize, 4, 16] {
        let z_step = (band_top - band_bottom) / (levels as f64 - 1.0);
        // The ladder must really carry `levels` planes, or the arm is
        // measuring a different repetition factor than its name claims.
        assert_eq!(
            rs_cam_core::waterline::waterline_z_levels(band_top, band_bottom, z_step).len(),
            levels,
            "fixed-band arm did not produce {levels} planes"
        );
        group.bench_function(
            BenchmarkId::new("rolling61_ball6_band", format!("steps{levels}")),
            |b| {
                b.iter(|| {
                    let tp = waterline_toolpath_with_cancel(
                        &mesh,
                        &index,
                        &ball,
                        band_top,
                        band_bottom,
                        z_step,
                        &params,
                        None,
                        &never_cancel,
                    )
                    .unwrap();
                    black_box(tp.moves.len())
                })
            },
        );
    }
```

## Files touched

- `crates/rs_cam_core/src/mesh.rs` — `query_rect_into` added; `query_into`
  delegates to it.
- `crates/rs_cam_core/src/pushcutter.rs` — band query, `FiberWindow`,
  `fiber_lateral_reach_mm`, `PUSH_QUERY_SLACK_MM`, per-worker buffer reuse,
  `push_cutter_fiber_over` split out.
- `crates/rs_cam_core/tests/pushcutter_band_query_g1.rs` — new.
- `crates/rs_cam_core/src/waterline.rs` — **unchanged** (see "What was NOT
  done").
