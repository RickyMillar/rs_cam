# Gaps — Onsrud + Whiteside ingest (2026-05-29)

Sources attempted but not converted into rows, with the reason. No values were
invented to fill these.

## Blocked / unreachable

- **Interstate Plastics Onsrud router-bit page**
  URL: https://www.interstateplastics.com/onsrud-router-bits-lmt-onsrud-plastic-cutting-tools
  Reason: HTTP 403 Forbidden via WebFetch (bot-protected). Was intended as a
  secondary confirmation of the SP/HP material classification; the primary
  Onsrud FAQ#2 article supplied that classification instead, so no data lost.

## Read but intentionally NOT turned into numeric rows

- **Onsrud PCT-19 product cutting tools catalog**
  URL: https://onsrud.com/images/LMT%20Onsrud%20Product%20Cutting%20Tools%20Catalog%20PCT-19.pdf
  Status: catalog gives CED/CEL/SHK/OAL dimensions and series geometry, but no
  per-material chipload/RPM cutting data beyond what the SP/HP data sheets
  already provide. Used only to corroborate series geometry (e.g. 1F upcut O
  flute), not as a numeric feeds/speeds source. No row emitted.

- **Onsrud series detail pages** (e.g. /Series/63-000.asp, /Series/62-750.asp)
  Status: these pages render largely from a navigation menu; WebFetch returned
  category/geometry labels (flute count, upcut/downcut, O-flute, solid carbide,
  target material) but NOT chipload or a reliable per-tool RPM. Used only to
  set tool_subfamily geometry, never as a numeric chipload/RPM source.
  NOTE: 63-000.asp resolved to an *aluminum* 1F upcut tool, NOT a plastic
  series — confirming the 63-x00 numbering spans multiple material classes and
  that geometry must be read per exact series, not by prefix.

## Image-only / non-extractable

- None encountered in this slice. Both the **Soft Plastic** and **Hard
  Plastic** data-sheet PDFs were FlateDecode-compressed but fully
  TEXT-extractable with `pdftotext -layout` (the WebFetch HTML->markdown path
  failed on them because it does not decompress PDF streams; the binaries were
  saved locally and extracted directly). The acquisition doc's warning that
  "several Onsrud /images/*.pdf data sheets are image-only" did NOT hold for
  the two plastics sheets — they yielded clean machine-readable tables.

## Data deliberately left null (not a gap, a constraint)

- **RPM on all Onsrud plastics rows**: the SP/HP data sheets tabulate chipload
  per CED only; RPM is given by formula, not as a value (one `12,500 RPM`
  footnote applies to series 37-50/37-60, which were not selected). RPM left
  null rather than derived/invented.

- **Chipload on all Whiteside rows**: Whiteside publishes recommended RPM and
  material applicability but no chipload chart. Chipload left null; rows graded
  "b" / row_kind "derived" (RPM-only), per the schema instruction.

- **Whiteside flute count for 1540/1550**: the product pages do not state flute
  count for these two V-groove bits. flute_count=2 was recorded as an inference
  (the related #1541 is noted 3-flute "for improved veining", implying the
  standard models are 2-flute). Flagged in provenance as inference, not quote.

## Phase 4 promotion deferral (2026-06-01)

**`onsrud-article-polycarbonate-optimum-chipload-window`** — cannot
be promoted to live LUT because the row lacks `diameter_mm`. The
Onsrud polycarbonate optimum-chipload-window article reports a
chipload range (0.1016 – 0.3048 mm/tooth) that applies across all
diameters in their polycarbonate routing line; the article itself
does not anchor on a specific diameter.

**Two paths to promote in a follow-up round:**
1. Pick a representative diameter (Onsrud's 1/4" / 6.35 mm is the
   most common polycarbonate routing diameter) and create three
   diameter-anchored rows (3.175, 6.35, 12.7 mm) all citing the
   same source.
2. Extend `VendorObservation` schema to allow `diameter_mm: Option<f64>`
   with explicit "applies to diameter range X-Y" annotation — bigger
   schema change, defer to Phase 5.

---

## Status (2026-06-01, Phase 5 Step 5.2)

**CLOSED — schema-side blocker eliminated.** Took Path 2.
`VendorObservation::diameter_mm` is now `Option<f64>` (commit
`6901795`). The Onsrud polycarbonate article row promoted live as
`onsrud-article-polycarbonate-optimum-chipload-window` in
`onsrud_plastic.json`. New `source_id`
`onsrud_routing_polycarbonate_article` added to the manifest.

Anchorless rows skip the diameter-ratio gate and return
`chipload_diameter_scale = 1.0` from the matcher — no extrapolation
math runs on them. Focused test
`diameter_window_row_matches_flat_query_no_extrapolation` pins
this contract.

Consolidation report:
`planning/feeds_phase5_consolidation_2026-06-01.md`.
