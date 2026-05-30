# Feeds Data Ingest — Phase C (AluminumAlloy expansion)

**Date:** 2026-05-31
**Predecessor:** Phase B (`585e2d8`).
**Plan reference:** `planning/feeds_data_ingest_completion_2026-05-31.md`
("Phase C" section, design decision D3).

## What landed

Five new `AluminumAlloy` variants behind the existing `brinell_hb()`
canonical accessor — Phase C of the completion plan. Bumps the
aluminum coverage from 2 alloys (6061-T6, 7075-T6) to 7.

### New variants

| Variant | Alloy / temper | Brinell HB | Source class | Citation |
|---------|----------------|------------|--------------|----------|
| `Alloy2024T3` | 2024-T3 (aerospace structural) | 120 | ASM matweb (500 g, 10 mm ball) | `hardness_extra.md` H.2 |
| `Alloy5052H32` | 5052-H32 (marine/sheet) | 60 | ASM matweb (500 g, 10 mm ball) | `hardness_extra.md` H.2 |
| `Alloy3003H14` | 3003-H14 (general sheet) | 42 | MakeItFrom (ASM not hosted) | `hardness_extra.md` H.2 |
| `Alloy1100O` | 1100-O (commercially pure, annealed) | 23 | MakeItFrom (ASM not hosted) | `hardness_extra.md` H.2 |
| `Alloy7050T7651` | 7050-T7651 (aerospace plate, SCC-resistant) | 147 | ASM matweb (calc); Kaiser mill = 150 | `hardness_extra.md` H.2 |

All five values are read verbatim from a fetched datasheet — no
interpolation, no scale conversion. ASM matweb returns HTTP 500 for
`ma1100*` / `ma3003*`, so MakeItFrom serves as the primary citation
for those two. 7050-T7651 has two consistent citations (147
calc vs 150 measured); we anchor to the matweb 147 to stay
consistent with the matweb-primary pattern used for 6061 / 7075.

### Where the variants are wired

- `material.rs::AluminumAlloy` — variants added with per-variant
  doc comments citing the staging source.
- `material.rs::AluminumAlloy::brinell_hb()` — extended match arm.
  Return type stays `f64`, NOT `u32` (the completion-plan D3 text
  said `u32`, but the existing live API is `f64` and downstream
  callers like `Material::hardness_index` rely on the float
  semantics; keeping the existing signature is the cleaner choice).
- `material.rs::AluminumAlloy::label()` — extended match arm.
- `Material::catalog()` — 5 new entries appended (drives GUI
  dropdowns automatically; no GUI-side enum match arms exist).
- `Material::to_key()` / `Material::from_key()` — 5 new key bindings
  (`"aluminum_2024_t3"` etc.). Snake-case + lowercase to match the
  existing 6061/7075 keys.
- `material.rs::tests::aluminum_brinell_matches_asm_anchors` —
  rewritten to a table-driven assertion over all 7 alloys (catches
  any future hand-typed mismatch).
- `material.rs::tests::aluminum_kc_computes_from_kienzle_pair` —
  extended to iterate over all 7 alloys (Kc remains the single
  shared VDI 3323 group-22 Kienzle pair per D3 of the plan).

### Literature_parity sentries (5 new)

11 → 16 sentries:

| Sentry | Anchors |
|--------|---------|
| `aluminum_brinell_2024_t3_matches_asm_anchor` | ASM matweb 2024-T3 = 120 HB |
| `aluminum_brinell_5052_h32_matches_asm_anchor` | ASM matweb 5052-H32 = 60 HB |
| `aluminum_brinell_3003_h14_matches_makeitfrom_anchor` | MakeItFrom 3003-H14 = 42 HB |
| `aluminum_brinell_1100_o_matches_makeitfrom_anchor` | MakeItFrom 1100-O = 23 HB |
| `aluminum_brinell_7050_t7651_matches_asm_anchor` | ASM matweb 7050-T7651 = 147 HB (calc) |

Each sentry pins the live `brinell_hb()` value to the verbatim
staged citation. The existing `aluminum_brinell_matches_asm_anchors`
sentry is unchanged so the 6061 / 7075 anchors stay tested in their
original form.

## Architectural notes

- **Canonical-accessor pattern preserved.** `brinell_hb()` stays the
  single source of truth for aluminum hardness (parallels
  `PlasticFamily::hardness()` for plastics). Downstream code reads
  through the accessor, never duplicates the value.
- **Plan rough-vs-reality:** the plan said "Add
  `AluminumAlloy::brinell_hb() -> u32` accessor". The function
  already existed and returned `f64`. Changing the return type would
  cascade through `Material::hardness_index` (which does
  `(brinell / 60.0).powf(0.4)` and needs float). Kept `f64`.
- **GUI integration:** the GUI's material picker is driven from
  `Material::catalog()` — adding the 5 new entries there surfaces
  them in the dropdown automatically. No GUI-side `AluminumAlloy::`
  match arms exist; exhaustive-match coverage at the enum-level
  pattern sites in `material.rs` is the safety net.
- **CLI smoke** in `crates/rs_cam_cli/src/smoke.rs` parses material
  strings against a small allowlist (`"6061"`, `"7075"`, etc.) and
  falls through to `None` for unknown keys. New alloys are reachable
  via `Material::from_key("aluminum_2024_t3")` etc. through the
  project-file IO path; explicit CLI smoke aliases can be added if
  needed but aren't required by D3.

## Verification gate (all green pre-commit)

```bash
cargo test -p rs_cam_core --lib              # 1668 passed
cargo test -p rs_cam_core --tests            # 1848 passed (was 1843; +5 sentries)
cargo test -p rs_cam_core --test literature_parity # 16 passed (was 11)
cargo clippy -p rs_cam_core --all-targets -- -D warnings # clean
```

F-024 / F-026 / F-027 / F-028 acceptance sentries: green.
`test_key_roundtrip` exercises every new key automatically.

## Files touched

```
crates/rs_cam_core/src/material.rs              (AluminumAlloy +5 variants, brinell_hb/label/catalog/to_key/from_key extended, 2 in-crate tests now table-driven)
crates/rs_cam_core/tests/literature_parity.rs   (+5 sentries)
CREDITS.md                                       (Phase C aluminum extension block)
planning/feeds_data_ingest_phaseC_2026-05-31.md (this doc)
```
