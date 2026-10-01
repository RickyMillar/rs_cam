---
id: 010
state: done
commit: 7a7c867d (origin/master; contains 54e91d0d + G-BOUNDARYPHANTOM)
started: 2026-10-02T09:56Z
finished: 2026-10-02T10:10Z
---
## Summary — CONFIRMED: "3D Rough 8" has 0 rapid collisions (was 8)

Ricky is away; the runner ran this under his 2026-10-02 "keep driving"
instruction. Simulation only; no G-code went to the machine.

| Run | 3D Rough 8 moves | Rapid collisions (all toolpaths) | Verdict |
|---|---|---|---|
| CLI project, 0.25 mm, 16G cgroup (186 s, max RSS 5.26 GiB) | 36 421 (was 37 408) | 0 (every toolpath 0) | OK |
| GUI (MCP generate_all, 0.25 mm, 2026-05-26 Shapeoko $$ imported) | — | 0 | get_project_diagnostics = [] |

No collision is left, so step 3 (0.125 mm re-run) does not apply.

Cycle time (GUI, wall clock with the imported kinematics, 0.25 mm):
- "3D Rough 8": 11 679 s = 3:14:39 (cutting 9 175 s, rapid 2 504 s; air 17.8 %).
  BEFORE: not recorded per toolpath (jobs 006/007 recorded project totals
  only, and the CLI summary has no per-toolpath time).
- Project total: 46 130 s = 12:48:50 now vs 12:44:47 at 0.25 mm earlier
  tonight (410bb857 build, pre-fix). That delta ALSO contains the
  arc-length change 6d78b231, so it is not attributable to this fix alone.
- CLI summary (no kinematics in the project file): total_runtime_s 35 827.

Fixture push (rivmap350 + models to a branch): NOT done — Ricky's data;
waiting for his OK in the morning.
