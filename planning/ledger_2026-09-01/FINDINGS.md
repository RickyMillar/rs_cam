# Track G findings — the strategy ledger on the wanaka job

**Status of this block: PRE-REGISTERED 2026-09-01, BEFORE any run.**
Nothing above the "Results" line changes after the first execution.

## Arm definitions

See `TRACK.md`. Restated fixed choices:

- Every CLI arm runs `rs_cam_cli project <toml> --resolution 0.3` with
  default flags (adaptive feed modulation ON — the CLI default since
  Checkpoint K, the same operating point as the Track B wanaka table).
- Arms A and B keep every upstream op of `wanaka200_mt2.toml` (the back
  setup and the front rough), so the prior stock state matches arm C.
  Only the two finish tier ops are replaced.
- Arm A: one `scallop` op, R1.0, `scallop_height 0.03`,
  `tolerance 0.05`, `outside_in`, `continuous false` (the shipped
  default), slope band 0-90, feeds 735/180, boundary disabled (whole
  board).
- Arm B: one `unified_finish` op, R1.0, the tier-1 params verbatim
  (scallop 0.03, raster_stepover 0.48621, `monotone_cell_decomposition
  true`, feeds 735/180), boundary disabled, heights auto.
- Arms D1/D2: harness instrument
  `whole_board_spiral_ledger_g1.rs`. Board outline = the terrain mesh
  XY bbox (measured 200 x 200 mm). The rectangle EDT is closed-form:
  level sets are nested axis-aligned rectangles about the centre. The
  spiral covers the WHOLE board with the R1.0 tool — no tier split, no
  band split. Gouge protection: every bridged spiral point is re-dropped
  through `point_drop_cutter` against the full terrain mesh with the
  production TaperedBallEndmill (ball dia 2.0, taper half-angle 5.7 deg,
  shaft dia 6.0, cutting length 20). Costing: the C1 relink block
  (hookup 25.0, sampling 0.5, `reorder: true`, board rectangle as
  boundary, `link_ceiling: None` — the fresh-stock first-experiment
  exception every harness number in this programme carries), feeds
  735/180, the project's machine kinematics (accel [500,500,270],
  junction deviation 0.02, max feed 10,000, rapid 5,000).
- CAL: `unified_finish_toolpath_with_cancel` on the same mesh, tier-1
  params, whole-board region, costed by `compute_cycle_time` with the
  same kinematics. Its pairing with CLI arm B measures the scale gap
  between the two integrators (dressups, feed optimization, arc fitting
  live only on the CLI side).

## The D1 derate rule (fixed before any run)

`s_flat = stepover for K_c 1.0, h 0.03 = 0.48621 mm` — the production
tier-1 `raster_stepover`.

March inward from the board outline. Ring 0 sits AT the outline
(inset 0). From ring k at inset `d_k`:

1. Place a TRIAL ring at `d_k + s_flat`.
2. Sample the terrain surface along both rings at matched lattice
   angles (a dia-0.01 ball probe through the drop-cutter).
3. `theta_k` = the maximum over matched samples of
   `atan(|z_trial - z_k| / gap_xy)` — the worst cross-ring slope the
   pass pair crosses.
4. Clamp: `theta_k <- min(theta_k, 45 deg)`. This is the production
   clamp — `shallow_region_max_slope_deg` clamps at
   `steep_threshold_deg = 45`, so D1 is exactly as honest as the
   shipped derate, no more.
5. `s_k = s_flat x cos(theta_k)`; ring k+1 sits at `d_k + s_k`.

No fixed-point refinement (one trial evaluation per ring — a stated
simplification; the achieved-spacing block is the backstop). The march
stops when the inset reaches half the short board side minus 0.05 mm;
`hub_cover_reach_mm` (0.243 mm at K_c 1, h 0.03) hands the centre to
the module's hub blend.

D2 uses the same construction with the derate deleted:
`s_k = s_flat` for every ring — the legacy raster's XY spacing.

## Measured spec columns (instruments, not assertion)

- **Achieved spacing** (D arms): 3D distance between adjacent rings'
  surface samples at matched lattice angles. Spec convention = Track
  B's: exceedance is `achieved > 1.05 x s_flat`; the arm meets spec
  when <= 10 % of samples exceed (the material-fraction bar). The full
  min/p50/p90/max block is printed.
- **CoverageAudit** (D arms): every terrain triangle centroid inside
  the board, lifted `h` along its face normal, tested against the
  FINAL relinked CL path at radius `K_c` (1.0 mm) — the C1-F4
  convention. Bar restated from C1-F4: <= 2 % unmachined area.
- Arms A/B/C: this ledger has no per-sample spacing instrument on the
  CLI wire. The spec column cites Track B: the shipped shallow band
  post-derate reads CLEAN on its acceptance instrument; the scallop op
  and the mid-steep/very-steep bands space on the surface by
  construction. Stated as a citation, not a new measurement.

## Pre-registered expectations

1. **D1 loses badly on cutting distance.** The worst-point rule
   (synthesis section 3 / section 9: one spacing per pass pays for the
   pass's worst point) applies to a whole-board ring at its worst:
   nearly every nested rectangle crosses a steep river bank somewhere,
   so nearly every ring derates toward the 45-deg clamp. Expected D1
   cutting = 1.3-1.41x D2's. The tiered stack derates per REGION and
   hands > 45 deg ground to surface-spaced bands, so it pays far less.
2. **D2 wins on retracts and loses spec.** Expected 0 or near-0 kept
   retracts (the C1 result), with the spacing block reading like the
   pre-fix raster: exceedance on the slope bands above ~18 deg, past
   the 10 % bar -> spec NO.
3. **D1 spec: still NO where ground exceeds 45 deg** (the clamp caps
   the derate); mostly clean below. The spacing block will say which.
4. **Expected time ordering (finish territory, one scale each):**
   C < B < A on the CLI scale (the tier split exists because R1.5
   covers coarse ground cheaper; the raw scallop is contour-parallel
   everywhere, section 0d/0k losses, plus ring fragmentation
   retracts). D2 < D1 on the harness scale. Where D2 lands against
   B/CAL is the genuinely open question — section 3 says spacing, not
   continuity, is the expensive problem, so D2 (same XY spacing as the
   legacy raster, zero retracts) should land NEAR the CAL raster time,
   minus link overhead. D1 lands well above CAL.
5. **What would surprise:** a bridge refusal on nested rectangles
   (should be impossible on a closed-form rectangle EDT); D1 within
   1.1x of D2 (would mean wanaka's steep fraction is smaller than
   every prior census says); D2 beating arm B's unified op on the
   CALIBRATED comparison by more than the link saving (~0.38 s per
   removed retract, synthesis section 3) — that would mean continuity
   matters more than section 3 measured; arm A beating arm C (would
   refute the tier split the product ships).
6. **Scale caveat, stated now:** CLI times and harness times are NOT
   one column. The CLI integrates dressup-fitted, feed-optimized,
   modulated motion over the full project; the harness integrates raw
   relinked motion over one op. The CAL row is the bridge; cross-scale
   rows in the final table are flagged.

---

## Results (all runs 2026-09-01, worktree at `ccf4d099`)

Every CLI arm ran `target/release/rs_cam_cli project <toml> --resolution
0.3` (one binary, default modulation ON). Every harness arm ran
`whole_board_spiral_ledger_g1` in release. Full logs and per-op
runtimes: `target/ledger_g/` (`armA.log`, `armB.log`, `armC.log`,
`d1.log`, `d2.log`, `cal.log`, `arm*_runtimes.txt`). Arm C reproduced
the Track B record exactly (26,108.2 s; tier 0 = 5,828.2 s; tier 1 =
12,193.4 s), so the reference chain is unbroken.

### THE TABLE — finish territory, one row per arm

| arm | scale | cutting mm | rapid mm | retracts | integrated s (h) | spec met? + measured spacing | notes |
|---|---|---:|---:|---:|---:|---|---|
| A. whole-board `scallop`, R1.0 | CLI | 117,654 | 11 | 0 retract links at generation (223 fragments, 222 surface links) | **10,057.0 (2.79 h)** | surface-spaced by construction (citation, not measured here) | **fastest CLI arm**; measured coverage debit: `untouched_material_mm2` = 1,425.1 |
| B. whole-board `unified_finish`, R1.0 | CLI | 142,635 | 14,695 | not on the CLI wire (CAL proxy: 829) | 12,647.3 (3.51 h) | CLEAN by the Track B acceptance instrument (slope derate always-on) | `unmachined_band_area_mm2` = 4,139 (report-only) |
| C. production two-tool tiers (R1.5 + R1.0) | CLI | 184,225 | 71,443 | not on the CLI wire | **18,021.6 (5.01 h)** | CLEAN per-op by the same citation; per-op coverage findings read 0.0 — but see G-UNIONCOV below | tier 0 = 5,828.2 s + tier 1 = 12,193.4 s |
| D1. whole-board spiral, spec-honest derate | harness | 136,230 | 7 | **0** | 11,227.7 (3.12 h) | **NOT MET** — exceed 11.33% vs 10% bar; p50 0.3629, p90 0.5156, max 3.71 | 292 rings; 99.7% of rings AT the 45-deg clamp; coverage audit 0.956% unmachined (bar 2%) |
| D2. whole-board spiral, XY-spaced | harness | 96,451 | 7 | **0** | **7,949.9 (2.21 h)** | **NOT MET** — exceed 50.24%; p50 0.5111, p90 0.7231, max 2.90 | 207 rings; coverage audit 1.041% unmachined; the legacy raster's spacing convention |
| CAL. shipped `unified_finish` in-harness | harness | 115,920 | 14,193 | 829 | 12,355.8 (3.43 h) | (same generator as B) | 45 planned regions, 30 shallow slope derates; the calibration row |

Whole-project totals (CLI scale, same upstream ops in every arm):
**A 17,808.0 s | B 20,398.3 s | C 26,108.2 s.** The upstream six ops
reproduced identically across all three runs (e.g. front rough
2,574.5 s in each), so every whole-project delta is the finish stack.

### Calibration — the two scales, side by side

The CAL row is the shipped generator arm B runs, costed by the harness
integrator. CLI arm B reads 12,647.3 s; CAL reads 12,355.8 s — a 1.024x
time ratio. The MOTION is not identical: the CLI op carries the claims
pipeline (`claims_reference = "auto"`), the dressup chain (arc fitting,
feed optimization, rapid reorder) and the remaining-stock context, and
its cutting distance is 1.23x CAL's (142,635 vs 115,920 mm). So the
2.4% time agreement is partly coincidence of offsetting differences,
not proof of integrator identity. Rule used in the reading below:
treat a cross-scale time gap as real only when it is far larger than
that content gap — D2 vs B (-37%) and D1 vs C (-38%) qualify; D1 vs B
(-11%) is suggestive, not settled.

G-UNIONCOV (filed on master, `planning/finishing_status_2026-09-01.md`
section 12): per-op coverage findings cannot see BETWEEN-op territory
gaps in a multi-op chain. Arm C is a two-op chain, so its per-op 0.0
coverage findings do not prove the union covers the board. Arms A, B,
D1, D2 are single-op and are not exposed; the D arms additionally
carry an independent union audit (the CoverageAudit column).

### Expectations, scored

1. **D1 loses badly on distance — CONFIRMED, at the top of the
   predicted band.** D1/D2 cutting = 136,230/96,451 = **1.412x**
   (predicted 1.3-1.41x). Mechanism confirmed as registered: 291 of
   292 rings hit the 45-deg clamp — on wanaka, a whole-board ring
   ALWAYS crosses a steep bank, so the worst-point rule taxes every
   ring at the maximum.
2. **D2 wins retracts, loses spec — CONFIRMED.** 0 retracts, 7 mm of
   rapid on a 96 km cut; 50.24% of spacing samples exceed. Note the
   p50 itself (0.5111) sits just over the 1.05x bar: MOST of this
   board is sloped enough to break XY spacing, not just the banks.
3. **D1 spec — NOT MET, marginally (11.33% vs 10%).** The 45-deg clamp
   leaves the >45-deg ground exceeding, exactly the registered
   mechanism. A whole-board spiral has no waterline band to hand that
   ground to.
4. **Time ordering — HALF-CONFIRMED, with two pre-registered
   surprises that FIRED.**
   - C < B < A predicted; measured **A < B < C**, both halves wrong.
   - Surprise (a): **arm A beats arm C** — 10,057.0 vs 18,021.6 s.
     The raw whole-board R1.0 scallop, the op the operator named,
     beats the production tier split by 7,964.6 s (2.2 h) on the CLI
     wire, with a measured 1,425 mm2 untouched-material debit the
     tiers do not show per-op.
   - Surprise (b): **D1 does NOT land well above CAL** — 11,227.7 vs
     12,355.8 s. Continuity plus steady straight-line kinematics
     (7 mm of rapid, zero retracts, feed pinned at 735) absorbs the
     whole worst-point spacing tax on this board. D1 cuts 18% MORE
     distance than CAL and still integrates 9% FASTER.
5. **The tier-split anatomy, visible in the C row:** tier 1 alone
   (12,193.4 s, 126,070 mm) already costs what whole-board arm B costs
   (12,647.3 s, 142,635 mm) — the planned tier map hands tier 1 most
   of the board, and tier 0's 5,828.2 s + 36,865 mm of rapid buys the
   remaining sliver with a second tool. On THIS board the split is the
   losing layer, not the winning one.

### What this answers for the operator

- **"What parts are gaining my time and what are losing me":** on this
  board the two-tool tier split is the single largest loss —
  5.0 h vs 3.5 h for one whole-board R1.0 unified op with the same
  spec honesty, and vs 2.8 h for the raw scallop (with its 1,425 mm2
  coverage debit to inspect first). The slope derate and the C2
  decomposition are not the cost; the split is.
- **"Spiral over the whole area in one go with the new method":** at
  like-for-like (dishonest) spacing it is the fastest thing measured
  (2.21 h, zero retracts) and fails spec the same way the legacy
  raster did. Spec-honest per-ring derate brings it to 3.12 h — still
  faster than every production arm — but a WHOLE-BOARD spiral pays
  the worst bank on every ring (99.7% clamped) and still misses the
  10% bar. The measured shape of the answer is synthesis section 9's:
  the spiral wants compact sub-regions, not the whole board; its
  zero-retract continuity is real and cheap, its one-spacing-per-ring
  is the defect.

### Caveats (all pre-registered or standing)

- Harness times carry `link_ceiling: None` (fresh stock) and no
  dressups/modulation; CLI times are the full production chain. The
  table's scale column marks every row.
- The D arms' gouge check models the R1.0 tapered ball exactly
  (`TaperedBallEndmill::new(2.0, 5.7, 6.0, 20.0)`), 0 dropped points.
- Arm A's spec column is a construction citation, not a measurement;
  its 1,425 mm2 untouched finding is measured and unexplained — read
  it before adopting arm A.
- Tool wear and load are not priced: arm C spreads 184 km of cutting
  over two cutters; every whole-board arm puts its full distance on
  one R1.0 tool.
- The wanaka board is one job; nothing here re-ranks the fixture
  programme's per-shape results.

### Artifacts

- SVGs: `target/ledger_g1/wanaka_d1_spiral_honest.svg`,
  `wanaka_d2_spiral_xy.svg`, `wanaka_cal_unified.svg`.
- The arm C/A/B `simulation.json` artifacts (7 GB each) were deleted
  after extracting `toolpath_runtimes` (kept as
  `target/ledger_g/arm*_runtimes.txt`); the first arm B run failed on
  a full disk at the write step and was re-run clean.
