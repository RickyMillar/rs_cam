# Prompt for agent-driven smoke acceptance run

You are working in `/home/ricky/personal_repos/rs_cam`.

Goal: perform the first **agent-driven Suggest / Sim / Optimize smoke comparison** against the reference material. This is different from the Tier 0 `param_sweep` baseline: here you should create/load representative projects, build toolpaths, simulate them, collect diagnostics/tool-load/optimizer results, and compare manually/programmatically to the seed goal rows.

Primary inputs:

- Acceptance spec: `planning/SUGGEST_SIM_OPTIMIZE_ACCEPTANCE.md`
- Sweep plan: `planning/SUGGEST_SIM_OPTIMIZE_SWEEP_PLAN.md`
- Smoke case matrix: `planning/toolpath_acceptance/cases_agent_smoke.csv`
- Goal matrix: `planning/toolpath_acceptance/seed_goal_matrix.csv`
- Toolpath catalog: `planning/toolpath_acceptance/toolpath_catalog.csv`

Output directory:

- `target/acceptance_sweeps/agent_smoke_<YYYYMMDD_HHMM>/`

Human report:

- `planning/toolpath_acceptance/baselines/<YYYY-MM-DD>_agent_smoke_acceptance.md`

## Important constraints

- Do not reset, clean, stash, or revert the worktree.
- Do not fix product code in this run. If a toolpath fails, record it and continue.
- Prefer live `rs_cam` MCP tools if available. If the MCP connection/project cannot be used, record that blocker and stop rather than inventing results.
- Keep runtime bounded: run the full 18-case matrix only if the first 5 cases are stable. Otherwise run a smaller subset and report why.
- Optimizer can be slow. Only run optimizer for rows where `run_optimizer=yes` in `cases_agent_smoke.csv`, and skip it if a case already fails generation/simulation.

## Setup

Start by recording repository state and validating inputs:

```bash
git status --short
python3 - <<'PY'
import csv
from collections import Counter
for path in [
  'planning/toolpath_acceptance/toolpath_catalog.csv',
  'planning/toolpath_acceptance/seed_goal_matrix.csv',
  'planning/toolpath_acceptance/cases_agent_smoke.csv',
]:
    rows=list(csv.DictReader(open(path)))
    print(path, len(rows), 'rows')
    if rows and 'goal_type' in rows[0]:
        print(' goal types:', Counter(r['goal_type'] for r in rows))
PY
```

Create a run directory and a working `results.csv` with the schema from `planning/SUGGEST_SIM_OPTIMIZE_SWEEP_PLAN.md`. If full schema is too much for this first run, at minimum emit these columns:

```text
case_id,operation_kind,fixture,tool_family,tool_diameter_mm,flute_count,material_family,goal_id,goal_type,stage,feed_rate_mm_min,rpm,nominal_fz_mm_tooth,depth_per_pass_mm,stepover_mm,stepover_pct_diameter,scallop_height_mm,chipload_verdict,power_verdict,deflection_verdict,drill_chip_welding_verdict,drill_peck_adequacy_verdict,drill_plunge_feed_verdict,rapid_collision_count,holder_collision_count,air_cut_pct,optimizer_outcome,overall_verdict,failure_class,notes
```

## Case execution procedure

For each row in `cases_agent_smoke.csv`:

1. Load the project template with `rs_cam_load_project(project_template)`.
2. Inspect context:
   - `rs_cam_project_summary()`
   - `rs_cam_inspect_model()`
   - `rs_cam_inspect_stock()`
   - `rs_cam_inspect_machine()`
   - `rs_cam_list_tools()`
   - `rs_cam_list_toolpaths()`
3. Ensure the intended tool exists.
   - If missing, use `rs_cam_add_tool` with `tool_type`, `tool_name`, and `tool_diameter_mm`.
   - Set flute count/stickout if needed with `rs_cam_set_tool_param`.
4. Create or use a single target toolpath for the case.
   - Prefer adding a fresh toolpath with `rs_cam_add_toolpath` for the `operation_kind` and model 0.
   - If the operation cannot be added via MCP, record `harness_error` and continue.
   - Disable unrelated existing toolpaths if they would pollute simulation.
5. Apply `baseline_params` using `rs_cam_set_toolpath_param` when the field exists.
   - If a parameter name is not accepted, record it in notes and continue with defaults if safe.
6. Generate:
   - `rs_cam_generate_toolpath(index)`
   - If generation fails or move count is zero unexpectedly, record `harness_error` or `generation_empty`.
7. Simulate:
   - `rs_cam_run_simulation({"resolution": 0.75})` for the smoke run.
   - Collect `rs_cam_get_diagnostics()`.
   - Collect `rs_cam_get_tool_load_report()`.
   - For drill cases, collect `rs_cam_get_cut_trace({"toolpath_id": index})` and read `drill_summaries`.
8. Optionally narrate/screenshot failures:
   - `rs_cam_narrate_toolpath(index)`
   - `rs_cam_screenshot_toolpath(...)` for failures only.
9. If `run_optimizer=yes` and sim completed:
   - `rs_cam_optimize_toolpath(index)`
   - Re-check whether the outcome is `Ranked`, `Skipped`, etc.
   - Mark failure if the recommendation worsens any gate or finish quality tier.
10. Append result rows.

## Comparison rules

### Nominal chipload

Compute:

```text
nominal_fz_mm_tooth = feed_rate_mm_min / (rpm * flute_count)
```

For `vendor_lut_chipload` and `community_nominal_chipload` rows:

- `pass` if nominal fz is inside `[nominal_chipload_min_mm_tooth, nominal_chipload_max_mm_tooth]`.
- `warn` if within 10% outside the band.
- `fail` if more than 10% outside, unless a machine cap or tool limit clearly explains it.

Important: record nominal fz separately from sim-corrected chip thickness. Do not mix the units.

### Sim/tool-load

For milling cases:

- pass if chipload, power, and deflection verdicts are `Within`.
- fail if any is `Exceeds`.
- mark `unmodeled` if any needed criterion is `Unmodeled`.
- fail on any rapid/holder/shank collision.
- warn/fail on high air-cut according to operation family; do not over-weight air-cut for sparse trace/project-curve paths.

### Finish quality

For `surface_finish_stepover_scallop` rows:

- final finish 5–20% stepover is acceptable by default.
- 20–35% is semi-finish only.
- >35% is roughing-only and should fail final-finish cases.
- If scallop height is directly exposed, compare against the row's `scallop_height_max_mm`.
- Optimizer fails if it recommends coarser quality than the requested `quality_tier`.

### Drill

For `drill_native_gates` rows:

- milling chipload is not applicable.
- use `drill_gates` from `rs_cam_get_tool_load_report()` and `drill_summaries` from cut trace.
- optimizer should normally return `Skipped`/not applicable. A confident feed-RPM-DOC recommendation for drill is suspicious and should be recorded as `optimizer_should_skip`.

### Effective-diameter / V-bit rows

For `community_effective_diameter_chipload` rows:

- mark chipload comparison as advisory unless effective engaged diameter is available from params/tool geometry.
- prefer `warn`/`unmodeled` over false precision.
- fail only on clear load/collision/drill-quality problems or an overconfident optimizer recommendation.

## Variant handling

The `variants` column is there to probe calibration. If time permits, run at least one low and high variant for each of the first five cases. For the first smoke pass, it is acceptable to record baseline rows only for the remaining cases.

Variant result rows should use the same `case_id` and a `variant_id` note such as:

- `baseline`
- `feed_rate_0.5x`
- `feed_rate_1.5x`
- `stepover_quality_bad`
- `drill_deep_fail`

## Report structure

Write `planning/toolpath_acceptance/baselines/<YYYY-MM-DD>_agent_smoke_acceptance.md`:

```md
# Agent smoke acceptance baseline — <date>

## Scope

Agent-created toolpaths and sims compared to `seed_goal_matrix.csv`. This is an exploratory smoke run, not a complete Tier 1 harness.

## Repository state

## Input sanity

- catalog rows:
- goal rows:
- smoke cases:

## Commands/tools used

List MCP tools and shell commands used.

## Result summary

| Count | Value |
|---|---:|
| cases attempted | |
| cases generated | |
| cases simulated | |
| optimizer runs | |
| pass | |
| warn | |
| fail | |
| unmodeled/uncovered | |

## Results by case

| Case | Operation | Goal | Suggest | Sim | Optimize | Verdict | Notes |
|---|---|---|---|---|---|---|---|

## Top failures

Include exact failure class and whether it looks like product bug, dataset issue, harness issue, or unmodeled gap.

## Reference comparison notes

Discuss nominal chipload vs corrected chipload, finish quality, drill gates, and V-bit ambiguity.

## Recommended next steps

1. ...
```

## Return to Ricky

Return a short summary with:

- report path;
- results CSV path;
- cases attempted/generated/simulated;
- top 3 pass/fail findings;
- whether the next step should be harness implementation or dataset correction.
