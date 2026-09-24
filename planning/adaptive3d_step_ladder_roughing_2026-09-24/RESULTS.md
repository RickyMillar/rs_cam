# Phase 1 arms on rivmap100 (2026-09-24)

Measured through the GUI MCP, simulation 0.5 mm, accel time from
`rs_cam_cli nc-time --max-feed 6000 --rapid-feed 10000` (built-in
shapeoko_xxl_ricky_tuned kinematics: accel X/Y/Z 500/500/270 mm/s²,
junction deviation 0.020 mm). Face alone = 605 s; rough time = export total −
605 s. Scallop and Project Curve disabled. Artifacts:
session scratchpad `arms/` (not in the repo).

Model depth: after Face the stock top is Z 12; the rough goes to about
Z 2–4. So the whole rough is about 8–10 mm deep. B10 is one level.

| arm | rough accel s | moves | cut mm | rapid mm | entries | peak axial DOC mm | vol/accel s (Face+Rough) mm³/s |
|---|---|---|---|---|---|---|---|
| A-regen (DPP 2, Global) — baseline | 1741 | 10 442 | 26 613 | 8 253 | 268 | 2.0 | 31.5 |
| A2 (DPP 2, By Area) | 1481 | 8 905 | 23 705 | 5 681 | 219 | 2.0 | 35.2 |
| B5 (DPP 5, By Area) | 871 | 5 373 | 15 121 | 3 017 | 125 | 5.07 | 48.6 |
| B10 (DPP 10, By Area) | 635 | 3 922 | 9 839 | 2 191 | 84 | 8.0 | 57.6 |
| Bx (10 @ leave 5.5, then 5 @ 0.5) — INVALID as a ladder proxy | 944 | 5 798 | 15 772 | 3 788 | 152 | 5.07 | 46.3 |

Notes:

- The Phase 1 gate passes: B5 and Bx beat A on time.
- The pre-session arm A (19 624 moves, 3528 s) did not reproduce. A
  regenerate with the same params gives 10 442 moves, twice, the same.
  Cause not established (a different prior stock is likely, not verified).
  A-regen is the baseline.
- By Area alone saves 260 s (15 %).
- The removed volume (Face + Rough) falls from 73 966 mm³ (A-regen) to
  71 428 mm³ (B10), about 3.4 %. Larger steps leave more for the finish.
  Section 4's "same volume" assumption does not hold; the finish time
  must be part of the score.
- Load: the static rigidity rule passes B10 (8.0 of 9.0 mm, 0.58 kW,
  53 µm). The simulation's `entry_load` finding grows with the step
  (critical: A-regen 26 samples, peak 1.14 mm; B10 560 samples, peak
  5.96 mm). The load gates skip entry spans.
- Bx is not a valid proxy of the ladder. With leave 5.5 on an 8 mm deep
  model the first operation cuts almost nothing, and the second one repeats
  B5. Do not cite its 944 s.
- B10 confirms F3. The model is 8–10 mm deep, so B10 is one level, and the
  drape takes the whole depth next to the walls (peak axial DOC 8.0 mm,
  `entry_load` 560 samples to 5.96 mm). The fit rule exists to stop that
  bite near the walls and give those bands to 5 mm passes.
- This model is too shallow to test the ladder's nesting: [10, 5] is one
  coarse slab here. The synthetic fixture (≥ 25 mm deep, several coarse
  slabs, [10, 5, 1]) is the only test of the nesting.
- Pre-registered bound for Arm C ([10, 5], By Area) on rivmap100: rough
  accel time between B10 (635 s) and B5 (871 s). The ladder's gain over B5
  is bounded by B5's time in the open floors. The ladder's gain over B10 is
  in load (no 8 mm wall bites) and in the finish, not in rough time.
- Radial engagement is not measurable at 0.5 mm for these roughs (31–69 %
  blind). The whole-cycle engagement number waits for a finer grid.

## Finish check (Scallop after each rough)

Scallop generated against a 0.2 mm simulation of each rough (the planner's
own pre-simulation); scored at 0.5 mm; accel time from `nc-time`.

| arm | rough accel s | scallop accel s | rough + scallop accel s |
|---|---|---|---|
| A-regen | 1741 | 5598 | 7339 |
| B5 | 871 | 5568 | 6439 |
| B10 | 635 | 5580 | **6215 (−15 %)** |

- The finish time does not change with the rough. The extra stock that
  larger steps leave costs the scallop nothing measurable.
- The finished part is the same: Face + Rough + Scallop removed volume
  89 187.7 / 89 188.4 / 89 188.1 mm³; uncut, truncated and untouched areas
  0.0 on every arm; the finish renders differ by 0 pixels over 20.
- So on rivmap100 the zero-code B10 + By Area already takes the whole gain
  in time. The ladder's value on this model is load (no 8 mm bites at the
  walls), not time.

## Follow-up found during the finish check

- G-MCPMODAL: an MCP `generate_toolpath` on a rest operation (Scallop after
  3D Rough) stops behind the GUI pop-up "Simulation resolution … Use 0.200 mm
  / Cancel". MCP cannot answer it: `cancel_generation` says there is nothing
  to cancel, and `set_ui_view modal:none` does not close it. The generate
  waits until a person clicks. Owner: viz MCP (not this package).

## The scoring instrument (Phase 0, landed 1f3e6f3c + a0447f46)

`rs_cam_cli rough-score <project.toml> --toolpath <i> --resolution 0.5
[--set <i>.<param>=<value> …]` prints one JSON record per toolpath. From
Phase 2 on, every arm uses this one instrument.

Parity rule (operator, 2026-09-24): the GUI, MCP and CLI must give IDENTICAL
numbers for the same project state. The first comparison (MCP B5 871 s
against `rough-score` 902 s) was NOT paired, so it proves nothing either
way:

- the CLI read the on-disk TOML, which has `region_ordering = "global"`;
  the MCP arm B5 used `by_area`;
- `nc-time` replays the G-code with the built-in preset; `rough-score`
  integrates the IR with the project's machine profile.

A paired check (the GUI state saved to a scratch TOML, then `rough-score` on
that file) is queued. With identical inputs, any difference is a defect.

Corrections to PLAN.md section 4 found while it was built:

- On a machine with a `kinematics` block, the published simulation
  `total_runtime_s` is already the accel time. `rough-score` re-accumulates
  the samples, so its `sim_nominal` block is nominal length / feed.
- `sim_nominal.cutting_runtime_s` counts entries and links as cutting;
  `accel_time.cutting_s` holds only clearing/finishing moves.
- The simulation reads the feed before modulation; `accel_time` reads the
  modulated feed that the export uses. Each block names its `feeds_basis`.

First reading (rivmap100 on-disk TOML, index 1, `depth_per_pass=5`, Global): accel total 902 s; entry moves 310 s (34 %); 124 plunge
entries against 15 helix entries although the operation asks for helix;
rough-only removed volume 47 611 mm³. The entry share is a lever of its own
(cause not checked: a helix that falls back to a plunge, or the untagged vertical descents); it is outside this package.

## Parity check: GUI/MCP against `rough-score` (paired)

The GUI state (DPP 5, By Area) was saved to a scratch TOML and scored with
`rough-score` on that file.

- With the same prior-stock resolution, the move count is identical (5373)
  and the GUI toolpath `total_runtime_s` equals `rough-score`
  `accel_time.total_s` to 13 digits (813.711 s). The shared session works.
- G-RESTRES (defect): the same parameters give different geometry
  depending on the resolution of the LAST simulation. The GUI generated the
  rough from a Face stock simulated at 0.2 mm (left by the scallop's
  pre-simulation) and got 7925 moves; at 0.5 mm, 5373. The generated result
  does not record the stock resolution it read, and a simulation at another
  resolution does not mark it stale. This also explains the pre-session
  arm A that did not reproduce (19 624 against 10 442 moves). The CLI takes
  the resolution as an argument; the project file does not carry it.
- G-RESTSTALE (defect): at the restore, the scallop generated 245 584 moves
  against the A rough, which is B5's count: it read a stale stock snapshot
  and nothing flagged it. See also G-FRESHNESSDISAGREE
  (`arch_consolidation_2026-09-09/STATUS.md`).
- Instrument defect (being fixed): `rough-score` printed a second,
  re-accumulated nominal time base under look-alike names
  (`cutting_runtime_s` 760.90 against the GUI's 703.12) and no cut / rapid
  distance. It must print the GUI's published fields, same names, same
  values.
- MCP gives removed volume only for the whole project, not per toolpath.

Instrument fixed (9699f0d5): `rough-score` prints the core
`ToolpathDiagnostic` and `SimulationToolpathCutSummary` as the session
publishes them. On the parity TOML it matches the GUI on moves (5373), cut
and rapid distance, `total_runtime_s` (813.7110364401705) and
`cutting_runtime_s` (703.1247). A test (`rough_score_publishes_the_gui_numbers`)
holds the parity. Follow-up G-MCPCUTROW: the MCP `get_cut_trace`
`toolpath_summaries` row leaves out `average_engagement`,
`total_removed_volume_est_mm3`, `peak_axial_doc_mm` and `per_kinematics`,
which the core struct has.
