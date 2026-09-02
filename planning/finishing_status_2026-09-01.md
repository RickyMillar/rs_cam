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
| **C2 monotone cells** (thin-organic) | **SHIPPED DEFAULT-ON** (C4 passed 2026-09-01; flip `83449244`, wanaka project saved dial-on) | 1.090× finish tiers, −14 % rapid distance, production-validated twice (CLI 08-30, GUI 09-01) | `thin_organic_2026-08-27/FINDINGS.md` §7 |
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

**PRIZE MEASURED 2026-09-02 (Track H W0b, slope term):** guarded
mean/p1 of `s_max·cos θ` = **2.780** over the full front finish
territory, **1.7–2.4 inside single catchments** — the prize is
real, ~2× against any per-region worst-point derate. CATCHMENT
decomposition recovers only ~25–30 % of it — but the mechanism
attribution is OPEN: a slope-homogeneous decomposition (hillslope
facets, or existing bands with owned-cells θ_max) captures a
slope-driven prize by construction, and variable-spacing passes
capture it within a pass. Next cheap measurement before any
design: mean/min-owned of the field per EXISTING band region,
splitting the prize between "better regions" and "variable
spacing". The variable-spacing candidate is the Eikonal
spacing-weighted ring family (`scallop_isofield.rs` is the
substrate; bridging is field-agnostic); it requires its own
pre-registration — §9's constant-spacing shape evidence neither
passes nor fails it. The
curvature-driven `s_max` term (Kumazawa W) is additional and
unmeasured. Record: `valley_tracing_2026-09-02/FINDINGS.md` §W0b.

**SPLIT MEASURED AND DECOMPOSITION REFUTED 2026-09-02 (Track M
M3 + M4, `planning/metrology_2026-09-02/FINDINGS.md`):** the prize
on owned Shallow cells is 1.354× floor (26.1 % of cutting length).
M3: a K = 3 slope banding captures 71.9 % of it on SPACING alone,
confetti merged (the operator's tiny-islands suspicion is real on
the 20–40° bands but the prize mass is flat and contiguous). M4
then COSTED it: the banded arm pays 8,411 fragments vs 1,835 and
reads **1.836× the control's time** (K = 2 robustness arm: 1.772×)
— the link bill eats the 21 % distance refund 2.5× over. Mechanism:
the merge bounds island AREA; fragments scale with boundary
PERIMETER, which is dendritic at every slope threshold. **The
decomposition route is CLOSED. The only standing route to this
prize is spacing varying WITHIN a continuous pass (zero added
fragments by construction)** — the Eikonal candidate above, whose
pre-registration remains the operator's to assign, with its bar now
sharp: capture a useful slice of the 26 % with NO fragment bill.

**RUN AND CLOSED 2026-09-02 (Track M E1/E1b, operator-assigned,
`planning/metrology_2026-09-02/FINDINGS.md` §M5):** the graded
raster (column-integrated conservative pitch field; every pass a
continuous single-valued curve) swept its one design knob — the
horizontal smoothing W. E-frag and E-spec PASS at W ≥ 4 mm (the
zero-added-fragments mechanism works); **E-time FAILS at every W —
best 1.013× vs the < 0.95× bar.** Mechanism, measured: flat ground
sits p50 **0.90 mm** from steep ground (p90 2.00 mm) — the prize is
interleaved with the gullies at sub-stepover scale, so any
machinable smoothing clamps the field before wander stops.
**AVENUE F IS CLOSED ON THE TIER-1 FRINGE** (SCOPE CORRECTED
same day, operator-caught — see below).** The full ledger:
prize real (26.1 %); excision refunds nothing (G2); decomposition
refunds nothing (M4); continuous variation refunds nothing (E1).
The shipped worst-point raster is the measured honest optimum of
its family here — the prize is a property of the terrain. Reopening
conditions recorded in §M5: a spec change (bounded cusp exceedance,
operator's call), or smoother work whose flat-to-steep p50 is well
above the stepover (the E1 instrument then applies as-is).

**SCOPE CORRECTION (operator-caught, 2026-09-02, §M6):** the whole
G2/F1/F2/E1 chain ran on the TIER-1 FINE ISLAND — the dendritic
fringe inherited from the valley harness — not on tier-0 mountain
ground. The closures stand for that fringe only; tier 0 is
UNMEASURED and the E2 re-run (censuses first, then the same bars) is
in flight. The river-offset field — spacing-graded rings grown from
the drainage network, the programme's founding image — has never
been costed on mountain ground and waits on the E2 censuses.

**E2 RAN same day (§M6): tier-0 closes too, by different mechanism.**
The mountains ARE different (flat-to-steep p50 4.88 mm; refund ~16 %;
half the tier already cuts contour-like scallop) — but the graded
family's best is 1.021×, never crossing, and the contour-alignment
census kills the river-offset arm without a build: |da/ds|
across/along = **1.56×**, far below the ~3–5× grain coherence
contour-aligned fronts need. Wanaka is knobbly at stepover scale in
every direction. AVENUE F IS NOW CLOSED ON BOTH TIERS of this board,
each with its measured mechanism. Standard reopening censuses for
any future part: flat-to-steep p50 ≫ stepover AND grain coherence
≳ 3× (~40 s each on the E2 harness).

### G. Saturated derate in the Shallow band — prize real but UNSIZED; mechanism corrected 2026-09-02 (from Track H)

**15 of 16 Shallow regions derate 0.486 → 0.344 mm (≈ 1.41× the
cutting distance on nearly the whole band) because their θ_max
saturates at 45.000°, the band's own clamp**
(`valley_prize_census_h0.rs` M4 block).

**Mechanism BROADENED same day by Track H's V1 run:** the first
reading named the 2 mm `overlap_mm` band-boundary fringe. On
region 1 (the largest, probed directly) BOTH fringe-excision
identifications leave θ_max = 45.000°, because **42.90 % of the
region polygon's 3D area is steeper than 45°**
(`valley_branch_falsifier_h1.rs` slope census) — and those are
cells the Shallow band does not own and cannot finish to spec by
construction (~47 % of the polygon audits uncovered at spec — an
ownership artefact, not an op defect). Correct statement:
**non-owned steep inclusions inside Shallow polygons set the
band's derate; the fringe is one source of such cells.** The spec
question is unchanged and sharper: should cells the band cannot
meet spec on anyway set the band's stepover?

**MEASURED AND REFUTED 2026-09-02 (Track M G2,
`planning/metrology_2026-09-02/FINDINGS.md` §M2):** the two
hypotheses are decided — **(a)**, 95.65 % of the steep 3D area sits
on cells the planner CORRECTLY labels MidSteep/VerySteep, inside
the Shallow polygon only via the 2 mm overlap dilation; the
planner-defect arm is 0.57 % (sub-cell walls). No planner fix. And
the refund is now MEASURED: **1.000× on all three time-carrying
regions** — min-area absorption folds steep islands INTO the owned
set (owned-cell p99 at 73° on two regions), and under the
worst-point rule one steep cell pins the region. Excision refunds
nothing; the residual (p99 basis 1.13–1.19×) is a spacing-POLICY
question and belongs to avenue F. **Avenue G is CLOSED as a derate
fix.** What survives: (i) the absorption ticket — absorbed steep
islands are spec-unfinishable Shallow ground (137 mm² board-wide)
AND derate pins, a small real defect; (ii) the audit consequence,
shipped — `metrology::ownership` audits band-CELL ownership
(`PlannedRegions::labels`), and only 45.83 % of in-polygon cells
are owned by Shallow, so polygon-scoped audits misread every
dendritic Shallow op.

### S. Scallop-only strategy choice — OPEN, measured WIN 2026-09-02 (Track M M7)

The operator asked whether the unified band mix earns its overlap.
Measured (`scallop_solo_vs_unified_s1.rs`): the whole-board shipped
scallop beats the production unified op **0.806×** at equal coverage —
one continuous spiral, ONE entry plunge (vs 225), 11 m of rapids (vs
5.4 km), 15 % less cutting distance (the overlaps and seams). Keeping
the planner's regions but scalloping them all LOSES (1.082×) — the
region structure is the overhead. No contradiction with the F/E chain:
the raster stays the best raster on its band; the scallop never pays
the joints. NOT yet enacted — productization gate: run the real
project (both tiers) with scallop ops replacing unified, simulate,
pass the union/ownership audit and the standard gates. The cheapest
enactment is a strategy choice (the scallop op ships today), not new
code. Record: `planning/metrology_2026-09-02/FINDINGS.md` §M7.

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
| G | strategy ledger (whole-board arms vs tiers) | `planning/ledger_2026-09-01/` | **MEASURED** — see §13; arm B row contaminated, correction noted |
| H | valley tracing (drainage-tree finishing) | `planning/valley_tracing_2026-09-02/` | **CLOSED (2026-09-02, `99eb30aa`) — tracing arm failed all three V1 bars on the pre-registered FAVOURABLE region**: fragments 14.2× (2819 vs 199), distance 3.28×, time 3.53× vs the shipped raster. Mechanism: adjacent branch offset-fans overlap on dendritic ground. RE-BOUNDED same day (operator-caught): the arm traced the DENSEST tree rung on a band-region corridor — a configuration that guaranteed overlap by arithmetic (meta-error §2); refutes dense-tree-on-band-region only. The operator then reframed catchments as full-slope ZONES (phase W); W0b (corrected hydrology, operator-caught lake+flat defects fixed, labelling verified at 0.87 % divide crossings) rules it: compact-basin area 31.5/12.6/3.6 % vs the 50 % bar - FAILS at every rung, no artifact left. PHASE W CLOSED; TRACK H CLOSED on both arms. Avenue-F prize measured and routed to section 5 F (2.78 territory / 1.7-2.4 in-basin - decomposition does not capture it). Tracing arm closed (V1, bounded); the full-territory slope mix (69.3 % > 45 deg 3D) is on record for any future full-slope proposal. Survives: D_pot 5.9–9.3 pp in-mask direction prize, measured and unharvested (every known harvest mechanism now individually refuted; avenue F is the surviving lever); avenue G mechanism corrected (steep inclusions in Shallow polygons, not fringe — S′ ≡ S on region 1, 42.9 % of its 3D area > 45°); region-polygon ≠ band-territory finding routed to Track M. See §14 |

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

## 11. The coarse-skips-fine-islands question — already shipped, A/B cancelled (2026-09-01)

The operator observed overlapping coverage in the C4 pattern views and
asked whether tier 0 could skip islands the fine tier re-finishes. The
answer, verified in code and pixels: **it already does.** A
`PlannedTierRegions` boundary with `tier: 0` is the complement arm
(`session/multitool.rs:978-1010`, operator-requested 2026-08-27), and
`wanaka200_mt2`'s tier-0 op carries exactly that boundary. The tier-0
pattern render shows the island-shaped voids directly.

What remains visible as overlap is the 2 mm seam band, by design: the
complement is of the fine tiers' OWNED cells, not their machined extent,
so both tools cut the band and the seam blends. `overlap_mm` is the dial;
shrinking it trades against seam marks and stays at its default absent a
quality complaint.

An earlier note in this session said the skip was "off in the project" —
that was wrong (inferred from the planner dial's default instead of the
project's boundary) and is corrected here. No A/B is worth running: the
skip-off arm has nothing to teach.

## 12. G-UNIONCOV — multi-op finishing has NO whole-board coverage check (2026-09-01, operator-caught)

An overlap/floor variant (`wanaka200_mt2_overlap02.toml`: tier + region
overlap 2.0 → 0.2 mm, region floors 100/50 mm²) read **clean on every
wire** — `untouched_material_mm2 0.0`, `reached_uncut_estimate 0.0`,
zero collisions, air-cut verdict OK — while leaving standing material
across whole patches of the fine-detail territory. The operator caught
it by eye in the simulation view. Confirmed two ways: the side-by-side
stock render, and arithmetic — tier 1 emitted 16,112 mm of cutting,
which at its stepover covers ~7,800 mm² against 9,652 mm² of owned
islands.

**The mechanism of the silent pass:** every coverage finding is measured
PER OP against that op's own boundary. A multi-op tier chain can have
every op clean inside its territory while strips or patches between the
territories belong to no op. There is no union-coverage-vs-target
instrument on any wire. This is the same defect class as the empty-gate
vacuous pass (2026-08-05) and the fifth instance of a
metric-blind-spot deciding a verdict.

**Status:** variant REJECTED and deleted from the GUI (the saved
project was never modified). The apparent 1.67× saving is void. Two
suspects, deliberately not yet separated: (a) the region floors
absorbing small band islands whose "surrounding band" lies outside the
op boundary; (b) 0.2 mm overlap + `center` containment leaving
unowned seam ribbons. **Attribution is gated on building the union
instrument first** — one whole-board comparison of final simulated
stock vs target surface + stock_to_leave, reported per project, so a
boundary change can never silently pass again. Until it exists, any
boundary-config change is validated only by operator eyeball.

**What stays true:** the fringe cost is real — baseline tier 1 spends
~6× its owned area cutting the 2 mm blend band around dendritic
coastlines. The prize for a SAFE overlap reduction is large; it must be
re-approached one dial at a time, under the union instrument.

## 13. Track G — the strategy ledger (2026-09-01). The tier split is the losing layer on this board.

Full table and caveats: `planning/ledger_2026-09-01/FINDINGS.md` (merge
`6677370d`). The same-scale CLI comparison, one binary, upstream ops
identical:

| arm | finish time | spec |
|---|---:|---|
| A. whole-board scallop, R1.0 | **10,057 s (2.79 h)** | by construction; 1,425 mm² (~3.6 %) untouched debit |
| B. whole-board unified, R1.0 | 12,647 s (3.51 h) | CLEAN, measured |
| C. production two-tool tiers | **18,022 s (5.01 h)** | per-op clean; union unprovable (G-UNIONCOV) |
| D2. whole-board spiral, XY | 7,950 s — fastest measured | **FAILS** (50.2 % exceed) |
| D1. whole-board spiral, honest | 11,228 s, 0 retracts | **FAILS** (11.3 % exceed on >45° ground) |

Consequences, stated plainly:

- **On this board the tool split costs +42 % (B → C)** — the R1.5/R1.0
  equal-cusp stepovers differ only 1.23×, so the coarse tier buys
  little, while the split pays dendritic fringe (~6× tier 1's owned
  area), territory hopping (71,443 rapid mm vs arm A's 11), and
  double-cut bands. The operator's suspicion ("splitting could make
  terrain slower") is measured true HERE. Not a global refutation: the
  split's premise needs a large tool-size gap, which this ladder lacks.
- **The worst-point rule confirmed exactly**: whole-board spiral rings
  pay the steepest bank they cross — 99.7 % of D1's rings hit the 45°
  derate clamp and it still fails spec on steeper ground. A single-op
  whole-board strategy cannot meet spec on terrain with steep walls;
  band/shape selection is necessary, not stylistic.
- **The product action is the planner-preview price panel**: the planner
  would have recommended single-tool on this board if it priced its own
  plan. That ticket is now evidence-backed, not a UX nicety.
- Pre-registered surprises that fired, kept as registered: A beats C;
  D1 lands below the in-harness unified calibration row.

## 14. Track H — valley tracing opened, prize unmeasured (2026-09-02)

The operator proposed a drainage-tree finishing strategy for valley
areas: find each catchment, trace a valley tree, offset-trace around
it with a surface-relative stepover. Research came back on both
sides; the campaign is `planning/valley_tracing_2026-09-02/`
(TRACK.md = question + priors, FINDINGS.md = pre-registered bars).

- The literal reading (offset loops around the whole tree) is the
  medial-axis level-set family §9 already falsified on branched
  geometry. Track H does not test it again.
- The open reading is per-branch bidirectional tracing with a local
  width cap — pencil's offset machinery plus junction handling.
- Nothing runs before the V0 prize census: fraction of finish time
  in valley territory, and the honest raster's ×floor there. The
  bars (≥ 10 % of time, ≥ 1.15× floor) are pre-registered; either
  failing closes the track with no build.
- Hard caps restated: G-UNIONCOV (§12) blocks any "faster AND
  complete" claim for a valley/raster hybrid; the wanaka200 facet
  rule blocks spacing claims on the current export.

**CLOSED same day.** The track ran its full pre-registered ladder
in one session: V0 census (bars passed) → V0-att attribution (v1
formula failed on measurement and was superseded; v2's D_pot
cleared the bar at 5.9–9.3 pp) → V1 falsifier on the favourable
region, where the tracing arm **failed all three bars by
multiples** (fragments 14.2×, distance 3.28×, time 3.53×) —
adjacent branch offset-fans overlap on dendritic ground, §9's
pathology one level down. What survives: the measured, unharvested
D_pot; the avenue-G mechanism correction above; and the
region-polygon ≠ band-territory finding for Track M. Full record:
`planning/valley_tracing_2026-09-02/FINDINGS.md`.

## 14. Track M complete — one metrology home, and the union audit's first catch (2026-09-02)

`crates/rs_cam_core/src/metrology/` now carries the ONE implementation
of each ruler: costing (`relink_and_cost_under`, six test copies
retired to thin adapters), the L_min floor, achieved contact spacing
(Track B's gate quantity), the Monge estimator + both strategy censuses
with named evidence-cited thresholds, and the NEW `union_coverage`
audit (G-UNIONCOV's fix) with a loud `assert_within` failure API. The
measurement contract (None vs 0.0, the two air-cut denominators, the
two time scales) is written once, in the module docs. The b1 acceptance
sentinel is byte-identical through the move. Merge `5887b11d`.

The audit's pre-registered proof held both ways: the rejected
overlap-0.2 variant FAILS (18,341 mm², 45.8 % above spec, one
17,161 mm² connected patch, max standing 5.43 mm) and discriminates at
5.99× against production.

**And the first catch: PRODUCTION IS NOT UNION-CLEAN.** 3,062 mm²
(7.65 % of the footprint) stands above spec in 3,422 small patches —
under clean per-op wires. Unattributed; the shape (many small patches)
is consistent with the tip-float finding (valley floors the R1.0
cannot reach, worst 2.3 mm) but that is a hypothesis, not a
measurement. **Handed to the valley workstream** (a separate agent owns
valley milling); the metrology side is done — the ruler exists, every
future comparison runs under it.
