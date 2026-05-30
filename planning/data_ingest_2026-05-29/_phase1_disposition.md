# 2026-05-29 staging — Phase 1 promotion disposition

Notes on what got promoted from this directory by Phase 1 of the
`feeds_data_ingest_2026-05-30_phased_plan.md`. Anything reported by
`validate_lut.py` against this directory was either (a) promoted to
the live observation files with diameters/flute counts/pass_role
fixes applied at promotion time, or (b) deferred per the disposition
below.

## Promoted live (Phase 1C, 2026-05-30)

- `amana.json` — non-wood rows (acrylic / aluminum / HDPE / PC) split
  into `amana_plastic_oflute.json`, `amana_zrn_aluminum.json`,
  `amana_vgroove_aluminum_acrylic.json` (diameters set 6.35 mm per the
  verifier report), plus the acrylic compression spiral row appended
  to `amana_compression.json`.
- `onsrud_whiteside.json` — `onsrud_*` rows with diameter went live as
  `onsrud_plastic.json` (9 rows); `whiteside_*` rows went live as
  `whiteside_rpm_assorted.json` (4 rows, RPM-only is intentional —
  Whiteside doesn't publish chipload).
- `harvey_helical_garr.json` — Helical 6061 rows promoted as
  `helical_aluminum.json` (2 rows; `pass_role` normalised
  "rough" → "roughing" at promotion).

## Deferred to Phase 3 (with reason)

- **Garr aluminum (10 rows)** — the chart shares CPT across the 242M /
  842M / A3 series but each series has a different flute count
  (242M=2f, 842M=2f, A3=3f). Promoting as-is would store an unknown
  flute count and the lookup's diameter-scale gate would extrapolate
  against an averaged value. Real fix is to split per series and
  fetch true flute count per the Garr PDF series-spec block. Listed
  for Phase 3 beat A (Amana long tail + Garr re-split).
- **`onsrud-article-polycarbonate-optimum-chipload-window`** — narrative
  datapoint extracted from an Onsrud article, no diameter; stays as a
  provenance-only note (will not become a LUT row).
- **`amana.json` vbit/engrave wood rows with `dia: null`** — these are
  the pre-promotion staging copies. The LIVE versions are in
  `crates/rs_cam_core/data/vendor_lut/observations/amana_vgroove_engraving.json`
  with verifier-recovered diameters (15°/18°/30°/45° = 6.35 mm or as
  per verifier, 60° = 12.7 mm, 90° = 9.525 mm, 120° = 6.35 mm). The
  staging copies remain here as the historical raw extraction record.

## Validator output is expected to be noisy on this dir

`python3 ../data_ingest_2026-05-30/validate_lut.py
planning/data_ingest_2026-05-29` will report the disposition above as
PROBLEMS — that's the validator's job. The live LUT (the rows that
matter for runtime safety) is clean; this dir is a pre-promotion
snapshot kept for source-trail purposes.
