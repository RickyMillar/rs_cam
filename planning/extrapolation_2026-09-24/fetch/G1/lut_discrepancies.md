# G1 fetch: LUT rows that do not match the charts

Date: 2026-09-24. Author: the G1 research agent. Status: findings only. No
LUT file was changed.

Each item names the LUT row, the stored text that disagrees, and the effect.

## D1. Six Amana ZrN rows file tapered tools as `ball_nose`

The Amana ZrN 2D/3D carving chart keys each section on a list of tool
numbers. The distributor product pages identify the tools
(`sources/amana_46xxx_identity.txt`). Most of the small ball tools on the
chart are tapered ball carving bits, and the chart keys them on the tip.

| LUT row | Chart cell | Tools that the chart lists in the cell |
|---|---|---|
| `amana-ball-softwood-parallel-1000-2f-zrn`, `amana-ball-mdf-parallel-1000-2f-zrn` | 2 Flute Ball Nose, 1mm | 46256 (5.4° taper, 1 mm tip, 2 flutes) and 46471 (0.10°, a straight ball) |
| `amana-ball-softwood-parallel-0794-3f-zrn`, `amana-ball-mdf-parallel-0794-3f-zrn` | 3 Flute Ball Nose, 1/32"-1mm | 46280, 46291, 46580 (6.2° taper, 1/32" tip), 46470 (6.2° taper, 0.8 mm tip); v8 adds 46473 (0.5 mm tip) |
| `amana-ball-softwood-parallel-1500-4f-zrn` | 4 Flute Ball Nose & Flat Bottom, 1.5mm | 46472 (5.4° taper, 1.5 mm tip) only |
| `amana-ball-softwood-parallel-1587-2f-zrn` | 2 Flute Ball Nose, 1/16" | 46252 (5.5° taper, 1/16" tip) only |

Evidence lines (verbatim from `sources/amana_46xxx_identity.txt`):

- `title: Amana 46256 - SC Carving 5.4° Taper Ball Nose, 1mm D x 0.5mm R x 1-7/64" CH x 1/4" Shank, ZrN | FastoolNow.com`
  with `spec: Flutes 2`.
- `title: Amana 46472 Metric CNC 2D/3D Carving 5.4 Deg Tapered Angle Ball Nose x 1.5 D x 0.75 R x 25 CH x 6 Shank x 75mm Long x 4 | FastoolNow.com`
- `title: Amana 46280 - SC Carving 6.2° Taper Ball Nose Bit, 1/32" D x 1/64" R x 1" CH x 1/4" Shank, ZrN | FastoolNow.com`

Effect: the lookup asks for `tapered_ball_nose` on a tapered tool. It does not
see these rows. So the acceptance case (wanaka "3D Finish 6", a 1.0 mm tip
tapered ball) resolves to a row of 3.175 mm or larger, although Amana prints a
1 mm tapered-ball cell. The values in these rows match the chart. Only the
tool family is wrong, or at best half right for the 1 mm and 0.794 mm cells,
where the chart gives one value to a tapered and a straight tool. The
grade also differs: the LUT softwood rows are `exact`, grade a; the candidate
softwood and hardwood rows from the same cells are `derived`, grade b, by the
R5 convention for a shared "Wood, MDF, Sign-Foam" row.

`candidate_rows.json` holds the same cells as `tapered_ball_nose` rows
(`x-g1-amana-zrn-tapered-*`), with the tool numbers in the notes. The
reconciler decides whether the old rows keep `ball_nose`, move, or both.

## D2. `amana-ball-softwood-parallel-1587-2f-zrn`: kind and grade disagree

The row has `row_kind: derived` and `evidence_grade: a`. Its value comes from
the printed IPM (55" - 90" at 18,000 RPM and 2 flutes gives 0.00153" -
0.0025"), not from the printed chip load (0.003" - 0.005"). The chart is
self-inconsistent in this cell. A derived value with grade a breaks the grade
convention that R5 used (derived rows are grade b or c). The candidate rows
keep the printed chip load and carry grade b with the conflict in the notes.

## D3. The Amana 3D profiling tapered rows have printed neighbours now

`amana_3d_profiling.json` holds ten `tapered_ball_nose` rows at 3.175 mm and
6.0 mm with 2 flutes (`derived`, grade c, already marked by R5). The chart
prints no 2-flute tapered cell at 1/8". It prints these tapered cells instead:

- 2 flute, 6mm - 1/4" column: 0.007" - 0.009" (46283, 3° taper, 1/4" tip).
- 3 flute, 1/8" - 3.2mm column: 0.0015" - 0.0025" (46284, 46286, 46287,
  46288, 46474).
- 4 flute, 1/8" column: 0.0005" - 0.00065" (46583).

The derived rows (for example hardwood 3.175 mm, 0.01 - 0.02 mm/tooth) are
not on the chart. The printed cells are in `candidate_rows.json`.

## D4. Whiteside SC64 and SC66 presets use a diameter that is not the tip

| LUT row | `diameter_mm` | `tip_diameter_mm` | `flute_count` |
|---|---|---|---|
| `whiteside-sc64-conical-ball-nose-spiral-fusion360` | 1.442 | 1.5875 | 2 |
| `whiteside-sc66-conical-ball-nose-spiral-fusion360` | 2.9867 | 3.175 | 2 |

The Whiteside brochure prints the geometry
(`sources/whiteside_cnc_brochure_1-14-19.txt`):

```
  Part #    Radius                           Length
                        Dia.      Angle                SC75        Pencil        45°        2”
  SC64      1/32”      1/16”       11°       2-1/2”    SC76      1/32” Flat      45°        2”
  SC66      1/16”       1/8”        7°       2-1/2”                                                 SC76
```

The column header is "Radius / Ball Dia. / Included Angle / Overall Length".
So the SC64 ball diameter is 1/16" (1.5875 mm) and the SC66 ball diameter is
1/8" (3.175 mm). The full catalog heads the conical ball nose spirals "Four
Flute" (`sources/whiteside_full_catalog_2027.txt`, lines near "CONICAL BALL
NOSE SPIRALS"). The layout does not tie that heading to SC64 and SC66 without
doubt. Treat the 4-flute reading as probable, not proven.

Effect: the lookup keys these rows at 1.442 mm and 2.9867 mm (the Fusion 360
preset's own diameter parameter), not at the tip. The Phase 0 inventory lists
"Whiteside SC64 tapered ball, 1.442 mm" as the smallest tapered row; on the
tip frame it is 1.5875 mm. Both rows are grade c presets and print one value
(0.1016 mm/tooth) at both sizes.

## D5. Checked and found correct

- Onsrud 77-100 flute counts. The LUT uses 3 flutes at 1/8" and 2 flutes at
  1/4". Onsrud's own series list agrees: 77-102 to 77-108 "3 flute", 77-112 to
  77-116 "2 flute" (`sources/onsrud_77100_series_list.txt`). The ShopBot chart
  reprints 77-102 with "2" flutes (`sources/shopbot_feeds_speeds_2016.txt`);
  that reprint is wrong, not the LUT.
- The Amana ZrN v1 PDF at the manifest URL has the same sha256 as the LUT
  copy (52465d1d...). The LUT transcription of the 1 mm, 0.794 mm and 1.5 mm
  values matches the chart.
