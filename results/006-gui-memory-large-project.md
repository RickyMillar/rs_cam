---
id: 006
state: done
commit: 5b34e710
started: 2026-09-30T20:38:46Z
finished: 2026-09-30T20:54:42Z
---
## Summary

rivmap350.toml in the release GUI at 5b34e710, under systemd-run
MemoryMax=16G MemorySwapMax=0. RSS sampled every 5 s. Ricky drove.

| Step (UTC) | Peak RSS | Notes |
|---|---|---|
| Load + generate all (20:41–20:50) | 8.73 GB | 0.8 -> 7.9 GB in 30 s at +210 s, then flat at 8.14 GB for 7 min; 8.73 GB at the end |
| Simulate 0.5 mm (20:50) | 2.53 GB | RSS dropped 8.78 -> 1.48 GB when the sim started; sim done in well under 1 min |
| Simulate 0.2 mm (20:52) | 9.17 GB | 2.5 -> 9.1 GB in 30 s, flat, then released to 2.0 GB; Ricky: "running noticeably smoother" |
| Simulate 0.1 mm (20:54, Ricky's extra test) | > 16 GB: OOM-killed | 10.09 GB at the last sample, then over 16 GB within 5 s |

0.1 mm kill, from the kernel log:
  Memory cgroup out of memory: Killed process 918703 (rs_cam_gui)
  total-vm:23870700kB, anon-rss:16736460kB
  scope: Failed with result 'oom-kill'; 16.0G memory peak, 0B swap peak.
The desktop was not affected.

GUI responsive: yes at 0.5 and 0.2 (Ricky). No panic or error text on
stderr (the GUI writes none). systemd-run --scope returned rc=0 even for
the OOM kill, so read the journal, not the exit code.

Not yet reported by Ricky: rapid collision count and cycle time shown in
the GUI. The runner adds them to this file if Ricky reads them.

## Runner notes

- Pre-fix reference: 2.3 -> 19.4 GB in 30 s. After the fix, 0.5 and 0.2
  are both bounded and release their memory at the end.
- 0.2 -> 0.1 is 4x the cells; 9.17 GB x 4 would be about 37 GB, so the
  kill fits a simple cell-count scale. The steep step (10 -> 16+ GB in
  5 s) suggests one large allocation.
- Ricky's point: 0.5 mm is too coarse for the R1.0 scallop detail; 0.2 mm
  is his working resolution.

Logs on the runner's PC: /tmp/claude-1001/-home-ricky-personal-repos-rs-cam--claude-worktrees-bridge-cse-01X9bPqEnCfUkrbNprBjUs3d/84f1aa02-8de5-5da0-aff8-1fa9722b654e/scratchpad/job006_rss.log (5 s samples),
/tmp/claude-1001/-home-ricky-personal-repos-rs-cam--claude-worktrees-bridge-cse-01X9bPqEnCfUkrbNprBjUs3d/84f1aa02-8de5-5da0-aff8-1fa9722b654e/scratchpad/job006_marks.log (Ricky's step times)

## Addendum 2026-10-01: 0.2 mm through the MCP is OOM-killed

Ricky asked for the collision count and cycle time at 0.2 mm, so the
runner drove the same project through the MCP. Binary: release
rs_cam_gui at origin/master 5b34e710 (rebuilt 2026-10-01 10:12 NZDT),
under systemd-run MemoryMax=16G MemorySwapMax=0.

| Step (UTC, 2026-09-30) | RSS (GiB) | Notes |
|---|---|---|
| load_project rivmap350.toml (21:15) | 0.1 -> 0.8 | 2 setups, 8 toolpaths |
| generate_all simulation_resolution_mm=0.2 (21:16-21:21) | 8.13, flat | ok: 5 generated, 0 errors, 8 steps, 1 simulation |
| run_simulation at stored 0.2 (21:22) | 8.13 -> 12.9 in 15 s | OOM-killed at 21:22:19 |

Journal: run-rf3757d2....scope: "A process of this unit has been killed
by the OOM killer"; 16.0G memory peak, 0B swap peak. The MCP connection
closed. No collision count or cycle time was produced.

Difference from Ricky's GUI run on 2026-09-30 (same commit): there, RSS
fell from 8.78 to 1.48 GB when the simulation started, then peaked at
9.17 GB. Here, the 8.13 GiB that generate_all holds is NOT released, and
the closing simulation stacks on top of it. Runner hypothesis only (not
verified in code): the MCP path keeps the plan's own 0.2 mm simulation,
or the generation data, alive while run_simulation builds a new one.

Log on the runner's PC: /tmp/claude-1001/-home-ricky-personal-repos-rs-cam/065a14e8-82c6-476f-9331-96d953a0dcb4/scratchpad/job006b_rss.log

## Addendum 2: generate via MCP, simulate by the GUI button (2026-09-30 UTC)

Same binary and 16G cap. Fresh GUI (PID 1136082).

| Step (UTC) | RSS (GiB) | Notes |
|---|---|---|
| load + generate_all 0.2 (21:25-21:26) | 8.20, flat | ok: 7 generated (the first MCP run reported 5 for the same call; cause unknown), 0 errors, 8 steps, 1 simulation |
| Ricky clicks Simulate, 0.2 (21:27:3x) | 8.79 -> 12.63 -> peak 14.84 -> 9.10 | survived; no drop to ~1.5 GB at sim start, as in Ricky's 09-30 run |

Result (get_diagnostics after the GUI simulation):
- rapid_collision_count 8, all in "3D Rough 8" (index 4, adaptive3d). Every other toolpath 0.
- collision_count 0; verdict "WARNING: rapid collisions detected".
- total_runtime_s 0.0, air % 0.0, samples_total 0: the MCP diagnostics do not
  see the GUI-button simulation's cycle time (looks like G-MCPSIMMIRROR).
  Cycle time from Ricky's screen: 8:13:19 (cutting only, no accel).

Peak 14.84 GiB is 1.2 GiB under the cap. The MCP run_simulation path went
past 16 GiB from the same 8.1 GiB start, so the two paths differ by at least ~1.2 GiB.

## Addendum 3: CORRECTION, and a UX finding from Ricky

### Correction to addendum 1 and 2

The GUI "Capture cutting metrics" checkbox (Simulation > Setup & run)
defaults OFF on every launch: `SimulationMetricOptions` derives `Default`
(`enabled: false`), and `playback_state.rs:63` uses that default. The
session/MCP default is ON (`SimulationOptions::default`,
`metrics_enabled: true`, `session/mod.rs:1226`).

So the runs in this job measured two different things:
- Every GUI-button simulation (09-30: 0.5/0.2/0.1; addendum 2) ran
  WITHOUT a cut trace. Its peaks (2.53, 9.17, 14.84 GiB) are stock only,
  and they do NOT exercise the G-SIMMEM fix.
- The MCP run_simulation (addendum 1) ran WITH the trace, and it went
  past 16 GiB from an 8.13 GiB start.
The addendum-1 hypothesis "generate_all's plan memory is not released" is
WITHDRAWN: the gap between the two paths is the trace. rivmap350 at
0.2 mm WITH the trace does not fit under 16 GiB at 5b34e710 after an MCP
generate_all.

Proof that the GUI run had no trace: its cycle time 8:13:19 equals
sum(cutting_distance / feed_rate) over the 7 enabled toolpaths (29 599 s),
so all 7 fell back to CycleTimeBasis::CuttingOnly; get_diagnostics shows
samples_total 0 and total_runtime_s 0.0. The rapid-collision count (8, all
in "3D Rough 8") is still valid: it does not come from the trace.

Also done: the repo's 2026-05-26 Shapeoko XXL `$$` (the
from_grbl_settings_parses_real_shapeoko_xxl_dump fixture) was imported by
MCP import_machine_settings (not saved to the project file). It had no
effect because no trace existed. The library entry shapeoko_pro_xxl is out
of date (scalar 350, $11 0.010, no per-axis values).

### UX finding (Ricky: "not ideal. I've been clueless.")

Ricky ran simulations for two sessions and saw a "cutting only, no accel"
time, an empty chipload check and 0 % air, with no hint that one
default-off checkbox caused all of it. Runner's read of the defects:

1. Surface parity: the same project gives a trace on MCP and none in the
   GUI (violates the GUI/MCP/CLI number-parity rule).
2. Silent empty state: without a trace, surfaces show 0.0 (air %,
   runtime) where the code's own rule says null = NOT MEASURED.
3. Wrong remedy: CuttingOnly's remedy says "Run a simulation", which the
   operator had just done. It never names the checkbox, and it hides
   the second cause (no machine kinematics) because `worse()` folds both
   into one label.
4. The remedy text lives in panels (readiness, preflight, export wizard),
   not beside the time where the operator reads it.
5. The toggle exists to save memory, and it hides the memory problem it
   was added for: a 0.2 mm run "works" only because it measures nothing.

Suggested direction (the lead decides): delete the toggle and always
capture (breaking changes are ruled OK), and bound the trace instead; or,
at minimum, make the GUI default match the session default, and have
every trace-dependent surface say "not measured: cutting metrics were
not captured" with the control's name. Name each cause separately in the
cycle-time label ("no trace", "no machine kinematics").

### Confirmation (22:04 UTC)

Ricky ticked "Capture cutting metrics" and clicked Simulate at 0.2 mm
(same GUI, MCP-generated state, 2026-05-26 kinematics imported). RSS
9.15 -> 10.55 -> 11.93 -> 14.09 GiB in 15 s, then OOM-killed at 22:04:48
("16.0G memory peak, 0B memory swap peak"). So with the trace ON, the GUI
path fails exactly as the MCP path did. rivmap350 at 0.2 mm with a cut
trace needs more than 16 GiB at 5b34e710 after a generate_all: the
G-SIMMEM fix is not enough for this project.
