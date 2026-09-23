# Phase 0: the gap inventory

Date: 2026-09-24. Status: read-only inventory, no number moves.

Inputs: the FM1 matrix `planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv`
(960 cells, on db10818a) and the 389 LUT observations under
`crates/rs_cam_core/data/vendor_lut/observations/`.

Scripts (re-run them after any LUT or matrix change):

- `scripts/inventory.py` maps every cell to a gap group and writes
  `inventory_cells.csv` (one row per cell and group; a cell can be in two
  groups).
- `scripts/lut_axes.py` lists the LUT coverage along each gap axis and
  writes `lut_axes.txt`.

## 1. The cells per group

| Group | Sub-class | Cells | State |
|---|---|---|---|
| none | printed row, in range, with a band | 242 | ship |
| none | formula judged BACKED (FORMULA_BACKING_v2) | 82 | ship |
| none | registry tool rule | 192 | refuse (not a gap) |
| G1 size | row transferred more than 1.4x in diameter | 4 | ship, extrapolated |
| G2h hardness | row transferred more than 1.4x in Janka, same category | 34 | ship, extrapolated |
| G2 category | V-bit in MDF / plywood, no row in the category | 64 | refuse |
| G2 category | ball nose in plywood, no row | 24 | refuse |
| G2 category | ball nose in MDF, formula under 0.5x of the chart | 10 | refuse |
| G3 family | bull nose on parallel / trace finishes | 48 | refuse |
| G3 family | V-bit adaptive | 16 | refuse |
| G3 family | V-bit waterline / steep-shallow | 8 | refuse |
| G3 family | tapered ball on waterline / steep-shallow | 16 | refuse |
| G3 family | tapered ball on profile / trace / pencil, no row in the family | 18 | refuse |
| G3 family | ball nose finishes in MDF / plywood, never judged | 20 | refuse |
| G4 band | printed row with one value (no minimum) | 83 | ship, bandless |
| G5 geometry | V-bit parallel finishes at the engaged width | 16 | refuse |
| G5 geometry | V-bit RPM-only anchor, chipload from the formula | 8 | ship, bandless |
| G6 drill | every drill and pin-drill cell | 80 | refuse |
| G7 physics | no Kc: power on 208 of 448 shipped cells only | (overlay) | ship |
| G8 long tool | the 0.88 / 0.75 long-tool share | 428 | ship (overlay) |

Refusals: 192 tool rule + 320 no basis = 512, as in PLAN §1. Every one of
the 320 maps to one group above (G2 98, G3 126, G5 16, G6 80).

## 2. Corrections to PLAN §2

1. **The 38 "extrapolated" cells are mostly hardness transfers, not size
   transfers.** Only 4 cross the 1.4x threshold on diameter (a 3.175 mm
   ball nose on a 6 mm row; a 6.35 mm V-bit on a 12.7 mm row). The other 34
   cross it on the Janka ratio.
2. **26 of the 34 are an engine-versus-engine mismatch.** An MDF query has
   Janka 1100 (`SheetGoodKind::Mdf::effective_janka_lbf`), but an MDF row
   with no hardness scales from 700 (`vendor_lookup::family_default_janka`).
   So a printed MDF row is scaled by (700 / 1100)^0.5 = 0.80 on an MDF
   query and flagged "extrapolated". The same pair of tables disagrees for
   softwood (600 / 500), hardwood (1450 / 1290) and hardwood plywood
   (1200 Baltic birch / 1100). This is a G2h defect. It moves numbers, so
   it waits for the rulings.
3. **The split above is approximate.** The CSV does not carry the raw
   diameter and hardness ratios; `inventory.py` recomputes them from the
   tool diameter and the two Janka tables. On a tapered ball the lookup uses
   the engaged diameter, not the tip. Proposal: add
   `chipload_diameter_ratio_raw` and `chipload_hardness_ratio_raw` columns
   to the FM1 instrument (an instrument change, not a fix).
4. **The bandless count is 83, as PLAN says.** A simple "no minimum" test
   gives 91. The other 8 are V-bit rows that print an RPM only; their
   chipload comes from the formula. They belong to G5, not G4.

## 3. LUT coverage per axis (from `lut_axes.txt`)

- **G1 size.** 57 series print two or more diameters in one source, tool
  subfamily, material, flute count and role. Flat end mills have 41 (the
  Amana Spektra MDF and softwood series print 13 sizes from 0.79 to
  12.7 mm). Ball nose has 10, tapered ball 5, V-bit 1, bull nose 0.
  - The Onsrud 77-100 tapered rows do not form a series: the 1/8 in size
    has 3 flutes and the 1/4 in size has 2. A two-point fit across them has
    no residual.
  - Sub-1.5 mm rows already in the LUT: Spektra flat 0.79 / 1.5 / 1.59 mm;
    Amana ball 0.794 mm; Amana ZRN ball 1.0 mm (softwood); Whiteside SC64
    tapered ball, 1.442 mm, hardwood, one value 0.1016 mm/tooth, grade c
    (a Fusion 360 preset, not a chart).
  - The acceptance case, wanaka "3D Finish 6" (tp 11, DropCutter, 1.0 mm
    tip tapered ball, hardwood), resolves to a row of 3.175 mm or larger.
    The lookup prefers the grade-a Onsrud rows to the grade-c Whiteside
    1.442 mm preset.
- **G2 category.** 12 flat-end tool series print all five wood categories
  in one source (Spektra, Onsrud). Ball nose prints softwood, hardwood and
  MDF (6 series) but never plywood. Tapered ball prints hardwood, softwood,
  MDF and hardwood plywood (Onsrud). V-bit prints plywood once.
- **G3 family.** Only 3 ball-nose series print a finish value and a
  roughing value that differ (Amana v7). The Onsrud tapered rows repeat one
  printed value in both roles. No bull-nose, V-bit or flat-end series in the
  LUT prints two roles with distinct values.
- **G4 band.** 110 flat-end rows print a band and 104 print one value
  (86 of them Spektra). Median min / max ratio per family: flat 0.88,
  ball 0.62, tapered ball 0.60, bull 0.54, V-bit 0.43.
- **G5 geometry.** 23 V-bit rows with a chipload; 2 with an RPM only.
- **G6 drill.** 0 drill rows.

## 4. What this means for Phase 1 (fetch)

- G1: the LUT can fit flat end mills and ball nose over a wide span. The
  tapered ball cannot be fitted from the LUT. The acceptance case needs
  printed sub-1.5 mm tapered-ball rows (PreciseBits, Amana, Onsrud micro
  charts). That is the G1 fetch's first target.
- G1 and G5 share a frame question: the Onsrud sheet's "cutting diameter"
  is the tip; the lookup scales by the engaged diameter at the cut depth.
  A law fitted on tips and applied at engaged diameters mixes two frames.
  The G1 record must name the diameter it keys on before it fits.
- G2: the flat-end series give per-category ratios now; ball-nose and
  V-bit plywood rows need a fetch.
- G3: the LUT has almost no two-role series; the fetch needs charts that
  print roughing and finish columns for one tool.
- G4: the LUT alone gives the band shape per family.
- G6: a fetch from zero.
- G7, G8: models in the engine (`feeds::force`); the fetch is literature
  Kc for MDF and plywood, and vendor overhang notes.
