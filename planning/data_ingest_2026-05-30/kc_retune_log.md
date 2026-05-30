# Phase 2 Kc / Anisotropy Re-tune Log — 2026-05-30

This log captures the before/after of the Phase 2 calibrated Kc /
grain-anisotropy re-tune for the
`feeds_data_ingest_2026-05-30_phased_plan.md`. Per the plan, Phase 2's
guiding principle is **the physics is the truth; the tests get
updated to match.**

## Pre-change knob inventory

| File | Constant | Pre-Phase-2 value | Purpose |
|------|----------|-------------------|---------|
| `tool_load/power.rs:36` | `ANISOTROPY_MULTIPLIER` | 2.5 | Kc multiplier in power gate |
| `tool_load/deflection.rs:59` | `WITHIN_BOUND_MM` | 0.050 | Under = Within(Validated) |
| `tool_load/deflection.rs:62` | `EXCEEDS_BOUND_MM` | 0.200 | Over = Exceeds |
| `tool_load/mod.rs::ToleranceBands` defaults | breakage / burn / power_breach / deflection_breach | 0 / 0 / 0 / 0 | Per-verdict gate widening |

## Pre-change wood/EWP Kc values (in `Material::kc_n_per_mm2`)

```
SolidWood:
  GenericSoftwood       6.0
  RadiataPine           6.0
  SouthernYellowPine    7.0
  GenericHardwood       14.0
  HardMaple             15.0
  Walnut                12.0
  Birch                 13.0
  WhiteOak              14.0
  Jarrah                19.0
  Ipe                   28.0
Plywood:
  Softwood              8.0
  BalticBirch           13.0
  HardwoodFaced         11.0
SheetGood:
  Mdf                   10.0
  Hdf                   12.0
  Particleboard         9.0
Foam:
  Low / Medium / High   1.0 / 2.0 / 3.0
Custom:
  user-provided         passthrough Some(kc) if positive-finite
```

These values were calibrated empirically against the closed acceptance
loop, not derived from primary literature. The product `Kc · 2.5 ·
anisotropy` is the physically-meaningful number; today's split happens
to land at the right ballpark for some shapes by accident.

## Pre-change pinned-number tests

- `tool_load/power.rs::light_cut_is_within_with_available_kw`:
  asserts `peak_kw < 0.01`. Hand-computed: Kc=15 (HardMaple),
  Kc_eff=2.5·15=37.5, P = 37.5·1·3.175·1000 / 60e6 ≈ 0.00198 kW.
- `tool_load/power.rs::heavy_cut_exceeds_machine_with_available_kw`:
  Ipe slot 20 mm DOC, 6000 mm/min. Kc=28, Kc_eff=70.
  P = 70·20·6.35·6000 / 60e6 ≈ 0.889 kW vs available ≈ 0.568 kW.
  Asserts Exceeds.
- `tool_load/deflection.rs::wanaka_endmill_back_rough_lands_in_approximate_band`:
  HardMaple slot at 2.5 mm DOC, 1500 mm/min, 6 mm flat at 45 mm
  stickout. Asserts peak µm in [100, 200] band.
- `tool_load/deflection.rs::wanaka_tapered_ball_finishing_is_within`:
  HardMaple, 0.5 mm DOC, π/4 arc, 800 mm/min, tapered ball 2 mm tip
  6 mm shank 35 mm stickout. Asserts peak < 50 µm.

## Pre-change acceptance smoke baseline (per `planning/acceptance_loop/STATE.md` round-10)

AS001 0.076 mm | AS002 0.053 mm | AS003 0.076 mm | AS004 0.005 mm |
AS005 0.051 mm | AS013 0.105 mm | AS015 0.197 mm — all Within.

These come from MCP runs (`load_project` + `generate_all` +
`run_simulation` on `test_data/ux_*.toml`). The Phase 2 owner needs
the MCP up via `cargo run -p rs_cam_viz --bin rs_cam_gui -- --mcp`.

This log is updated by Step 2B after the change with the new pinned
values.

## After-change record

(Populated by Step 2B.)
