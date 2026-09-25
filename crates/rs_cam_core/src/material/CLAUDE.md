# `material/` — the material catalogue and the wood species library

What a workpiece is made of, and the one force line the feeds and tool-load
layers read from it. The principal type is `Material` in `mod.rs`.

## Files

- `mod.rs` — `Material`, `WoodSpecies` and the other grade enums,
  `MaterialCategory`, the GUI picker merge, and the per-material constants:
  Janka hardness, `force_line()`, `WoodSpecies::fpl_density`, cutting speed,
  plunge and drill envelopes.
- `force_line.rs` — `ForceLine`, `ForceLineRefusal`, `WoodDensity`,
  `ChipRegime` (ruling B6): the Curti 2021 density law, the Goli 2018 MDF
  line, FPL Ch.4 Eq. (4-11), the card and hover texts.
- `wood_species_library.rs` — the loader for the ~130-row Janka library
  behind `Material::SolidWoodByJanka`; `data/wood_species.toml` holds it.

## Invariants

- One line per material: `Material::force_line()`. Solid wood is the Curti
  2021 helix-0 envelope times ρ = SG × 1120 (FPL Table 5-3a, or Table 5-5a
  Gb by Eq. 4-11); MDF is Goli 2018. Every other material refuses with a
  named `ForceLineRefusal`, and the gates refuse with `MaterialUnvalidated`.
- No scalar `Kc`, no anchor, no grain factor other than 1.0. Never type an
  envelope value or a density as a literal: store the printed coefficient
  or the printed SG, and cite the row.
- ρ outside 287-1080 kg/m³ refuses (Ipe, 1207). A mean chip outside the
  printed range does not refuse; `ChipRegime` names it.
- A library row reads its `specific_gravity_12` (FPL rows only); a Wood
  Database row refuses `NoDensity`. The Janka value never reaches the line.
- The species library is DATA. Edit `data/wood_species.toml`, then re-pin
  `LIBRARY_CANONICAL_HASH` in the sentry. Every library `source_id` must
  name an entry in `data/vendor_lut/source_manifest.json`.

## Sentries

- `cargo test -p rs_cam_core -q --test the_force_line_is_printed_per_family_b6`
- `cargo test -p rs_cam_core -q --test wood_species_library_provenance`
- `cargo test -p rs_cam_core -q --test efficiency_abstains_without_kc_g_specenergy`
- `cargo test -p rs_cam_core -q --test _litmatrix_ipe_janka_scaling`
