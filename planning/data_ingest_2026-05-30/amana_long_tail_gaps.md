# Amana long tail — gaps

Per honesty-or-gap rule (Phased Plan rule #1): every URL/source attempted but not extractable is logged here with the verbatim failure reason. No row was guessed.

---

## G1. Spoilboard `Spoilboard_2-Speed-Chart.pdf` (the URL from the master plan)

- Attempted URL: `https://www.amanatool.com/pub/media/productattachments/Spoilboard_2-Speed-Chart.pdf`
- Result: pdftotext output `Syntax Error: Document stream is empty` (HTTP/2 404 confirmed via `curl -sIL`; Cloudflare returned a 404 page, not the PDF).
- Reason: stale URL on Amana's CDN. The correct slug is `Spoilboard_2_2-Speed-Chart.pdf` (the chart for the **2+2** insert family). That alternate URL fetched OK — see G2.

## G2. Spoilboard 2+2 chart — diameter unknown (5 RPM/IPM/CPT cells × 2 SKUs LOST)

- URL OK: `https://www.amanatool.com/pub/media/productattachments/Spoilboard_2_2-Speed-Chart.pdf`
- Chart content fetched successfully (10 cells = 2 tools × 5 materials). Verbatim sample: `RC-2251  18,000 / 130" / 0.0038"  18,000 / 110" / 0.0031"  18,000 / 220" / 0.0062"  18,000 / 270" / 0.0075"  18,000 / 120" / 0.0031"`.
- Blocker: chart lists tool SKUs only (RC-2251, RC-2252); does not provide `diameter_mm`. The schema's `diameter_mm` is REQUIRED (non-Option, no serde default — see `validate_lut.py::REQUIRED`).
- Diameter lookup attempt: Amana product pages return HTTP 403 to curl + Mozilla UA via Cloudflare's managed challenge (verbatim: `<title>Just a moment...</title>` ... `<noscript><div class="h2"><span id="challenge-error-text">Enable JavaScript and cookies to continue</span></span></div></noscript>`). WebFetch is blocked on this CDN per the plan's note.
- URL slug hints (NOT primary content): the URL slugs `rc-2251-...-2-1-2-dia-...` and `rc-2252-...-3-dia-...` suggest 2.5" (63.5 mm) and 3" (76.2 mm) respectively, but a URL string is not a fetched-source citation per honesty rule #1. ROW NOT EMITTED.
- Recovery path: open the product page in a real browser, paste the catalogued diameter into a follow-up ingest with `source_url` set to the product page and a `source_page` note quoting the catalog spec block. Until then the chart is non-promotable.

## G3. Spektra-Coated 3D Profiling Chart v6 — zero unique extractions

- URL OK: `https://www.amanatool.com/pub/media/productattachments/Spektra-Coated-3D-Profiling-Feed-Chip-Load-Chart-v6.pdf`
- Chart content fetched successfully (2 pages).
- Reason for no rows: every cell on this chart duplicates either an existing live row in `amana_ball_nose.json` (3F 1/32"=0.794, 4F 1/16"=1.5875, 4F 1.5 mm under the ZrN sibling chart) or the ZrN sibling chart printed at the same nominal CPT (Spektra is the coating-upgrade SKU family; chart values are identical to ZrN within the printed precision).
- This is a deliberate non-extraction, logged so the next collector doesn't redo the work. If a future row needs a Spektra-specific provenance trail (e.g. for warranty / supplier-preference UI hints) the same numbers can be re-emitted with `source_id=amana_spektra_3d_profiling_v6`.

## G4. ZrN chart 3F Ball Nose intermediate diameters (3/16", 6mm, 1/4")

- Cells present: printed CPT range `0.004" - 0.006"` per cell across material rows; printed IPM `215" - 320"` (matches CPT derivation at 18000 RPM × 3 flutes).
- Reason for non-extraction: existing live row `amana-ball-softwood-parallel-1587-2f-zrn` in `amana_ball_nose.json` already documents an IPM/CPT inconsistency at the 1/16" 2F column on the same chart. Without manual cross-check of the 3F Ball Nose 3/16" / 6 mm / 1/4" cells against derived IPM, the safest path is to skip these mid-range diameters and emit only the unambiguous large sizes (3/8" and 1/2"). Mid-range 3F Ball Nose diameters can be added in a follow-up after the 1/16" inconsistency is resolved.

## G5. ZrN chart 3F Flat Bottom rows

- Cells present: full 6-column table (1/32"-1mm, 1/16", 1/8", 3/16"-1/4, 3/8", 1/2") with CPT ranges.
- Reason for non-extraction this round: the `amana_flat_end.json` file currently only carries 2F and 3F upcut spirals at 3.175 mm and 6.0 mm for `tool_subfamily=upcut|compression`. Adding the ZrN 3F Flat Bottom diameter sweep would require a new subfamily key choice that intersects the existing matcher logic (the matcher currently picks by subfamily preference order in `vendor_lookup.rs`). To avoid disturbing the live-matcher behaviour on a high-volume table, the 3F Flat Bottom rows are deferred to a follow-up that can be promoted together with a matcher review. The 2F Flat Bottom rows ARE emitted in this round because they have no overlapping subfamily (the live 2F flat_end rows in `amana_flat_end.json` are `upcut` / `compression`, not `zrn_2d3d_carving`).

## G6. ZrN chart 2F Ball Nose 6mm and 1/4" columns (no NEW rows added here)

- The 6mm / 1/4" 2F Ball Nose ZrN cells exist on this chart with CPT range `0.004" - 0.006"` (6mm Wood) and `0.004" - 0.006"` (1/4" Wood).
- Existing live rows at the same diameter in `amana_ball_nose.json` use `tool_subfamily=solid_carbide` (e.g. `amana-ball-softwood-parallel-6000-2f`, CPT 0.030-0.050 mm). The chart's CPT for ZrN 6mm 2F Wood is **larger** than the live solid-carbide row at the same diameter (0.1016-0.1524 vs 0.030-0.050) — this is consistent with the ZrN row's coating allowing higher feed, but emitting a parallel `zrn` subfamily 6mm 2F row would split the lookup target without a matcher-side preference rule. Deferred to a follow-up that adds an explicit `zrn`-vs-`solid_carbide` ranking in `vendor_lookup.rs::pick_subfamily`. The 3F sizes 3/8" / 1/2" emitted in this round avoid that ambiguity (no existing 3F ball-nose row at those diameters anywhere in the LUT).

## G7. ZrN chart 3F Ball Nose 1.5 mm 4F row

- The 4 Flute Ball Nose 1.5 mm cell is already live as `amana-ball-softwood-parallel-1500-4f-zrn` in `amana_ball_nose.json` per existing provenance — NOT re-collected.

---

## Coverage summary (rows yielded vs deferred)

- Rows yielded: 37 (all schema-clean, 0 validator PROBLEMS).
- Deferred (logged here): 2+2 spoilboard (10 cells × 2 SKUs lost on diameter), Spektra-Coated 3D chart (12+ cells duplicate), ZrN 3F Flat Bottom (24+ cells deferred for matcher review), ZrN 3F Ball Nose mid-range (18+ cells deferred for IPM/CPT cross-check), ZrN 2F Ball Nose 6mm/1/4" coating-vs-uncoated subfamily ambiguity (~8 cells deferred).
