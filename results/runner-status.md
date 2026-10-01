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

## 2026-10-02 ~05:50 UTC: state for the lead

- origin/master = 0617bf8e (memory programme waves 1-3, pushed 2026-10-02).
- Local branch memory-budget-2026-10-01 = 410bb857, 15 commits ahead of
  master, all gates green (fmt, clippy -D warnings, focused core tests incl.
  perf goldens, viz 1101/0, cli, mcp), NOT merged until Ricky's on-screen look:
  - Wave 4 (operator rulings 2026-10-02): "Capture cutting metrics" checkbox
    and the core metrics flag DELETED (always capture); default memory budget
    = half of system RAM; File > Preferences sets it, applied live.
  - Preferences window with a category bar: General, Memory, Display,
    Simulation, Files & Libraries, Diagnostics, Automation. New core module
    crates/rs_cam_core/src/settings/ (one loader/writer for
    ~/.config/rs_cam/settings.toml). Cut-trace files are NOT written by default
    (operator ruling); artifacts moved from <build tree>/target to
    ~/.cache/rs_cam/artifacts. Library folders: env > settings > default, CLI
    resolves the same.
  - Cut-metric cards: the bottom time-series drawer is DELETED; each card flips
    to a sparkline (median line over a faint min-max band, whole run + limits),
    and a < > button opens one trace in a zoomable modal.
- rivmap350 0.2 mm, metrics always on: generate + simulate peak 7.32 GiB,
  6.72 GiB at rest (was OOM at 16 GiB).
- Please avoid these areas until the merge: ui/preferences.rs, ui/sim_diagnostics.rs,
  ui/components/sparkline.rs, ui/sim_trace_modal.rs, ui/sim_timeline.rs,
  core settings/, budget/, compute/worker*.
- Jobs 001-008 answered; the queue is empty.

## 2026-10-02 ~09:20 UTC: merged and pushed

origin/master = fd4c841c: the memory programme waves 4 + the UI work, merged
with the lead's 6d78b231 (arc length). Verified after the merge: clippy
-D warnings clean; kinematics_per_axis_rate_p1 9/0, perf_golden_sim_metrics
5/0, memory_budget_core 14/0, query_cycle_time 6/0, core --lib machine/budget/
session/settings 237/0, viz 1104/0, cli 67/0. Ricky saw it on screen and approved.
The file claims in the earlier status are released.
New since 0617bf8e: always capture (checkbox and core flag deleted); budget
default half of RAM; File > Preferences with categories; settings module in
core; cut-trace files off by default (~/.cache/rs_cam/artifacts when on);
cut-metric cards flip to a sparkline (median over a band, cutting-only, air =
gap) and open a zoomable trace modal; SimulationCutSample::is_air() names the
gates' air rule (no gate result changes).

## 2026-10-02 ~09:40 UTC: Ricky is away (overnight) — runner policy

Ricky: "you can keep driving these projects while I am gone". So the runner
RUNS jobs marked needs_ricky_ok (he approved 001/003/008/009 the same way)
and says so in each result. Limits: no G-code for the machine; no heavy test
gate beyond what a job names; UI changes wait on a branch for his look.
origin/master = 38cf0677 (adds crates/rs_cam_mcp_proxy: the runner now
restarts its own GUI via gui_status / gui_restart, no /mcp).
Open defect found (runner fixing first): MCP get_project_diagnostics returns
[] on master while inspect_collisions / get_diagnostics report the 8 real
rapid collisions in rivmap350 "3D Rough 8".
