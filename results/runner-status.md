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

## 2026-10-02 ~10:00 UTC: safety fix pushed (origin/master 54e91d0d)

FIXED (pre-existing since at least fd06f407, not from the memory programme):
the rapid-collision SAFETY verdict was silently dropped for every toolpath
that does not start at move 0. ProjectSession::diagnostics_with_evidence
counted collisions from the run-global index list but looked up the worst
collision by comparing the toolpath-LOCAL RapidCollision::move_index with
the GLOBAL boundary range; the verdict needed both. Now one attribution in
global indices, the verdict needs only the count, and get_toolpath_diagnostics
lists the project safety findings for its toolpath. Sentry
controller::tests::rapid_collision_verdict_g_rapidframe (fails before, passes
after). Live on rivmap350: get_project_diagnostics now returns a CRITICAL
project.rapid_collision for "3D Rough 8" (8, worst at local move 33874);
before it returned []. The CLI's verdict list was affected too (its summary
printed only the count).
Note for the lead: the finding's built-in text says "Likely cause:
inter-region rapid moves not lifting to safe-Z" — for these 8 that is wrong
(they lift to safe-Z, then G0 into the r-wide boundary band; see 009 addendum).
Also pushed: budget in MCP generation_status; test artifacts default to a temp
dir; BREAKING CLI: `project` writes simulation.json only with --sim-artifact.
The runner restarts its own GUI via the proxy (3 unattended restarts OK).

## 2026-10-02 ~11:40 UTC: collision move-frame consistency pushed (origin/master b04aa203)

One attribution for every collision surface: compute::simulate::locate_global_move
(half-open [start_move, end_move)). Fixed: the triage/Inspector line named no
toolpath and gave a local index as a bare move; HOLDER collision indices are
local but several GUI sites read them as run moves (a holder hit could land on
the wrong toolpath); the viewport marker pick read the rapid list for a
holder marker; the "Likely cause ... increase retract_z" hint is replaced by
what was measured. Verified on master incl. your 7a7c867d: clippy clean, core
lib stock/session/diagnostics/adaptive3d 469/0, adaptive3d_boundary_clear_parity
3/0, perf_golden 5/0, viz 1110/0, cli 68/0.

## 2026-10-02 ~11:20 UTC: master cc2036a4 pushed (b04aa203 + your 5cf7a1ea)

Your 5cf7a1ea (Onsrud V-bit rows, chipload.rs) merged with the runner's
work cleanly; verified after the merge: clippy clean; core lib tool_load/
feeds/stock/diagnostics 883/0; predicted_feed_gates_f035 4/0; g_cuthist 2/0;
g_chip_ulp 6/0; vendor_lut_sub_1mm 4/0; perf_golden 5/0; viz 1110/0.
The runner GUI now runs cc2036a4.
On branch freshness-followups (NOT merged; UI text for Ricky's look):
drop_simulation(cause) records why a simulation was dropped; a late run keeps
its cut trace (the drain dropped it); stale labels name the cause ("machine
settings changed after this run"); a run in flight reads "Running". Sentry
g_machinestale. Touches session/ (21 drop_simulation call sites) — tell the
runner if that collides with your work.
Possible flaky test (not confirmed): a viz lib test run hung 39 min once;
candidate: controller/tests/stale_cards_g_stalecards.rs real-lane tests
(pump_until_settled waits up to 600 s each, called more than once). Also
seen under load: compute::worker::tests timing tests
(analysis_cancel_completes_quickly, cancel_all_marks_both_lanes_cancelling,
cancelled_toolpath_reports_cancelled_and_no_partial_trace) fail 2/15 runs.

## 2026-10-02 ~13:30 UTC: test robustness pushed (origin/master 6247c467)

The viz lib hang (39 min) amplifier: stale_cards real-lane tests called
pump_until_settled 4x with 600 s each; now one 120 s budget, a stricter
settled check and a lane-state dump on timeout. The three worker cancel
tests slept 20 ms then cancelled a job the lane had not started under load;
now wait_until_running. 10/10 under load; full viz 1110/0.
POSSIBLE DEFECT (read from code, not reproduced, production not changed):
submit_analysis (compute/worker.rs:1491) clears the analysis queue; if a
Generate All plan's simulation step is queued but not started and the
operator starts a collision check, the queued simulation is dropped with no
result, plan_simulation_landed (controller/events/compute.rs:2360) never
fires, and the plan stays in flight. Also: Drop for ThreadedComputeBackend
(worker.rs:1194) joins lanes without a cancel, so a timed-out test waits for
the running job.
Two more worker tests still sleep 20 ms before acting
(duplicate_queued_toolpaths_are_coalesced,
resubmitting_active_toolpath_cancels_and_replaces_it).
The runner's disk filled overnight (per-worktree build caches); recovered,
132 GB free.

## 2026-10-01 ~16:30 UTC (lead): Ricky's OK to push rivmap350

Ricky (2026-10-01): "the rivmap350, yeah push it". Please push
rivmap350.toml and its model files to branch `fixtures-rivmap350`, folder
planning/fixtures/rivmap350/, with relative model paths (as for
fixtures-wanaka100). Note the commit in this file when done.
Also from Ricky: leave the f034 air-cut re-bench (not wanted now); he will
ask you to show him the freshness-followups changes himself.

## 2026-10-01 ~late (lead): rivmap350 push still wanted — tiered-finish trial

Ricky (2026-10-01) asked for a large trial of tiered finishing chains on
the 350 model (tools x cusp x strategy, time vs finish quality). The
cloud runs it now on the x3.5 proxy (rivmap100_memory_repro.toml). To
confirm the winners on the real board, please push rivmap350.toml and its
models to branch `fixtures-rivmap350` (folder planning/fixtures/rivmap350/,
relative model paths), and list Ricky's tool library in this file (the
finishing tools he owns: ball / tapered ball sizes, any Ø6.35 ball).
No heavy job on the PC is asked for.

## 2026-10-08 (runner): rivmap350 pushed — branch `fixtures-rivmap350` (e0df41ba)

Ricky confirmed again today ("yeah push it"). Folder
planning/fixtures/rivmap350/: rivmap350.toml unchanged (model paths are
already relative), the rivmap export, a README, and `tool_library/`, a
copy of Ricky's ~/.config/rs_cam/tools/*.toml. Caution: the library is a
catalogue with generic sizes (balls 3-25 mm), so a row does not prove
ownership. The tools on the board in the project: Ø6.35 end mill, tapered
ball R1.59 (shank 6.35), tapered ball R1.0 x 3.175 x 15 (half angle 2.2°),
6 mm ball nose, 20° V-bit, 6 mm long end mill (pins). Ask Ricky through
this file if you need the owned list confirmed.

Also on master since your last note (pushed by the runner): verbose UI
text step 1 (66a9a01a), compact legend, cache keys, load-report cache.
Open for Ricky: arc tolerance default cusp/2 (your trial), memory budget
vs the 16G cgroup (runner proposes min(half RAM, cgroup max)).
