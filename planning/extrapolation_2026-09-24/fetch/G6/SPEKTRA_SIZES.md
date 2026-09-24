# G6: the other Amana Spektra sizes (B5 clean-up, data only)

Date: 2026-09-25. Scope: B5_PLAN decision 2, "the other Spektra sizes, 1/4 in
and up, are a later transcription job". No Rust edit, no cargo, no LUT edit.

Files:

- `scripts/spektra_candidate_rows.py`: the typed transcription (it does not
  read the PDF). It writes `candidate_rows_spektra_all.json`.
- `candidate_rows_spektra_all.json`: 27 candidate rows, the typed table of
  all 39 chart lines (20 size keys), `ramp_down_check` and `already_in_lut`.
- `scripts/spektra_verify.py`: the independent check. It parses the PDF
  text again with `pdftotext -layout`. Its output is in `spektra_verify.out`.

The source is `amana_spektra_spiral_plunge_v24`. The PDF is
`fetch/G6/pdf/amana_spektra_v24.pdf`, and its sha256 is
`5b6fef854b2cf6b422e2eb5bbc86e29b4dcab5b19195b7ff228b70f99cf86d3a`. This is
the same value as in the manifest.

## 1. What the chart prints

The header prints "CNC Operating Spindle Speed: 18,000 RPM / Depth of Cut: 1 x
Tool Diameter" for all rows. The values below are in inches (Feed in IPM,
Chip in in/tooth, Ramp Down in IPM). The chart puts "**" (the breakage
warning) on the 1/32 in, 1.5 mm, 1/16 in, 3/32 in and 0.023 in rows.

| Z | Size | mm | W/P Feed | W/P Chip | W/P Ramp | MDF Feed | MDF Chip | MDF Ramp | LUT today |
|---|---|---|---|---|---|---|---|---|---|
| 2 | 1/32" ** | 0.794 | 35 | .0010 | 17.5 | 70 | .0020 | 35 | long_tail: sw, mdf |
| 2 | 1.5mm ** | 1.5 | 70 | .0020 | 35 | 105 | .0030 | 52.5 | long_tail: sw, mdf |
| 2 | 1/16" ** | 1.588 | 70 | .0020 | 35 | 105 | .0030 | 52.5 | long_tail: sw, mdf |
| 2 | 3/32" ** | 2.381 | 80 | .0023 | 40 | 160 | .0046 | 80 | long_tail: sw, mdf |
| 2 | 3mm | 3.0 | 145 | .0040 | 72.5 | 180 | .0050 | 90 | long_tail: sw, mdf |
| 2 | 1/8" | 3.175 | 145 | .0040 | 72.5 | 180 | .0050 | 90 | flat_end: all 5 |
| 2 | 3/16" | 4.763 | 180 | .0050 | 90 | 215 | .0060 | 107.5 | long_tail: sw, mdf |
| 2 | 5mm | 5.0 | 180 | .0050 | 90 | 215 | .0060 | 107.5 | long_tail: sw, mdf |
| 2 | 6mm | 6.0 | 180 | .0050 | 90 | 215 | .0060 | 107.5 | flat_end: all 5 |
| 2 | 1/4" | 6.35 | 180 | .0050 | 90 | 215 | .0060 | 107.5 | flat_end: all 5 |
| 2 | 3/8" | 9.525 | 230 | .0064 | 115 | 390 | .0108 | 195 | long_tail: sw, mdf |
| 2 | 12mm | 12.0 | 200 | .0057 | 100 | 350 | .0096 | 175 | long_tail: sw, mdf |
| 2 | 1/2" | 12.7 | 200 | .0057 | 100 | 350 | .0096 | 175 | long_tail: sw, mdf |
| 3 | 0.023" ** | 0.584 | 55 | .0010 | 27.5 | 110 | .0020 | 55 | none |
| 3 | 1/8" | 3.175 | 215 | .0040 | 72 | 270 | .0050 | 90 | flat_end: all 5 |
| 3 | 6mm | 6.0 | 270 | .0050 | 90 | 325 | .0060 | 109 | flat_end: all 5 |
| 3 | 1/4" | 6.35 | 270 | .0050 | 90 | 325 | .0060 | 109 | flat_end: all 5 |
| 3 | 3/8" | 9.525 | 345 | .0064 | 115 | 580 | .0108 | 195 | long_tail: sw, mdf |
| 3 | 1/2" | 12.7 | 300 | .0057 | 100 | 500 | .0096 | 167 | long_tail: sw, mdf |
| 3 | 3/4" | 19.05 | 330 | .009 | 110 | 360 | .010 | 120 | long_tail: sw, mdf |

The chart prints two flute counts only: 2 (13 sizes) and 3 (7 sizes). The
3 Flute block prints 1/2 in before 3/8 in. The tool numbers are in the
typed table of the JSON file.

## 2. The chart compared with the LUT

- **Correction to the prompt and to INVENTORY §3.** The prompt names only
  `amana_flat_end.json`. `amana_long_tail.json` also holds 26 Spektra rows
  (softwood and MDF, pocket, roughing) at every size except 1/8 in, 6 mm,
  1/4 in and the 3 Flute 0.023 in. Thus no size is missing from the LUT.
  The missing cells are hardwood, plywood_softwood and plywood_hardwood at
  9 sizes: 27 cells.
- **The numbers.** All 86 Spektra rows in the LUT agree with the printed
  chip load x 25.4. The verifier found no number discrepancy.
- **The grade (13 rows).** The softwood rows in `amana_long_tail.json` are
  `exact` / `a`. The R5 rule makes a shared Wood/Plywood column `derived` /
  `b` (as in `amana_flat_end.json`). This is a LUT discrepancy. The
  orchestrator must correct it.
- **The shape of the long_tail rows.** They have no `hardness_kind` or
  `hardness_value`, so the lookup uses `family_default_janka` (the G2h
  defect of INVENTORY §2.2). They also have no `notes`. They set
  `chipload_min_mm_tooth` = `chipload_max_mm_tooth`, and `material_label`
  is "wood/plywood". The 27 new rows use the `amana_flat_end.json` shape.
- **Not printed.** `amana-flat-softwood-contour-12000-2f` (0.05-0.08 mm) and
  the other `derived` / `c` rows that cite this chart are not on the chart.
  The manifest already records this (R5). Printed 12 mm: 0.1448 mm.

## 3. The candidate rows

`candidate_rows_spektra_all.json` has 27 rows. Each has the
`amana_flat_end.json` Spektra shape. The rows are in the (pocket, roughing)
family, `tool_subfamily` `spektra_spiral_plunge`, `rpm_nominal` 18000,
`row_kind` `derived`, `evidence_grade` `b` (the Wood/Plywood column applied
to one family). The hardness values are 1450 (hardwood), 600
(plywood_softwood) and 1000 (plywood_hardwood).

| Z | Sizes (mm) | Chip (mm/tooth) |
|---|---|---|
| 2 | 1.5 / 3.0 / 5.0 / 9.525 / 12.0 / 12.7 | 0.0508 / 0.1016 / 0.127 / 0.1626 / 0.1448 / 0.1448 |
| 3 | 9.525 / 12.7 / 19.05 | 0.1626 / 0.1448 / 0.2286 |

No row has an adaptive copy. The prompt files the rows under (pocket,
roughing) only, and `DRILL_RULES.home` reads that family only. The
`amana_flat_end.json` rows have adaptive copies. The orchestrator decides
if the new rows get adaptive copies too.

No row field holds a Ramp Down value. The values are in `ramp_down_check`.

## 4. The verification

`python3 scripts/spektra_verify.py` ends with exit code 0 and
"transcription failures: 0". It checks these items:

1. The PDF sha256 agrees with the manifest.
2. Every typed number and tool number agrees with the parsed PDF text. No
   parsed line is missing from the typed table.
3. Each candidate row: diameter, flute count, chip load (printed x 25.4),
   column, grade, RPM and filing. No field carries a Ramp Down.
4. Each `ramp_down_check` entry agrees with the parsed Feed and Ramp Down.

A test with two planted faults (one chip load and one typed Feed changed)
gave exit code 1 and named both faults.

The chart's own identities, on all 40 cells (20 sizes x 2 columns):

- **Ramp Down = Feed Rate / Z** is true on 33 cells to 0.01 IPM. The other
  7 cells are:
  - 3 Flute 1/8 in, W/P: 72 printed, 71.67 calculated (rounded to a whole
    number).
  - 3 Flute 1/2 in, MDF: 167 printed, 166.67 calculated (rounded).
  - 3 Flute 6 mm and 1/4 in, MDF: 109 printed, 108.33 calculated.
  - 3 Flute 3/8 in, MDF: 195 printed, 193.33 calculated (+0.9 %).
  - 3 Flute 0.023 in (both columns): 27.5 and 55 printed. These are Feed / 2,
    not Feed / 3. The chart applies the 2-flute rule on this 3-flute row. The
    row is not a candidate and is not in the LUT.
- **Feed = 18000 x Z x Chip.** The ratio is 0.965-1.019 on 38 cells. On the
  3 Flute 3/4 in row it is 0.679 (W/P) and 0.667 (MDF). The printed feed
  agrees with about 12,000 RPM (330 / (3 x .009) = 12,222; 360 / (3 x .010) =
  12,000). The chart does not print that RPM. A row with `rpm_nominal` 18000
  and chip 0.2286 mm gives 486 IPM, and Amana prints 330 IPM. The two
  `amana_long_tail.json` rows at 19.05 mm already have this fault.

## 5. The G6 drill range

Today `DRILL_RULES` has `range_mm: (3.175, 6.0)` and `flutes: [2, 3]`.

- The Ramp Down identity is true at every size from 3.0 to 12.7 mm, on both
  flute counts. The worst cell there is +0.9 % (3 Flute 3/8 in, MDF).
- Above 12.7 mm, only the 3 Flute 3/4 in row is printed. Its feed does not
  agree with the 18,000 RPM header (ratio 0.67-0.68).
- Below 3.0 mm, only 2 Flute rows are printed. They all have the breakage
  warning ("**").

Proposal: `range_mm: (3.0, 12.7)`. With it, the drill claim covers the
printed metric 3 mm row and every size up to 1/2 in. `flutes` stays
`[2, 3]`. Every size in the range has a printed row for 2 flutes. The 3 Flute
sizes are 3.175, 6.0, 6.35, 9.525 and 12.7 mm.

When the range changes, the orchestrator must also change these items (I did
not edit them):

- `drill.rs:86` and the doc comments at `drill.rs:74-78` and `:321`;
- the texts at `support.rs:111`, `support.rs:250`, `ramp.rs:152` and
  `ramp.rs:560`;
- `feeds/CLAUDE.md:22`;
- the test messages at `the_dial_holds_the_load_and_never_cuts_the_feed_fm7.rs:344`
  and `clueless_cells_refuse_and_backed_cells_ship_fm5.rs:102` (a 6.35 mm
  flat plunge then ships);
- `literature_matrix/cells.toml:451`, `:2304` (3.0 mm) and `:3962` (12 mm).
  These refusals then ship. The 2.0 mm cell at `:3433` still refuses.

## 6. The questions for the operator

1. **Load these 24 rows, and widen the G6 range to 3.0-12.7 mm?** The 24
   rows are hardwood, plywood_softwood and plywood_hardwood at 2 Flute 1.5,
   3, 5, 9.525, 12 and 12.7 mm and at 3 Flute 9.525 and 12.7 mm. The 1.5 mm
   rows are side rows only; they are outside the proposed drill range.
   Recommendation: yes.
2. **Load the 3 rows at 3 Flute 3/4 in?** Recommendation: no. The printed
   feed does not agree with 18,000 RPM. Also mark the two `amana_long_tail.json`
   rows at 19.05 mm with this fault, or remove them.
3. **Correct the 13 `amana_long_tail.json` softwood rows** from `exact` /
   `a` to `derived` / `b` (R5), and give the 26 long_tail rows the
   `hardness_value` that the `amana_flat_end.json` rows have? Recommendation:
   yes. The hardness value changes the numbers on those rows (G2h).
