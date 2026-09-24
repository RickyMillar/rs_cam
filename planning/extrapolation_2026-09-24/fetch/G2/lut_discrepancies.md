# G2 fetch: LUT discrepancies

Date: 2026-09-24. Author: G2 research agent. Status: findings only. No LUT
file, manifest or Rust file changed.

## 1. The LUT omits the Onsrud 37-series V-bit rows

- The five Onsrud sheets that the LUT stores
  (`crates/rs_cam_core/data/vendor_lut/sources/onsrud_*_cutting_data.txt`)
  print four V-bit and engraving series on every sheet: 37-00/37-20,
  37-50, 37-60 and 37-80.
- No LUT observation cites a 37-series row. The only `37-` match in
  `observations/` is in `amana_long_tail.json`, and it is not an Onsrud row.
- The manifest `coverage_notes` of the five sheets do not name the
  37-series.
- Effect: the matrix refuses V-bit cells in MDF and plywood (INVENTORY §1,
  64 cells) while the stored texts print a V-bit band for MDF, hard plywood
  and soft plywood.
- The re-downloaded PDFs of 2026-09-24 have the same `pdf_sha256` as the
  manifest, and `pdftotext -layout` gives byte-identical text. The LUT copies
  are current.
- `candidate_rows.json` holds the transcription (55 rows from the five
  sheets, 21 rows from the two laminated tables on catalog page 119).

## 2. Freud chart: partly transcribed, no stored text

Source: `freud_router_bit_feed_and_speed_for_cnc_20170822`.

- The manifest entry has no `stored_text` and no `pdf_sha256`. The G2 folder
  now holds both (`sources/freud_router_bit_feed_and_speed_for_cnc_20170822.txt`,
  sha `ff1a29ea…71976`).
- I checked each LUT Freud row against the chart. Every chipload value that
  the LUT holds matches the printed cell.
- The LUT leaves these printed cells out of the solid carbide table:
  - Softwood 3/8 in (.016"-.019"), MDF/Particle Board 3/8 in (.018"-.021").
  - Plywood 1/8 in (.003"-.005") and 3/8 in (.015"-.018").
  - The whole Laminated Particle Board column.
  - The whole Acrylics/Soft Plastic column.
- The LUT leaves out the whole second table ("Carbide Tipped Straight and
  Profile Bits"). That table names no V-groove tool, so it must not become a
  `chamfer_vbit` row.
- Label question: `freud-solid-carbide-quarter-hard-plastic` has
  `material_family: acrylic` and the value .005"-.008". That value is the
  "Solid Surface/ Hard Plastic" column. The chart puts acrylic in the
  "Acrylics/ Soft Plastic" column (.006"-.009" at 1/4 in). The rendered page
  shows the column headers; see FETCH_NOTES §4.
- The chart prints "Plywood" with no hardwood/softwood split. The LUT maps it
  to `plywood_hardwood` only.

## 3. Spektra flat end mill: two printed columns, not five categories

Target 3 of the G2 task lists the Spektra series as "held in all five
categories". The manifest entry for `amana_spektra_spiral_plunge_v24` says
that the chart prints one "Wood/Plywood" column and one "MDF/Laminate"
column. The LUT agrees: only the MDF rows are `exact`/`a`. The hardwood,
softwood and both plywood rows are `derived`/`b` copies of the one
Wood/Plywood value. So the Spektra series gives one printed ratio
(MDF : Wood/Plywood) per size, not five category values. The G2 trend must
not count the four derived copies as four witnesses.
