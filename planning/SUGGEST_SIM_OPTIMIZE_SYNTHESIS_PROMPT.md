# Prompt for coordinator synthesis after Tier 0 + agent smoke

You are working in `/home/ricky/personal_repos/rs_cam`.

Goal: synthesize the completed Tier 0 baseline and the agent-driven smoke acceptance run into a concrete Tier 1 implementation plan. Do not implement code in this pass unless Ricky explicitly asks.

Read:

1. `planning/SUGGEST_SIM_OPTIMIZE_ACCEPTANCE.md`
2. `planning/SUGGEST_SIM_OPTIMIZE_SWEEP_PLAN.md`
3. `planning/toolpath_acceptance/toolpath_catalog.csv`
4. `planning/toolpath_acceptance/seed_goal_matrix.csv`
5. `planning/toolpath_acceptance/cases_agent_smoke.csv`
6. Tier 0 report: `planning/toolpath_acceptance/baselines/2026-05-24_tier0_baseline.md`
7. Latest agent smoke report under `planning/toolpath_acceptance/baselines/*_agent_smoke_acceptance.md`
8. If present, latest `target/acceptance_sweeps/agent_smoke_*/results.csv`

Constraints:

- Do not reset/clean/stash/revert worktree changes.
- Treat generated artifacts under `target/` as read-only evidence.
- If the agent smoke report is missing or incomplete, summarize what is blocked and stop before making implementation claims.

## Synthesis tasks

### 1. Baseline facts

Extract:

- Tier 0 pass/fail status.
- Count of existing `param_sweep` tests, variants, operations covered.
- Uncovered operation rows.
- NO_EFFECT / fixture limitation list.
- Agent smoke cases attempted/generated/simulated/optimized.
- Agent smoke pass/warn/fail/unmodeled counts.

### 2. Crosswalk acceptance areas

Create a table:

| Acceptance area | Tier 0 evidence | Agent smoke evidence | Remaining gap | Tier 1 action |
|---|---|---|---|---|
| Suggest nominal chipload | | | | |
| Sim tool-load verdicts | | | | |
| Drill native gates | | | | |
| 3D finish quality gates | | | | |
| Optimizer honesty | | | | |
| Export gate | | | | |

### 3. Classify findings

For every failure or warning from the smoke run, classify as one of:

- `product_bug`
- `dataset_issue`
- `harness_issue`
- `fixture_issue`
- `unmodeled_gap`
- `expected_current_limitation`

Include a one-sentence rationale and the file/module likely involved if it is a product bug.

### 4. Decide Tier 1 shape

Produce a recommended Tier 1 implementation scope:

- exact case count target;
- which cases from `cases_agent_smoke.csv` should graduate unchanged;
- which cases need fixture/data correction first;
- which gates should be enforced hard in CI;
- which gates should be warn-only initially;
- whether optimizer should run in PR smoke or only standard acceptance.

### 5. Implementation checklist

Write a checklist for the first code change, preferably small:

1. Add/parse `cases_smoke.csv` or use existing `cases_agent_smoke.csv` as seed.
2. Add a core/CLI runner that emits `results.csv`.
3. Compute nominal fz and join goal rows.
4. Run generation + simulation + diagnostics.
5. Capture tool-load report and drill summaries.
6. Implement finish-quality check.
7. Add optimizer check for selected rows.
8. Add summary/failures CSV.

For each item, name likely files and any public API risks.

## Output

Create:

`planning/toolpath_acceptance/baselines/2026-05-24_acceptance_synthesis.md`

Use this structure:

```md
# Suggest / Sim / Optimize acceptance synthesis — 2026-05-24

## Inputs reviewed

## Executive summary

## Evidence summary

## Acceptance crosswalk

## Failure/warning classification

## Tier 1 recommendation

## First implementation slice

## Open questions for Ricky
```

Return to Ricky with:

- synthesis report path;
- top 3 findings;
- recommended next implementation slice;
- explicit ask/question if a tradeoff needs owner input.
