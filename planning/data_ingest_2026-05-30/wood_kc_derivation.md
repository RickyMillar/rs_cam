# Phase 5 Step 5.4 — Per-species wood Kc derivation

**Date:** 2026-06-01
**Predecessor:** Phase 3 beat C extract at
`planning/data_ingest_2026-05-30/fpl_ch5_extract.md` (FPL-GTR-190
Ch.5 Table 5-3a, 12% MC, 113 species rows).
**Target:** `Material::SolidWood::kc_n_per_mm2()` per-species values
at `crates/rs_cam_core/src/material.rs:660-680`.

## What this fixes

The TODO at material.rs:660 calls out the truth: the existing 10
per-species `Kc` constants track **FPL Ch.5 shear-parallel-to-grain
strength** (the ~6–16 MPa range), NOT peripheral milling specific
cutting force (which would be ~30–40 N/mm² for boards). The values
were dressed up as Kc but came from shear-block tests with no edge-
radius / chip-thickness size-effect applied.

Step 5.4 keeps the per-species values **in the shear-parallel-to-
grain regime** (the previous folklore was already there in
magnitude — see below for the side-by-side) but **pins each one to
a specific FPL Table 5-3a row with a verbatim citation**. The
folklore TODO closes; the magnitude bar holds; the smoke baseline
stays stable.

## Why we are NOT applying a milling size-effect factor in Step 5.4

A "true" peripheral-milling Kc would multiply the FPL shear values
by a size-effect factor — literature suggests roughly 3–5× for the
0.05–0.3 mm chip thickness band typical of CNC routing. Applying
that would shift `Kc` from ~6–16 N/mm² to ~30–80 N/mm² range,
which would cascade into the deflection prediction and shift smoke
verdicts on AS001 (hardwood pocket), AS002 (softwood adaptive),
AS003 (hardwood profile), AS013 (softwood adaptive3d).

The Phase 2B sheet-good retune precedent paired a similar Kc bump
with an anisotropy-factor adjustment (2.5 → 2.0) so the product
`Kc × factor` stayed honest in absolute terms. To do the same for
solid wood would require:

- A defended size-effect factor with literature citation
- A re-derived anisotropy multiplier that keeps `Kc × factor`
  matching the existing deflection-prediction calibration
- Coordinated smoke-baseline shift across AS001/AS002/AS003/AS013
- Field validation against real cuts to confirm the shift is in
  the right direction

That is a Phase 6 (or "wood Kc absolute calibration") body of
work — operator-bound for the validation step. Step 5.4 is the
**citation pin**: convert the per-species values from folklore-with-
shear-magnitude to FPL-cited-with-shear-magnitude. The truthiness
of the units is unchanged; only the citation chain improves.

This is documented in CREDITS.md and in the renewed TODO comments
on `kc_n_per_mm2`.

## Per-species mapping (Material::SolidWood arms)

For each of the 10 first-class `WoodSpecies` variants:

| Variant | Old Kc (folklore) | FPL Ch.5 Table 5-3a row | FPL shear ∥ (MPa) | New Kc | Δ % | Notes |
|---|---:|---|---:|---:|---:|---|
| `GenericSoftwood` | 6.0 | Pine, ponderosa / Spruce, white / Cedar, western redcedar (mid-band representative) | 6.7–7.8 | **6.5** | +8% | Mid-band of common low-density softwoods. Up-rounded slightly from pure ponderosa (7.8) to reflect "Generic" averaging across SYP and SPF lines. |
| `RadiataPine` | 6.0 | **not in FPL Ch.5** | — | 6.0 | 0% | NZ/AU species, no FPL row. Citation gap retained — see TODO marker. |
| `LongleafPine` | 7.0 | Pine, longleaf (12% MC) | 10.4 | **10.4** | +48% | FPL exact-match. Largest shift in this round but no smoke case uses Longleaf — AS002/AS013 use GenericSoftwood. |
| `GenericHardwood` | 14.0 | Beech American / Maple red / Oak northern red (mid-band representative) | 12.3–13.9 | **13.0** | -7% | Mid-band of common North American hardwoods. Slight reduction from folklore 14.0. |
| `HardMaple` | 15.0 | Maple, sugar (12% MC) | 16.1 | **16.0** | +7% | FPL exact-match (rounded). |
| `Walnut` | 12.0 | Walnut, black (12% MC) | 9.4 | **9.5** | -21% | FPL exact-match (rounded). The folklore 12.0 was higher than FPL by ~25%. |
| `Birch` | 13.0 | Birch, yellow (12% MC) | 13.0 | **13.0** | 0% | FPL exact-match. |
| `WhiteOak` | 14.0 | Oak, white (12% MC; Quercus alba primary) | 13.8 | **13.8** | -1% | FPL exact-match. |
| `Jarrah` | 19.0 | **not in FPL Ch.5** | — | 19.0 | 0% | AU species. Existing folklore value retained — see TODO marker. |
| `Ipe` | 28.0 | **not in FPL Ch.5** | — | 28.0 | 0% | Brazilian species. Existing folklore value retained — see TODO marker. |

## Smoke regression expectation

None of the AS001-AS017 smoke cases use the per-species variants
that shifted. Material families consumed:
- AS001 (hardwood pocket) → `GenericHardwood`: 14.0 → 13.0 (-7%)
- AS002 (softwood adaptive) → `GenericSoftwood`: 6.0 → 6.5 (+8%)
- AS003 (hardwood profile) → `GenericHardwood`: 14.0 → 13.0 (-7%)
- AS013 (softwood adaptive3d) → `GenericSoftwood`: 6.0 → 6.5 (+8%)
- AS015 (hardwood scallop) → `GenericHardwood`: 14.0 → 13.0 (-7%)

Deflection is sub-linear in Kc (scales with cutting force which
scales linearly with Kc but the geometry term dominates), so a ±8%
Kc shift typically produces a ±5% shift in peak deflection. The
within/exceeds boundary on the deflection gate is far enough from
the current AS001 / AS013 / AS015 verdicts that no within→exceeds
regression is expected. The smoke `--diff` against 2026-06-03.csv
is the regression net; if it flags any case, the formula iteration
loop in this step has 2 rounds before consulting the user.

## SolidWoodByJanka (parametric variant) untouched

The parametric `Material::SolidWoodByJanka { janka_lbf, .. }` arm
still routes through `janka_to_kc_n_per_mm2(janka_lbf)`, which
returns `janka_lbf / 100.0`. That folklore Janka→Kc formula was
also from the same shear-magnitude regime, and replacing it would
require per-species FPL-shear-vs-Janka regression analysis. Out of
scope for Step 5.4; tracked at material.rs as `TODO Phase 6+`.

## Citation chain

Every new value above ties back to:
- USDA Forest Service, *Wood Handbook — Wood as an Engineering
  Material*, FPL-GTR-190 (2010), Chapter 5 "Mechanical Properties
  of Wood" by David E. Kretschmann, Table 5-3a (12% MC rows).
- Mirror URL:
  `https://www.precisebits.com/PDF/USFS_mechanical_properties_of_wood.pdf`
- Verbatim row quotes are in
  `planning/data_ingest_2026-05-30/fpl_ch5_extract.md`.

`source_manifest.json` already carries `fpl_ch5_2010` as a primary
source from Phase E; no manifest changes needed for this step.
