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

### V0 — NOT RUN

(instrument name, commit, table, verdict go here)

### V1 — NOT OPENED

### V2 — NOT OPENED
