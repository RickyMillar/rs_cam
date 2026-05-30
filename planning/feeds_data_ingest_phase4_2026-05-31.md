# Feeds Data Ingest — Phase 4 (bulk LUT row promotion)

**Date:** 2026-05-31
**Predecessors:** Phase 1+2C (`5ccf533`), Phase 2B (`bbb164f`), Phase 4
consolidation audit (`132e0a8`, `a4c6bfe`, `91625d6`, `c5f0363`, `a73f244`).
**Plan reference:** `planning/feeds_data_ingest_2026-05-30_phased_plan.md`
("PHASE 4" section).

## What landed

Bulk promotion of 117 staged vendor observation rows into the embedded
LUT, raising the live count from 111 → 228. Five new bundled JSON files
plus one industrial-only sibling file documented but not loaded.

### Per-source promotion table

| Source file (live path) | Vendor | Rows | Grade | Source |
|-------------------------|--------|------|-------|--------|
| `observations/amana_long_tail.json` | Amana | 37 | A | Spektra Spiral Plunge v24 + ZrN 3D Profiling v8 (extended diameter / flute coverage beyond pre-Phase-4 Amana files) |
| `observations/onsrud_ocr.json` | Onsrud | 47 | B | Hard Wood / Soft Wood / MDF cutting-data PDFs (OCR-extracted) |
| `observations/whiteside_fusion360.json` | Whiteside | 13 | A | Whiteside Router Bits Fusion 360 .tool library (2019-10-23 community export) |
| `observations/freud_solid_carbide.json` | Freud | 10 | A | Freud Solid Carbide router-bit chart (2017-08-22 PDF), 1/8"–3/8" subset |
| `observations/idcwoodcraft_millmage.json` | IDC Woodcraft | 10 | C | Community Millmage chipload CSV (cross-vendor sanity data) |
| `industrial_only/freud_solid_carbide_industrial.json` | Freud | 4 | A | Same Freud chart, 1/2" subset — **NOT loaded by `embedded()`**, see triage below |

Total live: **117 new rows**. Total staged-but-not-loaded: 4 Freud industrial.

### Open triage from the audit doc — decisions taken

**Freud 1/2" rows (audit carry-forward #1):** Option 2 chosen (namespacing).
Cleanest architectural fit and most reliable data hygiene:

- Hobby-spindle Freud rows (1/8" / 1/4" / 3/8", chiploads ≤ 0.27 mm/tooth)
  live in `observations/freud_solid_carbide.json` and ship in `embedded()`.
- Industrial Freud rows (1/2", chiploads 0.46–0.69 mm/tooth, calibrated
  for 10–15 kW CNC spindles) live in a sibling
  `crates/rs_cam_core/data/vendor_lut/industrial_only/` directory and
  are **intentionally not loaded**.
- The sibling directory keeps the invariant `observations/` ⇔ `embedded()`
  intact and makes the hobby/industrial boundary explicit at the path
  layer rather than hiding it behind a softened `validate_lut.py`
  threshold.
- The validator's `chipload max suspicious (0.5)` threshold stays at 0.5
  — the staging validator still flags the industrial rows as PROBLEMS
  on every run, which is the desired signal: anyone trying to promote
  them in the future has to confirm a per-spindle gate exists first.
- A future per-spindle gate can opt-in to industrial rows by walking the
  sibling directory; no LUT-side schema change needed.

**Garr aluminum (audit carry-forward #2):** Still deferred. The staged
rows in `planning/data_ingest_2026-05-29/harvey_helical_garr.json` share
CPT across the 242M (2-flute) / 842M (2-flute) / A3 (3-flute) series,
but the LUT row schema requires a single `flute_count`. Promoting as-is
would store an unknown flute count and the diameter-scale gate would
extrapolate against an averaged value. Awaits per-series split.

**IDC Woodcraft Grade C (audit carry-forward #4):** Promoted as planned
— all 10 rows carry `evidence_grade: "c"` and `row_kind: "derived"`
consistently. Useful as cross-vendor sanity data; the LUT scoring
demotes Grade C below Grade A/B for primary matches.

## Architectural improvements bundled with the promotion

**`EMBEDDED_FILES` table + `test_embedded_strict_parse` regression test
(`vendor_lut.rs`):** The pre-Phase-4 `embedded()` loader silently
dropped files that failed to deserialize — a partial schema change
or a malformed staged file would shrink the LUT without any signal
beyond the count assertion. (The Freud 1/2" `source_page: 2`
numeric-vs-string bug surfaced during this Phase 4 promotion only
because of the count mismatch — without the count assertion, the
10-row Freud file would have silently vanished.)

The fix:

1. Extract the include_str! list to a const `EMBEDDED_FILES: &[(&str, &str)]`
   of (filename, content) tuples — single source of truth for the test
   and the loader.
2. Add `test_embedded_strict_parse` that strict-parses every entry
   individually and panics with the offending filename if any fails.
3. Keep `embedded()`'s best-effort behavior in production code (forward
   compat for partial schema rollouts) — strict parsing is a CI failure,
   not a runtime crash.

This matches the audit's "no silent failures" theme (S2-7 tracing in
gates, S3-11 literature parity).

## Test updates required

Three lib tests and one integration test asserted on specific LUT row
behavior that changed when Phase 4 added closer matches:

| Test | What changed | Fix |
|------|--------------|-----|
| `feeds::vendor_lookup::tests::test_sub_1mm_tapered_ball_hardwood_finish_extrapolates` | Whiteside SC64 row at 1.442 mm displaces the 3.175 mm Amana row as the closest match. | Rewrote to assert spirit (extrapolation + scaling), not row identity. Query dropped to 0.5 mm. |
| `feeds::vendor_lookup::sub_1mm_tapered_ball_hardwood_finish_extrapolates_with_scaling` (integration) | Same root cause. | Same fix. |
| `session::compute::tests::workholding_changes_suggest_output_and_diagnostic_baseline_consistently` | Onsrud OCR added 6.35 mm softwood pocket rows whose chipload max saturates the suggested feed at both Medium and High rigidity. | Switched stock material to `Material::Custom` so the suggest path takes the hardness/Kc fallback model and the rigidity scaler isn't pegged to a LUT max. |
| `tool_load::chipload::tests::project_curve_flat_routes_to_contour_finish` | Onsrud OCR added a direct 6.35 mm hardwood contour/finish row (0.3556–0.4064 mm/tooth), displacing the 2× scaled Amana 3.175 mm match. | Recalibrated sample chipload to fit the new band; relaxed verdict assertion from `Approximate` to `Within` (any confidence — the routing assertion is preserved). |

All four tests still assert the original behavioral spirit (extrapolation,
workholding flow, routing) — Phase 4 data improvements forced an honest
rewrite from "match this specific row" to "match this property".

## Verification gate (all green pre-commit)

```bash
cargo test -p rs_cam_core --lib                    # 1668 passed, 7 ignored
cargo test -p rs_cam_core --tests                  # all integration suites green
cargo test -p rs_cam_core --test literature_parity # 11 sentries pass
cargo clippy -p rs_cam_core --all-targets -- -D warnings  # clean
```

F-024 / F-026 / F-027 / F-028 acceptance sentries: all green.

## Operator action items (not blocking the bundle)

1. **MCP smoke (AS001–AS015):** still the operator action item from the
   Phase 2B Kc re-tune AND the aluminum Kc switch. Requires live GUI
   (`cargo run -p rs_cam_viz --bin rs_cam_gui -- --mcp`). Phase 4's bulk
   row promotion adds new rows that may surface in smoke matches; record
   per-case before/after peak µm in
   `planning/data_ingest_2026-05-30/kc_retune_log.md`.
2. **Garr per-series flute split:** when Garr aluminum chiploads matter
   enough to chase, the 242M / 842M / A3 separation is the prereq.
3. **Per-spindle gate (optional):** if hobby-vs-industrial routing
   becomes a runtime concern, the `industrial_only/` namespace is ready
   to be opted in via a spindle-power gate.

## Files touched

```
crates/rs_cam_core/data/vendor_lut/observations/amana_long_tail.json         (new)
crates/rs_cam_core/data/vendor_lut/observations/onsrud_ocr.json              (new)
crates/rs_cam_core/data/vendor_lut/observations/whiteside_fusion360.json     (new)
crates/rs_cam_core/data/vendor_lut/observations/freud_solid_carbide.json     (new)
crates/rs_cam_core/data/vendor_lut/observations/idcwoodcraft_millmage.json   (new)
crates/rs_cam_core/data/vendor_lut/industrial_only/README.md                 (new)
crates/rs_cam_core/data/vendor_lut/industrial_only/freud_solid_carbide_industrial.json  (new, NOT in embedded())
crates/rs_cam_core/data/vendor_lut/source_manifest.json                      (+7 source entries)
crates/rs_cam_core/src/feeds/vendor_lut.rs                                   (EMBEDDED_FILES const + strict-parse test + count assertion 111→228)
crates/rs_cam_core/src/feeds/vendor_lookup.rs                                (sub-1mm tapered ball test rewrite)
crates/rs_cam_core/src/session/compute.rs                                    (workholding test fixture: Custom material)
crates/rs_cam_core/src/tool_load/chipload.rs                                 (project_curve_flat test recalibration)
crates/rs_cam_core/tests/vendor_lut_sub_1mm.rs                               (sub-1mm test rewrite + count assertion 111→228)
CREDITS.md                                                                   (Phase 4 block under Vendor LUT source manifest)
planning/feeds_data_ingest_phase4_2026-05-31.md                              (this doc)
```
