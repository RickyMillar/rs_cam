# Onsrud OCR — Gaps (2026-05-30)

Companion to `onsrud_ocr.json` and `onsrud_ocr_provenance.md`.

## PDFs attempted

| PDF | URL | Result |
|---|---|---|
| Hard Wood (HP) | https://www.onsrud.com/images/Hard%20Wood.pdf | `pdftotext -layout` extracted full tabular text; OCR (tesseract 5.3.4) confirmed values on spot-checked rows. **Extracted.** |
| Soft Wood (SW) | https://www.onsrud.com/images/Soft%20Wood.pdf | pdftotext extracted full table. **Extracted.** |
| MDF (CW) | https://www.onsrud.com/images/MDF.pdf | pdftotext extracted full table. **Extracted.** |

### Note on "image-only" claim

The 2026-05-29 `onsrud_whiteside_gaps.md` flagged these wood-router sheets as
image-only based on prior failure notes. The 2026-05-30 re-check found that
`pdftotext -layout` against the binary PDF (fetched via `curl -A "Mozilla/5.0"`)
extracts the tabular data cleanly. Cross-check via `pdftoppm -r 300` + tesseract
confirmed the pdftotext values on spot-checked rows (e.g. Hard Wood 60-100C 1/2"
= .021-.023). The original "image-only" tag did NOT hold — same situation as the
plastics sheets in the 2026-05-29 round.

OCR pipeline was still set up per the beat brief (tesseract 5.3.4 extracted from
deb to `/tmp/tesseract_extract/`); used only as a cross-check rather than the
primary source. Rows tagged `evidence_grade: b` regardless, to acknowledge the
column-position-to-diameter mapping in the parser (small risk of off-by-one
column assignment under unusual whitespace patterns; mitigated by parsing
multiple sheets with identical headers and getting consistent results).

## Per-row OCR uncertainties (rows NOT recorded)

### Series 40-000, Hard Wood, 3/8" column

pdftotext line snippet (L27): `40-000 1xD  .006-.008 .006-.008 .007-.009  .008-.010 .008-.010 .009-.007`

The trailing `.009-.007` parses to chipload min > max, which is physically
impossible. Tesseract OCR for the same row reads `008.010 .008-.010 .009-.007`,
confirming the source PDF likely renders this cell as `.009-.011` but both
pdftotext and tesseract misread the final digit as `7`. Without an unambiguous
reading, **GAP — not recorded**. Affects: hardwood-40-000-3_8.

(This row would have been outside the BEST/BETTER selection anyway — 40-000 is
not on the Application table's recommended list — so the impact is zero on the
selected row set.)

## Rows extracted from chart but staged out for review (validator-flagged)

The validator's `chipload max suspicious (>0.5 mm)` threshold flagged 57 of the
104 extracted rows. These are real, verbatim Onsrud chart values — the high-feed
spiral series (60-100C, 60-300, 60-100MW) and the large-diameter rows for
60-000/60-800 publish chiploads in the 0.5–0.8 mm/tooth range. These values are
correct for Onsrud's target market (industrial high-feed CNC routers running
~24,000 RPM with 3-4 kW spindles), but exceed what's typical for the
Shapeoko-class machines the LUT primarily serves.

These rows are staged out of `onsrud_ocr.json` pending project-side review on
whether to:
1. Adopt them as-is (they're primary-source verbatim) and accept that the LUT
   spans both light-router and industrial-router regimes,
2. Carry them in a separate observation file flagged for the gate to skip when
   matching against low-power machines, OR
3. Drop them as out-of-scope.

The 57 staged-out rows live at `/tmp/onsrud_ocr/high_value_rows.json` and are
listed below for traceability:

| Sheet | Series | Diameter | Chipload (mm/tooth) |
|---|---|---|---|
| hardwood | 60-000HH | 3/4" | 0.5334–0.5842 |
| hardwood | 60-000HH | 5/8" | 0.4826–0.5334 |
| hardwood | 60-100C | 1/2" | 0.5334–0.5842 |
| hardwood | 60-100C | 3/4" | 0.6350–0.6858 |
| hardwood | 60-100C | 3/8" | 0.4826–0.5334 |
| hardwood | 60-100C | 5/8" | 0.5842–0.6350 |
| hardwood | 60-100MW | 1/2" | 0.4572–0.5080 |
| hardwood | 60-100MW | 3/4" | 0.5588–0.6096 |
| hardwood | 60-100MW | 5/8" | 0.5080–0.5588 |
| hardwood | 60-300 | 1/2" | 0.6604–0.7112 |
| hardwood | 60-300 | 3/4" | 0.7620–0.8128 |
| hardwood | 60-300 | 3/8" | 0.6096–0.6604 |
| hardwood | 60-300 | 5/8" | 0.7112–0.7620 |
| hardwood | 60-350 | 1/2" | 0.5080–0.5588 |
| hardwood | 60-350 | 3/4" | 0.6096–0.6604 |
| hardwood | 60-350 | 3/8" | 0.4572–0.5080 |
| hardwood | 60-350 | 5/8" | 0.5588–0.6350 |
| hardwood | 60-800 | 1/2" | 0.4826–0.5334 |
| hardwood | 60-800 | 3/4" | 0.5842–0.6350 |
| hardwood | 60-800 | 5/8" | 0.5334–0.5842 |
| mdf | 60-000HH | 1/2" | 0.4572–0.5080 |
| mdf | 60-000HH | 3/4" | 0.5842–0.6350 |
| mdf | 60-000HH | 5/8" | 0.5080–0.5588 |
| mdf | 60-100C | 1/2" | 0.4572–0.5080 |
| mdf | 60-100C | 3/4" | 0.5842–0.6350 |
| mdf | 60-100C | 5/8" | 0.5080–0.5588 |
| mdf | 60-100MW | 3/4" | 0.4826–0.5334 |
| mdf | 60-100MW | 5/8" | 0.4572–0.5080 |
| mdf | 60-300 | 1/2" | 0.4572–0.5080 |
| mdf | 60-300 | 3/4" | 0.5842–0.6350 |
| mdf | 60-300 | 5/8" | 0.5080–0.5588 |
| mdf | 60-350 | 3/4" | 0.4826–0.5334 |
| mdf | 60-800 | 1/2" | 0.4826–0.5334 |
| mdf | 60-800 | 3/4" | 0.5842–0.6350 |
| mdf | 60-800 | 5/8" | 0.5334–0.5842 |
| softwood | 60-000HH | 1/2" | 0.4572–0.5080 |
| softwood | 60-000HH | 3/4" | 0.5588–0.6096 |
| softwood | 60-000HH | 5/8" | 0.5080–0.5588 |
| softwood | 60-000LH | 3/4" | 0.4826–0.5334 |
| softwood | 60-100C | 1/2" | 0.6604–0.7112 |
| softwood | 60-100C | 3/4" | 0.7620–0.8128 |
| softwood | 60-100C | 3/8" | 0.6096–0.6604 |
| softwood | 60-100C | 5/8" | 0.7112–0.7620 |
| softwood | 60-100MW | 1/2" | 0.5588–0.6096 |
| softwood | 60-100MW | 1/4" | 0.4572–0.5080 |
| softwood | 60-100MW | 3/4" | 0.6604–0.7112 |
| softwood | 60-100MW | 3/8" | 0.5080–0.5588 |
| softwood | 60-100MW | 5/8" | 0.6096–0.6604 |
| softwood | 60-300 | 1/2" | 0.6604–0.7112 |
| softwood | 60-300 | 3/4" | 0.7620–0.8128 |
| softwood | 60-300 | 3/8" | 0.6096–0.6604 |
| softwood | 60-300 | 5/8" | 0.7112–0.7620 |
| softwood | 60-350 | 1/2" | 0.4826–0.5334 |
| softwood | 60-350 | 3/4" | 0.5334–0.5842 |
| softwood | 60-800 | 1/2" | 0.4826–0.5334 |
| softwood | 60-800 | 3/4" | 0.5842–0.6350 |
| softwood | 60-800 | 5/8" | 0.5334–0.5842 |

## Other Onsrud /images/*.pdf data sheets not addressed this round

From the 2026-05-29 gaps file, additional Onsrud `/images/` PDFs may exist
(e.g. specialty materials like aluminum composites, MDO/HDO plywood). This
beat focused on the three wood-router primary sheets per the brief; other
sheets are not in the 2026-05-29 gaps inventory and were not enumerated.

Strategy for future rounds: enumerate `https://www.onsrud.com/images/` via
a directory crawl (currently returns 403 to direct listing) or scrape the
Onsrud Technical Data page links to discover additional sheet URLs.
