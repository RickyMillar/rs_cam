# Tier 0 acceptance baseline — 2026-05-24

## Scope

Existing `param_sweep` only. No new acceptance harness. This baseline is a
coverage/regression snapshot, not final Suggest/Sim/Optimize proof. It exists
to establish a green-baseline reference point against which the Tier 1+
acceptance harness (see `planning/SUGGEST_SIM_OPTIMIZE_SWEEP_PLAN.md`) will be
built.

## Repository state

- git HEAD: `07a723d` (master)
- working tree: 52 modified/untracked entries (pre-existing user changes, not
  introduced by this run; nothing reset or stashed per agent prompt
  constraints)
- the acceptance-plan docs (`planning/SUGGEST_SIM_OPTIMIZE_ACCEPTANCE.md`,
  `planning/SUGGEST_SIM_OPTIMIZE_SWEEP_PLAN.md`) and seed CSVs
  (`planning/toolpath_acceptance/{toolpath_catalog,seed_goal_matrix}.csv`) are
  in place and parseable

## Dataset sanity

- `toolpath_catalog.csv`: **23 rows × 10 columns** (one per `OperationType`
  variant, including `alignment_pin_drill`)
- `seed_goal_matrix.csv`: **299 rows × 28 columns**
- goal type counts:
  - `community_nominal_chipload`: 140
  - `vendor_lut_chipload`: 67
  - `community_effective_diameter_chipload`: 50
  - `surface_finish_stepover_scallop`: 30
  - `drill_native_gates`: 12

## Commands run

All commands run from repo root. Wall-clock taken from `/usr/bin/time -v`
where available; otherwise from the sweep runner's own `Finished in` line.

| Command | Result | Runtime | Notes |
|---|---|---:|---|
| `cargo test --test param_sweep sweep_pocket_stepover -- --nocapture` | 1 passed | 0.94 s | smoke #1 |
| `cargo test --test param_sweep sweep_adaptive_stepover -- --nocapture` | 1 passed | 0.82 s | smoke #2 |
| `cargo test --test param_sweep sweep_dropcutter_stepover -- --nocapture` | 1 passed | 1.54 s | smoke #3 |
| `cargo test --test param_sweep sweep_drill_cycle -- --nocapture` | 1 passed | 0.67 s | smoke #4 |
| `cargo test --test param_sweep sweep_scallop_height -- --nocapture` | 1 passed | 1.43 s | smoke #5 |
| `cargo test --test param_sweep -- --nocapture` | **54 passed; 0 failed** | 23.46 s wall (peak RSS 175 MiB) | full Tier 0 sweep |
| `python3 toolpath_stress_test/agents/analyze_sweep.py target/param_sweeps/` | 105 variants analysed | < 1 s | 96 PASS / 0 FAIL / 9 NO_EFFECT / 0 UNEXPECTED |

Per-test logs are under `target/tier0_logs/` (kept out of git).

## Artifact summary

- `target/param_sweeps/` exists: **yes**
- `sweep_result.json` count: **54**
- bad JSON files: **0**
- total variants: **105**
- operations covered: **22** of 23 catalog rows
  - `adaptive` 4, `adaptive3d` 4, `chamfer` 1, `drill` 2, `dropcutter` 3,
    `face` 4, `horizontal_finish` 2, `inlay` 2, `pencil` 2, `pocket` 5,
    `profile` 3, `project_curve` 2, `radial_finish` 1, `ramp_finish` 2,
    `rest` 2, `scallop` 2, `spiral_finish` 2, `steep_shallow` 1, `trace` 2,
    `vcarve` 2, `waterline` 4, `zigzag` 2
  - **uncovered catalog row**: `alignment_pin_drill` (not in
    `crates/rs_cam_core/tests/param_sweep.rs`)
- parameters covered: **35** distinct knobs across the 54 sweeps
  - `stepover` is the most-exercised knob (9 variants across families);
    `depth`, `direction`, `feed_rate` each have 4

## Failures / anomalies

- **Test failures**: none. 54/54 ok.
- **Analyzer FAIL/UNEXPECTED**: none.
- **Analyzer NO_EFFECT** (9 — flagged but all are documented expected
  behaviour for that knob; tracked for future investigation, not regressions):
  - `face/direction=one_way` — direction change doesn't show in aggregates
  - `scallop/direction=inside_out` — same class
  - `profile/climb=False` and `pocket/climb=False` — climb flip doesn't shift
    measured metrics in current fingerprint set
  - `inlay/glue_gap=0.0` and `inlay/glue_gap=0.5` — affects male plug, not the
    female pocket fingerprint
  - `pencil/num_offset_passes=0.0` and `=3.0` — requires creases on the
    fixture; current pencil fixture has none
  - `pencil/bitangency_angle=120.0` — crease detection only triggers when
    creases exist

These are fixture/measurement limitations of the current sweep, not product
defects, but Tier 1+ should add fixtures that actually exercise them so the
NO_EFFECT count can be driven to zero.

## Coverage against acceptance plan

The Tier 0 sweep is a generation-regression harness. It exercises "param
moves the geometry as expected" but does **not** consult the new goal matrix,
does **not** classify sim verdicts against any expected value, and does
**not** invoke the optimizer. Each row below is graded against the targets
in `planning/SUGGEST_SIM_OPTIMIZE_ACCEPTANCE.md`.

| Acceptance area | Current Tier 0 coverage | Gap |
|---|---|---|
| Suggest nominal chipload | none — Tier 0 doesn't read `seed_goal_matrix.csv`, doesn't compute `nominal_fz_mm_tooth`, and doesn't compare first-shot params to a goal row | Tier 1 harness must compute `feed / (rpm × flutes)`, look up the matching goal row, and emit a `suggest_verdict` |
| Sim tool-load verdicts | none — no sweep asserts on `tool_load_report().chipload/power/deflection` outcomes | Tier 1 must call `get_tool_load_report` after each sim and compare verdict to the expected verdict implied by each variant's intent (in-band / under / over) |
| Drill native gates | partial — `sweep_drill_cycle` regenerates drill paths but doesn't read `cut_trace.drill_summaries` or `tool_load_report().drill_gates` | Tier 1 must consume drill-native gates and label variants relative to the per-material D/d thresholds |
| 3D finish quality gates | partial — `sweep_scallop_height` and `sweep_dropcutter_stepover` exist, but only as geometric regression; no scallop-vs-stepover quality verdict produced | Tier 1 must compute predicted scallop height from `(stepover, ball_diameter)` and label against the `surface_finish_stepover_scallop` rows |
| Optimizer honesty | none — `optimize_toolpath` is never called by `param_sweep` | Tier 1 must invoke optimizer on selected baselines and assert no gate/quality regression in `Ranked` candidates; assert `Skipped` for drill |
| Export gate | none — `param_sweep` never runs `export_gcode` | Tier 1 (or a later tier) must attempt export on `Exceeds` configurations and assert refusal/warning |

Also worth recording for Tier 1 planning:

- **fixture diversity**: Tier 0 fixtures are baked into `param_sweep.rs` and
  not externally listed. Tier 1 needs `cases_smoke.csv` referencing the
  fixtures in `crates/rs_cam_core/data/` and `fixtures/`. None of the
  Tier 0 sweeps use the STEP fixtures (`gui_step/plate_100x60x10.step`,
  `stepped_block.step`) or the `terrain_small.stl` 3D relief beyond the
  built-in hemisphere/test mesh, so STEP-flat and stress-3D coverage is
  missing.
- **machine envelope**: Tier 0 uses default tools/machines per sweep. There
  is no per-variant `material_family` or `tool_family` tag in
  `sweep_result.json`, so cases cannot be joined to `seed_goal_matrix.csv`
  by tool/material. Tier 1 result schema must carry those identifiers.

## Recommended next actions

1. **Land the Tier 1 acceptance runner** at
   `crates/rs_cam_core/tests/acceptance_sweep.rs` per
   `SUGGEST_SIM_OPTIMIZE_SWEEP_PLAN.md` §"New acceptance runner". Schema in
   `results.csv` should match §"Result schema" — in particular it must carry
   `goal_id`, `evidence_grade`, `nominal_fz_mm_tooth`,
   `chipload_verdict`, `optimizer_outcome`, and a final `overall_verdict`
   per row.
2. **Add `cases_smoke.csv`** (40–80 rows) covering the operation matrix in
   the plan's Tier 1 table. Keep total runtime under 10 minutes so it can
   gate PRs.
3. **Plug the existing `param_sweep` outputs into the Tier 1 runner** as a
   regression-only sub-stage, so the coverage already demonstrated here
   (105 variants, 35 knobs) remains green while the goal-matrix-driven
   stage layers on top. Do **not** duplicate generation logic — call the
   same operation generators / sim / tool-load APIs used by GUI/CLI.
4. **Add an `alignment_pin_drill` Tier 0 sweep** to close the one
   uncovered catalog row. Trivial extension to `param_sweep.rs`.
5. **Re-fixture the 9 NO_EFFECT cases** so their parameters actually have
   geometry to act on (e.g. add a creased mesh for pencil, a male-plug
   fingerprint for inlay glue_gap). Track these as Tier 1 follow-ups, not
   blockers.

## Top 3 blockers for Tier 1

1. **No goal-matrix consumption path.** Tier 0 emits fingerprints, but the
   sweep result has no `goal_id`, `evidence_grade`, or
   `nominal_fz_mm_tooth` column, and there is no joiner between
   `sweep_result.json` and `seed_goal_matrix.csv`. This is the first piece
   of code Tier 1 needs.
2. **No optimizer invocation in the sweep.** "Optimizer honesty" is one of
   the three top-line acceptance bars and is currently 0% covered. Tier 1
   must call `optimize_toolpath` for at least the under/over/in-band trio
   per baseline case and assert on the outcome variant + gate deltas.
3. **No sim-verdict labelling.** Variants in `sweep_result.json` are not
   tagged with an expected verdict (`Within` / `Exceeds` /
   `not_applicable`), so false-safe / false-fail classification is
   impossible today. The Tier 1 case schema must carry an
   `expected_*_verdict` per row so the sweep can detect calibration
   regressions.
