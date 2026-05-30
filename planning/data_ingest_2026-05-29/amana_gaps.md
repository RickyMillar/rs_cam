# Amana Tool — Gaps and access notes (2026-05-29)

## Access method note (not a gap, but important for future runs)
The `WebFetch` tool returned **HTTP 403 Forbidden** for every
`amanatool.com/pub/media/productattachments/*.pdf` URL — the CDN blocks the
WebFetch User-Agent. All five ingested PDFs were fetched successfully via
`curl -L -A "<browser UA>"` (HTTP 200), then text-extracted with
`pdftotext -layout`. Future Amana ingests should use the curl+UA path, not
WebFetch, for these PDF URLs.

## Charts checked but intentionally NOT ingested as new rows (already covered)
These exist and are readable, but the repo already has equivalent Grade-A rows
from the same family; ingesting again would duplicate:
- **Spektra Plastic O-Flute** (`Spektra-Plastic-O-Flute-Speed-Chart-v11.pdf`,
  HTTP 200, readable): identical diameter/chipload table to the standard
  Plastic O-Flute (`Plastic-O-Flute-Speed-Chart-v2.pdf`) which was ingested.
  Only difference is the Spektra coating. No new numbers.
- **ZrN / Spektra 3D Profiling** (`ZrN-3D-Profiling-Feed-Chip-Load-Chart.pdf`):
  already in `amana_3d_profiling.json` and `amana_ball_nose.json`
  (tapered_ball_nose, softwood/hardwood/mdf/acrylic at 3.175/6.0 mm). Not
  re-ingested.
- **Spiral Ball Nose** (`Spiral-Ball-Nose-Speed-Chart-v7.pdf`): already in
  `amana_ball_nose.json`. Not re-ingested.
- **Spoilboard 2-Wing surfacing** (`Spoilboard_2-Wing-Speed-Chart-v8.pdf`):
  already in `amana_facing.json` (facing_bit, 22 mm). Not re-ingested.
- **Solid Carbide Spektra Spiral Plunge 2/3 Flute**
  (`Solid-Carbide-Spektra-Spiral-Plunge-2-3-Flute-v24.pdf`): already in
  `amana_flat_end.json` (upcut + compression). Not re-ingested.
- **Insert V-Groove v16** (`Insert-V-Groove-Speed-Chart-v16.pdf`): already in
  `amana_vbit.json` at 90°. The newly-ingested AMS-159 V-Groove chart is a
  DIFFERENT source/tool (solid carbide + carbide tipped, fixed-RPM 18k chart),
  so its 90° row is kept under a distinct source_id with an `-ams159` id suffix.

## True gaps (could not extract usable values)
- **Older 15/60/90 V-Groove Engraving chart**
  (`15-60-90_Degree-V-Groove-Engraving-Speed-Chart.pdf`): NOT fetched/verified
  in this run. The 15/30/45/120° Spektra engraving chart and the
  18/30/45/60/90° AMS-159 chart together already cover 15/18/30/45/60/90/120°,
  so this older chart is superseded for angle coverage. Left unfetched to
  avoid duplicate angle rows; revisit only if a unique material column is
  needed.
- **Carbon/composite chart** (`File-1436543087.pdf`): out of core scope
  (CFRP/fiberglass, no `material_family` enum value). Not fetched.

## Schema-fit caveats recorded (not gaps, flagged for the ingest reviewer)
- The **Plastic O-Flute** and **Compression "Plastic"** charts publish a single
  plastic recommendation that is NOT broken out by plastic type. Rows were
  emitted for acrylic / hdpe / polycarbonate sharing identical source numbers
  and quotes. This is faithful transcription, not interpolation, but the
  reviewer may wish to collapse to a single representative `acrylic` row if the
  LUT keying prefers one row per (tool, material, diameter).
- The **AMS-159 V-Groove "Hard Plastic"** and **Spektra Engraving "Hard
  Plastic"** rows were mapped to `material_family: acrylic` (closest enum
  member for a hard plastic). The chart label is literally "Hard Plastic".
- V-groove / engraving charts give **chip load per tooth** but no axial
  depth-of-cut numbers (DOC is the 1×/2×/3× tool-diameter rule only), so
  `ap_min/max_mm` and `ae_*` are null on those rows; only `ap_rule` is set.
- The **60°/90° V-groove** and all **compression** cells are single values (no
  range), so `chipload_min == chipload_max` on those rows.
