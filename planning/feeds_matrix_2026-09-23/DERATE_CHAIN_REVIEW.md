# Derate chain review for ruling R4 (2026-09-23)

This file answers the operator's R4 request: "We need a solid review on how
they combine and what the value is!" It shows every multiplication between a
published number and the feed that Suggest ships. It gives the value at each
stage for eight cells.

## 0. Method

- Machine: `MachineProfile::default()` = `generic_wood_router`
  (machine/mod.rs:272-298): spindle 8000-24000 RPM, 0.8 kW, travel 4000
  mm/min, `safety_factor` 0.75 (:293).
- Instrument tool: 2 flutes, `cutting_length = max(3 x D, 12)`,
  `stickout = cutting_length + 8` (tests/feeds_matrix_instrument_fm1.rs:174,
  :177). Workholding `Medium`. `SpindleStrategy::default()` (MatchChart).
- Materials: softwood = `GenericSoftwood` (Janka 600), hardwood =
  `GenericHardwood` (1450), plywood_hardwood = `Plywood { BalticBirch }` (1200).
- I did not run cargo. I read the code and reproduced every value by hand
  from the LUT row, the code constants and the CSV
  (`matrix_2026-09-23.csv`). All eight shipped feeds reproduce to 1 mm/min.
  All eight shipped chiploads reproduce to 4 decimals.
- "Chip" means feed / (RPM x flutes), in mm/tooth. The tables carry the chip
  and the feed side by side.
- The power check (Step 6) is not run here. The exact reproduction shows that
  it did not bind on these eight cells. The counterfactual numbers in
  section 4 assume that it stays inactive. Treat them as upper values until a
  cargo run confirms them.
- "Start" is the published number the chain begins from: the LUT row's raw
  band midpoint, or the formula `k0 D^p (1/H)^q`. Stage 0 compares that start
  with the vendor chart. The "combined multiplier" is shipped chip / start.

### The stages, in order of application

| # | Stage | Code | Acts on |
|---|---|---|---|
| S0 | Chart to start (LUT transcription, or the formula) | vendor_lut observations; feeds/mod.rs:1381 | the start number |
| S1 | Drill multiplier 2.5 (drill only) | feeds/mod.rs:1380-1386 | chip target |
| S2 | Diameter transfer `(D_query / D_row)^0.61` | feeds/vendor_lookup.rs:433, :525 | chip target and band |
| S3 | Hardness transfer `(H_row / H_query)^0.5` | feeds/vendor_lookup.rs:477, :459, :526 | chip target and band |
| S4 | Band DOC derate `doc_derating_scale` | feeds/mod.rs:1485-1497; geometry.rs:143, :232 | shown band only, not the feed |
| S5 | RPM source (vendor nominal, or 200 m/min at the engaged D) | feeds/mod.rs:1302-1307, :1580; drill tiers :1123, :1325-1329 | feed (chip unchanged) |
| S6 | Chip thinning (computed, NOT applied) | feeds/mod.rs:1841-1848; geometry.rs:112 | nothing (1.0 always) |
| S7 | Depth ladder in the feed `depth_tier_multiplier(ap, d)` | feeds/mod.rs:1855-1857; geometry.rs:261 | feed |
| S8 | L/D (stickout) derate 0.88 / 0.75 | feeds/mod.rs:1859-1877 | feed |
| S9 | Workholding 0.85 / 1.0 / 1.03 | feeds/mod.rs:1879-1886 | feed |
| S10 | Power clamp | feeds/mod.rs:1963-2237 | feed (limit) |
| S11 | Machine cutting-feed ceiling | feeds/mod.rs:2285-2299 | feed (limit) |
| S12 | Safety factor 0.75 | feeds/mod.rs:2311-2314; machine/mod.rs:293 | feed, plunge, ramp |
| S13 | Rubbing floor `min(0.025, band max)` | feeds/mod.rs:997, :1041-1046, :2394-2420 | feed (lift) |
| S14 | Drill envelope 50-400 mm/min per mm, RPM follows down (drill only) | feeds/mod.rs:2447-2490; material/mod.rs:1284-1286 | feed and RPM |
| S15 | Suggest round-down to 1 mm/min | feeds/suggest/apply.rs:95 | feed |
| S16 | Suggest pass 9 re-derive at the final depth | feeds/suggest/adaptive_entry.rs:275-282, :343 | feed (depth ladder only) |

S4 changes the band that the UI shows and the floor reads. It does not change
the chip target. The chip target is the transferred band midpoint
(vendor_lookup.rs:530).

## 1. The eight cells, stage by stage

### Cell 1. EndMill d6 Pocket softwood (VendorBacked)

CSV: RPM 17000, feed 1514, r_chip 0.0675, bounds 0.0500-0.0850, row
`amana-flat-softwood-pocket-6000-2f`. Feed per chip unit = 17000 x 2 = 34000.

| Stage | What it multiplies | Value for this cell | Where | Source | Cumulative chip / feed |
|---|---|---|---|---|---|
| Published | Amana Spektra v24, 6 mm Wood/Plywood, 1 x D | 0.127 mm/tooth | EVIDENCE 5.2-19, 3.1-1 | Amana chart | 0.127 |
| S0 | LUT row 0.050-0.085, midpoint | x0.531 (0.0675 / 0.127) | amana_flat_end.json | unsourced: band not on the cited page (5.2-19) | 0.0675 / 2295 |
| S2 | diameter 6 / 6 | x1.000 | vendor_lookup.rs:525 | partial (exponent 0.61) | 0.0675 |
| S3 | hardness 600 / 600 | x1.000 | vendor_lookup.rs:526 | partial (exponent 0.5) | 0.0675 |
| S4 | band derate, ratio d/d = 1 | x1.000 (band only) | feeds/mod.rs:1485-1497 | Onsrud rule at 1 x D | band 0.050-0.085 |
| S5 | RPM, vendor nominal | 17000 | feeds/mod.rs:1580 | row rpm_nominal | 0.0675 / 2295 |
| S7 | depth ladder, ap 4.2 / 6 = 0.70 | x1.000 | feeds/mod.rs:1855 | Onsrud/Freud/Techno/Amana (5.2-13) | 0.0675 / 2295 |
| S8 | L/D, 26 / 6 = 4.33 > 4 | x0.88 | feeds/mod.rs:1865-1877 | unsourced (5.2-14) | 0.0594 / 2019.6 |
| S9-S11 | workholding, power, ceiling 4000 | x1.000 | :1886, :1963, :2285 | machine facts | 0.0594 / 2019.6 |
| S12 | safety factor | x0.75 | feeds/mod.rs:2312 | unsourced (5.2-14) | 0.04455 / 1514.7 |
| S13 | floor, 0.04455 >= 0.025 | x1.000 (not fired) | feeds/mod.rs:2399 | unsourced (4.1-1 to 4.1-4) | 0.04455 / 1514.7 |
| S15 | round down | x0.9995 | apply.rs:95 | engine rule T-9 | 0.04453 / 1514 |
| S16 | pass 9: rigidity clamp moves dpp 4.2 to 1.2; tier 1.0 at both | x1.000 (no rescale) | adaptive_entry.rs:343 | engine rule | **0.04453 / 1514** |

Shipped 1514 = CSV 1514. Combined multiplier: 0.660. The shipped chip is 0.89 of
the shown band minimum 0.050.

### Cell 2. EndMill d3.175 Trace hardwood (FormulaOnly)

CSV: RPM 20051, feed 1002, r_chip 0.0311, no bounds, `ChiploadClampedToFloor`.
Feed per chip unit = 20051 x 2 = 40102.

| Stage | What it multiplies | Value for this cell | Where | Source | Cumulative chip / feed |
|---|---|---|---|---|---|
| Published | Onsrud 52-200 Hard Wood 1/8 in, 0.076-0.127, midpoint | 0.1016 | EVIDENCE 3.1-2 | Onsrud chart | 0.1016 |
| S0a | formula `k0 D^p` = 0.024 x 3.175^0.61 | 0.04856 (x0.478 of published) | feeds/mod.rs:1381; machine/mod.rs:42-49 | k0 uncited; p fit to a community table (6-1, 3.1-24) | 0.04856 |
| S0b | hardness `(1/H)^q`, H = (1450/600)^0.4 = 1.423, q 1.26 | x0.641 | feeds/mod.rs:1381; material/mod.rs:804 | q does not reproduce the table's hardwood column (6-4) | **start 0.03113** / 1248.3 |
| S5 | RPM, 200 m/min / (pi x 3.175) | 20051 | feeds/mod.rs:1302-1307 | repo value, not reviewed here | 0.03113 / 1248.3 |
| S7 | depth ladder, ap 0.1905 / 3.175 = 0.06 | x1.000 | feeds/mod.rs:1855 | Onsrud rule | 0.03113 / 1248.3 |
| S8 | L/D, 20 / 3.175 = 6.30 > 6 | x0.75 | feeds/mod.rs:1865-1877 | unsourced | 0.02335 / 936.2 |
| S9-S11 | workholding, power, ceiling | x1.000 | :1886, :1963, :2285 | machine facts | 0.02335 / 936.2 |
| S12 | safety factor | x0.75 | feeds/mod.rs:2312 | unsourced | 0.01751 / 702.1 |
| S13 | floor, no band, floor 0.025 | x1.428 (lift) | feeds/mod.rs:2394-2420 | unsourced | 0.02500 / 1002.55 |
| S15 | round down | x0.9995 | apply.rs:95 | engine rule | **0.02499 / 1002** |

Shipped 1002 = CSV 1002. Combined multiplier: 0.803 against the formula start
(0.515 against `k0 D^p` alone). The floor cancels both the L/D derate and the
safety factor, and 0.20 of the formula target is lost as well.

### Cell 3. BallNose d3.175 Scallop hardwood (VendorBacked, the reduced row)

CSV: RPM 17500, feed 875, r_chip 0.0220, bounds 0.0170-0.0271, row
`amana-ball-hardwood-scallop-6000-2f`, `lut_is_extrapolated true`,
`ChiploadClampedToFloor`. Feed per chip unit = 35000.

| Stage | What it multiplies | Value for this cell | Where | Source | Cumulative chip / feed |
|---|---|---|---|---|---|
| Published | Amana Spiral Ball Nose v7, Hardwood 1/8 in 0.076-0.127 (1/4 in 0.127-0.178) | 0.1016 (1/4 in: 0.1524) | EVIDENCE 4.1-8, 4.1-12 | Amana chart | 0.1016 |
| S0 | LUT row, a 6 mm row, 0.025-0.040, midpoint | x0.320 (0.0325 / 0.1016); against the printed 1/4 in midpoint 0.1524 it is x0.213 | amana_ball_nose.json | unsourced reduction "derived (reduced) for 3D-finish use" (4.1-12) | 0.0325 |
| S2 | diameter, lookup D = 3.175 (ball), row 6.0: (3.175/6)^0.61 | x0.678 | vendor_lookup.rs:433, :525 | partial | **0.02204** / 771.5 |
| S3 | hardness 1450 / 1450 | x1.000 | vendor_lookup.rs:526 | partial | 0.02204 |
| S4 | band derate, ratio 1.0 | x1.000 (band 0.0170-0.0271) | feeds/mod.rs:1485-1497 | Onsrud rule | band 0.0170-0.0271 |
| S5 | RPM, vendor nominal | 17500 | feeds/mod.rs:1580 | row rpm_nominal | 0.02204 / 771.5 |
| S7 | depth ladder, ap 0.1905 / 3.175 | x1.000 | feeds/mod.rs:1855 | Onsrud rule | 0.02204 / 771.5 |
| S8 | L/D 6.30 | x0.75 | feeds/mod.rs:1865-1877 | unsourced | 0.01653 / 578.6 |
| S9-S11 | workholding, power, ceiling | x1.000 | | machine facts | 0.01653 / 578.6 |
| S12 | safety factor | x0.75 | feeds/mod.rs:2312 | unsourced | 0.01240 / 434.0 |
| S13 | floor = min(0.025, band max 0.0271) = 0.025 | x2.016 (lift) | feeds/mod.rs:1041, :2399-2419 | unsourced | 0.02500 / 875.0 |
| S15 | round down | x1.000 | apply.rs:95 | engine rule | **0.02500 / 875** |

Shipped 875 = CSV 875. Combined multiplier: 0.769 against the raw 6 mm midpoint
(1.134 against the transferred target 0.02204). The engine uses a reduced 6 mm
row and scales it down. The chart prints a 1/8 in row directly, and that row
is 4.6 x the transferred target.

### Cell 4. TaperedBallNose d3.175 Scallop hardwood (the wanaka200 row, 6-10 / 6-11)

CSV: RPM 18500, feed 925, r_chip 0.0194, bounds 0.0129-0.0259, r_axial 0.3175,
row `amana-tapered-hardwood-scallop-3175-2f`, `ChiploadClampedToFloor`.
Instrument tool: tip 3.175, half angle 7 deg, shank 6.175. Feed per chip unit
= 37000.

| Stage | What it multiplies | Value for this cell | Where | Source | Cumulative chip / feed |
|---|---|---|---|---|---|
| Published | no row in the cited ZrN chart; nearest Onsrud 77-100 1/8 in 0.076-0.127 | 0.1016 | EVIDENCE 4.1-14, 4.1-15, 6-11 | Onsrud chart | 0.1016 |
| S0 | LUT row 0.012-0.024, midpoint | x0.177 | amana_3d_profiling.json | unsourced: no printed row (4.1-15, 6-11) | 0.0180 |
| S2 | diameter: DOC defaults to d = 3.175 (no axial hint), cone D there = 3.5887; (3.5887/3.175)^0.61 | x1.0776 | feeds/mod.rs:1293-1298; vendor_lookup.rs:525 | partial; the diameter choice is engine-only (3.5-7) | **0.01940** / 717.7 |
| S3 | hardness 1450 / 1450 | x1.000 | vendor_lookup.rs:526 | partial | 0.01940 |
| S4 | band derate, 3.175 / 3.5887 = 0.885 | x1.000 (band 0.0129-0.0259) | feeds/mod.rs:1485-1497 | Onsrud rule | band 0.0129-0.0259 |
| S5 | RPM, vendor nominal | 18500 | feeds/mod.rs:1580 | row rpm_nominal | 0.01940 / 717.7 |
| S7 | depth ladder on the TIP, 0.3175 / 3.175 | x1.000 | feeds/mod.rs:1855 | Onsrud rule | 0.01940 / 717.7 |
| S8 | L/D on the TIP, 20 / 3.175 = 6.30 | x0.75 | feeds/mod.rs:1865-1877 | unsourced | 0.01455 / 538.3 |
| S9-S11 | workholding, power, ceiling | x1.000 | | machine facts | 0.01455 / 538.3 |
| S12 | safety factor | x0.75 | feeds/mod.rs:2312 | unsourced | 0.01091 / 403.7 |
| S13 | floor = min(0.025, band max 0.02586) = 0.025 | x2.291 (lift) | feeds/mod.rs:2399-2419 | unsourced | 0.02500 / 925.0 |
| S15 | round down | x1.000 | apply.rs:95 | engine rule | **0.02500 / 925** |

Shipped 925 = CSV 925. Combined multiplier: **1.389** against the raw midpoint.
The shipped chip 0.025 is ABOVE the raw row's maximum 0.024. The default-DOC
diameter scale lifts the band to 0.0259, and then the floor lands above the
published maximum.

At the shipped depth 0.3175 mm the cone diameter is 1.905 mm. That gives a
scale of x0.732 and a band of 0.0088-0.0176. With that one diameter, the
floor caps at 0.0176 and the feed is 650 mm/min. Without the safety factor,
the L/D derate and the floor lift, the feed is 487 mm/min (chip 0.0132).

The post-simulation gate uses a third diameter. On the wanaka200 tool (6-10)
the gate scales the same row by diameter x1.006, hardness x1.099 and the depth
ladder x0.704 at the measured peak (7.004 mm on a 3.2085 mm cone). The product
is x0.779, band 0.00935-0.0187. So Suggest and the gate scale one row from
different diameters for one cut.

### Cell 5. VBit d6.35 VCarve softwood (VendorBacked)

CSV: RPM 11027, feed 625, r_chip 0.0430, bounds 0.0430-0.0430, r_axial 5.0, row
`amana-vgroove-softwood-trace-60deg-2f` (12.7 mm, 0.0762, no hardness, no RPM).
Feed per chip unit = 11027 x 2 = 22053.

| Stage | What it multiplies | Value for this cell | Where | Source | Cumulative chip / feed |
|---|---|---|---|---|---|
| Published | Amana AMS-159 60 deg 1/2 in, printed 0.003 in | 0.0762 | EVIDENCE 4.4-1 | Amana chart | 0.0762 |
| S0 | LUT row 0.0762 (single value) | x1.000 | amana_vgroove_engraving.json | transcribed | 0.0762 |
| S2 | diameter: engaged V at axial hint 5.0 = 5.774; (5.774/12.7)^0.61 | x0.618 | feeds/mod.rs:1293-1298; vendor_lookup.rs:525 | partial | 0.04711 |
| S3 | hardness: row has none, family default softwood 500 against query 600 | x0.913 | vendor_lookup.rs:459, :477, :526 | unsourced anchor; the chart prints one figure for all materials | **0.04301** / 948.5 |
| S4 | band derate, 5.0 / 5.774 = 0.866 | x1.000 (band 0.0430-0.0430) | feeds/mod.rs:1485-1497 | Onsrud rule | band 0.0430 |
| S5 | RPM: row has none, 200 m/min / (pi x 5.774) | 11027 (chart condition 18000) | feeds/mod.rs:1302-1307 | repo value, not reviewed here | 0.04301 / 948.5 |
| S7 | depth ladder, 5.0 / 6.35 = 0.79 | x1.000 | feeds/mod.rs:1855 | Onsrud rule | 0.04301 / 948.5 |
| S8 | L/D, 27.05 / 6.35 = 4.26 > 4 | x0.88 | feeds/mod.rs:1865-1877 | unsourced | 0.03785 / 834.6 |
| S9-S11 | workholding, power, ceiling | x1.000 | | machine facts | 0.03785 / 834.6 |
| S12 | safety factor | x0.75 | feeds/mod.rs:2312 | unsourced | 0.02838 / 625.9 |
| S13 | floor, 0.0284 >= 0.025 | x1.000 (not fired) | feeds/mod.rs:2399 | unsourced | 0.02838 / 625.9 |
| S15 | round down | x0.9985 | apply.rs:95 | engine rule | **0.02834 / 625** |

Shipped 625 = CSV 625. Combined multiplier: 0.372. The shipped chip is 0.66 of
the engine's own single-value band 0.0430. No diagnostic says so, because the
floor does not fire. The family default Janka for softwood is 500. The
`GenericSoftwood` query is 600. So the anchor mismatch alone costs x0.913.

### Cell 6. EndMill d6 Adaptive softwood, depth 1.5 x D (VendorBacked)

CSV: RPM 18000, feed 1559, r_chip 0.0875, bounds 0.0650-0.1100, dpp 9.0, row
`amana-flat-softwood-adaptive-6000-2f`. Feed per chip unit = 36000.

| Stage | What it multiplies | Value for this cell | Where | Source | Cumulative chip / feed |
|---|---|---|---|---|---|
| Published | Amana Spektra v24 6 mm Wood, 1 x D | 0.127 | EVIDENCE 5.2-8, 5.2-19 | Amana chart | 0.127 |
| S0 | LUT row 0.065-0.110, midpoint | x0.689 | amana_flat_end.json | unsourced: band and ap range not on the page (5.2-19) | 0.0875 / 3150 |
| S2, S3 | diameter 6/6, hardness 600/600 | x1.000 | vendor_lookup.rs:525-526 | partial | 0.0875 |
| S4 | band derate: no axial hint, ratio d/d = 1 | x1.000 (band stays raw 0.065-0.110; the gate uses x0.875 = 0.0569-0.0963) | feeds/mod.rs:1293, :1485-1497 | Onsrud rule not carried (5.2-7) | band 0.065-0.110 |
| S5 | RPM, vendor nominal | 18000 | feeds/mod.rs:1580 | row rpm_nominal | 0.0875 / 3150 |
| S7 | depth ladder, 9.0 / 6 = 1.5, step to the 2 x D point | x0.75 | feeds/mod.rs:1855; geometry.rs:261 | **sourced** at 2 x D (5.2-4); the step between points is engine choice | 0.06563 / 2362.5 |
| S8 | L/D 4.33 | x0.88 | feeds/mod.rs:1865-1877 | unsourced | 0.05775 / 2079.0 |
| S9-S11 | workholding, power, ceiling | x1.000 | | machine facts | 0.05775 / 2079.0 |
| S12 | safety factor | x0.75 | feeds/mod.rs:2312 | unsourced | 0.04331 / 1559.25 |
| S13 | floor | x1.000 (not fired) | | unsourced | 0.04331 |
| S15 | round down | x0.9998 | apply.rs:95 | engine rule | **0.04331 / 1559** |
| S16 | pass 9: dpp stays 9.0 | x1.000 | adaptive_entry.rs:343 | engine rule | 0.04331 / 1559 |

Shipped 1559 = CSV 1559. Combined multiplier: 0.495. The shipped chip is 0.67
of the shown band minimum 0.065 and 0.76 of the gate's band minimum 0.0569.

### Cell 7. BullNose d6 Profile plywood_hardwood (VendorBacked)

CSV: RPM 16000, feed 1240, r_chip 0.0587, bounds 0.0587-0.0587, dpp 1.2,
r_axial 4.8, row `idcwoodcraft-cm-14-compression` (flat compression, 6.35 mm,
0.0635, grade c, derived, no hardness). Feed per chip unit = 32000.

| Stage | What it multiplies | Value for this cell | Where | Source | Cumulative chip / feed |
|---|---|---|---|---|---|
| Published | Onsrud 52-200 Hard Plywood 1/4 in 0.152-0.203, midpoint | 0.1778 | EVIDENCE 3.3-8, 3.1-4 | Onsrud chart | 0.1778 |
| S0 | LUT row 0.0635 (IDC database, flat row used for a bull tool) | x0.357 | idcwoodcraft_millmage.json | grade c derived; no wood source for bull (3.3-16) | 0.0635 |
| S2 | diameter (6 / 6.35)^0.61 | x0.966 | vendor_lookup.rs:525 | partial | 0.06134 |
| S3 | hardness: row has none, family default 1100 against Baltic birch 1200 | x0.957 | vendor_lookup.rs:459, :526 | unsourced anchor | **0.05873** / 1879.4 |
| S4 | band derate, ratio 1.0 | x1.000 | feeds/mod.rs:1485-1497 | Onsrud rule | band 0.0587 |
| S5 | RPM, vendor nominal | 16000 | feeds/mod.rs:1580 | row rpm_nominal | 0.05873 / 1879.4 |
| S7 | depth ladder, ap 4.8 / 6 = 0.8 | x1.000 | feeds/mod.rs:1855 | Onsrud rule | 0.05873 / 1879.4 |
| S8 | L/D 4.33 | x0.88 | feeds/mod.rs:1865-1877 | unsourced | 0.05168 / 1653.8 |
| S9-S11 | workholding, power, ceiling | x1.000 | | machine facts | 0.05168 / 1653.8 |
| S12 | safety factor | x0.75 | feeds/mod.rs:2312 | unsourced | 0.03876 / 1240.4 |
| S13 | floor | x1.000 (not fired) | | unsourced | 0.03876 |
| S15 | round down | x0.9997 | apply.rs:95 | engine rule | **0.03875 / 1240** |
| S16 | pass 9: dpp 4.8 to 1.2; tier 1.0 at both | x1.000 (no rescale) | adaptive_entry.rs:343 | engine rule | 0.03875 / 1240 |

Shipped 1240 = CSV 1240. Combined multiplier: 0.610. The shipped chip is 0.66
of the engine's own single-value band 0.0587, with no diagnostic.

### Cell 8. EndMill d6 Drill softwood (FormulaOnly)

CSV: RPM 10158, feed 2400 = plunge, r_chip 0.1790, `DrillFeedClampedToEnvelope`.

| Stage | What it multiplies | Value for this cell | Where | Source | Cumulative chip / feed (RPM) |
|---|---|---|---|---|---|
| Published | Onsrud 72-000 Wood drill, 6 mm, 0.330-0.381, midpoint | 0.356 | EVIDENCE 4.3-10, 6-8 | Onsrud chart (boring drill, gang borer at 4500 RPM) | 0.356 |
| S0 | formula `k0 D^p (1/H)^q`, 0.024 x 6^0.61 x 1.0 | 0.0716 (x0.201) | feeds/mod.rs:1381 | k0 uncited (6-1) | **start 0.0716** |
| S1 | drill multiplier | x2.5 | feeds/mod.rs:1380-1386 | unsourced (6-8) | 0.1790 |
| S5 | RPM, 200 m/min / (pi x 6) = 10610; drill tier 8000-14000 does not clamp | 10610 | feeds/mod.rs:1302-1329, :1123 | tiers unsourced (4.3-14) | 0.1790 / 3798.2 (10610) |
| S7 | depth ladder, ap 0.05 | x1.000 | feeds/mod.rs:1855 | n/a for a drill | 0.1790 / 3798.2 |
| S8 | L/D 4.33 | x0.88 | feeds/mod.rs:1865-1877 | unsourced | 0.1575 / 3342.5 |
| S9-S11 | workholding, power, ceiling 4000 | x1.000 | | machine facts | 0.1575 / 3342.5 |
| S12 | safety factor | x0.75 | feeds/mod.rs:2312 | unsourced | 0.1181 / 2506.8 |
| S13 | floor | x1.000 (not fired) | | unsourced | 0.1181 |
| S14 | envelope max 400 x 6 = 2400; RPM follows down x0.957 | feed x0.957, RPM x0.957, chip x1.000 | feeds/mod.rs:2447-2490; material/mod.rs:1284-1286 | unsourced (4.3-14) | 0.1181 / 2400 (10158) |
| S15 | round down; plunge = feed | x1.000 | apply.rs:95; feeds/mod.rs:2490 | engine rule | **0.1181 / 2400 (10158)** |

Shipped 2400 at 10158 RPM = CSV. Combined multiplier: 1.650 against the formula
start (0.660 against the formula x 2.5).

## 2. Summary across the eight cells

Each column is one stage multiplier on the chip. "Floor" is the lift factor
when the floor fires. "Combined" is shipped chip / start. "Ratio" is shipped
chip / nearest published band midpoint.

| Cell | S0 chart to start | Drill x2.5 | Transfer D x H | Depth ladder | L/D | Safety | Floor lift | Round | **Combined** | Shipped chip | Published midpoint (EVIDENCE) | **Ratio** |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 EndMill d6 Pocket sw | 0.531 | - | 1.000 | 1.00 | 0.88 | 0.75 | 1.000 | 0.9995 | **0.660** | 0.0445 | 0.127 Amana 6 mm (5.2-19) | **0.351** |
| 2 EndMill d3.175 Trace hw | 0.306 (formula) | - | in start | 1.00 | 0.75 | 0.75 | 1.428 | 0.9995 | **0.803** | 0.0250 | 0.1016 Onsrud 1/8 in (3.1-2) | **0.246** |
| 3 Ball d3.175 Scallop hw | 0.320 | - | 0.678 x 1.000 | 1.00 | 0.75 | 0.75 | 2.016 | 1.000 | **0.769** | 0.0250 | 0.1016 Amana ball 1/8 in (4.1-8) | **0.246** |
| 4 TBN d3.175 Scallop hw | 0.177 | - | 1.078 x 1.000 | 1.00 | 0.75 | 0.75 | 2.291 | 1.000 | **1.389** | 0.0250 | 0.1016 Onsrud 77-100 (4.1-14) | **0.246** |
| 5 VBit d6.35 VCarve sw | 1.000 | - | 0.618 x 0.913 | 1.00 | 0.88 | 0.75 | 1.000 | 0.9985 | **0.372** | 0.0283 | 0.0762 Amana 60 deg (4.4-1) | **0.372** |
| 6 EndMill d6 Adaptive sw | 0.689 | - | 1.000 | 0.75 | 0.88 | 0.75 | 1.000 | 0.9998 | **0.495** | 0.0433 | 0.127 Amana 6 mm (5.2-8) | **0.341** |
| 7 Bull d6 Profile ply | 0.357 | - | 0.966 x 0.957 | 1.00 | 0.88 | 0.75 | 1.000 | 0.9997 | **0.610** | 0.0388 | 0.1778 Onsrud Hard Plywood (3.3-8) | **0.218** |
| 8 EndMill d6 Drill sw | 0.201 (formula) | 2.5 | 1.000 | 1.00 | 0.88 | 0.75 | 1.000 | 1.000 (envelope moves RPM, not chip) | **1.650** | 0.1181 | 0.356 Onsrud 72-000 (4.3-10) | **0.332** |

Workholding, power and the machine ceiling are 1.000 on all eight cells. They
are left out of the table for width.

What the table shows:

- The safety factor is on every cell. The L/D derate is on every cell,
  because the instrument stickout is 4.3 x D on d6 and 6.3 x D on d3.175.
  Together they give x0.66 (d6) or x0.5625 (d3.175).
- The one sourced scale, the depth ladder, acts on one cell of eight.
- On the three small-tool cells (2, 3, 4) the floor undoes the whole chain.
  The shipped chip is then 0.025 whatever the start was. On cell 4 it ships
  above the raw row's maximum.
- The largest loss in five cells is S0, the LUT row against the chart
  (x0.18 to x0.69). That loss is R5 territory, not R4. It is larger than the
  derate chain in cells 3, 4 and 7.
- End to end, every cell ships 0.22 to 0.37 of the nearest published midpoint.

## 3. Plain-language answers

### The safety factor

The safety factor multiplies every feed by 0.75 after all other steps. No
chart or book in the tree gives this number, so it is a margin the repository
chose. If it becomes 1.0, the feed rises by one third on every cell where the
floor does not fire: cell 1 goes from 1514 to 2019 mm/min. On the small
hardwood cells the feed does not change, because the floor already cancels the
factor there. The factor also multiplies the plunge, the ramp feed and the
feed ceiling (4000 x 0.75 = 3000), and Step 6 checks power at that 0.75 x
feed (feeds/mod.rs:1998), so a factor of 1.0 moves four numbers and tightens
the power check, which this review did not run.

### The L/D (stickout) derate

The L/D derate lowers the feed when the tool sticks out far from the collet.
It divides the stickout by the tip diameter. Above 4 it multiplies the feed by
0.88, and above 6 it multiplies by 0.75. No source in the code gives either
threshold or either factor. With the instrument's tools it fires on every
cell, because a 1/8 in tool with 20 mm of stickout is at 6.3.

### The rubbing floor

The rubbing floor raises the feed when the chip per tooth falls under 0.025 mm.
None of the three sources that the code cites states that number. It fires on
333 of 960 cells, because small tools start near 0.02 to 0.05 and the safety
factor with the L/D derate removes about 44 % before the floor looks. In 247 of
those cells the recipe's own target is above 0.025, so the engine's own margins
trip the engine's own floor. When it fires, it cancels those margins and it
can put the chip at or above the vendor band maximum (cell 4).

### How the depth ladder interacts with the others

The depth ladder is the one scale that three vendors print: full chip at
1 x D, 25 % less at 2 x D, 50 % less at 3 x D. The engine applies it twice, as
a step in the feed and as a straight line in the shown band. The two agree
only at the printed points (1.5 x D gives 0.75 in the feed, 0.875 in the band).
It multiplies with the safety factor and the L/D derate, so cell 6 ships
0.75 x 0.88 x 0.75 = 0.495 of its row. If that product drops under 0.025, the
floor removes the ladder along with everything else.

### Why a tapered tool gets two different diameters

It gets three diameters, not two. The L/D derate, the depth ladder, the
stepover and the depth cap use the tip diameter (3.175 mm). Suggest picks the
band and the RPM at the cone diameter at a depth equal to the tip diameter
(3.59 mm), because Scallop gives no depth hint. The post-simulation gate uses
the cone diameter at the measured peak depth. The recipe itself cuts 0.3175 mm
deep, where the engaged diameter is only 1.9 mm.

The R4 one-diameter rule should pick the cone diameter at the shipped depth, in
Suggest and in the gate. The gate then applies the same rule to its measured
depth, so any difference is a depth finding and not a diameter finding. For
cell 4 that choice moves the band from 0.0129-0.0259 to 0.0088-0.0176. The
shipped feed moves from 925 to 650 mm/min with today's floor, and to 487 mm/min
with no safety factor, no L/D derate and no floor lift. These figures assume
that the same row still matches at the 1.905 mm query; the lookup score can
pick another row, and the RPM anchor can change with it.

## 4. Recommendation

"Number that moves" gives the shipped feed now and after the change, all other
stages held. The numbers assume that the power check stays inactive.

| Stage | Keep as scale / keep as limit / flag only / delete | Reason | Number that moves (cell) | Basis |
|---|---|---|---|---|
| S0 LUT band | Keep as scale, only where the row matches its printed page | The row is the vendor's own number when it is transcribed. Five of eight start rows are not (R5). | Cell 3: a printed 1/8 in row (mid 0.1016) in place of the reduced 6 mm row moves the target from 0.0220 to 0.1016 | published source |
| S0 formula `k0 D^p (1/H)^q` | Keep as scale with the `FormulaOnly` label (R1 ruled yes) | It is the only number where no row exists | Cell 2 start 0.0311 | judgement (R1) |
| S1 drill x2.5 | Flag only ("unsourced drill multiplier" Caution); keep the value until a source exists | No publisher gives a drill-to-mill ratio (6-8) | Cell 8 target 0.179; at x1.0, 0.0716 | judgement |
| S2 diameter transfer ^0.61 | Keep as scale; flag when `is_extrapolated`; prefer a printed row at the tool's diameter | The exponent is fit to a community table, not printed by a vendor | Cell 3 x0.678; cell 5 x0.618 | partial source |
| S3 hardness transfer ^0.5 | Keep as scale across families only; x1.0 inside a family that the chart does not split | Vendors print one band for hard and soft wood (q near 0.08); 0.5 is a physics bracket | Cell 5 x0.913 (625 to 685); cell 7 x0.957 (1240 to 1295) | partial source |
| S3 family default Janka | Delete the mismatch: use the query family's own anchor, or x1.0 when the row has no hardness | Softwood default 500 disagrees with `GenericSoftwood` 600 | Cell 5: 625 to 685 | judgement (engine-versus-engine) |
| S4 band DOC derate | Keep as scale, one implementation with S7, at the depth that ships | Same published rule as S7; today the band ignores depth on roughing ops (5.2-7) | Cell 6 shown band 0.065-0.110 to 0.049-0.083 (step at 1.5 x D) | published source (R3) |
| S5 RPM | Keep; out of R4 scope | Moves the feed, not the chip | Cell 5 RPM 11027 against the chart's 18000 | not reviewed |
| S6 chip thinning | Keep as today (computed, not applied) | No wood source conditions a band on ae (feeds/mod.rs:1787-1848) | none (1.0) | published absence |
| S7 depth ladder | **Keep as scale** | The only derate that three vendors print (5.2-4, 5.2-17) | Cell 6 x0.75 (2079 without the safety factor, 2362 without both margins) | published source |
| S8 L/D | Flag only ("long stickout" Caution) | Unsourced thresholds and factors (5.2-14) | Cell 1: 1514 to 1721; cell 6: 1559 to 1771; cells 2-4 unchanged (floor) | judgement |
| S9 workholding | Flag only | Unsourced; 1.0 on all eight cells | none at Medium | judgement |
| S10 power | Keep as limit, stated in the result | Machine fact | none on these cells | machine fact |
| S11 feed ceiling | Keep as limit, stated in the result | Machine fact | none on these cells | machine fact |
| S12 safety factor | Flag only: a visible operator margin, labelled "repo margin, unsourced"; default shown to the operator | No source (5.2-14); it trips the floor in 247 cells (4.1-5) | Cell 1: 1514 to 2019; cell 7: 1240 to 1653; cell 8: RPM 10158 to 8000 at 2400 | judgement |
| S13 rubbing floor | Flag only: Caution, no clamp | No source (4.1-1 to 4.1-4); a vendor publishes lower bands (4.1-26) | Cell 3: 875 to 433 with both margins, 771 without them; cell 4: 925 to 403 / 717 | published absence |
| S13 band-max cap on the floor | Delete with the clamp | It lifts a mid-band recipe to the band maximum (4.1-6) | Cell 4: ships above the raw row maximum today | engine-versus-engine |
| S14 drill envelope and RPM tiers | Flag only, named unsourced in the drill provenance; keep the RPM follow-down | No source (4.3-14) | Cell 8: feed 2507 to 2400, RPM 10610 to 10158, chip unchanged | judgement |
| S15 round down | Keep | 1 mm/min, and it protects the ceilings (T-9) | x0.998 or more | engine rule |
| S16 pass 9 re-derive | Keep | It reapplies S7 at the shipped depth | none on these cells | engine rule |
| Tapered diameter | One diameter: the cone diameter at the shipped depth, in Suggest and the gate | 3.5-7, 6-11 | Cell 4: band 0.0129-0.0259 to 0.0088-0.0176; feed 925 to 650 | judgement (R4 text) |

The R4 end state is: row plus depth ladder, both margins as flags, the floor as
a Caution. The shipped feeds would then be:

| Cell | Now | R4 end state | Chip now to R4 end |
|---|---|---|---|
| 1 | 1514 | 2295 | 0.0445 to 0.0675 |
| 2 | 1002 | 1248 | 0.0250 to 0.0311 |
| 3 | 875 | 771 | 0.0250 to 0.0220 (Caution still fires) |
| 4 | 925 | 717 (487 at the one-diameter rule) | 0.0250 to 0.0194 (0.0132) |
| 5 | 625 | 948 | 0.0283 to 0.0430 |
| 6 | 1559 | 2362 | 0.0433 to 0.0656 |
| 7 | 1240 | 1879 | 0.0388 to 0.0587 |
| 8 | 2400 at 10158 RPM | 2400 at 8000 RPM | 0.1181 to 0.1500 |

Even the R4 end state ships 0.13 to 0.56 of the nearest published midpoint.
The derate chain is not the largest gap. The LUT rows (S0) are, and R5 owns
them.

## 5. Gaps

- The power model (Step 6) and pass 10 were not run. The counterfactual feeds
  are upper values until a cargo run confirms them.
- The 200 m/min base cutting speed and the drill RPM tiers set the RPM. This
  review did not check their sources.
- The published midpoints are the nearest printed rows. They are 1 x D
  conditions. For the finish cells (2, 3, 4, 5) no vendor prints a
  light-finish chip load, so the ratio is not like-for-like (3.2-14).
