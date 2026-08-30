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

---

## 8. The § 4.3 proposal, measured overnight — it helps a lot and still loses

Run `e5af6336`, four candidates on four analytic fixtures, every one scored
against the local floor `L_min = Σ area_t / s_max(t)` with analytic
curvature. **This is the × floor column §6 asked for.**

| fixture | **floor** | spiral | 0° raster | field `D=t1` | **field `D=sweep`** |
|---|---|---|---|---|---|
| SPHERE | 244.05 mm | 1.087× | **0.997×** ⚠ | 2.339× | **1.508×** |
| WAVY | 156.27 mm | 1.341× | 1.101× | 4.917× | **1.884×** |
| RIBBON | 373.88 mm | 3.745× | 1.310× | 9.001× | **2.071×** |
| BAND/shallow | 641.28 mm | *no path* | 1.127× | 4.455× | **2.094×** |

**The direction diagnosis is confirmed.** Swapping `D` from curvature to
the sweep axis cuts the field's distance by **1.55× (sphere), 2.61×
(wavy), 4.34× (ribbon), 2.13× (band)** — a large, consistent, one-change
improvement, and it fixes the fragmentation too (ribbon 674 → 98
fragments, 138 → 34 retracts). The pre-registered falsifier asked for
"less than `D=t1` on every arm, ≥2× on the sphere": the first clause
passes on all four, the sphere's specific 2× bar does **not** (1.55×).
Recorded as stated, not softened — and the reason is instructive: a sphere
is umbilic, so a noise direction still carries the *correct magnitude*
everywhere. The direction source hurts least exactly where curvature
carries no information.

**And it still loses to a plain raster on every fixture.** 1.51–2.09×
floor against the raster's 1.00–1.31×. The mechanism is the one already
identified for the spiral, appearing again in a different costume: Zou's
level schedule takes the **minimum increment over the whole curve**, so
every pass is spaced by its worst point — the same worst-sector rule that
costs the spiral 36× over-cover on a ribbon. **Any method that assigns one
spacing to a whole pass pays for that pass's worst point.**

### The honest caveat: the fixtures cannot show this proposal's advantage

A locally-varying stepover can only beat a uniform one where `s_max`
actually varies. On these fixtures it barely does — the sphere is
**umbilic**, so `s_max` is a single constant (0.47431 everywhere, min =
median = max), and the others are gentle. An honest raster on the sphere
would sit at exactly **1.0× floor** and could not be beaten by any
spacing law, because there is nothing to adapt to.

That is the third instance of one meta-error in this programme:

1. §F2-1 measured spacing on facets 3× the stepover — **the fixture could
   not resolve the quantity.**
2. §F2-4 measured a retract-elimination method against a baseline with
   **no retracts to eliminate.**
3. §8 measures a *locally-adaptive spacing* law on surfaces with
   **almost no local variation to adapt to.**

Each time the fixture made the claimed advantage structurally unable to
appear. **The rule this programme should adopt: state the mechanism by
which the candidate is supposed to win, then check the fixture can express
it, before running anything.**

So §4.3 is **not refuted** — it is untested on the geometry that would
decide it: a surface mixing tight and slack curvature, where an honest
raster must use `min(s_max)` globally while a field uses `s_max(x)`
locally. The predicted gain is `mean(s_max)/min(s_max)` over the region,
which is ~1.00 on these fixtures and would need a genuinely varied surface
to show. That is the experiment; nothing else in §6 changes.

### What is settled

- **The raster is at or below the floor** on every fixture — 0.997× on the
  sphere is under-coverage, and its 1.10–1.31× elsewhere is the honest
  number to beat. It is a much better baseline than this programme
  assumed.
- **Neither exotic method beats it on distance**, and distance is what
  dominates time now that `surface_link` has absorbed the retract wall.
- **The worst-point spacing rule is the common defect** of the spiral and
  both field variants. A method that beats the raster must vary spacing
  *along* a pass, not just between passes — which no candidate here does.

---

## 9. The operator's own proposal, measured (2026-08-31) — falsified where it was aimed, best-in-programme where it wasn't

Looking at the ribbon figure the operator proposed: *"parallel passes down
each of the arms, along the length of each arm, and then a spiral in the
center."* Implemented as a third choice of `D` — the region's **medial
axis**, obtained as `rotate(grad(EDT), 90°)` — with the iso-scallop
magnitude unchanged. Run `16ac0dc1`.

**The derivation came first, and it recast the idea.** With that `D`, the
target field reduces to `V = −|V|·∇̂EDT`, so φ is a reparameterised negative
distance transform and its level sets are **iso-distance offsets of the
boundary**. The operator's mental picture *is* contour-parallel machining,
reached from the opposite direction — and at the hub, where the EDT has a
local maximum, level sets around a maximum are closed loops encircling it.
**The "spiral in the centre" is what the field produces, not a special
case.** That makes the direct prior `FINDINGS.md` §0k (contour-per-cell,
**0.686×**) and §0d (contour whole-region, **0.91×**), both losses.

| fixture | floor | spiral | 0° raster | field `D=t1` | field `D=sweep` | **field `D=medial`** |
|---|---|---|---|---|---|---|
| SPHERE | 244.05 | 1.087× | 0.997× ⚠ | 2.339× | 1.508× | **1.036×** |
| WAVY | 156.27 | 1.341× | 1.101× | 4.917× | 1.884× | 1.495× |
| RIBBON | 373.88 | 3.745× | 1.310× | 9.001× | 2.071× | 2.525× |
| BAND | 641.28 | *no path* | 1.127× | 4.455× | 2.094× | 2.045× |

### The falsifier fired

Stated before the run: *on the ribbon the medial row must produce fewer
fragments **and** fewer links than `D=sweep`, with distance within ~1.5×.*
Measured: **179 fragments vs 98, and 160 links vs 63.** It fragments
*more*, on the exact geometry it was proposed for. **The proposal is
refuted there, and §0k's contour loss carried after all.**

The mechanism is visible in the numbers and is intrinsic to offsets on a
branched region: each offset ring **splits** as it passes a branch point,
so a star with eight arms shatters every contour into pieces. Sweeping in
one direction does not. Distance stayed within the rail (943.9 vs 774.5 =
1.22×), so this is a *direction* result, not a spacing-basis artefact.

### But the sphere row is the best honest number this programme has produced

**1.036× the floor, 13 fragments, 12 links, zero retracts, 22.5 s** — against
the raster's 22.0 s at **0.997×**, which is under-coverage, not a win. On a
disk the medial axis is a single point, so the offsets *are* concentric
rings, and contour-parallel is classically strong on round pockets. Same
time as the raster, and it actually meets the finish spec.

That is worth stating plainly: **on compact, round-ish regions the
operator's construction is the best candidate measured** — and it is also
the only candidate that has ever come within 4 % of the floor while
honestly covering the part.

### What this settles about the whole programme

Five strategies, four fixtures, one floor. The pattern across the table is
now unambiguous and it is **not** about which clever field you choose:

- **Region shape decides the strategy, not the algorithm's sophistication.**
  Compact/round → offsets win (medial 1.036×). Elongated/branched →
  one-direction sweeps win (raster 1.310× vs medial 2.525×). Every
  candidate that ignores shape loses on some shape.
- **Every method that assigns one spacing to a whole pass pays for that
  pass's worst point** — the spiral per ring, both field arms per level.
  That is the single common defect, and beating the raster requires varying
  spacing *along* a pass, which nothing here does.
- **The raster's lead is partly unbilled finish debt** (§1), and the medial
  row is the first candidate to match it on time while actually meeting
  spec.

So the near-optimal method is a **shape-selected** one: decompose, then
give each piece the strategy its shape earns — offsets for the compact
pieces, direction sweeps for the elongated ones — which is what the
operator's original instinct ("parallel down the arms, spiral in the
centre") described. It was right about the *decomposition*; the measurement
says the two halves must be **different strategies**, not one field that
tries to be both.

---

## 10. The preferred-direction field, researched (2026-08-31) — `D = t1` was RIGHT, and §4.3 was a departure from the literature, not a repair of it

Full extraction: `planning/conformal_finish_2026-08-28/preferred_direction_field_research.md`.
Sources retrieved: **Kumazawa, UBC MASc 2012** (Zou's [4]/[21], via Wayback — UBC's own host is Cloudflare-blocked) and **Kim, MIT PhD** (the full source behind [28], via MIT DSpace).

**The correction.** §8 and §9 of this document assumed our `D = t1` was a bad
substitution for a field the paper had and we lacked. That is wrong.
Kumazawa is explicitly about **three-axis ball-end** machining, and its rule
is *feed along the most convex principal direction* — which is `t1`. Kim
gives the closed form (Eq. 23) for the strip width about it. **We
implemented the literature's own answer.** Zou's [24] Lo and [25] Fard &
Feng, which our extraction named as the missing front-end, are both
**five-axis flat-end** work: [25]'s entire contribution is choosing a tool
*orientation* so a tilted flat-end's cutting **ellipse** is widest. A
ball-end on 3 axes has neither degree of freedom, and all of it collapses
to `r₁ = r₂ = r` — Zou's own stated best case.

**Why ours failed anyway — and it is structural.** The literature pipeline
is: direction field → **detect degeneracies** → **classify** them
(trisector / wedge / merged, by a discriminant sign) → **trace
separatrices** → **segment the surface** → per-patch sequential iso-scallop
seeded to minimise drift. We built the direction rule and **none of the four
stages that make it usable**. And on degeneracy both sources stop:
Kumazawa, verbatim — *"there is not one single preferred direction, because
all directions will be the preferred … when the surface at that point is
completely planar … it could be said that **all points in a surface are
degenerate points**"*; Kim excludes umbilics by assumption (`κ₁ ≠ κ₂`).
Neither publishes a fallback direction or a numeric degeneracy threshold.

**So the fixture error happened a fourth time, and this one was baked in by
construction.** Our SPHERE has `W_max − W_min = 0` *exactly* — the method's
own advantage is identically zero there. Kumazawa's conclusion is that it
*"benefits from surfaces that have a large number of features such as
mounts and valleys … where the difference between the maximum and minimum
`W` are notable."* None of our four fixtures is such a surface. **No choice
of `D` would have beaten the raster on the geometry we tested.**

**And the published prize is small.** Kumazawa's Table 1, measured against
an honest iso-scallop from the better border: **−2.6, −5.1, −1.9, −5.4,
−7.2, −3.3 %**. The large figures quoted in that literature are against
iso-parametric and iso-planar baselines, not against a good raster.

**Two things worth keeping.** First, `[28]`'s machine-derived field
maximises `F₀ = ϑ₀·w₀` (speed × strip width) off a velocity polygon and
**explicitly discards acceleration**, deferring it to a smoothing pass — so
it is *not* what our accel-bound result needs, and Kim proves it collapses
to the geometric field under an isotropic velocity envelope. Second, and
better: **Kim Eq. 32 is this document's §1 floor, written in 2001** —
`T_c = ∫∫ dA/(w·ϑ) + link terms on region boundaries`, i.e. area over
strip-width-times-speed, plus linking. We re-derived a twenty-five-year-old
result independently and can now cite it.

### Consequence for §4.3

The §4.3 proposal (`D` from the cell's sweep axis) is therefore **not a
repair of the literature's method — it is a departure from it.** It beat
`D = t1` by 1.55–4.34× on our fixtures *because those fixtures are umbilic
or near-umbilic*, where `t1` is noise and any consistent direction wins.
On a surface with genuine mounts and valleys the ordering could invert, and
that is untested. Both remain open; neither is a candidate for production
on this evidence.
