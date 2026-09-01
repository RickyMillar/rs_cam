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

### V1 — CONDITIONALLY OPEN (2026-09-02), pending V0-att.

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

### V2 — NOT OPENED
