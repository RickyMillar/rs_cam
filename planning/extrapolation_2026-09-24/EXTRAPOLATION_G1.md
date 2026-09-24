# G1: Size (a row exists for the family but not at this diameter)

Status: Phase 2 (trend) done 2026-09-24; Phase 3 (fit, witness) not started; nothing lands before the Phase 4 rulings.

Inputs:

- The LUT at 23bcdc59: 389 rows in `crates/rs_cam_core/data/vendor_lut/observations/*.json`.
  The Carbide 3D rows (G9) are parked outside the live LUT and are not in these tables.
- `fetch/G1/verified_rows.json`: 96 rows. All 96 candidate rows were confirmed by the verifiers.
  The script `scripts/g1_verified_rows.py` writes the file.
- `inventory_cells.csv` (Phase 0).

Script: `scripts/trend_g1.py`. It is read-only on the LUT. Its full output
is in `fetch/G1/trend_g1.out`. Every slope, ratio and percentage in this file
is **derived** by that script. No vendor prints a slope.

## 0. The gap

### 0.1 The cells in the matrix

The Phase 0 inventory puts **4 shipping cells** in G1. These cells ship on a
row that the engine transfers by more than 1.4x in diameter.

| Tool | D mm | Operation | Material | Anchor row | Row D | Anchor evidence |
|---|---|---|---|---|---|---|
| BallNose | 3.175 | Scallop | hardwood | `amana-ball-hardwood-scallop-6000-2f` | 6.0 | grade c, derived: "Derived (reduced) for 3D-finish use ... this band is not printed on the cited chart" |
| BallNose | 3.175 | UnifiedFinish | hardwood | same | 6.0 | same |
| BallNose | 3.175 | SpiralFinish | hardwood | same | 6.0 | same |
| VBit | 6.35 | VCarve | softwood | `amana-vgroove-softwood-trace-60deg-2f` (AMS-159) | 12.7 | grade a, exact, one value 0.0762 mm |

Counts:

- Tool: BallNose 3, VBit 1.
- Operation: Scallop 1, UnifiedFinish 1, SpiralFinish 1, VCarve 1.
- Material: hardwood 3, softwood 1.

Two facts about these 4 cells:

- The 3 ball-nose cells rest on a row that the chart does not print. The
  LUT holds the chart's printed hardwood 1/8 in band (0.0762-0.127 mm) at
  the cell's own size, as a roughing row. The gap for these cells is the
  pass role (G3), not the size.
- The V-bit cell scales a size-flat row. The only V-bit series in the LUT
  prints 0.0762 mm at 9.525 mm and at 12.7 mm (slope 0.00). The 0.61 law
  still multiplies the row by (6.35 / 12.7)^0.61 = 0.66.

### 0.2 The cells outside the matrix

The FM1 matrix holds 2 sizes per tool kind (3.175 / 6.0 mm, and V-bit
6.35 / 12.7 mm). It holds no tool under 1.5 mm. The size rule refuses
every sub-1.5 mm tool whose row is more than 2x or less than 0.5x its
lookup diameter. The acceptance case is outside the matrix:

- wanaka "3D Finish 6": tp 11, DropCutter, a 1.0 mm tip tapered ball, hardwood.
  Today the lookup finds a row of 3.175 mm or larger and the size rule refuses it.

### 0.3 The current engine rule

| Rule | Function / constant (file) | What it does |
|---|---|---|
| Size law | `CHIPLOAD_DIAMETER_EXPONENT = 0.61`, `apply_chipload_law` in `build_result` (`crates/rs_cam_core/src/feeds/vendor_lookup.rs`) | Scales the row band by `(query D / row D)^0.61`. The same scale applies to min, max and mid. |
| Clamp | `SCALE_CLAMP_LO = 0.1`, `SCALE_CLAMP_HI = 10.0` (same file) | Limits the combined scale. |
| Flag | `is_extrapolated_for_ratios`, `CHIPLOAD_EXTRAPOLATION_LN_THRESHOLD = ln 1.4` (same file) | Marks the row "extrapolated" on the RAW diameter x hardness ratio. The 4 cells above carry this flag. |
| Size rule | `micro_extrapolation_refusal` with `MICRO_TOOL_DIAMETER_MM = 1.5`, `MICRO_ROW_RATIO_MIN = 0.5`, `MICRO_ROW_RATIO_MAX = 2.0` (`crates/rs_cam_core/src/feeds/support.rs`), called from `support_for_lookup` | Refuses a tool under 1.5 mm (lookup diameter) when the row is outside 0.5x-2.0x of it. |

The size rule and the law both act on the LOOKUP diameter. On a tapered
ball, that is the engaged diameter at the cut depth, not the tip (§1.5).

## 1. The trend

### 1.1 What the script counts

- 363 printed wood points (grade a or b, not `fallback`) before the dedupe.
- 63 series with 2 or more diameters: 42 from the LUT only, 21 from the G1 rows only.
- 12 of the 63 are copies, so 51 series are distinct. A copy is the same printed cell applied to a second
  material (for example the Amana and SpeTool "Wood, MDF, Sign-Foam" rows, or
  the Spektra "Wood/Plywood" column). A copy is not a second witness.
- Grade c series (presets, community values, repo-authored bands) are listed
  apart in §1.7. They are not in the slope tables.

### 1.2 Slopes per tool family (distinct series, copies left out)

| Family | Series with >= 3 sizes | Median slope | Range | Series with 2 sizes | Median | Range | Widest span |
|---|---|---|---|---|---|---|---|
| flat_end | 34 | +0.48 | +0.29 to +1.25 | 7 | +0.41 | +0.37 to +1.38 | 16.0x |
| ball_nose | 0 | - | - | 5 | +0.46 | +0.22 to +0.83 | 2.0x |
| tapered_ball_nose | 4 | +0.64 | +0.00 to +1.06 | 0 | - | - | 8.0x |
| chamfer_vbit | 0 | - | - | 1 | +0.00 | +0.00 | 1.3x |
| bull_nose | 0 | - | - | 0 | - | - | - |

### 1.3 Slopes per source (distinct series)

| Family | Source | Series | Slopes, >= 3 sizes | Slopes, 2 sizes (no residual) | Frame |
|---|---|---|---|---|---|
| ball_nose | amana_ball_nose_v7 | 3 | - | +0.22, +0.42, +0.58 | cutting D |
| ball_nose | amana_zrn_3d_profiling | 1 | - | +0.83 | tip (tapered tools, D1) |
| ball_nose | amana_zrn_3d_profiling_v8 | 1 | - | +0.46 | cutting D |
| chamfer_vbit | amana_ams159_vgroove_v2 | 1 | - | +0.00 | body D |
| flat_end | amana_compression_spirals_v8 | 2 | - | +0.86, +1.31 | cutting D |
| flat_end | amana_spektra_spiral_plunge_v24 | 6 | +0.33, +0.33, +0.41, +0.46, +0.57, +0.62 | - | cutting D |
| flat_end | amana_zrn_3d_profiling_v8 | 1 | +0.92 | - | cutting D |
| flat_end | freud (2017-08-22) | 4 | +1.05, +1.09, +1.25 | +1.38 | cutting D |
| flat_end | onsrud_hard_plywood | 4 | +0.29, +0.41, +0.48 | +0.37 | cutting D |
| flat_end | onsrud_hard_wood | 4 | +0.38, +0.38, +0.40 | +0.41 | cutting D |
| flat_end | onsrud_mdf | 4 | +0.29, +0.34, +0.36, +0.37 | - | cutting D |
| flat_end | onsrud_soft_plywood | 4 | +0.34, +0.37, +0.48 | +0.37 | cutting D |
| flat_end | onsrud_soft_wood | 3 | +0.38, +0.49 | +0.38 | cutting D |
| flat_end | spetool_carbide_spiral (G1) | 9 | +0.75, +0.80, +0.86, +0.86, +0.94, +0.96, +1.04, +1.07, +1.11 | - | cutting D |
| tapered_ball_nose | amana_zrn_3d_profiling_v8 (G1) | 3 | +0.00 (4F), +0.43 (3F), +0.85 (2F) | - | tip |
| tapered_ball_nose | spetool_2d3d_tapered (G1) | 1 | +1.06 | - | tip |

The per-series table (n, span, r², slopes on min and max) is in the appendix §A1.

### 1.4 Against the shipped 0.61 and the prior exponents

Prior per-family exponents (`planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md` §4.1, quoted):

| Family | Fits | Range | Median | Span mm |
|---|---|---|---|---|
| Onsrud (own charts, both woods) | 50 | 0.20-0.66 | 0.37 | 1.6-50.8 |
| Onsrud (LUT subset) | 9 | 0.29-0.49 | 0.375 | 3.2-19.1 |
| Amana Spektra spiral plunge | 2 | 0.586-0.619 | 0.60 | 0.79-12.7 |
| Amana ZrN 2D/3D carving | 2 | 0.488-0.924 | 0.71 | 3.2-6.4 |
| Freud | 3 | 1.05-1.25 | 1.09 | 3.2-12.7 |
| IDC Woodcraft (grade c) | 1 | 0.230 | 0.23 | 3.2-12.7 |
| Whiteside Fusion360 (grade c) | - | 0.0 | 0.0 | 3.2-6.4 |

This run reproduces the prior values:

- Onsrud: 0.29-0.49.
- Spektra 2-flute: 0.57 (MDF) and 0.62 (softwood) over 13 sizes.
- Freud: 1.05-1.25.

It adds three new sets:

- SpeTool spiral: 9 series, 0.75-1.11. SpeTool joins Freud near 1.0.
- SpeTool tapered: 1.06 in the tip frame, 9 sizes, 0.5-4.0 mm.
- Amana tapered: 0.00 / 0.43 / 0.85 for 4 / 3 / 2 flutes, in the tip frame.

Of the 38 distinct series with 3 or more sizes, 16 have a slope above 0.61 and 22 at or below it.

The shipped law applied across each series: printed mid at the smallest size, divided by
(printed mid at the largest size x (dmin / dmax)^0.61). A ratio under 1 means that
the law gives more than the chart prints at the small end.

| Family | Series | Median ratio | Range |
|---|---|---|---|
| flat_end | 34 | 1.10 | 0.30-1.59 |
| tapered_ball_nose | 4 | 0.90 | 0.50-1.58 |

The ratio splits by vendor, not by size:

| Cluster | Series | Slope | Ratio at dmin | Law at the small end |
|---|---|---|---|---|
| Onsrud, Spektra 3F, Spektra hardwood | 19 | 0.29-0.49 | 1.06-1.59 | below the chart (conservative) |
| Spektra 2F MDF / softwood (16x span) | 2 | 0.57 / 0.62 | 1.13 / 0.95 | on the chart |
| SpeTool spiral, Freud, Amana ZrN flat | 13 | 0.75-1.25 | 0.30-0.87 | above the chart (permissive), up to 3.3x |
| Amana tapered 2F, SpeTool tapered | 2 | 0.85 / 1.06 | 0.53 / 0.50 | above the chart, 1.9-2.0x |
| Amana tapered 3F, 4F | 2 | 0.43 / 0.00 | 1.26 / 1.58 | below the chart |

### 1.5 Below 1.5 mm

The table has every distinct printed cell under 2.0 mm. `*` marks a size under 1.5 mm.

- **% of D**: chipload / D x 100.
- **Law value**: the shipped law applied to the tapered row that the lookup
  finds today (Onsrud 77-100 1/8 in, 3 flutes, hardwood, 0.0762-0.127 mm),
  scaled by (d / 3.175)^0.61. It is the value that the size rule stops.
- **Frame**: the diameter that the row keys on.

| Family | Source | Subfamily | Fl | D mm | Frame | Printed min-max mm | % of D | Law value mm | Law % of D | Printed mid / law mid |
|---|---|---|---|---|---|---|---|---|---|---|
| tapered_ball_nose | SpeTool 2D/3D | tapered | 2 | 0.5 * | tip | 0.01778 (one value) | 3.6 | 0.0247-0.0411 | 4.9-8.2 | 0.54 |
| tapered_ball_nose | Amana ZrN v8 | tapered | 3 | 0.794 * | tip | 0.01905-0.0508 | 2.4-6.4 | 0.0327-0.0545 | 4.1-6.9 | 0.80 |
| tapered_ball_nose | SpeTool 2D/3D | tapered | 2 | 0.794 * | tip | 0.02032 | 2.6 | 0.0327-0.0545 | 4.1-6.9 | 0.47 |
| tapered_ball_nose | Amana ZrN v8 | tapered | 2 | 1.0 * | tip | 0.01905-0.0508 | 1.9-5.1 | 0.0377-0.0628 | 3.8-6.3 | 0.70 |
| tapered_ball_nose | SpeTool 2D/3D | tapered | 2 | 1.0 * | tip | 0.0254 | 2.5 | 0.0377-0.0628 | 3.8-6.3 | 0.51 |
| tapered_ball_nose | Amana ZrN v8 | tapered | 4 | 1.5 | tip | 0.0127-0.01651 | 0.8-1.1 | 0.0482-0.0804 | 3.2-5.4 | 0.23 |
| tapered_ball_nose | SpeTool 2D/3D | tapered | 2 | 1.5 | tip | 0.0381 | 2.5 | 0.0482-0.0804 | 3.2-5.4 | 0.59 |
| tapered_ball_nose | Amana ZrN v8 | tapered | 2 | 1.5875 | tip | 0.0762-0.127 (IPM gives half) | 4.8-8.0 | 0.0499-0.0832 | 3.1-5.2 | 1.53 |
| tapered_ball_nose | Amana ZrN v8 | tapered | 4 | 1.5875 | tip | 0.0127-0.01651 | 0.8-1.0 | 0.0499-0.0832 | 3.1-5.2 | 0.22 |
| tapered_ball_nose | SpeTool 2D/3D | tapered | 2 | 1.5875 | tip | 0.0381 | 2.4 | 0.0499-0.0832 | 3.1-5.2 | 0.57 |
| ball_nose (D1) | Amana ZrN v1 (LUT) | zrn | 3 | 0.794 * | tip | 0.01905-0.0508 | 2.4-6.4 | 0.0327-0.0545 | 4.1-6.9 | 0.80 |
| ball_nose (D1) | Amana ZrN v1 (LUT) | zrn | 2 | 1.0 * | tip | 0.01905-0.0508 | 1.9-5.1 | 0.0377-0.0628 | 3.8-6.3 | 0.70 |
| ball_nose (D1) | Amana ZrN v1 (LUT) | zrn | 4 | 1.5 | tip | 0.0127-0.01651 | 0.8-1.1 | 0.0482-0.0804 | 3.2-5.4 | 0.23 |
| ball_nose (D1, D2) | Amana ZrN v1 (LUT) | zrn | 2 | 1.5875 | tip | 0.0388-0.0635 (from the IPM) | 2.4-4.0 | 0.0499-0.0832 | 3.1-5.2 | 0.77 |
| flat_end | Amana Spektra | spiral plunge | 2 | 0.794 * | cutting D | 0.0254 softwood / 0.0508 MDF | 3.2 / 6.4 | 0.0327-0.0545 | 4.1-6.9 | 0.58 / 1.16 |
| flat_end | Amana Spektra | spiral plunge | 2 | 1.5 | cutting D | 0.0508 softwood / 0.0762 MDF | 3.4 / 5.1 | 0.0482-0.0804 | 3.2-5.4 | 0.79 / 1.18 |
| flat_end | Amana Spektra | spiral plunge | 2 | 1.5875 | cutting D | 0.0508 softwood / 0.0762 MDF | 3.2 / 4.8 | 0.0499-0.0832 | 3.1-5.2 | 0.76 / 1.14 |
| flat_end | SpeTool spiral | up / down | 2 | 1.5875 | cutting D | 0.0381 up / 0.0254 down (all 3 woods) | 2.4 / 1.6 | 0.0499-0.0832 | 3.1-5.2 | 0.57 / 0.38 |

What the micro data shows:

- **Two vendors print tapered-ball chiploads under 1.5 mm, keyed on the tip.**
  - Amana prints 0.794 mm and 1.0 mm.
  - SpeTool prints 0.5, 0.794 and 1.0 mm.
  - SpeTool 0.5 mm is the only printed wood chipload under 0.79 mm.
- **At a tapered tip of 1.0 mm or less, the printed chip is 1.9-6.4 % of the tip.**
  - The Amana band is 1.9-6.4 %.
  - The SpeTool single value is 2.5-3.6 %.
  - The shipped law on the Onsrud 1/8 in row gives 3.8-8.2 %.
- **The printed mid is 0.47-0.80 of the law mid at every tapered size under 1.5 mm.**
  - So the law is permissive there by 1.25x to 2.1x.
  - The size rule stops this. The direction of the rule agrees with the charts.
- **The acceptance case (1.0 mm tip, 2 flutes) has two printed values:**
  - Amana: 0.019-0.051 mm (mid 0.035).
  - SpeTool: 0.0254 mm.
  - Both are "Wood, MDF, Sign-Foam" rows. Neither vendor prints a hardwood value apart.
- **The Amana 4-flute cell prints 0.0127-0.0165 mm at every size from 1.5 to 3.175 mm.**
  - That is 0.4-1.1 % of D.
  - It is the lowest printed wood chip in the set.
- **Several printed micro values are under the 0.025 mm rubbing floor:**
  - the SpeTool 0.5 and 0.794 mm values;
  - the Amana band minimum at 1.0 mm and under;
  - every Amana 4-flute cell.
  - (The band minimum now outranks the floor, ruling R4 Q9; see EXTRAPOLATION_G8 §1.5.)
- **At 1.5-1.6 mm the flat-end rows print 1.6-5.1 % of D.** The flat-end
  micro rows exist as printed rows. They do not need a size law down to 0.79 mm.

### 1.6 Local shape of the tapered series (ratios between adjacent printed sizes)

| Source | Fl | Size pair mm | Mid pair mm/tooth | Local slope |
|---|---|---|---|---|
| Amana ZrN v8 | 2 | 1.0-1.5875 | 0.0349-0.1016 | +2.31 |
| Amana ZrN v8 | 2 | 1.5875-6.35 | 0.1016-0.2032 | +0.50 |
| Amana ZrN v8 | 3 | 0.794-3.175 | 0.0349-0.0508 | +0.27 |
| Amana ZrN v8 | 3 | 3.175-4.7625 | 0.0508-0.0825 | +1.20 |
| Amana ZrN v8 | 4 | 1.5-1.5875 | 0.0146-0.0146 | +0.00 |
| Amana ZrN v8 | 4 | 1.5875-3.175 | 0.0146-0.0146 | +0.00 |
| SpeTool | 2 | 0.5-0.794 | 0.0178-0.0203 | +0.29 |
| SpeTool | 2 | 0.794-1.0 | 0.0203-0.0254 | +0.97 |
| SpeTool | 2 | 1.0-1.5 | 0.0254-0.0381 | +1.00 |
| SpeTool | 2 | 1.5-1.5875 | 0.0381-0.0381 | +0.00 |
| SpeTool | 2 | 1.5875-2.0 | 0.0381-0.0762 | +3.00 |
| SpeTool | 2 | 2.0-3.0 | 0.0762-0.1016 | +0.71 |
| SpeTool | 2 | 3.0-3.175 | 0.1016-0.1016 | +0.00 |
| SpeTool | 2 | 3.175-4.0 | 0.1016-0.1270 | +0.97 |

Sensitivity: the Amana 2F series without the self-inconsistent 1/16 in cell
gives a two-point slope of +0.95 (1.0 to 6.35 mm). With the cell, the 3-point fit gives +0.85.

The charts print steps, not smooth curves:

- SpeTool doubles the chip between 1.5875 and 2.0 mm.
- SpeTool holds the value flat across the inch-metric pairs (1.5 / 1.5875 and 3.0 / 3.175).
- Amana 3F is almost flat from 0.794 to 3.175 mm, then steep.

Adjacent-size slopes run from 0.00 to 3.00. A fitted exponent smooths over
steps of this size.

### 1.7 Grade c series (not fitted; listed so they are not mistaken for evidence)

| Family | Source | Subfamily | Material | Fl | Role | D -> mid | Slope |
|---|---|---|---|---|---|---|---|
| ball_nose | amana_ball_nose_v7 | solid_carbide (derived, reduced) | softwood / hardwood / mdf | 2 | finish | 3.175 -> 0.024 / 0.018 / 0.0205; 6.0 -> 0.040 / 0.031 / 0.0335 | +0.80 / +0.85 / +0.77 |
| ball_nose | idcwoodcraft (2026-05-30) | ball_nose_spiral | hardwood | 2 | finish | 3.175 -> 0.0346, 6.35 -> 0.0468, 12.7 -> 0.0476 | +0.23 |
| ball_nose | whiteside_fusion360 | ball_nose_spiral | hardwood | 2 | finish | 4.7625 and 6.35 -> 0.1016 | +0.00 |
| flat_end | amana_spektra (repo band) | upcut | softwood / hardwood | 2 | roughing | 3.175 -> 0.040 / 0.030; 6.0 -> 0.0675 / 0.0435 | +0.82 / +0.58 |
| flat_end | idcwoodcraft | compression_spiral | plywood_hardwood | 2 | finish | 3.175 -> 0.0397, 6.35 -> 0.0635 | +0.68 |
| flat_end | whiteside_fusion360 | upcut / downcut | hardwood | 2 | finish | 0.1016 at 3.175-6.35 mm (upcut 0.2822 at 12.7) | +0.76 / +0.00 |
| tapered_ball_nose | amana_zrn_3d_profiling (D3, not on the chart) | 3d_profiling | softwood / hardwood / mdf | 2 | finish | 3.175 -> 0.020 / 0.015 / 0.017; 6.0 -> 0.031 / 0.025 / 0.028 | +0.69 / +0.80 / +0.78 |
| tapered_ball_nose | whiteside_fusion360 (D4) | conical_ball_nose | hardwood | 2 | finish | 1.442 and 2.9867 -> 0.1016 | +0.00 |

### 1.8 The diameter each series keys on

| Series | Key |
|---|---|
| Flat end and ball nose (Onsrud, Amana, Freud, SpeTool spiral) | the cutting diameter |
| Amana ZrN tapered cells (v8 rows; and the LUT D1 rows filed as ball) | the tip. The chart prints the tool "Dia."; the tool numbers prove the tip (46256: "Diameter (D) 1mm, Radius (R) 0.5mm, Angle 5.4°") |
| SpeTool 2D/3D tapered | the tip. Column header "Tip Diameter (MM)" / "(INCH)" |
| Onsrud 77-100 tapered | the tip (the sheet's "cutting diameter"). No series: 3 flutes at 1/8 in and 2 flutes at 1/4 in. The cross-flute two-point slope is +0.58, for reference only |
| Whiteside SC64 / SC66 (grade c) | the Fusion 360 preset D (1.442 / 2.9867 mm), not the tip (1.5875 / 3.175 mm) (D4) |
| V-bit (AMS-159) | the body diameter; the series mixes a 90° and a 60° tool |

Every tapered source keys on the tip: Amana, SpeTool, Onsrud, Harvey
("Use the end diameter of the tool to select the correct Chip Load (IPT)")
and PreciseBits ("Diameter (D) = tip diameter"). No source keys on the
engaged diameter or on the top of the cone. The engine looks up and scales
a tapered ball at the engaged diameter. A law fitted on these rows is a law
in the tip frame.

### 1.9 What the data shows, in plain words

1. **No one exponent describes the vendors.** For flat end mills, the
   vendors fall into two clusters:
   - Onsrud and most Spektra series: about 0.3-0.5.
   - SpeTool, Freud and Amana ZrN flat: about 0.75-1.25.

   The shipped 0.61 sits between the clusters. At a 4-8x step it misses each cluster by up to 1.6x (conservative) or 3.3x (permissive).
2. **Inside one series the fit is good.** For the flat-end series with 3
   or more sizes, r² is 0.78-1.00; most are 0.93 or more. The spread is between
   vendors and series, not inside a series.
3. **Tapered balls in the tip frame are per chart.**
   - Amana 2F: +0.85 (+0.95 without the defect cell).
   - SpeTool 2F: +1.06.
   - Amana 3F: +0.43.
   - Amana 4F: 0.00.

   The flute count changes the shape on the one chart that prints three flute counts.
4. **Under 1.5 mm the charts print less than the law.** Every tapered
   micro cell is 0.47-0.80 of what the shipped law gives from the Onsrud
   1/8 in row. The size rule refuses these cells, and that is the correct direction.
5. **The acceptance case does not need a size law.** Two charts print a
   1.0 mm tapered-tip value. The blockers are three rulings (§3.2):
   - the hardwood-from-"Wood" rows;
   - the D1 tool family;
   - the tip-versus-engaged frame (G5).

What the data does NOT show:

- It does not show a law below 0.5 mm. No wood chart prints a size under 0.5 mm.
- It does not show a hardwood value apart from softwood at any size under
  1.5 mm. Every micro value is a "Wood, MDF, Sign-Foam" row, or a Spektra
  MDF or softwood value.
- It does not show a ball-nose series with 3 or more sizes. The largest
  printed ball span is 2.0x (Amana v7, 3.175-6.35 mm).
- It does not show a bull-nose series of any kind, or a V-bit series with
  3 or more sizes. The only V-bit pair is flat.
- It does not show a chipload in the engaged-diameter frame.
- It does not give a physical reason for a vendor's exponent. The
  vendors' charts can encode different limits: tool deflection gives
  p = 0, tool strength gives p = 1 (VERDICT §4.1). Nothing here tells which limit binds.

## 2. Sources

Hash column: for a PDF source, the sha256 is of the PDF. The stored `.txt` has
its own hash. For an HTML or compiled source, the sha256 is of the stored text.
Verdicts are the verifier reports of 2026-09-24.

| source_id | Vendor | What it prints | Grade | URL reachable | sha256 (of) | Verifier verdicts |
|---|---|---|---|---|---|---|
| amana_zrn_3d_profiling_v8 | Amana | tapered ball carving cells keyed on the tip: 2F 1.0 / 1.5875 / 6.35 mm, 3F 0.794 / 3.175 / 4.7625, 4F 1.5 / 1.5875 / 3.175 (4F same value at every size); one "Wood, MDF, Sign-Foam" row, 18 000 RPM, 1 x D | MDF rows a (2F 1/16 in: b, IPM gives half); wood rows b, derived | yes | 5cdfb9c0... (PDF) | 27 confirmed |
| amana_zrn_3d_profiling (v1) | Amana | the same chart, version 1; same bytes as the LUT copy | - | yes | 52465d1d... (PDF) | no rows; claim confirmed (v1 = v8 in every cited cell) |
| amana_46xxx_tool_identity | Amana via distributors | spec blocks (D, R, angle, flutes) per tool number: proves 46256, 46252, 46291, 46580, 46470, 46472 are tapered | - | yes (dynamic pages; per-page hashes differ, arrowtooling 46580 matches) | 0f4b08e1... (compiled text) | 7 claims confirmed, 1 grade_wrong (46280 angle and flutes are inferred), 1 wrong ("36 of 41" is 34 of 39 unique) |
| spetool_2d3d_tapered_router_bit_chart | SpeTool | "Tip Diameter" 0.5, 1.0, 1.5, 2.0, 3.0, 4.0 mm and 1/32, 1/16, 1/8 in; one value each; one "Wood, MDF, Sign-Foam" row; 2 flutes and in/tooth derived from the feed identity (2.00 on 9 rows) | b (scan; unit and flutes derived) | yes | 0ae9e840... (PDF) | 27 confirmed |
| spetool_carbide_spiral_router_bit_chart | SpeTool | flat spirals 1/16-1/2 in; hardwood, softwood, MDF apart; up, down, compression; one value per cell | b (scan; unit and flutes derived) | yes | 375966a5... (PDF) | 42 confirmed |
| spetool_vgroove_signmaking_chart | SpeTool | two engraving tip widths (0.005, 0.015 in) with the same chip per material; V-bit rows print no size | - | yes | ee494044... (PDF) | no rows; claim confirmed |
| spetool_speed_feeds_index | SpeTool | the tile "2D & 3D Tapered Router Bits" linked to the tapered chart | - | yes | ebd51f43... (stored text, re-hashed by the reconciler; the verifier matched the first copy, 3960721e...) | family claim confirmed live; stored copy lacked the label (fixed) |
| harvey_tapered_ball_SF_723300 | Harvey | metals only; "Use the end diameter of the tool to select the correct Chip Load (IPT)" | - | yes | 39d1f75e... (PDF) | 3 claims confirmed (one sheet of 16 checked) |
| precisebits_sweetspot_test | PreciseBits | a feed-test start rule "F = 0.03 x D x No. flutes x RPM (3% chipload per flute)", the same for all wood hardnesses | - | not verified | 0759b8a9... (text) | not verified |
| precisebits_tapered_ball_carving_250_shank | PreciseBits | no chip load; stepdown 1x tip (2x max); stepover 8 % / 40 % of the tip | - | not verified | e363e07c... (text) | not verified |
| precisebits_tapered_ball_carving_125_shank_4f | PreciseBits | no chip load; 4F 1/16 in tip | - | not verified | 4ff6497c... (text) | not verified |
| precisebits_tapered_stub_pcb | PreciseBits | PCB / metal; feed per rev linear in the tip, 0.25-0.63 mm; "Diameter (D) = tip diameter" | - | not verified | b7663451... (text) | not verified |
| onsrud_77100_series_list | Onsrud | 77-102..108 "3 flute", 77-112..116 "2 flute"; no tip under 1/8 in | - | not verified | 724cdb5a... (text) | not verified |
| shopbot_feeds_speeds_2016 | ShopBot | an Onsrud reprint; 77-102 with "2" flutes (wrong) | - | not verified | 7ad6d740... (PDF) | not verified |
| whiteside_cnc_brochure_1-14-19 | Whiteside | geometry only: SC64 ball 1/16 in, SC66 1/8 in | - | not verified | 00c71165... (PDF) | not verified |
| whiteside_full_catalog_2027 | Whiteside | no chip load; conical ball spirals under "Four Flute" (probable) | - | not verified | 08152897... (PDF) | not verified |
| idcwoodcraft_chipload_csv_2026-09-24 | IDC Woodcraft | community presets with no material; tapered 0.76-3.18 mm tip give 0.0016-0.0054 in/tooth (derived) | c | not verified | 2ac61cdd... (text) | not verified |

Row totals: 96 candidate rows, 96 confirmed, 0 dropped, 0 downgraded.

The verifiers raised four findings. These findings touch the evidence
behind rows, not row values. The reconciler put them in the row notes and
in `sources.json`:

- 46280: the angle and the flute count are derived from its siblings.
- The count "36 of 41" is 34 of 39 unique numbers.
- The stored titles of 46470 and 46472 are cut before "Flute".
- The SpeTool index copy lacked the label. The reconciler appended the
  seven labels from a live copy and hashed the file again.

Dead ends (from `fetch/G1/FETCH_NOTES.md` §6-7):

- A tapered or micro-ball wood chipload that separates hardwood from softwood under 1.5 mm: none found.
- Onsrud 77-100 below 1/8 in: the series has no smaller tip. https://onsrud.com/Series/77-100.asp returns 404.
- PreciseBits: a test method and a 3 % x D start rule, no values.
- Whiteside, Freud, Carbide 3D, Bits & Bits, Kyocera SGS, Datron: no wood chipload chart for tapered or micro tools.
  - The Freud product pages render by script.
  - amanatool.com blocks curl with a Cloudflare challenge.
- Harvey: all 16 tapered-ball sheets are metals only.
- A micro straight ball under 1.5 mm in wood: only Amana 46471, which shares the 2F 1 mm cell with the tapered 46256.
- A micro flat end mill in wood beyond the Spektra rows: the Amana 3F "Flat Bottom" 1/32 in-1 mm cell exists.
  Its tools (46570 / 46571 / 46581) are not identified, so the cell was not transcribed.
- A bull-nose size series: none.
- A V-bit size series with 3 or more sizes: none.
- Amana 46285, 46289, 46496, 46593, 46596: not identified.

## 3. For Phase 3

### 3.1 Candidate forms the trend supports

| Form | What it claims | Range it could claim | Residual it would carry | Where the trend supports it |
|---|---|---|---|---|
| A. Interpolation inside one printed series | log-log between the two nearest printed sizes of the anchor's own series (same source, subfamily, material cell, flutes, role) | only between the printed sizes of that series | the adjacent-size step, stated as the two printed values | every series in §1.3. It makes no claim beyond the chart |
| B. Per-source power law | the anchor series' own slope | the printed span of the series plus one modest step (the PLAN §3 limit) | the series r² and the spread of that source's slopes | the flat-end series with >= 3 sizes (r² 0.78-1.00); SpeTool tapered (0.5-4.0 mm, r² 0.95) |
| C. Generic law (today's 0.61) inside the 0.5x-2x window | one exponent for every family | 0.5x-2x of the row (VERDICT §4.1 regime) | the between-vendor spread: at 2x, (2)^(0.29-0.61) to (2)^(1.25-0.61) = 0.80x-1.56x | the flat-end median (0.48) and the Spektra 16x series (0.57 / 0.62). Not the tapered set (0.00-1.06 by flute count) |
| D. Size-flat | no size effect | the printed sizes | none | Amana tapered 4F; AMS-159 V-bit pair; SpeTool engraving tips (G5 evidence) |

### 3.2 Recommendation per sub-class

| Sub-class | Recommendation | Why |
|---|---|---|
| Flat end, 0.79-19 mm | **per-family (per source)**, form A first, form B inside the source's span; form C only as the 0.5x-2x fallback with the spread on the card | two vendor clusters (0.3-0.5 and 0.75-1.25); in-series fits are tight; micro sizes are printed rows (Spektra 0.79 / 1.5 / 1.59 mm, SpeTool 1.59 mm) |
| Ball nose | **refuse beyond the printed span** (2x); form A inside Amana v7 3.175-6.35 | no series with 3 sizes; 2-size slopes 0.22-0.83. The 3 G1 BallNose cells anchor on a grade c "reduced" row. Their real gap is the finish role (G3), and a printed 3.175 mm hardwood band exists at roughing |
| Tapered ball, tip >= 0.5 mm | **per-chart, tip frame**, printed rows first, form A between a chart's printed tips | four slopes 0.00-1.06 split by flute count. Two charts print the micro sizes directly. The acceptance case has printed cells |
| Tapered ball, tip < 0.5 mm | **refuse** | no wood chart prints a size under 0.5 mm |
| V-bit | **refuse a size law**; hand the question to G5 | the only V-bit pair is size-flat, and the width a V-bit engages is a G5 geometry question. The 1 G1 VBit cell today scales a flat row by 0.66 |
| Bull nose | **refuse** | no series |

A generic law (form C beyond 0.5x-2x) is **not** supported. The vendors
disagree by a factor of 4 in the exponent. The widest series (Spektra 16x)
is the only one near 0.61.

### 3.3 Second witnesses that could exist

- **A physical rule, the bracket.** Deflection-limited p = 0 and
  strength-limited p = 1 (VERDICT §4.1). Every distinct printed slope in this
  set is in [0.00, 1.38]. Nine are above 1.0: Freud (1.05, 1.09, 1.25, 1.38),
  SpeTool spiral (1.04, 1.07, 1.11), SpeTool tapered (1.06) and Amana
  compression hardwood (1.31, 2 sizes).
  - It is a bound, not a value.
  - A Phase 3 fit can cite it to limit a per-source slope.
- **A start rule (grade c).** The PreciseBits sweet-spot rule is
  fz = 0.03 x D per flute, which is linear in D (p = 1.0). It agrees with the
  SpeTool tapered slope (1.06) and the SpeTool spiral cluster.
  - The PreciseBits page calls it a test start value, not a recommendation.
  - It is unverified.
  - At 1.0 mm it gives 0.030 mm/tooth. That is between the SpeTool (0.0254)
    and the Amana (mid 0.035) printed values.
- **The minimum chip (G8).** G8 derives an hmin band of 0.6-3.9 um from
  metals literature (unverified for wood). The smallest printed chip in
  §1.5 is 12.7 um (Amana 4F at 1.5 mm), about 3x the top of that band.
  - A floor could set the lower edge of form B below 0.5 mm.
  - G8 recommends refusal as a generic claim.
- **Simulation.** The engine's deflection model (`feeds::force`) could give
  the chip at constant tip deflection against the tip size for one tapered geometry.
  - That is a model witness in the tip frame.
  - It needs the G5 frame ruling first.
  - It gives a shape, not an absolute value (`feeds/force.rs` says "approximate").
- **A second chart at the same sizes.** Amana and SpeTool both print 0.794 mm and 1.0 mm.
  - At 1.0 mm the SpeTool value (0.0254) is inside the Amana band (0.019-0.051).
  - At 0.794 mm the SpeTool 2F value (0.0203) is inside the Amana 3F band
    (0.019-0.051). The flute counts differ.
  - This is the only two-vendor agreement at a micro size. It is a witness
    for the printed rows, not for a law.

### 3.4 Cells that stay refused under every form

- A tapered tip under 0.5 mm.
- A tapered tip on a flute count that no chart prints at that size (for
  example 3F under 0.794 mm, or 4F under 1.5 mm).
- A ball nose more than 2x from a printed ball row. No ball series spans more than 2x.
- A bull nose at any size other than a printed row.
- A V-bit size transfer until G5 rules.
- Hardwood at a micro size, if ruling G1-R1 applies the 2026-05-02 manifest note.

### 3.5 Rulings for Phase 4

- **G1-R1: hardwood from a shared "Wood, MDF, Sign-Foam" row.**
  - The R5 convention (2026-09-23, ruled) gives hardwood and softwood as derived, grade b.
  - The manifest note on the same Amana chart (2026-05-02) refuses hardwood at sub-1 mm.
  - `verified_rows.json` keeps the 18 hardwood rows (R5). A ruling can delete them.
- **G1-R2: the six LUT rows in D1** (`amana-ball-*-zrn` at 0.794, 1.0, 1.5 and 1.5875 mm).
  - These rows file tapered tools as `ball_nose`.
  - Options: move them to `tapered_ball_nose`, keep both, or replace them with the v8 rows.
  - The 1.0 mm and 0.794 mm cells also list a near-straight tool (46471), so `ball_nose` is half right there.
  - D2: the 1.5875 mm row takes its value from the IPM, but it has grade a.
- **G1-R3: the Whiteside SC64 / SC66 presets.**
  - Key them at the tip (1.5875 / 3.175 mm) or leave them at the preset D (D4).
  - Both are grade c and print one value at both sizes.
- **G1-R4 (shared with G5): the frame.**
  - Every tapered chart keys on the tip. The engine keys on the engaged diameter.
  - A tip-frame claim needs the engine to look up at the tip, or G5 to give a
    rule that converts one frame to the other.
- **G1-R5: the size rule.**
  - Form A / B claims inside a chart's printed tips would replace `micro_extrapolation_refusal` for tapered balls.
  - The rule stays for families with no micro series (ball nose, bull nose, V-bit), or is kept with this trend as its evidence.

## Appendix A1. Slope per series (from `trend_g1.py` §B)

Slope = the log-log OLS slope on the band mid. "min" and "max" = the slopes on
each limit, where both limits are printed. A series with 2 sizes has no residual.

| family | source | subfamily | material | fl | role | n | span | slope mid | r2 | slope min | slope max | note |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| ball_nose | amana_ball_nose_v7 | solid_carbide | hardwood | 2 | roughing | 2 | 2.0x | +0.58 | - | +0.74 | +0.49 | 2 sizes: no residual |
| ball_nose | amana_ball_nose_v7 | solid_carbide | mdf | 2 | roughing | 2 | 2.0x | +0.22 | - | +0.26 | +0.19 | 2 sizes: no residual |
| ball_nose | amana_ball_nose_v7 | solid_carbide | softwood | 2 | roughing | 2 | 2.0x | +0.42 | - | +0.49 | +0.36 | 2 sizes: no residual |
| ball_nose | amana_zrn_3d_profiling | zrn | softwood | 2 | finish | 2 | 1.6x | +0.83 | - | +1.54 | +0.48 | 2 sizes; D1 tapered cells filed as ball; D2: 1.5875 value from IPM |
| ball_nose | amana_zrn_3d_profiling_v8 | zrn | softwood | 3 | finish | 2 | 1.3x | +0.46 | - | +0.54 | +0.41 | 2 sizes: no residual |
| chamfer_vbit | amana_ams159_vgroove_v2 | carbide_tipped_vgroove | softwood | 2 | finish | 2 | 1.3x | +0.00 | n/a | +0.00 | +0.00 | 2 sizes; constant |
| flat_end | amana_compression_spirals_v8 | compression | hardwood | 2 | roughing | 2 | 2.0x | +1.31 | - | +1.31 | +1.31 | 2 sizes: no residual |
| flat_end | amana_compression_spirals_v8 | compression | mdf | 2 | roughing | 2 | 2.0x | +0.86 | - | +0.86 | +0.86 | 2 sizes: no residual |
| flat_end | amana_spektra_spiral_plunge_v24 | spektra_spiral_plunge | hardwood | 2 | roughing | 3 | 2.0x | +0.33 | 0.99 | - | +0.33 | |
| flat_end | amana_spektra_spiral_plunge_v24 | spektra_spiral_plunge | hardwood | 3 | roughing | 3 | 2.0x | +0.33 | 0.99 | - | +0.33 | |
| flat_end | amana_spektra_spiral_plunge_v24 | spektra_spiral_plunge | mdf | 2 | roughing | 13 | 16.0x | +0.57 | 0.96 | - | +0.57 | |
| flat_end | amana_spektra_spiral_plunge_v24 | spektra_spiral_plunge | mdf | 3 | roughing | 6 | 6.0x | +0.46 | 0.78 | - | +0.46 | |
| flat_end | amana_spektra_spiral_plunge_v24 | spektra_spiral_plunge | plywood_hardwood | 2 / 3 | roughing | 3 | 2.0x | +0.33 | 0.99 | - | +0.33 | copy of hardwood (2 series) |
| flat_end | amana_spektra_spiral_plunge_v24 | spektra_spiral_plunge | plywood_softwood | 2 / 3 | roughing | 3 | 2.0x | +0.33 | 0.99 | - | +0.33 | copy of hardwood (2 series) |
| flat_end | amana_spektra_spiral_plunge_v24 | spektra_spiral_plunge | softwood | 2 | roughing | 13 | 16.0x | +0.62 | 0.88 | - | +0.62 | |
| flat_end | amana_spektra_spiral_plunge_v24 | spektra_spiral_plunge | softwood | 3 | roughing | 6 | 6.0x | +0.41 | 0.87 | - | +0.41 | |
| flat_end | amana_zrn_3d_profiling_v8 | zrn_2d3d_carving | softwood | 2 | roughing | 3 | 2.0x | +0.92 | 0.94 | +1.14 | +0.78 | |
| flat_end | freud (2017-08-22) | solid_carbide | hardwood | 2 | finish | 4 | 4.0x | +1.25 | 0.99 | +1.62 | +1.04 | |
| flat_end | freud | solid_carbide | mdf | 2 | roughing | 3 | 4.0x | +1.09 | 0.97 | +1.26 | +0.97 | |
| flat_end | freud | solid_carbide | plywood_hardwood | 2 | finish | 2 | 2.0x | +1.38 | - | +1.58 | +1.22 | 2 sizes: no residual |
| flat_end | freud | solid_carbide | softwood | 2 | finish | 3 | 4.0x | +1.05 | 1.00 | +1.16 | +0.97 | |
| flat_end | onsrud_hard_plywood | 60_000hh_series | plywood_hardwood | 2 | roughing | 2 | 1.3x | +0.37 | - | +0.39 | +0.35 | 2 sizes: no residual |
| flat_end | onsrud_hard_plywood | 60_000lh_series | plywood_hardwood | 2 | roughing | 4 | 2.0x | +0.48 | 1.00 | +0.51 | +0.46 | |
| flat_end | onsrud_hard_plywood | 60_100mw_compression | plywood_hardwood | 2 | finish | 5 | 4.0x | +0.29 | 0.93 | +0.31 | +0.28 | |
| flat_end | onsrud_hard_plywood | 60_350_series | plywood_hardwood | 2 | semi_finish | 3 | 1.7x | +0.41 | 0.99 | +0.39 | +0.43 | |
| flat_end | onsrud_hard_wood | 60_000hh_series | hardwood | 2 | roughing | 2 | 1.3x | +0.41 | - | +0.44 | +0.39 | 2 sizes: no residual |
| flat_end | onsrud_hard_wood | 60_000lh_series | hardwood | 2 | roughing | 4 | 2.0x | +0.38 | 0.97 | +0.40 | +0.36 | |
| flat_end | onsrud_hard_wood | 60_100mw_compression | hardwood | 2 | finish | 4 | 3.0x | +0.40 | 0.99 | +0.43 | +0.38 | |
| flat_end | onsrud_hard_wood | 60_200_downcut | hardwood | 2 | finish | 4 | 3.0x | +0.38 | 0.99 | +0.43 | +0.33 | |
| flat_end | onsrud_mdf | 60_000lh_series | mdf | 2 | roughing | 4 | 2.0x | +0.37 | 0.94 | +0.40 | +0.35 | |
| flat_end | onsrud_mdf | 60_100mw_compression | mdf | 2 | finish | 5 | 4.0x | +0.34 | 0.91 | +0.36 | +0.31 | |
| flat_end | onsrud_mdf | 60_200_downcut | mdf | 2 | finish | 4 | 3.0x | +0.29 | 0.93 | +0.35 | +0.25 | |
| flat_end | onsrud_mdf | 60_350_series | mdf | 2 | semi_finish | 3 | 1.7x | +0.36 | 0.98 | +0.38 | +0.34 | |
| flat_end | onsrud_soft_plywood | 60_000hh_series | plywood_softwood | 2 | roughing | 2 | 1.3x | +0.37 | - | +0.39 | +0.35 | 2 sizes: no residual |
| flat_end | onsrud_soft_plywood | 60_000lh_series | plywood_softwood | 2 | roughing | 4 | 2.0x | +0.48 | 1.00 | +0.51 | +0.46 | |
| flat_end | onsrud_soft_plywood | 60_100mw_compression | plywood_softwood | 2 | finish | 4 | 2.7x | +0.37 | 0.96 | +0.39 | +0.35 | |
| flat_end | onsrud_soft_plywood | 60_350_series | plywood_softwood | 2 | semi_finish | 3 | 1.7x | +0.34 | 1.00 | +0.36 | +0.33 | |
| flat_end | onsrud_soft_wood | 60_000lh_series | softwood | 2 | roughing | 3 | 1.7x | +0.49 | 1.00 | +0.52 | +0.46 | |
| flat_end | onsrud_soft_wood | 60_100mw_compression | softwood | 2 | finish | 2 | 1.5x | +0.38 | - | +0.41 | +0.35 | 2 sizes: no residual |
| flat_end | onsrud_soft_wood | 60_200_downcut | softwood | 2 | finish | 4 | 3.0x | +0.38 | 0.99 | +0.43 | +0.33 | |
| flat_end | spetool_carbide_spiral (G1) | compression | hardwood / mdf / softwood | 2 | roughing | 4 | 4.0x | +0.75 / +0.86 / +0.80 | 0.97 / 0.98 / 0.99 | - | same | printed apart |
| flat_end | spetool_carbide_spiral (G1) | downcut | hardwood / mdf / softwood | 2 | roughing | 5 | 8.0x | +0.94 / +1.11 / +1.07 | 0.97 / 0.96 / 0.97 | - | same | printed apart |
| flat_end | spetool_carbide_spiral (G1) | upcut | hardwood / mdf / softwood | 2 | roughing | 5 | 8.0x | +0.86 / +1.04 / +0.96 | 0.98 / 0.97 / 0.98 | - | same | printed apart |
| tapered_ball_nose | amana_zrn_3d_profiling_v8 (G1) | zrn_2d3d_carving_tapered | Wood, MDF, Sign-Foam (3 copies) | 2 | finish | 3 | 6.3x | +0.85 | 0.85 | +1.07 | +0.72 | tip frame; 1/16 in cell self-inconsistent |
| tapered_ball_nose | amana_zrn_3d_profiling_v8 (G1) | zrn_2d3d_carving_tapered | Wood, MDF, Sign-Foam (3 copies) | 3 | finish | 3 | 6.0x | +0.43 | 0.86 | +0.63 | +0.33 | tip frame |
| tapered_ball_nose | amana_zrn_3d_profiling_v8 (G1) | zrn_2d3d_carving_tapered | Wood, MDF, Sign-Foam (3 copies) | 4 | finish | 3 | 2.1x | +0.00 | n/a | +0.00 | +0.00 | constant |
| tapered_ball_nose | spetool_2d3d_tapered (G1) | spetool_2d3d_tapered | Wood, MDF, Sign-Foam (3 copies) | 2 | finish | 9 | 8.0x | +1.06 | 0.95 | - | +1.06 | tip frame |

The per-point table (min, max, mid at each diameter, for all 63 series) is
`trend_g1.py` §A, in `fetch/G1/trend_g1.out`.
