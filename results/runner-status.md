---
runner_session: rs-cam-be (local terminal on Ricky's PC; replaces session_01X9bPqEnCfUkrbNprBjUs3d)
updated: 2026-09-30T21:06:35Z
---
## Summary

The runner moved from the Remote Control session to a local terminal
session. The mailbox protocol in README.md is unchanged.

- The runner polls origin/comms/runner every 60 s.
- The runner cannot send a send_message to the lead yet: the lead session
  is not reachable from this terminal. Until that changes, poll
  results/ yourself.
- The runner now has the rs-cam MCP (rs-cam-capped: the GUI under
  systemd-run MemoryMax=16G). It can drive the GUI for MCP jobs.

Queue: empty. Jobs 001-006 are done.

## 2026-10-01 ~22:20 UTC: new runner task from Ricky

Ricky asked the runner to audit memory feasibility across the whole app
and to draft a memory-budget architecture (config budget, preflight
estimate, queueing, graceful stop, degrade-with-warning, e.g. lighter
toolpath renders). Read-only agents; no code changes. The spec goes to
planning/memory_budget_2026-10-01/ (not committed until the lead agrees);
the path follows here. Tell the runner if this collides with your G-SIMMEM
work.

Re-opened from result 006 addendum 3 (the correction over-withdrew):
after a GUI Generate All on 09-30, RSS fell 8.78 -> 1.48 GB when the sim
started; after MCP generate_all (plan ran 1 simulation) RSS held
8.1-9.2 GB and never fell. Same commit, same 0.2 mm, trace off in both
GUI-button runs. That ~7 GB of retained memory is unexplained and is
audit item 1.

## 2026-10-01 ~22:45 UTC: memory programme RUNNING here (operator ruling)

Ricky ruled: the runner session orchestrates the memory programme, with
parallel agents in worktrees, merged later. Branch
`memory-budget-2026-10-01` off fd06f407 (local until merge). The plan is
results/memory-programme-plan.md (the repo copy is
planning/memory_budget_2026-10-01/PLAN.md). Please steer clear of these
files until the merge: controller/events/{simulation,compute}.rs,
compute/simulate.rs, compute/sim_prefix.rs, dexel_stock/, stock/dexel*.rs,
stock_mesh.rs, app/simulation.rs, worker/execute/, session/cycle_time.rs,
ui/readiness*.rs, interrupt.rs, rest_stock.rs grid cap, a new budget/ folder.
The lead's fd06f407 touched none of them.

## 2026-10-02 ~02:20 UTC: memory programme waves 1-3 done on the local branch

Branch `memory-budget-2026-10-01` at 0617bf8e (local, not pushed), 3 waves,
all gates green (fmt, clippy -D warnings, focused core tests incl. both perf
goldens, viz/cli/mcp crates). CLI A/B: simulation.json byte-identical.
rivmap350 0.2 mm: re-run peak 14.64 -> ~5 GiB; metrics ON now fits at
6.87 GiB (was OOM at 16G). Full numbers: results/memory-programme-baselines.md.
Breaking changes are listed in the branch's FEATURE_CATALOG diff (CLI
`project` needs --output-dir; null trace figures; generate_all reply keys).
The branch is ready for the operator's merge call.
