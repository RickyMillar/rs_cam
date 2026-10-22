# Phase 0 notes (2026-10-23)

Agent: Phase 0 baseline. No cargo, no crate edits, no commit, no
`gui_restart` or `gui_stop`.

## Pages

- `phase0/rivmap100.html`
- `phase0/corne.html`
- `phase0/wanaka200.html`

Each page is built by `phase0/tools/build_page.py` from the records in
`phase0/data/` (one JSON per column, one readings file and one header file
per fixture). The numbers on a page come from the MCP replies; the readings
are mine.

## GUI

- Binary `/home/ricky/personal_repos/rs_cam/target/release/rs_cam_gui`,
  mtime 1792706351.82 at every check.
- rivmap100 (all three columns) and Corne ContourParallel: GUI pid 946291.
- The lead restarted the GUI while this session was paused. Corne
  AgentSearch and ContourSpiral, the Corne N5 re-run of ContourParallel
  and all wanaka200 columns: GUI pid 84659 (proxy 84652), same binary.
- The operator used the live GUI (scrub, view, clicks) during the Corne
  work. I re-checked `list_toolpaths` before each generate.

## What ran

Per fixture and strategy: load the fixture, save to
`phase0/projects/<fixture>_<strategy>.toml`, disable the ops that are not a
rough or a prerequisite, set `clearing_strategy` on each rough, save,
generate each enabled op in order (`timeout_s` 900), `run_simulation` at the
stored resolution, `collision_check` per rough, `export_gcode` with
`accept_unmodeled_tool_load`, images, `inspect_spans`, `narrate_toolpath`.
`trochoid_cap_mult` is 1.6 (stored) on every rough.

Wall clock per generate (s; the call also makes upstream stock current):

| Fixture | Op | ContourParallel | AgentSearch | ContourSpiral |
|---|---|---|---|---|
| rivmap100 | 3D Rough | 5.2 | 1.8 | 5.2 |
| Corne | 3D Rough 1 | 1.0 | 1.0 | 1.1 |
| Corne | 3D Rough 1 (copy) | 0.9 | 2.2 | 2.1 |
| wanaka200 | 2 Back Rough | 14.4 | 14.1 | 14.9 |
| wanaka200 | 6 3D Rough (front) | 14.0 | 13.2 | 13.9 |

Prerequisites: rivmap100 "Face 5" was current on load (it started
computing when the project opened). wanaka200 "1 Pin Drill" was current on
load. Corne has no prerequisite op.

No generate timed out and none refused. Bar 4 passes in all nine columns.

## Refusals and errors, verbatim

1. wanaka200 fixture load (`load_project`
   `planning/multitool_2026-08-23/wanaka200_mt1.toml`):

   > Failed to load: Failed to load project: The project file uses 1 retired key(s). 'setups[0].toolpaths[2].operation.params.depth': Removed (R-B, F19): the floor of a Drill is operation.params.floor. The old depth counted from the stock top: for the same cut write floor = { from = "stock_top", depth = <this value> } in the same table, or floor = { from = "each_hole", depth = 0.0 } to drill each hole of a STEP model to its own floor. Delete the key.

2. After I deleted the line `depth = 12.0` in the copy
   `phase0/projects/wanaka200_base.toml`:

   > Failed to load: Failed to load project: TOML parse error: TOML parse error at line 431, column 1
   >     |
   > 431 | [setups.toolpaths.operation.params]
   >     | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   > missing field `floor`

   So a delete alone does not load. I added the key that the first refusal
   prescribes for the same cut: `floor = { from = "stock_top", depth = 12.0 }`.
   The copy then loads. Diff against the fixture: line 432
   `depth = 12.0` became `floor = { from = "stock_top", depth = 12.0 }`.
   No other retired key was reported. The drill op is disabled in every
   column, so this edit does not change a rough.

3. `export_gcode` to `phase0/gcode/` first failed: "Export failed: No such
   file or directory (os error 2)". The folder did not exist; I made it.

4. wanaka200 ContourParallel, front rough generate, first call:

   > a generation plan is already running, or one is waiting on the operator to confirm a simulation resolution. Poll `generation_status` for its step, or call `cancel_generation` and try again.

   `generation_status` then showed an idle lane and no plan; the second call
   ran. The back-rough plan had a simulation step still finishing. For the
   later columns I polled `generation_status` until `plan` was null.

5. Corne ContourParallel N5 re-run: `run_simulation` returned
   `{"error":"Simulation cancelled"}` once. I did not cancel it; the
   operator was using the GUI at that time. The second call ran.

## Hard-bar instruments

- Bar 1: `run_simulation` `rapid_collision_count` (project and per op) and
  `collision_check` `collision_count` per rough. The per-op
  `collision_count` in `run_simulation` is null; the holder value comes
  from `collision_check`.
- Bar 2: NOT MEASURED everywhere. `untouched_material_mm2`,
  `reached_uncut_estimate_mm2`, `truncated_core_mm2` and
  `unmachined_band_area_mm2` are null on every adaptive3d row; the
  narration says "Standing material: not measured".
  `metrology/union_coverage.rs` is behind `test-support` and no MCP tool
  reaches it.
- Bar 3: NOT MEASURED everywhere. `compute/catalog/registry.rs`
  `REG_ADAPTIVE3D` has `model_gouge: ModelGougeRule::Exempt`, so
  `safety.cuts_into_model` never runs for a 3D Adaptive. The cut-trace
  issue kinds are only `air_cut` and `low_engagement`
  (`stock/simulation_cut.rs` `SimulationCutIssueKind`). I read the
  stock-side ids `project.crosses_standing_material` and
  `project.link_cuts_stock` from the `run_simulation` triage and
  `get_toolpath_diagnostics`; their counts are on each page.

## Hard-bar table

Bar 3 has two parts. 3a: mesh gouge on the emitted moves (no instrument
for 3D Adaptive). 3b: the stock-side ids `project.crosses_standing_material`
and `project.link_cuts_stock`. "0 reported" means neither id appears in
the triage or in `get_toolpath_diagnostics`.

| Fixture | Strategy | Bar 1 | Bar 2 | Bar 3a | Bar 3b | Bar 4 |
|---|---|---|---|---|---|---|
| rivmap100 | ContourParallel | PASS (0 / 0) | NOT MEASURED | NOT MEASURED | 0 reported | PASS |
| rivmap100 | AgentSearch | PASS (0 / 0) | NOT MEASURED | NOT MEASURED | 0 reported | PASS |
| rivmap100 | ContourSpiral | PASS (0 / 0) | NOT MEASURED | NOT MEASURED | 0 reported | PASS |
| Corne | ContourParallel | PASS (0 / 0) | NOT MEASURED | NOT MEASURED | FAIL: link_cuts_stock 1 (6.50 mm) + 3 (3.38 mm) | PASS |
| Corne | AgentSearch | PASS (0 / 0) | NOT MEASURED | NOT MEASURED | FAIL: crosses_standing_material 4 (peak 15.0 mm), copy op | PASS |
| Corne | ContourSpiral | PASS (0 / 0) | NOT MEASURED | NOT MEASURED | 0 reported | PASS |
| wanaka200 | ContourParallel | PASS (0 / 0) | NOT MEASURED | NOT MEASURED | FAIL: link_cuts_stock CRITICAL, 20 samples on 12 links, worst 11.26 mm | PASS |
| wanaka200 | AgentSearch | PASS (0 / 0) | NOT MEASURED | NOT MEASURED | 0 reported | PASS |
| wanaka200 | ContourSpiral | PASS (0 / 0) | NOT MEASURED | NOT MEASURED | 0 reported | PASS |

## How the readings were made

The first `Read` calls on the rivmap100 images and on four Corne
ContourParallel images returned no image (a request limit). I first
drafted those readings from the .nc, the spans and the narration. Later I
looked at every one of those images and checked each reading. Three
readings changed:

- rivmap100 AgentSearch V3: the spiral sets are at the upper right and
  the upper left, not the lower left.
- rivmap100 ContourParallel N2: the keep-down links fan out from the lower
  diagonal, not from one corner point.
- rivmap100 AgentSearch and ContourSpiral, part question: the stock
  renders show the narrow valleys left higher than ContourParallel (N9).

## N5 standing spurs (operator observation)

Operator words (2026-10-23, watching the GUI on the Corne case): "as it's
milling down it leaves tall spurs of material; these could snap and cause
issues; prominent ones 7 min in".

Method:

- `tools/move_time.py` finds the move at t = 420 s from the G-code feed
  times. Fitted to the Corne AgentSearch cut trace: scale 1.016, maximum
  error 0.8 s over 3 000 trace points.
- The GUI simulation is scrubbed there (`sim_jump_to_move`) and captured
  (`screenshot_gui`).
- `tools/spurs.py` replays the exported .nc on a 0.2 mm heightmap (flat
  end mill stamps). A spur is material more than 1.5 mm tall and less
  than 2 mm wide (grey opening with a 1 mm disk). Free-standing means no
  wide material within 2 mm.

The operator watched the AgentSearch column. The GUI held that project
and showed a 0.10 mm re-simulation at that time.

Result:

- Corne AgentSearch: present. At move 3577 (7:32 on the GUI clock) 6
  free-standing spurs stand 13 mm tall (setup Z 17 down to Z 4), 0.3-1.0 mm
  wide. The main one is a wedge at (104.5, 86.0), 5 mm long, where the
  spiral area meets a straight pass at y 86.5. At every level the nearest
  pass centres are 3.2-3.6 mm from the spur with a 3.0 mm tool radius, so
  a 0.2-0.6 mm sliver stays at the same XY on each level and the slivers
  stack. Still standing at the end: 39 free-standing, the tallest 16.5 mm
  (two of them 1.0-1.2 mm wide at y 86.6).
- Corne ContourParallel: absent (0 free-standing at 7 min and at the end).
- Corne ContourSpiral: absent at 7 min; 2 hole cores 3 mm tall at the end.
- rivmap100: ContourParallel none; AgentSearch and ContourSpiral one-level
  fins (2 mm) mid-rough, removed by the end.
- wanaka200: AgentSearch present (19 free-standing on the back, up to
  15.6 mm at the edge band; 31 small ones on the front). ContourParallel
  and ContourSpiral: only nubs under 3.4 mm.

## Surprises

- Entry spans nest. `inspect_spans kind:"entry"` returns a short span
  inside each longer one (rivmap100: exactly half of all Entry spans). A
  plain count doubles the entries. The pages count top-level spans and
  show the nested ones apart.
- Entry time dominates rivmap100: 2 364 of 3 263 s (ContourParallel),
  1 123 of 1 484 s (AgentSearch) and 6 100 of 7 085 s (ContourSpiral).
  The plan quoted 36-40 % for 2026-09-25.
- AgentSearch and ContourSpiral do not cut the last rivmap100 level
  (Z 0.5). ContourParallel cuts it, but the narration says that plane holds
  no material. The same "cuts but no material" note shows on the last level
  of the wanaka200 roughs.
- Corne: AgentSearch and ContourSpiral leave the band outside the case
  wall standing (planner gate post 0.27-0.31 at the last level against
  0.000 for ContourParallel). The stock render shows the band high with
  round holes punched into it.
- wanaka200 ContourParallel back rough: 1 726 G0 runs, most of them 0.5 mm
  hops over the level, and a CRITICAL `project.link_cuts_stock` (a fed link
  removes 11.26 mm).
- The Corne ContourParallel cycle time is not reproducible: identical moves
  gave 2 022.1 s, then 1 563.6 s on a re-run (the feed modulation reads the
  previous simulation).
- The simulation cell is too coarse for the engagement metrics on most
  columns ("cell_too_coarse_for_tip_contact"). The air-cut numbers are
  withheld or degraded; collisions, removal and axial DOC stay valid.
- The wanaka200 fixture does not load at HEAD (retired drill key, above).
- The export of wanaka200 is one file for both setups; each setup zeroes Z
  at its own stock top.
- Peak axial DOC is 2-4 times the commanded depth per pass on several
  columns (Corne up to 16.5 mm on 6.0; wanaka200 ContourParallel back
  17.1 mm on 5.46). The narration says to check upstream coverage first.

## Tools (phase0/tools/)

- `ingest.py`: copies a saved MCP reply to clean JSON.
- `links_entries.py`: links view and entries view from the .nc. Move i of
  an op is section line i + 1. Check: the G0-run count equals the
  narration's retract count in all 15 op sections.
- `zoom.py`: zoom crop of the moves and entries in one XY box.
- `move_time.py`: move index at a cycle time.
- `spurs.py`: the N5 heightmap replay.
- `build_page.py`: the pages.
