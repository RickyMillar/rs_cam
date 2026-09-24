# G7: the engine's Kc constants against the documents they cite

Date: 2026-09-24. Read-only audit. No file outside `fetch/G7/` changed.

The G7 "LUT" is not the vendor chipload LUT. It is the `Kc` table in
`crates/rs_cam_core/src/material/mod.rs` (`Material::kc_n_per_mm2`) and the
force anchor in `crates/rs_cam_core/src/feeds/force.rs`. This file compares
each constant with the stored text it cites. Row ids refer to
`candidate_rows.json`. Anything marked **derived** is a calculation of mine,
not a printed number.

## How the engine uses Kc (context)

- `force::affine_coefficients_for_kc(kc)` returns
  `Ks = 49.95 · kc / 35.1` (N/mm²) and `F_edge = 5.30 · kc / 35.1` (N/mm).
- `tool_load::power::PowerTerms::of` multiplies both terms by
  `GRAIN_ANISOTROPY_FACTOR = 2.0`.
- So every material's force and power are the Kopecký MDF line (D1),
  scaled by `kc / 35.1`. The value called "Kc" is only a ratio to the
  `GenericHardwood` value 35.1.

Engine coefficients per material (**derived** from the constants above):

| Material (engine) | "Kc" | Ks N/mm² | F_edge N/mm |
|---|---|---|---|
| GenericSoftwood (2.7 × 6.5) | 17.55 | 24.98 | 2.65 |
| GenericHardwood (2.7 × 13.0) | 35.1 | 49.95 | 5.30 |
| SheetGood::Mdf | 31.4 | 44.68 | 4.74 |
| SheetGood::Particleboard | 35.0 | 49.81 | 5.28 |
| Plywood::BalticBirch | 13.0 | 18.50 | 1.96 |
| Plywood::HardwoodFaced | 11.0 | 15.65 | 1.66 |
| Plywood::Softwood | 8.0 | 11.38 | 1.21 |
| Plastic::Hdpe | 40.0 | 56.92 | 6.04 |

## D1. The force anchor is an MDF fit, attached to hardwood, with an unknown chip width

- Engine: `force.rs` names `LIT_KS 49.95` and `LIT_FEDGE 5.30` "the
  woodresearch.sk 201905/12 quasi-orthogonal conventional fit" and attaches
  them to `GenericHardwood` as "a mid-hardwood close to the study's species
  class".
- Document (`g7_kopecky2019_mdf_quasi_orthogonal`), verbatim:
  "The paper is focused on the analysis of cutting forces in milling of MDF";
  "Sample: Medium density fibreboard (MDF)"; "Conventional milling: Fc1z =
  49.954 hm + 5.3047".
- Finding 1: the material is **MDF (684 kg/m³)**, not a hardwood. The
  comment is wrong.
- Finding 2: `Fc1z` is in N per tooth. The paper does not print the chip
  width that it covers. The engine reads it as N per mm of edge. Back-
  calculation from the paper's Eq. 8 and Tab. 1 (**derived**): the slope
  gives b = 17.8 mm, close to the 18 mm board; the intercept gives
  b = 11.8 mm. If b is about 18 mm, the per-mm line is about
  `2.8·h + 0.29` N/mm, which is 11–18 times lower than the engine's line.
- Cross-check against the other MDF measurement (Goli 2018, row
  `x-g7-goli2018-mdf-up-straight`, printed per mm of edge): `Ks 31.44`,
  `Int 3.36`. The engine's MDF line is `44.68·h + 4.74`, which is 1.42× the
  direct measurement on both terms (**derived**). The per-mm reading of
  Kopecký (49.95 / 5.30) is 1.6× Goli; the per-18-mm reading is 0.09×.
  Neither reading agrees with Goli within the stated SD (2.68).
- Which way the evidence leans (a check for the reconciler, not a ruling):
  the per-mm reading (49.95 N/mm²) is 1.6× Goli's MDF 31.44 and of the same
  order as Curti 2021 at 700 kg/m³ (Ks 18–53, **derived**) and Pałubicki's
  total kc 32–38. The per-18-mm reading (about 2.8) is about 10× below every
  independent measurement here. But the paper's own Tab. 1 gives
  τγ = 1.1757 MPa, which is 15–20× below the shear yield stress that
  Orlowski and Ochrymiuk 2017 print for Scots pine from cutting tests
  (τγ⊥ = 24.82, τγ∥ = 18.40 MPa; redalyc 48550384003, read, not stored).
  So the paper's units are internally suspect, and it cannot anchor the
  engine under either reading.
- Background: `planning/UNIFIED_LOAD_MODEL_2026-06-18.md` §6 records the
  choice to attach the fit to `GenericHardwood` "as the anchor wood"; it
  does not say that the study material is MDF.
- Consequence: every force and power number in the engine scales from this
  anchor. The reconciler must decide the anchor. Proposal for the record
  (not a fix): anchor on a document that prints the force per mm of edge
  (Goli 2018 for MDF; Curti 2021 for solid wood, D5), and use Kopecký only
  for the slope-to-intercept shape, if at all.

## D2. MDF Kc 31.4 is an affine slope; the engine treats it as a scale factor

- Engine: `SheetGoodKind::Mdf => 31.4` with the comment "PMC6315737
  round-shape Ks for MDF: average 31.44".
- Document (`g7_goli2018_round_shape_ks`), verbatim:
  "[row] Ks [N mm−2] | 31.44 (2.68) | 25.81 | 35.58" and
  "[row] Int [N mm−1] | 3.36 (0.27) | 2.96 | 3.83".
- The transcription is right. The use is not: 31.4 is already the slope of
  the force line, but the engine passes it through `× 49.95 / 35.1`, so the
  shipped MDF slope is 44.68, not 31.44 (**derived**). The measured
  intercept 3.36 is not used; the engine's is 4.74.
- Conditions the value applies to: up-milling, one straight blade, D 80 mm,
  rake 25°, h 0.041–0.091 mm, vc about 12.6 m/s (**derived**).

## D3. Particleboard 35.0 is a total kc, not a slope

- Engine: `SheetGoodKind::Particleboard => 35.0`, "average of slow (32.0)
  and fast (37.6) peripheral up-milling principal cutting force".
- Document (`g7_palubicki2021_particleboard`), verbatim: "The average specific
  principal cutting force for PB peripheral up-milling is equal to 32.0 N/mm2
  for slow and 37.6 N/mm2 for fast milling."
- The numbers are right and the mean is correct arithmetic (34.8, rounded to
  35.0). But the quantity is `kc = Fc/(b·h)`, the whole force over the chip
  area, averaged over h up to about 0.31 mm. The engine feeds it into the
  affine model as if it were a slope, and then adds an edge term on top. The
  edge effect is counted twice.
- Conditions: vc 40 and 60 m/s (router tools run about 3–20 m/s), one knife,
  R 82.5 mm, rake 13°, fz 1.5 mm.
- Goli 2023 figure read (derived, grade c) for particleboard at h 40–100 µm:
  Ks about 12–23 N/mm² up-milling, Int about 2.6–3.3 N/mm.

## D4. HDPE 40.0 is a yield stress, not a cutting force

- Engine: `PlasticFamily::Hdpe => Some(40.0)`, "Yang 2022 measured
  cutting-yield-stress midpoint of 33.85–46.89 N/mm²".
- Document (`g7_yang2022_hdpe`), verbatim: "The yielding stress derived from
  the analyses have the values of 46.89 and 33.85 MPa for the two cutting
  tools".
- The numbers are right. The quantity is a shear yield stress from the
  Atkins cutting analysis of orthogonal cuts at about 10 mm/s (the text also
  prints 10 mm/min). In that model `kc = τ·γ/Q + R/(Q·h)`. The paper does
  not print γ or Q, so no kc can be computed from it. A yield stress is not
  a specific cutting force. (The engine already refuses a PMMA value,
  276.5 N/mm², that `kc_extra.md` marked "do NOT promote"; the HDPE value
  has a weaker basis than that one, not a stronger one.)
- Status: the HDPE Kc has no document behind it as a cutting force. By the
  R1 rule it should refuse, or carry a stated proxy rule.

## D5. Solid wood: `MILLING_KC_FACTOR 2.7 × FPL shear strength` has no printed source

- Engine: `MILLING_KC_FACTOR = 2.7`; the comment cites
  `planning/KC_MILLING_CALIBRATION_2026-06-17.md` (deleted 2026-09-17; read
  with `git show planning-pre-purge-2026-09-17:<path>`).
- That document derives it as "MILLING_KC_FACTOR ≈ 2.7 (cited: 32–38 / 13)":
  the Pałubicki **particleboard** total kc (32–38, D3) divided by the FPL
  **hardwood** shear strength (13). It calls the paper "Sydor et al."; the
  stored paper has one author, Pałubicki. So 2.7 is repo-fitted as a ratio
  across two materials and two quantities. No document in this fetch prints
  a factor from shear strength to milling Kc.
- The closest printed solid-wood law is Curti 2021 Table 5
  (`x-g7-curti2021-ksnorm-*`): `Ks = Ks_norm(GA)·ρ`, for example, straight
  blade up-milling `Ks_norm = −5e-6·GA² + 1e-3·GA + 0.026`. Evaluated
  (**derived**) at ρ 700 kg/m³: Ks 18.2 (along the grain) to 52.9 (across);
  Int about 3.9 N/mm. At ρ 450: Ks 11.7 to 34.0. The engine's hardwood line
  (49.95 / 5.30) sits at the across-grain end; its softwood line
  (24.98 / 2.65) sits in the middle of the ρ 450 range.
- Đurković 2017 oak (`x-g7-durkovic2017-oak-force-per-blade`): measured
  kc 23.7–60.6 N/mm² over em 0.13–0.02 mm (**derived**, b = 30 mm assumed,
  motor-power based). The Krsljak textbook model in the same paper gives
  58–141 N/mm² for the same cuts (grade c).
- `janka/100` for `SolidWoodByJanka`: no document here prints a Janka-to-Kc
  law. Curti 2021 prints a density law; density, not Janka, is the printed
  input.

## D6. Plywood 8 / 13 / 11 are raw shear strengths, without the milling factor

- Engine: `Plywood::Softwood 8.0`, `BalticBirch 13.0`, `HardwoodFaced 11.0`,
  marked `TODO Phase 3`, "track shear-parallel shear strength of the
  dominant veneer rather than peripheral milling specific cutting force".
- These values do not get `MILLING_KC_FACTOR`, while solid wood does. So
  Baltic birch plywood (13.0) reads 2.7 times lower than solid birch
  (2.7 × 13.0 = 35.1) from the same FPL row. That is an internal
  inconsistency, not a physical claim.
- The only plywood force data found (Goli 2023, figure read, **derived**,
  grade c): poplar plywood, 430 kg/m³, straight blade, up-milling, Ks about
  23–43 N/mm², Int about 3.0–4.9 N/mm; down-milling Ks 24–43; 30° helix Ks
  20–29.5 with Int near zero. The engine's Baltic birch line
  (18.50 / 1.96) is below the measured poplar plywood, though Baltic birch
  is much denser (about 650–700 kg/m³, not printed in any stored source).

## D7. `GRAIN_ANISOTROPY_FACTOR 2.0` cites a paper that does not measure grain anisotropy

- Engine (`tool_load/power.rs`): "Pałubicki 2021 ... measured the directional
  spread of specific cutting force for particleboard peripheral up-milling
  across grain orientations."
- The deleted calibration document also cites "Pałubicki 2021 ~1.3–1.5" for
  the grain spread.
- Document: the stored Pałubicki text has no grain-orientation measurement.
  Particleboard is isotropic (Goli 2023: "particleboard and MDF has shown a
  perfectly isotropic behaviour").
- The printed sources of a factor near 2 are Goli 2018 ("the normalized
  cutting forces are almost two times larger when machining across the
  grain", beech and poplar LVL) and Curti 2021 (max/min force ratio "varies
  from 1.34 to 3.73"). These apply to solid wood and LVL, not to MDF or
  particleboard. The engine applies 2.0 to every material, including MDF,
  where Goli 2018 shows a flat force against angle.

## Not a discrepancy

- The aluminium Kienzle pair (800 N/mm², mc 0.25) was not re-checked: the
  Machining Doctor pages return HTTP 403 (as in 2026-05-29). Nothing found
  contradicts it.
