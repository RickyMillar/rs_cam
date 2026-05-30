# Feeds Data Ingest — Phase D (PlasticFamily expansion + Rockwell R)

**Date:** 2026-05-31
**Predecessor:** Phase C (`1ebf55e`).
**Plan reference:** `planning/feeds_data_ingest_completion_2026-05-31.md`
("Phase D" section, design decision D2).

## What landed

Six new `PlasticFamily` variants behind the existing `hardness()`
canonical accessor — Phase D of the completion plan. Bumps plastic
coverage from 5 families (Generic, Acrylic, HDPE, Delrin,
Polycarbonate) to 11.

Adds `PlasticHardness::RockwellR` variant (separate scale from
Rockwell M; 1/2" ball / 60+100 kgf for the softer engineering
plastics that report on it).

### New variants

| Variant | Material | Hardness | Source (verbatim quote in staging) |
|---------|----------|----------|-----------------------------------|
| `UhmwPe` | UHMW-PE (Mitsubishi TIVAR 1000 Natural Virgin) | Shore D 66 (ASTM D2240) | TIVAR 1000 datasheet |
| `Polypropylene` | PP-H (SIMONA + Direct Plastics PP-H) | Shore D 70 (ASTM D2240 + ISO 868) | Two-source corroborated |
| `Nylon66` | Nylon 6/6 (Mitsubishi Nylatron GS, MoS2-filled cast machinable) | Shore D 85 (ASTM D2240) | Nylatron GS datasheet (also reports M 85 / R 115) |
| `Abs` | ABS (MakeItFrom material-group) | Rockwell R 105 (midpoint of 100-110 range) | MakeItFrom material properties |
| `Petg` | PETG (Plaskolite VIVAK Sheet) | Rockwell R 115 (ASTM D-785) | VIVAK datasheet |
| `RigidPvc` | Rigid PVC Type 1 (Interstate AM ASTM D-1784 class 12454-B) | Shore D 74 (scale-only — D2240 implicit) | Interstate AM product page |

All values read verbatim from a fetched datasheet — no
interpolation, no scale conversion. The ABS midpoint is a single
arithmetic step (`(100+110)/2`); the staging doc records this as
the canonical way to surface a MakeItFrom range as a single scalar.

### Kc treatment (refusal-first stays the rule)

**None of the 6 new families have a fetched milling-regime Kc.**
All return `kc_n_per_mm2() = None`. Refusal stays the rule per D2 +
the staging docs:

- UHMW-PE, Polypropylene, Nylon 6/6: no primary milling Kc study
  found in any data_ingest round.
- ABS, PETG, Rigid PVC: same — no manufacturer Kienzle coefficient
  and no peer-reviewed milling-force measurement on file.
- PMMA Round-2 (`kc_extra.md`) found a 276.5 N/mm² nanoscale value
  with an **explicit "do NOT promote — size-effect inflated"
  caveat**; that caveat stands and PMMA stays None (per the
  completion plan's "explicitly out of scope" list).
- POM/Delrin and PC remain genuine gaps with no primary source.

The literature_parity sentry `plastics_without_primary_source_refuse_kc`
now exercises all 10 None-returning families (4 pre-existing + 6 new)
so a future blind bump that adds a fabricated constant will fire
this test immediately.

## Plastic hardness routing through `vendor_normalize::material_to_lut`

The LUT's `MaterialFamily` enum has only Acrylic / Hdpe /
Polycarbonate / Delrin on the plastic side. The 6 new families
route by chemistry to the closest existing bin:

- UhmwPe, Polypropylene → MaterialFamily::Hdpe (polyolefin bin)
- Nylon66 → MaterialFamily::Delrin (engineering thermoplastic bin —
  POM and PA are both crystalline engineering polymers)
- Abs, Petg, RigidPvc → MaterialFamily::Acrylic (rigid amorphous bin)

These are best-effort routing hints, NOT citation-backed mappings.
The LUT doesn't carry vendor rows for the new families yet, so a
lookup will miss cleanly and the formula fallback takes over —
the bin choice has minimal practical impact today. A future
`MaterialFamily` expansion can replace these aliases with per-family
bins as vendor rows land.

## Rockwell R handling in `vendor_normalize`

`PlasticHardness::RockwellR` is folded into the same `(HardnessKind::ShoreD, v)`
surfacing as `RockwellM` — the LUT's `HardnessKind` enum still has
only Janka / Hb / ShoreD. Doc-comment updated to call out the new
scale alongside the existing M-scale carry-through. TODO Phase 3+:
plumb a Rockwell `HardnessKind` through `vendor_lut.rs` once a
vendor LUT row records a Rockwell hardness (currently none do).

## S2-9 LimitInputs deferral — explicit non-fold

The completion plan listed "Bundle audit deferral S2-9 (leaky
LimitInputs)" as a Phase D step. After surveying the actual code,
S2-9 doesn't share code paths with the plastic expansion:

- Plastic Kc lookup lives in `material.rs::Material::kc_n_per_mm2`
  and `feeds/vendor_normalize.rs::material_to_lut`.
- S2-9 LimitInputs leak lives in `feed_modulation.rs` (struct
  definitions) and `session/compute.rs` lines 1502+ (callers that
  pre-multiply `Kc × GRAIN_ANISOTROPY_FACTOR`).

They share a *principle* (canonical accessors as single source of
truth) but the edits don't overlap. Per the project rule
"consolidate, don't patch — but don't bundle just to bundle":
S2-9 is **deferred** to its own surgical commit when prioritized.
The audit doc's S2-9 entry stays accurate; no false claim of
completion.

## Wiring summary

- `material.rs::PlasticFamily` — +6 variants with per-variant doc
  citations
- `material.rs::PlasticHardness` — +RockwellR variant
- `PlasticFamily::label()` / `PlasticFamily::hardness()` — extended
- `Material::kc_n_per_mm2()` Plastic arm — None for all 6 new
- `Material::catalog()` / `to_key()` / `from_key()` — +6 entries
  (snake_case keys: uhmw_pe, polypropylene, nylon66, abs, petg,
  rigid_pvc)
- `Material::test::plastic_kc_only_some_when_validated` — extended
  to cover all 10 None families
- `Material::test::plastic_hardness_preserves_scale` — extended to
  cover all 6 new families (matches! against the exact scale variant
  to catch silent Shore D ↔ Rockwell flips)
- `feeds/vendor_normalize.rs::material_to_lut` — extended PlasticFamily
  match for the 6 new families (chemistry-routed bins), and
  PlasticHardness match for the new RockwellR scale
- `tests/literature_parity.rs` — +6 sentries (one per new family),
  `plastics_without_primary_source_refuse_kc` extended to cover all
  10 None families

## Verification gate (all green pre-commit)

```bash
cargo test -p rs_cam_core --lib              # 1668 passed
cargo test -p rs_cam_core --tests            # 1854 passed (was 1848; +6 sentries)
cargo test -p rs_cam_core --test literature_parity # 22 passed (was 16)
cargo clippy -p rs_cam_core --all-targets -- -D warnings # clean
```

F-024 / F-026 / F-027 / F-028 acceptance sentries: green.
`test_key_roundtrip` exercises every new key automatically.

## Files touched

```
crates/rs_cam_core/src/material.rs               (PlasticFamily +6, PlasticHardness +RockwellR, all match arms extended)
crates/rs_cam_core/src/feeds/vendor_normalize.rs (PlasticFamily routing for 6 new, RockwellR carry-through)
crates/rs_cam_core/tests/literature_parity.rs    (+6 sentries; refusal test extended)
CREDITS.md                                        (Phase D plastic-family block)
planning/feeds_data_ingest_phaseD_2026-05-31.md  (this doc)
```
