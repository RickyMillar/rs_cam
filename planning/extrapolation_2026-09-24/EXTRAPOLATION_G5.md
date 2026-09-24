# G5: Engaged geometry (V-bit width, tapered cone, bull corner)

Status: Phase 2 (trend) done 2026-09-24; Phase 3 (fit, witness) not started; nothing lands before the Phase 4 rulings.

Inputs: the LUT (`crates/rs_cam_core/data/vendor_lut/observations/*.json`),
`fetch/G5/verified_rows.json` (171 rows, written by
`scripts/g5_verified_rows.py`), `inventory_cells.csv` and the FM1 matrix
`planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv`. The tables in §1
are the output of `scripts/trend_g5.py` (read-only). Run it again after a LUT
or matrix change:

```
python3 planning/extrapolation_2026-09-24/scripts/trend_g5.py
```

Marks: **printed** = the words or numbers are on the stored document.
**derived** = this record computed or inferred it.

## 0. The gap

### 0.1 The cells (T8)

All 24 G5 cells are V-bit cells: 60° included angle, 2 flutes, the FM1
instrument tool (`tests/feeds_matrix_instrument_fm1.rs`, `tool_of`).

| Sub-class | D mm | Operations | Materials | Cells | State |
|---|---|---|---|---|---|
| V-bit parallel finish at the engaged width | 6.35, 12.7 | DropCutter, RampFinish, RadialFinish, HorizontalFinish (4 each) | softwood 8, hardwood 8 | 16 | refuse |
| V-bit RPM-only anchor, chipload from the formula | 6.35 (7), 12.7 (1) | Chamfer 2, Inlay 2, Trace 2, VCarve 2 | hardwood 5, softwood 3 | 8 | ship, bandless |

No tapered-ball or bull-nose cell is in G5 in the inventory. PLAN §2 says
"every tapered cell is read at the cone". That is an engine rule on cells
that ship (118 tapered cells ship; §1.6). It is not a refusal. The bull-nose
refusals (48) are in G3.

### 0.2 The engine rules that serve or refuse these cells

- **Refusal.** `feeds::support::formula_backing` (`crates/rs_cam_core/src/feeds/support.rs`)
  returns `Clueless { reason: VBIT_PARALLEL }` for
  `(ChamferVbit, Parallel, Finish)` in softwood and hardwood. The text is
  "No published figure backs the formula for a V-bit on parallel finish
  passes: at the engaged width the chip is below half of every V-bit
  figure."
- **The lookup diameter.** `vendor_normalize::lookup_diameter_for_input`
  (`feeds/vendor_normalize.rs`) calls
  `ToolGeometryHint::engaged_diameter_at_doc` (`feeds/mod.rs`) at the op's
  axial hint. For a V-bit that is `w = 2 · ap · tan(angle / 2)`, clamped to
  D. An op with no axial hint uses `ap = D`, so its width clamps to D and
  the lookup keys at the nominal diameter. The ops with a hint (`VCarve`,
  `RampFinish`, `Waterline`, `SteepShallow`; `compute/catalog.rs`
  `feeds_hints`) key at the engaged width. `feeds::calculate` uses the same
  diameter for the SFM RPM and for the formula chipload.
- **The row scale.** `vendor_lookup::build_result` multiplies a row by
  `(d_lookup / d_row)^0.61` (`CHIPLOAD_DIAMETER_EXPONENT`). A row with no
  `diameter_mm` gets a scale of 1.0 at any width.
- **The feed ladder and the depth cap.** `feeds::geometry::feed_ladder_diameter_mm`
  and `depth_cap_diameter_mm` (`feeds/geometry.rs`) use the nominal
  diameter for a V-bit. Their doc comments say "The band de-rates a V-bit at
  its engaged width" (R2 did not rule on V-bit feeds).
- **The band de-rate.** `feeds::calculate` de-rates the band by
  `doc_derating_scale(ap / d_band)`, with `d_band` the engaged width at the
  shipped depth.
- **The 8 shipping cells.** They match `whiteside-1540-vgroove-60deg-quarter-rpm`,
  a row that prints RPM only. The chipload comes from the formula
  (`feeds.vendor_row_publishes_no_chipload`, `feeds/mod.rs`). The 12.7 mm
  VCarve hardwood cell also takes this 1/4 in row's RPM.
- **The same tool, two keys.** EVIDENCE 3.4-13 (feeds matrix): one 6.35 mm
  60° V-bit in one Finish role got 0.0741 mm/tooth on DropCutter (no hint)
  and 0.0172 on RampFinish (hint 0.5 mm), a ratio of 4.3. Only the op wiring
  sets the key.

## 1. The trend

### 1.1 Row counts (T0)

| origin | group | grade | kind | rows |
|---|---|---|---|---|
| G5 verified | amana_insert_v_groove_v16 | a | chipload | 57 |
| G5 verified | amana_insert_v_groove_v16 | b | chipload | 48 |
| G5 verified | onsrud_{hard_wood, soft_wood, mdf, hard_plywood, soft_plywood}_cutting_data | a | chipload | 12 each, 60 |
| G5 verified | amana_15_60_90_vgroove_engraving_2f | b | chipload | 6 |
| LUT | on document | wood | chipload | 16 |
| LUT | on document | wood | RPM only | 2 |
| LUT | on document | non-wood | chipload | 3 |
| LUT | not on cited document (D1, D2) | wood | chipload | 7 |
| LUT | not on cited document (D1, D2) | non-wood | chipload | 1 |

Verified: 171 of 171 candidate rows. No row was "wrong", "not_found" or
"grade_wrong". The eight LUT rows that are not on their cited document (T5)
are left out of every value, ratio and slope below.

### 1.2 The diameter each V-bit chart keys on (T1, wood rows)

The five Onsrud sheets print the same 37-series cells (T2 check: identical
on all five sheets). The table shows the hard-wood sheet once for each
series.

| origin | source | subfamily | rows | angles | flutes | diameter key | tip mm | chip mm/tooth | grade | depth rule (printed) |
|---|---|---|---|---|---|---|---|---|---|---|
| G5 | amana_insert_v_groove_v16 | insert_vgroove | 105 | 40-160 (17 angles) | 1, 2 | none printed | - | 0.0610; MDF 0.1194 / 0.1219 | a, b | none printed |
| G5 | amana_15_60_90_vgroove_engraving_2f | 2f engraving | 6 | 15, 60, 90 | 2 | none printed | - | 0.0762-0.1778 (unit ambiguous) | b | 1 x Tool Diameter |
| G5 | onsrud (x5) | 37-00 (60°) | 1 per sheet | 60 | 1 | 6.35 (1/4 column = shank) | - | 0.1016-0.1524 | a | Cut: Varies |
| G5 | onsrud (x5) | 37-20 (30°) | 1 per sheet | 30 | 1 | 6.35 (same cell) | - | 0.1016-0.1524 | a | Cut: Varies |
| G5 | onsrud (x5) | 37-50 V-bottom SC | 3 per sheet | 90 | 2 | 4.76, 6.35, 9.53 | - | 0.0762-0.1524 | a | 1/2 CED (HW, MDF) / 1/2 x D (SW, plywoods) |
| G5 | onsrud (x5) | 37-60 V-bottom CT | 4 per sheet | 90 | 2 | 9.53, 12.7, 19.05, 25.4 | - | 0.1016-0.1524 to 0.2032-0.2540 | a | same |
| G5 | onsrud (x5) | 37-80 lettering CT | 3 per sheet | not printed | 2 | 25.4, 31.75, 50.8 | - | 0.1016-0.1524 | a | Cut: Varies |
| LUT | amana_ams159_vgroove_v2 | carbide_tipped_vgroove | 3 | 60, 90 | 2 | 9.525, 12.7 (SKU, not on chart) | - | 0.0762 | a | 1xD |
| LUT | amana_ams159_vgroove_v2 | conical_engraving | 4 | 30, 45 | 1 | 6.35 (SKU) | - | 0.0762-0.1778 | a | 1xD |
| LUT | amana_ams159_vgroove_v2 | solid_carbide_vgroove | 2 | 18 | 1 | 6.35 (SKU) | - | 0.0762-0.1778 | a | 1xD |
| LUT | amana_spektra_engraving_v4 | spektra_engraving | 7 | 15, 30, 45, 120 | 1 | 6.35 or none (SKU) | 0.127, 0.381, 1.067 | 0.0508-0.1524; 0.0762-0.1778 | a | 1xD |

What the charts key on:

- **Onsrud** keys the chip load on the printed "Cutting Diameter" (the top of
  the V). On the 90° tools the printed cut is "1/2 CED". At that depth the
  engaged width equals the cutting diameter: `w = 2 · (D/2) · tan 45° = D`
  (derived). So the Onsrud 90° value is the value at full cone engagement.
- **Amana** keys the chip load on the angle and the tool number only. No
  Amana chart prints a diameter column. The LUT `diameter_mm` on the AMS-159
  and Spektra rows comes from the SKU, not from the chart.
- **No chart prints a value at an engaged width**, and no chart prints a
  scale from the top diameter to the engaged width.

### 1.3 Size trend: the Onsrud 37-series (T2)

| series | D mm | D in | chip mm/tooth | mid | mid / D % | log slope vs previous | log slope vs first |
|---|---|---|---|---|---|---|---|
| 37-00 (60°) | 6.35 | 0.25 | 0.1016-0.1524 | 0.1270 | 2.00 | - | - |
| 37-20 (30°) | 6.35 | 0.25 | 0.1016-0.1524 | 0.1270 | 2.00 | - | - |
| 37-50 | 4.7625 | 0.1875 | 0.0762-0.1524 | 0.1143 | 2.40 | - | - |
| 37-50 | 6.35 | 0.25 | 0.0762-0.1524 | 0.1143 | 1.80 | 0.00 | 0.00 |
| 37-50 | 9.525 | 0.375 | 0.0762-0.1524 | 0.1143 | 1.20 | 0.00 | 0.00 |
| 37-60 | 9.525 | 0.375 | 0.1016-0.1524 | 0.1270 | 1.33 | - | - |
| 37-60 | 12.7 | 0.5 | 0.1016-0.1524 | 0.1270 | 1.00 | 0.00 | 0.00 |
| 37-60 | 19.05 | 0.75 | 0.1524-0.2032 | 0.1778 | 0.93 | 0.83 | 0.49 |
| 37-60 | 25.4 | 1 | 0.2032-0.2540 | 0.2286 | 0.90 | 0.87 | 0.60 |
| 37-80 | 25.4 | 1 | 0.1016-0.1524 | 0.1270 | 0.50 | - | - |
| 37-80 | 31.75 | 1.25 | 0.1016-0.1524 | 0.1270 | 0.40 | 0.00 | 0.00 |
| 37-80 | 50.8 | 2 | 0.1016-0.1524 | 0.1270 | 0.25 | 0.00 | 0.00 |

- The 37-60 midpoint rises with the cutting diameter. The endpoint log
  slope is 0 (3/8 to 1/2 in), 0.60 (3/8 to 1 in) or 0.85 (1/2 to 1 in).
  Four points, two of them equal. It is not a fit.
- 37-50 (3/16 to 3/8 in) and 37-80 (1 to 2 in) are flat: slope 0.
- At one cutting diameter (1 in) the series differ by 1.80x (37-60 against
  37-80). The series matters more than the size.
- Chip / D falls from 2.4 % to 0.25 % across the 37-series. The chip does
  not scale with D.

### 1.4 Angle, flute and material trend on the Amana charts (T3, T4)

Amana insert V-groove v16, all 21 (angle, flutes, RPM) groups:

| angle | flutes | RPM | hardwood | softwood | plywood (derived b) | MDF | MDF / wood |
|---|---|---|---|---|---|---|---|
| 40, 45, 46, 50, 60, 70, 72, 90, 91, 100, 110 | 1 | 18000 | 0.0610 | 0.0610 | 0.0610 | 0.1194 | 1.96 |
| 120 | 1 | 14000 | 0.0610 | 0.0610 | 0.0610 | 0.1219 | 2.00 |
| 120 | 2 | 15000 | 0.0610 | 0.0610 | 0.0610 | 0.1194 | 1.96 |
| 120, 130, 135, 140, 150, 160 | 2 | 18000 | 0.0610 | 0.0610 | 0.0610 | 0.1219 | 2.00 |
| 140 | 2 | 16000 | 0.0610 | 0.0610 | 0.0610 | 0.1219 | 2.00 |
| 150 | 1 | 14000 | 0.0610 | 0.0610 | 0.0610 | 0.1219 | 2.00 |

(The script prints one line per group; the rows above merge equal lines.)

The other Amana wood rows (T3, second table):

| source | angles | flutes | diameter_mm (row) | tip mm | chip mm/tooth | grade |
|---|---|---|---|---|---|---|
| AMS-159 | 18, 30, 45 | 1 | 6.35 (SKU) | - | 0.0762-0.1778 | a |
| AMS-159 | 60 | 2 | 12.7 (SKU) | - | 0.0762 | a |
| AMS-159 | 90 | 2 | 9.525 (SKU) | - | 0.0762 | a |
| Spektra engraving | 15, 30, 45 | 1 | 6.35 or none | 0.127, -, 1.067 | 0.0762-0.1778 | a |
| Spektra engraving | 120 | 1 | 6.35 | 0.381 | 0.0508-0.1524 | a |
| 2F engraving (G5) | 15, 60, 90 | 2 | none | - | 0.0762-0.1778 | b |

Material ratio on one printed cell (T4):

| vendor chart | cells | MDF / hardwood | plywood / hardwood |
|---|---|---|---|
| Onsrud 37-series (5 sheets) | 12 | 1.00 | 1.00 |
| Amana insert V-groove v16 | 21 | 1.96, 2.00 | 1.00 (one shared column) |

- The Amana chip load does not change with the angle (40° to 160°), with
  the flute count (1 or 2) or with the RPM (14,000 to 18,000).
- Two vendors disagree by 2x on MDF against wood for one tool class. This
  is G2 material; G5 records it and does not resolve it.
- The 2F engraving chart's value is per revolution by its own feed. On two
  flutes the per-tooth value is half (0.0381-0.0889 mm). No slope or ratio
  here uses it.

### 1.5 The depth ladder on a V-bit is set by the angle (T7)

For a pointed V, `ap / w = 1 / (2 · tan(angle / 2))`. The depth does not
change it (derived).

| included angle deg | ap / w | doc_derating_scale |
|---|---|---|
| 15 | 3.798 | 0.500 |
| 18 | 3.157 | 0.500 |
| 30 | 1.866 | 0.783 |
| 40 | 1.374 | 0.907 |
| 45 | 1.207 | 0.948 |
| 53.13 | 1.000 | 1.000 |
| 60 | 0.866 | 1.000 |
| 90 | 0.500 | 1.000 |
| 120 | 0.289 | 1.000 |
| 150 | 0.134 | 1.000 |

- At 53.13° and above, the band de-rate "at the engaged width" is always
  1.0. It has no effect on any cell in the matrix (60°).
- Below 53.13° it de-rates the band by the angle alone: 0.50 on every
  15° or 18° cut at any depth. The Amana charts for these angles print
  "Depth of Cut: 1 x Tool Diameter". If D is the tool's diameter, a shallow
  engraving cut is inside 1 x D and the chart asks for no de-rate. The
  engine reading and the chart reading disagree for narrow angles
  (derived; the chart does not say which D).
- `engaged_diameter_at_doc` for a V-bit ignores `tip_diameter`. The
  PreciseBits width formula adds the tip ("(TAN(Half Angle) × 2 × Cutting
  Depth) + (Runout + Tip Diameter)", printed). On a flat-tip engraving tool
  the engine width is too small by the tip (derived observation, not a
  fix).

### 1.6 Tapered ball and bull nose: the diameter each rule keys on (T6, T10)

| family | source | grade/kind | wood rows | diameter_mm | tip_diameter_mm | depth rule |
|---|---|---|---|---|---|---|
| tapered_ball_nose | onsrud_{hard_wood, soft_wood, mdf, hard_plywood} | a/exact | 8 each | 3.175, 6.35 | - | cut depth per pass = cutting edge diameter (1xD); 2xD reduce 25 %; 3xD reduce 50 % |
| tapered_ball_nose | amana_zrn_3d_profiling | c/derived | 8 | 3.175, 6 | - | "3d profiling finish" (not printed) |
| tapered_ball_nose | whiteside_fusion360_tools_2019-10-23 | c/fallback | 2 | 1.442, 2.9867 | 1.5875, 3.175 | - |
| bull_nose | onsrud_cutting_data_recommendations | c/derived | 3 | 6 | - | 0.3-1.25 x D (not printed) |

- **Tapered ball: every printed key is the tip.** The Onsrud 77-100 row
  keys on the "cutting diameter" (1/8 in, 1/4 in); the LUT note reads it as
  the ball tip. PreciseBits keys the depth per pass on the tip:
  "recommended - 1X tip dia. / maximum - 2X tip dia." (printed; no chip
  load). No chart keys a chip load on the engaged cone.
- **The engine keys on the cone.** After R2 the lookup, the feed ladder and
  the band use the engaged diameter at the depth. On the matrix tool (7°
  half angle):

| tip mm | ap case | ap mm | engaged d mm | engaged / tip | row scale (d/tip)^0.61 |
|---|---|---|---|---|---|
| 3.175 | 0.5 (RampFinish default) | 0.500 | 2.313 | 0.729 | 0.824 |
| 3.175 | 1 x tip (Onsrud "1xD") | 3.175 | 3.589 | 1.130 | 1.078 |
| 3.175 | 2 x tip (PreciseBits maximum) | 6.350 | 4.368 | 1.376 | 1.215 |
| 6 | 0.5 (RampFinish default) | 0.500 | 3.317 | 0.553 | 0.697 |
| 6 | 1 x tip (Onsrud "1xD") | 6.000 | 6.782 | 1.130 | 1.078 |
| 6 | 2 x tip (PreciseBits maximum) | 12.000 | 8.255 | 1.376 | 1.215 |

  At the printed condition (1 x tip) the cone key moves the tip row by
  x1.078 on this taper. The matrix shows it (Pocket sends no hint, so the
  lookup depth defaults to the tip diameter): the 3.175 mm Pocket cells ship
  0.1095 mm/tooth from a row with a 0.1016 midpoint. 118 tapered cells
  ship; 1 is flagged extrapolated. Over the depths above the cone key moves
  the tip row by -30 % to +22 %. It depends on the taper angle.
- **Bull nose: no printed row.** The three wood bull rows are `derived c`
  and cite a sheet that prints no bull series (R5, EVIDENCE 3.3-16/17). The
  PreciseBits MM208 page prints corner radii only. No wood vendor keys a
  bull-nose chip load on the full diameter or on an effective diameter.

### 1.7 Engine side: the 24 G5 cells (T9)

Method (derived). The formula chip at nominal D is the matrix Pocket value
for the same D and material (Pocket sends no hint, so it keys at D; the same
value as the no-hint parallel ops had before R1). The formula chip at width
`w` is that value times `(w / D)^0.61`. The check against the matrix:
VCarve 5.0 mm gives 0.0448 (matrix 0.0448); RampFinish 0.5 mm gives 0.0172
softwood (EVIDENCE 3.4-13: 0.0172). Values are before the hardness scale
on the vendor columns.

Depth cases. The 16 refused cells print no depth. Each case names its
source:

- 0.06 x D: the r_axial that the same op ships on the ball nose in the
  matrix (0.1905 at 3.175, 0.36 at 6.0). The shipped V-bit Trace family
  cells print the same 0.06 x D.
- 0.2 x D: the rigidity cap depth that EVIDENCE 3.4-6 records (1.27 / 2.54 mm).
- 0.5 mm: the RampFinish `max_stepdown` default (`compute/operation_configs.rs`).

Anchors: AMS-159 60° 2F row 0.0762 at `diameter_mm` 12.7 (SKU); the
insert chart 60° 1F 0.061 with no diameter (scale 1.0 at any width). The
last column tests the at-w formula against half of the lowest printed 60°
wood figure (0.5 x 0.061 = 0.0305).

| D | op | material | state | depth case (source) | ap mm | w at ap mm | w / D | w the lookup uses | formula chip, engine key | formula chip at w | engine / at-w | AMS-159 row at nominal D | AMS-159 row at w | insert row (no diameter) | at-w < 0.5 x lowest 60° figure |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 6.35 | VCarve | hardwood | ok | r_axial 5 (matrix); hinted | 5.000 | 5.774 | 0.909 | 5.774 | 0.0448 | 0.0448 | 1.00 | 0.0499 | 0.0471 | 0.0610 | no |
| 6.35 | Inlay | softwood | ok | r_axial 0.381 (matrix); no hint, lookup at D | 0.381 | 0.440 | 0.069 | 6.350 | 0.0741 | 0.0145 | 5.10 | 0.0499 | 0.0098 | 0.0610 | yes |
| 6.35 | Inlay | hardwood | ok | same | 0.381 | 0.440 | 0.069 | 6.350 | 0.0475 | 0.0093 | 5.10 | 0.0499 | 0.0098 | 0.0610 | yes |
| 6.35 | Trace | softwood | ok | same | 0.381 | 0.440 | 0.069 | 6.350 | 0.0741 | 0.0145 | 5.10 | 0.0499 | 0.0098 | 0.0610 | yes |
| 6.35 | Trace | hardwood | ok | same | 0.381 | 0.440 | 0.069 | 6.350 | 0.0475 | 0.0093 | 5.10 | 0.0499 | 0.0098 | 0.0610 | yes |
| 6.35 | Chamfer | softwood | ok | same | 0.381 | 0.440 | 0.069 | 6.350 | 0.0741 | 0.0145 | 5.10 | 0.0499 | 0.0098 | 0.0610 | yes |
| 6.35 | Chamfer | hardwood | ok | same | 0.381 | 0.440 | 0.069 | 6.350 | 0.0475 | 0.0093 | 5.10 | 0.0499 | 0.0098 | 0.0610 | yes |
| 6.35 | DropCutter, RadialFinish, HorizontalFinish | softwood | refused | 0.06 x D | 0.381 | 0.440 | 0.069 | 6.350 | 0.0741 | 0.0145 | 5.10 | 0.0499 | 0.0098 | 0.0610 | yes |
| 6.35 | DropCutter, RadialFinish, HorizontalFinish | softwood | refused | 0.2 x D | 1.270 | 1.466 | 0.231 | 6.350 | 0.0741 | 0.0303 | 2.44 | 0.0499 | 0.0204 | 0.0610 | yes |
| 6.35 | DropCutter, RadialFinish, HorizontalFinish | hardwood | refused | 0.06 x D | 0.381 | 0.440 | 0.069 | 6.350 | 0.0475 | 0.0093 | 5.10 | 0.0499 | 0.0098 | 0.0610 | yes |
| 6.35 | DropCutter, RadialFinish, HorizontalFinish | hardwood | refused | 0.2 x D | 1.270 | 1.466 | 0.231 | 6.350 | 0.0475 | 0.0194 | 2.44 | 0.0499 | 0.0204 | 0.0610 | yes |
| 6.35 | RampFinish | softwood | refused | 0.5 max_stepdown | 0.500 | 0.577 | 0.091 | 0.577 | 0.0172 | 0.0172 | 1.00 | 0.0499 | 0.0116 | 0.0610 | yes |
| 6.35 | RampFinish | hardwood | refused | 0.5 max_stepdown | 0.500 | 0.577 | 0.091 | 0.577 | 0.0110 | 0.0110 | 1.00 | 0.0499 | 0.0116 | 0.0610 | yes |
| 12.7 | VCarve | hardwood | ok | r_axial 5 (matrix); hinted | 5.000 | 5.774 | 0.455 | 5.774 | 0.0448 | 0.0448 | 1.00 | 0.0762 | 0.0471 | 0.0610 | no |
| 12.7 | DropCutter, RadialFinish, HorizontalFinish | softwood | refused | 0.06 x D | 0.762 | 0.880 | 0.069 | 12.700 | 0.1131 | 0.0222 | 5.10 | 0.0762 | 0.0150 | 0.0610 | yes |
| 12.7 | DropCutter, RadialFinish, HorizontalFinish | softwood | refused | 0.2 x D | 2.540 | 2.933 | 0.231 | 12.700 | 0.1131 | 0.0463 | 2.44 | 0.0762 | 0.0312 | 0.0610 | no |
| 12.7 | DropCutter, RadialFinish, HorizontalFinish | hardwood | refused | 0.06 x D | 0.762 | 0.880 | 0.069 | 12.700 | 0.0725 | 0.0142 | 5.10 | 0.0762 | 0.0150 | 0.0610 | yes |
| 12.7 | DropCutter, RadialFinish, HorizontalFinish | hardwood | refused | 0.2 x D | 2.540 | 2.933 | 0.231 | 12.700 | 0.0725 | 0.0297 | 2.44 | 0.0762 | 0.0312 | 0.0610 | yes |
| 12.7 | RampFinish | softwood | refused | 0.5 max_stepdown | 0.500 | 0.577 | 0.045 | 0.577 | 0.0172 | 0.0172 | 1.00 | 0.0762 | 0.0116 | 0.0610 | yes |
| 12.7 | RampFinish | hardwood | refused | 0.5 max_stepdown | 0.500 | 0.577 | 0.045 | 0.577 | 0.0110 | 0.0110 | 1.00 | 0.0762 | 0.0116 | 0.0610 | yes |

(The script prints one line per op; the rows above merge the three no-hint
parallel ops, which give equal values.)

What the table shows:

- On a V-bit finish pass the engaged width is 7 % to 23 % of the nominal
  diameter at 0.06 x D to 0.2 x D, and 4.5 % to 9 % on RampFinish at 0.5 mm.
- The formula keyed at the nominal diameter gives 2.44x (0.2 x D) to 5.10x
  (0.06 x D) the formula keyed at the engaged width. The ratio is
  `(D / w)^0.61` and depends only on `ap / D` at a fixed angle.
- **The 6 shipping Trace, Inlay and Chamfer cells at 6.35 mm** ship the
  nominal-key chip (0.0741 softwood, 0.0475 hardwood) at a shipped depth of
  0.381 mm, where the width is 0.44 mm. At that width the same formula
  gives 0.0145 / 0.0093, 5.10x less. The key comes from the missing axial
  hint, not from a rule.
- At the engaged width, the formula chip is below half of the lowest
  printed 60° wood figure in 25 of 28 refused-cell depth cases. The three
  exceptions are the 12.7 mm softwood no-hint ops at 0.2 x D (0.0463). So
  the refusal text holds on this table for the engaged-width key.
- At the nominal key, the formula chip (0.0475 to 0.1131) sits inside 0.5x
  to 2x of the printed 60° figures (0.061, 0.0762). The R1 test passes or
  fails only by the choice of key.
- An anchorless row (the insert chart) ships its printed value at any
  width. A row with a SKU diameter (AMS-159) is scaled by `(w / 12.7)^0.61`
  to 0.0098-0.0471. One printed class of number, two outcomes, set by
  whether the LUT row carries a diameter that the chart does not print.

### 1.8 What the data does NOT show

- No printed chip load at an engaged width, and no printed scale from the
  top diameter to the width. The `(w / D)^0.61` column is the repo's size
  law applied to a width. No chart supports it.
- No V-bit row for a 3D parallel, radial, horizontal or ramp finish pass.
  Every printed V-bit row is a groove, trace or engraving cell.
- No printed V-bit value at a depth under the full cone (Onsrud "1/2 CED")
  or for a condition other than "1 x Tool Diameter" (Amana), except "Cut:
  Varies" on 37-00/37-20/37-80, which names no depth.
- No V-bit size law. The only series is 37-60 (four points, slopes 0 to
  0.87). Amana prints no size.
- No tapered-ball chip load keyed on the cone, and no bull-nose wood chip
  load at all.
- No trend across the angle: Amana prints one value for 40° to 160°.

## 2. Sources

Verdicts: the verifiers re-downloaded each verified source and compared the
text byte for byte. "Not verified" sources carry no candidate row.

| id | vendor | what it prints | grade | URL reachable | sha256 (first 12) | verifier verdicts |
|---|---|---|---|---|---|---|
| amana_insert_v_groove_v16 | Amana | one chip load per tool number and angle (40-160°), 1 or 2 flutes; wood .0024 in, MDF .0047/.0048 in; no diameter, no depth; footnote RC-1142 hobby machines 12,000 RPM, 30 % slower feed | a (b: plywood split, RC-1146, RC-1101) | yes (verifier) | d3da2d5fd7b6 | 105 confirmed |
| onsrud_hard_wood_cutting_data | Onsrud | 37-series chip load by cutting diameter; Cut 1/2 CED or Varies; 1xD / 2xD / 3xD header | a | yes (verifier) | f123d5f67c03 | 12 confirmed |
| onsrud_soft_wood_cutting_data | Onsrud | same cells; Cut 1/2 x D | a | yes (verifier) | a5b83da2433c | 12 confirmed |
| onsrud_mdf_cutting_data | Onsrud | same cells in MDF | a | yes (verifier) | 94464ac393d9 | 12 confirmed |
| onsrud_hard_plywood_cutting_data | Onsrud | same cells in hard plywood | a | yes (verifier) | 65874767572e | 12 confirmed |
| onsrud_soft_plywood_cutting_data | Onsrud | same cells in soft plywood | a | yes (verifier) | f1c369ed8c8c | 12 confirmed |
| amana_15_60_90_vgroove_engraving_2f | Amana | 2F, 18,000 RPM, 1 x D; 0.003-0.007 in "Per Tooth IPR" with footnote "Inches per revolution" | b (unit) | yes (verifier) | 36f4a62673d2 | 6 confirmed |
| amana_15_60_vgroove_engraving_2f | Amana | subset of the 15/60/90 chart, same cells | b | yes (verifier) | 8e4281d620a8 | 0 rows; 4 note claims confirmed |
| amana_ams159_vgroove_v2 | Amana | one chip load per angle; 1 x D; no diameter | a | not verified | 69628c747676 | not verified, 0 rows |
| amana_spektra_engraving_v4 | Amana | tip width and one band per angle; 1 x D | a | not verified | 30e58f4d88e1 | not verified, 0 rows |
| onsrud_pct19_catalog | Onsrud | 37-series geometry (angles, flutes, sizes); no chip load | a (geometry) | not verified (verifiers read its stored text for the join) | 83dbf74242ca | not verified, 0 rows |
| precisebits_tapered_ball_2f | PreciseBits | stepdown 1x tip recommended, 2x tip maximum; stepover 8 % / 40 % of tip; no chip load | text only | not verified | f131e5a712d2 | not verified, 0 rows |
| precisebits_vtip_2500 | PreciseBits | V-tip stepdown by material, max DOC by angle; no chip load | text only | not verified | d8fbce48a11b | not verified, 0 rows |
| precisebits_vtip_scoreengrave | PreciseBits | micro V-tip stepdown by material; no chip load | text only | not verified | 5ec82145e96a | not verified, 0 rows |
| precisebits_calc | PreciseBits | V width formula incl. tip and runout; chip-thinning calculator | text only | not verified | 28b445b260ff | not verified, 0 rows |
| precisebits_calcv10_js | PreciseBits | radial chip thinning `chip · D / (2 sqrt(D a - a²))` (the engine's formula); linear Janka feed scale | code | not verified | 85a0590675a6 | not verified, 0 rows |
| precisebits_calibrating_feeds | PreciseBits | sweet-spot test; start feed 0.03 x D x flutes x RPM; not a recommendation | text only | not verified | 613df1dbb281 | not verified, 0 rows |
| precisebits_engraving_application | PreciseBits | tool family overview; no figure | text only | not verified | fa9726d3c01d | not verified, 0 rows |
| precisebits_feeds_and_speeds | PreciseBits | 2009 promise of a feeds database; no figure (dead end) | none | not verified | 063dd038e28f | not verified, 0 rows |
| precisebits_bullnose_2f_125 | PreciseBits | bull-nose corner radii; no chip load | text only | not verified | 90a672c9b45d | not verified, 0 rows |
| whiteside_120_vgroove_collection | Whiteside | RPM 14,000-16,000 (max 18,000); no chip load | RPM only | not verified | 069895ef9457 | not verified, 0 rows |
| whiteside_1550_product | Whiteside | RPM 16,000-20,000 (max 24,000); no chip load | RPM only | not verified | 1f7ac0217d6b | not verified, 0 rows |
| vectric_tool_database_v11 | Vectric | chip load from flutes, RPM, feed; V-bit diameter = tool diameter | c | not verified | 71579a4d7b73 | not verified, 0 rows |
| cnccookbook_vbit_form_milling | CNCCookbook | "effective diameter in an 0.010" depth of cut is only 0.020""; used for SFM, not chip load | c | not verified | 5f237f5546d3 | not verified, 0 rows |

Dead ends (FETCH_NOTES.md §3, §4):

- `onsrud.com/Series/EngravingTools.asp` and `Series/37-{00,20,50,60,80}.asp`:
  the product tables load by script; the static HTML holds only filter
  facets. Not stored. The catalogue gives the same geometry.
- `precisebits.com/reference/precisefedsped.asp`: a 2009 promise of a
  database that was never published.
- `pdf/pb_jsall.1.02.js`, `pb_jsdefer.1.0.js`: site scripts, no formulas.
- Harvey Performance "In the Loupe" (metal, search results only): corner
  radius thins the chip; no formula. Not stored.
- industrialmonitordirect.com (blog): "treat the minor diameter as the
  effective cutting diameter". Grade c at best; contradicts the engine's
  corner model. Not stored.
- Whiteside V-groove pages: RPM only. No Whiteside chip load exists to
  fetch.
- Not published anywhere the fetch reached: a V-bit chip load at the
  engaged width; a tapered-ball chip load on the cone; a wood bull-nose
  chip load on either diameter; the Amana insert tool diameters (the
  catalogue was not pulled).

LUT rows that are not on their cited document (T5; `fetch/G5/lut_discrepancies.md`
D1, D2). They are out of every trend above.

| row | cited source | angle | diameter_mm | chip mm/tooth | grade/kind |
|---|---|---|---|---|---|
| amana-vbit-softwood-trace-6000-2f | amana_insert_v_groove_v16 | 90 | 6.00 | 0.0400-0.0850 | a/exact |
| amana-vbit-hardwood-trace-6000-2f | amana_insert_v_groove_v16 | 90 | 6.00 | 0.0280-0.0600 | a/exact |
| amana-vbit-mdf-trace-6000-2f | amana_insert_v_groove_v16 | 90 | 6.00 | 0.0300-0.0650 | a/exact |
| amana-vbit-plywood-hardwood-trace-6000-2f | amana_insert_v_groove_v16 | 90 | 6.00 | 0.0300-0.0600 | a/exact |
| amana-vbit-acrylic-trace-6000-2f | amana_insert_v_groove_v16 | 90 | 6.00 | 0.0200-0.0450 | a/exact |
| amana-vbit-softwood-contour-12000-2f | amana_insert_v_groove_v16 | 90 | 12.00 | 0.0600-0.1100 | a/exact |
| whiteside-vbit-hardwood-trace-12000-2f | whiteside_vgroove_collection | 120 | 12.00 | 0.0400-0.0800 | b/exact |
| whiteside-vbit-mdf-trace-12000-2f | whiteside_vgroove_collection | 120 | 12.00 | 0.0450-0.0850 | b/exact |

## 3. For Phase 3

The G5 question is a KEY question: which diameter a printed number applies
to. The data answers it for V-bits (the top diameter or the angle) and for
tapered balls (the tip). It gives no number at any other key. So every form
below either keeps the printed key, or is an unprinted law.

### 3.1 V-bit, Trace-family ops (the 8 shipping RPM-only cells)

Candidate forms:

1. **Printed row at the printed key (nominal diameter or angle only), no
   width scale.** Onsrud keys on the cutting diameter; Amana on the angle.
   The verified rows add printed 60° wood chip loads (insert 0.061 1F;
   Onsrud 37-00 0.1016-0.1524 1F; 2F engraving 0.0762-0.1778, unit
   ambiguous). Range: the printed conditions (groove, trace, engrave; Onsrud
   90° at 1/2 CED; Amana 1 x D). One witness (vendor print).
2. **The formula keyed at nominal D** (today, by accident of the missing
   hint). Inside 0.5x-2x of the 60° figures (T9). No printed key supports
   the formula itself.
3. **Any value keyed at the engaged width.** No print. At the shipped
   0.381 mm depth it is below half of every 60° figure.

Second witness that could exist: the simulation's chip samples on a V-bit
trace or V-carve fixture (the measured chip at the shipped depth against
the printed value); the PreciseBits width formula (geometry only; it does
not carry a chip load).

Recommendation: **per-family**. Key a V-bit Trace-family row at the printed
key (form 1) and name the key on the card. The 6.35 mm cells then need a
60° row with a chip load; the verified rows supply it. The ruling must also
decide whether the hinted VCarve op keeps the width key (T9 row 1: 0.0448
at w against 0.061 printed), so that one tool gets one key.

### 3.2 V-bit parallel finish (the 16 refused cells)

Candidate forms:

1. **Printed row at the nominal key.** No chart prints a 3D finish pass for a
   V-bit. The pass engages 4.5 % to 23 % of D; the printed conditions engage
   the full cone. This moves a groove figure to a different cut. No witness.
2. **Width-keyed law `(w / D)^0.61`.** No print. It lands below half of the
   lowest 60° figure in 25 of 28 cases (T9), and below the 0.025 mm
   rubbing floor in 19 of 28. By R1 it refuses.
3. **Refuse** (today).

Second witness that could exist: the simulation's chip and force samples
for a 60° V-bit on a parallel pass over the terrain or wanaka fixture; a
physical rule for the minimum chip (edge radius). Neither exists in the
repo for a V-bit today.

Recommendation: **refuse**. Keep `VBIT_PARALLEL`. Reopen only with a
simulation witness on a V-bit finish pass and a ruling on the key.

### 3.3 The V-bit band de-rate at the engaged width (engine rule, all V-bit cells)

- At 53.13° and above it is a no-op (T7). Below 53.13° it de-rates by the
  angle alone, 0.50 at 15°-18°, and the Amana charts for those angles print
  "1 x Tool Diameter" as the only depth condition.
- The width ignores the tip diameter on flat-tip engraving tools.

Candidate forms: (a) de-rate at the nominal diameter (the printed Onsrud and
Amana reading); (b) keep the width. Witness that could exist: none printed;
the simulation on a 15° engraving fixture.

Recommendation: **per-family**, on the printed key. This is a ruling on an
existing rule, not a new claim. It moves numbers only below 53.13°.

### 3.4 Tapered ball (every shipping tapered cell)

- Every printed key is the tip; the engine keys on the cone (R2). At the
  printed 1 x tip condition the cone key scales the tip row by x1.078 on a
  7° taper; at 0.5 mm it scales it by 0.70-0.82; at 2 x tip by 1.215.
- Candidate forms: (a) key the row at the tip and keep the cone for the
  depth ladder only; (b) keep the cone key, with the `(d / tip)^0.61`
  scale recorded as an `Extrapolated` claim and a range of 0.5x-2x tip
  depth (the PreciseBits printed stepdown range).
- Witness that could exist: the PreciseBits depth rule (printed, depth
  only); the simulation's chip at the cone on the wanaka "3D Finish 6"
  case (shared with G1).

Recommendation: **per-family**, and decide with G1. The G1 size law must
name the same key (INVENTORY §2.3). Until the ruling, the cone key is an
unprinted scale of 0.70x to 1.22x on this taper.

### 3.5 Bull nose

No printed wood row on any key. The three LUT rows are `derived c`.
Recommendation: **refuse** as a G5 claim. The 48 bull finish refusals stay
in G3. The PreciseBits radial-thinning code is a second witness for the
engine's radial rule only; it says nothing about the corner or the depth.

### 3.6 Cells that stay refused under every form above

- 16 V-bit parallel finish cells (softwood, hardwood).
- Every bull-nose finish cell (G3 owns them).
- V-bit cells on MDF and plywood stay with G2. The verified Onsrud and
  Amana rows print those materials, but the two vendors disagree 2x on
  MDF (T4).

## 5. The landing (B4, 2026-09-25)

Ruling B4: key a V-bit row at its printed key; V-bit parallel finish
refuses. Plan and decisions: `B4_PLAN.md`.

| Commit | Step |
|---|---|
| d5b7e34d | the V-bit key is the nominal cutting diameter everywhere; the Amana AMS-159 / Spektra-engraving rows carry what the chart prints (no cutting diameter, Operating RPM 18 000); `SizeBasis::AngleKey`; chip rows first for a V-bit, so Suggest and the gate read one row |
| 99586f99 | the band de-rate at the nominal diameter (tapered keeps the cone); the card line "keyed at the printed angle / cutting diameter"; FM1 `lut_key` |

Measured (FM1): 18 V-bit cells ship the printed AMS 0.0762 mm/tooth at the
printed 18 000 rpm (2743 mm/min): 12.7 mm Trace-family x2.25 (was the
engine 8000 rpm); softwood VCarve x2.64; hardwood x1.31-1.39; softwood
6.35 mm Trace-family x0.84; 12.7 mm MDF / plywood VCarve x1.62 (form C).
8 cells refuse: the 6.35 mm V-bit in MDF / plywood (the only printed row
is 25.4 mm, 4x). Refusals 410 -> 418. The RPM-only anchor population on
the embedded LUT is now empty (every V-bit query finds a chip row).

Kept open: the Onsrud softwood / hardwood 37-series rows stay parked (they
win on hardness score, then refuse or halve their cells); Onsrud rows keep
the engine RPM (a V-bit RPM claim is a later package); the insert-v16 and
Whiteside 120 deg rows not on their cited documents; V-bit ProjectCurve
routes to None; the axial envelope de-rates through the chip-thinning
diameter (ball nose too).
