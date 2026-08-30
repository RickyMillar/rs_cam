# Conformal-spiral arm (F2 phase 1) — findings

> ## ⚠ §F2-1's SPACING AND TIME NUMBERS ARE WITHDRAWN (2026-08-30, same day)
>
> An operator looked at the two SVGs and said they "don't look how you would
> expect". They were right, and the defect is in the **fixture**, not the
> algorithm. Measured after the fact:
>
> | quantity | value |
> |---|---|
> | equal-cusp stepover being measured | **0.486 mm** |
> | median triangle edge, chosen flat window | **1.42 mm** (2.9× the stepover) |
> | median triangle edge, whole `terrain_small.stl` | 0.80 mm (1.6× the stepover) |
> | region boundary loop | **30 vertices** for a 24×18 mm ellipse ⇒ ~4.6 mm segments |
>
> **You cannot measure 0.486 mm ring spacing on a mesh whose facets are
> 1.42 mm across.** Everything in §F2-1 that depends on fine geometry is
> therefore an artifact of faceting:
>
> - measured spacing (median 0.2275 mm, "79.3 % outside the band") — the
>   rings are riding facets, not a surface;
> - the "≈2× over-cover" and the **3.4× slower than raster** headline;
> - the curvature census (|κ| median 0.516 1/mm ⇒ R ≈ 1.94 mm) — that is
>   the *faceting* radius, not the terrain's curvature;
> - region 3D area 694.6 mm² vs 339.3 mm² projected (2.05×) — facet
>   roughness at the 1.4 mm scale, which is also why the spiral's distance
>   looks inflated against an XY-spaced raster.
>
> **A selection bias made it worse, and it was mine.** The flat-window
> census picks the flattest patch — and a decimated terrain mesh puts its
> *largest* triangles exactly where the surface is flattest. The census
> therefore steered the measurement into the coarsest region of the mesh
> (0.876 mm² median facet vs 0.277 mm² mesh-wide). The visible symptoms:
> the outer rings' 2.4 mm jumps (all at path indices < 600, swinging
> r = 1.2 → 0.8 across the ellipse edge) and points up to 3.27 mm outside
> the ellipse are the coarse 30-vertex boundary loop, not a path defect.
>
> **The operator's second observation — "way more dense around the outside
> than the inside" — is a real signal and is measured**: 50 ring radii with
> **median 0.665** (an even spread would sit at 0.500), so rings pile up
> toward the rim. On a flat disk the exact map is `z/ρ`, so equal 3D
> spacing *is* equal disk spacing and the median must be ≈0.5; the observed
> outward bias is therefore a defect signature, not geometry. Mechanism:
> the coverage search requires **one** ring to sweep *everything* still
> uncovered outside its radius, so the step is set by the single worst
> point. Near the rim the disk circle maps onto that 30-vertex, ~4.6 mm
> boundary polygon (with excursions to 3.27 mm outside the ellipse), so the
> worst point is wildly far, the step collapses, and rings crowd the rim.
>
> A related figure worth pinning for the re-run: measured median spacing was
> **0.2275 mm against a 0.4862 mm target — almost exactly half**, and the
> half-width 0.243 mm is precisely the lateral distance at which a
> `K_c = 1.0` ball stops covering the `h = 0.03` iso-scallop surface. So
> "each ring spaced by the HALF-width" is a specific, falsifiable
> alternative to the distortion story: it would mean the search is not
> crediting the band the *previous* ring already covered. Against it: the
> module's flat-disk unit test asserts stepover within ±25 % and passes on
> the exact-affine fixture. The clean fixture will separate these two —
> if analytic-fixture spacing lands on 0.486 the terrain number was
> faceting; if it lands on 0.243 there is an off-by-one-band defect in the
> ring recursion.
>
> **A second, independent fairness defect** in the cost table, which
> survives any fixture change: the spiral spaces on the **3D surface**
> while `raster_candidate` spaces in **XY projection**. On sloped ground
> the raster's achieved surface spacing is `0.4862 / cos θ` — it
> under-covers where the spiral over-covers, so the two arms were never
> delivering the same finish and the ratio compared two different measures
> ([[instrument-integrity]]: both sides of a ratio must be the same
> measure).
>
> **What still stands** (structural, resolution-independent): the spiral is
> genuinely one continuous path with zero retracts and zero disk-domain
> self-intersections; bridging works and its overhead is bounded; the
> fold diagnosis and the mean-value/Tutte fix; `N_C` halving breaking the
> mechanism outright. **What is withdrawn**: every spacing figure, the
> cost table, the "3.4× slower" verdict, and the recommendation built on
> it.
>
> **Root cause of the fixture choice:** PROGRAMME.md F2 step 1 says
> "simply connected **synthetic** surface". I substituted
> `fixtures/terrain_small.stl` for repo-portability and did not check its
> resolution against the quantity being measured. Re-run is on an analytic
> fixture with facets ≪ stepover; §F2-2 will carry the corrected numbers.

## §F2-1 First working spiral, and what it costs (2026-08-30) — SEE WITHDRAWAL ABOVE

Instrument `crates/rs_cam_core/tests/conformal_spiral_synthetic_f2.rs`,
module `crates/rs_cam_core/src/conformal_spiral.rs` (`a330b48b`), fixture
`fixtures/terrain_small.stl` (in-repo, 40,342 triangles). Two arms: a steep
envelope PROBE (whole inset ellipse, ~50 mm relief) and an envelope-INSIDE
test (censused flattest 24×18 mm window, 7.68 mm relief, 308 triangles).

### Headline

**The mechanism works, and phase 1 PASSES its falsifier — on the flat arm.**
One continuous 50-ring spiral, **zero retracts**, zero disk-domain
self-intersections, complete coverage after bridging, bridge overhead
**11.2%** against a 25% bar. That is the paper's core claim reproduced:
stay-down continuity over a region, with holes not yet in play.

**And it is 3.4× SLOWER than a plain ball-end raster on the same region**
(302.1 s vs 89.3 s, F-034, `link_ceiling: None`). The raster already had
zero retracts here, so the spiral's entire pitch bought nothing and cost
3.6× the cutting distance (3232 mm vs 904 mm).

### The two arms

| | ARM STEEP (probe) | ARM FLAT (inside envelope) |
|---|---|---|
| region | 9,107 tris, ~50 mm relief | 308 tris, 7.68 mm relief |
| flatten | 0 flips, 0 non-positive weights | 0 flips, 0 non-positive weights |
| radial distortion climb | 2.394 | 1.068 |
| outcome | `RingSearchStalled{rings 1, uncovered 19136}` | **planned, 50 rings** |
| verdict | condition F: **genuine coverage infeasibility** (mechanism/geometry, not the map) | falsifier PASS |

The steep arm's refusal is now attributable: the map is a proven embedding
(Tutte), the fold census is 0, and the search still could not place a
second ring — at this relief, concentric disk circles cannot cover the
surface ring-by-ring. Six blocker points at disk radius ≈1.0 stopped the
descent.

### Spacing is the real problem on the flat arm

Measured adjacent-ring 3D spacing against the 0.4862 mm equal-cusp target:
median **0.2275 mm**, p90 0.4714, and **79.3% of samples outside ±25%** —
77.4% of them *below* target. The spiral is over-covering by roughly 2×,
which is exactly where the 3.6× cutting distance comes from. Causes, in
order of evidence:

1. **Conformal distortion is unavoidable here.** The mean-value map is an
   embedding, not an isometry: an area-distortion spread of
   9.7e-5…1.8e-2 (1/mm²) means one disk radius maps to wildly different
   3D band widths. The binary search sizes each ring by its *worst*
   uncovered point, so the whole ring pays for the most-compressed sector.
2. **The curvature census says the flat law is not the target anyway**:
   interior-edge |κ| median 0.516 1/mm (R≈1.94 mm) with 55.5% convex, so
   the admissible stepover envelope is 0.25–0.70 mm depending on sign and
   magnitude. A single global ring spacing cannot satisfy that spread.
3. Sampling is *not* the explanation: N_S adequacy ratio 0.767 (under the
   1.0 bar), and halving N_S *reduced* the ring count 50→43, the signature
   of an already-optimistic predicate — not of under-sampling causing the
   narrow spacing.

7.1% of spacing samples imply scallop overshoot beyond the paper's own 12%
trial figure.

### Sensitivity (G-SAMPLING — neither paper states a rule)

| row | N_S | N_C | rings | uncovered | overhead % | spiral mm |
|---|---|---|---|---|---|---|
| baseline | 20000 | 360 | 50 | 0 / 0 | 11.215 | 3720.4 |
| half N_S | 10000 | 360 | 43 | 0 / 0 | 11.560 | 3277.0 |
| half N_C | 20000 | 180 | **REFUSED** `RingSearchStalled{rings 2}` | — | — | — |

Halving the angular lattice **breaks the mechanism outright** — the paper's
Table 1 case 1.4 failure mode reproduced on our fixture. The method is
sensitive to a parameter neither paper gives a rule for.

### Containment

961 of 22,303 cutting moves (4.3%) leave the region ellipse — the lateral
CL shift on slopes. Counted, deliberately **not** clipped: clipping splits
the polyline and would fabricate retracts into the very arm whose retract
count is the claim. The ellipse sits 8 mm inside the mesh, so escapes leave
the region, never the stock.

### What this does and does not settle

- **Settled**: the pipeline is implementable from the sources; the
  continuity claim is real and measured; the falsifier passes inside the
  envelope; the steep-region limit is a mechanism limit, not our map's
  fault.
- **Not settled**: whether it can ever be *fast*. On the friendliest
  possible geometry (convex, hole-free, low-relief) it loses 3.4× on time
  and 3.6× on distance. F-034-vs-raster was chartered as context, not a
  bar — but a 3.4× deficit on the easy case is not a margin that hole
  handling or ceiling-honest relinking will reverse.
- **Untested**: holes/islands (phase 3, reading gate open), Wanaka regions,
  any ceiling-honest re-run (`relink_and_cost_under`).

### Recommendation

Do not proceed to phase 3 (slit map / holes) on time-efficiency grounds.
The distortion-driven over-covering is upstream of everything holes would
add, and it is the same class of finding as F1: the mechanism transfers,
the *economics* do not. If the arm continues, the next lever is
**distortion-aware ring spacing** (per-sector radius, or an
area-preserving rather than conformal map) — attacking the 2× over-cover
directly — not more topology.

Artifacts: `terrain_small_conformal_spiral_flat_disk_f2.svg`,
`terrain_small_conformal_spiral_flat_xy_f2.svg`.
