# Vendor breadth gaps — Phase 3 / G beat (2026-05-30)

## URLs attempted

| URL | HTTP | Result | Action |
|-----|------|--------|--------|
| https://www.whitesiderouterbits.com/products/1540 | 200 | Locked behind Shopify "Locksmith" (requires customer login). Page returns product structure but no feeds/speeds beyond the RPM range already captured in `whiteside_rpm_assorted.json`. | No new chipload data extractable. |
| https://www.whitesiderouterbits.com/products/ru4000h | 200 | Returns product spec page; "Recommended RPM" only, no chipload-per-tooth. (Same as 2026-05-29 round's finding.) | Use Fusion 360 .tool file instead — succeeded (G.1). |
| https://www.whitesiderouterbits.com/pages/feed-and-speed | 404 | Not found. | — |
| https://www.whitesiderouterbits.com/pages/feeds-and-speeds | 404 | Not found. | — |
| https://www.whitesiderouterbits.com/pages/cnc | 404 | Not found. | — |
| https://www.whitesiderouterbits.com/pages/cnc-router-bits | 404 | Not found. | — |
| https://www.whitesiderouterbits.com/ (home) | 200 | Found CNC brochure PDF link + Vectric/Fusion/Carveco tool file pages. | Followed all. |
| https://cdn.shopify.com/s/files/1/1698/7023/files/CNC_Brochure_2-6-23.pdf | 200 | Whiteside CNC brochure — part numbers, cutting dia/length, CNC sets. **No chipload data** — purely a product catalog. | No data extractable. |
| https://www.whitesiderouterbits.com/pages/vectric-tool-files | 200 | Page links to a Dropbox-hosted Vectric .tool library; Vectric-format binary files. | Not parsed (Vectric proprietary). Fusion 360 path succeeded — same author would carry same feeds. |
| https://www.whitesiderouterbits.com/pages/fusion-360-tool-files | 200 | Page links to Fusion 360 .tool library Dropbox URL. | **G.1 SUCCESS** — see provenance. |
| https://www.dropbox.com/s/bqw4gqeggcv30o1/Whiteside-Router-Bits-Fusion360.tools?dl=1 | 200 | ZIP containing `tools.json` with 46 Whiteside tool records and full feeds/speeds. **High-yield.** | 13 rows extracted (only flat_end/ball_nose/tapered_ball_nose families to fit existing schema). |
| https://www.whitesiderouterbits.com/pages/carveco-tool-database | not fetched | Skipped — same data expected as Fusion 360 file. | — |
| https://www.freudtools.com/ | 200 | Found `window.freud.data.main.downloadables.router-cnc` listing a "Router Bit Feed Rates and Speeds" PDF. | Followed. |
| https://www.freudtools.com/support/ | 404 | Not found. | — |
| https://www.freudtools.com/router-bit-feed-and-speed | 404 | Not found. | — |
| https://www.freudtools.com/router/router-bits | 404 | Not found. | — |
| https://www.freudtools.com/index.php/customer-service/customer-resources | 404 | Not found. | — |
| https://www.freudtools.com/downloads | 200 | Lists ~17 router-cnc PDFs incl. "Router Bit Feed Rates and Speeds". | Followed. |
| https://www.freudtools.com/public/assets/freud/downloadables/freudtools-router-bit-feed-and-speed-for-cnc-20170822.pdf | 200 | 98 KB PDF; 2 pages. Text layer extracts cleanly via `pdftotext -layout`. Contains TWO chipload tables (Solid Carbide; Carbide-Tipped Straight & Profile). **G.2 SUCCESS.** | 14 rows extracted from the Solid Carbide chart. The Carbide-Tipped chart (six rows of 1/8"–3/4" by 7 material columns = 42 cells) was **NOT** ingested in this round to keep the row count focused; future rounds can mine it. |
| https://www.vortextool.com/ | 200 | Home page links to wood/plastic/metal tooling category pages and PDF catalogs. Found chip-load chart link in main image area. | Followed. |
| https://www.vortextool.com/resources/ | 404 | Not found. | — |
| https://www.vortextool.com/media/assets/chipLoadChart.pdf | 200 | 6.2 MB single-page PDF; created by "Adobe Photoshop CS5 Windows". `pdftotext -layout` returns garbled fragments only (image-only chart). `pdfimages` extracts a single 3212×2260 JPEG image. **OCR not attempted — `tesseract` not installed on the collection host.** | **GAP — image-only source; needs OCR-capable host or human transcription.** |
| https://www.vortextool.com/media/assets/Vortex_Catalog.pdf | 200 | 9 MB main wood-tooling catalog; contains a "Chip Load Chart" page (p.14) that is also image-only. Catalog product pages have CED/CEL/SHK DIA/OAL but NO chipload columns. | **GAP — no machine-readable chipload data anywhere on Vortex's site.** |
| https://idcwoodcraft.com/pages/database-downloads | 200 | Lists downloads: Vectric .tool library (FileMonk-hosted ZIP), Carveco database (FileMonk ZIP), Fusion 360 (FileMonk ZIP), and the chipload CSV API endpoint. | **G.4 SUCCESS via CSV.** |
| https://feeds-speeds-chipload-api.fly.dev/download-csv | 200 | 963 KB CSV, 79 rows, parses cleanly. Vendors covered: IDC Woodcraft (63), Cadence Manufacturing and Design (16). Columns: vendor, model, name, type, diameter, numflutes, feedrate, rpm, etc. Material is per-row JSON blob. | 10 IDC rows extracted. Cadence rows not ingested this round (focus on IDC-line per beat scope). |

## NEW vendor enum variants needed (must add before promotion)

| Variant | Source | Rows blocked |
|---------|--------|--------------|
| `Vendor::Freud` | freudtools.com chipload PDF | 14 |
| `Vendor::Idcwoodcraft` (or `Vendor::Idc`) | idcwoodcraft.com chipload CSV | 10 |

(Cadence Manufacturing and Design is also present in the IDC CSV with 16
rows but was not ingested this round; if mined later, would need
`Vendor::Cadence`.)

Recommend adding `Vendor::Freud` and `Vendor::Idcwoodcraft` to
`crates/rs_cam_core/src/feeds/vendor_lut.rs::Vendor` and to the
`VENDORS` set in `validate_lut.py`. After that the 24 rows currently
flagged "vendor not in enum" all become schema-clean.

## Vortex chipload chart — re-collection plan

When `tesseract` (or equivalent OCR) becomes available, run:

```bash
pdfimages -j /tmp/vortex_chipload.pdf /tmp/vx
tesseract /tmp/vx-000.jpg /tmp/vortex_ocr --psm 6
# Review /tmp/vortex_ocr.txt against /tmp/vx-000.jpg manually.
# Image is high resolution (3212×2260 at 300 DPI) so OCR should be
# reasonable, but per the Phase 3 beat-D contract any uncertain cell
# must be logged as GAP rather than guessed.
```

The same chart is also linked from the Vortex catalog (p.14) and from
the Vortex Tool Selection Guide mobile app. Mobile-app feeds would be
the cleanest source if an app-data extraction path were available.

## Validator flag — "chipload max suspicious" on 4 Freud rows

The validator flags `chipload_max_mm_tooth > 0.5` as suspicious. Four
Freud rows breach this threshold:

- `freud-solid-carbide-half-hardwood` — max 0.5334 mm (verbatim .021")
- `freud-solid-carbide-half-softwood` — max 0.5842 mm (verbatim .023")
- `freud-solid-carbide-half-mdf-particle` — max 0.6858 mm (verbatim .027")
- `freud-solid-carbide-half-plywood-hardwood` — max 0.5334 mm (verbatim .021")

These are HONEST verbatim values from the Freud Solid Carbide chart for
1/2" cutters. Freud's own chart explicitly lists chiploads up to 0.027"
in the 1/2" × MDF cell. The validator threshold is conservative; values
are correct. Recommend either:

1. Loosen the validator's suspicion threshold to ≥1.0 mm (real chip
   thicknesses can legitimately exceed 0.5 mm on big bits in soft
   sheet goods).
2. Or leave the warning in place and reference this gap entry from
   any auditor confused by the flag.

## Rows NOT ingested (deferred to future rounds)

- **Freud Carbide-Tipped chart** (page 2 of the same PDF): 42 more
  cells across 1/8"–3/4" × 7 materials. Skipped to keep row count
  focused on the highest-grade source (Solid Carbide is the relevant
  family for CNC routers; the Carbide-Tipped chart is for hand-fed
  routers and would need a distinct `tool_subfamily`).
- **Cadence Mfg & Design** (16 rows in the IDC CSV): not in the beat
  G scope; would need a new `Vendor::Cadence` enum entry.
- **Whiteside form-mill family rows** (33 of the 46 Fusion 360 tools)
  — V-grooves, round-noses, point-cutting roundovers, engraving bits,
  spoilboard cutters, plunge roundovers, bowl-and-tray. These don't
  map cleanly to the existing `tool_family` enum (`flat_end |
  ball_nose | tapered_ball_nose | bull_nose | chamfer_vbit |
  facing_bit`) — V-grooves go to `chamfer_vbit` only if angle is
  recorded; spoilboard cutters could be `facing_bit`. Future rounds
  with explicit family mapping rules can absorb these.
