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

## Results

(after the runs)
