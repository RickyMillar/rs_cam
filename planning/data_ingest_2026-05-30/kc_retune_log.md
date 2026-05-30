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

## After-change record (Phase 2B applied 2026-05-30)

### Renames + value changes

- `ANISOTROPY_MULTIPLIER` → `GRAIN_ANISOTROPY_FACTOR` in
  `tool_load/power.rs`, value `2.5` → `2.0`. Citation: Pałubicki 2021,
  *Materials* 14(9):2208, DOI 10.3390/ma14092208. Mirror constants
  in `session/compute.rs` and `feed_modulation.rs` comment updated to
  match.

### Wood/EWP Kc (changed)

| Material | Pre-2B | Post-2B | Source |
|----------|--------|---------|--------|
| `SheetGood::Mdf` | 10.0 | 31.4 | PMC6315737 round-shape Ks average |
| `SheetGood::Hdf` | 12.0 | 36.8 | Derived as MDF × density ratio (HDF/MDF ≈ 1.17); flagged TODO Phase 3 for direct measurement |
| `SheetGood::Particleboard` | 9.0 | 35.0 | Pałubicki 2021 average of slow (32.0) and fast (37.6) milling |

### Wood/EWP Kc (kept with TODO Phase 3)

| Material | Value (unchanged) | Reason kept |
|----------|------|-------------|
| `WoodSpecies::*` (10 species) | 6.0–28.0 | No fetched direct per-species Kc; current values track shear-parallel-to-grain. Phase 3 beat C will fetch the FPL Ch.5 shear-∥ extract that backs a derivation. |
| `Plywood::*` (3 grades) | 8.0, 11.0, 13.0 | Same — no fetched per-grade Kc. |

### Combined `Kc × factor` product comparison (the physically-meaningful number)

| Material | Pre-2B (Kc × 2.5) | Post-2B (Kc × 2.0) | Ratio |
|----------|-------------------|---------------------|-------|
| Particleboard | 9 × 2.5 = 22.5 | 35 × 2.0 = 70.0 | **3.1×** |
| MDF           | 10 × 2.5 = 25.0 | 31.4 × 2.0 = 62.8 | **2.5×** |
| HDF           | 12 × 2.5 = 30.0 | 36.8 × 2.0 = 73.6 | **2.5×** |
| Plywood (Softwood) | 8 × 2.5 = 20.0 | 8 × 2.0 = 16.0 | **0.80×** |
| Plywood (BalticBirch) | 13 × 2.5 = 32.5 | 13 × 2.0 = 26.0 | **0.80×** |
| HardMaple (solid) | 15 × 2.5 = 37.5 | 15 × 2.0 = 30.0 | **0.80×** |
| Ipe (solid) | 28 × 2.5 = 70.0 | 28 × 2.0 = 56.0 | **0.80×** |

Sheet goods are now predicted to need substantially more spindle
power (2.5–3.1× the pre-Phase-2B value) — closer to the measured
peripheral-milling cutting forces and a major safety win.

Solid wood / plywood Kc unchanged → product drops 20 % (the
`2.5 → 2.0` factor) because their Kc constants weren't moved. This
is a small step toward physical correctness; Phase 3 beat C will
inform a per-species Kc update that brings the product back up.

### Test fixtures updated

- `test_sheet_good_kc_progression` → renamed to
  `test_sheet_good_kc_in_measured_literature_band`. The
  pre-Phase-2B `mdf > particleboard` ordering doesn't survive the
  measured-literature values (Pałubicki's particleboard runs above
  PMC6315737's MDF average). New assertions: all sheet-good Kc sit
  in 30–40 N/mm² band; HDF (densest) tops the bunch.
- `light_cut_is_within_with_available_kw` comment updated to reflect
  `Kc_eff = 30.0` (was 37.5). Assertion `peak_kw < 0.01` still
  holds — actual ≈ 0.00159 (was 0.00198). Within.
- `vbit_triangular_cross_section_halves_power_vs_flat` hand-compute
  comment updated to `Kc_eff = 30.0`, expected literal
  `30.0 * 0.5 * 1.0 * feed / 60_000_000.0` (was 37.5 * ...).
- `heavy_cut_exceeds_machine_with_available_kw` — no comment change
  needed; assertion is just `peak > available`. Verified by hand:
  Ipe Kc=28, Kc_eff=56, P = 56·20·6.35·6000/60e6 = 0.711 kW vs
  0.568 kW available → still Exceeds (by less, but still by physics
  margin).
- `wanaka_*` tests — deflection uses raw Kc with no anisotropy
  factor, so no change.

### Smoke-test coverage NOT executed this round

The MCP smoke suite (AS001–AS015) requires the live GUI + MCP server
and was NOT run in this autonomous session. **Operator action item:**
boot the GUI (`cargo run -p rs_cam_viz --bin rs_cam_gui -- --mcp`),
re-run the smoke suite, and record per-case before/after peak µm.
Verdict flips that match real-world operator experience are kept;
flips that don't match should be filed as deflection-model investigations
per the plan's abort-criteria — they are NOT to be reverted by
silently widening `WITHIN_BOUND_MM` / `EXCEEDS_BOUND_MM` or backing
off the Phase 2B values.

### Deflection bounds — kept as physical safety thresholds

`WITHIN_BOUND_MM = 0.050` (50 µm) and `EXCEEDS_BOUND_MM = 0.200`
(200 µm) were NOT changed. Per the plan: below 50 µm surface finish
is negligibly degraded; 50–200 µm visibly degraded but tool/work
safe; above 200 µm dimensional accuracy compromised AND risk of
chatter / tool breakage. These are reasonable physical thresholds
unchanged by the Phase 2B Kc move. If MCP smoke shows any cut
flipping past 200 µm AND the cut is operator-known-good in
practice, file a deflection-model finding (force arm, near-tip
integration) — do NOT widen the bound.

## Phase A baseline (post-`5a67c1d`) — 2026-05-31

Per `planning/feeds_data_ingest_completion_2026-05-31.md` Phase A,
this section captures the LUT runtime behavior **after** the Phase 4
bulk row promotion (111 → 228 embedded rows) and **before** any
Phase B–E additions land. Used as the empirical baseline that Phase F
will diff against.

The two-bullet decomposition mirrors the plan's split:

1. **MCP smoke AS001–AS015** — operator-only (requires live GUI +
   `--mcp`). Fill in this table after running the smoke suite.
2. **`param_sweep` battery** — autonomous; results captured in
   `target/sweep_baseline_5a67c1d.log` and snapshotted to
   `planning/data_ingest_2026-05-30/sweep_snapshot_5a67c1d.tar.gz`.
   Verdict summary recorded below the smoke table.

### AS001–AS015 smoke (operator)

Boot the GUI with `cargo run -p rs_cam_viz --bin rs_cam_gui -- --mcp`,
then for each case below: `load_project` → `generate_all` →
`run_simulation`. Record per-case peak deflection µm, chipload
verdict, power verdict, rapid-collision count. Compare per-case peak
µm against the Phase 2B baseline (round-10 STATE.md numbers reproduced
in the table for convenience).

| Case | Phase 2B peak µm | Phase A peak µm | Δ µm | Chipload verdict | Power verdict | Rapid collisions | Notes |
|------|------------------|-----------------|------|------------------|---------------|------------------|-------|
| AS001 | 0.076 | _TODO_ | | | | | |
| AS002 | 0.053 | _TODO_ | | | | | |
| AS003 | 0.076 | _TODO_ | | | | | |
| AS004 | 0.005 | _TODO_ | | | | | |
| AS005 | 0.051 | _TODO_ | | | | | |
| AS006 | _n/a_ | _TODO_ | | | | | |
| AS007 | _n/a_ | _TODO_ | | | | | |
| AS008 | _n/a_ | _TODO_ | | | | | |
| AS009 | _n/a_ | _TODO_ | | | | | |
| AS010 | _n/a_ | _TODO_ | | | | | |
| AS011 | _n/a_ | _TODO_ | | | | | |
| AS012 | _n/a_ | _TODO_ | | | | | |
| AS013 | 0.105 | _TODO_ | | | | | |
| AS014 | _n/a_ | _TODO_ | | | | | |
| AS015 | 0.197 | _TODO_ | | | | | |

**Notes column:** record any LUT match displacement (e.g. "now matches
Onsrud OCR softwood pocket row instead of Amana"), any verdict flip
(Within ↔ Approximate ↔ Exceeds), or anything that warrants follow-up
before Phase F.

**Acceptance bar:** no case should regress past 200 µm. A µm increase
within the Within band is acceptable (the new LUT row often gives a
better-calibrated chipload). A flip to Exceeds is a finding — file
under the deflection-model investigation policy, do NOT widen
`EXCEEDS_BOUND_MM`.

### `param_sweep` battery (autonomous)

Captured 2026-05-31 via
`cargo test --test param_sweep -- --ignored 2>&1 > target/sweep_baseline_5a67c1d.log`.

| Metric | Value |
|--------|-------|
| cargo test result | `ok. 54 passed; 0 failed; 0 ignored` in 41.36s |
| Total variants analyzed | 105 |
| PASS | 96 |
| FAIL | 0 |
| NO_EFFECT | 9 |
| UNEXPECTED | 0 |
| Snapshot path | `planning/data_ingest_2026-05-30/sweep_snapshot_5a67c1d.tar.gz` (22 MB) |
| Verdicts JSON | `planning/data_ingest_2026-05-30/sweep_verdicts_5a67c1d.json` |
| Analyzer | `python3 toolpath_stress_test/agents/analyze_sweep.py target/param_sweeps/` |

**NO_EFFECT decomposition (all known-expected, no new findings):**

| Operation | Param | Value | Rule |
|-----------|-------|-------|------|
| face | direction | one_way | direction_change_may_not_show_in_aggregates |
| inlay | glue_gap | 0.0 | affects_male_plug_primarily |
| inlay | glue_gap | 0.5 | affects_male_plug_primarily |
| pencil | bitangency_angle | 120.0 | changes_crease_detection |
| pencil | num_offset_passes | 0.0 | more_passes_means_more_moves_if_creases_exist |
| pencil | num_offset_passes | 3.0 | more_passes_means_more_moves_if_creases_exist |
| pocket | climb | false | direction_reversal_may_not_show_in_aggregates |
| profile | climb | false | direction_reversal_may_not_show_in_aggregates |
| scallop | direction | inside_out | direction_change_may_not_show_in_aggregates |

These are the same 9 sweeps that surface as NO_EFFECT on every clean
baseline; the rule column matches `EXPECTED_EFFECTS` in
`toolpath_stress_test/agents/analyze_sweep.py`. No silent truncation.

Phase F will re-run with identical parameters and diff verdict counts.
**Acceptance bar:** zero FAIL/UNEXPECTED regressions vs this baseline.
NO_EFFECTs flipping to PASS (parameter that previously showed no
effect now responds because a closer Phase B–E LUT row is winning)
are OK and recorded as wins.
