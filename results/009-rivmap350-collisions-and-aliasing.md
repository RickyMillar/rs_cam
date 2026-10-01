---
id: 009
state: done
commit: CLI release at branch e5bd3292 (= master 0617bf8e core + UI/wave 4; simulation outputs byte-identical to master per the W2 CLI A/B)
started: 2026-10-02T06:25Z
finished: 2026-10-02T08:10Z
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

## Part B: wall texture at 0.25 mm — NOT a moire

Screenshot: results/009-walls-0.25mm.png (compare results/007-walls-0.2mm.png).
The GUI at 0.25 mm (generate_all at 0.25, kinematics imported) also confirms
the SAME 8 collision moves (inspect_collisions: local 27653 ... 35040).

The texture is the same at 0.25 as at 0.2 mm: fine horizontal lines up the
ramped wall and a wavy ripple band at the wall foot, same direction and
similar spacing. A moire would change with the cell; this did not, so it is
not a simulation aliasing artefact. Ricky's verdict (verbatim): "you can see
the ramped walls still have some texture.. maybe its just an artifact of the
toolpath moving wiggly over that area". Runner reading: the ripples follow
the iso-scallop rings (Scallop Finish, R1.0 tapered ball, scallop_height 0.1,
ring pitch 0.87 mm along the surface); each ring leaves a cusp up to 0.1 mm,
and the viewer's grazing light exaggerates it. So most likely the real cusp
pattern of the toolpath, not a gouge. Levers if it matters: smaller scallop
height, or a different finishing direction on steep walls.

Files on the runner's PC: setup-2 G-code
/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/065a14e8-82c6-476f-9331-96d953a0dcb4/scratchpad/j009_gcode/rivmap350_2_Setup_2.nc ;
logs .../scratchpad/job009a.log

## Addendum: read-only root-cause trace of the 8 collisions (runner, 2026-10-02)

No code was changed. Trace from code on master 6013b583, plus a G-code check.

**Emitter (proven):** `adaptive3d/path.rs:1545-1556` lifts to safe_z, rapids
across, then G0s down to `descent_floor = rapid_floor_z + 0.5`
(path.rs:1509-1514). `rapid_floor_z` comes from `plan_entry`
(`adaptive3d/clearing.rs:762-797`), the max conservative top in a tool-radius
disc of the PLANNER'S OWN stock (path.rs:647-674, pre-cleared outside the
boundary at path.rs:803-830, stamped by every planned segment). No dressup
lowers this rapid. TSP is ruled out for these: a reordered group re-frames at
safe_z with a fed first move (dressup/tsp.rs:622-686); a G0 to depth can only
survive in planner order (fits "existed before fd06f407").

**Cause (mechanism proven, application to rivmap350 confirmed by the G-code
shape):** the planner's pre-clear uses `processed_set.single_union()`
(session/compute/generation.rs:432-442), NOT inset by the tool radius; the
post-generation clip insets each region by r for containment "inside"
(generation.rs:1105-1143) and turns removed cuts into safe-z rapids
(geometry/boundary.rs:455-503). So a band ~r wide along every boundary edge
(holes included) is "cut" in the planner stock but still standing. If the
outline has more than one polygon (model_outline with holes=true can), 
`single_union` returns None and the planner pre-clears NOTHING.
`tests/adaptive3d_boundary_clear_parity.rs:14-40` already measures this
over-claim (~27 % of cells) but rates it "Safety LOW / no consumer outside
adaptive3d" — it misses `plan_entry`, which reads that stock for every rapid
floor.

**G-code check (runner):** every one of the 8 is a tiny boundary crumb:
G0 down, a ~1 mm G1 plunge at F843, a 0.4-4 mm lateral cut, then retract to
Z5 — and all 8 sit at the board edges (Y~23, X~34-46, X~337-346, Y~486 on a
380 x 510 stock): the r-wide band case. "3D Rough 8": stock_source fresh;
boundary model_outline (model 5) holes=true, containment inside, offset 0.

**Why no sentry catches it:** adaptive3d_entry_stock_aware asserts "no rapid
enters material" but with `boundary: None` (line 144);
adaptive3d_planner_never_ahead_of_emitted_path and the wanaka phantom-rapid
test use no boundary; the parity test counts cells, not rapids, and its D4
arm is #[ignore] (line 736).

**Recommended fix layer (the lead's decision, D4):** pre-clear the planner
stock against the SAME processed, r-inset region set the clip uses, passed as
a set (not single_union); plus D4's waterline_cleanup boundary half. Not a
patch to descent_floor. Sentry without a fixture file: the synthetic plate of
adaptive3d_entry_stock_aware + an "inside" boundary (a rectangle with a hole,
and a two-polygon case to force single_union = None), run through the session
door so the clip runs, replay on fresh dexels, assert zero rapid collisions at
0.25 mm; optionally un-ignore the D4 parity arm.
