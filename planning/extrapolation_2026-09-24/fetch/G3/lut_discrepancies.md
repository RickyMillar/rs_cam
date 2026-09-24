# G3 LUT discrepancies

Date: 2026-09-24. Scope: rows and Phase 0 claims that touch the G3 axis
(operation family and pass role). This file does not edit the LUT.

No LUT row that claims `exact` carries a number that differs from its chart.
The items below are about labels and about one Phase 0 statement.

## 1. The Phase 0 "two-role" series are derived rows, not printed roles

`lut_axes.txt` (G3 section) and `INVENTORY.md` §3 say: "Only 3 ball-nose
series print a finish value and a roughing value that differ (Amana v7)".

- The Amana Spiral Ball Nose v7 chart prints no operation and no pass role.
  The stored text (`fetch/G3/sources/amana_ball_nose_v7.txt`, identical to the
  LUT copy) has one chip-load band per material and diameter. Its only
  condition is "Depth of Cut: 1 x Tool Diameter".
- The LUT rows with `pass_role: finish` that cite `amana_ball_nose_v7`
  (`amana-ball-{softwood,hardwood,mdf}-parallel-{3175,6000}-2f`,
  `amana-ball-hardwood-scallop-6000-2f`) are `derived` / grade `c`. The LUT
  marks them correctly. Their values (for example hardwood 3.175 mm
  0.012-0.024 mm) are not on the chart. The printed 1/8 in hardwood band is
  0.003-0.005 in (0.076-0.127 mm), which the `pocket`/`adaptive` rows carry.
- So the "distinct values" pairs compare a derived finish row with a printed
  roughing row. They are not vendor evidence of a finish/roughing ratio. The
  same applies to the `amana_zrn_3d_profiling` "finish vs semi_finish distinct
  values" pair: all ten `amana-tapered-*` rows that cite it (parallel/finish
  and scallop/semi_finish) are `derived` / `c`.
- Recommendation for the G3 record: the LUT holds **zero** printed series
  whose two roles carry distinct values. (The 32 Onsrud 77-100 rows are
  printed and sit in finish and roughing roles, but with one value copied
  into both.) `lut_axes.py` should group by `row_kind` (or skip `derived`) before
  it reports "distinct values".

## 2. The four `bull_nose` rows now have a printed alternative

The LUT has four `bull_nose` rows (`onsrud-bull-*` in
`amana_3d_profiling.json`). All are `derived` / grade `c`, and their notes say
that the cited Onsrud sheets print no bull or corner-radius series. That is
correct.

A printed wood chart for a corner-radius tool now exists (Amana 46460 /
46462, `fetch/G3/sources/amana_corner_radius_spiral_plunge.txt`):

| Diameter | Softwood (in/tooth) | Hardwood | MDF |
|---|---|---|---|
| 1/4 in (6.35 mm), 1/16 R | .007-.009 | .005-.007 | .006-.008 |
| 1/2 in (12.7 mm), 1/8 R | .009-.011 | .007-.009 | .008-.010 |

The derived LUT row `onsrud-bull-hardwood-adaptive-6000-2f` gives
0.032-0.060 mm at 6.0 mm. The printed hardwood band at 6.35 mm is
0.127-0.178 mm, about 3-4 times higher. The softwood row (0.055-0.095 mm)
is also about 2.4-3.2 times below the printed band (0.178-0.229 mm).
The reconciler decides whether the printed rows replace the derived ones.

## 3. Checks that found no discrepancy

- The Onsrud Hard Wood sheet at `https://onsrud.com/images/Hard%20Wood.pdf`
  still serves sha256 `f123d5f6…` (the manifest hash) on 2026-09-24.
- The 2018 image-only Hard Wood sheet on disk (`pdf/onsrud_h_wood2.pdf`,
  sha256 `e3df6499…`, URL not recorded by the earlier run) prints 77-100 at
  1/8 in .003-.005 and at 1/4 in .005-.007. These equal the LUT
  `onsrud-hardwood-77-100-*` rows.
- The IDC Woodcraft PDF gives 60 / (22000 x 2) = 0.0346 mm (1/8 in ball) and
  70 / (19000 x 2) = 0.0468 mm (1/4 in ball). These equal the LUT rows
  `idcwoodcraft-bn-18-ball-nose` and `idcwoodcraft-bn-14-ball-nose`.
