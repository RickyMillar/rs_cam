# DELTA — gen-w2b (push-cutter lane), finding G1

Lane: **PUSH-CUTTER**. Owned files: `crates/rs_cam_core/src/pushcutter.rs`,
`crates/rs_cam_core/src/waterline.rs`, `crates/rs_cam_core/src/mesh.rs`.
Baseline: `BASELINES.md` (Phase 0, captured 2026-08-19).

> **Status: numbers pending.** This file is written before the bench run so the
> shape of the claim is fixed in advance; the measured columns are filled in
> below and any prediction that did not survive is called out by name.

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

`band_query_actually_prunes` is the arm that matters most for a *perf* change:
without it, a band that silently degenerated back to the full grid would still
pass every equivalence test above. It is the `gate_population_vacuity_xvac`
lesson — a bar written as a comparison is vacuous until you have checked its
population — applied to a golden instead of a gate.

## Numbers

_(filled in from the bench run — see below)_

## What was NOT done, and why

_(filled in below)_

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
