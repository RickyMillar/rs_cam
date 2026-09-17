# `material/` — the material catalogue and the wood species library

What a workpiece is made of, and the force constants the feeds and
tool-load layers read from it. The principal type is `Material` in `mod.rs`.

## Files

- `mod.rs` — `Material`, `WoodSpecies` and the other grade enums,
  `MaterialCategory`, the GUI picker merge, and the per-material constants:
  Janka hardness, `kc_n_per_mm2`, cutting speed, plunge and drill envelopes.
- `wood_species_library.rs` — the loader for the ~130-row Janka library
  behind `Material::SolidWoodByJanka`; `data/wood_species.toml` holds it.

## Invariants

- `kc_n_per_mm2` returns `None` when the material has no primary
  measurement. The tool-load gates then refuse with
  `UnmodeledReason::MaterialUnvalidated`. They do not predict force from a
  fabricated constant.
- `Some(value)` does not mean "cited". Three `WoodSpecies` arms
  (`RadiataPine`, `Jarrah`, `Ipe`) carry a folklore base, named
  `KC_FOLKLORE_*`. `WoodSpecies::kc_provenance` is the only thing that tells
  a citation from a guess; the number alone cannot. Every new species arm
  gets a tag.
- `KcProvenance` tags the first-class `WoodSpecies` arms only. A
  `wood_species_library()` row feeds `Material::SolidWoodByJanka`, whose Kc
  comes from the janka/100 approximation; its provenance column is
  `source_id`.
- The species library is DATA. Add, edit or remove a species in
  `data/wood_species.toml`, then re-pin `LIBRARY_CANONICAL_HASH` in the
  sentry. Do not write a Rust row. The file is embedded with `include_str!`,
  so a run needs no file on disk.
- Every library `source_id` must name an entry in
  `data/vendor_lut/source_manifest.json`.

## Sentries

- `cargo test -p rs_cam_core -q --test wood_species_library_provenance`
- `cargo test -p rs_cam_core -q --test efficiency_abstains_without_kc_g_specenergy`
- `cargo test -p rs_cam_core -q --test _litmatrix_ipe_janka_scaling`
