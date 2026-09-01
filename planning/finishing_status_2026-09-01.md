# Finishing programme — consolidated status and avenues (2026-09-01)

> Written at the operator's request after the F1/F2/C2 campaigns. This
> document does three jobs: (1) state the status of every arm in one
> place; (2) audit HOW each arm was tested, because fixture defects
> decided several verdicts; (3) rank the avenues that can still produce a
> win. Every number cites the run or document that produced it. Where a
> verdict depends on a fixture that could not express the candidate's
> advantage, this document says so — the verdict is then *bounded*, not
> final.

## 1. Status board

| Arm | Status | Key number | Record |
|---|---|---|---|
| **C2 monotone cells** (thin-organic) | **BUILT, default-off. C4 operator review PENDING** — the only gate left | 1.090× finish tiers, −14 % rapid distance, production-validated twice (CLI 08-30, GUI 09-01) | `thin_organic_2026-08-27/FINDINGS.md` §7 |
| **Per-region sweep angle** | Measured, folded into C2's PCA rotation. No separate build | 1.10× total; predictor is kept retracts, not fragments | thin-organic §0e/§0f |
| **Per-cell direction (D1)** | **REFUTED** | 0.917× — a cost | thin-organic §0j |
| **Contour-per-cell (D2)** | **REFUTED** | 0.686× | thin-organic §0k |
| **F1 direction field** (Zou) | **CLOSED on Wanaka**, mechanism understood. Untested on the paper's own surface class | coherence length 0.35 mm < stepover 0.486 mm — the field turns inside one pass | `conformal_finish_2026-08-28/FINDINGS.md` §F1-1..3 |
| **F1 premise** (anisotropy prize) | **CONFIRMED real** on Wanaka | ceiling +9.75 % (region 1) to +21.5 % (whole surface, R1.5) — unreachable by this method there | synthesis §11 |
| **§4.3 variant** (D = sweep axis) | Open, untested where it could win | beat D = t1 by 1.55–4.34× on umbilic fixtures; still lost to raster on all of them | synthesis §8 |
| **Medial-axis field** (operator's proposal) | Falsified on branched geometry; **best-in-programme on compact geometry** | ribbon: 179 vs 98 fragments (falsifier fired). Sphere: **1.036× floor, 0 retracts, meets spec** | synthesis §9 |
| **F2 conformal spiral** | Mechanism **works**; **NOT RECOMMENDED** as built | removes all retracts, 1.87× slower on branched geometry (2.9× distance; ring anisotropy median 36.7×, worst 113×) | `FINDINGS_F2.md` §F2-4 |
| **F2 slit map** (holes) | Not built. Topology case **proven necessary**, economics **against** | no slope band is a topological disk at useful size; but it unlocks a ring scheme measured 1.87× slower | §F2-4 A + D |
| **Retract chasing generally** | **CLOSED** as a direction | `surface_link` already converts ~80 % of fragment joins (52/63, 54/64); residual ≈ 0.38 s/link | §F2-4 C, synthesis §3 |
| **The floor (L_min)** | Instrument built; reframed the whole programme | raster sits at 1.10–1.31× floor on real shapes — and at **0.997× on the sphere, which is under-coverage** | synthesis §1, §8 |

## 2. What each arm was tested ON, and what that bounds

The programme committed the same meta-error four times: it ran a
candidate on a fixture that could not express the candidate's advantage.
The adopted rule: **state the mechanism by which the candidate is
supposed to win, then check the fixture can express it, before running
anything** (synthesis §8). §11 extended the rule to closures: do not
close an arm on an argument from scale without measuring the scale.

| # | Error | Verdict it contaminated | Corrected by |
|---|---|---|---|
| 1 | Facets 2.9× the stepover (`terrain_small.stl`) | §F2-1 spiral spacing/time — **withdrawn** | analytic fixtures, facets ≤ stepover/3 |
| 2 | Retract-eliminator benchmarked on geometry with **zero retracts** (sphere/wavy) | §F2-2's first recommendation — **suspended** | ARM RIBBON (genuinely fragmenting, validity-gated) |
| 3 | Locally-adaptive spacing tested on near-constant-curvature fixtures | §8's "field loses to raster" — bounded, **not refuted**: prize `mean(s_max)/min(s_max)` ≈ 1.00 there by construction | not yet corrected — needs the missing fixture (§5, avenue D) |
| 4 | Direction method tested on an **umbilic** sphere where its own advantage is identically zero | §10's first closure — withdrawn after §11 measured Wanaka's anisotropy as real | `wanaka_curvature_anisotropy.rs` |

### The fixture inventory as it stands

| Fixture | Expresses | Cannot express |
|---|---|---|
| SPHERE cap (analytic, umbilic) | the floor *exactly* (constant curvature); under-coverage detection | any direction or adaptive-spacing advantage (both ≡ 0 by construction) |
| WAVY patch (analytic) | mild variation | strong curvature contrast (gentle by design) |
| ARM RIBBON (8-arm, simply connected, analytic) | fragmentation, branching cost, ring anisotropy | topology (holes) — deliberately excluded |
| ARM BAND (4 bumps, slope-banded, analytic) | real band topology — holes by construction | — |
| Wanaka region 1 (captured, real) | the actual failure geometry: shallow, branched, multiply connected | needs hole machinery for any spiral arm |
| wanaka200_mt2 (production project) | end-to-end A/B with real chain, feeds, kinematics | per-mechanism attribution (too integrated) |

**The gap:** no fixture has (a) genuinely varying curvature
(`mean(s_max)/min(s_max)` well above 1), (b) direction zones coherent
over several stepovers (`w30 ≥ 0.70`), and (c) simple connectivity. That
is the **bike-seat class** — the geometry Zou's paper validates on, and
the geometry Kumazawa names as the method's home ("mounts and valleys
where the difference between maximum and minimum W is notable"). Errors
3 and 4 both trace to this gap. One fixture closes both, and also hosts
the honest-raster comparison (§5, avenue B).

## 3. Value salvaged, by arm — including from arms that lost

The operator asked for this explicitly: a losing arm can still leave
working assets. These exist, are tested, and are reusable regardless of
any verdict above.

**From F1 (direction field):**
- `direction_field.rs` — working per-triangle Poisson solve + marching
  iso-curve extraction, unit-tested. F2's evaluation reused parts.
- `wanaka_curvature_anisotropy.rs` — Monge-quadric curvature census with
  two anti-faceting scale rules. **A cheap pre-check for "is direction
  worth choosing on this surface at all"** (prize ceiling in %).
- `zone_coherence_census.rs` — measures direction coherence length and
  `w30` per zone in one run, **before any implementation**. This is the
  reopening gate for the whole direction family.
- The literature record: `preferred_direction_field_research.md`
  established that D = t1 IS the literature's own 3-axis ball-end answer
  (Kumazawa), that the published prize vs an honest iso-scallop is only
  1.9–7.2 %, and that Kim Eq. 32 (2001) is our §1 floor — citable now.

**From F2 (conformal spiral):**
- **Bridging** (log-rectangle, Eqs. 7–9): joins nested loops into one
  continuous path. Zero retracts, zero self-intersections, verified full
  coverage, 5–13 % length overhead across fixtures. **Orthogonal to the
  field choice** (synthesis §2a) — any nested level sets can be bridged.
  This is the single most product-shaped asset the programme built.
- `CoverageAudit` + achieved-surface-spacing instrument (with its own
  sampling floor and blindness audit, `72e844da`) — "did it finish the
  part, at what spacing" for ANY candidate.
- Ring-anisotropy and quasi-conformal-dilatation metrics — the pre-check
  for "will one-radius rings over-cover here".
- `plan_spiral`'s refusal-first topology handling
  (`NotSimplyConnected { boundary_loops }`) — fails typed, never
  silently splits.
- The topology census result itself: slope bands on bumpy surfaces are
  multiply connected **by construction**. Any future region-level
  strategy must handle holes or refuse.

**From the floor work (synthesis §1/§8):**
- The `L_min = ∫dA/s_max` integrand and ×floor scoring columns — every
  future candidate gets an absolute score, not just a pairwise one.
- The under-coverage finding: the harness raster spaces in **XY
  projection** while the spec lives on the surface — 3.3 % wide on the
  sphere. **Open question, unverified: does the SHIPPED shallow-band
  raster share this defect?** If yes, it is a quality defect in
  production and every "raster wins" margin shrinks (avenue B).

**From the retract/cells work:**
- C2 itself (shipped, gated, telemetried — `monotone_cells` on both
  wires with `membership_fallbacks`/`empty_fallbacks`).
- The finding that kept retracts, not fragment count, predict time
  (§0e) — reuse in any future router/order work.
- `relink_and_cost_under` with the realistic machined-stock link ceiling
  (§0i) — the only operator-honest costing regime; every future arm must
  run under it.

## 4. Where the remaining time actually is

Synthesis §3, condensed — the three costs and their current state:

1. **Coverage (cutting distance).** The dominant open cost. Every
   candidate that assigns one spacing to a whole pass pays for that
   pass's worst point — the spiral per ring, both field arms per level.
   The raster pays a uniform global stepover. Nothing varies spacing
   *along* a pass.
2. **Kinematics (turns, short passes).** C2's territory. Largely
   collected: 1.090× production-validated.
3. **Connection (links, retracts).** Largely gone. `surface_link`
   converts ~80 % of fragment joins; the residual costs ≈ 0.38 s each.
   Not worth further pursuit on this machine.

## 5. Avenues, ranked

Each avenue states its mechanism and its fixture check, per the
programme's own rule.

### A. Unlock the wins already built (operator time, zero code)

1. **C4 review of C2.** Evidence pack ready:
   `~/Downloads/c4_review/armA_dial_off_surface.html` vs
   `armB_dial_ON_surface.html` (5.36 M triangles each, zoomable).
   Narrowed question: only the **rotated** regions changed pass
   direction (28 of 64 on tier 0, 6 of 16 on tier 1); non-rotated
   regions emit the identical lattice. Pass → flip the default →
   −25 min and −14 % rapids on every future wanaka-class job.
2. **Phase U review of the multitool tier overlay** (slope-compensated
   assignment, 22.0 % vs raw 71.6 %, decided in Phase T). Same shape:
   one eyeball unlocks a decided plan.

### B. Verify the shipped raster meets its own scallop spec (small, high value)

**Mechanism:** the harness raster under-covers on slope (surface spacing
= XY spacing / cos θ; measured 3.3 % wide on the sphere at 0.997× floor).
If the shipped shallow band spaces the same way, the product does not
deliver the scallop it is configured for on sloped shallow ground — up to
41 % wide at the 45° band edge in the worst case.
**Fixture check:** the sphere fixture settles it exactly; the instrument
(achieved-surface-spacing) exists.
**Work:** one instrument run against the SHIPPED `unified_finish`
shallow arm, not the harness. If confirmed: a slope-aware stepover
derate is a quality fix (costs time, buys spec), and every strategy
comparison must be re-scored at equal *achieved* scallop — the synthesis
calls this "the single most valuable missing measurement in the whole
programme" (§1).

### C. Shape-selected spiral for compact regions ("the spiral look, without the conformal map")

**Mechanism:** synthesis §9's settled pattern — region shape decides the
strategy. Compact/round regions want offsets: the medial-axis/EDT field
scored **1.036× floor, 13 fragments, 0 retracts, meets spec** on the
sphere, the best honest number the programme produced, matching raster
time while the raster under-covered. Bridge those nested offset rings
into one continuous spiral with F2's bridging (§3 salvage), which is
field-agnostic. **The conformal slit map is NOT needed for this** — that
is the part that measured 1.87× slower and stays shelved.
**The operator's appearance point is a real, separate prize:** a
continuous spiral leaves no raster lines and no cell seams on round
work. On showpiece wood (bowls, dishes, rosettes) an operator may accept
a small time cost for that surface. Time is not the only currency; C4
exists precisely because pattern changes are judged by eye.
**Fixture check:** sphere/dome fixtures exist and are the geometry class
in question. The gate for real use is a compactness/elongation test
(the elongation machinery exists in C2) plus the ring-anisotropy
pre-check (exists) plus a topology check (`plan_spiral` refuses holes
typed). Wanaka has few compact shallow regions — the first real target
is round-pocket / dish-like work, so add one such fixture or project.
**Work:** port bridging out of the F2 research module; wire
EDT-offset rings (exist in `scallop_isofield` lineage) to it; gouge-check
via drop-cutter; C4-style eyeball on a dish fixture.

### D. The bike-seat replication (direction field on its home turf)

**Mechanism:** the method was closed on Wanaka because the field turns
inside one stepover there. The paper's own validation class — blade,
bike seat, saddle: smooth, swept, simply-connected sheets — has long
coherent direction zones AND varying curvature. Zou's Table 1 reports
**−13.0 % path length on the bike seat** and −10.1 % on the blade vs
classic iso-scallop. Replicating that validates our implementation
(solver, spacing, extraction all exist) and defines the product niche:
carved chair seats, guitar bodies/necks, boat parts — swept shapes wood
routers actually cut.
**Fixture check FIRST, and it is cheap:** run the two existing censuses
on the candidate surface before any pathing — anisotropy census (prize
ceiling in %) and coherence census (`w30 ≥ 0.70` over several
stepovers). The paper's models are GrabCAD; an analytic swept-saddle
sheet with controlled curvature contrast is the durable alternative
(facets ≤ stepover/3, per the standing rule).
**This same fixture decides §4.3** (D = sweep vs D = t1 where curvature
varies — synthesis §10 left both open) **and hosts the honest-raster
arm** (avenue B's comparison at equal achieved scallop). Three open
questions, one fixture.
**Bar:** beat the honest raster at equal achieved scallop by a margin in
the direction of Table 1's −10/−13 %, under the realistic link ceiling,
scored ×floor. If it cannot beat the honest raster on its own home
geometry, the arm closes permanently with clean evidence.

### E. Complete the §6 floor experiment

Add the missing **honest raster** arm (XY stepover derated by cos θ_max,
or locally by cos θ) to the ×floor table on all fixtures. Cheap: the
scoring, fixtures and integrator exist. This re-baselines every
comparison at equal delivered finish and quantifies how much of the
raster's lead is unbilled finish debt.

### F. Research frontier: spacing that varies along a pass

The one defect every candidate shares (synthesis §8): one spacing per
pass pays for the pass's worst point. No method here varies spacing
along a pass. **Do not design anything yet** — first measure the prize:
`mean(s_max)/min(s_max)` per region on Wanaka (the anisotropy instrument
nearly computes this already). If the per-region ratio is a few percent,
this frontier is not worth its complexity on terrain and should be
recorded as such; if large on bike-seat-class work, it stacks with
avenue D.

## 6. Not recommended — do not reopen without the stated evidence

| What | Why | Reopens only if |
|---|---|---|
| Conformal slit map | unlocks a ring scheme measured 1.87× slower on the geometry it unlocks | a per-sector / distortion-aware ring spacing exists first (the single-radius ring is the defect, not the map) |
| Retract elimination as a goal | ~80 % already converted to stay-down links; residual ≈ 0.38 s each | machine with much slower Z or higher safe-Z cost |
| Per-cell direction / per-cell contour | D1 0.917×, D2 0.686× — measured costs | never on this evidence |
| Direction field on terrain | coherence length 0.35 mm < one stepover; no decomposition repairs it (§F1-3) | a surface passes the coherence census (`w30 ≥ 0.70`, coherence ≥ several stepovers) |
| Any efficiency number from a mesh with facets ≈ stepover | cost the programme two full withdrawal rounds | never — the facet rule is standing |

## 7. Document map after this consolidation

- This file — status board, fixture audit, avenues.
- `finishing_synthesis_2026-08-30.md` — the theory: floor, three costs,
  §8–§11 measurements, **§12 closure note added 2026-09-01**.
- `conformal_finish_2026-08-28/` — programme charter (status header
  updated 2026-09-01), F1/F2 findings, five paper extractions, reading
  gate, primitives inventory.
- `thin_organic_2026-08-27/FINDINGS.md` — §0a–§0k lever measurements,
  §7 C2 build + acceptance + **C4 evidence pack (2026-09-01)**.
- `planning/PROGRESS.md` — recent-work section for 2026-08-19 →
  2026-09-01 added.

## 8. Track index (added 2026-09-01, second session)

Execution moved to one track per avenue, evidence separated by
directory. A track's TRACK.md carries its question, pre-registered bar
and status; its FINDINGS.md carries the evidence.

| Track | Avenue | Dir | Status |
|---|---|---|---|
| A | C4 + Phase U operator reviews | evidence: `~/Downloads/c4_review/`, `thin_organic_2026-08-27/FINDINGS.md` §7 | **C4 PASSED** (2026-09-01, pattern evidence — see §7 ruling); **Phase U island placement RATIFIED** (operator: "the filled island looks good"); **spiral appearance APPROVED** (operator, dish SVG pair) — Track C moves to a productisation ticket |
| B | honest raster on shipped code (+ ×floor honest arm) | `planning/honest_raster_2026-09-01/` | **COMPLETE — DEFECT CONFIRMED** (`fe186d01`): shipped spacing = s_XY/cos θ on slopes, ×floor 0.808 at 40°; sphere refunded by convex focusing up to 17.75°; honest arm hits 1.0000× spec at 1.009× floor, costing 1.09–1.25× distance. **FIXED, ALWAYS ON** (operator ruling + implementation 2026-09-01 — see §9) |
| C | shape-selected spiral (offsets + bridging) | `planning/spiral_finish_2026-09-01/` | **COMPLETE — BAR MET** (`c7beb6b3`/`aec52a65`): sphere 1.113× floor, dish 1.223×, 0 retracts, 0.0000 % unmachined, spacing within 0.8 % of spec, bridge overhead ~5.6–5.8 %. Research module `spiral_finish_compact.rs`, refusal-first on non-compact shapes. Next gate: operator appearance review of the SVGs, then productisation decision |
| D | bike-seat gate (fixture + two censuses only) | `planning/bikeseat_gate_2026-09-01/` | **CLOSED — GATES FAIL** (`c4710501`): prize ceiling +3.85 % vs the 5 % bar (gate 1 fail) even though coherence passes at 3.45 stepovers, w30 0.887 (gate 2 pass); negative control separated. Structural finding: the coherence gate 2 demands is exactly what lets a fixed per-region angle capture ~half the prize, so the field's margin over a rotated raster cannot clear the bar in this class at R = 1.0 mm. The direction-field family is now closed at the gate on BOTH geometry classes — terrain fails coherence, swept sheets fail the prize. Salvage: the swept-sheet coherence result argues FOR C2's shipped per-region rotation |
| E | folded into Track B step 2 | — | — |
| F | spacing-along-pass prize measurement | not opened | after B/C/D land |

## 9. Track B consequence — a product decision, not a polish item (2026-09-01)

Track B confirmed it: on sloped shallow ground the shipped raster's
achieved surface spacing is `s_XY / cos θ` — the configured scallop spec
is not met, by up to 1.31× at 40°, and the ×floor score of 0.808 means
the pass is under-covering, not efficient. Convex curvature refunds the
error on gentle domes (contact focusing cancels sec θ up to ~18° on the
sphere fixture), so terrain crowns are partly protected; planar and
concave slopes are not.

The honest arm (stepover × cos θ_max per region) lands spacing at
exactly spec for 1.09–1.25× cutting distance on the test slopes. The
decision — derate globally per region, derate locally, or expose a dial
— changes every sloped shallow job's runtime and finish, so it binds on
the operator. Every prior "raster wins by <25 %" margin on sloped ground
is undecided until candidates are re-scored at equal achieved scallop.

**DECISION (operator, 2026-09-01): fix it, ALWAYS ON — no dial.** Derate
per region by cos θ_max of that region's measured slope. The convex-
refund conservatism is accepted for v1; no curvature-aware refund.

**IMPLEMENTED (2026-09-01).** The Shallow arm derates before any lattice
is built (`unified_finish::shallow_region_max_slope_deg`, floor 1°,
clamp at the planner's steep threshold), reports each derate through
`ToolpathStats::derived_stepovers`, and the reworked
`shipped_raster_spacing_b1` instrument is the standing acceptance gate:
CLEAN on all three fixtures, planes at exactly 1.0000× s_max, flat
ground byte-identical. Record: `honest_raster_2026-09-01/FINDINGS.md`
§"Fix acceptance".

## 10. Track D consequence — the direction-field question is answered, cheaply (2026-09-01)

The two-gate design ends the direction-field family without building the
pipeline a fourth time:

- Wanaka-class terrain: prize real (+9.75–21.5 %), coherence absent
  (0.35 mm vs a 0.486 mm stepover). Gate 2 fails.
- Bike-seat-class swept sheets: coherence present (3.45 stepovers, w30
  0.887), prize under the bar (+3.85 % vs 5 %). Gate 1 fails.
- The gates are structurally in tension in this class: coherence is what
  lets ONE well-chosen angle per region capture about half the pointwise
  excess, and the residual scales with κ·R — raising curvature to feed
  the prize approaches the gouge bound and calls for a smaller ball,
  which resets the prize. The literature's own honest margins (1.9–7.2 %
  vs a good iso-scallop) sit consistent with this.
- What the operator's product should take from it: the per-region
  PCA rotation C2 already ships is the collectable share of the
  direction prize. Avenue D is closed; avenue F (spacing varying ALONG a
  pass) remains the only open path to the remaining coverage headroom,
  and it must measure its prize before designing anything.
