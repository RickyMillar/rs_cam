# G2: Material category (a row exists for another wood category)

Status: Phase 2 (trend) done 2026-09-24. Ruling B2 taken the same day;
P2 steps 1-4 landed (§5).

Inputs: the LUT (389 rows), `fetch/G2/verified_rows.json` (91 rows),
`inventory_cells.csv` and the FM1 matrix
`planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv`.
Scripts: `scripts/g2_verified_rows.py` writes the verified rows from
`fetch/G2/candidate_rows.json` and `fetch/G2/verifier_verdicts.json`;
`scripts/trend_g2.py` prints every table below (`python3
planning/extrapolation_2026-09-24/scripts/trend_g2.py`, tables T0 to T10).
Both are read-only on the LUT. Every ratio in this file is **derived**. A
printed value carries its source id.

## 0. The gap

### 0.1 The cells

`inventory_cells.csv` holds 98 G2 cells (all refused) and 34 G2h cells (all
shipped, flagged "extrapolated" on hardness). Table T9 gives the G2 cells and
what the same cell does in softwood and hardwood.

| Tool | Material | Cells | Same cell in softwood / hardwood | Operations |
|---|---|---|---|---|
| VBit | mdf | 8 | VendorBacked (trace rows) | Chamfer, Inlay, Trace, VCarve |
| VBit | mdf | 12 | FormulaOnly (judged Backed) | Face, Pocket, Profile, ProjectCurve, Rest, Zigzag |
| VBit | mdf | 12 | refused (G3 / G5) | DropCutter, HorizontalFinish, RadialFinish, RampFinish, SteepShallow, Waterline |
| VBit | plywood_hardwood | 8 / 12 / 12 | the same three splits | the same operations |
| BallNose | plywood_hardwood | 14 | VendorBacked (Amana v7 rows) | Adaptive, Adaptive3d, Face, Pocket, ProjectCurve, Rest, Zigzag |
| BallNose | plywood_hardwood | 10 | FormulaOnly (judged Backed) | Pencil, Profile, SteepShallow, Trace, Waterline |
| BallNose | mdf | 10 | FormulaOnly (judged Backed) | Pencil, Profile, SteepShallow, Trace, Waterline |

Totals: V-bit 64 (mdf 32, plywood 32), ball nose in plywood 24, ball nose in
MDF 10. Each count covers the two matrix sizes (V-bit 6.35 / 12.7 mm, ball
3.175 / 6.0 mm).

The 34 G2h cells (table T7): 26 are MDF cells on a printed MDF row, 6 are
softwood cells on a hardwood row, 1 is a softwood cell on a softwood row and
1 is a plywood cell on a plywood row. By tool: BallNose 15, EndMill 8,
BullNose 6, VBit 4, TaperedBallNose 1.

### 0.2 The engine rules that serve or refuse these cells

| Rule | Function and file | What it does |
|---|---|---|
| Category filter | `material_category`, `materials_compatible`, `passes_must_match` (`crates/rs_cam_core/src/feeds/vendor_lookup.rs`) | Softwood + hardwood = category 0; both plywoods = 5; MDF, HDF, particleboard = 6. A row in another category is a non-match (operator ruling 2026-09-24). |
| Operation filter | `passes_must_match` (same file) | `operation_family` must match exactly. The Amana v7 MDF ball rows are pocket / adaptive only. |
| V-bit angle gate | `find_best_vbit_row_where` (same file), `VBIT_ANGLE_TOLERANCE_DEG` = 20 | A V-bit row whose angle differs from the tool by more than 20 deg is skipped. A row with no angle passes with no bonus. The matrix V-bit is 60 deg. So the LUT's MDF and plywood V-bit rows today (Amana insert v16 at 90 deg, Whiteside at 120 deg) never reach it. |
| Formula judgement | `formula_backing` (`crates/rs_cam_core/src/feeds/support.rs`), reasons `VBIT_MDF_PLY`, `BALL_MDF`, `BALL_PLY` | With no row, the formula ships only when the class is judged Backed. The three G2 classes are Clueless. |
| Hardness transfer (G2h) | `hardness_ratio_raw`, `family_default_janka`, `apply_chipload_law` with `CHIPLOAD_HARDNESS_EXPONENT` = 0.5; flag `is_extrapolated_for_ratios` (ln 1.4) (`vendor_lookup.rs`) | Inside a category the band scales by (row Janka / query Janka)^0.5. A row with no `hardness_value` takes the family default. |
| Query Janka | `SheetGoodKind::effective_janka_lbf`, `PlywoodGrade::effective_janka_lbf`, `WoodSpecies::janka_lbf` (`crates/rs_cam_core/src/material/mod.rs`), used by `material_to_lut` (`feeds/vendor_normalize.rs`) | MDF 1100, Baltic birch 1200, generic softwood 600, generic hardwood 1450. |

## 1. The trend

### 1.1 What enters the trend (T0, T1)

- The trend uses 480 rows: 389 LUT rows and the 91 verified G2 rows.
- Only printed rows enter (`row_kind` exact). Left out: 53 wood rows that
  are derived grade c or fallback (Amana v7 x0.16 finish rows, ZrN tapered
  seed rows, Onsrud bull seed rows, IDC, Whiteside Fusion 360), 1 ZrN
  derived/a row, and 4 RPM-only rows.
- The Spektra flat chart prints two columns only, "Wood/Plywood" and
  "MDF/Laminate". All its non-MDF rows are one column (`wood_plywood`). They
  give one ratio per size, not four.
- The ZrN charts print one row "Wood, MDF, Sign-Foam". Their mdf and softwood
  rows are one cell with two labels (8 tool keys). They give no ratio.
- A row that the LUT copies into several operation families counts once.

Result: **77 tools** (same chart, family, subfamily, diameter, flutes, angle)
print a chipload in 2 or more columns: flat end 45, V-bit 21, ball nose 7,
tapered ball 2, facing 2.

### 1.2 The ratio per family and pair (T2; ratio of the mid chipload)

Ratio = the column's chipload / the same tool's base column. "n" is the
number of tools.

| Family | mdf / hardwood | mdf / softwood | plywood_hw / hardwood | plywood_sw / softwood | softwood / hardwood |
|---|---|---|---|---|---|
| V-bit | 1.00 [1.00, 1.08] n 13 | 1.00 [0.76, 1.00] n 12 | 1.00 [1.00, 1.02] n 12 | 1.00 [1.00, 1.00] n 11 | 1.00 [1.00, 1.42] n 19 |
| tapered ball | 1.00 [1.00, 1.00] n 2 | 1.00 n 2 | 1.00 n 2 | no row | 1.00 n 2 |
| ball nose | 1.14 [1.10, 1.50] n 7 | 0.91 [0.88, 1.00] n 7 | **no row** | **no row** | 1.29 [1.20, 1.50] n 7 |
| flat end | 0.93 [0.75, 1.58] n 17 | 0.88 [0.75, 1.36] n 15 | 1.07 [0.79, 1.18] n 13 | 1.06 [1.00, 1.17] n 7 | 1.06 [1.00, 1.43] n 14 |
| facing | 1.16 n 1 | 0.90 [0.89, 0.90] n 2 | 1.09 n 1 | no row | 1.30 n 1 |

Flat end, other bases: mdf / wood_plywood (Spektra) 1.25 [1.11, 2.00] n 19;
mdf / wood (Amana compression) 1.70 [1.44, 1.97] n 2. The median of the
max-chipload ratio agrees with the median of the mid ratio within 0.04 in
every row of T2. The ranges differ most on flat softwood / hardwood (max
[1.00, 1.20]) and ball mdf / hardwood (max [1.09, 1.40]).

### 1.3 The spread inside a family (T3)

The flat-end MDF ratio depends on the chart, not on the family:

| Chart | mdf / hardwood | mdf / softwood | plywood_hw / hardwood |
|---|---|---|---|
| Onsrud wood sheets (60-series) | 0.91 [0.75, 1.12] n 14 | 0.85 [0.75, 1.06] n 12 | 1.11 [1.00, 1.18] n 11 |
| Freud solid carbide | 1.57 [1.28, 1.58] n 3 | 1.16 [1.10, 1.36] n 3 | 0.89 [0.79, 1.00] n 2 |
| Amana Spektra (vs Wood/Plywood) | 1.25 [1.11, 2.00] n 19 (mdf / wood_plywood) | | |
| Amana compression (vs Wood) | 1.70 [1.44, 1.97] n 2 (mdf / wood) | | 1.00 n 1 (plywood / wood) |

The V-bit spread comes from one chart. Amana insert V-groove v16 (6 mm,
90 deg; no stored text in the manifest) prints softwood 1.42 x hardwood and
MDF 1.08 x hardwood. Every other V-bit chart prints one value for all its
columns: Onsrud 37-series (11 tools), Amana AMS-159 (4), Spektra engraving
(3). Whiteside 120 deg V-groove prints MDF 1.08 x hardwood.

### 1.4 The Onsrud 37-series V-bit (T6, verified rows)

Each size has one band, and the band is the same on every table that prints
the size:

| Series | Size | Band mm/tooth | Tables |
|---|---|---|---|
| 37-00 / 37-20 engraving, 1 flute | 1/4 in shank column | 0.1016-0.1524 | 7 of 7 |
| 37-50 V bottom, 90 deg, 2 flutes | 3/16, 1/4, 3/8 in | 0.0762-0.1524 | 7 of 7 |
| 37-60 V bottom, 90 deg, 2 flutes | 3/8, 1/2 in | 0.1016-0.1524 | 7 / 6 |
| 37-60 | 3/4 in | 0.1524-0.2032 | 6 |
| 37-60 | 1 in | 0.2032-0.254 | 6 |
| 37-80 lettering, 2 flutes | 1 in (60 deg), 1 1/4, 1 1/2, 2 in | 0.1016-0.1524 | 7 / 5 / 2 / 7 |

The 7 tables are hard wood, soft wood, MDF, hard plywood, soft plywood,
laminated chipboard and laminated plywood. The laminated chipboard table
prints 37-60 at 3/8, 9/16 and 7/8 in. No catalog part has 9/16 or 7/8 in
(probable layout error in the source, transcribed as printed).

### 1.5 Ball nose by size (T5, Amana v7, printed roughing rows)

| d mm | soft | hard | mdf | mdf / hard | mdf / soft | hard / soft |
|---|---|---|---|---|---|---|
| 1.588 | 0.102 | 0.076 | 0.102 | 1.33 | 1.00 | 0.75 |
| 3.175 | 0.152 | 0.102 | 0.152 | 1.50 | 1.00 | 0.67 |
| 6.35 | 0.203 | 0.152 | 0.178 | 1.17 | 0.88 | 0.75 |
| 9.525 | 0.229 | 0.178 | 0.203 | 1.14 | 0.89 | 0.78 |
| 12.7 | 0.254 | 0.203 | 0.229 | 1.12 | 0.90 | 0.80 |
| 15.875 | 0.279 | 0.229 | 0.254 | 1.11 | 0.91 | 0.82 |
| 19.05 | 0.305 | 0.254 | 0.279 | 1.10 | 0.92 | 0.83 |

On this chart MDF sits between softwood and hardwood. The ratio changes with
size because the chart steps each column by 0.001 in. All seven points are
one chart. The ZrN chart puts MDF equal to softwood (one cell). The
unverified Amana PCD chart puts MDF below "Wood" (derived from the fetch
notes: 1/4 in 0.73, 3/8 in 0.81 of the mid).

### 1.6 The hardness transfer the engine applies today (T7)

| Row family -> query | Row Janka -> query Janka | Row Janka from | Scale (^0.5) | Cells |
|---|---|---|---|---|
| mdf -> mdf | 700 -> 1100 | `family_default_janka` | x0.80 | 26 |
| hardwood -> softwood | 1450 -> 600 | per-row `hardness_value` | x1.55 | 3 |
| hardwood -> softwood | 1290 -> 600 | `family_default_janka` | x1.47 | 3 |
| softwood -> softwood | 500 -> 600 | `family_default_janka` | x0.91 | 1 |
| plywood_hardwood -> plywood_hardwood | 1000 -> 1200 | per-row `hardness_value` | x0.91 | 1 |

The rows behind them: the MDF cells use `amana-ball-mdf-{pocket,adaptive}-{3175,6350}-2f-v7`
(12 cells) and `onsrud-mdf-60-100mw-1_{8,4}-finish` (14 cells). The
hardwood -> softwood cells use `amana-ball-hardwood-scallop-6000-2f` (BallNose,
3) and `whiteside-1540-vgroove-60deg-quarter-rpm` (VBit, 3; an RPM-only
anchor, see G5). The Amana scallop row is derived grade c: one of the v7
finish rows reduced from the printed band by a factor with no named source.
So these 3 cells stack two unsourced factors (the reduction and the x1.55
hardness law).

Compare the law with the printed softwood / hardwood ratio (T7 foot):

| Family | Printed softwood / hardwood (mid) | Janka law, 1450 -> 600 |
|---|---|---|
| ball nose | 1.29 [1.20, 1.50] n 7 | x1.55 |
| flat end | 1.06 [1.00, 1.43] n 14 | x1.55 |
| V-bit | 1.00 [1.00, 1.42] n 19 | x1.55 (x1.47 from 1290) |
| tapered ball | 1.00 n 2 | x1.55 |

### 1.7 The two Janka tables (T8, lbf)

| Family | Query table (`material/mod.rs`) | Row default (`family_default_janka`) | Sourced value (G2 fetch) |
|---|---|---|---|
| softwood | 600 (GenericSoftwood) | 500 | species tables |
| hardwood | 1450 (GenericHardwood) | 1290 | species tables |
| plywood_softwood | 600 | 550 | none; no standard |
| plywood_hardwood | 1200 Baltic birch / 1000 hardwood-faced | 1100 | none; face species only (engine Birch 1260) |
| mdf | 1100 | 700 | **none found** |
| hdf | 1300 | 900 | not searched |
| particleboard | 750 | 600 | 500 lbf **minimum** (ANSI A208.1-2016 Table B; Wood Handbook 1999 Table 10-8) |

No pair agrees. Neither table cites a source in the code.

### 1.8 What the data shows

1. **V-bit and tapered ball: the category does not change the printed
   chipload.** Onsrud prints one V-bit band per size on all 7 tables and one
   77-100 band on all 4 tables that print it. Amana AMS-159 and Spektra
   engraving print one value across their wood columns. Only Amana insert v16
   separates the columns (MDF and plywood 1.02-1.08 x hardwood).
2. **Plywood tracks solid wood.** Plywood_hardwood / hardwood has family
   medians 1.00-1.09 and all points 0.79-1.18 (V-bit, tapered ball, flat,
   facing). This is the tightest pair in T4.
3. **MDF does not have one ratio.** Family medians are 0.93-1.16 against
   hardwood, but the points run 0.75-1.58 (1.97 on Amana compression
   against "Wood"). Inside flat end the charts disagree on the sign:
   Onsrud puts MDF below hardwood (0.91), Freud and Amana put it above
   (1.25-1.70).
4. **The Janka law overstates the softwood gain.** The law gives x1.55 for a
   hardwood row on a generic softwood query. The printed ratio is 1.00 on
   V-bit and tapered ball, 1.06 on flat end and 1.29 on ball nose. The law
   exceeds the largest printed ball-nose ratio (1.50).
5. **26 of the 34 G2h cells are not a transfer.** A printed MDF row serves an
   MDF query, and the band drops to x0.80 only because the two engine Janka
   tables disagree (700 against 1100). Neither number has a source.

### 1.9 What the data does NOT show

- It has no ball-nose plywood row. Point 2 comes from other tool families.
- It has no bull-nose row in more than one category that is printed
  (the Onsrud bull rows are derived grade c).
- It does not show a ratio against Janka for MDF or plywood: these boards
  have no sourced Janka value.
- It does not show that the V-bit equality holds at the engaged width or at
  60 deg on every chart. The Onsrud 60 deg evidence is one size (37-80,
  1 in).
- The ball-nose MDF trend is one chart (Amana v7). The PCD chart, which
  disagrees, is not verified.
- No fit is made here. Phase 3 fits.

## 2. Sources

Grade: a = vendor or standard document with a stored text and a sha256;
c = machine builder or reseller chart. Verdicts: c = confirmed rows. "not
checked" = no verifier ran; the fetch agent's claim stands and its rows stay
out of `verified_rows.json`. The harness list of unverified sources names
`amana_ball_nose_v7`, but a full verdict for it exists (15 confirmed); this
file treats it as verified.

| Source id | Vendor | What it prints | Grade | URL reachable | PDF sha256 | Verdict |
|---|---|---|---|---|---|---|
| onsrud_hard_wood_cutting_data | Onsrud | 37-series V-bit and engraving; 77-100; 60-series | a | yes | f123d5f6…69dc (= manifest) | 11 c |
| onsrud_soft_wood_cutting_data | Onsrud | the same, soft wood | a | yes | a5b83da2…6c54 (= manifest) | 11 c |
| onsrud_mdf_cutting_data | Onsrud | the same, MDF | a | yes | 94464ac3…df4c (= manifest) | 11 c |
| onsrud_hard_plywood_cutting_data | Onsrud | the same, hard plywood | a | yes | 65874767…5e24 (= manifest) | 11 c |
| onsrud_soft_plywood_cutting_data | Onsrud | the same, soft plywood; no 77-100 | a | yes | f1c369ed…1ca1 (= manifest) | 11 c |
| onsrud_laminated_chipboard_cutting_data | Onsrud | page 119 upper table; 37-60 at 3/8, 9/16, 7/8 in | a | yes | 21f3c09a…65e2 | 10 c |
| onsrud_laminated_plywood_cutting_data | Onsrud | page 119 lower table; 37-80 at 1, 1 1/2, 2 in | a | yes | 7f271689…816b | 11 c |
| onsrud_pct19_catalog | Onsrud | part definitions (flutes, angles, sizes), pp. 20-22, 119 | a | yes | 83dbf742…144b | used by the verifiers; no rows |
| amana_ball_nose_v7 | Amana | ball nose, softwood / hardwood / MDF, 1/16-3/4 in | a | yes | e851a270…3563 (= manifest) | 15 c |
| amana_pcd_ball_nose_v2 | Amana | PCD ball, Wood / MDF / uncoated chipboard, 1/4, 3/8 in | a | not checked | 0dc5282a…d673 | not checked (4 rows out) |
| amana_spektra_3d_profiling_v6 | Amana | ball, one row "Wood, MDF, Sign-Foam"; repeats ZrN | a | not checked | 84feaace…3497 | not checked (6 rows out) |
| amana_spiral_ball_nose_v6 | Amana | same wood / MDF values as v7 | a | not checked | 05f75582…19f2 | corroboration only |
| amana_spiral_ball_nose_2015 | Amana (reseller host) | same values as v7 | a | not checked | 70d4e9c3…1681 | corroboration only |
| freud_router_bit_feed_and_speed_for_cnc_20170822 | Freud | solid carbide and carbide tipped; MDF/PB, laminated PB, hardwood, softwood, plywood columns | a | not checked | ff1a29ea…1976 | first stored text; no new rows |
| leitz_lexicon7_05_routing | Leitz | "Correction factor for vf": MDF 0.8 (0.9, 0.6) vs coated chipboard; hardwood 0.7-0.9 and chipboard 1.1-1.3 vs softwood; no plywood factor | a | not checked | 76cbbc24…7072 | statements only |
| techno_cnc_chipload_rev2 | Techno CNC | groups "Softwood & Plywood", "MDF & Particle Board"; hardwood lower | c | not checked | d4f5c880…aae5 | statements only |
| sienci_feeds_speeds_imperial | Sienci | feeds only; groups "Softwood/Soft Plywood/MDF", "Hardwood / Hard Plywood" | c | not checked | 5e37bea0…7e54 | statements only |
| sorotec_schnittwerte | Sorotec | Holz hart / weich / MDF; MDF 2-6 x wood (outlier) | c | not checked | 67300f95…c6d2 | statements only |
| shopbot_feedsandspeeds_2016 | ShopBot | copy of Onsrud values; 37-82 .004-.006 in 6 tables | c | not checked | 7ad6d740…f56e | corroboration only |
| ansi_a208_1_2016_particleboard | CPA / ANSI | particleboard hardness 2225 N (500 lb) minimum, Table B | a | not checked | c459462c…3be6 | Janka floor only |
| wood_handbook_1999_ch10 | USDA FPL (mirror) | Table 10-8 particleboard hardness minima; Table 10-10 MDF has no hardness | a | not checked | 040c02ca…9f99 | Janka floor only |
| weyerhaeuser_mdf_spec | Weyerhaeuser | MDF density 788 kg/m3; no hardness | a | not checked | ef9db59f…7b72 | negative result |

Verifier caveats that apply to all confirmed Onsrud rows: the sha is the PDF
sha, not the stored-text sha (the stored text adds 2 header lines). Flute
count, angle, part and `hardness_value` come from PCT-19 or the engine proxy,
not from the sheet. `g2_verified_rows.py` records two wording corrections:
the 37-00 tip range (0.005-0.090 in, not 0.005-0.040 in) and the Amana v7
`ap_rule` (printed text, not a paraphrase).

Dead ends (FETCH_NOTES.md §1, §7):

- **Ball nose in plywood: no vendor chart prints it.** Checked: Amana v6,
  v7, 2015, Spektra 3D v6, ZrN, PCD, the Onsrud sheets and Freud. Amana ball
  v8-v10 return HTTP 404.
- No vendor states a plywood chipload factor against solid wood.
- No vendor states an MDF percentage rule ("increase 10-20 %" is blog text
  only).
- No published Janka value for MDF (ANSI A208.2-2016 returned HTTP 403;
  Roseburg returned HTML) or for plywood.
- Whiteside, Vortex, CMT, Carbide 3D and SpeTool charts with wood columns
  were not reachable (JavaScript tables, forum pages).
- The Onsrud 37-series web pages load their tables by JavaScript (PCT-19
  replaced them). `fpl.fs.usda.gov` blocks curl (a mirror was used).

## 3. For Phase 3

### 3.1 V-bit in MDF and plywood (64 cells)

- **Form:** no extrapolation. The printed Onsrud 37-series rows cover MDF,
  both plywoods and laminated chipboard (91 verified rows, 76 of them
  V-bit). A second family of charts agrees that the category does not move
  a V-bit chipload (AMS-159, Spektra engraving: 1.00; insert v16 and
  Whiteside 120 deg: 1.02-1.08 x hardwood).
- **Range it could claim:** the printed sizes, 3/16 in to 2 in, and the
  1/4 in shank engraving column.
- **What landing moves (T9, T10), if the rows load as trace / finish:**
  - 16 cells (Chamfer, Inlay, Trace, VCarve x 2 sizes x 2 materials) can
    reach a row. But the matrix V-bit is 60 deg, and the angle gate skips
    the 90 deg 37-50 / 37-60 rows. Only 37-80 at 1 in (60 deg) and the rows
    with no angle (37-00/37-20, 37-80 at 1 1/4 and 2 in) pass. The 1 in row
    scales by (6.35 / 25.4)^0.61 = x0.43 on the 6.35 mm tool and by
    (12.7 / 25.4)^0.61 = x0.66 on the 12.7 mm tool. Both ratios pass the
    ln 1.4 threshold, so both sizes land as G1 size transfers flagged
    "extrapolated", not as clean rows. The 37-00/37-20 row has no diameter
    (the column is the shank), so it does not scale.
  - 24 cells (Face, Pocket, Profile, ProjectCurve, Rest, Zigzag) stay
    refused by the judgement `VBIT_MDF_PLY`. Its text ("No published V-bit
    chipload exists for MDF or plywood") becomes false once the rows load.
    The judgement must be run again against the new rows, as it was for
    softwood and hardwood.
  - 24 cells (DropCutter, HorizontalFinish, RadialFinish, RampFinish,
    SteepShallow, Waterline) stay refused. Their softwood and hardwood
    siblings refuse too (G3 / G5).
- **Rulings needed:** the cells printed with no catalog part (37-60 at
  3/8 in, 37-80 at 1 1/4 in, laminated chipboard 9/16 and 7/8 in); the
  laminated-plywood table mapped to `plywood_hardwood`; the frame of the
  37-00/37-20 shank column.
- **Recommendation: transcribe (LUT change), not extrapolate.**

### 3.2 Ball nose in plywood (24 cells)

- **Form the trend supports:** plywood = the solid-wood row of the face
  class x 1.00. Evidence: plywood_hardwood / hardwood 1.00-1.09 by family
  median, 0.79-1.18 over all points, on V-bit, tapered ball, flat end and
  facing. Machine builders (Techno CNC, Sienci; grade c) group plywood with
  solid wood.
- **Range it could claim:** 0.79-1.18 x the hardwood row, only where a
  hardwood ball row exists (Amana v7 pocket / adaptive, 1/16 to 3/4 in).
- **Second witness that could exist:** none printed for a ball nose. A
  simulation witness needs a plywood `Kc` (G7 has none). A literature cell
  for plywood chipload was not found.
- **Cells that stay refused:** the 10 contour / trace cells (Pencil,
  Profile, SteepShallow, Trace, Waterline). The hardwood ball rows are
  pocket / adaptive; these cells need the G3 family transfer as well.
- **Recommendation: refuse.** If the operator wants these cells, the only
  candidate is a **generic** "plywood from hardwood x 1.00" claim with one
  class of witness (other tool families). It must carry the 0.79-1.18
  range on the card and it must be a Phase 4 ruling.

### 3.3 Ball nose in MDF (10 cells)

- The 10 cells are contour / trace operations. MDF ball rows exist (Amana
  v7, now 7 sizes) but only in the pocket / adaptive family. The gap is the
  operation family (G3), not the category.
- The ball-nose MDF ratio is one chart: 1.10-1.50 x hardwood, 0.88-1.00 x
  softwood. ZrN says MDF = wood. The unverified PCD chart says MDF < wood.
- **Recommendation: refuse under G2; hand to G3.** Do not derive an MDF
  ball row from a solid-wood ball row.

### 3.4 A generic MDF ratio

- **Recommendation: refuse (per family at most).** The points run
  0.75-1.58 against hardwood, and inside flat end the vendors disagree on
  the sign (Onsrud 0.91, Freud 1.57, Amana 1.25-1.70). Leitz gives
  "MDF = 0.8" against coated chipboard, a third base. The category ban
  between solid wood and MDF matches the data for a generic claim.
- Per family, V-bit and tapered ball could claim MDF = hardwood x 1.00
  [1.00, 1.08]; both families already have printed MDF rows (Onsrud).

### 3.5 G2h: the hardness transfer (34 cells)

- **26 MDF cells (x0.80): a defect, not a trend.** A printed MDF row on an
  MDF query should reach the query unscaled, as the Onsrud 77-100 rows do
  through their per-row `hardness_value` 1100. Candidate fix for Phase 4:
  one Janka table for the row default and the query, or no hardness scale
  inside categories 5 and 6 (these boards have no sourced Janka). The same
  table split gives the 500 -> 600 softwood cell and the 1000 -> 1200
  plywood cell (x0.91).
- **6 hardwood -> softwood cells (x1.47, x1.55): the law exceeds the
  printed ratio.** Three of them also start from an unsourced derived
  grade c row (`amana-ball-hardwood-scallop-6000-2f`). The printed softwood / hardwood ratio is 1.00 (V-bit,
  tapered ball), 1.06 (flat) and 1.29 [1.20, 1.50] (ball nose). Candidate
  forms for Phase 3: a per-family cap at the largest printed ratio, or a
  per-family exponent fitted on the T2 softwood / hardwood points. Second
  witness: the simulation force at a matched chip (G7 `Kc` for the two
  generic woods exists).
- **Janka values:** only particleboard has a sourced number (500 lbf, a
  minimum). The 700 / 1100 MDF pair and every plywood value need a ruling
  or a removal.

### 3.6 Summary per sub-class

| Sub-class | Cells | Recommendation | Basis |
|---|---|---|---|
| V-bit MDF / plywood | 64 | transcribe printed rows (not an extrapolation); 16 can move, 24 need the judgement re-run, 24 stay refused (G3/G5) | Onsrud 37-series, 7 tables, 91 verified rows |
| Ball nose plywood | 24 | refuse; option: generic plywood <- hardwood x 1.00 [0.79, 1.18] by ruling | no ball chart; other families only |
| Ball nose MDF | 10 | refuse under G2; G3 owns the family gap | MDF ball rows exist, wrong family |
| Generic MDF ratio | — | refuse | vendors disagree on the sign (0.75-1.58) |
| G2h MDF x0.80 | 26 | fix the table split (defect) | same row family, no sourced Janka |
| G2h hardwood -> softwood | 6 | per-family cap or exponent (Phase 3 fit) | law x1.55 > printed 1.00-1.50 |

**Biggest open gap:** no reachable vendor chart prints a ball-nose chipload
in plywood. The 24 plywood ball cells have no printed witness in their own
family.

## 5. The landing (P2, 2026-09-24)

Ruling B2. Plan and the orchestrator's decisions 1-7: `P2_PLAN.md`.

| Commit | Step |
|---|---|
| 1b69ceaa | Steps 1-2: 40 Onsrud 37-series V-bit rows (MDF, hard and soft plywood, laminated chipboard -> particleboard) and 15 Amana ball v7 pocket rows; 441 -> 496 rows. Strict V-bit judgement (decision 2). |
| a32ea149 | Step 3: one Janka table. A solid-wood row with no Janka reads `WoodSpecies::GenericSoftwood` (600) or `GenericHardwood` (1450); the composite categories take no hardness scale. |
| TODO (not committed) | Step 4: the soft/hard cap, `feeds/extrapolation/hardness.rs`. |

### Amendment 7 (step 1, measured)

The softwood and hardwood 37-series V-bit rows are parked too: 40 V-bit
rows load, not 60 (441 -> 496, not 516). When they loaded, they displaced
the Whiteside 1/4 in RPM anchor of 5 softwood V-bit cells. The engine RPM
(10 026) replaced a printed 22 000, and the feed moved x0.34, not the -26 %
that the plan predicted from the chipload alone. G2 is the MDF / plywood
gap; the solid-wood V-bit size key is the question of ruling B4.

### Step 4: the soft/hard cap (decision 4)

`feeds::extrapolation::hardness_basis(query, row)` is the one place where
the hardness ratio and the hardness scale of a matched row come from.
`build_result` reads it, and `LookupResult::hardness_basis` carries it.
The basis is `Unscaled`, `CompositeBoard`, `Law` or `Capped`. It is a typed
clamp, not an `Extrapolation` impl (P2_PLAN §4).

The rule: when the query and the row are solid wood, the Janka law's scale
is above 1, and the scale is above the cap of the ROW's tool family, the
applied scale is the cap. The raw ratio is unchanged, so
`is_extrapolated` still reads it and a capped row stays flagged.

The caps (`SOFT_OVER_HARD_PRINTED_MAX`) are the upper ends of the
"softwood / hardwood" ranges in §1.2, table T2. Each value was checked
against that table; no value differs from the plan.

| Row family | Cap | Evidence (§1.2 T2) | The cap acts below (query Janka, hardwood row 1450) |
|---|---|---|---|
| ball nose | 1.50 | [1.20, 1.50], n 7 (Amana v7; §1.5 T5 at 3.175 mm: 0.006 / 0.004 in) | 644 lbf |
| flat end | 1.43 | [1.00, 1.43], n 14 | 709 lbf |
| V-bit | 1.42 | [1.00, 1.42], n 19 (Amana insert v16, §1.3) | 719 lbf |
| tapered ball | 1.00 | 1.00, n 2 (Onsrud 77-100) | every query under the row's Janka |
| facing bit | 1.30 | 1.30, n 1 | 858 lbf |
| bull nose | 1.43 (flat end, borrowed) | no printed pair (§1.9) | 709 lbf |

The threshold column is `row / cap^2`. The tapered cap blocks every upward
transfer on a tapered row, also a softwood row (600) on a softer softwood.

The card: `why::draw_row_basis_lines` paints the cap headline as a
visible line, with the detail on its hover. The claim hover names a
capped hardness scale. The `approx x` token does not paint for a capped
row. Example (a 6.0 mm ball Scallop in generic softwood):

- headline: "extrapolated (G2 hardness): capped at x1.50";
- detail: "hardness transfer capped at x1.50 (the largest
  softwood/hardwood ratio that ball nose charts print); the Janka law gives
  x1.55 (raw ratio 2.42, row 1450 lbf, printed on the row)".

FM1 has three new columns after `claim_residual`: `hardness_ratio_raw`,
`hardness_scale`, `hardness_basis`.

Sentries: `a_hardness_transfer_caps_at_the_printed_soft_hard_ratio_g2`
(new); `one_janka_table_for_row_and_query_g2` (its softwood arm re-pinned
from the law 1.554563175515 to the flat-end cap 1.43, band
0.508508-0.581152); `lookup_parity` (a capped case, and the calculator and
the gate must carry the same hardness basis); viz
`a_claimed_row_states_its_claim_on_the_card_g_claimcard` (a capped cell
shows its cap line).

### Cells that move (step 4)

Predicted (P2_PLAN §4): 6 cells, the softwood ball Scallop, UnifiedFinish
and SpiralFinish at 3.175 and 6.0 mm on
`amana-ball-hardwood-scallop-6000-2f`. The hardness scale goes x1.555 ->
x1.50 (-3.5 %). No V-bit cell is capped: the 3 Whiteside softwood V-bit
cells left that row in step 1. No status moves.

Off the matrix the cap acts on every solid-wood query softer than the
threshold of the table above. Predicted moves in the literature matrix
(run `literature_matrix` for this step):

- `ball_3mm_scallop_softwood` (Janka 380, on the Amana ball scallop row):
  the law x1.953 goes to x1.50; the band goes from 0.0320-0.0512 to
  0.0246-0.0393 mm/tooth. The expected band is 0.025-0.055. Expect green;
  verify.
- `tapered_ball_2mm_scallop_oak` (Janka 1290): on a 1450 tapered row the
  law x1.062 goes to x1.00 (the printed band).

The two `#[ignore]` harnesses in `law_magnitude_measurement.rs` recover
the unscaled band as `band / (law_d * law_h)`. On a capped row that
divisor is not the applied scale, so their "unscaled" column is wrong
there. The FM1 column `hardness_basis` reads `Law` also for a row at the
query's own hardness (scale 1.00): count the moved cells by `Capped`.

Measured (FM1 re-run after step 4, 2026-09-24):

- No status moves: 464 ship (274 VendorBacked, 108 Extrapolated, 82
  FormulaOnly), 496 refuse.
- 9 cells carry `hardness_basis = Capped`:
  - 6 ball-nose softwood cells (Scallop, UnifiedFinish, SpiralFinish at
    3.175 and 6.0 mm) on `amana-ball-hardwood-scallop-6000-2f`: the law
    gave x1.555, the cap x1.50; band maximum x0.964 (0.0600 at 6.0 mm,
    0.0407 at 3.175 mm).
  - 3 V-bit softwood cells (Trace, Chamfer, Inlay at 6.35 mm) on the
    Whiteside RPM-only row: capped at x1.42, but the row prints no
    chipload, so no band moves.
- Shipping cells per basis: Law 199, CompositeBoard 174, Capped 9, none
  (FormulaOnly) 82.

### Known reds, not caused by P2

- `feed_explanation_snapshot_b3` and `tapered_width_model_parity_c3`: red
  on HEAD before P1 and after it (EXTRAPOLATION_G1 §5).
- `drop_cutter_flat_roughing_row_g_dcflat`: red before and after step 3
  (a stale R5 id, a bandless Spektra row: the A2 point-mode package).
