# B6 plan (G7 material physics): one force line per material

Date: 2026-09-25. Ruling: RULINGS.md §B6 (accepted 2026-09-24). Planner: one
Plan agent; decisions: the orchestrator (session rs-cam-13). Work tree:
`../rs_cam_b6`, branch `b6-force-line` from 9d547298. A separate tree keeps
the FM1 diff free of the uncommitted work of the other sessions.

## 0. Summary

B6 removes the scalar `Kc` and the MDF force anchor. One typed force line per
material replaces them:

```rust
Material::force_line() -> Result<ForceLine, ForceLineRefusal>
```

- Solid wood: the Curti 2021 density law. It is evaluated at helix 0 and takes
  the upper envelope over grain angle and milling mode, per term. The density
  comes from FPL Table 5-3a.
- MDF: the Goli 2018 printed line.
- Every other material refuses with a named reason.

Power reads `line × grain_factor`. Deflection reads the line alone. Every
shipping line has `grain_factor = 1.0`, so the 2.0 anisotropy factor goes.
The power ladder, pass 10 and the gates keep their shape.

Prediction for FM1: power falls to 0.35-0.62x on solid wood and MDF; force
moves 0.71-1.07x; plywood loses force and power on every cell; no cell moves
between ok and refused.

## 1. Decisions

1. **Type.** `Material::force_line()` replaces `Material::kc_n_per_mm2()`.
   `ForceLine` carries `ks_n_per_mm2`, `f_edge_n_per_mm`, `grain_factor`,
   `chip_range_mm: (lo, hi)` and a `basis` (source id, form, density,
   evaluation point). The card and MCP print it. B6 removes
   `affine_coefficients_for_kc`, `affine_coefficients`, the three `LIT_*`
   constants, `MILLING_KC_FACTOR`, `janka_to_kc_n_per_mm2`, `KC_FOLKLORE_*`,
   `KcProvenance` and the plywood, sheet and plastic Kc literals. No shim.
2. **Solid wood.** `Ks = Ks_n · ρ`, `F_edge = Int_n · ρ` (Curti Table 5,
   helix 0). Each term takes its upper envelope over grain angle 0-180° and
   over up- and down-milling. The envelope holds the grain spread, so the
   solid-wood `grain_factor` is 1.0.
3. **Density.** `ρ = SG × 1000 × 1.12` (FPL Table 5-3a, 12 % MC). A generic
   species takes the mean SG of the FPL rows that the old Kc comments name. A
   species with no printed density refuses. A density outside 287-1080 kg/m³
   refuses.
4. **MDF** takes the Goli 2018 line direct, `grain_factor` 1.0. HDF,
   particleboard, every plywood grade, every plastic (HDPE included),
   aluminium, foam, fiberglass and Custom refuse (§3).
5. **Chip range.** A mean chip outside the printed range does not refuse. The
   basis carries one field, `chip_regime`, and the card states "outside the
   measured chip range" as an extrapolation. This applies below 0.04 mm and
   above the top of the range (planner question 2: accepted; a refusal above
   0.10 mm would drop power on most MDF roughing cells).
6. **Consumers.** The power ladder, pass 10 and the gates read the new line.
7. **Plywood dial (planner question 3).** With no force line, pass 6b uses its
   existing section proxy, and the plywood depth and stepover rise on 48 FM1
   cells. Accepted: the ruling refuses a plywood force model, and the card
   already states the proxy.
8. **Aluminium (planner question 4).** Refuse. The only pair is a Kienzle
   power law from an aggregator with no chip range.
9. **Helix.** `ToolConfig::helix_deg` exists (default 30°), but no force or
   power function reads it. B6 does not connect it: the field is a default on
   most tools, and Curti's helix-15 and helix-30 lines are 5-10x lower at h
   0.05 mm. Helix 0 is the largest force Curti prints. Follow-up: a typed
   helix with provenance that selects the printed Curti row.

### Planner findings

- **Envelope cost.** At h 0.05 and 0.10 mm the per-term envelope is 0.28 %
  and 0.31 % above the worst single (mode, grain angle) state.
- **Density basis.** FPL footnote b: "Specific gravity based on weight when
  ovendry and volume at 12% moisture content" (`fpl_ch5_extract.md:32-34`).
  So `SG × 1000 × 1.12` is the density at 12 % MC. Curti weighed control
  cubes at the test MC of 8.7-11.5 % (`marcon2021…txt:151-154`, Table 1). The
  two bases agree within about 3 %. `SG × 1000` alone is 12 % low.
- **Both terms scale with density.** Curti divided both Ks and Int by
  density, so B6 multiplies both by ρ (verifier note, `verified_rows.json`).

## 2. The forms and every constant

### 2.1 Curti 2021 Table 5, helix 0 (printed)

Units: N/mm² per kg/m³ (Ks_n) and N/mm per kg/m³ (Int_n). GA in degrees,
0 = along the grain.

| mode | Ks_n(GA) | Int_n(GA) |
|---|---|---|
| up | −5·10⁻⁶ GA² + 1·10⁻³ GA + 26·10⁻³ | −1·10⁻⁷ GA² + 1·10⁻⁵ GA + 55·10⁻⁴ |
| down | −3·10⁻⁶ GA² + 5·10⁻⁴ GA + 56·10⁻³ | −3·10⁻⁷ GA² + 4·10⁻⁵ GA + 47·10⁻⁴ |

The envelope (derived; maximum over GA in [0, 180], then the larger mode):

| term | up max | down max | envelope |
|---|---|---|---|
| Ks_n | 0.07600 (GA 100) | **0.0768333** (GA 83.3) | **0.0768333** |
| Int_n | 0.00575 (GA 50) | **0.0060333** (GA 66.7) | **0.0060333** |

The code stores the eight printed coefficients and computes the envelope in a
function; a test pins the result. Range as printed: ρ 287-1080 kg/m³, mean
chip 0.04-0.10 mm, rake 25°, one laboratory, Table 6 NRMSE 8.1-37.8 %.

### 2.2 Goli 2018 MDF (printed, Table 3)

Ks 31.44 N/mm² (SD 2.68), Int 3.36 N/mm (SD 0.27); mean chip 0.041-0.091 mm;
up-milling, straight blade, rake 25°, 711 kg/m³. Isotropic in plane, so
`grain_factor` 1.0. Goli's h is the mean chip `fz · (1 − cos ψ) / ψ`; B6
compares the chip range against this mean chip.

### 2.3 Density per engine material (FPL Table 5-3a, 12 % MC)

| engine material | FPL row(s) | SG | ρ = SG·1120 |
|---|---|---|---|
| GenericSoftwood | mean of ponderosa pine 0.40, white spruce 0.36, western redcedar 0.32 | 0.3600 | 403.2 |
| GenericHardwood | mean of American beech 0.64, red maple 0.54, northern red oak 0.63 | 0.6033 | 675.7 |
| LongleafPine | Pine, longleaf | 0.59 | 660.8 |
| HardMaple | Maple, sugar | 0.63 | 705.6 |
| Walnut | Walnut, black | 0.55 | 616.0 |
| Birch | Birch, yellow | 0.62 | 694.4 |
| WhiteOak | Oak, white | 0.68 | 761.6 |
| RadiataPine | FPL Table 5-5a "Pine, radiata ... Green 0.42" (Gb), by FPL Ch.4 Eq. (4-11), MCfs 30 % | G12 0.4501 | 504.1 |
| Jarrah | FPL Table 5-5a "Jarrah ... Green 0.67" (Gb), by Eq. (4-11) | G12 0.7499 | 839.9 |
| Ipe | FPL Table 5-5a "Ipe ... Green 0.92" (Gb), by Eq. (4-11) | G12 1.0776 | 1207.0: above 287-1080, refuse `DensityOutOfRange` |
| SolidWoodByJanka, FPL rows (98) | SG per row (`fpl_ch5_extract.md`) | per row | per row |
| SolidWoodByJanka, Wood Database rows (34) | Janka only | — | refuse |

### 2.4 The resulting lines, against the old engine

`kc_eq(h) = Ks + F_edge / h`. "Old" is the `LIT_*` anchor line; "old x2"
adds the 2.0 grain factor that power used.

| material | Ks | F_edge | kc_eq 0.05 | kc_eq 0.10 | old 0.05 / 0.10 | old x2 0.05 / 0.10 | force new/old | power new/old |
|---|---|---|---|---|---|---|---|---|
| GenericSoftwood | 30.98 | 2.433 | 79.6 | 55.3 | 78.0 / 51.5 | 155.9 / 102.9 | 1.02 / 1.07 | 0.51 / 0.54 |
| GenericHardwood | 51.92 | 4.077 | 133.5 | 92.7 | 155.9 / 102.9 | 311.9 / 205.9 | 0.86 / 0.90 | 0.43 / 0.45 |
| LongleafPine | 50.77 | 3.987 | 130.5 | 90.6 | 124.8 / 82.4 | 249.5 / 164.7 | 1.05 / 1.10 | 0.52 / 0.55 |
| HardMaple | 54.21 | 4.257 | 139.4 | 96.8 | 191.9 / 126.7 | 383.9 / 253.4 | 0.73 / 0.76 | 0.36 / 0.38 |
| Walnut | 47.33 | 3.717 | 121.7 | 84.5 | 114.0 / 75.2 | 227.9 / 150.5 | 1.07 / 1.12 | 0.53 / 0.56 |
| Birch | 53.35 | 4.190 | 137.1 | 95.2 | 155.9 / 102.9 | 311.9 / 205.9 | 0.88 / 0.93 | 0.44 / 0.46 |
| WhiteOak | 58.52 | 4.595 | 150.4 | 104.5 | 165.5 / 109.3 | 331.1 / 218.6 | 0.91 / 0.96 | 0.45 / 0.48 |
| SheetGood::Mdf | 31.44 | 3.36 | 98.6 | 65.0 | 139.5 / 92.1 | 279.0 / 184.2 | 0.71 / 0.71 | 0.35 / 0.35 |

The shape changes for solid wood: Ks/F_edge is 12.73 per mm (9.42 before).
For MDF it is 9.36 per mm.

## 3. What refuses, with the reason text

`ForceLineRefusal` is `Copy + PartialEq`; `card_text() -> (headline, detail)`.

| Material | Today | Headline / detail |
|---|---|---|
| Plywood (all grades) | 8.0 / 13.0 / 11.0 raw shear | "No force line: plywood (ruling B6)" / "The only plywood measurement is a figure read for poplar plywood (Goli 2023). No source measures Baltic birch or softwood plywood, and the density law does not fit plywood (G7 T5)." |
| Particleboard | 35.0 (a total kc used as a slope) | "No force line: particleboard (ruling B6)" / "The router-rig line is a figure read (Goli 2023). Pałubicki 2021 prints a total kc at 40-60 m/s, which is a different quantity." |
| HDF | 36.8 (MDF × a density ratio) | "No force line: HDF (no measurement)" / "No fetched source measures HDF." |
| Plastic, every family | HDPE 40.0 | "No force line: plastics (ruling B6)" / "No source prints a cutting-force line for a plastic. The HDPE figure (Yang 2022) is a yield stress, not a cutting force." |
| Aluminium | Kienzle through the MDF anchor | "No force line: aluminium (no per-edge line)" / "The only pair is a Kienzle kc1.1 800 / mc 0.25 from an aggregator (VDI 3323 group 22, grade A-secondary). It is a power law with no printed chip range, not an affine per-edge line." |
| Foam | 1 / 2 / 3, no source | "No force line: foam (no measurement)" / "No fetched source measures foam. The old 1-3 N/mm² values had no source." |
| Fiberglass | None | "No force line: fiberglass (no measurement)" / "No fetched source measures a glass-resin laminate." |
| Custom | typed `kc` through the anchor | "No force line: custom material (ruling B6)" / "A custom material carries no measured line. B6 takes no typed line." |
| Wood Database rows; FPL rows with no SG | janka/100 × 2.7 | "No force line: no printed density for this species" / "FPL Table 5-3a has no specific gravity for this species, and no fetched source prints its density. The Curti law needs a density." |
| Ipe (the fetch inside B6: ρ 1207.0 kg/m³) | folklore × 2.7 | the "Density outside 287-1080" row below |
| Density outside 287-1080 | — | "No force line: density {ρ:.0} kg/m³ is outside 287-1080" / "Curti 2021 fits five species from 287 to 1080 kg/m³ and states that the model must not extrapolate." |

Custom refuses (no user line): the Sim gates already refuse Custom; a typed
line has no source and no range; without the anchor a typed scalar has no
meaning. `Material::Custom.kc` is removed.

## 4. The type (new file `crates/rs_cam_core/src/material/force_line.rs`)

```rust
pub const CURTI_2021_SOURCE_ID: &str = "g7_curti2021_generalized_wood_model";
pub const GOLI_2018_SOURCE_ID: &str = "g7_goli2018_round_shape_ks";
pub const CURTI_DENSITY_RANGE_KG_M3: (f64, f64) = (287.0, 1080.0);
pub const CURTI_CHIP_RANGE_MM: (f64, f64) = (0.04, 0.10);
pub const GOLI_2018_MDF_CHIP_RANGE_MM: (f64, f64) = (0.041, 0.091);
pub const FPL_12PCT_MOISTURE_FACTOR: f64 = 1.12;
// the eight printed Curti helix-0 coefficients; GOLI_2018_MDF_KS 31.44, _INT 3.36

pub(crate) fn curti_helix0_envelope() -> (f64, f64);

pub struct ForceLine { /* private: ks, f_edge, grain_factor, chip_range_mm, basis */ }
impl ForceLine {
  ks_n_per_mm2, f_edge_n_per_mm, grain_factor, chip_range_mm, basis, kc_eq(h),
  source_id, form_id, chip_regime(mean_chip_mm), card_text(),
  pub fn curti_density_law(rho, WoodDensity) -> Result<Self, ForceLineRefusal>
}
pub enum ForceBasis { CurtiDensityLaw { density: WoodDensity }, Goli2018Mdf }
pub struct WoodDensity { specific_gravity, rho_kg_m3, source: DensitySource }
pub enum DensitySource { FplRow(&'static str), FplMean(&'static [&'static str]), FplLibraryRow }
pub enum ChipRegime { Measured, BelowMeasured { mean_chip_mm }, AboveMeasured { mean_chip_mm } }
pub struct ForceAtPoint { pub line: ForceLine, pub chip_regime: ChipRegime }
pub enum ForceLineRefusal { Plywood, Particleboard, Hdf, Plastic, Aluminum, Foam, Fiberglass,
                            Custom, NoDensity, DensityOutOfRange { rho_kg_m3: f64 } }
```

`form_id`: `"curti2021_density_law_helix0_envelope"` or
`"goli2018_mdf_affine"`. The mean chip is `h_m = fz · (1 − cos ψ) / ψ`.

## 5. Files and functions, by editor

Editor A (engine) and editor B (tests, viz, MCP, FM1, docs) have disjoint
files. The orchestrator compiles after A, then runs B.

### Editor A: core `src/` and data

- `material/force_line.rs` (new): §4 and unit tests (envelope, density
  arithmetic, range edges).
- `material/mod.rs`: `pub mod force_line;`; delete `KcProvenance`; replace
  `WoodSpecies::kc_provenance` with `WoodSpecies::fpl_density()`; delete
  `Custom.kc`, `janka_to_kc_n_per_mm2`, `MILLING_KC_FACTOR`, `KC_FOLKLORE_*`;
  replace `kc_n_per_mm2` with `force_line()`; update the docs and the unit
  tests that pinned Kc values.
- `material/wood_species_library.rs`: `WoodSpeciesEntry` gains
  `#[serde(default)] specific_gravity_12: Option<f64>`.
- `data/wood_species.toml`: `specific_gravity_12` on the FPL rows, from the SG
  column of `fpl_ch5_extract.md`; no key where FPL prints "—".
- `feeds/force.rs`: delete the `LIT_*` constants and the coefficient
  functions; `lateral_cutting_force` reads `material.force_line().ok()?`.
- `feeds/efficiency.rs`, `feeds/predict.rs`, `tool_load/deflection.rs`,
  `tool_load/distribution.rs`: read the line. `DeflectionUnmodeled::
  MaterialCustom` goes.
- `feeds/operating_point.rs`: `PowerFigure` gains `force: ForceAtPoint`; new
  `force_at_operating_point(operation, tool, material, fallback)`. The
  `MaterialUnvalidated` wording becomes "this material has no measured force
  line (ruling B6)".
- `tool_load/power.rs`: delete `GRAIN_ANISOTROPY_FACTOR`;
  `PowerModelInputs.kc_n_per_mm2` becomes `line: ForceLine`; `PowerTerms::of`
  uses `line.grain_factor()`; `sample_power_kw(tool, &ForceLine, …)`; one
  refusal through `force_line()`; the confidence text of §6.
- Wording only: `tool_load/verdict.rs`, `gcode/mod.rs:858`,
  `diagnostics/adapters/from_tool_load.rs:991`,
  `tool_load/optimize/orchestration_skip_tests.rs`.

Files of other sessions, smallest edit (a type change at a call site):

| File | Owner | Region |
|---|---|---|
| `feeds/mod.rs` | rs-cam-2f (G10) | `power_model_terms` l.1371-1398; step 6 l.2146; step 7 `fits_power` l.2459-2463; final power l.2745-2755 (disjoint from step 8) |
| `dressup/feed_modulation.rs` | rs-cam-e2 | `PowerLimitInputs` l.236-268; l.573-579; test l.1586-1590 |
| `session/compute.rs` | rs-cam-e2 | l.1836-1886 |

`provenance.rs`, `rationale.rs`, `suggest/*` and viz `why.rs` get no edit.

### Editor B: tests, viz, MCP, FM1, docs

- New core sentry `tests/the_force_line_is_printed_per_family_b6.rs` (§7).
- Core tests that change: `literature_parity`, `wood_species_library_provenance`,
  `a_rescaled_feed_stays_inside_the_power_ceiling_g_t15`,
  `suggest_power_ceiling_after_pass9_g_suggest_powerstale`,
  `power_ceiling_parity_f2`, `the_power_ladder_pulls_the_right_lever_g_ladder`,
  `a_published_power_is_at_the_depth_that_cuts_g_s2`,
  `cut_efficiency_is_closed_form_g_specenergy`,
  `efficiency_abstains_without_kc_g_specenergy`,
  `a_refused_deflection_is_not_a_zero_g_t4`, `constrained_max_modulation_f039`,
  `modulation_reads_the_depth_in_mm_g_axialunits`,
  `smoke_baseline_regression_f037` (comment). Fixtures on RadiataPine, Jarrah
  or Ipe move to a species with a density; power budgets are re-tuned so each
  test still fires; no non-vacuity assert is removed.
- `feeds_matrix_instrument_fm1.rs`: two new last columns, `force_line` (form
  id or `refused:<Variant>`) and `chip_regime`.
- Viz: `ui/feeds/compare.rs` (the power row paints the force line and the chip
  line), `ui/properties/stock.rs` ("Kc:" becomes "Force line:"),
  `ui/sim_diagnostics.rs`, `ui/preflight.rs` (wording), `app/mcp/generation.rs`
  (`basis.force_line`), `app/mcp/tests.rs`,
  `tests/every_stage_that_moves_a_number_is_on_the_card_g_visible.rs` (the
  weak-spindle case moves from Baltic birch to hardwood), new viz sentry
  `tests/the_power_row_names_its_force_line_b6.rs`.
- Docs: `CREDITS.md` (Curti 2021, Goli 2018; the woodresearch.sk anchor
  retired), `material/CLAUDE.md`, `feeds/CLAUDE.md`, `tool_load/CLAUDE.md`.

## 6. Card lines and MCP fields

Feeds card, under the power row (dim):

- Solid wood: `Force line: Curti 2021 density law, ρ {ρ:.0} kg/m³ (FPL SG {sg:.2}), upper envelope`
- MDF: `Force line: Goli 2018 MDF, printed`
- Refused: the §3 headline.
- Only when the regime is not Measured: `Mean chip {h:.3} mm is below the measured {lo:.3}-{hi:.3} mm: the force line is extended (stated extrapolation)` ("above" in the other case).

Hover, solid wood: "Ks = {Ks_n:.7} × ρ = {Ks:.2} N/mm², F_edge = {Int_n:.7} ×
ρ = {Fe:.3} N/mm per mm of edge. Ks_n and Int_n are the upper envelope of
Curti 2021 Table 5 at helix 0, over grain angle 0-180° and up- and
down-milling, taken per term (derived; Ks_n from down-milling GA 83°, Int_n
from down-milling GA 67°). ρ = SG {sg:.4} × 1000 × 1.12 = {ρ:.1} kg/m³ at 12 %
MC, from FPL Table 5-3a ({row names}). Valid: ρ 287-1080 kg/m³, mean chip
0.04-0.10 mm, rake 25°, one laboratory; Table 6 NRMSE 8.1-37.8 %. Helix: the
engine has no helix input; helix 0 is the largest force Curti prints. Grain
factor 1.0: the envelope holds the grain spread. Source:
g7_curti2021_generalized_wood_model."

Hover, MDF: "Ks 31.44 N/mm², F_edge 3.36 N/mm per mm of edge, printed (Goli
2018 Table 3: up-milling, straight blade, rake 25°, 711 kg/m³). Valid: mean
chip 0.041-0.091 mm. Grain factor 1.0: MDF is isotropic in plane. Source:
g7_goli2018_round_shape_ks."

Stock panel: `Force line:` / `Ks {Ks:.1} N/mm², F_edge {Fe:.2} N/mm` or `—`.

Power gate confidence (`power.rs`): `force line {form_id} ({source_id}); grain
factor {g:.1}; helix 0 (no helix input); a sample outside the measured mean
chip {lo}-{hi} mm extends the line`.

MCP `basis.force_line`:

```json
{ "headline": str, "detail": str, "source_id": str|null, "form": str|null,
  "ks_n_per_mm2": f|null, "f_edge_n_per_mm": f|null, "grain_factor": f|null,
  "chip_range_mm": [lo, hi]|null, "specific_gravity": f|null,
  "density_kg_m3": f|null, "density_source": str|null,
  "evaluated_at": { "mean_chip_mm": f,
    "chip_regime": "measured"|"below_measured_range"|"above_measured_range" }|null,
  "refused": str|null }
```

## 7. Tests

`the_force_line_is_printed_per_family_b6` (core) pins:

1. The envelope from the eight printed coefficients: Ks_n 0.0768333, Int_n
   0.0060333 (±1e-7); at most 0.5 % above the worst single state.
2. The §2.3 and §2.4 values for the 7 species (SG, ρ ±0.1, Ks ±0.01, F_edge
   ±0.001); the generic SGs are the means of the named rows.
3. MDF: (31.44, 3.36), range (0.041, 0.091), grain 1.0.
4. Every `Ok` line has `grain_factor == 1.0`; `PowerTerms` equals the hand
   formula.
5. Every §3 material refuses with its variant (every `PlywoodGrade`,
   `PlasticFamily`, `AluminumAlloy`, `FoamDensity`, `FiberglassGrade`, HDF,
   particleboard, Custom, Ipe (`DensityOutOfRange`, after the §10 fetch), one
   Wood Database row); RadiataPine and Jarrah have a line.
6. `curti_density_law` refuses 286.9 and 1080.1 and accepts 287.0 and 1080.0.
7. Chip regime at 0.03 / 0.05 / 0.12 mm is Below / Measured / Above; at 0.03
   mm `power_at_operating_point` is `Ok`.
8. One line for every consumer: `lateral_cutting_force`,
   `power_at_operating_point`, `cut_efficiency` and the deflection predictor
   read the same Ks and F_edge; for plywood all abstain.
9. A source scan of `crates/rs_cam_core/src` finds none of `LIT_KS`,
   `LIT_FEDGE`, `LIT_ANCHOR`, `MILLING_KC_FACTOR`, `GRAIN_ANISOTROPY_FACTOR`,
   `janka_to_kc`, `kc_n_per_mm2`, `affine_coefficients`.
10. The §6 headlines for GenericHardwood, MDF and plywood.

`the_power_row_names_its_force_line_b6` (viz) pins the force-line line for
hardwood, the refusal line for plywood and the extrapolation line on a finish
cell with a mean chip below 0.04 mm.

Verification order: the B6 sentries; the `feeds/CLAUDE.md` sentries;
`adaptive_feed_modulation_pipeline_f036b`, `arc_fit_disposition_a5`, T15,
`literature_matrix`, core `--lib`, the viz Feeds-card tests; clippy and fmt;
FM1. `wanaka_suggest_integration` only after the operator agrees.

## 8. FM1 prediction (matrix 2026-09-23)

Baseline: 534 ok cells (softwood 160, hardwood 150, mdf 129,
plywood_hardwood 95); `force_n` on 534, `power_kw` on 188.

| Column | softwood | hardwood | mdf | plywood_hardwood |
|---|---|---|---|---|
| `force_n` | ×0.93-1.07 | ×0.78-0.90 | ×0.71 | 95 cells empty |
| `power_kw` | ×0.46-0.62 | ×0.39-0.52 | ×0.35 | 36 cells empty |
| power-limited feeds | 0 → 0 | 0 → 0 | 0 → 0 | 0 → 0 |
| dial cells | scale moves < 1 % | < 1 % | none | 48 cells to the section proxy; depth and stepover up about 8-25 % |
| `AxialDocClampedByEnvelope` | small moves | small moves | none | 4 cells can lose the clamp |
| ok ↔ refused | 0 | 0 | 0 | 0 |

No feed or RPM moves from power (peak 0.10 kW against 0.8 kW). The sim CSV:
the 6 plywood rows go from `Within` to `Unmodeled`; `power_peak_kw` ×0.35-0.6.
Outside FM1: Suggest on Ipe, plywood, particleboard,
HDF, HDPE, aluminium, foam and Custom has no power ceiling. The literature
cell `flat_12mm_adaptive2d_oak_power` can stop firing; check it.

## 9. Breaking changes

1. Removed: `Material::kc_n_per_mm2`, `force::affine_coefficients`,
   `affine_coefficients_for_kc`, `KcProvenance`, `WoodSpecies::kc_provenance`,
   `GRAIN_ANISOTROPY_FACTOR`, `MILLING_KC_FACTOR`, `janka_to_kc_n_per_mm2`,
   `KC_FOLKLORE_*`, `LIT_*`, `DeflectionUnmodeled::MaterialCustom`. New:
   `Material::force_line`, the `material::force_line` module.
2. `PowerModelInputs`, `PowerLimitInputs` and `sample_power_kw` change;
   `PowerFigure` gains `force`.
3. `Material::Custom.kc` is removed. Old projects still load (serde ignores
   the key). Custom refuses force and power everywhere.
4. Force and power refuse for plywood, particleboard, HDF, HDPE, aluminium,
   foam, Ipe (ρ above the Curti range, §10), the Wood Database library rows
   and the FPL rows with no SG.
5. Solid-wood and MDF power falls to 0.35-0.62x; force moves 0.71-1.12x.
6. `WoodSpeciesEntry` gains `specific_gravity_12`.
7. The FM1 CSV gains two columns. The stock panel row "Kc" becomes "Force
   line".

## 10. Open question for the operator

1. **Radiata pine, Jarrah and Ipe have no printed density in the repo, so B6
   refuses their force and power.** Radiata pine is the operator's local
   timber. Recommendation: a small G7 fetch for a printed density at 12 % MC
   (radiata, jarrah, ipe), verified as G7 rows, in a follow-up.

   **Answered 2026-09-25: the operator ruled the fetch inside B6.** FPL Table
   5-5a prints the basic SG `Gb` of all three (source id
   `g7_fpl_gtr190_ch5_table5_5a`); FPL Ch.4 Eq. (4-11) with MCfs 30 %
   converts it to 12 % MC (source id `g7_fpl_gtr190_ch4_sg_conversion`; it
   reproduces FPL's own white-ash example within 0.4 %). Radiata pine 504.1
   and jarrah 839.9 kg/m³ get a Curti line (Ks 38.73 / 64.53 N/mm², F_edge
   3.041 / 5.067 N/mm). Ipe 1207.0 kg/m³ is above the Curti range and refuses
   `DensityOutOfRange`. Rows: `fetch/G7/density_rows.json`; §2.3 and §3 carry
   the result. Test fixtures on radiata pine and jarrah keep a line; the Ipe
   fixtures that need power move to white oak.

Follow-ups outside B6: a typed tool helix that selects the printed Curti
helix row; a user-typed line for Custom; a Kienzle form for aluminium.
