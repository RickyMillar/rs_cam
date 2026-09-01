# Track B findings — does the SHIPPED shallow raster meet its own scallop spec?

**Status: RUN COMPLETE (2026-09-01). VERDICT: DEFECT CONFIRMED.**

The pre-registration below is unchanged from before the run.

**Status of the registration block: PRE-REGISTERED before the run.** The verdict bands are fixed in
`TRACK.md` and are not restated in a softer form here. This section
registers the measurement plan and every additional falsifier BEFORE the
first run. Results land below the line after the run.

## Instrument

`crates/rs_cam_core/tests/shipped_raster_spacing_b1.rs`, `#[ignore]`
evidence convention. It drives the SHIPPED code path only:
`unified_finish_toolpath_with_cancel` with default
`UnifiedFinishParams` shape (`monotone_cell_decomposition: false`, the
shared 0-degree lattice), so the emitted motion comes from
`build_shallow_raster_grid` + `raster_toolpath_from_grid`
(`crates/rs_cam_core/src/unified_finish.rs`, `FinishBand::Shallow` arm).
The harness `raster_candidate` is not called anywhere in this instrument.

## Fixtures (analytic only, facets <= stepover/3, ratio printed as proof)

1. **SPHERE CAP** — restated from `conformal_spiral_synthetic_f2.rs`
   (R_s = 20 mm, cap radius 6 mm), densified to rings 64 x sectors 384 so
   the facet budget also holds for the honest-arm stepover. Umbilic, so
   `s_max` is one exact number:
   `scallop_math::stepover_from_scallop_curved(1.0, 0.03, 1/20)`.
2. **PLANE 20 deg** and **PLANE 40 deg** — a plane tilted about the X
   axis, slope rising along +Y, 12 x 12 mm in XY. The 0-degree raster
   advances its passes along +Y, so the cross-feed direction climbs the
   slope and the prediction is exact: achieved surface spacing
   `= s_XY / cos(theta)`. `s_max` is the flat stepover
   (`kappa = 0`). 40 deg sits just inside the Shallow band's 45-degree
   edge, which makes it the worst slope the shipped arm can own.

## What is measured, registered before the run

1. **All-Shallow gate (hard assert).** Every `region_table` entry must be
   the Shallow band. If any other band appears, the run REFUSES rather
   than measuring a mixed population.
2. **Facet proof (hard assert).** Max 3D triangle edge / stepover must be
   <= 1/3 for BOTH the stock stepover and the honest stepover. Printed.
3. **XY pass spacing.** Adjacent raster rows' Delta-y from the emitted
   toolpath. Prediction: exactly `raster_stepover` (the XY-projection
   mechanism). If Delta-y is NOT uniform at the stepover, the cos-theta
   mechanism claim is wrong as stated and the verdict must say so.
4. **Achieved surface spacing, measured from the toolpath itself.** Each
   cutting move's CL target is mapped to its analytic contact point
   (sphere: radial projection to the sphere; plane: orthogonal
   projection). For each contact sample on pass i, the achieved spacing
   is the min 3D distance to pass i+1's contact polyline. Interior
   samples only (CL >= 1 mm + 1 tool radius inside the boundary), so rim
   roll-off cannot contaminate the population.
5. **Slope-band table.** Samples binned by 5-degree bands of BOTH the
   total slope and the cross-feed slope `theta_y = atan(|dz/dy|)`. The
   mechanism's own variable is `theta_y`: a slope aligned with the pass
   direction does not widen cross-feed spacing, and the instrument must
   not credit the defect with samples the mechanism cannot touch.
6. **Verdict population (registered choice).** The TRACK bands speak of
   "sloped samples". Sloped = total slope > 5 deg. This is the
   CONSERVATIVE choice for confirming: on the sphere it includes samples
   whose cross-slope is near zero and which therefore cannot exceed, so
   it dilutes the exceeding fraction. The cross-slope population is
   printed beside it as context. Exceedance = achieved > 1.05 x s_max;
   material fraction bar = 10% (TRACK.md verbatim).
7. **x floor.** `L_min = area_3D / s_max` (exact on both fixtures:
   umbilic sphere, flat plane — the constant IS the integrand).
   `x floor = total_cutting_distance / L_min`. Prediction if the
   mechanism holds on the plane: `x floor ~= cos(theta)` at
   `s_XY = s_max` — BELOW 1.0, under-coverage visible in the score.
8. **Step 2, HONEST-RASTER arm (run only after the verdict on 1-7 is
   printed).** Same shipped path, `raster_stepover = s_max x
   cos(theta_max)` with `theta_max` the region's measured max face slope.
   Predictions: achieved spacing <= s_max everywhere measurable; x floor
   rises to ~1/cos(theta) x cos(theta) = ~1.0 on the planes.

## Additional falsifiers, registered before the run

- **F-B1 (mechanism):** if Delta-y between adjacent emitted rows is not
  `raster_stepover` (tolerance 1e-6 mm), the XY-projection mechanism
  claim fails and no cos-theta arithmetic may be quoted.
- **F-B2 (exactness):** on each plane the measured median achieved
  spacing must match `s_XY / cos(theta)` within 2%. If it does not, the
  contact-mapping instrument is broken and NO verdict may be issued from
  it (the analytic corroboration block decides nothing on its own).
- **F-B3 (instrument self-check):** the ball-centre-to-surface distance
  at every sampled contact must be `K_c` within 1% (sphere:
  `|c - S| = R_s + K_c`). A violation means the CL->contact map is
  reading rim roll-off and the interior inset failed.
- **F-B4 (honest arm):** the honest arm must not leave ANY interior
  sample above `s_max x 1.02` on the planes. If it does, "derate by
  cos(theta_max)" is not the fix the synthesis assumed and FINDINGS must
  say so.

---

## Results (run 2026-09-01, `cargo test --release -p rs_cam_core --test shipped_raster_spacing_b1 -- --ignored --nocapture`)

## Verdict, per the pre-registered bands

**DEFECT CONFIRMED.** The shipped `FinishBand::Shallow` arm spaces its
passes in XY projection. On a constant slope theta with the slope across
the pass direction, its achieved surface spacing is `s_XY / cos(theta)`
exactly. At `s_XY = s_max`, 100% of sloped samples exceed
`1.05 x s_max` on both tilted-plane fixtures — far past the 10%
material-fraction bar.

**With one measured qualification the pre-registration did not predict:
the sphere-cap fixture reads CLEAN** (max achieved / s_max = 0.9818).
The mechanism is present there too (F-B1: the emitted rows are
XY-uniform at the stepover on every fixture), but a second mechanism
cancels it on convex ground. See "The convexity correction" below.

Per-fixture verdicts:

| fixture | sloped samples | > 1.05 x s_max | max achieved/s_max | verdict |
|---|---|---|---|---|
| SPHERE CAP (R_s 20, cap r 6) | 302 | 0 (0.0%) | 0.9818 | CLEAN |
| PLANE 20 deg | 256 | 256 (100.0%) | 1.0642 | **DEFECT CONFIRMED** |
| PLANE 40 deg | 256 | 256 (100.0%) | 1.3054 | **DEFECT CONFIRMED** |

The registered falsifiers: F-B1 passed on all six arms (Delta-y uniform
at the stepover to < 1e-6 mm). F-B2 passed exactly — plane medians
0.51741 = s_XY x sec(20 deg) and 0.63470 = s_XY x sec(40 deg), error
< 0.01%. F-B3 passed (max contact-map error 0.00115 mm on the sphere,
0.0 on the planes). F-B4 passed — the honest arm's achieved spacing is
1.0000 x s_max on both planes, exactly.

## The decisive numbers

| fixture | arm | s_XY (mm) | median achieved (mm) | achieved / s_max | cut (mm) | x floor |
|---|---|---|---|---|---|---|
| SPHERE CAP | shipped, s_XY = s_max | 0.47431 | 0.45208-0.46081 by band | 0.953-0.972 | 308.1 | 1.262 |
| SPHERE CAP | honest, s_XY = s_max cos(17.3) | 0.45281 | 0.43164-0.43970 | 0.910-0.927 | 311.0 | 1.274 |
| PLANE 20 | shipped | 0.48621 | 0.51741 | **1.0642** | 310.1 | **0.984** |
| PLANE 20 | honest, cos(20) | 0.45689 | 0.48621 | 1.0000 | 338.9 | 1.075 |
| PLANE 40 | shipped | 0.48621 | 0.63470 | **1.3054** | 312.3 | **0.808** |
| PLANE 40 | honest, cos(40) | 0.37246 | 0.48621 | 1.0000 | 390.2 | 1.009 |

Notes on the table:

- The plane x floor sits at ~cos(theta) for the shipped arm (0.984 vs
  cos 20 = 0.940 predicted; 0.808 vs cos 40 = 0.766 — the excess over
  the prediction is turnaround chords and entry plunges, which
  `total_cutting_distance` counts). Below 1.0 proves under-coverage.
- The honest arm lands at 1.009x floor on the 40-degree plane: an
  honest raster on constant-slope ground is essentially AT the
  theoretical minimum. Its cost over the shipped arm is the cutting
  distance ratio 1.093x (20 deg) and 1.249x (40 deg) — the measured
  price of the finish the shipped arm was being credited with.
- The sphere x floor (1.262) is NOT comparable to the harness's 0.997x
  (synthesis §1): this instrument machines the full cap through the
  shipped orchestrator with plunges and turnarounds counted and no
  relink. The spacing distribution is the decisive measurement here,
  not the sphere's x floor.

## Slope-band table (shipped arm, achieved surface spacing vs s_max)

| fixture | cross-slope band | n | median | p90 | max | median/s_max | % > 1.05 s_max |
|---|---|---|---|---|---|---|---|
| SPHERE | 0-5 deg | 165 | 0.45208 | 0.45335 | 0.45336 | 0.9531 | 0.0% |
| SPHERE | 5-10 deg | 122 | 0.45558 | 0.45880 | 0.45882 | 0.9605 | 0.0% |
| SPHERE | 10-15 deg | 62 | 0.46081 | 0.46312 | 0.46566 | 0.9715 | 0.0% |
| PLANE 20 | 20-25 deg | 256 | 0.51741 | 0.51741 | 0.51741 | 1.0642 | 100.0% |
| PLANE 40 | 40-45 deg | 256 | 0.63470 | 0.63470 | 0.63470 | 1.3054 | 100.0% |

## The convexity correction — why the sphere reads CLEAN

The synthesis §1 sphere claim ("achieved surface spacing a median 3.3%
wider than admissible") came from the harness's ANALYTIC instrument
(`achieved_raster_spacing`): it multiplies the XY stepover by the slope
factor `sqrt(1 + (n_y/n_z)^2)` per triangle. That models the
XY-projection widening and nothing else.

The direct measurement models the whole geometry: the scallop spec on a
convex sphere binds the spacing of adjacent CONTACT rings
(`sphere_coverage_spacing_mm`, restated in the F2 file), and the ball's
contact point is not vertically under its centre on a slope. On a
convex surface the contact normal fan converges: CL rows `s_XY` apart
produce contact rings approximately

    s_contact = s_XY x sec(theta_y) x R_s / (R_s + K_c)

apart. At R_s = 20, K_c = 1 the focusing factor is 0.952, which beats
`sec(theta_y)` up to `cos(theta_y) = R_s/(R_s+K_c)`, i.e. 17.75 deg.
The F2 cap's rim slope is 17.46 deg — the entire fixture sits inside
the cancellation, so the shipped raster meets the spacing spec
everywhere this instrument can measure. The formula reproduces the
measured medians to < 0.5% in every sphere band (e.g. 0-5 deg band:
0.474 x 0.952 x 1.001 = 0.4522 vs measured 0.45208).

Consequences:

- The mechanism has a sign structure the programme's instruments did
  not carry: FLAT sloped ground pays `sec(theta_y)` in full (measured
  exactly); CONVEX ground gets a curvature refund `1/(1 + K_c kappa_n)`;
  CONCAVE ground pays MORE than `sec(theta_y)` (the fan diverges).
- The harness's 0.997x-floor sphere reading is not explained by
  interior over-spacing. Consistency arithmetic that this run makes
  possible: a CL lattice clipped at the region circle (XY r <= 6)
  reaches contact radius only 6 x 20/21 = 5.71 mm, leaving an uncovered
  rim annulus of ~10 mm^2 (~9% of the region), and CL path length
  overstates contact path length by (R_s+K_c)/R_s = 1.05 on a sphere.
  Together those account for a sub-floor CL length with in-spec
  interior spacing. This is an interpretation, not a measurement — an
  instrument for rim coverage would settle it — but the interior
  spacing measurement stands on its own.

## What the verdict means for prior comparisons

1. **The shipped shallow band shares the harness raster's defect.** Both
   build an XY lattice at the given stepover and emit through
   `raster_toolpath_from_grid`; F-B1 pins the emitted rows to XY-uniform
   spacing. Every prior "raster wins" comparison on sloped, non-convex
   ground credits the raster with a finish it does not deliver, by up to
   `1/cos(theta_max)` in spacing — 1.064x at 20 deg, 1.305x at 40 deg,
   1.41x at the 45-degree band edge.
2. **The measured price of honesty** is the honest-arm cut ratio:
   1.093x (20 deg), 1.249x (40 deg). Any comparison that a raster won
   by less than that margin on slopes of that order is undecided, not
   won.
3. **The sphere headline of synthesis §1 needs a correction.** "Achieved
   surface spacing a median 3.3% wider than allowed" on the sphere does
   not survive direct contact measurement — the analytic instrument
   omits convex contact focusing. On the F2 sphere cap the raster's
   interior spacing MEETS the spec. The under-coverage the 0.997x floor
   reading points at is real but lives at the rim (coverage), not in
   the interior (spacing). Comparisons on that fixture were closer to
   fair than §1 states; comparisons on flat-but-tilted or concave
   ground are exactly as unfair as §1 states, now with the shipped arm
   confirmed as the carrier.
4. **The honest raster is cheap to state and nearly optimal on constant
   slope**: derate the stepover by `cos(theta_max)` of the region and
   the 40-degree plane lands at 1.009x floor with spacing exactly at
   s_max. On MIXED-slope regions `cos(theta_max)` over-derates the flat
   parts — that is avenue E's locally-varying-spacing question, out of
   Track B's scope.

## Files

- Instrument: `crates/rs_cam_core/tests/shipped_raster_spacing_b1.rs`
- Full output: reproduce with the command at the top of this section.

---

# Fix acceptance (2026-09-01, operator ruling: ALWAYS ON, no dial)

**Status of this block: PRE-REGISTERED before the post-fix run.** The fix
derates each Shallow region's effective raster stepover by
`cos(theta_max)` before any lattice is built. `theta_max` is the maximum
slope the region's covered cells carry, read from the classification
slope map and clamped to the planner's `steep_threshold_deg`
(`unified_finish::shallow_region_max_slope_deg`). Regions at
`theta_max <= 1 deg` keep the shared 0-degree memo unchanged
(`SHALLOW_DERATE_MIN_SLOPE_DEG`; `sec(1 deg) - 1 = 0.00015`). Each derate
is reported through `ToolpathStats::derived_stepovers` with region index,
`theta_max`, the configured value, and the derated value.

## Registered falsifiers, before the post-fix run

- **F-FIX1 (spacing):** the reworked `shipped_raster_spacing_b1`
  instrument must read CLEAN (`achieved <= 1.02 x s_max`) on BOTH plane
  fixtures and STAY clean on the sphere. Any fixture above the bar
  falsifies the fix.
- **F-FIX2 (motion agrees with the report):** the emitted rows must be
  XY-uniform at the report's own `derated_stepover_mm` (tolerance
  1e-6 mm). A disagreement means the audit trail lies about the motion.
- **F-FIX3 (flat identity):** a flat fixture's emission must be
  byte-identical to the pre-fix arm: no derate entry, rows exactly at the
  configured stepover, on the shared memo path. Sentried (non-ignored) in
  `crates/rs_cam_core/tests/shallow_raster_slope_derate.rs`.
- **F-FIX4 (price):** the instrument's cutting-distance and x-floor
  columns are re-read after the fix and reported beside the pre-fix
  table. Expectation from the pre-fix honest arm: ~1.09x (20 deg) and
  ~1.25x (40 deg) cutting distance; x floor ~1.0 on the planes.

## Results (post-fix run, 2026-09-01, same command as the Track B run)

**ALL FOUR FALSIFIERS PASS. OVERALL VERDICT: CLEAN.** The instrument now
asserts the CLEAN bar and is the standing fix-acceptance gate.

| fixture | theta_max (report) | effective s_XY (mm) | max achieved / s_max | verdict |
|---|---|---|---|---|
| SPHERE CAP (R_s 20, cap r 6) | 16.818 deg | 0.45403 | 0.9391 | CLEAN |
| PLANE 20 deg | 20.000 deg | 0.45689 | **1.0000** | CLEAN |
| PLANE 40 deg | 40.000 deg | 0.37246 | **1.0000** | CLEAN |

- **F-FIX1** passed: both planes land at exactly 1.0000 x s_max; the
  sphere stays clean and moves DOWN (0.9818 -> 0.9391 — the convex
  refund plus the derate; the conservatism is the accepted v1 cost).
- **F-FIX2** passed: emitted Delta-y equals the report's
  `derated_stepover_mm` to < 1e-6 mm on every fixture.
- **F-FIX3** passed: the flat sentry takes no derate and emits rows at
  the configured stepover exactly; the `crease_own_region_pr6b` taper
  pin (a real production fixture) is byte-identical through the change.
- **F-FIX4** — the measured price (vs the pre-fix shipped table above):

| fixture | cut mm (pre-fix) | cut mm (post-fix) | ratio | x floor (pre) | x floor (post) |
|---|---|---|---|---|---|
| SPHERE CAP | 308.1 | 311.6 | 1.011x | 1.262 | 1.277 |
| PLANE 20 | 310.1 | 338.9 | 1.093x | 0.984 | 1.075 |
| PLANE 40 | 312.3 | 390.2 | 1.249x | 0.808 | 1.009 |

The plane rows are numerically IDENTICAL to the pre-fix manual honest
arm — the internal derate reproduces it exactly. The 40-degree plane
sits at 1.009x the theoretical floor with spacing at spec.

## Implementation record

- `crates/rs_cam_core/src/unified_finish.rs`: `FinishBand::Shallow` arm
  derates before any lattice exists; `shallow_region_max_slope_deg`
  (covered-neighbour guard + steep-threshold clamp);
  `SHALLOW_DERATE_MIN_SLOPE_DEG = 1.0`; a derated region gets a private
  region-windowed lattice (the C2 rotated-arm construction), and the C2
  decomposition and emission share it — one frame, one lattice.
- Audit trail: `UnifiedFinishReport::shallow_slope_derates` ->
  `ToolpathStats::derived_stepovers` (`DerivedStepoverFinding` gained
  `slope_derate: Option<SlopeDerateDetail>`), rendered by the
  `CONFIG_DERIVED_STEPOVER` diagnostic with its own message arm.
- Sentries: `crates/rs_cam_core/tests/shallow_raster_slope_derate.rs`
  (flat identity + 30-degree derate, non-ignored); the reworked
  `shipped_raster_spacing_b1` asserts CLEAN.
- Wanaka expectation (stated, not yet measured on the GUI): shallow
  regions carrying slope up to the 45-degree band edge derate by up to
  cos 45 = 0.707, so shallow-band cutting distance grows by up to
  1.41x on the steepest shallow regions and by ~1.1-1.25x on
  20-40-degree ground; mid-steep and very-steep bands are untouched.
  Each sloped region also builds its own windowed lattice (the C2
  rotated-arm cost). This buys the configured scallop actually being
  met — the pre-fix speed was under-delivery, not efficiency.


## The wanaka price, measured (2026-09-01, CLI `project`, 0.3 mm, worktree at `8cd15837`)

`planning/multitool_2026-08-23/wanaka200_mt2.toml`, both C2 dial arms,
post-fix, against the §7 pre-fix records (`thin_organic_2026-08-27/
FINDINGS.md` §7). Runtimes are the integrated machine estimate
(`total_runtime_s` / `toolpath_runtimes`), the same wire §7 read.

| | pre-fix | post-fix | ratio |
|---|---:|---:|---|
| C2 OFF: Finish tier 0 (R1.5) | 5,581.4 s | 6,566.2 s | 1.176x |
| C2 OFF: Finish tier 1 (R1.0) | 12,270.6 s | 13,704.9 s | 1.117x |
| C2 OFF: whole project | 25,938.8 s | 28,361.2 s | **1.093x (+40.4 min)** |
| C2 ON: Finish tier 0 (R1.5) | 5,137.3 s | 5,828.2 s | 1.134x |
| C2 ON: Finish tier 1 (R1.0) | 11,234.7 s | 12,193.4 s | 1.085x |
| C2 ON: whole project | 24,447.9 s | 26,108.2 s | **1.068x (+27.7 min)** |

Reading:

- Against the recorded 24,447.9 s dial-on baseline the operator will
  feel **+27.7 min on this board (1.068x)** — inside the 1.09–1.25x
  fixture bracket, because most of the board's shallow area is gentle.
- Only the two unified-finish tiers moved; the derate touches no other
  operation type.
- The pre-fix runtimes bought spacing the spec did not permit on the
  sloped fraction; the delta is the price of the finish the raster was
  being credited with.
