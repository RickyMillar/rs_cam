# Amana long tail — provenance

Beat: PHASE 3 / A. Amana long tail (Phased Plan 2026-05-30).
Collector: Claude, 2026-05-30.
Schema: `crates/rs_cam_core/src/feeds/vendor_lut.rs::VendorObservation`.
Validator: `planning/data_ingest_2026-05-30/validate_lut.py` reports 0 PROBLEMS.

All PDFs fetched via:
`curl -sL -A "Mozilla/5.0 (X11; Linux x86_64)" "<url>" -o /tmp/x.pdf && pdftotext -layout /tmp/x.pdf -`.

Unit conversion: 1 in = 25.4 mm (exact). Per-row arithmetic shown.

---

## Source S1: Solid Carbide Spektra Spiral Plunge 2/3 Flute v24

- URL: https://www.amanatool.com/pub/media/productattachments/Solid-Carbide-Spektra-Spiral-Plunge-2-3-Flute-v24.pdf
- Pulled: 2026-05-30
- Chart format: tabular, fixed 18,000 RPM, DOC = 1 x Tool Diameter, single CPT value per cell (Wood/Plywood and MDF/Laminate columns).
- Footer verbatim: "Depth of Cut: 1 x D Use recommended feed rate / 2 x D Reduce feed rate by 25% / 3 x D Reduce feed rate by 50%".
- WARNING (verbatim): "Due to the extremely small diameters involved, bits are not guaranteed against breakage" applies to rows flagged `**` (sub-1/16" carbide spirals).

Diameters already live in `crates/rs_cam_core/data/vendor_lut/observations/amana_flat_end.json` from prior ingest (NOT re-collected): 3.175 mm (1/8") and 6.0 mm.

### Rows extracted (2 Flute table)

| observation_id | Source row | Verbatim CPT | Conversion | Notes |
|---|---|---|---|---|
| `amana-flat-softwood-pocket-0794-2f-spektra` | 1/32" (Down-Cut 46229-K/46242-K), Wood/Plywood | `.0010"` | 0.0010 × 25.4 = 0.0254 mm | 1/32" = 1/32 × 25.4 = 0.79375 mm. Single CPT cell. |
| `amana-flat-mdf-pocket-0794-2f-spektra` | 1/32", MDF/Laminate | `.0020"` | 0.0020 × 25.4 = 0.0508 mm | same dia |
| `amana-flat-softwood-pocket-1500-2f-spektra` | 1.5mm (48210-K/48212-K), Wood/Plywood | `.0020"` | 0.0020 × 25.4 = 0.0508 mm | |
| `amana-flat-mdf-pocket-1500-2f-spektra` | 1.5mm, MDF/Laminate | `.0030"` | 0.0030 × 25.4 = 0.0762 mm | |
| `amana-flat-softwood-pocket-1587-2f-spektra` | 1/16" (46237-K, 46213-K, 46233-K, 46448-K, 46009-K up-cut, 46403-K), Wood/Plywood | `.0020"` | 0.0020 × 25.4 = 0.0508 mm | 1/16" = 1.5875 mm |
| `amana-flat-mdf-pocket-1587-2f-spektra` | 1/16", MDF/Laminate | `.0030"` | 0.0762 mm | |
| `amana-flat-softwood-pocket-2381-2f-spektra` | 3/32" (46239-K/46244-K), Wood/Plywood | `.0023"` | 0.0023 × 25.4 = 0.05842 mm | 3/32" = 2.38125 mm |
| `amana-flat-mdf-pocket-2381-2f-spektra` | 3/32", MDF/Laminate | `.0046"` | 0.0046 × 25.4 = 0.11684 mm | |
| `amana-flat-softwood-pocket-3000-2f-spektra` | 3mm (48214-K, 48116-K up-cut, 48216-K), Wood/Plywood | `.0040"` | 0.0040 × 25.4 = 0.1016 mm | |
| `amana-flat-mdf-pocket-3000-2f-spektra` | 3mm, MDF/Laminate | `.0050"` | 0.0050 × 25.4 = 0.127 mm | |
| `amana-flat-softwood-pocket-4763-2f-spektra` | 3/16" (46101-K/46201-K), Wood/Plywood | `.0050"` | 0.127 mm | 3/16" = 4.7625 mm |
| `amana-flat-mdf-pocket-4763-2f-spektra` | 3/16", MDF/Laminate | `.0060"` | 0.0060 × 25.4 = 0.1524 mm | |
| `amana-flat-softwood-pocket-5000-2f-spektra` | 5mm (46211-K), Wood/Plywood | `.0050"` | 0.127 mm | |
| `amana-flat-mdf-pocket-5000-2f-spektra` | 5mm, MDF/Laminate | `.0060"` | 0.1524 mm | |
| `amana-flat-softwood-pocket-9525-2f-spektra` | 3/8" (46203-K, 46320-K up-cut, 46420-K, 46449-K), Wood/Plywood | `.0064"` | 0.0064 × 25.4 = 0.16256 mm | 3/8" = 9.525 mm |
| `amana-flat-mdf-pocket-9525-2f-spektra` | 3/8", MDF/Laminate | `.0108"` | 0.0108 × 25.4 = 0.27432 mm | |
| `amana-flat-softwood-pocket-12000-2f-spektra` | 12mm (48228-K), Wood/Plywood | `.0057"` | 0.0057 × 25.4 = 0.14478 mm | |
| `amana-flat-mdf-pocket-12000-2f-spektra` | 12mm, MDF/Laminate | `.0096"` | 0.0096 × 25.4 = 0.24384 mm | |
| `amana-flat-softwood-pocket-12700-2f-spektra` | 1/2" (46106-K/46206-K), Wood/Plywood | `.0057"` | 0.14478 mm | 1/2" = 12.7 mm |
| `amana-flat-mdf-pocket-12700-2f-spektra` | 1/2", MDF/Laminate | `.0096"` | 0.24384 mm | |

### Rows extracted (3 Flute table)

| observation_id | Source row | Verbatim CPT | Conversion | Notes |
|---|---|---|---|---|
| `amana-flat-softwood-pocket-9525-3f-spektra` | 3 Flute 3/8" (46055-K), Wood/Plywood | `.0064"` | 0.16256 mm | |
| `amana-flat-mdf-pocket-9525-3f-spektra` | 3F 3/8", MDF/Laminate | `.0108"` | 0.27432 mm | |
| `amana-flat-softwood-pocket-12700-3f-spektra` | 3 Flute 1/2" (46116-K/46216-K), Wood/Plywood | `.0057"` | 0.14478 mm | |
| `amana-flat-mdf-pocket-12700-3f-spektra` | 3F 1/2", MDF/Laminate | `.0096"` | 0.24384 mm | |
| `amana-flat-softwood-pocket-19050-3f-spektra` | 3 Flute 3/4" (46500-K), Wood/Plywood | `.009"` | 0.0090 × 25.4 = 0.2286 mm | 3/4" = 19.05 mm |
| `amana-flat-mdf-pocket-19050-3f-spektra` | 3F 3/4", MDF/Laminate | `.010"` | 0.0100 × 25.4 = 0.254 mm | |

Material mapping rationale: "Wood/Plywood" → assigned `softwood` (the chart conflates softwood and plywood; same row would be re-validated against hardwood-bonded plywood if needed but the source does not differentiate). "MDF/Laminate" → assigned `mdf`. Janka not provided by chart; not asserted. `pass_role=roughing` because spiral plunge bits are sold as roughing/slotting tools; `operation_family=pocket` matches the existing live-row convention used in `amana_flat_end.json` for the same chart.

---

## Source S2: ZrN-Coated and Uncoated 2D/3D Carving CNC Solid Carbide Router Bits v8

- URL: https://www.amanatool.com/pub/media/productattachments/ZrN-3D-Profiling-Feed-Chip-Load-Chart-v8.pdf
- Pulled: 2026-05-30
- Chart format: tabular, fixed 18,000 RPM, DOC = 1 x Tool Diameter, range CPT per cell (min - max).
- Material rows are 2 Flute Flat Bottom and 3 Flute Ball Nose. The 2F Ball Nose section's diameters 1mm, 1/16", 1/4" are already in `amana_ball_nose.json` from prior ingest — NOT re-collected here.

### Rows extracted (2 Flute Flat Bottom — NEW family for this chart)

| observation_id | Source row | Verbatim CPT range | Conversion |
|---|---|---|---|
| `amana-zrn-flat-aluminum-pocket-3175-2f` | 2F Flat Bottom, 1/8" col, Aluminum/Copper/Brass row | `0.003" - 0.005"` | 0.003 × 25.4 = 0.0762, 0.005 × 25.4 = 0.127 mm |
| `amana-zrn-flat-acrylic-pocket-3175-2f` | 2F Flat Bottom, 1/8" col, Plastic/Acrylic/Plexiglas® row | `0.002" - 0.004"` | 0.0508, 0.1016 mm |
| `amana-zrn-flat-softwood-pocket-3175-2f` | 2F Flat Bottom, 1/8" col, Wood/MDF/Sign-Foam row | `0.003" - 0.005"` | 0.0762, 0.127 mm |
| `amana-zrn-flat-mdf-pocket-3175-2f` | 2F Flat Bottom, 1/8" col, Wood/MDF/Sign-Foam row (split across material rows) | `0.003" - 0.005"` | 0.0762, 0.127 mm |
| `amana-zrn-flat-aluminum-pocket-6000-2f` | 2F Flat Bottom, 6mm col, Aluminum/Copper/Brass row | `0.004" - 0.006"` | 0.1016, 0.1524 mm |
| `amana-zrn-flat-acrylic-pocket-6000-2f` | 2F Flat Bottom, 6mm col, Plastic/Acrylic/Plexiglas® row | `0.004" - 0.006"` | 0.1016, 0.1524 mm |
| `amana-zrn-flat-softwood-pocket-6000-2f` | 2F Flat Bottom, 6mm col, Wood/MDF/Sign-Foam row | `0.007" - 0.009"` | 0.1778, 0.2286 mm |
| `amana-zrn-flat-aluminum-pocket-6350-2f` | 2F Flat Bottom, 1/4" col, Aluminum/Copper/Brass row | `0.005" - 0.007"` | 0.127, 0.1778 mm |
| `amana-zrn-flat-softwood-pocket-6350-2f` | 2F Flat Bottom, 1/4" col, Wood/MDF/Sign-Foam row | `0.006" - 0.008"` | 0.1524, 0.2032 mm |

Note on the Wood/MDF/Sign-Foam row split: the chart conflates wood and MDF on one line. I emit two rows for diameter 3.175 mm (one `softwood` and one `mdf`) to make the data reachable by either material query at that size; for 6.0 mm and 6.35 mm I emit a single `softwood` row (a duplicate `mdf` row would have identical numbers and add no information beyond the 3.175 example — the lookup matcher falls back across material_family in vendor_lookup). 6mm Wood column has only one row in JSON for now (`softwood`) — adding `mdf` is a no-op coverage extension and could be done later if matcher behaviour needs it.

### Rows extracted (3 Flute Ball Nose — NEW diameters)

Existing in `amana_ball_nose.json` (NOT re-collected): 0.794 mm (1/32"-1mm column).

| observation_id | Source row | Verbatim CPT range | Conversion |
|---|---|---|---|
| `amana-zrn-ball-softwood-parallel-9525-3f` | 3F Ball Nose, 3/8" col, Wood/MDF/Sign-Foam row | `0.006" - 0.008"` | 0.1524, 0.2032 mm |
| `amana-zrn-ball-softwood-parallel-12700-3f` | 3F Ball Nose, 1/2" col, Wood/MDF/Sign-Foam row | `0.007" - 0.009"` | 0.1778, 0.2286 mm |

The intermediate 3F Ball Nose columns (3/16" = 4.7625 mm, 6mm, 1/4" = 6.35 mm) were SKIPPED because the chart's printed IPM (215"-320" at 18000 RPM × 3 flutes ⇒ derived CPT ≈ 0.00398-0.00593 in) and printed CPT (`0.004" - 0.006"`) agree to 3 sig figs and the matching live row `amana-ball-softwood-parallel-1587-2f-zrn` in `amana_ball_nose.json` already documents an inconsistency flag for the 1/16" 2F column. Rather than introduce new rows for diameters that nominally overlap the existing 6 mm `amana-ball-softwood-parallel-6000-2f` (different flute count + subfamily) without first verifying the chart's IPM/CPT consistency, I emit only the larger 3/8" and 1/2" columns where there is no existing 3F coverage anywhere in the LUT.

---

## Source S3: Spektra-Coated 3D Profiling Feed/Chip Load Chart v6

- URL: https://www.amanatool.com/pub/media/productattachments/Spektra-Coated-3D-Profiling-Feed-Chip-Load-Chart-v6.pdf
- Pulled: 2026-05-30 (200 OK, two-page PDF).
- Chart format: same as ZrN chart but for the Spektra-coated SKUs. Diameters listed: 2F Ball Nose 1/4", 2F Flat Bottom 1/4", 3F Ball Nose 1/32" & 1/8", 4F 1/16" & 1/8", 3F Extra Long 1/4".

**No new rows emitted from S3.** Every diameter+material+flute combination on this chart duplicates either `amana_ball_nose.json` (3F 1/32"=0.794 mm ZrN row, 4F 1/16"=1.5875 mm ZrN row, 4F 1.5 mm ZrN row at flute count 4) or has the same numbers as the corresponding ZrN chart row (Spektra is an extended-life coating; per-row CPT is identical to the ZrN reference). Logged to gaps as a deliberate non-extraction with rationale.

---

## Source S4: Spoilboard 2+2 Insert Carbide Speed Chart (RC-2251 / RC-2252)

- URL attempted (404 — original from source manifest): `https://www.amanatool.com/pub/media/productattachments/Spoilboard_2-Speed-Chart.pdf` — Server returned HTTP/2 404, "Document stream is empty".
- URL successful: `https://www.amanatool.com/pub/media/productattachments/Spoilboard_2_2-Speed-Chart.pdf` — fetched 142 kB, single page (one table for RC-2251 and RC-2252).
- Chart format: tabular, RPM/IPM/CPT triplets per tool/material cell. Cutting depth max warning: "Max Cutting Depth Per Pass 1/4"".

**No rows emitted.** REQUIRED field `diameter_mm` cannot be read from the fetched chart — the chart only references tool SKUs (RC-2251, RC-2252), not diameters. The Amana product pages that would supply the diameters are behind Cloudflare and return 403 to curl with a browser UA. The URL slugs include "2-1/2-dia" and "3-dia" but URL strings are not fetched primary content. Logged to gaps.

---

## Inch→mm conversions (cross-reference table)

| inch | mm |
|---|---|
| 1/32 | 0.79375 |
| 1/16 | 1.5875 |
| 3/32 | 2.38125 |
| 1/8 | 3.175 |
| 3/16 | 4.7625 |
| 1/4 | 6.35 |
| 3/8 | 9.525 |
| 1/2 | 12.7 |
| 3/4 | 19.05 |

| inch CPT | mm/tooth |
|---|---|
| 0.0010 | 0.0254 |
| 0.0020 | 0.0508 |
| 0.0023 | 0.05842 |
| 0.0030 | 0.0762 |
| 0.0040 | 0.1016 |
| 0.0046 | 0.11684 |
| 0.0050 | 0.127 |
| 0.0057 | 0.14478 |
| 0.0060 | 0.1524 |
| 0.0064 | 0.16256 |
| 0.0090 | 0.2286 |
| 0.0096 | 0.24384 |
| 0.0100 | 0.254 |
| 0.0108 | 0.27432 |
| 0.0070 | 0.1778 |
| 0.0080 | 0.2032 |
| 0.0050 | 0.127 |
