# F201: unsourced feeds formula constants (citation task)

Status: OPEN. Operator ruling 6 (2026-10-22), option B. The values stay.
The sentry `every_feeds_formula_constant_states_its_source_f201` lists them
and pins the count (162 unsourced, 3 cited; 158 before feeds contract P13a, 159 before P11c, 154 before P10, 149 before P6; 150 before the P5 chain ruling deleted the deflection back-off floor, 146 before feeds contract P5, 144 before P3). Option A (the formula path
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

## Unsourced rows (162)

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
| `calculate.slotting_doc_cap_d` | 0.25 | `:1548` | same. Feeds contract P11c (2026-11-05): deleted from the registry; the slot depth cap is `RigidityProfile::slot_doc_factor` (per-machine data; the hobby grade default 0.25 is the Shapeoko port, Unsourced) |

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

### Dressup entry plunge feed (feeds contract P3, K4, lead decision D5)

`PlungeFeedRule::FirstLinearFeedFraction` (`compute/catalog/entry_contract.rs`):
the plunge feed of the dressup entry for the air lead and the plunge
fallback. The rule moved here from `dressup_apply.rs` `entry_plunge_rate`;
the values did not change. Phase 8 replaces the rule.

| Row | Value | Site |
|---|---|---|
| `dressup.entry_plunge_rate.first_linear_feed_fraction` | 0.5 | `ENTRY_PLUNGE_FIRST_FEED_FRACTION` |
| `dressup.entry_plunge_rate.fallback_mm_min` | 500 | `ENTRY_PLUNGE_FALLBACK_MM_MIN` |

### Suggest back-off numbers (feeds contract P5, K2b)

The pass 3 move budget and the pass 6 deflection target and floor are the
bases of the `Runtime` and `Deflection` limits of the safe envelope
(`feeds/envelope.rs`). P5 moved them into the registry (card rows 25 and
27). The values did not change. The unit test
`the_runtime_rows_hold_the_pass_3_numbers` holds each row equal to the
constant that the pass reads.

| Row | Value | Site |
|---|---|---|
| `feeds.suggest.STEPOVER_BACKOFF_TARGET_MOVES` | 500 000 | `suggest/invariants.rs` |
| `feeds.suggest.STEPOVER_BACKOFF_DIAMETER_FRACTION` | 0.5 | `suggest/invariants.rs` |
| `feeds.suggest.DEFLECTION_BACKOFF_TARGET_UM` | 200 | `suggest/invariants.rs` |

### Tool-fact legacy defaults (feeds contract P6, K3, 2026-11-04)

An `Unknown` tool fact plans at the project-loader default that the file
format had before K3. The pin rose from 149 to 154. Claim: "the
project-loader default before K3 (feeds contract P6)".

| Row | Value | Site |
|---|---|---|
| `compute.tool_config.LEGACY_DEFAULT_CUTTING_LENGTH_MM` | 25.0 | `compute/tool_config.rs` |
| `compute.tool_config.LEGACY_DEFAULT_TAPER_HALF_ANGLE_DEG` | 15.0 | `compute/tool_config.rs` |
| `compute.tool_config.LEGACY_DEFAULT_SHANK_DIAMETER_MM` | 6.35 | `compute/tool_config.rs` |
| `compute.tool_config.LEGACY_DEFAULT_STICKOUT_MM` | 45.0 | `compute/tool_config.rs` |
| `compute.tool_config.LEGACY_DEFAULT_FLUTE_COUNT` | 2 | `compute/tool_config.rs` (a `u32`; the row holds it as f64) |

### Axial geometry rules of the add door (feeds contract P10, ruling D8, 2026-11-05)

A new op has no geometry default without a tool. An axial field (a depth
per pass, a Z step on a tool with no ball tip, the Ramp Finish step, a peck)
is `k x D`. No file in the repo prints an axial `k` for a default, so the
lead ruled `k = today's default / 6 mm`: a 6 mm tool keeps the depth it had
before P10. One row per distinct old default. The pin rose from 154 to 159.
Claim: "today's X mm at reference D 6 mm; no record in the repo".

| Row | Value | Fields | Site |
|---|---|---|---|
| `compute.geometry_default.AXIAL_K_0_5_MM` | 0.5 / 6 | Trace `depth_per_pass`, Ramp Finish `max_stepdown` | `compute/geometry_default.rs` |
| `compute.geometry_default.AXIAL_K_1_0_MM` | 1.0 / 6 | Face `depth_per_pass`; Waterline, Steep and Shallow, Unified Finish `z_step` | `compute/geometry_default.rs` |
| `compute.geometry_default.AXIAL_K_1_5_MM` | 1.5 / 6 | Pocket, Zigzag, Inlay plug `depth_per_pass` | `compute/geometry_default.rs` |
| `compute.geometry_default.AXIAL_K_2_0_MM` | 2.0 / 6 | Profile, 2D Adaptive `depth_per_pass` | `compute/geometry_default.rs` |
| `compute.geometry_default.AXIAL_K_3_0_MM` | 3.0 / 6 | 3D Rough `depth_per_pass`; Drill, Pin Drill `peck_depth` | `compute/geometry_default.rs` |

The radial rule (every stepover) reads the family `ae_factor` rows of
`feeds.operation_default_profile`, which were rows before P10.

### Engagement floor, helix and dwell (feeds contract P13a, review H3b, L3, L4, 2026-11-05)

Four values shipped with no row. Each now has one, and no value moved. The
pin rose from 158 to 162.

| Row | Value | Site | Basis site |
|---|---|---|---|
| `feeds.MIN_AP_MM` | 0.05 mm | `feeds/mod.rs` (`MIN_AP_MM_ROW`) | the engagement floor of the calculator (`feeds/calculator/cut.rs`) and the envelope depth floor: `Basis::Rule` |
| `feeds.MIN_AE_MM` | 0.02 mm | `feeds/mod.rs` (`MIN_AE_MM_ROW`) | the engagement floor of the calculator and the envelope width floor: `Basis::Rule` |
| `tool.DEFAULT_HELIX_DEG` | 30 degrees | `tool/mod.rs` (`DEFAULT_HELIX_DEG`) | none: no basis names the helix today. The five cutter constructors and the two `default_helix_deg` loaders read the one constant |
| `compute.operation_configs.DRILL_DWELL_S` | 0.5 s | `compute/operation_configs.rs` | none: the G82 dwell has no feeds basis |

`default_engagement` read the floor as the literals `.max(0.05)` and
`.max(0.02)`; it now reads `MIN_AP_MM` and `MIN_AE_MM`.

### Machine default (outside the F201 edit boundary)

`machine/mod.rs:51` `ChipLoadFormula::default()`: k0 0.024, p 0.61,
q 1.26, "Soft wood baseline from Shapeoko empirical data" (`reference/`).
The registry reads the three values from the default; the marker is not
beside the number, because this task did not edit `machine/`.

## Out of the registry (stated reason)

- Design tolerances and solver limits: `SPINDLE_CEILING_HEADROOM` (0.95),
  `POWER_LADDER_AP_FLOOR_MM` / `_AE_` (0.5), the bisection limits, the
  chip-thinning clamp (1-4). (`MIN_AP_MM` and `MIN_AE_MM` left this list in
  P13a: they ship a value, so they are rows.)
- Per-machine data: `RigidityProfile` values (`adaptive_doc_factor`,
  `adaptive_woc_factor`), set per machine profile.
- Species Janka (`WoodSpecies::janka_lbf`) and aluminium Brinell
  (`AluminumAlloy::brinell_hb`): their docs cite planning data-ingest files.
  A later pass can move them into the registry as `Cited` rows.
- Drill chip-welding and peck bands: on the drill gate and Suggest default
  path, not on `feeds::calculate`.
