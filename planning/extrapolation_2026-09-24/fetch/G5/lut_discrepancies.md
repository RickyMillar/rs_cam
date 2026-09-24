# G5: LUT rows that do not match their cited document

Date: 2026-09-24. Agent: G5 fetch (engaged geometry). Read-only: no LUT file
is changed. Each finding names the observation_id, the LUT value and the
printed cell. The stored texts are under `sources/`; `sources.json` has the
hashes.

## D1. `amana_vbit.json`: six rows cite the insert V-groove chart, and no number in them is on it

Cited source: `amana_insert_v_groove_v16`
(`sources/amana_insert_v_groove_v16.txt`, `..._raw.txt`, PDF sha256
`d3da2d5f…`). Column order checked on a rendered image of the page.

What the chart prints (every row, verbatim from the raw text):

- Columns: Hardwood, Softwood, Plywood/Chipboard, MDF, Plastic, Foam. Each
  column is Feed Rate IPM, Chip Load Per Tooth, Ramp Down.
- 90° tools are 1 flute at 18,000 RPM, for example
  `RC-1141 90° 1 18,000 40" .0024" 20" 40" .0024" 20" 40" .0024" 20" 90" .0047" 45" 40" .0024" 20" 90" .0047" 45"`.
- One value per cell, no band. Hardwood = Softwood = Plywood = .0024 in
  (0.061 mm). MDF = .0047 in (0.119 mm) on the 40°-110° rows and on
  RC-1029, and .0048 in (0.122 mm) on the other 120°-160° rows.
- No tool diameter, no depth of cut, no RPM range, no ap or ae.

| observation_id | LUT value | Printed on the chart | Verdict |
|---|---|---|---|
| amana-vbit-softwood-trace-6000-2f | 90°, 2 flutes, d 6.0, RPM 12,000-22,000, chip 0.040-0.085 mm, ap 0.4-1.5, ae 0.2-1.2 | 90° tools are 1 flute, 18,000 RPM, .0024 in = 0.061 mm, no diameter, no ap/ae | not on chart |
| amana-vbit-hardwood-trace-6000-2f | 2 f, d 6.0, RPM 12,000-20,000, chip 0.028-0.060 mm | same cell as softwood: 0.061 mm | not on chart |
| amana-vbit-mdf-trace-6000-2f | 2 f, d 6.0, RPM 12,000-19,000, chip 0.030-0.065 mm | MDF .0047 in = 0.119 mm (about 2x the wood cell) | not on chart; MDF sits BELOW wood in the LUT and ABOVE it on the chart |
| amana-vbit-plywood-hardwood-trace-6000-2f | 2 f, d 6.0, chip 0.030-0.060 mm | Plywood/Chipboard .0024 in = 0.061 mm | not on chart |
| amana-vbit-acrylic-trace-6000-2f | 2 f, d 6.0, chip 0.020-0.045 mm | Plastic .0024 in = 0.061 mm | not on chart |
| amana-vbit-softwood-contour-12000-2f | 90°, 2 f, d 12.0, contour semi_finish, chip 0.060-0.110 mm | no diameter, no contour/semi-finish row | not on chart |

The rows carry `row_kind exact`, `evidence_grade a` and
`accessed_on 2026-03-10`. The manifest entry has no stored text and no hash,
so no one can show which document the numbers came from. The MDF inversion
is the one R5 flagged (EVIDENCE 3.1-3, 5.2-10).

Replacement candidates: the 105 `x-g5-amana-insert-*` rows in
`candidate_rows.json` (hardwood, softwood, MDF grade a; plywood grade b
because the chart has one Plywood/Chipboard column).

## D2. `amana_vbit.json`: two Whiteside rows cite a page that prints RPM only

Cited source: `whiteside_vgroove_collection`
(https://www.whitesiderouterbits.com/collections/120-v-groove; stored
`sources/whiteside_120_vgroove_collection.txt`).

What the page prints: "Recommended RPM: 14,000-16,000 (max 18,000)" and the
sizes "1/4"SH, 3/4"CD" and "1/2"SH, 1-1/2"CD". It prints no chip load and
no 12 mm size.

| observation_id | LUT value | Printed | Verdict |
|---|---|---|---|
| whiteside-vbit-hardwood-trace-12000-2f | 120°, d 12.0, RPM 12,000-18,000, chip 0.040-0.080 mm, ap/ae ranges, grade b exact | RPM 14,000-16,000 (max 18,000); no chip load; CD 3/4 in or 1-1/2 in | chip band and size not on page |
| whiteside-vbit-mdf-trace-12000-2f | 120°, d 12.0, chip 0.045-0.085 mm | same | chip band and size not on page |

The manifest's own coverage note says "V-bit RPM bands". The chip loads are
not from this page.

## D3. Minor: single printed values stored as a zero-width band

`amana-vgroove-softwood-trace-60deg-2f`, `amana-vgroove-hardwood-trace-60deg-2f`
and `amana-vgroove-softwood-trace-90deg-2f` store
`chipload_min_mm_tooth = chipload_max_mm_tooth = 0.0762`. The AMS-159 chart
prints one value, `0.003"`, for 60° and 90°. The manifest convention (see
`amana_spektra_spiral_plunge_v24`) is "Single values are encoded as
chipload_max_mm_tooth only". The number is right; the encoding makes a
printed single value look like a printed band (G4 matter).

## D4. Minor: Whiteside 1550 RPM range

`whiteside-1550-vgroove-60deg-half-rpm` stores RPM 16,000-24,000,
nominal 20,000. The page prints "Recommended RPM: 16,000-20,000 (max
24,000)". The LUT range reaches the printed maximum, not the recommended
upper value. No chip load is involved.

## Rows checked and found correct

- The AMS-159 rows (18°, 30°, 45° 1 flute 0.0762-0.1778 mm; 60°, 90° 2 flute
  0.0762 mm) match the chart.
- The Spektra engraving rows (15°, 30°, 45° 0.0762-0.1778 mm; 120°
  0.0508-0.1524 mm; tip widths 0.127 and 1.0668 mm) match the chart. The
  `diameter_mm 6.35` on some rows is not printed on the chart; the manifest
  says it comes from the SKU specification.
- The Onsrud sheets in `fetch/G5/pdf/` have the same sha256 as the manifest,
  and their text is identical to the LUT copies.
