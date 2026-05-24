# Prompt for Tier 0 baseline agent

You are working in `/home/ricky/personal_repos/rs_cam`.

Goal: run the **first non-invasive baseline** for the Suggest / Sim / Optimize acceptance effort. Do not implement the new harness yet. Use the existing parameter sweep infrastructure and write a baseline report.

Read first:

1. `planning/PROGRESS.md`
2. `FEATURE_CATALOG.md`
3. `planning/SUGGEST_SIM_OPTIMIZE_ACCEPTANCE.md`
4. `planning/SUGGEST_SIM_OPTIMIZE_SWEEP_PLAN.md`
5. `crates/rs_cam_core/tests/param_sweep.rs` header/comments

Important constraints:

- The worktree may already contain unrelated user changes. Do **not** reset, clean, stash, or revert anything.
- Generated artifacts under `target/` are fine.
- Only create/update the baseline report under `planning/toolpath_acceptance/baselines/` unless you find a tiny doc typo that blocks understanding.
- If tests fail, do not fix code in this pass. Capture the failure and continue with narrower commands if possible.

## Commands to run

Start with status and CSV validation:

```bash
git status --short
python3 - <<'PY'
import csv
from collections import Counter
for path in [
    'planning/toolpath_acceptance/toolpath_catalog.csv',
    'planning/toolpath_acceptance/seed_goal_matrix.csv',
]:
    rows=list(csv.DictReader(open(path)))
    print(path, len(rows), 'rows', len(rows[0]) if rows else 0, 'columns')
    if path.endswith('seed_goal_matrix.csv'):
        print('goal_type counts:', Counter(r['goal_type'] for r in rows))
PY
```

Run Tier 0 smoke subset, timing each command if possible:

```bash
cargo test --test param_sweep sweep_pocket_stepover -- --nocapture
cargo test --test param_sweep sweep_adaptive_stepover -- --nocapture
cargo test --test param_sweep sweep_dropcutter_stepover -- --nocapture
cargo test --test param_sweep sweep_drill_cycle -- --nocapture
cargo test --test param_sweep sweep_scallop_height -- --nocapture
```

If the smoke subset passes or only has isolated failures, run the full existing sweep:

```bash
cargo test --test param_sweep -- --nocapture
```

Then summarize generated artifacts:

```bash
python3 - <<'PY'
import json
from pathlib import Path
from collections import Counter, defaultdict
root=Path('target/param_sweeps')
files=sorted(root.rglob('sweep_result.json'))
print('sweep_result files:', len(files))
ops=Counter(); params=Counter(); variants=0
examples=[]
for p in files:
    try:
        data=json.load(open(p))
    except Exception as e:
        print('bad json', p, e)
        continue
    op=data.get('operation', p.parts[-3] if len(p.parts)>=3 else 'unknown')
    param=data.get('parameter_name', p.parts[-2] if len(p.parts)>=2 else 'unknown')
    ops[op]+=1; params[param]+=1; variants += len(data.get('variants', []))
    if len(examples)<10:
        examples.append((str(p), op, param, len(data.get('variants', []))))
print('operations:', dict(sorted(ops.items())))
print('parameters:', dict(sorted(params.items())))
print('total variants:', variants)
print('examples:')
for row in examples:
    print(' ', row)
PY
```

If available, run the existing analyzer:

```bash
python3 toolpath_stress_test/agents/analyze_sweep.py target/param_sweeps/
```

If that analyzer fails because of an environment/data issue, record the failure in the report and continue.

## Baseline report to write

Create:

`planning/toolpath_acceptance/baselines/<YYYY-MM-DD>_tier0_baseline.md`

Use this structure:

```md
# Tier 0 acceptance baseline — <date>

## Scope

Existing `param_sweep` only. No new acceptance harness. This baseline is a coverage/regression snapshot, not final Suggest/Sim/Optimize proof.

## Repository state

- git status summary:
- note any pre-existing modified/untracked files:

## Dataset sanity

- toolpath catalog rows:
- goal matrix rows:
- goal type counts:

## Commands run

| Command | Result | Runtime | Notes |
|---|---|---:|---|

## Artifact summary

- `target/param_sweeps/` exists: yes/no
- `sweep_result.json` count:
- total variants:
- operations covered:
- parameters covered:

## Failures / anomalies

List failing test names, panic messages, suspicious analyzer findings, or missing artifacts.

## Coverage against acceptance plan

| Acceptance area | Current Tier 0 coverage | Gap |
|---|---|---|
| Suggest nominal chipload | | |
| Sim tool-load verdicts | | |
| Drill native gates | | |
| 3D finish quality gates | | |
| Optimizer honesty | | |
| Export gate | | |

## Recommended next actions

1. ...
```

## What to return to Ricky

Return a concise summary with:

- whether smoke passed;
- whether full sweep ran;
- baseline report path;
- headline artifact counts;
- top 3 blockers/gaps for implementing Tier 1.
