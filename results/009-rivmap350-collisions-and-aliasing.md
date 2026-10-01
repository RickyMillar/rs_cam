---
id: 009
state: running
commit: CLI release at branch e5bd3292 (= master 0617bf8e core + UI/wave 4; simulation outputs byte-identical to master per the W2 CLI A/B)
started: 2026-10-02T06:25Z
finished:
---
## Summary — part A DONE: the 8 rapid collisions are REAL strikes (persist at 0.2 / 0.25 / 0.125 mm)

DO NOT RUN the "3D Rough 8" G-code on the machine until the lead has looked.

| Resolution | Rapid collisions ("3D Rough 8") | Max RSS (CLI, 16G cgroup) | Wall |
|---|---|---|---|
| 0.2 mm | 8 | 6.92 GiB | 298 s |
| 0.25 mm | 8 | 5.25 GiB | 215 s |
| 0.125 mm | 8 | 14.49 GiB | 542 s |

All other toolpaths: 0 collisions at every resolution. (The CLI reports only
counts per toolpath at 0.25/0.125; the move indices below are from the GUI
inspect_collisions at 0.2. Same count at every cell.)

"3D Rough 8": adaptive3d, contour_parallel, Ø6.35 FLAT end mill (tool 0),
DOC 13.1 (depth_per_pass), entry_style plunge, stock_to_leave_axial 4.8,
dressups: optimize_rapid_order (TSP) ON, arc_fitting ON (tol 0.05),
segment_merge ON, link_moves ON (max 10 mm), air_bridge_policy always;
boundary model_outline (model 5, holes) inside. Setup 2 (flipped).

Every collision has the SAME shape: retract to Z 5, rapid across at Z 5, then
a G0 STRAIGHT DOWN to the next region's start depth (Z -4.2 .. -10.2 in the
setup-2 G-code frame, Z0 = stock top). The simulator finds uncut stock in
that vertical rapid. Mapping: reported local move N = setup-2 G-code motion
line N+2 (checked against the GUI's first collision: sim frame
(46.0, 23.3, 31.0) -> (46.0, 23.3, 21.486) = G-code Z 5 -> -4.514, offset 26).

| local move | global move | G0 descent at X, Y | Z from -> to (G-code) |
|---|---|---|---|
| 27653 | 584351 | 46.038, 23.283 | 5 -> -4.514 |
| 27658 | 584356 | 111.125, 23.283 | 5 -> -4.437 |
| 27663 | 584361 | 192.088, 23.283 | 5 -> -4.779 |
| 27672 | 584370 | 337.608, 23.283 | 5 -> -6.723 |
| 28958 | 585656 | 346.075, 74.083 | 5 -> -9.922 |
| 30471 | 587169 | 33.867, 180.446 | 5 -> -4.219 |
| 33874 | 590572 | 87.312, 486.304 | 5 -> -10.246 |
| 35040 | 591738 | 34.050, 284.546 | 5 -> -7.160 |

Intent: the rapid descent to a region start (a link/entry rapid), i.e. the
toolpath descends at G0 to the new region's depth instead of to a clearance
just above the local stock and then feeding down. Runner hypothesis (not
traced in code): the region start depth is below stock that a LATER region
(or a later Z level) would clear, so the rapid passes through uncut material;
possibly the TSP reorder (fd06f407 "a reordered group's fed start is rejoined
by retract") or the air_bridge_policy. Note: these 8 already existed at
5b34e710 (job 006), i.e. before fd06f407.

Does "3D Rough 8" use what fd06f407 changed? Kernel ball/bull/tapered: NO
(flat end mill). 2D adaptive/: NO (adaptive3d). TSP rapid order: YES
(optimize_rapid_order on). Arc fit 3D tolerance: YES (arc_fitting on).

Side findings:
- MCP get_project_diagnostics returned [] and get_toolpath_diagnostics(4)
  listed no collision, while inspect_collisions/get_diagnostics report 8
  rapid collisions (branch GUI, after a GUI simulate). Surface parity gap.
- The CLI summary/tp_*.json carry only collision COUNTS, no positions/moves.
- 0.125 mm CLI peak 14.49 GiB on the 16 GiB cgroup (the default app budget,
  half of RAM, is 27 GiB on this PC, above the cgroup).

Part B (wall texture at 0.25 mm, Ricky's eye): pending.

Files on the runner's PC: setup-2 G-code
/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/065a14e8-82c6-476f-9331-96d953a0dcb4/scratchpad/j009_gcode/rivmap350_2_Setup_2.nc ;
logs .../scratchpad/job009a.log
