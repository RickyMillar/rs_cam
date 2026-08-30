# Toward the shortest machining time — a synthesis

> Written 2026-08-30 at the operator's request, after the F1/F2 conformal
> programme. It ties together the multitool tier split, the thin-organic
> directional splits (C2), the direction-field arm (F1) and the conformal
> spiral (F2), and asks whether a method exists that gets close to the
> theoretical minimum time for a given finish.
>
> **It is theory grounded in this session's measurements, not a proposal to
> build anything yet.** Every number is cited to the run that produced it.

## 1. The floor, and the fact that we have been beating it

For a tool of radius `K_c` and a scallop spec `h`, the maximum admissible
pass spacing at a point is `s_max(x)` — the classic iso-scallop stepover,
corrected for local curvature. The shortest possible cutting distance that
can meet the spec is therefore

```
L_min = ∫∫ dA / s_max(x)
```

— every pass spaced exactly at the local limit, nothing cut twice. Below
this, the spec is not met. It is a hard floor, not a target.

Measured against it, from the six-arm F2 run (`87c97c82`):

| arm | region 3D area | s_max | **L_min** | raster cut | ×floor | spiral cut | ×floor |
|---|---|---|---|---|---|---|---|
| SPHERE | 115.76 mm² | 0.47431 (exact, constant curvature) | **244.1 mm** | 243.4 | **0.997×** | 265.3 | 1.087× |
| WAVY | 81.61 mm² | 0.42406 (tightest) | 192.4 mm | 172.0 | 0.894× | 209.6 | 1.089× |
| RIBBON | 191.82 mm² | 0.48621 | 394.5 mm | 489.9 | 1.242× | 1400.0 | 3.549× |
| BAND/shallow | 320.82 mm² | 0.48621 | 659.8 mm | 722.6 | 1.095× | — | — |

**On the sphere the raster cuts 99.7 % of the theoretical minimum — which
is impossible for a path that meets the spec.** The sphere is the rigorous
case: curvature is constant, so `s_max` is a single exact number and the
floor is exact. A path shorter than the floor is under-covering, full stop.
The instrument says the same thing independently: the raster's *achieved*
surface spacing there is a median 0.48997 mm against an admissible
0.47431 — **3.3 % wider than allowed**, because `raster_candidate` spaces
in **XY projection** while the constraint lives on the **3D surface**. On a
slope of angle θ its real spacing is `s_XY / cos θ`.

(The wavy arm's 0.894× is *not* proof of the same thing — there `s_max`
varies and 0.42406 is its tightest value, so the true floor is lower than
the table's. Only the sphere settles it.)

**Consequence: every "the raster wins" comparison in this programme has
been unfair in the raster's favour.** It is fast partly because it is not
delivering the finish it is credited with. At *equal guaranteed scallop* a
raster must tighten its XY stepover to `s_max·cos θ_max` — the worst slope
in the region — and its distance grows accordingly. That comparison has
never been run, and it is the single most valuable missing measurement in
the whole programme.

## 2. Everything we have built is the same algorithm

The unifying observation, which only became visible once all four existed:

> **Every finishing strategy here machines the level sets of a scalar field
> `φ` defined on the surface. They differ only in how `φ` is chosen, and in
> how the level sets are connected to each other.**

| strategy | `φ` | level sets | connection |
|---|---|---|---|
| 0° raster | `x` | parallel lines | boustrophedon + links |
| PCA cells (C2) | `x` in a per-cell rotated frame | parallel lines, piecewise | per cell, then reorder |
| direction field (F1, Zou) | Poisson solution `Δφ = ∇·V` | iso-scallop curves following a preferred direction | unsolved — the paper does not order them |
| conformal spiral (F2, Shen) | disk radius under a conformal map | concentric rings | **bridged into one continuous spiral** |
| iso-scallop / `scallop_isofield` | Eikonal distance from boundary | nested offset rings | rings, or spiral |

Two things fall out of that table immediately.

**(a) The spiral's continuity trick is orthogonal to the field choice.**
Bridging joins nested level sets into one path. It is a property of the
*connection* step, not of conformal mapping. Any field whose level sets are
nested loops can be bridged the same way. The conformal map is not what
gives the spiral its zero retracts — the bridging is.

**(b) The direction field is the only one that targets the floor
directly.** Its whole construction is "spacing = the local iso-scallop
limit, direction = a preferred field". That is `L_min`'s integrand. The
raster's uniform stepover cannot be locally optimal unless curvature is
constant; the spiral's one-radius-per-ring is *provably* not locally
optimal on anything but a disk of revolution.

## 3. The three costs, and who pays which

Time decomposes into three terms, and each of our methods optimises one at
the expense of the others.

| cost | what minimises it | measured evidence |
|---|---|---|
| **1. Coverage** — cutting distance `∫dA/s` | spacing at the *local* limit everywhere | spiral pays 3.5× the floor on RIBBON (ring anisotropy median **36.7×**, worst 113× — one radius per ring, sized by its worst sector) |
| **2. Kinematics** — velocity lost to turns and short passes | long, straight, aligned passes | why contour-parallel lost historically (§0d 0.91×, §0k 0.686×) and why C2's PCA frame wins (1.090× production-validated) |
| **3. Connection** — links, retracts, hops | continuity, or cheap stay-down links | spiral: 0 retracts. Raster on BAND: 63 fragments → **52 stay-down links → only 10 retracts**, ≈0.38 s each |

The third row is the quiet finding of this session. **`surface_link` has
already dismantled most of the retract wall** — 52 of 63 fragment
boundaries never become lifts. On this machine, at 735 mm/min, the residual
per-link cost is ~0.38 s. The spiral spends **54 s** of extra cutting to
remove **9** of them (RIBBON). Continuity is no longer the expensive
problem; **spacing is**.

## 4. What the near-optimal method would look like

Combining what each piece demonstrably does well:

1. **Tier / tool split** (shipped multitool) — chooses `K_c` per region from
   the detail the geometry demands. Sets `s_max`'s scale. Orthogonal to
   everything below; keep as is.
2. **Decompose into monotone cells in a gated PCA frame** (C2, shipped,
   1.090× validated) — this is the *kinematics* term. It buys long straight
   passes and few pass-ends, and it is why C2 wins. Keep as the outer
   structure.
3. **Inside each cell, replace the uniform-stepover raster with an
   iso-scallop field solve** — Zou's Poisson formulation (F1's
   `direction_field`, already implemented and unit-tested), but with the
   preferred direction `D` supplied by **the cell's own PCA sweep
   direction** instead of by curvature. This is the *coverage* term: passes
   still run along the cell's long axis, but spaced at the local limit
   rather than at a global worst case.
   - This directly repairs F1's cause of death. Arm A failed because
     curvature-derived `D` is **noise on a Shallow near-umbilic region**
     (2,340 orientation inconsistencies) and because level sets of one
     global scalar thread every branch of a ribbon at once (median 28
     components per level). A per-cell field with `D` given, on a monotone
     cell, has neither problem: `D` is clean by construction, and a
     monotone cell has no branches to thread.
4. **Connect with the existing relinker** — stay-down surface links,
   TSP-ordered. Already shipped, already eating 80 % of the fragmentation.
5. **Bridge into a spiral only where a cell's level sets are nested loops**
   (pocket-like or island-surrounding cells) — reuse F2's bridging, which
   is the part of the conformal work that is unambiguously sound (zero
   retracts, zero self-intersections, 4.7–13 % overhead, verified full
   coverage). Do **not** use concentric rings from a conformal map as the
   general case; that is what costs 3.5× the floor.

The conformal slit map — the thing F2 spent its time on — is **not** in
this synthesis. Its topology handling is real and its bridging is
reusable, but its ring geometry is the wrong primitive for anything but a
disk of revolution, and §F2-4 measured the price.

## 5. How much is actually on the table

Honest arithmetic, not a promise:

- The raster is already at **1.10–1.24× the floor** on real-shaped regions
  (BAND, RIBBON) — *before* correcting for the fact that it under-covers on
  slope.
- The gain from iso-scallop spacing is roughly `mean(s_max) / min(s_max)`
  over a cell — i.e. how much curvature varies inside it. On Wanaka's
  gentle terrain (relief 9.8 mm over 200×200 mm) that is a **few percent**.
  On curvy work it is larger.
- The gain from *correcting* the XY-projection under-coverage is **negative
  time and positive quality**: the honest raster is slower than the one we
  have been benchmarking. Some of the apparent headroom is not headroom at
  all — it is unbilled finish debt.

So the realistic target is not "2× faster". It is **"meet the finish spec
everywhere, at 1.0–1.1× the floor, with the retract wall already gone"** —
which would be a genuinely optimal-ish finishing strategy, and which is
mostly assembling parts that already exist and are already validated.

## 6. The experiment that would settle it

One instrument, reusing everything built this session:

1. On the analytic fixtures (facets ≤ stepover/3 — the only reason this is
   now measurable at all; the v3 process-proof campaign died in July on
   exactly this, `[[project-v3-process-proof]]`, "NOT PROVABLE on this
   fixture (coarse TIN)"), compute `L_min = ∫dA/s_max(x)` per region
   directly from the mesh and the tool.
2. Run every candidate — raster, **honest raster** (XY stepover derated by
   `cos θ_max` so it actually meets spec), PCA cells, direction field with
   `D` from PCA, spiral — and report each as **× floor** and as **F-034
   time at equal ACHIEVED scallop**, using the coverage audit to verify
   they all finish the part.
3. The winner is whichever sits closest to 1.0× floor with the fewest
   links. That number is the answer to "how close to optimal are we?", and
   nothing in the programme has ever measured it.

The pieces for this exist today: `CoverageAudit` (does it finish the
part?), the achieved-surface-spacing block (is it the same finish?),
`relink_and_cost` (what does it cost?), the analytic fixtures (can the
measurement even be trusted?), and `direction_field` (the candidate that
targets the floor). What is missing is one afternoon of wiring and the
`L_min` integrand.

## 7. What I would not do

- Build the conformal slit map. Its topology case is proven (§F2-4: no
  slope band is a topological disk at useful size) but it would unlock a
  ring scheme measured at 1.87× slower on the geometry it unlocks.
- Chase retracts further. `surface_link` already converts ~80 % of fragment
  boundaries to stay-down links, and the residual is ~0.38 s each.
- Trust any efficiency number measured on a mesh whose facets are
  comparable to the stepover. That mistake has now cost this programme two
  full rounds of work (§F2-1 withdrawal, and the v3 campaign before it).
