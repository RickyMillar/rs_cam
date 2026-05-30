# Feeds Data Ingest — Phase B (Garr aluminum per-series flute split)

**Date:** 2026-05-31
**Predecessor:** Phase A baseline (`d654936`).
**Plan reference:** `planning/feeds_data_ingest_completion_2026-05-31.md`
("Phase B" section, design decision D4).

## What landed

Closed the Garr aluminum deferral that has been carried since
Phase 1C (`5ccf533`). The staged file
`planning/data_ingest_2026-05-29/harvey_helical_garr.json` held 10
Garr observations that shared CPT across multiple cutter series in a
single row — the live `VendorObservation` schema requires a single
`flute_count`, so the staged rows were unloadable without a split.

Promoted as `crates/rs_cam_core/data/vendor_lut/observations/garr_aluminum.json`
(11 rows, bundled count 228 → 239).

### Per-series promotion table

| New row id | Series | Flutes | Diameter | Op / pass | Chipload mm/tooth | Notes |
|------------|--------|--------|----------|-----------|-------------------|-------|
| `garr-242m-alum-slot-6000-flat-2f` | 242M | 2 | 6.0 | pocket / rough | 0.030–0.090 | Low-Range slot, Vc 125–180 |
| `garr-842m-alum-slot-6000-flat-2f` | 842M | 2 | 6.0 | pocket / rough | 0.030–0.090 | Low-Range slot, Vc 125–180 |
| `garr-a3-alum-slot-6000-flat-3f` | A3 | 3 | 6.0 | pocket / rough | 0.030–0.090 | Low-Range slot, Vc 125–180 |
| `garr-242m-alum-profile-6000-flat-2f` | 242M | 2 | 6.0 | contour / rough | 0.060–0.120 | Low-Range profile, Vc 150–200 |
| `garr-842m-alum-profile-6000-flat-2f` | 842M | 2 | 6.0 | contour / rough | 0.060–0.120 | Low-Range profile, Vc 150–200 |
| `garr-a3-alum-profile-6000-flat-3f` | A3 | 3 | 6.0 | contour / rough | 0.060–0.120 | Low-Range profile, Vc 150–200 |
| `garr-242m-alum-slot-3000-flat-2f` | 242M | 2 | 3.0 | pocket / rough | 0.015–0.045 | Low-Range slot, small-dia |
| `garr-842m-alum-slot-3000-flat-2f` | 842M | 2 | 3.0 | pocket / rough | 0.015–0.045 | Low-Range slot, small-dia |
| `garr-a3-alum-slot-3000-flat-3f` | A3 | 3 | 3.0 | pocket / rough | 0.015–0.045 | Low-Range slot, small-dia |
| `garr-a3-alum-hem-profile-6000-flat-3f` | A3 | 3 | 6.0 | adaptive / rough | 0.120–0.180 | High-Range HEM, ap=2xD, ae=30–40%xD |
| `garr-a3-alum-finish-6000-flat-3f` | A3 | 3 | 6.0 | contour / finish | 0.060 (single) | High-Range finish, ae=2.5%xD |

Total: **11 rows promoted**.

## Plan-vs-reality row count

The completion plan (D4) estimated **30 rows** under the assumption
"10 chart entries × 3 series". Reality:

- The staged file contained 10 Garr observations, but they aren't
  uniformly "1 entry shared across 3 series". The structure is:
  - **Low-Range page**: 3 entries × 242M/842M/A3 (the trio matches D4).
  - **Mid-Range page**: 2 entries × 142M/143M/A3 (different series set).
  - **High-Range page**: 2 A3-only entries (no split needed).
  - **General-Purpose** PDF: 3 entries with no series tag at all.
- D4 explicitly authorized only 242M (2-flute), 842M (2-flute), A3
  (3-flute). It did **not** pre-authorize 142M/143M flute counts.
- Per the project rule "no invention": rows whose flute count would
  have to be guessed are not promoted.

So the actual D4-authorized promotion is **11 rows**, not 30. The
plan number was an off-the-cuff multiplication based on an incomplete
read of the staging file; this doc records the honest decomposition.

## Skipped (NOT promoted in Phase B)

- **2 Mid-Range rows** (`garr-142m-alum-slot-6000-flat`,
  `garr-142m-alum-profile-6000-flat`): chart applies to 142M / 143M /
  A3, but the 142M and 143M flute counts aren't part of the D4 lock.
  Promoting only the A3 split of each would orphan the per-series
  intent; promoting all three would require flute counts not in the
  staged provenance or D4.
- **3 General-Purpose rows** (`garr-gp-alum-6000-flat`,
  `garr-gp-alum-3000-flat`, `garr-gp-plastics-6000-flat`): the
  General Purpose Milling Guide table is series-agnostic across
  Garr's whole catalog and the staging rows have
  `tool_subfamily: null`. There is no single published flute count
  to attach. Additionally, the GP plastics row carries
  `material_family: "plastic"` which is not a value in the live
  `MaterialFamily` enum — even with a flute count it would not
  deserialize.

**Future re-open path:** if Mid-Range 142M/143M flute counts can be
cited from Garr's general catalog (separate PDF, not the milling
guide), promote with the same split pattern. GP rows would need a
"generic / unbinned" flute-count convention or schema relaxation —
out of scope for this round.

## Architectural notes

- Mirrors the Phase 4 pattern: one `include_str!` line in
  `EMBEDDED_FILES`, count assertion bump in both
  `feeds/vendor_lut.rs` and `tests/vendor_lut_sub_1mm.rs`, source
  manifest entry per source PDF.
- `test_embedded_strict_parse` covers the new file automatically
  (no new test needed) — any future schema drift surfaces with the
  filename intact.
- Pass-role normalization: staged rows used `"rough"`, live schema
  serializes `LutPassRole::Roughing` as `"roughing"`. New rows use
  `"roughing"` to match.
- `tool_subfamily` is the per-series identifier (`"242m"`, `"842m"`,
  `"a3"`) so future filtering by series — e.g. a runtime selector
  picking 842M for high-helix or A3 for HEM — is keyed on a
  single-series string, not a combined trio.

## Verification gate

```bash
cargo test -p rs_cam_core --lib                              # passes
cargo test -p rs_cam_core --tests                            # passes
cargo test -p rs_cam_core --test literature_parity           # 11 sentries pass
cargo clippy -p rs_cam_core --all-targets -- -D warnings     # clean
```

F-024 / F-026 / F-027 / F-028 acceptance sentries: green.

## Files touched

```
crates/rs_cam_core/data/vendor_lut/observations/garr_aluminum.json   (new, 11 rows)
crates/rs_cam_core/data/vendor_lut/source_manifest.json              (+2 source entries)
crates/rs_cam_core/src/feeds/vendor_lut.rs                           (EMBEDDED_FILES + count 228→239)
crates/rs_cam_core/tests/vendor_lut_sub_1mm.rs                       (count 228→239)
CREDITS.md                                                           (Phase B block)
planning/feeds_data_ingest_phaseB_2026-05-31.md                      (this doc)
```
