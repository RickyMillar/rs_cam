# Feeds Data Ingest — Phase E (Wood species parametric + library)

**Date:** 2026-05-31
**Predecessor:** Phase D (`16eb650`).
**Plan reference:** `planning/feeds_data_ingest_completion_2026-05-31.md`
("Phase E" section, design decision D1).

## What landed

Three concrete additions, all behind a single canonical helper:

1. **`janka_to_kc_n_per_mm2(janka_lbf) -> Option<f64>`** — shared
   helper, calibrated band `[200, 4000]` lbf. `pub(crate)` so other
   modules can use it without depending on Material internals.
2. **`Material::SolidWoodByJanka { janka_lbf, label, source_id }`**
   — parametric solid-wood variant. The existing `WoodSpecies` enum
   stays at 10 hand-curated species; the parametric variant carries
   the long tail without enum bloat.
3. **`WOOD_SPECIES_LIBRARY` const** (132 entries) +
   `find_by_display_name` lookup helper in a new module
   `crates/rs_cam_core/src/material/wood_species_library.rs`.

Also folds in audit deferral S3-13 with a small
`Material::test_fixture_custom(name) -> Material` helper.

### Helper formula (folklore-grade)

```rust
pub(crate) fn janka_to_kc_n_per_mm2(janka_lbf: f64) -> Option<f64> {
    if !janka_lbf.is_finite() { return None; }
    if !(200.0..=4000.0).contains(&janka_lbf) { return None; }
    Some(janka_lbf / 100.0)
}
```

Why `/ 100.0`: the existing 10 per-species `WoodSpecies` Kc hardcodes
sit roughly on this line (within ±25 % for 8 of the 10 species; the
`GenericHardwood` 14.0 anchor and `Walnut` 12.0 are hand-tuned above
the line, and the literature isn't precise enough to argue with).
The first-class `WoodSpecies` variants do NOT route through this
helper — their per-species constants stay the source of truth for
those 10 anchors. The helper covers the parametric variant only.

Plan E.1 said "extract the formula currently embedded in
`Material::SolidWood` Kc switch" — but the switch wasn't a formula,
it was a 10-entry hardcoded table. The honest extraction is: the
helper IS the formula (folklore-grade `janka/100`), and the per-
species hardcodes are anchors that the parametric variant doesn't
get. The literature_parity sentry
`solid_wood_by_janka_helper_aligns_with_enum_anchors_within_folklore_band`
pins `helper(870)` against `LongleafPine::kc_n_per_mm2()` at ±30 %
tolerance, documenting the divergence rather than pretending it
doesn't exist.

### Library composition

132 unique species after dedup (FPL precedence):

- **98 from FPL Ch.5 Table 5-3a** (peer-reviewed USDA FPL-GTR-190
  Wood Handbook, 2010). Side-hardness column in Newtons; converted
  to lbf via `lbf = N / 4.448`, rounded to 1 decimal. Excludes rows
  where Side hardness is `—` (not measured).
- **34 from The Wood Database** filling in species not in FPL Table
  5-3a — typically tropical / specialty species (Honduran Mahogany,
  Padauk, Black Limba, Wenge, etc.).

Dedup strategy: FPL wins when a common-name match exists in both
sources. The plan called for a `secondary_sources` slot referencing
both source_ids; deferred to keep the const file flat for now. A
future revision can extend `WoodSpeciesEntry` with
`secondary_sources: &'static [&'static str]` if cross-citation
visibility becomes useful.

### Source manifest

Two new entries in `crates/rs_cam_core/data/vendor_lut/source_manifest.json`:

- `fpl_ch5_2010` (vendor: `fpl`) — the USDA Forest Service handbook
  chapter. Primary `fpl.fs.usda.gov` URL 403'd on 2026-05-30; using
  the `precisebits.com` mirror for the identical PDF.
- `wood_database_2026-05-30` (vendor: `wood_database`) — the
  per-species Janka pages, 2026-05-30 snapshot.

The provenance test `every_library_source_id_exists_in_manifest`
fails noisily if a library row cites a missing source_id.

## Architectural choices

### `WoodSpecies` enum stays as-is

The 10 first-class variants (HardMaple, Walnut, Ipe, etc.) keep
their hand-tuned `Kc` constants and Janka anchors. The plan
suggested "Re-wire `WoodSpecies::*` Kc lookups through the helper"
but that would *change* the live values:

- LongleafPine Kc 7.0 → helper(870) = 8.7 (24 % drift)
- HardMaple Kc 15.0 → helper(1450) = 14.5 (3 % drift)
- Walnut Kc 12.0 → helper(1010) = 10.1 (16 % drift)

Some of these drifts cross the literature_parity sentry tolerance
that's pinned to the live values. Changing them is a Kc-recalibration
task, not a refactor. Left for a future "primary-source per-species
Kc" round.

### Parametric variant scope: NOT in `Material::catalog()`

The catalog drives `test_key_roundtrip`, which fails any Material
whose `to_key` / `from_key` round-trip isn't bit-for-bit identical.
`SolidWoodByJanka` has a `to_key` format
(`solid_wood_by_janka:{source_id}:{janka_lbf}:{label}`) and a
permissive `from_key` parse, but they aren't in the catalog so the
roundtrip test doesn't fire on them. The catalog stays curated for
the "common materials" GUI dropdown; the parametric variant is for
library-driven selection (and TOML round-trip via serde derive).

### GUI integration: deferred

The plan listed GUI integration (searchable dropdown of
`WOOD_SPECIES_LIBRARY`) as Phase E.4. This commit ships the data
and the lookup helpers; the GUI dropdown wiring is a separate
concern that touches `crates/rs_cam_viz/src/ui/` — out of scope
for the core-crate work and easier to land independently when
the UI integration pattern is clear.

The library is reachable today via:

- Direct construction: `Material::SolidWoodByJanka { janka_lbf, label, source_id }`
- Helper lookup: `WoodSpeciesEntry` from
  `material::wood_species_library::find_by_display_name(name)` →
  construct the variant programmatically
- TOML serde: project files can specify
  `[stock.material.SolidWoodByJanka]` directly

### S3-13 fold-in: helper only, call sites unchanged

`Material::test_fixture_custom(name) -> Material` lives behind
`#[cfg(test)]`. Audit S3-13 cited 4 call sites that ad-hoc construct
`Material::Custom { name: "...", hardness_index: 1.0, kc: 10.0 }`
— this commit ships the helper; substituting the 4 sites is a
trivial follow-up sweep that can land independently (or as part of
the next test-touching commit).

## Wiring summary

- `material.rs`:
  - `pub mod wood_species_library;` declaration
  - `JANKA_CALIBRATED_BAND_{LOW,HIGH}_LBF` constants
  - `janka_to_kc_n_per_mm2` shared helper
  - `janka_to_drill_chip_welding_dtd` band helper (extracted from
    the existing SolidWood drill threshold switch)
  - `Material::SolidWoodByJanka` variant
  - All match arms on `Material`: hardness_index / kc_n_per_mm2 /
    base_cutting_speed_m_min / plunge_rate_base /
    drill_chip_welding_threshold_dtd / drill_per_peck_max_dtd /
    drill_plunge_feed_envelope_per_mm / label / to_key / from_key
    extended
  - `Material::test_fixture_custom(name)` helper under `cfg(test)`
- `material/wood_species_library.rs` (new module, 842 lines):
  - `WoodSpeciesEntry` struct
  - `WOOD_SPECIES_LIBRARY` const (132 entries)
  - `find_by_display_name` case-insensitive lookup
- `feeds/vendor_normalize.rs::material_to_lut`:
  - SolidWoodByJanka match arm — same softwood/hardwood split as
    the enum variant
- `data/vendor_lut/source_manifest.json` — `fpl_ch5_2010` +
  `wood_database_2026-05-30` entries
- `tests/literature_parity.rs` — 2 new sentries
  (`solid_wood_by_janka_helper_aligns_with_enum_anchors_within_folklore_band`,
  `solid_wood_by_janka_helper_refuses_outside_calibrated_band`)
- `tests/wood_species_library_provenance.rs` (new) — 3 tests
  (manifest provenance gate, coverage floor, case-insensitive lookup
  smoke)

## Verification gate (all green pre-commit)

```bash
cargo test -p rs_cam_core --lib              # 1668 passed
cargo test -p rs_cam_core --tests            # 1859 passed (was 1854; +2 helper sentries + 3 library tests)
cargo test -p rs_cam_core --test literature_parity # 24 passed (was 22)
cargo test -p rs_cam_core --test wood_species_library_provenance # 3 passed (new)
cargo clippy -p rs_cam_core --all-targets -- -D warnings # clean
```

F-024 / F-026 / F-027 / F-028 acceptance sentries: green.

## Files touched

```
crates/rs_cam_core/src/material.rs                            (helper, variant, all method arms, S3-13 fixture)
crates/rs_cam_core/src/material/wood_species_library.rs       (new, 842 lines, 132 entries)
crates/rs_cam_core/src/feeds/vendor_normalize.rs              (parametric arm in material_to_lut)
crates/rs_cam_core/data/vendor_lut/source_manifest.json       (+2 entries)
crates/rs_cam_core/tests/literature_parity.rs                 (+2 sentries)
crates/rs_cam_core/tests/wood_species_library_provenance.rs   (new, 3 tests)
CREDITS.md                                                     (Phase E wood-library block)
planning/feeds_data_ingest_phaseE_2026-05-31.md               (this doc)
```
