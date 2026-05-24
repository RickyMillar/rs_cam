# Suggest / Sim / Optimize executable sweep plan

Date: 2026-05-23

Companion spec: `planning/SUGGEST_SIM_OPTIMIZE_ACCEPTANCE.md`
Seed data:

- `planning/toolpath_acceptance/toolpath_catalog.csv`
- `planning/toolpath_acceptance/seed_goal_matrix.csv`

## Goal

Turn the acceptance dataset into repeatable evidence for the three behaviors we care about:

1. **Suggest baseline** — when a toolpath is created or defaulted, did feed/RPM/DOC/stepover land in a source-backed envelope?
2. **Simulation calibration** — when we perturb the cut above/below the source-backed envelope, do sim and tool-load gates classify it correctly?
3. **Optimizer honesty** — when optimizer recommends a faster candidate, did it preserve load gates, collision gates, drill gates, and surface-quality gates?

The plan is deliberately tiered. The first baseline should use existing sweep infrastructure and produce a coverage/gap report. Later tiers add a purpose-built acceptance harness.

## Artifacts to produce

Every sweep run writes to `target/acceptance_sweeps/<tier>/<timestamp>/` and may optionally create a human summary in `planning/toolpath_acceptance/baselines/<date>.md`.

Required machine-readable outputs:

| File | Producer | Contents |
|---|---|---|
| `manifest.json` | runner | git SHA, dirty status summary, command, tier, fixture set, resolution, machine profile |
| `cases.csv` | runner | expanded case list before execution |
| `results.csv` | runner | one row per executed baseline/variant/optimization candidate |
| `summary.json` | aggregator | counts by stage, op, material, tool, evidence grade, failure reason |
| `failures.csv` | aggregator | only failing rows, with actionable reason |
| `coverage.csv` | aggregator | catalog/goal coverage gaps |

Required human outputs:

| File | Contents |
|---|---|
| `README.md` or baseline report | commands run, runtime, pass/fail headline, top failures, screenshots/artifact links |
| selected PNG/SVG artifacts | only for failures and representative passes; avoid committing generated images unless specifically wanted |

## Result schema

`results.csv` should have these columns. If a value is not applicable, use an empty string plus an explicit `not_applicable_reason`.

### Identifiers

- `run_id`
- `tier`
- `case_id`
- `variant_id`
- `stage`: `suggest`, `sim`, `optimize`, `export_gate`
- `operation_kind`
- `operation_family`
- `fixture`
- `geometry_kind`: `svg`, `dxf`, `stl`, `step`, `synthetic`
- `setup_face`
- `tool_family`
- `tool_diameter_mm`
- `tool_flutes`
- `tool_stickout_mm`
- `material_family`
- `goal_id`
- `goal_type`
- `evidence_grade`

### Parameters

- `feed_rate_mm_min`
- `plunge_rate_mm_min`
- `rpm`
- `depth_per_pass_mm`
- `total_depth_mm`
- `stepover_mm`
- `stepover_pct_diameter`
- `scallop_height_mm`
- `stock_to_leave_mm`
- `safe_z_mm`
- `quality_tier`: `fine_visible`, `good_light_sanding`, `semi_finish`, `roughing_only`, or blank

### Suggest metrics

- `nominal_fz_mm_tooth = feed_rate_mm_min / (rpm * flutes)`
- `goal_nominal_fz_min_mm_tooth`
- `goal_nominal_fz_max_mm_tooth`
- `suggest_verdict`: `pass`, `fail`, `warn`, `uncovered`, `not_applicable`
- `suggest_reason`

### Sim metrics

- `toolpath_generated`: bool
- `move_count`
- `cycle_time_estimate_s`
- `air_cut_pct`
- `average_engagement`
- `peak_axial_doc_mm`
- `rapid_collision_count`
- `holder_collision_count`
- `chipload_verdict`: `Within`, `Exceeds`, `Unmodeled`, blank
- `power_verdict`
- `deflection_verdict`
- `corrected_chip_min_mm_tooth`
- `corrected_chip_p50_mm_tooth`
- `corrected_chip_peak_mm_tooth`
- `drill_chip_welding_verdict`
- `drill_peck_adequacy_verdict`
- `drill_plunge_feed_verdict`
- `sim_verdict`: `pass`, `fail`, `warn`, `uncovered`, `not_applicable`
- `sim_reason`

### Optimize metrics

- `optimizer_outcome`: `Ranked`, `MarginalSafe`, `TradeOff`, `NoSafeImprovement`, `Skipped`, blank
- `optimizer_recommended`: bool
- `recommended_cycle_time_delta_pct`
- `gate_regression_count`
- `quality_regression_count`
- `optimizer_verdict`: `pass`, `fail`, `warn`, `uncovered`, `not_applicable`
- `optimizer_reason`

### Overall

- `overall_verdict`: `pass`, `fail`, `warn`, `uncovered`, `not_applicable`
- `failure_class`: see taxonomy below
- `notes`

## Tiers

### Tier 0 — current baseline / no new harness

Purpose: establish the first baseline using existing infrastructure before building anything new.

Commands:

```bash
mkdir -p target/acceptance_baseline
cargo test --test param_sweep sweep_pocket_stepover -- --nocapture
cargo test --test param_sweep sweep_adaptive_stepover -- --nocapture
cargo test --test param_sweep sweep_dropcutter_stepover -- --nocapture
cargo test --test param_sweep sweep_drill_cycle -- --nocapture
cargo test --test param_sweep sweep_scallop_height -- --nocapture
cargo test --test param_sweep -- --nocapture
python3 toolpath_stress_test/agents/analyze_sweep.py target/param_sweeps/
```

Expected output:

- Existing JSON/SVG/PNG artifacts under `target/param_sweeps/`.
- A baseline report that says which existing sweeps pass, fail, or are insufficient for the new acceptance framework.

This tier does **not** yet prove Suggest/Sim/Optimize acceptance. It gives:

- operation/parameter coverage currently available;
- generation regressions;
- obvious geometry/simulation breakage;
- artifact locations for visual triage;
- a gap list for the new harness.

Expected runtime: minutes to tens of minutes, depending on current adaptive3d/3D fixture cost.

### Tier 1 — smoke acceptance sweep

Purpose: run in PRs once the new harness exists.

Size target: 40–80 generated/simulated cases, under 10 minutes locally.

Required operation coverage:

| Family | Operation cases |
|---|---|
| 2.5D clearing | `face`, `pocket`, `adaptive` |
| 2.5D contour | `profile` |
| Trace/V | `trace`, `v_carve`, `chamfer` |
| Drill | `drill`, `alignment_pin_drill` |
| 3D rough | `adaptive3d` |
| 3D finish | `drop_cutter`, `scallop`, `waterline`, `horizontal_finish` |

Smoke fixture/tool/material matrix:

| Case group | Fixture | Tool | Material | Goal rows |
|---|---|---|---|---|
| 2D rough | `fixtures/demo_pocket.svg` | 6.35 mm 2F flat end | softwood | community nominal + matching vendor if present |
| 2D contour/V | `fixtures/demo_star.svg` | 6.35 mm flat end + 90° V-bit | hardwood | community/effective-diameter rows |
| Drill | `fixtures/demo_pocket.svg` or synthetic hole list | 3.175 mm drill/flat end | softwood + hardwood | drill native gates |
| 3D rough | `fixtures/terrain_small.stl` | 6.35 mm 2F flat end | softwood | adaptive/vendor/community |
| 3D finish | `fixtures/terrain_small.stl` | 6.0 mm or 6.35 mm 2F ball nose | hardwood | vendor LUT + finish-quality rows |
| Flat/STEP | `fixtures/gui_step/plate_100x60x10.step` | 12.7 mm facing bit or 6.35 mm flat end | MDF | face/horizontal rows |

Variant strategy per case:

- baseline suggested/default params;
- low feed: `0.5x` baseline feed to test burn/under-chipload behavior;
- high feed: `1.5x` baseline feed to test overload behavior;
- quality perturbation for finish: one in-band stepover/scallop and one intentionally roughing-only value;
- drill perturbation: one acceptable peck and one excessive single-peck/deep-hole row.

CI gate for Tier 1:

- no generated toolpath panics/errors;
- zero rapid/holder collisions unless fixture is explicitly a collision-negative test;
- no optimizer recommendation with a gate or quality regression;
- uncovered rows are allowed but counted and must not increase without explanation.

### Tier 2 — standard acceptance sweep

Purpose: release/pre-merge confidence across every operation kind.

Size target: 250–600 cases, 1–2 hours on a dev machine; not mandatory for every PR.

Required operation coverage: every row in `toolpath_catalog.csv` gets at least one case.

Standard fixture matrix:

| Operation kind | Fixture(s) | Tool(s) | Material(s) | Primary goal types |
|---|---|---|---|---|
| `face` | `fixtures/gui_step/plate_100x60x10.step`, synthetic stock face | 12.7 facing bit, 6.35 flat | MDF, softwood | vendor/community chipload |
| `pocket` | `fixtures/demo_pocket.svg`, L-shape synthetic | 3.175/6.35 flat | softwood, hardwood, MDF | community + vendor |
| `profile` | `fixtures/demo_star.svg`, rectangle synthetic | 3.175/6.35 flat | hardwood, plywood_hardwood | community + vendor |
| `adaptive` | `fixtures/demo_pocket.svg`, L-shape synthetic | 6.35 flat, 6.0 bull | softwood, hardwood | vendor/community + chip thinning |
| `v_carve` | `fixtures/demo_star.svg` | 60°/90° V-bit | softwood, hardwood | effective diameter rows |
| `rest` | two-pass synthetic pocket | 3.175 flat after 6.35 previous tool | hardwood | rest/remaining-stock gate |
| `inlay` | `fixtures/demo_star.svg` | 60° V-bit | hardwood | effective diameter + fit quality |
| `zigzag` | `fixtures/demo_pocket.svg` | 6.35 flat | MDF, softwood | community chipload + air-cut |
| `trace` | `fixtures/demo_star.svg`, `rivers_aligned.dxf` | 3.175 flat, V-bit | hardwood | trace/effective diameter |
| `drill` | synthetic holes + stock pins | 3.175 and 6.35 drill/flat | softwood, hardwood, plastic | drill native gates |
| `chamfer` | `fixtures/demo_star.svg` | chamfer/V-bit | hardwood | effective diameter |
| `drop_cutter` | `fixtures/terrain_small.stl`, hemisphere synthetic | 6.0 ball, 6.35 flat | hardwood | vendor + finish quality |
| `adaptive3d` | `fixtures/terrain_small.stl` | 6.35 flat/bull | softwood, hardwood | adaptive/vendor/community |
| `waterline` | `fixtures/terrain_small.stl`, stepped block STEP | 6.0 ball, 6.35 flat | hardwood, MDF | contour + finish quality |
| `pencil` | terrain/hemisphere with creases | 3.175/6.0 ball | hardwood | trace + finish quality |
| `scallop` | terrain/hemisphere | 3.175/6.0 ball | hardwood | scallop-height quality |
| `steep_shallow` | terrain/hemisphere | 6.0 ball | hardwood | finish quality + contour/parallel split |
| `ramp_finish` | terrain/hemisphere | 6.0 ball | hardwood | load + smooth Z descent |
| `spiral_finish` | terrain/hemisphere | 6.0 ball | hardwood | finish quality |
| `radial_finish` | terrain/hemisphere | 6.0 ball | hardwood | finish quality + center crowding |
| `horizontal_finish` | `fixtures/gui_step/stepped_block.step`, terrain | 6.35 flat, 6.0 ball | MDF | flatness applicability |
| `project_curve` | `fixtures/demo_star.svg` projected to terrain | 3.175 ball/V-bit | hardwood | trace/effective diameter/collision |
| `alignment_pin_drill` | stock pins | 3.175/6.35 drill | softwood | drill native gates |

Standard perturbations:

| Purpose | Perturbations |
|---|---|
| Suggest landing | baseline suggested/default values only |
| Chipload calibration | feed multiplier `0.5x`, `0.8x`, `1.0x`, `1.2x`, `1.5x`; optionally RPM inverse variants where machine caps allow |
| Engagement/load | stepover/radial WOC `0.05D`, `0.1D`, `0.25D`, `0.5D`, `1.0D` depending on operation |
| DOC/load | depth per pass `0.25D`, `0.5D`, `1.0D`, `1.5D` for roughing; finish ops use small DOC/stock-to-leave variants |
| Finish quality | stepover/scallop bands: 5–10%, 10–20%, 20–35%, >35% |
| Drill | total D/d below warn, between warn/fail, above fail; single-peck above threshold |
| Optimize | run optimizer on nominal, under-chipload, over-load, and coarse-finish cases |

Tier 2 pass criteria:

- Every operation kind has at least one covered case.
- All grade-A vendor rows that map to an operation family pass suggest and sim calibration, or have a documented known-gap issue.
- Optimizer has no `Ranked` recommendation with a gate regression.
- Finish optimization has no coarse-quality regression hidden as a safe improvement.
- Drill optimizer returns `Skipped`/not applicable with drill-native explanation.

### Tier 3 — full calibration sweep

Purpose: tune the implementation against the full benchmark set.

Size target: 1,500–5,000 cases; overnight/manual.

Expansion rules:

- Start from every row in `seed_goal_matrix.csv`.
- Expand `operation_kind=*` vendor rows across all operation kinds that share the row's `operation_family` and pass role.
- Material expansion:
  - exact material rows first;
  - grouped fallback rows only when exact rows are absent;
  - never treat an extrapolated row as grade-A calibration evidence.
- Tool expansion:
  - exact diameter/flute rows first;
  - neighboring diameter interpolation rows marked `interpolated`;
  - V-bit/effective-diameter rows marked advisory unless exact geometry is measured.
- Fixture expansion:
  - each operation gets at least one easy, one boundary, and one stress fixture.

Tier 3 expected output:

- calibration confusion matrix per gate: `Within`, `Exceeds`, `Unmodeled`;
- optimize improvement distribution by operation family;
- top 20 false-safe cases;
- top 20 false-fail cases;
- uncovered catalog and goal rows;
- recommendations for LUT additions or code changes.

## Harness design

### Short-term: baseline via existing `param_sweep`

Use `crates/rs_cam_core/tests/param_sweep.rs` and `crates/rs_cam_cli/src/sweep.rs` as-is for Tier 0. This is artifact generation and regression detection, not full acceptance.

### New acceptance runner

Add a data-driven acceptance runner after Tier 0. Preferred location:

- `crates/rs_cam_core/tests/acceptance_sweep.rs` for core-only generation/sim/tool-load checks.
- Optional CLI wrapper later: `cargo run -p rs_cam_cli -- acceptance-sweep ...`.

Input files:

- `planning/toolpath_acceptance/toolpath_catalog.csv`
- `planning/toolpath_acceptance/seed_goal_matrix.csv`
- `planning/toolpath_acceptance/cases_smoke.csv`
- `planning/toolpath_acceptance/cases_standard.csv`
- generated `cases_full.csv` for Tier 3

Runner responsibilities:

1. Load case matrix and goal rows.
2. Build/import fixture geometry.
3. Construct tool/material/machine configs.
4. Generate suggested/default operation params through the same path used by GUI/CLI where possible.
5. Generate toolpath.
6. Run tri-dexel sim at tier-specific resolution.
7. Collect diagnostics and tool-load report.
8. Optionally run optimizer.
9. Compare against row-specific gates.
10. Write result rows and summaries.

Important: avoid a parallel one-off planner. The runner should call existing operation generation, simulation, tool-load, and optimizer APIs so failures reflect product behavior.

### Simulation resolutions

| Tier | Resolution | Reason |
|---|---:|---|
| Tier 0 existing | current test defaults | compare with existing artifacts |
| Tier 1 smoke | 0.75–1.0 mm | quick PR signal |
| Tier 2 standard | 0.5 mm | current diagnostic default |
| Tier 3 full | 0.25–0.5 mm selected by tool diameter | calibration detail; expensive |

## Pass/fail taxonomy

Use these `failure_class` values in `results.csv` and summaries:

| Class | Meaning |
|---|---|
| `suggest_miss_chipload_low` | nominal fz below goal min without cap explanation |
| `suggest_miss_chipload_high` | nominal fz above goal max |
| `suggest_miss_doc` | DOC outside row/rule |
| `suggest_miss_stepover_quality` | finish stepover/scallop outside requested quality tier |
| `sim_false_safe_chipload` | sim says Within for intentionally too-low/too-high chipload |
| `sim_false_fail_chipload` | sim says Exceeds for in-band case without another physical gate |
| `sim_false_safe_power` | power gate misses overload |
| `sim_false_safe_deflection` | deflection gate misses overload |
| `sim_collision` | rapid, holder, or shank collision |
| `sim_air_cut_high` | air-cut above op threshold |
| `drill_gate_miss` | drill-native verdict disagrees with D/d or peck target |
| `optimizer_gate_regression` | optimized candidate worsens load/collision/drill verdict |
| `optimizer_quality_regression` | optimized candidate worsens finish tier/scallop beyond allowed objective |
| `optimizer_should_skip` | optimizer recommended for not-applicable/unmodeled case |
| `export_gate_miss` | unsafe/unmodeled export not blocked |
| `uncovered_goal` | no source row applies |
| `harness_error` | fixture/import/generation/test runner failure |

## First baseline checklist

The first agent should **not** implement the new harness. It should run Tier 0 and write a baseline report.

1. Confirm current worktree and do not reset or clean unrelated changes.
2. Validate that the two CSVs parse and report row counts.
3. Run smoke subset commands from Tier 0.
4. If smoke passes, run full existing `param_sweep`.
5. Run `toolpath_stress_test/agents/analyze_sweep.py target/param_sweeps/` if available.
6. Summarize:
   - test commands and runtime;
   - pass/fail status;
   - number of `sweep_result.json` files;
   - operation/parameter coverage;
   - largest obvious failures from analyzer/logs;
   - what is missing versus this acceptance plan.
7. Write `planning/toolpath_acceptance/baselines/<YYYY-MM-DD>_tier0_baseline.md`.

## Promotion path

1. Complete Tier 0 baseline.
2. Add `cases_smoke.csv` and `cases_standard.csv`.
3. Implement core acceptance runner producing the schema above.
4. Make Tier 1 smoke runnable locally and in CI.
5. Run Tier 2 standard and triage failures into:
   - real product bug;
   - dataset row too strict/ambiguous;
   - harness mismatch;
   - accepted unmodeled gap.
6. Expand to Tier 3 full calibration only after Tier 2 is stable.
