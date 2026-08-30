# Conformal-spiral arm (F2 phase 1) — findings

> §F2-1's spacing and time numbers were WITHDRAWN on 2026-08-30 (fixture too
> coarse). §F2-2 below carries the corrected measurement on analytic
> fixtures and supersedes it. The withdrawal block is kept after §F2-2
> because its diagnosis is the reason the corrected run exists.

## §F2-2 Corrected measurement on analytic fixtures (2026-08-30)

Fixtures generated in-test, facets far below the measurand:

| arm | triangles | max edge | stepover / median edge | relief |
|---|---|---|---|---|
| SPHERE (R_s 20 mm, 12 mm cap) | 37,060 | 0.1585 mm | **4.42** | 0.92 mm |
| WAVY (14 mm patch, A 0.5, λ 8) | 14,178 | 0.1545 mm | 4.47 | ±0.5 mm |
| terrain flat (WITHDRAWN) | 308 | — | **0.34** | 7.68 mm |

### The three questions this run answered

**1. Is the ring spacing correct?  YES — pre-registered verdict, both arms.**

| arm | analytic target | measured median | verdict |
|---|---|---|---|
| SPHERE | 0.47431 mm (cross-derived 0.47413, 0.039 % apart) | **0.51046** | inside ±25 % ⇒ **spacing CORRECT** |
| WAVY | 0.42406 mm (peak-convex) | **0.45027** | inside ±25 % ⇒ **spacing CORRECT** |

The half-spacing alternative (0.23716 / 0.21203) is refuted on both arms;
the bands are disjoint by construction. **There is no off-by-one-band
defect.** Terrain's 0.2275 mm was faceting, exactly as the withdrawal
suspected. The operator's "rings crowd at the edges" observation is also
resolved as a fixture artifact: ring-radius median **0.541 (sphere)** and
**0.529 (wavy)** against an even-spread reference of 0.500 — versus
**0.665** on the withdrawn terrain arm. Path continuity likewise: worst
step/chord ratio **1.9× / 2.2×** on the analytic arms against **21×** on
terrain, whose 3.38 mm jumps all sat on ring 0 at the coarse boundary.

**2. Does it machine the whole part?  YES — now that the witness exists.**
Both analytic arms: **0.0000 % unmachined, 0 of 37,060 / 14,178 triangles**
(terrain flat: 0.136 %). Before the sampling fix the sphere left a
**23.5 % hole** under three passing gates — see §F2-3.

**3. Is it faster than a raster?  NO, on friendly geometry.**

| arm | path | moves | frags | retracts | cut mm | F-034 s | unmachined |
|---|---|---|---|---|---|---|---|
| SPHERE | conformal spiral | 5547 | 1 | 0 | 265.3 | **23.4** | 0.00 % |
| SPHERE | 0° ball raster | 484 | 4 | 0 | 243.4 | **22.0** | not audited |
| WAVY | conformal spiral | 5547 | 1 | 0 | 209.6 | **18.8** | 0.00 % |
| WAVY | 0° ball raster | 338 | 3 | 0 | 172.0 | **16.0** | not audited |

The spiral is **6 % slower on the sphere and 18 % slower on the wavy
patch**, with coverage now verified equal. Finish is not identical, and it
cuts both ways: on the sphere the spiral spaces *wider* (0.510 vs 0.490
achieved) so it is slightly coarser **and** still slower; on the wavy patch
it spaces *tighter* (0.450 vs 0.491), so perhaps half its 18 % deficit buys
a finer finish. Normalised, the honest range is **≈5–15 % slower**.

`link_ceiling: None` (chartered fresh-stock exception) — no time claim is
final until a `relink_and_cost_under` re-run.

**Why it loses even though it is continuous:** on a convex, hole-free
region the raster *already* has zero retracts, so stay-down continuity buys
nothing and the spiral pays for its longer path. Its advantage would need a
region where a raster fragments badly — which is the thin-organic geometry
where arm A died and where C2's PCA-frame cells already win (1.090×
production-validated).

### §F2-3 Three defects found on the way, one of them the paper's

1. **The coverage predicate could go blind** (ours). Sample apportionment
   made `N_S` a cap, so 46 % of the sphere's triangles got zero samples —
   and on a polar mesh the starved ones are the small central ones. Result:
   a 2.783 mm-radius unmachined hole, 23.5 % of the region, while
   `uncovered_after_rings`, `uncovered_after_bridging` and the whole
   falsifier read clean, because **all of them are built from the search's
   own sample set**. The wavy arm (no starvation, no hole) is the
   controlled comparison. Fixed: `N_S` is a floor, every triangle gets ≥1
   sample, `triangles_without_samples` is a tripwire, and the adequacy
   number is now a max over per-triangle densities (the old
   `√(area/N_S)` average reported a healthy 0.076 mm on the run with the
   hole).
2. **A search cannot be its own witness** (ours, structural). New
   `CoverageAudit` tests every mesh triangle centroid against the finished
   spiral and is a hard falsifier STOP above 2 %; an absent audit is also a
   STOP. Read its distance-vs-`K_c` and radial rows, not its area — it
   area-weights whole triangles from a centroid test, so it is an upper
   bound scaled by facet size (that is why a knife-edge read as 14.2 % on
   one fixture while a real hole read 23.5 % on another).
3. **The paper's criterion has ZERO MARGIN by construction** (theirs).
   Eqs. 1–4 push each ring in until it just reaches the outermost
   still-uncovered sample, so adjacent rings land exactly `2·reach` apart
   and the midline sits exactly **on** the coverage boundary — the
   iso-scallop condition restated. It guarantees coverage of the search's
   own samples and nothing else. Proved in closed form on the flat disk:
   the only uncovered strips were the three whose centroids sat within
   0.003–0.049 mm of a band edge, next-smallest margin 0.141 mm.
   **The paper's own cutting trial overshooting nominal scallop by up to
   12 % is what a zero-margin criterion predicts.** Mitigation added as
   `ring_spacing_safety`, **default 1.0** so the published criterion is
   unchanged unless asked for.

Two traps for anyone adding a safety dial here: `reach = √(K_c² − (K_c−h)²)`
is steep near `K_c`, so derating the **radius** 5 % derates the **reach**
43 % — the factor must apply to the lateral reach; and the ring step is
quantised by the sample comb (a sample-free annulus at every mesh vertex
ring), so **it is bounded below by the mesh's own facet pitch** — an
independent reason a coarse fixture can never produce a clean spacing
series.

### Verdict on arm B

The mechanism is **correct and complete**: continuous single path, zero
retracts, zero self-intersections, verified full coverage, spacing on its
analytic target, bridging overhead 11–13 %. It is **modestly slower than a
raster on the geometry a raster likes**, and the geometry where it might
win is the branched/perforated terrain that its own map handles worst
(ARM STEEP still refuses with a clean map — at ~50 mm relief concentric
disk circles cannot cover ring-by-ring).

**Recommendation SUSPENDED 2026-08-30 — the benchmark was wrong for the
question.** An operator pushed back: what about narrow branched valleys,
and mountain ranges with lakes? The objection is correct and the earlier
recommendation over-reached.

- On the fixtures above the raster baseline produced **4 and 3 fragments
  with ZERO retracts**. On the real target geometry (thin-organic region
  1) a 0° raster produces **564 fragments / 97 kept retracts**, and even
  the tuned PCA-cell plan carries 141 / 53. **A retract-elimination method
  was benchmarked on geometry with no retracts to eliminate**, so its
  entire value proposition was structurally unable to appear.
- Multiply-connected regions — lakes, islands, keep-outs — are *precisely*
  what the conformal slit map exists for, and were never tested at all.
  Recommending against building the hole machinery on the strength of
  hole-free economics is not evidence, it is extrapolation.
- What survives as a real risk, now stated as a falsifiable prediction:
  flattening a long branched ribbon compresses the arm tips, so one disk
  circle maps to very different 3D spacings across arms, and the
  worst-sector rule turns that variation into over-cover. That quantity is
  `RingAnisotropy::median_ratio` / `worst_ratio`, which the module measures
  and no arm has yet printed. It competes against the retract savings
  rather than automatically beating them.

**The gate is therefore ARM RIBBON** (in progress): a narrow, branched,
**simply-connected** fixture — no slit map needed — with facets ≤
stepover/3 and a raster baseline that genuinely fragments. It asks the one
question that decides the programme: *does eliminating N retracts beat
paying an M-fold over-cover on branched geometry?* If the spiral wins
there, the slit map becomes worth building for the lakes case. If the
over-cover swamps the savings, the recommendation returns — on evidence
from the right geometry class instead of the wrong one.

---

> ## ⚠ §F2-1's SPACING AND TIME NUMBERS ARE WITHDRAWN (2026-08-30)
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
> therefore an artifact of faceting: the spacing distribution, the "≈2×
> over-cover", the **3.4× slower than raster** headline, the curvature
> census (that R ≈ 1.94 mm is the *faceting* radius), and the 2.05× 3D-to-
> projected area ratio.
>
> **A selection bias made it worse, and it was mine.** The flat-window
> census picks the flattest patch — and a decimated terrain mesh puts its
> *largest* triangles exactly where the surface is flattest (0.876 mm²
> median facet there vs 0.277 mm² mesh-wide). The census steered the
> measurement into the coarsest region of the mesh. Visible symptoms: the
> outer rings' 2.4 mm jumps (all at path indices < 600) and points up to
> 3.27 mm outside the ellipse are the coarse 30-vertex boundary loop.
>
> **A second, independent fairness defect** in the cost table: the spiral
> spaces on the **3D surface** while `raster_candidate` spaces in **XY
> projection**, so the arms were never delivering the same finish and the
> ratio compared two different measures. Now reported side by side.
>
> **Root cause of the fixture choice:** PROGRAMME.md F2 step 1 says "simply
> connected **synthetic** surface". `fixtures/terrain_small.stl` was
> substituted for repo-portability without checking its resolution against
> the quantity being measured.

## §F2-1 First working spiral (2026-08-30) — SPACING AND TIME WITHDRAWN, see above

Structural results, which are resolution-independent and stand: one
continuous 50-ring path, zero retracts, zero disk-domain
self-intersections, complete bridging, 11.2 % bridge overhead; the fold
diagnosis and the mean-value/Tutte fix; halving `N_C` breaking the
mechanism outright (the 2025 paper's Table 1 case 1.4 reproduced).
Artifacts `wanaka_region1_direction_field_f1.svg`,
`terrain_small_conformal_spiral_flat_{disk,xy}_f2.svg`.
