# F201: unsourced feeds formula constants (citation task)

Status: OPEN. Operator ruling 6 (2026-10-22), option B. The values stay.
The sentry `every_feeds_formula_constant_states_its_source_f201` lists them
and pins the count (144 unsourced, 3 cited). Option A (the formula path
refuses an unsourced value) applies only if one of these values causes an
issue.

## Scope

The formula path is `feeds::calculate` and the `Material` methods that it
reads. The registry is `feeds::formula_constants()`
(`crates/rs_cam_core/src/feeds/mod.rs`). It holds
`material::MATERIAL_FORMULA_CONSTANTS`
(`crates/rs_cam_core/src/material/mod.rs`). Each row is a `FormulaConstant`
with a `ConstantSource`: `Cited { path, printed }` or `Unsourced { claim }`.

## How to close a row

1. Find a printed source: a vendor chart, a handbook table, or a
   measurement record.
2. Put the source text in the repo (for example under
   `crates/rs_cam_core/data/vendor_lut/sources/`, with a
   `source_manifest.json` entry).
3. Change the row to `FormulaConstant::cited(name, value, path, printed)`.
   `printed` is text that the file contains verbatim.
4. Lower `PINNED_UNSOURCED_COUNT` in the sentry by one.

If the source prints a different number, the value change is a feeds change:
re-bless the FM1 golden in the same commit (`feeds/CLAUDE.md`).

## Cited rows (3)

| Row | Value | File | Derivation |
|---|---|---|---|
| `material.DRILL_PLUNGE_CEILING_WOOD_PER_MM` | 580 | `crates/rs_cam_core/data/vendor_lut/sources/amana_spektra_spiral_plunge_v24.txt` | Wood/Plywood ramp down 72.5 in/min at 1/8 in: 72.5 × 25.4 / 3.175 |
| `material.DRILL_PLUNGE_CEILING_PLYWOOD_PER_MM` | 580 | same | same |
| `material.DRILL_PLUNGE_CEILING_SHEET_PER_MM` | 720 | same | MDF/Laminate ramp down 90 in/min: 90 × 25.4 / 3.175 |

## Unsourced rows (144)

`material/mod.rs` lines are the `const` definition. The function reads the
value from that constant.

### Cutting speed, `Material::base_cutting_speed_m_min` (m/min)

Source kind that settles it: a vendor chart that prints a surface speed per
material class, or a handbook table.

| Row | Value | Site | Note |
|---|---|---|---|
| `solid_wood` | 200 | `material/mod.rs:735` | ported from `reference/` (gitignored) |
| `plywood` | 180 | `material/mod.rs:740` | same |
| `sheet_good` | 170 | `material/mod.rs:745` | same |
| `plastic` | 250 | `material/mod.rs:750` | same |
| `aluminum` | 100 | `material/mod.rs:755` | "conservative SFM placeholder"; `garr_aluminum.json` prints Vc 125-180 m/min |
| `foam` | 300 | `material/mod.rs:760` | same as solid wood |
| `fiberglass` | 120 | `material/mod.rs:765` | comment says the midpoint of Garr SMM 79-157 (`garr_fiberglass.json`); that midpoint is 118 |
| `custom` | 200 | `material/mod.rs:770` | the solid wood value reused |

### Plunge baseline, `Material::plunge_rate_base` (mm/min at 6 mm)

Source kind: a vendor plunge or ramp-down column, or a measurement.

| Row | Value | Site |
|---|---|---|
| `solid_wood` (÷ feed scale) | 1000 | `material/mod.rs:780` |
| `plywood_and_sheet_good` (÷ feed scale) | 900 | `material/mod.rs:782` |
| `plastic` | 1500 | `material/mod.rs:787` |
| `aluminum` (÷ feed scale) | 250 | `material/mod.rs:789` |
| `foam` | 2000 | `material/mod.rs:794` |
| `fiberglass` (÷ feed scale) | 200 | `material/mod.rs:796` |
| `custom` (÷ feed scale) | 800 | `material/mod.rs:801` |
| `reference_diameter_mm` | 6 | `material/mod.rs:803` |
| `diameter_scale_min` / `_max` | 0.25 / 3.0 | `material/mod.rs:808`, `:813` |

### Feed scale, `Material::feed_scale_factor`

Source kind: a vendor chart with a hardness correction, or a measurement.

| Row | Value | Site | Note |
|---|---|---|---|
| `janka_reference_lbf` | 600 | `material/mod.rs:820` | law `(janka / 600)^0.4` from `reference/` |
| `exponent` | 0.4 | `material/mod.rs:825` | same law; also on aluminium |
| `plastic` | 0.5 | `material/mod.rs:830` | hardcoded |
| `aluminum_brinell_reference_hb` | 60 | `material/mod.rs:835` | "feed-rate placeholder" |
| `foam_low` / `_medium` / `_high` | 0.15 / 0.25 / 0.40 | `material/mod.rs:840-850` | per-density hardcode |
| `fiberglass` | 1.3 | `material/mod.rs:855` | "placeholder pending bench validation" |

### Board hardness proxies (Janka lbf, read through the feed scale)

Source kind: a measured hardness of the board, or a ruling that removes the
proxy.

| Row | Value | Site |
|---|---|---|
| `plywood_effective_janka_lbf.softwood` | 600 | `material/mod.rs:864` |
| `plywood_effective_janka_lbf.baltic_birch` | 1200 | `material/mod.rs:869` |
| `plywood_effective_janka_lbf.hardwood_faced` | 1000 | `material/mod.rs:874` |
| `sheet_good_effective_janka_lbf.mdf` | 1100 | `material/mod.rs:879` |
| `sheet_good_effective_janka_lbf.hdf` | 1300 | `material/mod.rs:884` |
| `sheet_good_effective_janka_lbf.particleboard` | 750 | `material/mod.rs:889` (the doc names ANSI A208.1 for 500, not 750) |

### Drill feed envelope, `Material::drill_plunge_feed_envelope_per_mm` (mm/min per mm)

Source kind: a vendor drilling chart per material.

| Row | Value | Site |
|---|---|---|
| `DRILL_PLUNGE_FLOOR_WOOD_PER_MM` | 50 | `material/mod.rs:658` (W6 R-10) |
| `DRILL_PLUNGE_FLOOR_BOARD_PER_MM` | 40 | `material/mod.rs:662` (W6 R-10) |
| `plastic_floor` / `_ceiling` | 60 / 500 | `material/mod.rs:899`, `:904` |
| `aluminum_floor` / `_ceiling` | 40 / 250 | `material/mod.rs:909`, `:914` |
| `foam_floor` / `_ceiling` | 100 / 1000 | `material/mod.rs:919`, `:924` |
| `fiberglass_floor` / `_ceiling` | 40 / 200 | `material/mod.rs:929`, `:934` |
| `custom_floor` / `_ceiling` | 40 / 500 | `material/mod.rs:939`, `:944` |

### `feeds::calculate` steps (`feeds/mod.rs`)

| Row | Value | Site | Source kind |
|---|---|---|---|
| `calculate.fallback_rpm` | 18 000 | `:1306` | guard for a zero diameter; a ruling may suffice |
| `drill_rpm_envelope_for_diameter` tiers: D ≤ 6 → 8000-14 000, D ≤ 10 → 6000-10 000, else 4000-8000 (8 rows) | | `:1314-1349` | vendor drilling chart (W6 audit found none) |
| `milling_rpm_ceiling_for_diameter` tiers: ≤ 3 → 22 000, ≤ 6 → 20 000, ≤ 8 → 18 000, ≤ 10 → 16 000, else 14 000 (9 rows) | | `:1395-1446` | the doc names Onsrud 70/85, Amana Spektra, GWizard; put the chart text in the repo |
| `MAX_SPINDLE_SPEEDUP` | 1.5 | `:367` | vendor rpm_max / rpm_nominal spread |
| `RUBBING_FLOOR_MM_TOOTH` | 0.025 | `:1174` | minimum chip thickness in a vendor or handbook source |
| `LD_SEVERE_THRESHOLD` / `LD_MODERATE_THRESHOLD` | 6 / 4 | `:1610`, `:1612` | tool vendor stickout guidance |
| `LD_SEVERE_SHARE` / `LD_MODERATE_SHARE` | 0.75 / 0.88 | `:1614`, `:1616` | same |
| `calculate.flute_guard_factor` | 0.8 | `:1530` | vendor depth-of-cut rule |
| `calculate.flute_guard_no_length_d` | 2.0 | `:1536` | same |
| `calculate.slotting_threshold_d` | 0.85 | `:1542` | vendor slotting rule |
| `calculate.slotting_doc_cap_d` | 0.25 | `:1548` | same |

### DOC/WOC factors (`feeds/mod.rs`), × tool diameter

Source kind: vendor DOC/WOC rules per operation; the claimed origin is
`reference/shapeoko_feeds_and_speeds/src/calcs.rs` (gitignored).

- `operation_default_profile` (`:2969`, group source `:2959`): 42 rows,
  7 families (Adaptive, Pocket, Contour, Parallel, Scallop, Trace, Face) ×
  3 roles × (ap, ae). The sentry prints each value. The Drill row (0 / 0) is
  out: it exists for type completeness.
- `default_engagement` adaptive matrix (`:3215-3305`, 19 rows): flat/bull
  (1.20, 0.14), ball (0.80, 0.10), tapered ball (0.70, 0.08); semi-finish
  scale (0.75, 0.85); ≥ 3 flutes ae × 0.85; feed scale > 1.40 → (0.80,
  0.90), > 1.15 → (0.90, 0.95), < 0.85 → (1.05, 1.05); rigidity divisor 1.5.
- `default_engagement` finish overrides (`:3310-3345`, 8 rows): ball finish
  (0.06, 0.025), ball semi (0.10, 0.04), tapered finish (0.10, 0.03),
  tapered semi (0.14, 0.05).

### Machine default (outside the F201 edit boundary)

`machine/mod.rs:51` `ChipLoadFormula::default()`: k0 0.024, p 0.61,
q 1.26, "Soft wood baseline from Shapeoko empirical data" (`reference/`).
The registry reads the three values from the default; the marker is not
beside the number, because this task did not edit `machine/`.

## Out of the registry (stated reason)

- Design tolerances and solver limits: `SPINDLE_CEILING_HEADROOM` (0.95),
  `POWER_LADDER_AP_FLOOR_MM` / `_AE_` (0.5), `MIN_AP_MM` (0.05),
  `MIN_AE_MM` (0.02), the bisection limits, the chip-thinning clamp
  (1-4).
- `default_engagement` ends with `.max(0.05)` and `.max(0.02)`. These repeat
  `MIN_AP_MM` and `MIN_AE_MM` as literals. Not changed here.
- Per-machine data: `RigidityProfile` values (`adaptive_doc_factor`,
  `adaptive_woc_factor`), set per machine profile.
- Species Janka (`WoodSpecies::janka_lbf`) and aluminium Brinell
  (`AluminumAlloy::brinell_hb`): their docs cite planning data-ingest files.
  A later pass can move them into the registry as `Cited` rows.
- Drill chip-welding and peck bands: on the drill gate and Suggest default
  path, not on `feeds::calculate`.
