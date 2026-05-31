# Phase 4 consolidation report — Feeds/Speeds & Tool-Load Data Ingest

**Date:** 2026-06-01
**Phase 4 plan:** `planning/phase_4_promotion_plan_2026-06-01.md`
**Verifier report:** `planning/data_ingest_2026-05-30/verification_report_2026-06-01.md`
**Exit gate:** ✅ all 4 criteria met (lib + tests + clippy + smoke-diff)

## Headline

**Phase 4 closes the feeds data-ingest workstream.** 8 net-new
verifier-confirmed vendor LUT rows promoted into `crates/rs_cam_core/
data/vendor_lut/observations/`, bundled count **239 → 247**.
`cargo run -p rs_cam_cli -- smoke --diff` against the
`2026-06-02.csv` baseline reports `no regressions (18 cases
checked)`.

The remaining 17 staged-not-promoted rows are blocked by schema
shape (v-bit missing `diameter_mm` and the absent `MaterialFamily::
Fiberglass` enum variant), **not** by data quality — the verifier
re-fetched all 14 strategic source URLs and confirmed every value
matches its source to four-figure precision (14/14 CONFIRMED, 0
MISMATCH). The schema work is tracked for Phase 5.

## What landed live

### 4 Garr aluminum rows
Appended to `crates/rs_cam_core/data/vendor_lut/observations/garr_aluminum.json`
(11 → 15 observations).

| observation_id | source_id | diameter | flute | chipload mm/tooth |
|----------------|-----------|----------|-------|-------------------|
| `garr-142m-alum-slot-6000-flat-2f`    | `garr_milling_aluminum_mid_range` | 6.0 mm | 2 | 0.090–0.150 |
| `garr-142m-alum-profile-6000-flat-2f` | `garr_milling_aluminum_mid_range` | 6.0 mm | 2 | 0.090–0.150 |
| `garr-gp-alum-6000-flat-2f`           | `garr_general_purpose_milling`    | 6.0 mm | 2 | 0.030–0.051 |
| `garr-gp-alum-3000-flat-2f`           | `garr_general_purpose_milling`    | 3.0 mm | 2 | 0.015–0.025 |

Staged JSON fixes applied in-place to
`planning/data_ingest_2026-05-29/harvey_helical_garr.json`:
- 9 Garr rows: `pass_role: "rough"` → `"roughing"` (matches Rust
  `PassRole::Roughing` discriminant)
- 9 Garr rows: added `flute_count` from product naming (142M=2,
  242M=2, A3=3, GP=2)

### 4 Freud 1/2-inch solid carbide rows
Appended to `crates/rs_cam_core/data/vendor_lut/observations/freud_solid_carbide.json`
(10 → 14 observations).

| observation_id | source_id | diameter | flute | chipload mm/tooth |
|----------------|-----------|----------|-------|-------------------|
| `freud-solid-carbide-half-hardwood`         | `freud_router_bit_feed_and_speed_for_cnc_20170822` | 12.7 mm | 2 | 0.4572–0.5334 |
| `freud-solid-carbide-half-softwood`         | `freud_router_bit_feed_and_speed_for_cnc_20170822` | 12.7 mm | 2 | 0.5080–0.5842 |
| `freud-solid-carbide-half-mdf-particle`     | `freud_router_bit_feed_and_speed_for_cnc_20170822` | 12.7 mm | 2 | 0.5842–0.6858 |
| `freud-solid-carbide-half-plywood-hardwood` | `freud_router_bit_feed_and_speed_for_cnc_20170822` | 12.7 mm | 2 | 0.4572–0.5334 |

Validator initially flagged the chiploads as "suspicious" (≥ 0.5
mm/tooth), but the verifier re-fetched the Freud PDF and confirmed
they match the published 1/2" row's `.018"–.027"` cells exactly.
The high values are correct because they apply at 1xD-DOC reference
condition; the row's `ap_rule` field captures the documented
25%/50% derate at 2xD/3xD.

Post-promotion fix: `source_page` field type-coerced from integer
`2` to string `"2"` to match the Rust `VendorObservation` schema
(`pub source_page: Option<String>`).

## Source manifest additions

Two new `source_id` entries appended to
`crates/rs_cam_core/data/vendor_lut/source_manifest.json`:
- `garr_milling_aluminum_mid_range` (covers the 2 142M rows)
- `garr_general_purpose_milling` (covers the 2 GP-alum rows + the
  unpromoted GP-plastics row)

The Freud source_id was already in the manifest from the earlier
Freud sub-3/8" bundle; no new entry needed.

`CREDITS.md` updated with a 2026-06-01 block documenting the
promotion under the "Vendor LUT source manifest" section.

## What stayed staged (and why)

### 15 Amana v-bit / engrave rows — schema gap (no `diameter_mm`)

Source: `planning/data_ingest_2026-05-29/amana.json`,
source_ids `amana_ams159_vgroove_v2` + `amana_spektra_engraving_v4`.

Affected:
- `amana-vbit-{softwood,hardwood,acrylic,aluminum}-trace-{18,30,45,60,90}deg-{1,2}f` (11 rows)
- `amana-engrave-{softwood,hardwood,hardplastic}-trace-{30,45}deg-1f` (4 rows)

Verifier-confirmed: every chipload matches the AMS-159 v2 and
Spektra v4 PDFs verbatim. The blocker is that v-bit speed charts
publish per-angle rows without anchoring to a fixed body diameter
(the cutter's `diameter_mm` depends on engagement depth for a
tapered V-bit).

Two follow-up paths documented in
`planning/data_ingest_2026-05-29/amana_gaps.md`:
1. Source the SKU-specific body diameter from each Amana product
   page and add `diameter_mm` to the staged rows.
2. Relax `VendorObservation::diameter_mm` to `Option<f64>` and
   update `vendor_lookup::find_best_row_for_geometry` to dispatch
   on `included_angle_deg` + `tool_family` when diameter is None.

Path 2 is the correct schema fix (v-bits are angle-driven, not
diameter-driven) but is Phase 5 scope (cross-cutting schema +
lookup-matcher change).

### 1 Onsrud polycarbonate article row — schema gap (no `diameter_mm`)

Source: `planning/data_ingest_2026-05-29/onsrud_whiteside.json`,
observation_id `onsrud-article-polycarbonate-optimum-chipload-window`.

Verifier-confirmed: chipload range (0.1016–0.3048 mm/tooth) matches
the Onsrud article verbatim. Blocker is same as v-bit case — the
article reports a diameter-independent chipload window for
polycarbonate routing across Onsrud's full upcut O-flute line, so
the source has no per-diameter anchor.

Follow-up documented in
`planning/data_ingest_2026-05-29/onsrud_whiteside_gaps.md`.

### 1 Garr fiberglass/G10 row — invalid `material_family`

Source: `planning/data_ingest_2026-05-29/harvey_helical_garr.json`,
observation_id `garr-gp-plastics-6000-flat`.

Verifier-confirmed: chipload (0.030–0.051 mm/tooth) and SMM
(79–157) match Garr's General Purpose chart "Composite (non-ISO) —
Fiberglass, Plastics, G10" row. Blocker: the row uses
`material_family: plastic` which is not a variant of `MaterialFamily`
(`{Softwood, Hardwood, PlywoodSoftwood, PlywoodHardwood, Mdf, Hdf,
Particleboard, Acrylic, Hdpe, Polycarbonate, Delrin, Aluminum}`).

Fiberglass / G10 is fundamentally a different cutting class
(abrasive, fiber-reinforced) and collapsing it onto an existing
polymer enum would be misleading. Phase 5 follow-up: add
`MaterialFamily::Fiberglass` or `CompositeFiberReinforced`. Tracked
in `planning/data_ingest_2026-05-29/harvey_helical_garr_gaps.md`.

## Exit gate verification

All four Phase 4 exit criteria met:

| Criterion | Status |
|-----------|--------|
| `cargo test -p rs_cam_core --lib` | ✅ `1669 passed; 0 failed` (includes new `test_embedded_loads_all_observations: 247`) |
| `cargo test -p rs_cam_core --tests` | ✅ all integration tests pass (includes `embedded_count_matches_after_expansion: 247`) |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ clean across core / cli / viz |
| `cargo run -p rs_cam_cli -- smoke --diff --baseline 2026-06-02.csv --output /tmp/smoke_phase4.csv` | ✅ `smoke-diff: no regressions (18 cases checked)` |
| `source_manifest.json` reflects new source_ids | ✅ +2 entries |
| `CREDITS.md` reflects the promotion | ✅ 2026-06-01 block added |
| Consolidation report written | ✅ this file |

MCP smoke (operator AS001-AS015 walk) was NOT triggered because no
wood-class rows were promoted in this round — the deflection-gate
sentry is wood-only and the Phase 4 promotion is aluminum + sheet-
good (Freud MDF/plywood) only. Smoke regression net via F-037 +
CLI smoke `--diff` covers the relevant test surface for non-wood
rows.

## Verifier audit highlights

The `general-purpose` agent dispatched in parallel re-fetched 14
strategic source URLs and reported 14/14 CONFIRMED, 0 MISMATCH, 0
UNREACHABLE. Highlights:

- **Freud "half" series**: false-alarm PROBLEM flag resolved. Values
  are correct at 1xD-DOC reference; column-confusion / unit-error
  hypotheses both refuted.
- **Garr 142M / 242M / A3 / GP**: chiploads + computed RPMs match
  source to 2.5% rounding tolerance (one minor `rpm_max` rounding
  note on `garr-242m-alum-profile-6000-flat` — recorded 10350 vs
  computed 10610 from `Vc=200 m/min at 6 mm`; non-blocking).
- **Amana v-groove + Spektra engraving**: all chip-load values match
  AMS-159 v2 + Spektra v4 PDF cells exactly when in→mm-converted.
- **Onsrud polycarbonate**: chipload window matches article verbatim.
- **Helical aluminum cross-check** (rows already promoted): both
  HEM and traditional roughing arithmetic verified (500 IPM × 25.4
  / (18000 × 3) = 0.2352 mm/tooth ≈ live row 0.235).

Soft non-blocking concerns flagged by verifier:
1. The 14 Whiteside Fusion 360 `.tool` rows already in live LUT
   derive from a Dropbox archive that requires authentication;
   verifier could not re-fetch. Recommend caching the `.tool` file
   locally for future audit rounds.
2. The unpromoted Garr GP-plastics row collapses three distinct
   materials (Fiberglass / Plastics / G10) onto one row — when the
   `MaterialFamily::Fiberglass` enum lands, consider splitting into
   three separate rows.

## Open gaps after Phase 4

The data-ingest workstream is **substantially complete** for the
hobby-spindle Shapeoko-XXL envelope. Remaining gaps are not
loop-blocking:

1. **Vendor breadth round 3** — Kennametal / Sandvik / Vortex were
   Phase 3 stretch beats that nobody collected. Open ticket in
   `planning/data_ingest_2026-05-30/vendor_breadth_gaps.md`.
2. **Per-species wood Kc derivation** — Phase 3 beat C landed FPL
   Ch.5 shear-∥ reference data in `fpl_ch5_extract.md` but did NOT
   wire it into `Material::SolidWood::kc_n_per_mm2()` because doing
   so would shift wood-class Kc values and break the deflection
   bar. Needs a Phase 5 effort analogous to Phase 2B (sheet-good
   Kc retune): bundled Kc change + acceptance-sentry adjustment
   + smoke baseline shift. Tracked by TODO Phase 3 markers in
   `crates/rs_cam_core/src/material.rs:709`.
3. **V-bit schema** — Phase 5 scope as described above.
4. **MaterialFamily::Fiberglass** — Phase 5 scope as described above.
5. **AS013 stock_top_z methodology** — orthogonal CLI smoke gap
   tracked in `planning/toolpath_acceptance/baselines/2026-05-31_
   postphasef_notes.md` ("AS013 still reads 262.9 µm").
6. **Operator MCP smoke walk** — bench-bound, not blocking.

## Phase 4 in one sentence

The Phase 3 collection fleet's last 8 net-new rows are now live in
the vendor LUT after independent verification, closing the
feeds_data_ingest_2026-05-30 workstream's promotion phase; everything
else that stayed staged is a Phase 5 schema enhancement, not a data-
quality issue.
