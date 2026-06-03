# Onsrud OCR — Provenance Notes (2026-05-30)
## Method
PDFs were fetched from `onsrud.com/images/{Hard,Soft}%20Wood.pdf` and `MDF.pdf`. The 2026-05-29 round logged these as image-only based on prior failure notes; today's re-check found that `pdftotext -layout` successfully extracts the full tabular text. As a cross-check, `pdftoppm -r 300` + `tesseract 5.3.4` was run against the Hard Wood sheet — OCR output matched pdftotext on every spot-checked row (e.g. 60-100C 1/2" = .021-.023). The OCR pipeline is documented for completeness; the extracted numbers in this round come from pdftotext (which is unambiguous), with OCR used only as confirmation.

Even though pdftotext is unambiguous, every row is graded `evidence_grade: b` per the OCR-specialist beat brief — the column-position-to-diameter mapping introduces a small risk of off-by-one alignment that warrants the B classification.

## Sheet-level mapping
| Sheet | URL | Source page | Application table → BEST classifications |
|---|---|---|---|
| Hard Wood | https://www.onsrud.com/images/Hard%20Wood.pdf | 115 | Single Pass: 60-100C; Roughing: 60-000; Finishing: 60-200 |
| Soft Wood | https://www.onsrud.com/images/Soft%20Wood.pdf | 114 | Single Pass: 60-100C; Roughing: 60-000; Finishing: 60-200 |
| MDF | https://www.onsrud.com/images/MDF.pdf | 116 | Single Pass: 60-100C; Roughing: 60-000; Finishing: 60-200 |

## DOC rule (all rows)
Every sheet's right margin states verbatim: `DEPTH OF CUT: 1 x D Use recommended chip load / 2 x D Reduce chip load by 25% / 3 x D Reduce chip load by 50%`. This is reflected in `ap_rule` on every row and is identical to the rule already in the live Onsrud plastic LUT rows.

## Per-row arithmetic and snippet

### `onsrud-hardwood-60-000lh-3_8-roughing`
- Source: hardwood.pdf page 115
- Series: `60-000 (LH)`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.013-.015` in/tooth → 0.3302–0.3810 mm/tooth
- pdftotext line snippet: `60-000 (LH)       1xD                                                                                       .013-.015          .014-.016          .016-.018 .017-.019`

### `onsrud-hardwood-60-000lh-1_2-roughing`
- Source: hardwood.pdf page 115
- Series: `60-000 (LH)`; diameter column: `1/2"` = 12.7 mm (1/2 × 25.4)
- Chipload from chart: `.014-.016` in/tooth → 0.3556–0.4064 mm/tooth
- pdftotext line snippet: `60-000 (LH)       1xD                                                                                       .013-.015          .014-.016          .016-.018 .017-.019`

### `onsrud-hardwood-60-000lh-5_8-roughing`
- Source: hardwood.pdf page 115
- Series: `60-000 (LH)`; diameter column: `5/8"` = 15.875 mm (5/8 × 25.4)
- Chipload from chart: `.016-.018` in/tooth → 0.4064–0.4572 mm/tooth
- pdftotext line snippet: `60-000 (LH)       1xD                                                                                       .013-.015          .014-.016          .016-.018 .017-.019`

### `onsrud-hardwood-60-000lh-3_4-roughing`
- Source: hardwood.pdf page 115
- Series: `60-000 (LH)`; diameter column: `3/4"` = 19.05 mm (3/4 × 25.4)
- Chipload from chart: `.017-.019` in/tooth → 0.4318–0.4826 mm/tooth
- pdftotext line snippet: `60-000 (LH)       1xD                                                                                       .013-.015          .014-.016          .016-.018 .017-.019`

### `onsrud-hardwood-60-000hh-3_8-roughing`
- Source: hardwood.pdf page 115
- Series: `60-000 (HH)`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.015-.017` in/tooth → 0.3810–0.4318 mm/tooth
- pdftotext line snippet: `60-000 (HH)       1xD                                                                                       .015-.017          .017-.019          .019-.021 .021-.023`

### `onsrud-hardwood-60-000hh-1_2-roughing`
- Source: hardwood.pdf page 115
- Series: `60-000 (HH)`; diameter column: `1/2"` = 12.7 mm (1/2 × 25.4)
- Chipload from chart: `.017-.019` in/tooth → 0.4318–0.4826 mm/tooth
- pdftotext line snippet: `60-000 (HH)       1xD                                                                                       .015-.017          .017-.019          .019-.021 .021-.023`

### `onsrud-hardwood-60-100mw-1_8-finish`
- Source: hardwood.pdf page 115
- Series: `60-100mw`; diameter column: `1/8"` = 3.175 mm (1/8 × 25.4)
- Chipload from chart: `.010-.012` in/tooth → 0.2540–0.3048 mm/tooth

### `onsrud-hardwood-60-100mw-3_16-finish`
- Source: hardwood.pdf page 115
- Series: `60-100mw`; diameter column: `3/16"` = 4.7625 mm (3/16 × 25.4)
- Chipload from chart: `.012-.014` in/tooth → 0.3048–0.3556 mm/tooth

### `onsrud-hardwood-60-100mw-1_4-finish`
- Source: hardwood.pdf page 115
- Series: `60-100mw`; diameter column: `1/4"` = 6.35 mm (1/4 × 25.4)
- Chipload from chart: `.014-.016` in/tooth → 0.3556–0.4064 mm/tooth

### `onsrud-hardwood-60-100mw-3_8-finish`
- Source: hardwood.pdf page 115
- Series: `60-100mw`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.016-.018` in/tooth → 0.4064–0.4572 mm/tooth

### `onsrud-hardwood-60-200-1_4-finish`
- Source: hardwood.pdf page 115
- Series: `60-200`; diameter column: `1/4"` = 6.35 mm (1/4 × 25.4)
- Chipload from chart: `.005-.007` in/tooth → 0.1270–0.1778 mm/tooth
- pdftotext line snippet: `60-200         1xD                                                                   .005-.007           .006-.008          .007-.009                      .008-.010`

### `onsrud-hardwood-60-200-3_8-finish`
- Source: hardwood.pdf page 115
- Series: `60-200`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.006-.008` in/tooth → 0.1524–0.2032 mm/tooth
- pdftotext line snippet: `60-200         1xD                                                                   .005-.007           .006-.008          .007-.009                      .008-.010`

### `onsrud-hardwood-60-200-1_2-finish`
- Source: hardwood.pdf page 115
- Series: `60-200`; diameter column: `1/2"` = 12.7 mm (1/2 × 25.4)
- Chipload from chart: `.007-.009` in/tooth → 0.1778–0.2286 mm/tooth
- pdftotext line snippet: `60-200         1xD                                                                   .005-.007           .006-.008          .007-.009                      .008-.010`

### `onsrud-hardwood-60-200-3_4-finish`
- Source: hardwood.pdf page 115
- Series: `60-200`; diameter column: `3/4"` = 19.05 mm (3/4 × 25.4)
- Chipload from chart: `.008-.010` in/tooth → 0.2032–0.2540 mm/tooth
- pdftotext line snippet: `60-200         1xD                                                                   .005-.007           .006-.008          .007-.009                      .008-.010`

### `onsrud-hardwood-60-800-3_8-roughing`
- Source: hardwood.pdf page 115
- Series: `60-800`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.017-.019` in/tooth → 0.4318–0.4826 mm/tooth
- pdftotext line snippet: `60-800         1xD                                                                                       .017-.019          .019-.021          .021-.023 .023-.025`

### `onsrud-softwood-60-000lh-3_8-roughing`
- Source: softwood.pdf page 114
- Series: `60-000 (LH)`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.013-.015` in/tooth → 0.3302–0.3810 mm/tooth
- pdftotext line snippet: `60-000 (LH)       1xD                                                                                          .013-.015          .015-.017          .017-.019 .019-.021`

### `onsrud-softwood-60-000lh-1_2-roughing`
- Source: softwood.pdf page 114
- Series: `60-000 (LH)`; diameter column: `1/2"` = 12.7 mm (1/2 × 25.4)
- Chipload from chart: `.015-.017` in/tooth → 0.3810–0.4318 mm/tooth
- pdftotext line snippet: `60-000 (LH)       1xD                                                                                          .013-.015          .015-.017          .017-.019 .019-.021`

### `onsrud-softwood-60-000lh-5_8-roughing`
- Source: softwood.pdf page 114
- Series: `60-000 (LH)`; diameter column: `5/8"` = 15.875 mm (5/8 × 25.4)
- Chipload from chart: `.017-.019` in/tooth → 0.4318–0.4826 mm/tooth
- pdftotext line snippet: `60-000 (LH)       1xD                                                                                          .013-.015          .015-.017          .017-.019 .019-.021`

### `onsrud-softwood-60-000hh-3_8-roughing`
- Source: softwood.pdf page 114
- Series: `60-000 (HH)`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.016-.018` in/tooth → 0.4064–0.4572 mm/tooth
- pdftotext line snippet: `60-000 (HH)       1xD                                                                                          .016-.018          .018-.020          .020-.022 .022-.024`

### `onsrud-softwood-60-100mw-1_8-finish`
- Source: softwood.pdf page 114
- Series: `60-100mw`; diameter column: `1/8"` = 3.175 mm (1/8 × 25.4)
- Chipload from chart: `.011-.013` in/tooth → 0.2794–0.3302 mm/tooth

### `onsrud-softwood-60-100mw-3_16-finish`
- Source: softwood.pdf page 114
- Series: `60-100mw`; diameter column: `3/16"` = 4.7625 mm (3/16 × 25.4)
- Chipload from chart: `.013-.015` in/tooth → 0.3302–0.3810 mm/tooth

### `onsrud-softwood-60-200-1_4-finish`
- Source: softwood.pdf page 114
- Series: `60-200`; diameter column: `1/4"` = 6.35 mm (1/4 × 25.4)
- Chipload from chart: `.005-.007` in/tooth → 0.1270–0.1778 mm/tooth
- pdftotext line snippet: `60-200         1xD                                                                      .005-.007           .006-.008          .007-.009                      .008-.010`

### `onsrud-softwood-60-200-3_8-finish`
- Source: softwood.pdf page 114
- Series: `60-200`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.006-.008` in/tooth → 0.1524–0.2032 mm/tooth
- pdftotext line snippet: `60-200         1xD                                                                      .005-.007           .006-.008          .007-.009                      .008-.010`

### `onsrud-softwood-60-200-1_2-finish`
- Source: softwood.pdf page 114
- Series: `60-200`; diameter column: `1/2"` = 12.7 mm (1/2 × 25.4)
- Chipload from chart: `.007-.009` in/tooth → 0.1778–0.2286 mm/tooth
- pdftotext line snippet: `60-200         1xD                                                                      .005-.007           .006-.008          .007-.009                      .008-.010`

### `onsrud-softwood-60-200-3_4-finish`
- Source: softwood.pdf page 114
- Series: `60-200`; diameter column: `3/4"` = 19.05 mm (3/4 × 25.4)
- Chipload from chart: `.008-.010` in/tooth → 0.2032–0.2540 mm/tooth
- pdftotext line snippet: `60-200         1xD                                                                      .005-.007           .006-.008          .007-.009                      .008-.010`

### `onsrud-softwood-60-350-3_8-semi_finish`
- Source: softwood.pdf page 114
- Series: `60-350`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.017-.019` in/tooth → 0.4318–0.4826 mm/tooth
- pdftotext line snippet: `60-350         1xD                                                                                          .017-.019          .019-.021                      .021-.023`

### `onsrud-softwood-60-800-3_8-roughing`
- Source: softwood.pdf page 114
- Series: `60-800`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.017-.019` in/tooth → 0.4318–0.4826 mm/tooth
- pdftotext line snippet: `60-800         1xD                                                                                          .017-.019          .019-.021          .021-.023 .023-.025`

### `onsrud-mdf-60-000lh-3_8-roughing`
- Source: mdf.pdf page 116
- Series: `60-000 (LH)`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.012-.014` in/tooth → 0.3048–0.3556 mm/tooth
- pdftotext line snippet: `60-000 (LH)       1xD                                                                                       .012-.014          .013-.015           .014-.016 .016-.018`

### `onsrud-mdf-60-000lh-1_2-roughing`
- Source: mdf.pdf page 116
- Series: `60-000 (LH)`; diameter column: `1/2"` = 12.7 mm (1/2 × 25.4)
- Chipload from chart: `.013-.015` in/tooth → 0.3302–0.3810 mm/tooth
- pdftotext line snippet: `60-000 (LH)       1xD                                                                                       .012-.014          .013-.015           .014-.016 .016-.018`

### `onsrud-mdf-60-000lh-5_8-roughing`
- Source: mdf.pdf page 116
- Series: `60-000 (LH)`; diameter column: `5/8"` = 15.875 mm (5/8 × 25.4)
- Chipload from chart: `.014-.016` in/tooth → 0.3556–0.4064 mm/tooth
- pdftotext line snippet: `60-000 (LH)       1xD                                                                                       .012-.014          .013-.015           .014-.016 .016-.018`

### `onsrud-mdf-60-000lh-3_4-roughing`
- Source: mdf.pdf page 116
- Series: `60-000 (LH)`; diameter column: `3/4"` = 19.05 mm (3/4 × 25.4)
- Chipload from chart: `.016-.018` in/tooth → 0.4064–0.4572 mm/tooth
- pdftotext line snippet: `60-000 (LH)       1xD                                                                                       .012-.014          .013-.015           .014-.016 .016-.018`

### `onsrud-mdf-60-000hh-3_8-roughing`
- Source: mdf.pdf page 116
- Series: `60-000 (HH)`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.017-.019` in/tooth → 0.4318–0.4826 mm/tooth
- pdftotext line snippet: `60-000 (HH)       1xD                                                                                       .017-.019          .018-.020           .020-.022 .023-.025`

### `onsrud-mdf-60-100mw-1_8-finish`
- Source: mdf.pdf page 116
- Series: `60-100mw`; diameter column: `1/8"` = 3.175 mm (1/8 × 25.4)
- Chipload from chart: `.010-.012` in/tooth → 0.2540–0.3048 mm/tooth

### `onsrud-mdf-60-100mw-3_16-finish`
- Source: mdf.pdf page 116
- Series: `60-100mw`; diameter column: `3/16"` = 4.7625 mm (3/16 × 25.4)
- Chipload from chart: `.010-.012` in/tooth → 0.2540–0.3048 mm/tooth

### `onsrud-mdf-60-100mw-1_4-finish`
- Source: mdf.pdf page 116
- Series: `60-100mw`; diameter column: `1/4"` = 6.35 mm (1/4 × 25.4)
- Chipload from chart: `.013-.015` in/tooth → 0.3302–0.3810 mm/tooth

### `onsrud-mdf-60-100mw-3_8-finish`
- Source: mdf.pdf page 116
- Series: `60-100mw`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.014-.016` in/tooth → 0.3556–0.4064 mm/tooth

### `onsrud-mdf-60-100mw-1_2-finish`
- Source: mdf.pdf page 116
- Series: `60-100mw`; diameter column: `1/2"` = 12.7 mm (1/2 × 25.4)
- Chipload from chart: `.016-.018` in/tooth → 0.4064–0.4572 mm/tooth

### `onsrud-mdf-60-100c-3_8-finish`
- Source: mdf.pdf page 116
- Series: `60-100c`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.017-.019` in/tooth → 0.4318–0.4826 mm/tooth

### `onsrud-mdf-60-200-1_4-finish`
- Source: mdf.pdf page 116
- Series: `60-200`; diameter column: `1/4"` = 6.35 mm (1/4 × 25.4)
- Chipload from chart: `.004-.006` in/tooth → 0.1016–0.1524 mm/tooth
- pdftotext line snippet: `60-200         1xD                                                                   .004-.006           .005-.007          .005-.007                       .006-.008`

### `onsrud-mdf-60-200-3_8-finish`
- Source: mdf.pdf page 116
- Series: `60-200`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.005-.007` in/tooth → 0.1270–0.1778 mm/tooth
- pdftotext line snippet: `60-200         1xD                                                                   .004-.006           .005-.007          .005-.007                       .006-.008`

### `onsrud-mdf-60-200-1_2-finish`
- Source: mdf.pdf page 116
- Series: `60-200`; diameter column: `1/2"` = 12.7 mm (1/2 × 25.4)
- Chipload from chart: `.005-.007` in/tooth → 0.1270–0.1778 mm/tooth
- pdftotext line snippet: `60-200         1xD                                                                   .004-.006           .005-.007          .005-.007                       .006-.008`

### `onsrud-mdf-60-200-3_4-finish`
- Source: mdf.pdf page 116
- Series: `60-200`; diameter column: `3/4"` = 19.05 mm (3/4 × 25.4)
- Chipload from chart: `.006-.008` in/tooth → 0.1524–0.2032 mm/tooth
- pdftotext line snippet: `60-200         1xD                                                                   .004-.006           .005-.007          .005-.007                       .006-.008`

### `onsrud-mdf-60-300-3_8-semi_finish`
- Source: mdf.pdf page 116
- Series: `60-300`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.017-.019` in/tooth → 0.4318–0.4826 mm/tooth
- pdftotext line snippet: `60-300         1xD                                                                                       .017-.019          .018-.020           .020-.022 .023-.025`

### `onsrud-mdf-60-350-3_8-semi_finish`
- Source: mdf.pdf page 116
- Series: `60-350`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.014-.016` in/tooth → 0.3556–0.4064 mm/tooth
- pdftotext line snippet: `60-350         1xD                                                                                       .014-.016          .016-.018           .017-.019 .019-.021`

### `onsrud-mdf-60-350-1_2-semi_finish`
- Source: mdf.pdf page 116
- Series: `60-350`; diameter column: `1/2"` = 12.7 mm (1/2 × 25.4)
- Chipload from chart: `.016-.018` in/tooth → 0.4064–0.4572 mm/tooth
- pdftotext line snippet: `60-350         1xD                                                                                       .014-.016          .016-.018           .017-.019 .019-.021`

### `onsrud-mdf-60-350-5_8-semi_finish`
- Source: mdf.pdf page 116
- Series: `60-350`; diameter column: `5/8"` = 15.875 mm (5/8 × 25.4)
- Chipload from chart: `.017-.019` in/tooth → 0.4318–0.4826 mm/tooth
- pdftotext line snippet: `60-350         1xD                                                                                       .014-.016          .016-.018           .017-.019 .019-.021`

### `onsrud-mdf-60-800-3_8-roughing`
- Source: mdf.pdf page 116
- Series: `60-800`; diameter column: `3/8"` = 9.525 mm (3/8 × 25.4)
- Chipload from chart: `.017-.019` in/tooth → 0.4318–0.4826 mm/tooth
- pdftotext line snippet: `60-800         1xD                                                                                       .017-.019          .019-.021           .021-.023 .023-.025`
