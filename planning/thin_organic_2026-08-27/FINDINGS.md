# Efficient finishing coverage of THIN ORGANIC regions — investigation

> Read-only investigation, 2026-08-27. **No cargo was run, no code changed, no
> git write performed.** Every number below is either (a) quoted from an
> existing ledger with its source named, or (b) derived arithmetically from
> code I read, and labelled as derived. Nothing here was measured tonight.
> §6 lists what I could not verify.
>
> Inputs read: `planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md` §0–§2,
> `planning/airrun_2026-08-19/P2_RUN_LOG.md` (Sessions 1–3),
> `unified_finish.rs` (module doc + region loop), `finish_planner.rs`
> (`decompose`, `FinishPlannerParams`), `surface_link.rs` (`RelinkParams`,
> `LinkCeiling`, `relink_fragments`, tonight's G-LINKVETO diff),
> `tier_islands.rs`, `scallop.rs` + `scallop_math.rs` + `scallop_isofield.rs`,
> `dropcutter.rs`, `toolpath.rs::raster_toolpath_from_grid`, `region_mask.rs`,
> `grid_field.rs`, `narrate.rs`, `compute/config.rs`,
> `session/multitool.rs` (working-tree diff),
> `planning/review_2026-07-29/CHECKPOINT_C_EVIDENCE.md`,
> `planning/review_2026-07-29/CLASSIFICATION_PERF_STUDY.md`,
> `planning/STRATEGY_ADVISOR_2026-06-17.md`,
> `research/multitool_finishing_optimization.md`.

---

## 0. The mechanism, stated exactly

The confined fine tier fragments for one reason, and it is structural, not a
tuning failure.

`unified_finish`'s Shallow band builds **one mesh-global, axis-aligned
drop-cutter grid** and rasters it:

- `unified_finish.rs:1842` — `shallow_grid` is memoised once for the whole op
  and shared by every Shallow region.
- `unified_finish.rs:2012` — `batch_drop_cutter_with_cancel(mesh, index,
  cutter, params.raster_stepover, **0.0**, …)`. That `0.0` is
  `direction_deg`.
- `dropcutter.rs:120` — the grid covers `mesh.bbox.expand_by(r)`, the whole
  board, regardless of which region will consume it.
- `unified_finish.rs:2083` → `toolpath.rs:607` `raster_toolpath_from_grid`,
  which walks `row`-major and **partitions each row into contiguous runs that
  are inside the region set** (`toolpath.rs:650-671`). Runs more than one cell
  diagonal apart each get their own `retract → traverse → replunge`
  (`toolpath.rs:694-710`).

So the fragment count of a confined shallow pass is a purely geometric
quantity:

```
fragments ≈ (region Y-extent / raster_stepover) × (mean island crossings per scan row)
```

On the confined wanaka tier 1 (R1.0, cusp target 0.01 mm →
`raster_stepover = 0.2821 mm`, `wanaka200_mt2.toml:921`) over a ~200 mm board
that is ~709 scan rows; the ledger's **17,083 intra-node retract trips**
(`surface_link.rs:429`, operator-measured 2026-08-27) implies **~24 finger
crossings per scan row**. That is the dendritic web being cut *across* its
fingers, 24 times a row, 709 rows deep. *(Derived: 17083 / 709 ≈ 24.)*

Tonight's fixes attacked the **price** of each junction, not the count:

- **G-LINKVETO** — a *ceiling* link is exempt from the territory veto, because
  it is airborne by construction. Retracts fell ~13× (ledger: 17,083 → 1,263
  remaining under the then-6 mm cap).

  **Amended 2026-08-27, same night.** As first written the exemption keyed on
  `link_ceiling.is_some()` alone, which over-reached: `project_curve`
  (engraving) passes a ceiling *unconditionally*
  (`compute/execute.rs:1981` — "the original ceiling caller"), so for that op
  the boundary veto became **unreachable** and 35 links crossed excluded
  territory where its sentry requires 0. "Airborne" answers the *gouge*
  question, not the *territory* one: a machining boundary also encodes
  keep-outs and fixtures, and `LinkCeiling` reads its clearance from the dexel
  stock, which need not model a clamp. The exemption is now an explicit **op
  prior** — `RelinkParams::airborne_links_may_leave_territory`, conjunctive
  with the ceiling so a surface-riding link stays vetoed everywhere — set
  `true` only at `unified_finish`'s intra-region relink, `false` for
  project_curve and scallop. Caught by
  `project_curve_chaining::a_link_may_not_leave_the_machining_boundary`;
  the converse is now pinned too, by
  `surface_link::tests::an_airborne_link_still_answers_a_territory_boundary`.
- **hookup raised to 25 mm** for planner tiers (`session/multitool.rs:731`).

Both are correct and both are cost-gated — a candidate link is still priced
against the retract it replaces with the F-034 integrator and dropped as
`slower_than_retract` when it loses (`surface_link.rs:485-513`). But the
17,000 junctions did not disappear; they became **`Linking`-intent FEED moves
at `feed_rate`** (`surface_link.rs:522-526`) travelling above the surface.
Three consequences the A/B must expect:

1. Total time falls (each kept link is provably faster than its retract).
2. `air_cut_pct_of_cutting_time` **rises** — a Linking feed above the surface
   removes nothing but is not a rapid, so it lands in the cutting denominator.
   **Do not read that rise as a regression.**
3. The path is still ~17,000 junction events long, which is exactly the
   population the F-034 junction-velocity model charges for on a low-accel
   router (`planning/ACCEL_FRIENDLY_TOOLPATHS_2026-06-20.md` lineage: the
   Grbl junction-deviation model crawls to ~174 mm/min at a 90° corner).

**The topology is the remaining lever.** Both levers below reduce the
*count*; neither is a link-tuning change.

---

## 0b. AUDIT CORRECTIONS (2026-08-27, late — read before acting on §1)

A read-only design audit of Lever 1 against the working tree found several
claims below to be wrong or measured on the wrong object. They are corrected
here rather than quietly edited away, because two of them change what the
implementation *is*, and one of them may falsify Lever 1's premise entirely.

**C1 — The fixture moved under this document.**
`planning/multitool_2026-08-23/wanaka200_mt2.toml` is UNTRACKED and was
regenerated during the session. It now describes an **R1.5 → R1.0** ladder at a
**30 µm** cusp (`scallop_height = 0.03`, tier stepovers 0.596992 and 0.486210,
`cell_mm = 0.3`, `overlap_mm = 2.0`, `coarseness = 1.0`), not the R2.0 → R1.0
at 10 µm quoted below. Every derived figure in §0 and §1.1 that used
`raster_stepover = 0.2821` is therefore stale: at 0.4862 the tier-1 scan-row
count over a 200 mm board is ≈ 411, not 709, so the implied crossings-per-row
is ≈ **41, not 24**. `max_rings` for tier 1 is ≈ **215**, not 364. The
*conclusions* (fingers are crossed many times per row; `max_rings` is nowhere
near binding) survive; the numbers do not. **Re-read the toml before quoting
any of them again.**

**C2 — The thinness metric was specified on the wrong grid. This is the big
one.** §1.1 puts the EDT on `decompose`'s label grid. But Step 6
(`finish_planner.rs:498-505`) extracts polygons through
`region_polygons_from_mask_clamped`, which **dilates the mask by `overlap_mm`**
(2.0 mm live) before marching squares. So:

* a 1.5 mm mask finger reaches the generator as a **≥ 5.5 mm polygon** — 3.7×
  the width the routing rule and the "4–6 rings" prediction assume;
* fingers closer than `2 × overlap_mm` = **4 mm merge into one polygon**.

Combined with `decompose`'s own `min_region_area_mm2` (16 mm² at R1.0, which
absorbs a 1.5 × 8 mm finger outright), its `close_radius_mm` morphological
close, and `MAX_REST_REGIONS = 64` truncating extraction *silently* on this
path, **the shallow band inside tier 1 may already be one fat polygon with
holes rather than a population of thin fingers.** If so Lever 1's premise is
false on this fixture and the §1.2 ranking is wrong.

`tests/thin_organic_island_widths.rs` exists to settle exactly this, and it
measures **two** populations for the reason above: Stage A (tier islands, what
the planner hands over) and **Stage B (the `decompose` regions the generator
actually receives, post-dilation)**. Stage B is the decisive one. Nothing in
§1 should be built until it has been read.

**C3 — Deriving the cascade's cusp from `raster_stepover` is mandatory, not
hygiene.** `scallop_height` and `raster_stepover` are independent fields with
defaults 0.1 and 1.0. On a Ø6 ball that is a **2.4× cusp difference**, silent
on every surface. They are only tied on the multitool planner path; off it they
disagree by construction.

**C4 — The "new `RegionKind` variant" route is closed.** Two shipped sentries
pull opposite ways: one requires every variant to have a *distinct*
`span_label()`, another reconstructs the label as `format!("{band} band")` and
asserts equality. The stored-strategy-field route is the only one left, and the
stored value must reach **both** `RegionAnnotation::semantic_label` and
`compute/annotate.rs`'s `SemanticKey::Strategy`, or the lie survives in the two
places that matter. A contour-routed node must also keep `band()` returning
`Some(Shallow)`, or `shallow_band_stock_to_leave_exhibit_d16_2` selects an
empty population and passes **vacuously** — the same failure class as §1.4's
widened census sentry.

**C5 — The proposed `uncut_core_mm2 == 0.00` tripwire is not buildable as
written.** `generate_unified_finish` records it unconditionally, so a
raster-only op already publishes `Some(0.0)` = "measured clean" for a cascade
that never ran. Worse, a degenerate seed ring returns empty rings *and* zeroed
metrics, so a region that emitted nothing also reads "measured clean". The
tripwire needs a per-region channel.

**C6 — The offset-library risk paragraph in §1.1 is wrong twice.** The shipped
cascade takes the arc-carrying branch, not `offset_polygon`; and
`generate_unified_finish` never records `offset_library_failures` at all, so it
is `None` for this op in both A/B arms. There is nothing to compare.

**C7 — Effort estimate revised: ~215 → 300–400 production lines**, because C2
and C4 both move work into touch points scoped against the wrong stage. The
~350 sentry lines stand, plus two more sentries C4 implies. The genuinely good
news the audit confirmed: the generator really is band-agnostic —
`route_greedy`, `choose_link`, `strippable_preamble`, `trailing_retracts` and
the relink block need **no** change, and §1.4's surface cache does discharge the
per-region-rebuild risk.

Two hazards worth carrying into any A/B: the cascade's stepover reduction is a
MIN over slope **and curvature**, and a thin finger is a high-curvature object
by construction — so §1.1's "slope-homogeneous, nothing to reduce" argument
covers only half the term, in the direction that hurts. And the cascade's seed
ring sits exactly on the marching-squares boundary where the strict ray-cast
containment test is documented as a coin flip; on a 4–6-ring finger that is
~20% of coverage, and dropped seed-ring points are excluded from `standing_mm2`
by design, so the loss is invisible to every instrument.

---

## 0c. MEASURED (2026-08-27, late) — the lever survives, the trigger does not

`tests/thin_organic_island_widths.rs`, real wanaka mesh (661,212 triangles,
200 × 200 mm), live mt2 dials, R1.5 → R1.0 ladder at 30 µm cusp. Tier map
694² @ 0.30 mm in 4.2 s; classification grid 825² @ 0.25 mm.

**The width-based routing rule proposed in §1.1 is DEAD.** Shallow band,
Stage B (the polygons the generator actually receives, post-`overlap_mm = 2.0`
dilation):

| | |
|---|---|
| widths | min 0.50, p25 5.15, **median 7.38**, p75 8.56, max 13.00 mm |
| area under 8·s (3.89 mm) | **0.0 %** |
| area under 16·s (7.78 mm) | 13.4 % |

At `THIN_K = 8` the rule fires on **nothing**. Building it would have been
300–400 production lines that never trigger. This is why C2 said measure first.

**But the territory really is a dendritic web, and the LEVER survives.**
Stage A (the raw tier-1 ownership mask, upstream of dilation): the dominant
component is **7,553 mm² at 3.84 mm wide** — 7.9 stepovers — inside a
1,337-component mask that the extractor merges down to 16 shallow polygons.
The dilation adds ~4 mm of width and welds fingers closer than 4 mm together.
So the *shape* premise in §1 was right; the *metric* was measuring the wrong
object.

**Stage C measures the defect itself** — scan rows at the tier's own stepover,
counting maximal inside-runs per row, which is exactly what
`raster_toolpath_from_grid` emits as separate fragments — against the ring
count an offset cascade needs for the same polygon:

| band | raster fragments | cascade rings | ratio |
|---|---|---|---|
| Shallow | 2,406 | 120 | **20.1×** |
| MidSteep | 1,547 | 91 | 17.0× |

**20.1× fewer junctions**, at the bottom of §1.1's predicted 20–100×.

A calibration check that was not designed in and is worth more than the
headline: **MidSteep already runs the ring cascade**, and this instrument —
which knows nothing about that — independently scores contouring 17× better
there. The metric reproduces a decision the codebase already made, rather than
merely flattering the hypothesis it was built to test.

### What this changes

1. **Keep Lever 1. Replace its trigger.** Not `min_width <= THIN_K · s`, which
   measures width; use **elongation / crossings**, which measures what actually
   fragments a raster. A 10 mm-wide, 300 mm-long branching snake is "wide" and
   still shreds a raster because one scan row enters and leaves it repeatedly.
   The cheapest honest trigger is the Stage C computation itself — rasterise
   the region polygon at the op's own stepover, count runs, compare to
   `⌈width / 2s⌉`, route to the cascade above a ratio (≈ 4). It is milliseconds
   per region and it measures the real quantity instead of proxying it.
2. **Lever 1's reach is smaller than §1 assumed.** Tier 1's covered area is
   29,148 mm², of which **MidSteep is 27,488 mm² and already contoured**.
   Lever 1 can only address the Shallow band, ~9,000 mm². The 20.1× applies to
   that share, not to the op.
3. **Stage C's 2,406 is a LOWER BOUND** on real shallow fragments: it counts
   crossings of the decompose polygons only, and the shipped raster also
   fragments on `min_z` clamping and coverage holes. It is not comparable
   like-for-like with the ledger's ~17k figure, which was measured on a
   different config and over the whole op.

### What is still not answered

The **accel-aware wall-clock** question, and it is the one that could still
invert this. `STRATEGY_ADVISOR_2026-06-17.md` measured parallel 446 s vs spiral
828 s on this very board at the load limit, because contour paths chain short
chords and the Grbl junction-deviation model crawls corners. Fewer junctions is
not the same as less time. Only the A/B in §2, costed through the F-034
integrator, settles it — and the cusp-pattern change remains an operator gate
under the C4 rule.

---

## 0d. THE ANSWER (2026-08-27) — Lever 1 is REFUTED. The raster already wins.

Stage D generates **both** patterns over the **same** regions with the same
tool, feeds and machine envelope (Shapeoko Pro XXL: accel [500, 500, 270]
mm/s², junction deviation 0.02, rapid 5000; feed 735, plunge 180), and costs
each through the F-034 integrator. No production code was written to do it —
both generators already ship, and `unified_finish` already calls both, just on
different bands.

**Both paths are relinked**, because production relinks every region's toolpath
regardless of which generator produced it. Comparing a bare raster against a
natively-chained cascade would have been rigged, and the first run of this
experiment made exactly that mistake — it reported the cascade *winning* 1.05×
until the raster got the relink pass production actually gives it.

| region | raster time | cascade time | ratio |
|---|---|---|---|
| 3104 mm² | 1053.7 s | 1203.3 s | 0.88× |
| 1788 mm² | 478.0 s | 517.0 s | 0.92× |
| 1621 mm² | 483.7 s | 505.0 s | 0.96× |
| **total** | **2015.4 s** | **2225.3 s** | **0.91×** |

**The contour cascade is ~10% SLOWER, consistently.** It also cuts 29% further
(11,174 mm vs 8,687 mm) and emits 3.2× the moves (44,315 vs 14,027) — short
chords the junction-deviation model then crawls through. This is the
`STRATEGY_ADVISOR_2026-06-17.md` result reproduced on a different band: on this
low-accel belt router, contour is the exception, not the default.

### Why the 20.1× junction win did not translate

Because **the relink pass had already solved fragmentation** — for both
patterns:

| | fragments | linked | kept retracts |
|---|---|---|---|
| raster, region 1 | 564 | 466 | **97** |
| cascade, region 1 | 483 | 411 | **71** |

The raster's 757 scan-line crossings collapse to **97 actual retracts**. Once
that is true, junction *count* stops being the binding constraint and cutting
*distance* plus chord density decide — and there the raster is ahead.

So the operator-visible defect that started this investigation (17,092 retract
round trips) was real, and it was fixed by **tonight's G-LINKVETO + hookup-25
link work**, not by anything topological. Lever 1 was solving a problem that no
longer exists.

### Standing recommendation

**Do not build Lever 1.** Do not build Lever 2 either on this evidence: its
whole rationale was reducing the same crossings, whose cost the relink already
absorbs, and its measured ceiling was 1.3–2× on a quantity that is no longer
binding.

If finishing time is attacked again on this fixture, the evidence points at
**cutting distance and chord density**, not path topology — the raster spends
8,687 mm at 735 mm/min ≈ 709 s of pure feed against a 1,054 s total, so ~33% is
still overhead worth attributing before anything is redesigned.

### What this does NOT establish

- **Three regions, one tier, one fixture.** The top three shallow regions by
  area, not the whole band, and nothing about other boards.
- **Fresh-stock links.** `link_ceiling: None` here; the live tier 1 is a rest
  op whose ceiling declines more links, which would narrow the raster's margin.
  Direction of the effect is known, magnitude is not.
- **Cusp quality is assumed equal, not measured.** Both were dialled to the
  same 30 µm target by the same law, but `CHECKPOINT_C_EVIDENCE.md` records the
  cascade's *achieved* cusp running 2.0–4.9× its dial. If that holds here the
  cascade is not only slower but coarser — which would strengthen this
  conclusion, not weaken it.
- **Inter-region routing is excluded.** Each region was costed in isolation.

---

## 0e. CORRECTION (2026-08-28) — §0d was wrong about Lever 2. Angle matters.

§0d recommended building neither lever, and dismissed Lever 2 on the reasoning
that crossings stop binding once relink absorbs them. **That was an assertion,
not a measurement, and the operator was right to push back on it.** Relink does
not delete a junction; it converts a retract into a feed move, and region 1
keeps 466 of them.

Stage E sweeps `direction_deg` (which `batch_drop_cutter` has always taken) and
costs raster + relink at each angle through the same integrator:

| region | best angle | time at 0° | time at best | gain |
|---|---|---|---|---|
| 3104 mm² | 135° | 1053.7 s | 932.9 s | **1.13×** |
| 1788 mm² | 0° | 478.0 s | 478.0 s | 1.00× |
| 1621 mm² | 45° | 483.7 s | 422.2 s | **1.15×** |
| **total** | | **2015.4 s** | **1833.1 s** | **1.10×** |

**Sweep direction is worth ~10% on this geometry**, and it needs no new
generator — only a per-region angle handed to a parameter that already exists.

### The predictor is KEPT RETRACTS, not fragment count

The best angle is not the one that fragments least. Region 1:

| angle | fragments | kept retracts | time |
|---|---|---|---|
| 75° | 488 (fewest) | 124 | 1155.4 s (**worst**) |
| 0° | 564 | 97 | 1053.7 s |
| 135° | 502 | **68** | 932.9 s (**best**) |

Fragment count spans 471–597 while time spans 933–1155 s. Retracts track time;
fragments do not. A relinked fragment is cheap; a kept retract pays the full
safe-Z round trip. So the quantity to minimise is **fragment adjacency** — do
consecutive pieces land within `hookup_distance` of each other — not crossings.

This also retires §1.2's framing: Lever 2's benefit was described there as
"bounded by how anisotropic the island happens to be", predicted at 1.3–2× on
fragments. Fragments were the wrong instrument; the measured time gain is 1.10×.

### Ceiling, and where the rest of the overhead is

One angle per region is a heuristic with a low ceiling: a branching web has no
single long axis. Even at its best angle region 1 runs 932.9 s against ~709 s
of pure cutting feed at 735 mm/min — **~24% is still overhead**.

The principled next rung is **boustrophedon / Morse cell decomposition**
(Choset & Pignon 1997; Acar & Choset 2002, both verified in §5): split a region
into cells each monotone in some direction, sweep each cell along its own axis,
order cells by a TSP over the adjacency graph. That is precisely the operator's
"down the longer sections of narrow paths", applied per section rather than per
region.

And the general problem really is hard — the milling problem of Arkin, Fekete &
Mitchell (2000) is NP-hard with constant-factor approximations only, and
Fekete et al. 2023 state that *"the number of turns in a tour is of crucial
importance for the overall cost"*. Hard in general does not mean unavailable in
practice: the decomposition above is standard, and the cheap 80% (one angle per
region) is measured above at 1.10× for a parameter that already exists.

### Revised recommendation

- **Lever 1 (contour): still refuted.** §0d's measurement stands — 0.91×.
- **Lever 2 (sweep angle): REINSTATED**, at a measured 1.10×, with the trigger
  keyed on retracts rather than crossings.
- Beyond that, cell decomposition is the honest next step, not a bigger dial.

---

## 0f. Is the best angle PREDICTABLE? Only where the shape has a real axis.

§0e proved sweep angle is worth 1.10×, but found it by brute force — 12 grid
builds plus 12 relinks per region, ~2 minutes. Far too expensive to run per
region at plan time. So: can a free geometric predictor find it?

Stage F takes each region's interior second moments (one covariance matrix) and
costs the predicted angle against 0°.

| region | elongation | PCA minor | t(0°) | t(pred) | gain |
|---|---|---|---|---|---|
| 3104 mm² | **4.15** | 119.6° | 1053.7 s | 974.2 s | **1.08×** |
| 1788 mm² | 1.65 | 155.4° | 478.0 s | 496.9 s | 0.96× |
| 1621 mm² | 1.78 | 43.1° | 483.7 s | 479.8 s | 1.01× |
| **total** | | | **2015.4 s** | **1950.9 s** | **1.03×** |

Blanket PCA captures only 1.03× of the 1.10× ceiling. **But the failure is not
random — it tracks elongation**, and the per-angle curves say why.

**Region 1 (elongation 4.15) has a genuine plateau**, not a spike:

```
120° 1.08×   135° 1.13×   150° 1.12×     ~45° wide
```

PCA-minor predicted 119.6°, the plateau's leading edge, and delivered 1.08×.
The predictor *works* here.

**Region 3 (elongation 1.78) has a 1.9°-wide spike**: 43.1° costs 1.01× while
45.0° costs 1.15×. A gain that evaporates within two degrees — and sitting
exactly on the grid diagonal — is not a property to build a dial on. Treated as
an artifact, not a signal.

### The implementable rule

**Compute PCA; sweep along the MINOR axis only when elongation exceeds ~3;
otherwise leave the angle at 0°.**

| | |
|---|---|
| gated heuristic (elongation > 3) | **1.04×**, free, no search |
| full 12-angle search | 1.10×, ~2 min per region |

The gated rule captures the largest region's whole plateau, skips the two where
no axis exists, and costs one covariance matrix.

Note the direction, which is the opposite of the textbook rule and worth stating
so nobody "fixes" it later: passes run along the **minor** axis, i.e. *across*
the narrow dimension. The classical "sweep along the long axis" minimises pass
COUNT; this workload is bound by kept RETRACTS, and stepping along the long axis
keeps consecutive passes adjacent so the relinker can chain them. `§0e`'s table
is the evidence: fragments span 471–597 while time spans 933–1155 s, and
retracts track time.

### Honest value

The three regions measured are 6,513 mm² of a ~9,000 mm² shallow band, which is
itself ~30% of tier 1's covered area (MidSteep is 27,488 mm² and already
contoured). So a 1.04–1.10× gain on the shallow band is **low single-digit
percent of the operation**. Real, cheap in the gated form, and not a campaign.

That is the honest ceiling for *any* sweep-direction work here, and it is why
the next real step is cell decomposition (§0e) rather than a better angle
heuristic: one angle per region cannot beat a region that needs three.

---

## 0g. C1+E1 MEASURED (2026-08-28) — cells reduce kept retracts, but only 9.6% on the top three regions.

Stages I/J in `tests/thin_organic_island_widths.rs` ran a **measurement-only,
emitted-lattice** boustrophedon decomposition of the same three Wanaka Shallow
regions used in §0d–§0f.  A cell is continued only across a one-to-one overlap
of consecutive raster scan-row runs; a split or merge closes the old cell(s)
and starts new one(s).  Thus every cell is Y-monotone for the shipped 0° grid
(paths run X).  This is not production `Polygon2` decomposition.

The candidate is deliberately costed fairly: both the undivided and cell arms
use the same 0° drop-cutter grid, feeds, Shapeoko Pro XXL F-034 kinematics,
full original-region boundary, `reorder: true`, `link_ceiling: None`, and
**both go through `relink_fragments`** before costing.  The candidate is
refused unless its reconstructed cell polygons select exactly the baseline's
emitted raster lattice points.  This is the §0d lesson applied mechanically.

| region | area | lattice cells | baseline kept retracts | cells kept retracts | F-034 baseline → cells |
|---|---:|---:|---:|---:|---:|
| 1 | 3104 mm² | 86 | 97 | **83** | 1053.7 → **999.9 s** |
| 2 | 1788 mm² | 35 | 30 | **33** | 478.0 → **492.5 s** |
| 3 | 1621 mm² | 58 | 40 | **35** | 483.7 → **463.0 s** |
| **top-three total** | **6513 mm²** | **179** | **167** | **151** | **2015.4 → 1955.4 s (1.03×)** |

So the predictor moves in the expected aggregate direction: **16 fewer kept
retracts (−9.6%) buys 60.0 s (−3.0%)**.  But it is not universal — region 2
regresses by three retracts and 14.5 s — and this is far below the pre-measure
intuition that decomposition would erase the remaining topology cost.  The
cell arm has fewer input fragments (985 → 342) and links (815 → 188), but the
relinker had already eliminated most of the original junctions.

### What this establishes, and what it does not

- **C1 is answered for the baseline direction:** the largest three regions are
  not three cells; they contain 86 / 35 / 58 emitted-lattice monotone cells.
  Stage I also prints every cell's area, PCA-minor diagnostic and monotone
  direction for inspection.
- **E1 exists as a reusable evidence kernel:** `relink_and_cost` accepts a raw
  candidate toolpath, relinks it with the real production parameters, and
  returns F-034 time, cutting distance, fragments, links and kept retracts.
  Future C/D candidates must use it, not compare raw generators.
- This is **not C2/C3**.  There is no production cell geometry, cell adjacency
  graph, cell TSP, per-cell direction or GUI overlay.  The candidate's global
  relink is intentionally optimistic relative to an eventual cell router.
- Matching raster lattice points does **not** prove identical connecting-feed
  geometry: cutting distance changed 17,544 → 17,394 mm (−0.9%).  The C4
  rendered-surface review still binds any implementation.
- These are still the fresh-stock (`link_ceiling: None`) arms.  A3 must
  re-baseline against the corrected live ceiling before turning this 1.03×
  measurement into an operator-time claim.

**Decision:** cell decomposition survives its cheapest falsifier, but as a
modest / conditional C-track investment, not the prior headline lever.  Do
not build C2 solely to chase retracts; first price a C3 cell order and D1
per-cell directions against this rig, where a material gain must beat the
measured 1.03× baseline.

## 0h. Rotated-cell precursor (2026-08-28) — the visual objection was right on region 1.

The 0° overlay in §0g made the limitation visible: all cells were aligned to
one arbitrary board axis.  Stage K therefore took **only region 1** — the sole
region whose PCA predictor passed §0f's elongation gate (4.15) — rebuilt the
raster lattice at its measured PCA-minor pass direction (**119.6°**), and
re-decomposed it.  This is a fresh decomposition in the rotated grid, not the
invalid operation of merely rotating the 0° cells.

| candidate, region 1 | cells | fragments | kept retracts | F-034 time | cutting distance |
|---|---:|---:|---:|---:|---:|
| 0° undivided | — | 564 | 97 | 1053.7 s | 8687 mm |
| 0° cells | 86 | 191 | 83 | 999.9 s | 8553 mm |
| PCA-minor undivided | — | 491 | 74 | 963.0 s | 8501 mm |
| **PCA-minor cells** | **69** | **141** | **53** | **875.9 s** | **8279 mm** |

Within the rotated direction, decomposition removes **21 / 74** remaining
retracts (−28.4%) and **87.1 s** (−9.0%).  Against the 0° undivided baseline,
the combined direction + cells candidate is **44 fewer retracts** and **177.8
s faster (1.20×)**.  Unlike §0g's aggregate 1.03× result, that is a material
single-region signal.

The visual companion makes the mechanism legible:

```text
/home/ricky/Downloads/svg/wanaka_monotone_cells_region_1.svg
/home/ricky/Downloads/svg/wanaka_monotone_cells_region_1_pca_minor.svg
```

The rotated view is qualitatively less like parallel slices imposed on a
branching map: several long fingers become single or few cells.  That is an
operator sanity check, **not** a quality acceptance — rotating a raster
changes the cusp pattern, and the 8279 mm versus 8687 mm cutting distance also
proves its connecting-feed geometry differs.  Both arms of each same-direction
comparison still select the same emitted lattice population, pass the same
production relink, and use the F-034 integrator; the cross-direction comparison
requires C4 rendered/simulated surface review.

**Decision:** this is enough evidence to price C2/C3 + D1 as one constrained
prototype, starting with the PCA-gated region-level direction rather than
trying to assign 179 independent angles.  It is not evidence to ship a
per-cell strategy yet: one region, fresh-stock ceiling, no cell adjacency
router, no GUI preview and no C4 visual-quality acceptance.

## 1. Ranked plan

**Rank 1 — Lever 1: contour-parallel (scallop ring cascade) for THIN regions.**
**Rank 2 — Lever 2: per-region sweep direction.**
Lever 2 is genuinely cheaper to build but its ceiling is much lower on *this*
shape class; see §1.3 for why the ranking is not close.

### 1.1 Lever 1 — route a thin region's shallow band through the ring cascade

#### Why it works here specifically

The mid-steep band already runs the region's own polygon through
`scallop_toolpath_structured_annotated_with_cancel` with `continuous: true`
(`unified_finish.rs:1931-1985`), and that call is **fully self-contained**: it
builds its own surface and slope map (`scallop.rs:2059`), takes the region set
as its ring boundary (`scallop.rs:2178-2185`), and needs nothing from the
band. T1's "the decompose steps are band-agnostic" finding extends to the
generator: the MidSteep arm's body would run verbatim on a Shallow region.

For a thin finger the cascade's rings run **along** the finger and collapse
after `⌈w / 2s⌉` iterations. Fragment count per island goes from
`(extent/s) × crossings` to `O(rings)`, and `continuous: true` chains those
rings into a spiral so most of them are not even junctions.

Derived prediction on tier 1 (`s = 0.2821`, island widths of order 1.5–3 mm on
the dendritic mask): **4–6 rings per finger cross-section, O(10) emitted
fragments per island, ≤24 islands** → **O(10²) junctions against today's
1.7 × 10⁴**. Even with a heavy discount for bifurcation splitting, that is
**20–100× fewer junction events.**

Two more arguments that matter more than the headline, because they are about
*why the cascade's known pathologies don't bite on this shape class*:

- **`ring_stepover` takes the MIN across the whole ring**
  (`scallop.rs:2131-2140`, quoted: *"a single steep sample sets the advance for
  the WHOLE ring. On terrain every large ring touches steep ground, so the
  cascade crawls at the worst-case rate."*). A thin **Shallow** island is
  slope-homogeneous by construction — it is below `steep_threshold_deg` and it
  is small — so the min-across-ring reduction has almost nothing to reduce.
  The pathology is an *area × slope-spread* pathology.
- **`max_rings` is not binding.** `scallop.rs:2167`:
  `max_rings = (max_extent / min_stepover) × 0.5 + 10`, with `max_extent` the
  **whole mesh** extent. On tier 1 that is `(200 / 0.2821) × 0.5 + 10 ≈ 364`
  rings of budget for a region that needs ~6. *(Derived.)* This is the reason
  `StepoverGeometry::CosineSlope` (the corrected slope law, `scallop.rs:375`)
  **can** be adopted for this arm even though Checkpoint C ruled it must not be
  landed alone globally: the coupling that blocked it —
  *"the stepover law and the ring budget are one problem"*
  (`scallop_math.rs:127-131`) — is a truncation coupling, and a thin region
  cannot truncate against a 364-ring budget. Tripwire: `uncut_core_mm2` /
  `untouched_material_mm2` must read `0.00` on every contour-routed region.

#### The quality hazard, stated

`scallop_math::variable_stepover` (`scallop_math.rs:143`) is **inverted in
slope**: it returns `R/cos θ` where the geometry wants `× cos θ`, i.e. it is
`1/√cos θ` too wide (documented table: 1.24× at 30°, 1.69× at 45°).

The shallow raster has the *sec θ* half of this problem too (rows are spaced in
XY and nothing compensates), so the honest comparison is only the **extra**
`1/√cos θ`: **1.00× at 0°, 1.07× at 30°, 1.11× at 40°**, and "Shallow" tops out
at `steep_threshold_deg = 45`. Cusp goes as `d²`, so worst case ≈ **1.2× the
raster's cusp at the band's own upper edge**, falling to parity on flat ground.

Two ways to close it, in preference order:
1. Pass `StepoverGeometry::CosineSlope` for this arm (see the `max_rings`
   argument above). Requires exposing the policy on the production entry —
   `scallop_toolpath_research` (`scallop.rs:2021`) already takes it, so this is
   a plumbing change, not new math.
2. Or clamp the thin-shallow arm to the flat-ground stepover
   (`stepover_from_scallop_flat`, `scallop_math.rs:29`), giving exact parity
   with the raster's spacing law by construction.

#### Thinness metric — what is computable today

`grid_field::distance_transform_2d` (`grid_field.rs:79`, Felzenszwalb &
Huttenlocher separable EDT, O(cells)) is already used exactly this way at
`region_mask.rs:194`. Feed it the **complement** of a band mask and every
inside cell reads its distance to the outside; the max over a connected
component is that component's inradius, so

```
min_width_mm = 2 · cell · max{ EDT over the component }
```

is exact for a strip (`= W`) and exact for a disc (`= 2r`) — unlike the
area/perimeter proxy, which halves on blobs and would mis-route them.

Home for the computation: **`finish_planner::decompose`**, which already owns
the label grid, the cell size and the component labelling
(`finish_planner.rs:433-465`, `absorb_small_regions`,
`count_components`). One EDT per band, three bands, one pass — single-digit ms
on the 445k-cell planning grid *(derived from the O(cells) bound; not timed)*.

The alternative home, `tier_islands.rs:458`'s `owned_mask`, is **wrong**: the
finish op re-decomposes its own bands *inside* the tier boundary, so tier
islands and planned regions are not 1:1.

Routing rule: `band == Shallow && min_width_mm <= THIN_K · raster_stepover`.
`THIN_K` is the one new dial; a starting value of **8** (≈ 4 rings per half
width, 2.3 mm on tier 1) is the natural anchor and should be swept.

#### Touch points

| # | File:line | Change | Size |
|---|---|---|---|
| 1 | `finish_planner.rs:235-238` | `PlannedRegion` gains `min_width_mm: Option<f64>`; `None` = **not measured** (the `ToolpathStats` contract) | ~5 |
| 2 | `finish_planner.rs:433-465` + the Step-6 extraction site | per-component EDT inradius; one pass per band | ~60 |
| 3 | `unified_finish.rs:664-671, 724-731` | `RegionStrategy` becomes a **stored** field on `RegionTableEntry` / `RegionPath`; `RegionKind::strategy()` becomes the default, not the truth | ~40 |
| 4 | `unified_finish.rs:733-783` | `RegionKind::ALL` / `span_label` / `from_span_label` gain the new node kind. **`span_label` is a compatibility surface** (TSP span remap, GUI span list, `v3_cascade_ab`'s `strategy_of_span`) — ADD a label, never re-spell `"Shallow band"` | ~25 |
| 5 | `unified_finish.rs:1855` | the thin-shallow arm: reuse the MidSteep body at `1931-1985` with `h` derived from `raster_stepover` via `scallop_math::scallop_height_flat` so the two bands' cusp dials cannot drift | ~50 |
| 6 | `scallop.rs:1887-1906` | expose `StepoverGeometry` (or the flat clamp) on a production entry point | ~20 |
| 7 | `unified_finish.rs:2208-2213` | `BandGenStats` accounting must not silently merge a contour-routed Shallow region into `report.shallow`'s raster stats | ~15 |

**~215 production lines + ~350 sentry lines.** One core editor; no viz/MCP
change (per-tier diagnostics already ride the per-op rows). Sentries, red
first: (a) a synthetic 2 mm × 60 mm strip at 0° emits **one** spiral node and
**zero** in-node retracts, where today it emits ~210 fragments; (b) a 40 mm
disc stays on the raster (`min_width_mm` = 40, above `THIN_K·s`); (c)
`uncut_core_mm2 == 0.00` on every contour-routed region; (d) `THIN_K = ∞`
reproduces today byte-identically — the repo's standing golden discipline.

#### Measurable prediction (tier 1, wanaka200)

| Instrument | Where it is read | Today | Predicted |
|---|---|---|---|
| `retract_trips.in_node` | `narrate.rs:1346`, `RetractTripCount` (`compute/config.rs:711`) | 1,263 (post-G-LINKVETO) | **< 200**, and the residual attributable to inter-island hops |
| `RelinkTotals.fragments` | `narrate.rs:1387` relink declines line | O(1.7 × 10⁴) | **O(10²)** — 20–100× fewer |
| `RelinkTotals.surface_links` | same | ~16,000 | **< 300** (the junctions no longer exist to link) |
| rapid_mm / cutting_mm | cut-trace per-op | 4.19 pre-fix (363.8 m / 86.8 m) | **< 0.3** |
| tier-1 op runtime | `toolpath_runtimes` | — | the target is Phase V's `rapid_s / cutting_s < 0.5` bar, met with margin |
| cutting distance | `ToolpathStats` | 86.8 m | **−7 % to −21 %** — Feng & Li's measured constant-cusp-vs-raster path-length result (§5), NOT a repo measurement |
| `uncut_core_mm2` | `ToolpathStats` | n/a (raster) | **0.00** — the cascade-truncation tripwire |
| `air_cut_pct_of_cutting_time` | narration | rising post-G-LINKVETO | **falls back**, because the link feeds inflating it disappear |

#### Risks

- **Cusp *pattern* changes visibly.** Ring cusps follow the island outline;
  raster cusps are parallel ridges. On a terrain relief this may read better
  (topographic) or worse. The C4 rule stands: the operator's eye is the
  accepting gate, so the tier-map/render eyeball is part of the A/B, not
  after it.
- **Per-region surface rebuild.** `scallop.rs:2059` builds a **mesh-global**
  heightmap + slope map **per call**, and `unified_finish` calls per region
  (`unified_finish.rs:1829-1832` already flags this: *"accepted and flagged as
  a P2.e datapoint if conditioned region counts ever grow"* — tier islands are
  exactly that growth). Scale from `CLASSIFICATION_PERF_STUDY.md` §4.1:
  2.990 s wall / 40.70 s CPU for an 849² grid on a 215k-triangle terrain;
  wanaka is 661k triangles, so **~3–6 s per call, × up to 24 islands ≈ 1.5–2.5
  minutes per tier** *(derived, not measured)*. Mitigation, in order:
  (i) batch all contour-routed regions into ONE scallop call — `scallop.rs:2178`
  already iterates a multi-region boundary internally off a single surface
  build, at the cost of the router's freedom to order those regions; or
  (ii) a finish-surface cache keyed like `geom_cache` (`Weak` + `Arc::ptr_eq`)
  — `finish_setup.rs` has **no cache at all** today.
  **This must be priced before the A/B, or a generation-time regression will be
  misread as a toolpath regression.**
  **RESOLVED 2026-08-27 — arm (ii) landed**, see §1.4.
- **Offset-library exposure.** The cascade calls `offset_polygon` per ring per
  region; `offset_library_failures` counts *calls*, and two of the three panic
  classes are `debug_assert!`s in the dependency, so **the count is not
  comparable across debug and release builds** (CLAUDE.md). Compare release to
  release.

### 1.2 Lever 2 — per-region sweep direction

#### The good news: the rotated raster already exists

`batch_drop_cutter_with_cancel` takes `direction_deg` and has a full rotated
path (`dropcutter.rs:110-165`, rotation branch from `:167`). `CLPoint.x/.y` are
**always world-frame** (`dropcutter.rs:53-56`), so
`raster_toolpath_from_grid` — which reads `grid.get(row,col).position()`
verbatim — is already rotation-correct with no change.

Downstream is safe by precedent: the 2.5D families ship an arbitrary raster
angle today and it is live, not a dead dial — `PocketConfig.angle`,
`ZigzagConfig.angle`, `RestConfig.angle`
(`compute/operation_configs.rs:321, 460, 519`) reach
`zigzag::zigzag_lines_reported` through `compute/execute.rs:1057, 1250, 1503`.
Dressups, TSP, spans and links all consume plain move lists.

**But production has never used it.** `rg direction_deg` returns only `0.0`
literals and comments (`compute/execute.rs:2723`, `steep_shallow.rs:453`,
`toolpath.rs:1266`, two tests, one bench). Two traps the code itself names:

- `dropcutter.rs:126-134` — **90°, 180° and 360° take the axis-aligned fast
  path**, i.e. they do *not* re-derive a rotated grid. Any per-region angle
  must either avoid those values or the branch must be fixed.
- `compute/execute.rs:2723` and the `SlopeMap::from_z_grid` conversion assume
  `direction_deg == 0.0` so that `u_start`/`v_start` read as world minima.
  Nothing on the unified Shallow path does that conversion, but a future
  consumer might.

#### The bad news: cost, and ceiling

- **Cost.** The grid is memoised **once per op** (`unified_finish.rs:1842`)
  precisely because it is mesh-global. A per-region angle destroys that
  sharing: one full-board drop-cutter grid per distinct angle. At tier 1's
  0.2821 mm over 200 mm that is ~709² ≈ 500k drop-cutter queries per grid.
  Extrapolating `CLASSIFICATION_PERF_STUDY.md` §4.1 (425² → 0.754 s wall /
  10.59 s CPU at 215k triangles) to 709² on 661k triangles gives **~5–10 s wall
  per grid**, × up to 24 islands *(derived)*. **The fix is a bbox parameter**:
  `dropcutter.rs:120` hardcodes `mesh.bbox.expand_by(r)`; sampling only the
  island's own bbox makes each grid 1–3 % of the board and the cost vanishes.
  Treat that as a prerequisite, not an optimisation.
- **Ceiling.** A single angle per island only helps an island that has a single
  dominant elongation. After the coarseness slider's morphological close, a
  wanaka tier-1 island is a *web* whose fingers radiate — the whole reason the
  ~24-crossings-per-row figure exists. A per-island angle reduces crossings for
  the fingers aligned with it and leaves the rest. Expected **1.3–2×** fewer
  fragments, against Lever 1's 20–100×. Getting more requires splitting each
  web into monotone cells first — which is Choset's boustrophedon
  decomposition (§5), i.e. new machinery, not a dial.

#### Choosing the angle

No convex hull exists in production (`rg convex_hull` finds only the
literature-matrix test harness). Two options:

- **PCA on the region's cells** — second moments of the island's mask cells,
  sweep **along** the major axis (so passes are long and few, and the step is
  across the narrow width). ~15 lines, O(cells), works on the mask already in
  hand at `finish_planner`'s Step 6 or `tier_islands.rs:458`.
- **Rotating calipers minimum width** of the convex hull — the textbook answer
  and correct for convex regions, but it needs a hull primitive and it is
  *wrong-shaped* for a dendritic web anyway (the hull of a web is a blob).

Recommend PCA. It shares its input with Lever 1's EDT, so both levers can read
one mask pass.

#### Touch points

| # | File:line | Change | Size |
|---|---|---|---|
| 1 | `dropcutter.rs:110-120` | optional sampling bbox (default = `mesh.bbox.expand_by(r)`, byte-identical) | ~25 |
| 2 | `dropcutter.rs:126-134` | make 90°/180° take the rotated path, or refuse them | ~10 |
| 3 | `finish_planner.rs:235-238` | `PlannedRegion` gains `sweep_deg: Option<f64>` (PCA major axis) | ~30 |
| 4 | `unified_finish.rs:1842, 1987-2090` | grid memo keyed by `(angle, bbox)` instead of a bare `Option`; pass `region.sweep_deg` | ~45 |

**~110 production lines + ~200 sentry lines.** Sentries: a 60 × 6 mm strip at
30° emits `⌈6/s⌉` fragments, not `⌈60·sin30°/s⌉`; `sweep_deg: None` is
byte-identical to today; a 90°-angled region really does produce a rotated
grid (the fast-path trap).

#### Measurable prediction (tier 1)

| Instrument | Today | Predicted |
|---|---|---|
| `RelinkTotals.fragments` | O(1.7 × 10⁴) | **1.3–2× fewer** — 0.9–1.3 × 10⁴ |
| `retract_trips.in_node` | 1,263 | 600–1,000 |
| cutting distance | 86.8 m | **≈ unchanged** (coverage area is unchanged; only the sweep direction moved) |
| generation wall time | — | **up**, unless touch point 1 lands first |

### 1.3 Why the ranking is not close

They are not the same kind of change. Lever 2 re-orients a fixed pattern;
Lever 1 replaces a pattern whose fragment count is `O(extent/s)` with one whose
fragment count is `O(width/s)`. On a web, `extent ≫ width` by definition — that
ratio *is* the defect. Lever 2's benefit is bounded by how anisotropic the
island happens to be; Lever 1's benefit grows as the island gets thinner, which
is the direction the fine tiers are heading.

Two secondary reasons:

- Lever 1 reuses a **shipped, sentried generator** (`scallop`) that the
  mid-steep band already drives per region. Lever 2 activates a rotated code
  path with **zero production callers today** — untested in the sense that
  matters.
- The two known cascade pathologies (min-across-ring; `max_rings` truncation)
  are area/slope-spread pathologies that thin slope-homogeneous islands do not
  trigger (§1.1). The cascade is a *better* algorithm on this shape class than
  it is on the whole board — which is the opposite of how it is usually
  discussed here.

They compose: build Lever 1, and let Lever 2 handle the residue — the regions
that come back "not thin" but are still elongated.

### 1.4 One prerequisite that is neither lever — **BUILT 2026-08-27**

The **per-call mesh-global surface build** (`scallop.rs:2059`, no cache in
`finish_setup.rs`) was on the critical path for *both* levers, and already for
Phase O/V as written: `unified_finish.rs:1829-1832` explicitly deferred it
until "conditioned region counts grow", and tier islands (cap 24 per tier,
`tier_islands.rs:269`) are that growth. Unpriced, a generation regression would
have been indistinguishable from a toolpath regression in the A/B.

**Landed as `finish_surface_cache.rs`** — mitigation arm (ii). All three
generation consumers (`scallop.rs`, `ramp_finish.rs`, `steep_shallow.rs`) now
go through `cached_finish_surface`, which memoises the *uncached* builder, so a
cached surface and a fresh one cannot diverge by construction. Capacity 2
(a `FinishSurface` is 49 B/cell — 3.7 MB for the shipped wanaka tapers, 127 MB
for a Ø1 ball at `envelope/4`).

Two things a later reader needs to know, because they are deviations from the
plan as briefed:

- **The key is CONTENT, not `Weak` + `Arc::ptr_eq`.** The brief asked for
  `geom_cache`'s discipline verbatim; it is not reachable. The
  `Arc<TriangleMesh>` stops at `ResolvedGenInputs` in `session/compute.rs` —
  `ExecutionContext::mesh` is `Option<&'a TriangleMesh>`, and `unified_finish`
  receives and forwards a bare `&TriangleMesh`. Threading an `Arc` down would
  have to pass through `unified_finish.rs`, whose per-region loop *is* the hot
  path. Content keying closes the ABA hazard harder anyway:
  identity-implies-content is what `geom_cache` must argue for separately,
  whereas here it is the key. Cost is a `DefaultHasher` digest over
  `vertices` + `triangles` (~16 MB on the reference mesh, single-digit ms)
  against the 3–6 s build it avoids. `faces` is counted but not digested — it
  is a pure derivation of those two at all four `mesh.rs` construction sites,
  with no in-place mutation in the workspace.
- **The generation-surface census sentry had to be widened.**
  `checkpoint_b_resolution_ab::only_three_consumers_can_see_the_generation_resolution`
  proves UnifiedFinish's waterline and raster bands never see a generation
  resolution. The memo adds a second door to the same builder, so matching only
  the direct builder would have dropped `ramp_finish.rs` and `steep_shallow.rs`
  off its list while they still consumed one — the sentry would have gone
  **green on a claim it had stopped testing**. It now matches all three
  spellings and skips the memo as plumbing.

Verified 2026-08-27: `cargo check --lib` and `--all-targets --features
heavy-tests` clean; `clippy -D warnings` clean; the memo's own 12 tests pass,
including `repeated_scallop_calls_share_one_surface_build` (two scallop calls,
**one** build — the win, measured off a counter inside the builder rather than
a stopwatch) and `a_cached_surface_equals_a_fresh_build` (bit-for-bit).
Output is unchanged: `finish_resolution_policy_pr3`'s pinned 2,820-move scallop
fingerprint still holds.

---

## 2. A/B design on wanaka200

Runnable with the instruments already in the tree. One cargo job machine-wide;
release binary pre-built before the MCP connect; memory watcher armed.

**Sim resolution follows the tool, not a fixed number** (operator rule,
2026-08-27): roughly **tip diameter ÷ 10**, and a run is bound by its
*smallest* tool. That is why 0.1 was ever needed — the R0.5 pencil forced it,
not the board. For this A/B:

| ladder tool | tip Ø | sim cell |
|---|---|---|
| R2.0 | 4.0 mm | 0.4 |
| R1.0 | 2.0 mm | **0.2** |
| R0.5 (pencil) | 1.0 mm | 0.1 |

So the timed arms run the **R2.0 → R1.0** ladder at **0.2 mm** — coarser than
the ledger's 0.15, which is faster and sidesteps the memory wall that killed a
prior CLI session mid-A/B. Keep the pencil out of the timed arms; price it
separately if its fate is being decided (§3.4 of the orchestration plan).

The honest cost of that choice: **engagement grades are not claimable at
0.2 mm.** The ledger already measured 58% of tier-B's material-removing
samples reading zero engagement at 0.15 with an R1.0 tip, and a coarser cell
is worse. Time, collisions, retract counts and relink attribution all stand —
and the decisive bar here is integrator-costed runtime, not engagement.

### Arms

| Arm | Config | Purpose |
|---|---|---|
| **A0** | `wanaka200_mt2.toml` as it stands after tonight's fixes (G-LINKVETO + hookup 25 + z_step equal-cusp + pinned `bottom_z`) | the baseline every other arm is read against — **must be re-measured, not quoted**, because the ledger's 17,083/363.8 m/86.8 m predate all four |
| **A1** | A0 + Lever 2 only (`sweep_deg` per region) | isolates sweep direction |
| **A2** | A0 + Lever 1 only (`THIN_K = 8`), shipped slope law | isolates topology |
| **A2′** | A2 + `StepoverGeometry::CosineSlope` on the thin arm | isolates the slope-law correction from the topology change — the two must not be confounded, which is the §3.4 lesson from Checkpoint C |
| **A3** | A1 + A2′ | composition |

Sweep `THIN_K ∈ {4, 8, 16}` on the winning arm only.

### Controls

- **Tier 0 must be byte-identical in every arm.** It is unconfined and has no
  thin regions; if its toolpath moves, the change leaked. Golden-compare the
  serialized op, the repo's standing discipline (`island_stay_down_links_o3`'s
  fresh-stock arm does exactly this).
- `THIN_K = ∞` / `sweep_deg = None` must reproduce A0 byte-identically. Run it
  once as arm **A0′** to prove the seam is inert before trusting any delta.

### Per-arm measurement set

All of this exists; nothing new needs building.

1. **`narrate_toolpath(tier k)`** — region/node count, cut runs vs
   marching-squares regions, air-cut line, Z ladder, peak axial DOC, plus the
   two Phase-O additions: `append_retract_trips` (`narrate.rs:1346`) and
   `append_relink_declines` (`narrate.rs:1387`), which give
   `too_far / off_surface / slower_than_retract / outside_boundary /
   ceiling_above_safe_z` and `fragments / linked`.
   Narration is ~4 ms — run it on every arm, every tier.
2. **Cut trace** — per-op `toolpath_runtimes`, cutting_s vs rapid_s,
   cutting_mm vs rapid_mm.
3. **`RetractTripCount`** — `total`, `in_node`, `between_nodes`,
   `in_node_rapid_mm`, `between_nodes_rapid_mm`. **Check `has_split()` first**:
   an untrustworthy split reports as unmeasured, never as a confident zero
   (`compute/config.rs:737-744`).
4. **`ToolpathStats`** — `uncut_core_mm2`, `untouched_material_mm2`,
   `reached_uncut_estimate_mm2`, `offset_library_failures`,
   `boundary_clip_dropped`, `zero_removal`. `None` = not measured; never
   coerce to zero.
5. **`get_diagnostics().triage`** — safety first (collisions, holder strikes),
   then actions, then advisories. `rapid_collision_count` is the primary
   "did anything bad happen" signal.
6. **`get_tool_load_report()`** — gates must read `Within` with **non-vacuous
   populations**: check `sample_count` / `sample_range` before believing any
   verdict (the empty-population trap).
7. **Measurability** — `MeasurabilityReport`. At 0.15 mm cells the ledger
   already records **58 % of tier-B's material-removing samples reading zero
   engagement** (R1.0 tip below cell resolution). Time and collision verdicts
   stand; **engagement-grade claims about the fine tier do not** — say so in
   the write-up rather than quoting an engagement number.
8. **Operator eyeball** — tier-map preview overlay before generation, final
   render after. The cusp *pattern* change is a look change, and the eye is the
   accepting gate for surface quality (C4).

### Derived instruments worth computing by hand

- **Coverage ratio** `= owned_area_mm² / (raster_stepover × cutting_mm)`.
  Ideal coverage of tier 1's 8,815 mm² at s = 0.2821 is 31.3 m of cutting
  *(derived)*; A0 cuts 86.8 m, a ratio of ~2.8×. Some of that gap is real
  (the overlap band, the mid-steep scallop's tighter variable stepover, the
  waterline band) — but it is the single number that says whether a change
  bought coverage efficiency or just moved air around.
- **Junctions per metre cut** `= RelinkTotals.fragments / cutting_mm`. This is
  the quantity both levers actually move, and it is machine-independent.

### Bars

| Gate | Bar |
|---|---|
| tier-0 golden | byte-identical across all arms |
| A0′ inert-seam | byte-identical to A0 |
| `RelinkTotals.fragments`, tier 1 | A2 ≤ 5 % of A0 (i.e. ≥ 20×); A1 ≤ 75 % of A0 |
| `retract_trips.in_node`, tier 1 | < 200 (A2) |
| rapid_s / cutting_s, tier 1 | **< 0.5** — Phase V's own bar |
| `uncut_core_mm2` on contour-routed regions | 0.00 |
| collisions | 0 / 0, all arms |
| load gates | `Within` with non-vacuous `sample_range` |
| total ladder time | **< 17,088 s** (beat C2); stretch **< 15,376 s** (beat P2) |
| generation wall time | reported per arm — a 2 min/tier regression from §1.4 is acceptable, an unreported one is not |
| operator eyeball | tier-map veto before generation, render after |

### Sequencing

Run **A0** and **A0′** first. If A0 does not reproduce the ledger's shape, stop
and re-attribute — four things changed tonight and none of them has a
post-change measurement in the ledger yet.

---

## 3. Lever 3 — vector-field / iso-scallop streamlines: honest assessment

**The field machinery already exists in this repo, was benchmarked, and was
deliberately NOT adopted. Any revival is a re-benchmark, not a build.**

`crates/rs_cam_core/src/scallop_isofield.rs` is a complete iso-scallop
level-set implementation: `|∇D| = 1/s(x,y)`, `D = 0` on the region boundary,
passes are the integer level sets, solved by fast sweeping with the Godunov
upwind update, extracted by marching squares
(`scallop_isofield.rs:1-69, 122, 405`). It is reachable only through
`scallop::RingSource::IsoField` and **no shipped caller selects it**.

What the measurement said (`planning/review_2026-07-29/CHECKPOINT_C_EVIDENCE.md`):

- §5 gates: no truncation on any of 6 fixtures at both resolutions; flat
  segments not forced to the steep segment's stepover **by construction**;
  runtime **0.42–1.03 s vs the cascade's 0.40–0.67 s** — *not slower*.
- §3.8 costs: deepest single gouge point **−1115 µm / −995 µm vs the cascade's
  −108.6 µm** (area better: 2.40 / 5.01 vs 12.29 mm²); 0.0–0.5 % of segments
  under 10 µm (needs a decimation pass the cascade already has); ring placement
  grid-quantised; saddles in `D` merge/split level sets silently.
- §7 recommended adoption. **§15 revised that to Option 4 — keep the seam,
  leave production on the offset cascade** — because a chord-fidelity fix
  showed the 2–3× quality margin *belonged to the instrument*. §14: no
  production path moved; the end-to-end COLUMNS A/B was never run; any revival
  **must re-baseline from `dde7a54`, not `28503db`**.

### What a minimal streamline prototype could reuse

- The Eikonal solve, boundary seeding (signed, so the interpolated zero lands
  on the true region edge — `scallop_isofield.rs:88-101`), level extraction,
  ring decimation (`decimate_closed_ring`), 3D lift and chord refinement — all
  shared with the cascade branch, which is what makes the comparison isolate
  ring *placement* alone.
- `grid_field::distance_transform_2d`, `marching_squares`, `SlopeMap`,
  `region_mask`, the F-034 link/route costing, the whole `RelinkTotals` /
  `RetractTripCount` instrument set.
- `adaptive3d`'s `clear_z_level_adaptive` already uses the same architecture
  (EDT → curvature field → per-cell threshold), so it is not foreign here.

### What it cannot reuse

- **A direction field.** `scallop_isofield` solves a *distance* field whose
  level sets are contours — it is the iso-scallop half, not the
  preferred-feed-direction half. Chiou & Lee / Kim & Sarma / Zou et al. build a
  **vector field** and then place streamlines *along* it, with spacing set by
  the scallop constraint. Nothing in the repo constructs, smooths, or
  singularity-handles a direction field, and nothing places streamlines
  (Jobard & Lefer's queue-and-reject placement is ~150 lines but has no home).
- **Ring identity downstream.** §3.8's own warning: saddles merge or split
  level sets silently, and `RegionTableEntry` / `ScallopRuntimeAnnotation` /
  the span vocabulary all assume node identity.
- **A cost function to optimise.** The literature's objective is path *length*
  (Zou et al. minimise it globally via a Poisson formulation). This repo's
  actual objective is **accel-aware cycle time** through the F-034 integrator,
  and `research/multitool_finishing_optimization.md` records that gap as
  genuinely open: *"all the verified tool-selection work optimizes path length
  or MRR, not accel-aware time."* A streamline field optimised for length can
  lose on this machine — `STRATEGY_ADVISOR_2026-06-17.md` measured exactly that
  shape of surprise on wanaka (parallel 446 s vs spiral 828 s at the load
  limit, on a low-accel belt router).

### Verdict

**Do not start Lever 3 as an alternative to Levers 1–2.** It is the same
family of answer as Lever 1 — contour-following coverage — with a strictly
larger blast radius and one adopt-then-retract already on the record. The
cheapest honest experiment, if the appetite exists after the A/B, is to route
**only the thin-shallow arm** through `RingSource::IsoField` as a fifth A/B arm
(the seam already accepts it, `scallop.rs:1246`), re-baselined from `dde7a54`,
with the §3.8 gouge and short-segment findings as pre-declared tripwires. That
is a config-level experiment inside work Lever 1 has to do anyway — perhaps
20 lines — and it answers the architecture question with a wanaka-class number
instead of an argument.

---

## 4. Correcting two premises in the brief

Both matter because they would have anchored the plan to the wrong evidence.

**(a) "The June 2026 contour-spiral vs parallel evidence — spiral won
wall-clock, travel AND air-cut on organic 2D shapes."** The June evidence is
real but it is **3D roughing, contour-spiral vs the AgentSearch clearing
strategy**, not spiral vs parallel raster, and not 2D. Resolved 2026-06-15 at
`TROCHOID_CAP_MULT_3D = 1.6` on wanaka200: **12,075 s vs 17,839 s (32 %
faster)**, 247k vs 230k mm cut (+7 %), **24k vs 87k mm rapid (3.6× less)**,
**15.6 % vs 39.6 % air**, flat load, 0 collisions.

The transferable part is the *mechanism*, and it transfers well: on **this
mesh**, a contour-following stay-down strategy beat a retract-heavy one by
3.6× on rapid distance and 2.5× on air-cut. That is the same lever Lever 1
pulls.

The countervailing datum, which must travel with it:
`planning/STRATEGY_ADVISOR_2026-06-17.md` measured **parallel 446 s vs spiral
828 s (1.85×)** on wanaka at the load limit for 2D adaptive, and concluded
*"for a light wood router, spiral is the exception, not the default"* — because
contour paths chain short chords and the Grbl junction-deviation model crawls
every corner. A ring cascade at a 10 µm cusp emits short chords by
construction. **This is why the A/B's decisive number is integrator-costed
runtime, not fragment count** — fragment count is the mechanism, runtime is
the verdict.

**(b) "Kim & Sarma — iso-scallop spacing."** Kim & Sarma (2002) is *"Toolpath
generation along directions of maximum kinematic performance; a first cut at
machine-optimal paths"*, CAD 34(6):453–468 — about the **anisotropy of the
machine's performance envelope** (motor speed limits), not scallop spacing. It
is arguably *more* relevant to this repo than the iso-scallop citation would
have been, given (a) above: it is the published form of "the best direction to
sweep depends on the machine, not only the geometry". The iso-scallop
attribution belongs to Suresh & Yang (1994) and Lin & Koren (1996).

---

## 5. Literature — claim / source / verified how

The repo already has a verified log for most of this:
`research/multitool_finishing_optimization.md` (two adversarial sweeps,
2026-06-24, with refuted specifics called out). **Extend it; do not restart
it.** Rows below marked *(repo log)* are already verified there and are quoted,
not re-derived.

| Claim as used here | Source | Verified how | Confidence |
|---|---|---|---|
| Constant scallop height as a machining objective (vs constant stepover); zig-zag ball-end finishing; reduces CL data and machining time | Suresh, K. & Yang, D.C.H. (1994), *Constant Scallop-height Machining of Free-form Surfaces*, ASME J. Eng. for Industry **116**(2):253–259, DOI 10.1115/1.2901938 | Web search surfaced the ASME landing page and a SciSpace record with matching title/venue/volume/pages; **ASME page returned HTTP 403, abstract not fetched**. Independently corroborated by the repo log *(repo log, verified high)* | **high** (existence + framing); abstract not read tonight |
| Each pass is a non-constant offset of the previous one to hold scallop height → "no redundant motion" | Lin, R.S. & Koren, Y. (1996), *Efficient Tool-Path Planning for Machining Free-Form Surfaces*, ASME J. Eng. for Industry **118**(1):20–28 | *(repo log, verified 3-0)* | **high** |
| **Constant-cusp paths are ~7–21 % shorter than iso-parametric/raster at equal tolerance** — the source of §1.1's cutting-distance prediction | Feng, H.-Y. & Li, H. (2002), *Constant scallop-height tool path generation for three-axis sculptured surface machining*, CAD **34**(9):647–654, DOI 10.1016/S0010-4485(01)00136-1 | *(repo log, verified high, with pages re-confirmed in sweep 2)*. **Caveat carried from that log: path LENGTH, not accel-aware cycle time; iso-scallop is costlier to generate** | **high**, with the caveat |
| A machining potential field over the part surface yields tool paths following locally optimal cutting directions | Chiou, C.-J. & Lee, Y.-S. (2002), *A machining potential field approach to tool path generation for multi-axis sculptured surface machining*, CAD **34**(5):357–371 | Search returned title/venue/volume/pages consistently across ScienceDirect and Semantic Scholar; **both fetches failed (403 / empty render)**, abstract read only via search summary. **It is a MULTI-AXIS (5-axis) method** — the direction field is coupled to tool orientation, which a 3-axis router does not have | **medium** — cite for the *idea* of a direction field, never for a number |
| Machine performance is strongly anisotropic; the best feed direction is a machine question, not only a geometry question | Kim, T. & Sarma, S.E. (2002), *Toolpath generation along directions of maximum kinematic performance; a first cut at machine-optimal paths*, CAD **34**(6):453–468 | Search returned the ScienceDirect record with matching title/volume/pages and a summary of the greedy best-performance-direction approach; primary PDF not fetched | **medium-high** |
| Modern formulation: trade off a preferred-feed-direction field against constant scallop height, minimising total path length; solved as a Poisson problem | Zou, Q., Wang, C.C.L. & Feng, H.-Y., *Length-optimal tool path planning for freeform surfaces with preferred feed directions*, arXiv:2009.02660 | **Abstract fetched directly from arXiv and read verbatim** — title, authors and the tradeoff/length claim confirmed first-hand. Also in *(repo log)* | **high** |
| Evenly-spaced, non-overlapping streamline placement at a controllable separating distance | Jobard, B. & Lefer, W. (1997), *Creating Evenly-Spaced Streamlines of Arbitrary Density*, Eurographics Workshop on Visualization in Scientific Computing '97, Boulogne-sur-Mer | Search returned consistent title/venue/year plus multiple independent reimplementations (MATLAB, C/C++/Rust, VTK/Kitware); **ResearchGate fetch 403, primary PDF not read** | **medium-high** (existence and method are not in doubt; no verbatim abstract) |
| Exact cellular decomposition into cells each covered by simple back-and-forth motion; coverage reduces to an exhaustive walk of the cell-adjacency graph; provably complete | Choset, H. & Pignon, P. (1997), *Coverage Path Planning: The Boustrophedon Decomposition*, Proc. 1st Int. Conf. Field and Service Robotics (FSR '97) | **Abstract fetched verbatim from the CMU Robotics Institute publications page** | **high** |
| Morse decompositions generalise the above using critical points of a Morse function; complete coverage in unknown spaces | Acar, E.U. & Choset, H. et al., *Morse Decompositions for Coverage Tasks* / *Sensor-based Coverage of Unknown Environments: Incremental Construction of Morse Decompositions*, IJRR 2002 | Search returned matching titles, authors and IJRR 2002 venue across Semantic Scholar and SAGE; primary not fetched | **medium-high** |
| Lawn mowing and milling are NP-hard; constant-factor approximations exist; **milling** is the variant where the cutter must stay inside the region | Arkin, E.M., Fekete, S.P. & Mitchell, J.S.B. (2000), *Approximation algorithms for lawn mowing and milling*, Computational Geometry **17**:25–50, DOI 10.1016/S0925-7721(00)00015-8 | **Verified indirectly but firmly**: the open-access ESA 2023 paper *The Lawn Mowing Problem: From Algebra to Algorithms* (Fekete et al., LIPIcs vol. 274, art. 45) was downloaded and text-extracted — its reference **[4]** is exactly this paper with matching volume/pages/DOI, and its body attributes to [4] both the NP-hardness (*"implies the NP-hardness of the LMP (Theorem 1 in [4])"*) and the **currently best approximation guarantee of `2√3 · α_TSP ≈ 3.46 α_TSP`** | **high**, with one correction below |
| **Turn count, not just path length, dominates coverage cost** — the published form of this repo's "air cost is COUNT-bound" | same ESA 2023 paper, quoting: *"the number of turns in a tour is of crucial importance for the overall cost; this has been previously studied by Arkin et al. [2]"* | **Extracted verbatim from the downloaded PDF** | **high** |
| Spiral path for a simply-connected pocket by interpolating growing disks on the medial axis; **no tool retractions**, self-intersection-free, complies with a maximum cutting width | Held, M. & Spielberger, C. (2009), *A smooth spiral tool path for high speed machining of 2D pockets*, CAD; follow-up 2014 for multiply-connected pockets | Search returned consistent title/authors/venue/year across ScienceDirect and Semantic Scholar; primary not fetched | **medium-high** |
| Optimal feed direction varies over a surface, so **whole-surface single-strategy finishing only reaches local optima** — partition first | Liu et al. (2015), CAD **66**:1–13 (machining strip-width tensor); region-based strip width (2018), IJAMT, DOI 10.1007/s00170-018-2427-6 | *(repo log, sweep 2 — FOUND and quoted, single-source, verification abstained; medium confidence)* | **medium** |
| Linking many short fragments to minimise air time is a GTSP-with-precedence problem | Castelino, D'Souza & Wright (2003), *Toolpath optimization for minimizing airtime during machining*, J. Manufacturing Systems **22**(3):173–180 | *(repo log, sweep 2 — medium confidence)* | **medium** |

**Correction to the brief's own framing of Arkin/Fekete/Mitchell:** the widely
repeated "(3+ε) for lawn mowing, 2.5 for milling" figures appear in search
summaries, but the ESA 2023 paper — by one of the same authors — attributes to
[4] a best guarantee of `2√3 α_TSP ≈ 3.46 α_TSP`. Cite the NP-hardness and the
milling-vs-mowing distinction; **do not cite a specific approximation constant
without reading the 2000 paper itself.**

### What the literature does and does not settle for us

- It settles that **constant-cusp/contour coverage is shorter than raster at
  equal tolerance** (Feng & Li) and that **turn count dominates coverage cost**
  (Arkin et al. via Fekete et al. 2023). Both point at Lever 1.
- It settles that **direction should vary per region** (Liu et al.), which is
  Lever 2's premise, and that the *right* per-region decomposition is a
  boustrophedon/Morse cellular one (Choset; Acar & Choset), not a
  single-angle-per-island heuristic — which is exactly why Lever 2's ceiling is
  low on a web.
- It does **not** settle anything about accel-aware cycle time on a low-accel
  belt router. The repo log's own conclusion stands: *"all the verified
  tool-selection work optimizes path length or MRR, not accel-aware time"* —
  and rs_cam has the integrator the literature lacks. That is why §2's decisive
  bar is integrator-costed runtime.

---

## 6. What I could NOT verify

**Because this session ran no cargo and no GUI/MCP:**

1. **Nothing in §1's predictions is measured.** Fragment counts, junction
   counts, cutting-distance deltas and cost figures are arithmetic from code I
   read plus ledger numbers. Each is labelled *(derived)* where it is.
2. **The post-fix A0 baseline does not exist.** Every ledger number I quote
   (17,083 in-node retracts, 363.8 m rapid / 86.8 m cutting, 1,263 residual
   retracts) predates one or more of tonight's four changes (G-LINKVETO,
   hookup 25, equal-cusp `z_step`, pinned `bottom_z`). §2 starts with A0
   deliberately.
3. **The per-call surface-build cost (§1.4) is extrapolated**, from
   `CLASSIFICATION_PERF_STUDY.md` §4.1's 215k-triangle terrain at 425²/849² to
   wanaka's 661k triangles at tier-1 resolution. The extrapolation crosses both
   a triangle-count and a grid-size axis and could be off by 2–3×.
4. **Island width on the real wanaka tier-1 mask is unmeasured.** The "1.5–3 mm
   fingers" figure behind the 4–6-rings estimate is inferred from the
   ~24-crossings-per-row arithmetic, not from the mask. `preview_tier_map`'s
   SVG would settle it in one call and should be the first thing run.
5. **Whether the shipped scallop cascade actually produces one connected
   spiral on a dendritic island.** `continuous: true` chains contours, but
   Phase O's own sentry already found the cascade reaching relink as two
   fragments whose junction exceeded the hookup — so the chaining is not
   unconditional. Untested on a branching region.
6. **Whether `RegionSet::contains` cost changes materially** when the shallow
   raster's per-point region test is replaced by ring generation. The raster
   currently does one `contains` per grid point; the cascade does per ring
   vertex. Direction of the change unknown.
7. **`min_width_mm` on a region with holes.** The EDT inradius is defined for
   the mask, but `region_polygons_from_mask` groups loops by even-odd depth and
   a region may carry holes; the metric's behaviour there is designed, not
   checked.

**Because primary sources were paywalled or would not render:**

8. Suresh & Yang (1994) — ASME page 403; abstract read only via search summary.
9. Chiou & Lee (2002) — ScienceDirect 403, Semantic Scholar returned an empty
   render; abstract read only via search summary. **Its multi-axis scope is
   asserted from the title and summary, not from the paper.**
10. Kim & Sarma (2002), Held & Spielberger (2009), Jobard & Lefer (1997),
    Acar & Choset (2002) — bibliographic records consistent across ≥2
    independent sites; **no primary abstract read**.
11. Arkin, Fekete & Mitchell (2000) — verified only *through* the open-access
    ESA 2023 paper's reference list and body citations. Its own abstract was
    not read, which is why §5 refuses to state an approximation constant.

**Design questions I deliberately did not answer, because they are the
operator's:**

12. Whether the ring cusp pattern is acceptable on a terrain relief (§1.1's
    first risk) — the C4 rule makes this an eyeball, not an analysis.
13. `THIN_K`'s value. §1.1 proposes 8 as a sweep anchor, not an answer.
14. Whether to batch contour-routed regions into one scallop call (§1.1
    mitigation (i)) and give up per-region routing freedom, or to build the
    finish-surface cache (mitigation (ii)). That is a cost measurement plus a
    routing-quality judgement, and neither exists yet.
