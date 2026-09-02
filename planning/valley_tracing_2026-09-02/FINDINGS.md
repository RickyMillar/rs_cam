# Track H — valley tracing: pre-registration and results

> Written 2026-09-02, BEFORE any instrument ran. The bars below are
> fixed before the first run. The operator may adjust a bar only
> before the phase that uses it runs — never after.
>
> Context and priors: `TRACK.md` beside this file.

## Pre-registration

### Q1 (phase V0) — is there a prize in valley territory?

**Measurements:**

- **M1** — fraction of front-finish cutting time spent inside valley
  territory (definition below).
- **M2** — ×floor ratio of the shipped honest raster inside valley
  territory only: `time_in_territory / L_min(territory)` with
  `L_min = ∫dA/s_max` over the territory's surface area.
- **M3 — misalignment.** Time-weighted angle between the region's
  C2 lattice direction and the local valley axis, inside the mask.
  This is the axis the candidate claims to exploit: if C2 already
  sweeps along the valleys, tracing has nothing left to win.
- **M4 — derate refund.** Per region: θ_max with mask cells excised
  vs with them included. The honest raster derates the WHOLE region
  by cos θ_max, so steep valley walls tax flat ground around them.
  M4 measures a *decomposition* prize (give valley walls their own
  territory), which is a different mechanism than tracing.

**Valley-territory definition (fixed before the run):** a
*territory*, not a crease line. The instrument computes TWO masks
and reports both —

- **mask A**: the `rivers_aligned.dxf` network buffered in XY by the
  local valley half-width (chamfer-DT width at each polyline sample,
  the `rest_field.rs` convention), clipped to finish territory.
- **mask B**: mesh-derived — valley lines from flow accumulation on
  the rasterized heightfield (priority-flood depression fill, then
  accumulation above a threshold chosen by network length
  stability), buffered by the same chamfer-DT local half-width.
  Clipped the same way. NOT the `crest_lines.rs` `κ₂` criterion —
  that detects narrow creases and reads near-zero on broad valley
  floors, which are the geometry under question; deciding with it
  would fail B1 by construction (the §2 meta-error).

The instrument reports M1–M4 per mask plus each mask's area share.
The DECIDING mask is B (the mesh is the ground truth; the DXF is
not incised). Mask A is reported for the DXF-probe caveat only.

**V0-pre — ceiling check, zero new strategy code.** Valley-following
is one direction choice, so its prize is bounded by the direction
prize ceiling. Run `wanaka_curvature_anisotropy.rs` restricted to
mask B before anything else. If the in-mask ceiling is below the
B2 bar, V0 closes immediately: no direction method can pay the bar
there. If it is above, B2 is confirmed reachable in principle.

**Bars (pre-registered):**

- **B1-prize:** mask B holds ≥ 10 % of front-finish cutting time,
  AND
- **B2-headroom:** the shipped honest raster reads ≥ 1.15× floor
  inside mask B, with the excess NOT explained by M3 ≈ 0 (see
  decision rules).

**Decision rules:**

- V0-pre in-mask ceiling < B2 bar → **Track H CLOSES**, zero new
  code beyond the mask.
- B1 or B2 fails → **Track H CLOSES with no build**, recorded here;
  the residual note goes to status doc avenue F if the evidence
  points there.
- B1 and B2 pass but M3 is small (the C2 lattice already runs along
  the valleys) → the headroom is not reachable by tracing; **Track
  H CLOSES**, and the evidence routes to whichever mechanism M4
  names.
- M4 dominates M2's excess → the prize belongs to *decomposition*
  (excise valley walls into their own territory), not tracing —
  record that verdict, close Track H's tracing arm, and open the
  decomposition question as its own ticket instead.
- B1, B2 pass and M3 is material → V1 opens.

Rationale for the bars: the whole-board raster already sits at
1.10–1.31× floor; the sweep-angle lever was worth 1.10× and was
judged worth pursuing; C2's ceiling was 1.215×. A territory below
10 % of time, or already within 15 % of its floor, cannot pay for a
new operation family plus junction handling plus the union-coverage
instrument debt.

### Q2 (phase V1) — does per-branch offset tracing beat a sweep?

**Arms, on ONE valley region chosen from the V0 census (largest
mask-B time share):**

- **arm S** — the shipped honest raster on that region (C2 lattice,
  cos θ_max derate), the control.
- **arm T** — per-branch bidirectional offset tracing: centerline
  per branch, perpendicular offset passes out one side and back,
  pass count capped by local half-width (pencil offset machinery),
  branches joined by the production relink.

**Costing (both arms identically):** `relink_and_cost_under`
(`crates/rs_cam_core/tests/thin_organic_island_widths.rs:1968`)
under the machined-stock `link_ceiling` regime, production relink
params, project machine kinematics. Spacing scored at equal
ACHIEVED scallop (the Track B convention) — not equal configured
stepover.

**Bars (pre-registered — synthesis §9's bar, adopted verbatim):**

- **B3-fragments:** arm T produces FEWER fragments AND fewer links
  than arm S.
- **B4-distance:** arm T cutting distance ≤ 1.5× arm S.
- **B5-time:** arm T `time_s` < arm S `time_s`.

**Decision rule:** all three pass → V2 opens. B3 fails → the
branch-point fragmentation mechanism from §9 reaches the per-branch
reading too; **Track H CLOSES**. B3 passes but B4 or B5 fails →
record the mechanism, close unless the loss is entirely junction
overhead AND a bounded junction fix is stated in one paragraph.

### Q3 (phase V2) — junctions and the hybrid

Not pre-registered yet. V2 opens only after Q2 passes; its
falsifier is written here before any V2 run. Known blocker to
restate then: G-UNIONCOV — the hybrid cannot claim completeness
until the union-coverage instrument exists.

## Standing hazards for every phase

- wanaka200 facet rule: ~0.35 mm median edge vs 0.486 mm stepover
  fails ≤ stepover/3. Efficiency numbers on the current export are
  NOT spacing claims. A finer export (rivmap `max_error`) is
  required before any spacing claim.
- `terrain_small.stl` is banned for any spacing claim.
- Air-cut % on fine tools at 0.3 mm resolution remains misattributed
  (rapid-safety M1 §L3). Do not gate any Track H verdict on it.
- State the win mechanism before running (status doc §2). For arm T
  the claimed mechanism is: passes follow the valley axis, so
  oblique-tributary zig-zag and per-region worst-point derate cost
  are avoided, at the price of junction handling. Any result that
  wins through a different mechanism is suspect until explained.

## Results

### V0 — RUN 2026-09-02. All gates pass. V1 OPENS.

**Instrument:** `crates/rs_cam_core/tests/valley_prize_census_h0.rs`,
numbers from commit `df3383a2` (instrument committed at `c14b2202`
before the first result run, per convention). Full log + SVGs:
`target/valley_census_h0/`. Verdict written by the orchestrator; the
implementing agent reported numbers only. The orchestrator ALSO read
the rendered mask SVGs before ruling (the render-the-surface rule):
mask B is a genuine dendritic valley-floor territory, and mask A's
DXF polylines visibly cross ridges — the not-incised caveat is
confirmed on the picture.

**Setup:** tier-1 Shallow band, 16 regions = 9289.4 mm² XY costed in
full; classification heightfield 825×825 @ 0.25 mm on the true
surface; equal-cusp stepover 0.4862 mm, derated to 0.3438 mm in 15
of 16 regions; whole-territory ×floor 1.1549 (cut-intent).

**The threshold rule degenerated and the verdict does not depend on
it.** |d ln L / d ln T| rises monotonically (0.48 → 3.66) — this
board has no plateau, so the pre-declared rule collapsed to the
lowest interior rung, T = 8 mm², where mask B = 61.61 % of territory
— above the 60 % sanity anchor. The agent did not re-pick; it added
a threshold sensitivity sweep, and every gate below is quoted across
the sweep, not at one rung.

**Gate-by-gate:**

- **V0-pre (ceiling): PASS.** In-mask direction-prize ceiling
  (bound, PCA frame) 1.2028× at R1.0 (1.2627× at R1.5), vs
  whole-territory 1.1099×. Above the B2 bar at every rung of the
  sweep (1.18–1.21×). Direction is worth roughly twice as much
  inside valleys as outside — the premise survives its bound.
- **B1 (≥ 10 % of time): PASS, threshold-robust.** M1 = 60.97 %
  at T = 8; still 19.3 % at T = 512. Fails only at the extreme
  T = 1024 skeleton (7.25 %). Caveat: M1's denominator is the
  tier-1 Shallow band only (the §0i regime), so spec-M1 over both
  tiers is smaller; at the picked rung the margin (6×) absorbs any
  plausible dilution.
- **B2 (≥ 1.15× floor in-mask): PASS.** M2 cut-intent 1.2139× at
  T = 8, in a 1.21–1.25× band at EVERY rung (time convention
  1.26–1.30×; all-non-rapid 1.36–1.44×, the extra being surface
  links). Honest reading: the excess is only mildly concentrated —
  in-mask 1.21–1.25 vs whole-territory 1.155 — and M1 tracks the
  mask's area share to ~0.7 pp everywhere, so valleys hold much
  time chiefly because they hold much area.
- **M3 (misalignment): MATERIAL — does not close the track.**
  44.0°–53.1° across the sweep where a fully isotropic axis field
  reads exactly 45.0°. The C2 lattice is effectively unaligned with
  the valley axes; the alignment headroom the tracing mechanism
  claims genuinely exists. CAUTION (added same day, review-caught):
  the in-mask excess (~21 %) and the direction ceiling (~20 %) being
  the same size is TWO candidate explanations of one number, not a
  confirmation of either — see the attribution amendment below.
- **M4 (derate refund): MEASUREMENT SATURATED — the routing rule
  COULD NOT BE EVALUATED.** All
  14 real regions read θ_max = 45.000° with refund 1.0000 at every
  threshold, because 45° IS the Shallow band's own clamp
  (`steep_threshold_deg`), reached through the 2 mm `overlap_mm`
  band-boundary fringe. M4 cannot see a valley-wall effect while
  the clamp is saturated; this is a property of the population, not
  a null about valleys. The "M4 dominates" routing rule therefore
  does not fire.

**Side-finding, ticketed separately (not Track H):** the shipped
derate on this board (0.486 → 0.344 mm in 15 of 16 regions, a ~29 %
distance cost on nearly the whole band) is set by the band-boundary
FRINGE (overlap-dilation cells at the band clamp), not by each
region's own terrain. Whether fringe cells belong in θ_max is a
spec question with a large measured price attached. → status doc
follow-on candidate.

**Deviations from spec, accepted:** M1 denominator narrower (upper
bound, margin absorbs it); low-ground DT substrate = TPI below
30 mm-window mean, declared pre-run, sensitivity at 15/50 mm flat;
hydrology restricted to land after the whole-board flood proved the
rivmap edge wall closes the basin (99.50 % raised — artefact,
printed as evidence); per-move time approximated by len/v_peak
rescaled to `compute_cycle_time` (rescale 1.03–1.07; distance share
agrees with time share to 0.04 pp, so not load-bearing); M2 printed
in both numerator conventions.

**Ruling (AMENDED same day, review-caught): B1 ∧ B2 ∧ V0-pre pass
and M3 is material — but V1 is CONDITIONALLY open, pending the
attribution measurement below.** The first ruling ("V1 OPENS")
treated M4's saturation as "the routing rule does not fire". That
was wrong: saturation means the decomposition-vs-tracing attribution
was NOT evaluated, and the derate arithmetic supplies a complete
alternative explanation of the whole in-mask excess.

**The derate arithmetic (from numbers already in the log):** the
shipped spacing in 15/16 regions is the derated 0.3438 mm. On a
flat valley floor s_max is the full 0.4862 mm, so a PERFECT raster
— no turns, no links, no misalignment — reads 0.4862/0.3438 =
**1.41× floor** there, and 1.00× on a 45° wall. Mask B is mostly
floor plus some wall; a floor/wall mix lands exactly in M2's
1.21–1.25× band with ZERO direction contribution. Out-of-mask
ground is steeper, so it sits nearer 1.0 — which also reproduces
"in-mask above whole-territory". M2's concentration may mean only:
valleys are flat, and flat ground pays the fringe-clamped derate
hardest.

**V0-att — attribution amendment (pre-registered 2026-09-02 BEFORE
its run):**

- **M2_spacing** = ×floor of a perfect raster at the shipped
  derated spacing, in-mask: `∫dA/s_shipped ÷ ∫dA/s_max` over mask
  cells. Everything needed is already computed by the census.
- **Residual** = M2 − M2_spacing (cut-intent convention) — the
  path-topology part, the ONLY part tracing can win.
- **Bar B2-att:** residual ≥ 5 pp of excess. Below that, arm T
  cannot pass B5 at equal achieved scallop; **Track H's tracing arm
  CLOSES** and the prize routes to the fringe/derate ticket.

This completes M4's intent (attribution); it does not move B1, B2,
or V0-pre.

**V0-att v1 RAN 2026-09-02 AND ITS FORMULA IS ILL-POSED — measured,
recorded, superseded (`9b1d7b74`).** Two structural defects, caught
by the implementing agent's stop-and-report guard, verified on the
numbers:

- With θ_max saturated at 45° in 15/16 regions, `∫dA/s_shipped ÷
  ∫dA/s_max` collapses to the constant `0.4862/0.3438 = √2`: it
  read 1.4140–1.4142 across all seven populations and can
  discriminate nothing. `dA` cancels within each region — the
  formula measures no geometry.
- It models a raster pitched ON THE SURFACE; the shipped raster
  pitches in XY. Measured M2 sits BELOW the model everywhere
  (residual −0.16 to −0.26 in every population) — that gap is the
  projection convention (measured along-track magnification 1.1518
  vs mean sec θ 1.4104 on the territory), not a topology finding.
  The 5 pp bar was never applied.

**V0-att v2 (pre-registered 2026-09-02 BEFORE its run) — the
direction-aware XY model that reproduces what ships:**

- **M2_xy(d) per population** = `Σ (a_cell / s_shipped(region)) ×
  sec(along-track slope in direction d)` ÷ `L_min`, with per-cell
  slope from the classification normals. Evaluated TWICE:
  - `d = d_shipped` — each cell's owning region's C2 lattice
    direction. This is the perfect (turn-free) shipped raster.
  - `d = d_valley` — the local valley-line tangent (nearest network
    line via the DT). This is the perfect valley-aligned raster at
    the SAME spacing. Evaluated on MASK populations only — outside
    a mask "the valley direction" is undefined and a territory
    figure would be meaningless.

  The along-track secant formula must be stated in the instrument's
  file header before the run (`tan φ = |∇z · d|`, `sec φ =
  √(1 + (∇z·d)²)`, or the agent's equivalent, written down) — a
  sign/convention slip already cost one rerun today.
- **Bracket check**: `A_xy/s_shipped ÷ L_min` (direction-independent
  lower bound) printed beside them; measured M2 must land between
  the bound and the v1 surface-pitch figure, else the model is
  wrong.
- **R_topo** = M2 − M2_xy(d_shipped): what the real generator
  spends beyond the perfect model (turn/edge structure). REPORTED
  FOR CONTEXT ONLY — it is NOT tracing's to win: a tracing arm has
  its own topology cost (junctions, width-capped stubs), and on
  §9's evidence that cost is larger on dendritic geometry, not
  smaller. Whether tracing recovers any of R_topo is exactly what
  V1's B3/B4 test; putting it in the V0 bar would assume V1's
  answer. Expected small or slightly negative (cut-intent already
  excludes links; the model has its own approximation error) — a
  small negative reads as "the model is close", nothing more.
- **D_pot** = M2_xy(d_shipped) − M2_xy(d_valley), in-mask: the
  direction prize at fixed spacing. CAVEAT, pre-registered: at
  fixed XY pitch an aligned pass on a wall has coarser cross-track
  surface spacing, so D_pot is an UPPER bound on the direction
  prize at equal achieved scallop.
- **Expected magnitude, stated up front so the result reads
  correctly:** mask B is mostly flat valley floor, where
  `sec(along-track) ≈ 1` in EVERY direction — D_pot is identically
  zero there. Only in-mask wall cells with a wall-aligned valley
  tangent contribute, and M3 (~45°, isotropic) says the shipped
  lattice is not systematically across-slope either. A D_pot of
  1–3 pp is the geometry speaking, not an instrument failure.
- **Bracket check:** `lower_bound ≤ measured M2 ≤ v1 surface-pitch
  figure` must hold per population (territory: 1.0027 ≤ 1.1549 ≤
  1.4142 ✓ by hand). A mask rung that violates it is a real model
  error, not noise.
- **Bar B2-att-v2 binds on D_pot ALONE:** D_pot ≥ 5 pp (0.05×) of
  floor, in-mask, at the picked rung and robust across the sweep.
  Below the bar the tracing arm CLOSES — D_pot is an upper bound,
  so an upper bound under the bar is a safe closure. At or above
  it, V1 proceeds knowing the bound is optimistic. P = R_topo +
  D_pot may be quoted only as a stated ceiling, never as the bar.

**V0-att v2 RAN 2026-09-02 (`1d6535cb`) — BAR PASSES. Ruling: V1
OPENS.** Bracket check passed on every population (territory
1.0027 ≤ 1.1549 ≤ 1.4142, reproduced to the digit). D_pot
(cut-intent): mask B 5.90 pp at the picked rung T = 8, 7.27–9.29 pp
at every other rung, mask A 8.00 pp — above the 5 pp bar at every
rung, so the pass is threshold-robust, though thin at the picked
rung. R_topo −3.45 to −5.99 pp in-mask ("model is close", context
only, unused). d_valley fallbacks ≤ 0.39 % and only shrink D_pot.
Standing caveat carried into V1: **D_pot is an UPPER bound** at
equal achieved scallop — the margin over the bar is 0.9 pp at the
picked rung, so V1's spacing-matched controls are load-bearing,
not formality.

### V1 — OPEN (2026-09-02, post-attribution). Not yet run.

**Arm details pre-registered before any V1 run (allowed: the phase
has not run; B3/B4/B5 unchanged):**

- **Coverage equality is a precondition of every comparison.** The
  arms must cover the SAME territory — the selected region in full
  — verified by CoverageAudit at spec scallop. A faster arm that
  covered less is not a result. (This is the per-instrument answer
  to the seam question; op-level union coverage remains G-UNIONCOV,
  Track M.)
- **Arm T is therefore a HYBRID within the region:** per-branch
  bidirectional offset tracing inside mask B ∩ region (centerline
  per branch, perpendicular offsets capped by local half-width,
  production relink), and the S′ raster outside the mask. Its
  fragments/links/distance/time are whole-arm, seams included —
  the seam cost is real and belongs to the candidate.
- **Spacing conventions:** arm S = shipped (region derate as
  shipped). Arm S′ = same raster, derate recomputed with the
  overlap-dilation fringe excised (falls back to the undilated
  region polygon if fringe identification is ambiguous — state
  which in the output). **Arm T's tracing passes derate PER BRANCH:
  XY offset pitch = stepover × cos(max cross-track slope along that
  branch's pass)** — the branch-local worst-point rule, which is
  what a shipped tracing op would do. The raster outside the mask
  uses S′'s convention. So B5 (T vs S′) compares each strategy's
  best shippable form.
- **Spec-meeting is a precondition, the ledger convention.** All
  three arms report their achieved surface spacing distribution
  (CoverageAudit, b1 convention) and their exceed % (fraction of
  achieved-spacing samples above 1.05× spec). An arm with
  exceed % > 5 % is marked FAILS SPEC and cannot win, whatever its
  time — the ledger D1/D2 rule.
- **Margin rule (pre-registered — the 0.9 pp upper-bound margin
  must bind the verdict, not decorate it):** if arm T's coverage
  shortfall against S′, or the time-equivalent of any achieved-
  spacing deficit (T coarser than S′), exceeds the D_pot margin at
  the picked rung (0.9 pp of floor), the tracing arm CLOSES even
  if the raw B5 time comparison favours T.
- **Region selection is the FAVOURABLE case and is named as such:**
  "largest mask-B time share" picks the most valley-dominated
  region — the fewest hybrid seams, the mechanism most likely to
  sink T. Legitimate as a first probe: if T cannot win here it
  cannot win anywhere. A pass here therefore opens a SECOND-region
  check at the median mask share BEFORE V2 — pre-registered now.
  The instrument prints the mask-share table for all 16 regions so
  the pick's position is visible.
- **Branch pruning:** centerlines from the flow network at the
  picked rung, pruned to branches ≥ 3× stepover so stubs do not
  fragment T for free; the pruning rule is stated in the
  instrument header before the run.
- **Reporting:** fragments and links from `relink_and_cost_under`'s
  `CandidateCost`, with the pre-relink fragment count beside them
  so B3 reads both ways; an SVG of arm T on the region (passes
  coloured by branch, seams visible) — the orchestrator eyeballs
  it before ruling, per the standing rule. No verdict from the
  implementing agent.

Region selection per Q2: the region with the largest mask-B time
share, from the census. Bars B3/B4/B5 as pre-registered above, plus
one amendment pre-registered before any V1 run:

- **Arm S′ (spacing-matched control), REQUIRED.** Arm T's
  surface-relative offsets will locally run ~0.486 mm on floors
  while arm S is pinned at the region's derated 0.344 mm — so T can
  "win" on spacing it was granted, not on tracing. S′ is the same
  C2 raster granted the same freedom: derate computed with fringe
  cells excluded (or the mask as its own region). **B5 binds
  against S′, not S.** Tracing that beats S but not S′ is avenue F
  / decomposition wearing a new name, and closes the tracing arm.

### V1 — RUN 2026-09-02. ALL THREE BARS FAIL. THE TRACING ARM CLOSES.

**Instrument:** `crates/rs_cam_core/tests/valley_branch_falsifier_h1.rs`
(`2ae57a31` → `99eb30aa`; the middle commit fixed the coverage
audit's lift direction — it was lifting INTO the material).
Region 1, the pre-registered favourable case (3104 mm², 64.4 % of
its own cutting time in-mask — largest of all 16). Artifacts:
`target/valley_falsifier_h1/`. The orchestrator read the arm-T SVG
before ruling.

| arm | fragments | links | retracts | time s | cut mm |
|---|---|---|---|---|---|
| S (shipped raster) | 199 | 195 | 3 | 1049.0 | 12 230 |
| S′ (fringe-excised) | ≡ S | ≡ S | ≡ S | ≡ S | ≡ S |
| T (tracing hybrid) | **2819** | 2790 | 28 | **3698.3** | **40 107** |

- **B3 (fewer fragments AND links): FAILS, 14.2×.** 2819 vs 199
  fragments. Per the pre-registered rule, this is §9's branch-point
  fragmentation mechanism reaching the per-branch reading.
- **B4 (distance ≤ 1.5×): FAILS, 3.28×.**
- **B5 (time < control): FAILS, 3.53×.**
- Coverage differential also against T (+0.34 pp unmachined,
  larger largest-hole). The margin rule never became relevant.

**The mechanism, visible in the SVG:** neighbouring branches'
offset fans overlap heavily across the same ground — valley
spacing on a dendritic network is smaller than the sum of adjacent
branch half-widths, so width-capped fans re-cut each other's
territory. 92 branches traced; per-branch pitch p50 0.3375 mm.
The concentric-tree reading died of ring-splitting (§9); the
per-branch reading dies of fan overlap. Same family, one level
down.

**Ruling: Track H's tracing arm CLOSES on the favourable case.**
If it cannot win here it cannot win anywhere; the pre-registered
second-region check is moot. V2 never opens.

**RULING RE-BOUNDED same day (operator-caught).** The operator
looked at the render and asked two questions the ruling had not:
the traced tree was the DENSEST rung (T = 8 — a branch every
~1.7 mm against fans 2.6–7 mm wide, so fan overlap was certain by
arithmetic), and the territory was a planner band-region — a
corridor of many catchments' parallel trunks — not the proposal's
one-catchment decomposition. That is meta-error §2 in this repo's
own doctrine: the configuration could not express the candidate's
advantage. The closure is therefore **BOUNDED, not final**: V1
refutes per-branch tracing of the dense network on band-region
territory. It does not test (a) a sparse trunk tree (e.g. the
T = 512 rung: ~455 mm of network, lines ~8 mm apart — fans would
barely overlap), or (b) catchment-decomposed territory. "If it
cannot win here it cannot win anywhere" is WITHDRAWN — region 1
was favourable for seams and unfavourable for fan separation, and
the ruling conflated the two axes. A V1b (same harness, sparse
tree) is the cheap next falsifier if the operator wants it.

**Findings that outlive the closure:**

1. **S′ ≡ S — fringe excision ALONE is insufficient on region 1;
   the avenue-G mechanism is broader, not absent.** Both permitted
   identifications (DT ≥ 2 mm interior; mask-as-region) leave
   θ_max = 45.000°, because **42.90 % of the region polygon's 3D
   area is steeper than 45°** — and per finding 2 those are the
   SAME cells the Shallow band does not own and cannot finish to
   spec by construction. So the correct mechanism statement is:
   **non-owned steep inclusions inside Shallow polygons set the
   band's derate; the 2 mm overlap fringe is one SOURCE of such
   cells, not the whole mechanism.** The spec question is
   unchanged, and the refund is NOT "possibly zero" — the
   unmeasured quantity is the refund from excising everything the
   band does not own (owned-cells-only θ_max), which region 1's
   slope census makes plausibly large. Region 1 shows only that
   the 2 mm-DT excision does not reach it.
2. **The Shallow region polygon is not the band's territory.** The
   coverage audit read ~47 % of the region polygon unmachined at
   spec — attributed by the slope census: every mm² steeper than
   the 45° clamp is under-covered by the Shallow arm BY
   CONSTRUCTION. Two hypotheses, NOT resolved here: (a) the region
   polygons are XY outlines whose steep holes are simply not
   carved out — then Track M must audit band-CELL ownership, not
   polygons, or every Shallow op reads ~half uncovered; (b) the
   planner's slope classification disagrees with the true-surface
   census (`ClassificationSampler::PRODUCTION`) — which would be a
   planner-side classification defect LARGER than anything Track H
   measured. Deciding between them is one comparison of the
   planner's band-cell set against the census slope map, and it
   belongs to whoever picks up avenue G.
3. **The achieved-spacing gate for terrain arms remains unbuilt.**
   The b1 instrument needs an analytic contact oracle and a 0°
   lattice; neither holds here. Moot for this ruling (T lost on
   raw distance and fragments, which no spacing convention can
   rescue) — but any future terrain arm comparison still lacks
   that gate.
4. **D_pot (5.9–9.3 pp upper bound) stands measured and
   UNHARVESTED.** Every known harvesting mechanism is now
   individually refuted on terrain: per-cell direction (D1,
   0.917×), the direction field (coherence), per-branch tracing
   (this run, 3.53×). What remains open is avenue F
   (spacing-along-pass) — a different lever entirely.

### V2 — NEVER OPENS (tracing arm closed at V1)

## Phase W — catchment ZONES (operator reframe, 2026-09-02)

The operator's clarified proposal: a catchment is a ZONE — one
basin, ridge divide to trunk, the FULL slope range, replacing the
band split within each zone. The operator offered two variants and
delegated the choice: (1) passes anchored on the valley bottom,
offset outward; (2) the EXISTING scallop op run per basin (anchored
on the divide, converging onto the valley bottom). Variant 2 is
adopted first: for a simply connected basin the two anchors give
the same pass family, and variant 2 costs zero generator code.
Whether basins ARE simply connected is part of what W0 measures —
tributary saddles pinch divide-inward offsets exactly where
confluences split trunk-outward ones, so "same family" is a claim
under test, not settled.

**No band clip this time.** W runs on the full front finish
territory, all slopes. The V0 Shallow-band clip was an unflagged
assumption and is the reason Track H tested flat floors.

**Comparator:** ledger arm A (whole-board `Scallop`, R1.0) — the
best measured single-op on this board (10 057 s finish, 11 mm
rapids, ~untouched debit 1 425 mm² pending the union instrument).
A win over arm A would still need the ledger's arm-B correction
re-run before any claim against PRODUCTION (arm C).

**Stated win mechanisms (X4):** (i) fewer interior pinch-outs if
basins are closer to topological disks than the board (measurable
from shape census + §9's compact-vs-elongated rule); (ii) the
untouched-debit question — UNMEASURABLE until Track M's union
audit exists, so it cannot be a bar; (iii) landform-conformal
appearance — real, operator-review material, never a bar.

### W0 — basin census. Pre-registered BEFORE its run.

Extract watersheds on the full territory (flow machinery from the
V0 instrument; basins draining to trunk outlets at the T = 512
trunk network; merge basins < 50 mm² into their downstream
neighbour). Report per basin: area, simple-connectivity, PCA
aspect ratio, slope-band mix, clipped-by-territory flag; plus
basin count, total divide length, and an SVG of the basin map over
the hillshade.

- **Bar W0-a (shape, DECIDING):** ≥ 50 % of front-finish-territory
  area lies in basins that are simply connected AND have PCA
  aspect ≤ 2 (the compact class per §9: sphere 1.036× vs ribbon
  2.525×; aspect 2 is the declared cut, derived from nothing
  sharper — stated as a judgment, pre-registered).
- **Seam budget (reported, ruled by the orchestrator):** two
  declared predictors bracket decomposition overhead — closed-ring
  model 0.38 s × 2 × N_basins; open-pass model 0.38 s ×
  L_divide / 0.486 mm. The budget is the D_pot prize scale
  (~150 s on this territory). If even the LOWER bracket exceeds
  it, W closes.
- W0-a fails → **phase W CLOSES on the census**, same as V0 would
  have — no build. Passes → W1: production scallop per basin vs
  arm A's scallop on the same territory, equal achieved scallop,
  existing harness; W1's bars are written before any W1 run.

### W0 — RUN 2026-09-02. Bar W0-a FAILS at every rung. PHASE W CLOSES.

**Instrument:** `catchment_basin_census_w0.rs` (`86663373` →
`193869af`; the middle commit fixed a self-caught sea-labelling
bug that had opened 31 602 basins). Full territory, no band clip:
28 545 mm² XY / 42 637 mm² 3D across all 23 planned regions.
Basin map SVG read by the orchestrator before ruling: large
coherent interior catchments, visibly elongated and lobed, with
confetti of tiny coastal basins along the shore and valley
corridors.

**A definition gap, reported by the agent's stop guard (the
orchestrator's spec error):** "drain to a trunk outlet" does not
reach coastal ground that drains straight to the sea — 33.6–67.0 %
of territory area across the rungs. Those basins have no
downstream neighbour, so the 50 mm² merge rule was inert (0 merges
at T = 512) and N_basins ≈ 1 600 is an artifact of the gap, not
catchment structure. Both seam predictors are therefore
artifact-driven and were not used in the ruling.

**The ruling is robust to the gap — charitable-bound arithmetic:**
compact area share (simply connected AND aspect ≤ 2) measured
19.54 % at T = 512, best rung 33.73 % at T = 256, vs the 50 % bar.
Grant the impossible best case — every coastal mm² re-merged into
perfectly compact basins — and the share still lands at **49.9 %
(T = 256) / 47.3 % (T = 512)**, under the bar at every rung. And
the REAL catchments carry the failure: trunk-keyed basins' compact
share collapses with scale — 29.9 % (T = 256) → 4.05 % (T = 512,
one basin) → 0.00 % (T = 1024). The larger the true catchment, the
more elongated and multiply connected (largest 8: aspects
1.7–2.9, up to 5 boundary loops). Wanaka's catchments are exactly
the shape class where §9 measured offset families losing.

**Ruling: bar W0-a FAILS at every rung, robust to the definition
gap. PHASE W CLOSES ON THE CENSUS — no build, as pre-registered.**

**RULING SUSPENDED same day (operator-caught, the third catch).**
The operator read the basin map and found the defect: the land
view was defined as z > 0, which treats every LAKE like the sea —
a hole in the world. Consequences: (a) ground draining to a lake
terminates at a fake lake-edge outlet — that is the confetti
ringing the lakes and much of the "coastal" artifact; (b) lake
beds (part of the 6.6 % unlabelled below-floor cells) carry no
label; (c) the charitable bound granted the CONFETTI compactness
but never included the lake beds, and the bar failed by only
0.1 pp at the best rung under that bound — so robustness does NOT
hold once lakes are modelled correctly. Geography, per the
operator: everything drains to the sea once lakes fill and
overflow; each lake belongs to exactly one catchment.

**W0b — corrected hydrology, pre-registered before its run:**

- Base level = ONLY water connected to the board border (sea +
  coastline trench). Interior water is land: the priority-flood
  fills lakes as depressions and flow continues through them to
  the sea. A lake and its drainage ring then land in one
  catchment, labelled like any other ground.
- Sea-edge micro-basins (true land outlets on the coast): each
  merges into the neighbour sharing its longest divide, repeated
  to fixpoint, so the map holds only trunk-keyed catchments plus
  coastal segments ≥ 50 mm². The merge rule is declared here, not
  invented in the instrument.
- Same reports, same rungs, same SVG. **Bar W0-a is unchanged
  (≥ 50 % compact + simple by area) and rules the corrected map.**
  The suspended ruling is replaced by whatever the corrected
  census shows.
- Flat resolution added (operator-caught, second W0b catch): the
  network visibly broke on flat ground; the priority-flood now
  imposes an epsilon drainage gradient across filled flats toward
  the spill edge, and the trunk-link count is reported before and
  after so the effect is measured.
- The river overlay rides the W0b basin map (operator request):
  one tree wholly inside one basin colour is the visual check.
- **Avenue-F prize field added as a REPORTED measurement, no
  bar:** p(x) = s_max(x)·cos θ(x); mean/min (and the
  1st-percentile-min variant) whole-territory and per basin. It
  gates whether a variable-spacing ring candidate is ever worth
  pre-registering — avenue F's own measure-first rule.

**Scope note on W0-a, recorded before W0b lands:** the bar's §9
anchor (offsets lose on elongated shapes) was measured with
CONSTANT-spacing rings; a spacing-weighted (Eikonal) ring family
absorbs the ring-anisotropy tax that produced that evidence. So
W0-a rules the plain-scallop-per-basin candidate (W1 as
registered). A weighted-spiral candidate is a DIFFERENT candidate:
it needs its own pre-registration, informed by the prize field,
and W0-a neither passes nor fails it. Cross-session context: a
parallel metrology-lane discussion converged on the same
catchment/Eikonal framing; the operator ruled that this session
owns the watershed work. The metrology lane's collision-free
assignments remain the union audit on band-cell ownership and its
two clippy-red files.

**Findings that outlive the closure:**

1. **The full-slope territory is 69.3 % steeper than 45° (3D
   area).** Removing the band clip lands any whole-strategy
   candidate mostly on very-steep ground — the direct explanation
   of V1's 47 % coverage reading, and the context any future
   full-territory proposal must state up front.
2. **6.6 % of territory cells sit at or below the land floor
   (z ≤ 0)** and can carry no basin label — bookkeeping for any
   future watershed work.
3. **Track M's commit `2e2ef306` left the workspace clippy gate
   red** on `union_coverage_m1.rs:71` (`ptr_arg`) and
   `wanaka_curvature_anisotropy.rs:489` (`wrong_self_convention`)
   — outside this track's footprint, relayed to the operator.
