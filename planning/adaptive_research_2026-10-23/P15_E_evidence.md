# P1.5 evidence agent E: the two new instruments on the Phase 0 projects

- Date: 2026-10-23, from 15:18 NZDT.
- GUI: branch `adaptive-instruments` 405cbd23. The binary is
  `/home/ricky/personal_repos/rs_cam/target/release/rs_cam_gui`,
  mtime_unix 1792721740.00. The GUI pid is 1275067 (proxy generation 3,
  started 1792721867).
- The first GUI process (pid 1260840) exited before this run started:
  code 1 at 1792721834.84 (15:17:14). The last stderr line was
  `Error: AppCreation("MCP-mode event loop exited before the embedded server completed")`.
  The coordinator restarted it. I did not call `gui_restart`.
- Projects: `phase0/projects/corne_contour_parallel.toml`,
  `corne_agent_search.toml` and `corne_contour_spiral.toml` (and wanaka200 if
  time allows). Every project was loaded with `discard_unsaved`. No
  project was saved.
- I ran no cargo, edited no crate code and made no commit.

## Finding 0: the coverage instrument cannot read in the GUI (root cause)

`triage.coverage` is `[]` (an empty list) in the `run_simulation` and
`get_diagnostics` replies. `narrate_toolpath` has no "coverage:" sentence.
`get_toolpath_diagnostics` has no `project.rough_leaves_islands` finding.
The empty list is not a measured zero and not an abstention. The
instrument published no reading at all.

Cause, from the code at 405cbd23:

- `crates/rs_cam_viz/src/controller/events/compute.rs`,
  `core_simulation_from_lane` (about line 2597): the GUI gives the session
  a simulation copy with `checkpoints: Vec::new()` and an empty mesh (W2-E
  memory work). Its doc comment says "No session reader reads ... the
  checkpoints". G-ROUGHCOV added such a reader, so that statement is now
  false.
- `crates/rs_cam_core/src/session/compute/simulation.rs`,
  `rough_coverage_at`: the line
  `.and_then(|bi| sim.checkpoints.iter().find(|c| c.boundary_index == bi))?;`
  returns `None` for every op. `rough_coverage_readings` then drops every
  op through `filter_map`, so the list is empty. The same function
  supplies `narrate_toolpath` and `get_toolpath_diagnostics`, so all
  three MCP surfaces are empty.
- I read that the core `AdoptSimulation` handler does not drop the
  checkpoints, and that the viz adopt does. I did not read the CLI path.
  If the CLI and the core tests keep the checkpoints, the sentries can be
  green while the GUI reads nothing.
- The second guard, `stored_run_is`, is very likely true in the GUI.
  That was inferred (a subagent read), not read in full: the builder of
  the view boundaries was not read.
- Fix options for the lead:
  - Keep the checkpoint `mesh_stock` `Arc`s in the session copy. The W2-E
    memory reasoning then needs a new decision.
  - Or compute the reading on the viz side from the view's checkpoints.
- `crates/rs_cam_viz/src/controller/tests/session_keeps_only_what_it_reads_w2e.rs`
  probably asserts the empty checkpoints.
- A GUI-path sentry must assert a non-empty `triage.coverage` after a GUI
  adopt, end to end. That also covers the inferred `stored_run_is` guard.

Thus every coverage reading below is "not published". I still record by
eye what the coverage instrument must read, so the re-run after the fix
has a reference.

## Project 1: corne_contour_parallel (the zero control)

Ops:
- index 0 "3D Rough 1" (id 1): 6 mm 2F flat, depth per pass 6.0,
  stepover 1.2, leave 0.5, tolerance 0.1, boundary model_silhouette
  offset 6.0.
- index 1 "3D Rough 1 (copy)" (id 3): 3 mm End Mill, depth per pass 3.0,
  stepover 0.6, leave 0.5, from remaining stock.
- Waterline 3 is disabled.

Timings: generate op 0 took about 1.1 s (8006 moves) and op 1 about 2.9 s
(5347 moves). `run_simulation`, both narrations and both diagnostics calls
each took under 10 s. I timed one narration (op 0): about 2.4 s,
including the round trip. The other calls ran in parallel blocks and are
not timed one by one.

Simulation: stored resolution 0.3 mm (mode auto, not set by this call).
Runtime 2022.1 s. Collisions 0, rapid collisions 0. Triage safety `[]`.
Triage actions: `project.link_cuts_stock` on both ops (move 2320 removes
6.50 mm at (21.4, −107.4, 11.0), limit 6.30; move 3153 removes 3.38 mm at
(115.7, −42.4, 0.5), limit 3.30). These are the Phase 0 G-LINKPLOUGH
readings. `triage.coverage`: `[]`.

### Coverage (G-ROUGHCOV)

- op 0: not published (Finding 0). No narration sentence.
- op 1: not published (Finding 0). No narration sentence.
- Eye, from `phase0/img/corne_contour_parallel_p15_stock.png`: every hex
  hole is cut clean and the band outside the wall is cleared. No spur
  shows. Red full-height stock stays only outside the boundary, at the
  top right and the bottom right of the top view. That is outside the
  population (silhouette + 6 mm). A correct instrument must therefore
  read zero islands here, or only inside-corner residue. I cannot test
  the corner-residue risk until the reading is published.

### Gouge (G-ROUGHGOUGE, `safety.cuts_into_model`, Blocking)

The finding is from generation, in `get_toolpath_diagnostics`. It is not
in the simulation triage.

- op 0: "3 of 30808 sampled cutting positions go below the model surface
  plus the stock to leave (less the chord tolerance of the op). The
  deepest is 0.064 mm at X 109.279 Y -4.017: the tool tip is at Z 17.000,
  and the tip may go down to Z 17.064 there." Move 3835, first level
  (z = 17).
  - Eye and mesh: the point is in the floor band outside the case wall,
    with no model surface under the tip. The rim of the wall is a 45°
    slope. A numpy read of the STL (flat disk, 7 radii x 48 angles) gives
    the highest model Z in the disk: 16.845 at the probe radius r 2.98
    (R − 0.02), and 16.664 at r 2.8.
  - The chord slack in effect: limit − CL − leave = 17.064 − 16.845 − 0.5
    = −0.28, so the slack is about 0.28 to 0.3 mm. That is not the op
    `tolerance` 0.1. It is the segment-merge tolerance (P15_B: the slack is
    max(tolerance, merge)). Op 1 gives the same: 3.700 − 3.5 − 0.5 = −0.3.
  - So the tip at Z 17.0 is about 0.155 mm above the model at the probe
    radius. It is 0.345 mm inside the 0.5 mm leave, and 0.064 mm past the
    0.28 mm slack. It is not inside the model.
  - Verdict: a true reading of the rule as written (the first level at
    Z 17 cuts into the leave on the rim chamfer). It is not a cut into
    the model.
- op 1: "3 of 5060 ... The deepest is 3.200 mm at X 30.380 Y -38.583: the
  tool tip is at Z 0.500, and the tip may go down to Z 3.700 there."
  Move 4453, last level (z = 0.5).
  - Eye: the point is inside a hex hole (marked G1 on
    `phase0/img/corne_contour_parallel_p15_gouge.png`, crop `_crop_G1`. This
    image marks gouge points, not islands).
  - Mesh: there is no surface under the tip (through hole). A surface at
    Z 3.5 enters the disk at the probe radius r 1.48 (R − 0.02). It is
    absent at r 1.45. So the 1.5 mm tool side overlaps the hole wall by
    about 0.02 to 0.05 mm radially.
  - P15_B predicted this mechanism for this column: near a vertical
    wall, a large `deepest_mm` with `at` within R of a wall is a true
    radial micro-gouge, not an instrument fault.
  - Verdict: the contact is real but tiny and radial. The Z-only rule
    reports a radial graze on a vertical wall as a 3.2 mm depth. That is
    the full wall height above the tip, not the depth into the model. The
    number misleads a reader. The rough has no radial leave (L2), so the
    rule probes at R − 0.02. Any wall that the side of the tool touches
    by more than 0.02 mm becomes a "depth" equal to the wall height.
- Export: `export_gcode` to `phase0/gcode/corne_contour_parallel_p15.nc`
  REFUSED. Text verbatim:

  > Export failed: Export failed: G-code export refused: a toolpath cuts into the model:
  > Toolpath '3D Rough 1': The toolpath cuts into the stock to leave or into the model: 3 of 30808 sampled cutting positions go below the model surface plus the stock to leave (less the chord tolerance of the op). The deepest is 0.064 mm at X 109.279 Y -4.017: the tool tip is at Z 17.000, and the tip may go down to Z 17.064 there. Look at the move named here: if the tool enters a wall of the model, report it as a planner defect; do not cut this toolpath. [Measured at generation on the emitted moves against the mesh of the toolpath model, tolerance 0.02 mm in Z; export refuses while this stands.]
  > Toolpath '3D Rough 1 (copy)': The toolpath cuts into the stock to leave or into the model: 3 of 5060 sampled cutting positions go below the model surface plus the stock to leave (less the chord tolerance of the op). The deepest is 3.200 mm at X 30.380 Y -38.583: the tool tip is at Z 0.500, and the tip may go down to Z 3.700 there. Look at the move named here: if the tool enters a wall of the model, report it as a planner defect; do not cut this toolpath. [Measured at generation on the emitted moves against the mesh of the toolpath model, tolerance 0.02 mm in Z; export refuses while this stands.]

  No override was passed.
- The zero control expected 0 gouge findings. It reads 2 (3 + 3
  samples). Both are real contacts under the rule as written: one into
  the leave on the rim chamfer, one a radial wall graze. Neither is a cut
  into the model deeper than about 0.05 mm. The default ContourParallel
  rough is now unexportable on Corne.
- The narration has no gouge sentence. That is by design (B adds none).
- G-WALLDIP (P15_B's best place for a real positive): the deepest sample
  is not on the outer wall. I did not confirm that the dips are present
  in this build's copy op. The finding reports only the deepest sample,
  so this is not a negative result for G-WALLDIP.

### Verdicts, project 1

- Coverage: publishes nothing (Finding 0). This is not a zero and not an
  abstention. It cannot be compared with the eye.
- Gouge: the positions are true on both ops. The op 0 reading is true:
  the planner breaks its own drape promise by 0.064 mm beyond the slack,
  inside the leave and not in the model. The op 1 position is true, but
  the magnitude is a false positive in effect: a 0.02–0.05 mm radial wall
  graze reads as 3.2 mm (the wall height above the tip), because the rule
  is Z-only. P15_B names this limit. The lead decides whether either is a
  planner defect, and whether a sub-0.05 mm graze must refuse the export.

## Project 2: corne_agent_search (the positive control)

Ops: the same two roughs as project 1, with `clearing_strategy`
agent_search. Index 0 "3D Rough 1" (id 1, 6 mm) and index 1 "3D Rough 1
(copy)" (id 3, 3 mm, from remaining stock).

Timings (round trip included): generate op 0 about 4.0 s (4168 moves),
op 1 about 5.5 s (6060 moves). No call took more than 10 s.

Simulation: stored resolution 0.3 mm (auto). Runtime 2112.4 s.
Collisions 0, rapid collisions 0. Triage safety `[]`. Triage actions:
`project.crosses_standing_material` on op 1, 4 occurrences, "2.6% of
cutting samples remove more than the commanded 3.00 mm depth per pass by
over 0.300 mm, peaking at 15.00 mm". The worst is move 1401 at
(105.84, −24.26, 2.0), the Phase 0 reading. `triage.coverage`: `[]`.

Narration: op 0 peak axial DOC 16.50 mm at move 4162, (88.6, −14.2),
z 0.5. Op 1 peak axial DOC 15.00 mm at move 1466, (4.4, −77.2). Neither
narration has a "coverage:" sentence.

### Coverage

- op 0 and op 1: not published (Finding 0).
- Eye (`phase0/img/corne_agent_search_p15_stock.png`; 3× top crop in my
  scratchpad). This is what a working instrument must read here:
  - Hole cores (G-HOLECORE): about 37 of about 70 visible hex holes hold a
    central core, by my count of the 3× crop. That is about half, which is
    more than Phase 0's "about a third". The count is by eye and is
    approximate. The cores are in all columns of holes except the middle
    one (x ≈ 75 mm in the stock frame).
  - Band (G-OUTERBAND, N7): the band outside the case wall stands
    (olive, mid height) all round, with round holes punched along it.
    Seven or eight of these holes lie along the top edge, and more lie
    along the bottom-right and bottom-left edges.
  - Two green strips stand at mid height inside the wall, along the top
    (stock frame x ≈ 25–55 and 70–90 mm, y ≈ 100 mm) and along the right
    side (x ≈ 140 mm, y ≈ 70–95 mm). Phase 0 did not name them.
  - Spur (G-SPUR) at the Phase 0 position, G-code frame (104.5, 86.0) =
    world (104.81, −23.32): not visible in the top view. The spur is
    0.3–1.0 mm wide, which is one to three pixels at 0.319 mm/px and one
    to three cells at 0.3 mm. Phase 0 measured it with `spurs.py` on a
    0.2 mm heightmap after a 0.1 mm re-simulation. The
    `crosses_standing_material` worst sample (move 1401) is 1.4 mm from
    the spur position. This fits a later pass that cuts the spur.
  - I cannot answer the brief's spur question ("does the tallest
    island's height and width match 13–16.5 mm tall, 0.3–1.0 mm wide?"),
    because the instrument published no island.
- Marked image: `phase0/img/corne_agent_search_p15_gouge.png` (gouge
  points G0 and G1, the spur and move 1401; crops `_crop_*`).

### Gouge

- op 0: Blocking `safety.cuts_into_model`, "3 of 13645 sampled cutting
  positions ... The deepest is 0.064 mm at X 109.279 Y -4.017: the tool
  tip is at Z 17.000, and the tip may go down to Z 17.064 there." Move
  2000. This is the same point and the same depth as on ContourParallel:
  the first level at Z 17 cuts into the leave on the rim chamfer. Both
  strategies share the level plan, so this does not depend on the
  strategy.
- op 1: Blocking, "3 of 6531 ... The deepest is 3.200 mm at X 30.380
  Y -38.583: the tool tip is at Z 0.500, and the tip may go down to Z
  3.700 there." Move 5158. This is the same hex-hole wall graze as on
  ContourParallel (radial overlap about 0.02–0.05 mm; the reading is the
  wall height).
- Move 1401: no gouge finding there, as P15_B predicted. The guard did not
  confuse a stock reading with a model gouge.
- Export to `phase0/gcode/corne_agent_search_p15.nc` REFUSED. Text
  verbatim:

  > Export failed: Export failed: G-code export refused: a toolpath cuts into the model:
  > Toolpath '3D Rough 1': The toolpath cuts into the stock to leave or into the model: 3 of 13645 sampled cutting positions go below the model surface plus the stock to leave (less the chord tolerance of the op). The deepest is 0.064 mm at X 109.279 Y -4.017: the tool tip is at Z 17.000, and the tip may go down to Z 17.064 there. Look at the move named here: if the tool enters a wall of the model, report it as a planner defect; do not cut this toolpath. [Measured at generation on the emitted moves against the mesh of the toolpath model, tolerance 0.02 mm in Z; export refuses while this stands.]
  > Toolpath '3D Rough 1 (copy)': The toolpath cuts into the stock to leave or into the model: 3 of 6531 sampled cutting positions go below the model surface plus the stock to leave (less the chord tolerance of the op). The deepest is 3.200 mm at X 30.380 Y -38.583: the tool tip is at Z 0.500, and the tip may go down to Z 3.700 there. Look at the move named here: if the tool enters a wall of the model, report it as a planner defect; do not cut this toolpath. [Measured at generation on the emitted moves against the mesh of the toolpath model, tolerance 0.02 mm in Z; export refuses while this stands.]

### Verdicts, project 2

- Coverage: publishes nothing (Finding 0). It misses every island the
  eye sees: about 37 hole cores, the band with punched holes, the two
  green strips, and (by Phase 0) the spur.
- Gouge: reads the same two points as on ContourParallel. It has no
  false positive at move 1401. The readings do not depend on the
  strategy.

## Project 3: corne_contour_spiral

Ops: the same two roughs, `clearing_strategy` contour_spiral.

Timings (round trip included): generate op 0 about 4.9 s (14839 moves),
op 1 about 5.5 s (5376 moves). No call took more than 10 s.

Simulation: stored resolution 0.3 mm (auto). Runtime 2015.8 s.
Collisions 0, rapid collisions 0. Triage safety `[]`. The only action is
`project.air_cut_high` on op 0 (46 %). There is no `link_cuts_stock` and
no `crosses_standing_material`. `triage.coverage`: `[]`.

Narration: op 0 peak axial DOC 16.50 mm at move 14833, (88.6, −14.2),
z 0.5. That is the same column as AgentSearch op 0. Op 1 peak 13.50 mm
at move 3119, (55.1, −12.6). Neither narration has a "coverage:"
sentence.

### Coverage

- op 0 and op 1: not published (Finding 0).
- Eye (`phase0/img/corne_contour_spiral_p15_stock.png`; 3× top crop in
  my scratchpad). This is what a working instrument must read:
  - Hole cores: exactly 2, both in the bottom-left holes, at about world
    (19.4, −85.0) and (54.0, −85.0) (from the crop-to-frame map,
    ±0.5 mm). This matches Phase 0 ("2 hole cores 3 mm tall at the end").
  - Band: the band outside the case wall stands all round (olive), with
    about 8 round holes punched in it. This matches N7.
  - A green mid-height strip stands inside the right wall (as on
    AgentSearch), with faint ones along the top and the left inner wall.
  - No spur is visible.

### Gouge

- op 0: Blocking, "3 of 42548 ... The deepest is 0.064 mm at X 109.279
  Y -4.017 ... tip ... Z 17.000 ... Z 17.064". Move 7411. The rim
  chamfer again.
- op 1: Blocking, "3 of 4980 ... The deepest is 3.200 mm at X 30.380
  Y -38.583 ... tip ... Z 0.500 ... Z 3.700". Move 4482. The hex-hole
  wall graze again.
- Export to `phase0/gcode/corne_contour_spiral_p15.nc` REFUSED. Text
  verbatim:

  > Export failed: Export failed: G-code export refused: a toolpath cuts into the model:
  > Toolpath '3D Rough 1': The toolpath cuts into the stock to leave or into the model: 3 of 42548 sampled cutting positions go below the model surface plus the stock to leave (less the chord tolerance of the op). The deepest is 0.064 mm at X 109.279 Y -4.017: the tool tip is at Z 17.000, and the tip may go down to Z 17.064 there. Look at the move named here: if the tool enters a wall of the model, report it as a planner defect; do not cut this toolpath. [Measured at generation on the emitted moves against the mesh of the toolpath model, tolerance 0.02 mm in Z; export refuses while this stands.]
  > Toolpath '3D Rough 1 (copy)': The toolpath cuts into the stock to leave or into the model: 3 of 4980 sampled cutting positions go below the model surface plus the stock to leave (less the chord tolerance of the op). The deepest is 3.200 mm at X 30.380 Y -38.583: the tool tip is at Z 0.500, and the tip may go down to Z 3.700 there. Look at the move named here: if the tool enters a wall of the model, report it as a planner defect; do not cut this toolpath. [Measured at generation on the emitted moves against the mesh of the toolpath model, tolerance 0.02 mm in Z; export refuses while this stands.]

### Verdicts, project 3

- Coverage: publishes nothing (Finding 0). It misses the 2 hole cores
  and the band.
- Gouge: the same two points as on the other two strategies.

## Cross-column observation on the gouge rule

On all three strategies and both ops, the deepest reading is at the same
XY with the same depth, and each finding has 3 samples. Op 0 is at
(109.279, −4.017), Z 17 (0.064 mm). Op 1 is at (30.380, −38.583), Z 0.5
(3.200 mm). The move index changes with the strategy (3835 / 2000 / 7411
and 4453 / 5158 / 4482).

- Op 0: the first level at Z 17 is shared by the strategies, so a contact
  that is the same at Z 17 is plausible.
- Op 1: the three op 1 paths differ, but they all reach the same point of
  the same hole at Z 0.5. This hints that a shared step emits the move,
  for example the last-level floor or entry handling. That is a
  question for the lead; I did not trace it.
- The rule reads the same as the eye in position. On op 1 the depth
  number is the wall height, not the overlap (see project 1). The rule
  gave no false positive at the AgentSearch stock reading (move 1401).
- The rule makes all six Corne roughs unexportable. The deepest real
  intrusion into the model is about 0.02–0.05 mm (radial, op 1). Op 0 is
  0.064 mm past the slack, inside the leave.

## Project 4: wanaka200_contour_parallel (G-LINKPLOUGH)

Enabled ops: index 0 "1 Pin Drill", index 1 "2 Back Rough" (Setup 1,
face_up Bottom), index 5 "6 3D Rough (front)" (Setup 2, face_up Top). The
back rough has leave 4.0, depth per pass 5.46, stepover 2.0, tolerance
0.1, boundary model_silhouette inside, offset 0.

Timings (round trip included): Pin Drill about 4.0 s. Back rough about
20.4 s (14180 moves). Front rough about 19.2 s (14049 moves).
`run_simulation` about 4.0 s. Back rough narration about 3.7 s.

Simulation: stored resolution 0.5 mm (auto). Runtime 8295.9 s.
Collisions 0, rapid collisions 0. Project verdict "ok". Triage safety
`[]`. Triage actions:
- `project.link_cuts_stock`, severity CRITICAL, on "2 Back Rough": "link
  move 5147 removes 11.26 mm at (216.9, 55.0, 8.63). 20 of 44782 link
  samples on 12 link moves remove more than 5.96 mm". This is the Phase 0
  G-LINKPLOUGH reading, unchanged.
- `project.alignment_pins_unkeyed` (project).

`triage.coverage`: `[]`.

### Gouge

- "2 Back Rough": Blocking, "2 of 66099 sampled cutting positions ... The
  deepest is 0.098 mm at X 137.973 Y 46.111: the tool tip is at Z 12.602,
  and the tip may go down to Z 12.700 there." Move 12699.
  - With leave 4.0, the tip is about 3.6 mm above the model (if the slack
    is near 0.3 mm as on Corne). The reading is 0.098 mm past the slack,
    inside the leave.
  - I did not check this against the mesh. The setup is flipped
    (face_up Bottom): the setup-local Z (levels 19.54 to 4.0) is not the
    STL frame (Z −2.8 to 7.0), and I did not apply the setup transform.
- Does the rule fire on link move 5147? No. The deepest sample is at move
  12699, and the finding has 2 samples in all. Move 5147 removes 11.26 mm
  of standing stock (at Z 8.63), but it stays above model + leave − slack.
  This is the design: the gouge rule is a model-side check. G-LINKPLOUGH
  is a stock-side defect, and `link_cuts_stock` holds it. The finding
  gives only the deepest sample, so I cannot fully rule out a shallower
  sample on move 5147. It is not likely, because 5147 is at Z 8.63 on the
  third level.
- "6 3D Rough (front)": Blocking, "15 of 51702 ... The deepest is 0.180 mm
  at X 10.500 Y 87.320: the tool tip is at Z 3.300, and the tip may go
  down to Z 3.480 there." Move 8887. I did not check it against the mesh.
- Export to `phase0/gcode/wanaka200_contour_parallel_p15.nc` REFUSED. Text
  verbatim:

  > Export failed: Export failed: G-code export refused: a toolpath cuts into the model:
  > Toolpath '2 Back Rough': The toolpath cuts into the stock to leave or into the model: 2 of 66099 sampled cutting positions go below the model surface plus the stock to leave (less the chord tolerance of the op). The deepest is 0.098 mm at X 137.973 Y 46.111: the tool tip is at Z 12.602, and the tip may go down to Z 12.700 there. Look at the move named here: if the tool enters a wall of the model, report it as a planner defect; do not cut this toolpath. [Measured at generation on the emitted moves against the mesh of the toolpath model, tolerance 0.02 mm in Z; export refuses while this stands.]
  > Toolpath '6 3D Rough (front)': The toolpath cuts into the stock to leave or into the model: 15 of 51702 sampled cutting positions go below the model surface plus the stock to leave (less the chord tolerance of the op). The deepest is 0.180 mm at X 10.500 Y 87.320: the tool tip is at Z 3.300, and the tip may go down to Z 3.480 there. Look at the move named here: if the tool enters a wall of the model, report it as a planner defect; do not cut this toolpath. [Measured at generation on the emitted moves against the mesh of the toolpath model, tolerance 0.02 mm in Z; export refuses while this stands.]

### Coverage

- Not published (Finding 0). The back rough is in a flipped setup, so a
  working instrument would abstain there with `setup_not_identity` (P15_A).
  The brief's question (is the planner's leave 4.0 respected, and are the
  islands real) cannot be answered on this binary.
- I took no stock screenshot of wanaka200, because no island position
  exists to compare.

## Slow calls (over 10 s)

- wanaka200 generate "2 Back Rough": about 20.4 s.
- wanaka200 generate "6 3D Rough (front)": about 19.2 s.

The gouge guard runs inside generation (P15_B estimates its cost but did
not measure it). These two times include the guard, but I cannot
separate its share. Phase 0 (`phase0/wanaka200.html`, ContourParallel
column, Bar 4) measured 14.4 s (back) and 14.0 s (front) without the
guard. So the guard adds about 6 s per wanaka200 rough, if the round trip
is small. On Corne the generates stayed under 6 s.
Every narration, diagnostics, simulation and export call took less
than 10 s. P15_A's warning about the slow walk does not apply here,
because the walk never runs in the GUI (Finding 0).

## Instrument verdicts

| Instrument | Corne CP (zero) | Corne AgentSearch (positive) | Corne ContourSpiral | wanaka200 CP |
|---|---|---|---|---|
| Coverage (`triage.coverage`, `rough_leaves_islands`, narration) | publishes nothing (Finding 0) | misses everything: about 37 hole cores, the band, the strips, the spur (Finding 0) | misses 2 hole cores and the band (Finding 0) | publishes nothing (Finding 0) |
| Gouge (`safety.cuts_into_model`) | reads true contacts; op 1 depth overstated (radial graze read as 3.2 mm) | the same two points; no false positive at move 1401 | the same two points | fires on the cutting moves (0.098, 0.180 mm); does not fire on link 5147 (by design, stock-side) |

- Coverage: **misses everything**. The cause is one line in the viz
  adopt (Finding 0), not the measure. The core measure is not tested by
  this run. It must be run again after the fix.
- Gouge: **reads what the eye saw** for position, on every column. On
  Corne op 1 the depth is a **false positive in magnitude**: a
  0.02–0.05 mm radial wall graze reports as 3.2 mm, the wall height
  (P15_B named this limit). On Corne op 0 it reports the planner's first
  level 0.064 mm past its own slack, inside the leave. As a result, all
  eight roughs in this run (six Corne, two wanaka200) refuse export. The
  lead decides whether such sub-0.1 mm readings must block.

## Open questions for the lead

1. Finding 0: keep the checkpoints in the session copy, or compute the
   reading on the viz side? Then rebuild, and send me the new pid and
   mtime. I will run all three Corne columns again for coverage.
2. Should the Z-only gouge rule report a radial overlap as a depth equal
   to the wall height? A reader takes "3.200 mm" as a deep gouge.
3. `triage.safety` is `[]` in every `run_simulation` reply while a
   Blocking `safety.cuts_into_model` stands on the same ops. Is that
   intended (the finding is from generation, not the simulation)? I did
   not call `get_project_diagnostics` to check the project-level list.
4. The chord slack in effect is about 0.3 mm (segment merge), not the op
   tolerance 0.1. Is a 0.3 mm slack inside a 0.5 mm leave intended?
5. The op 1 graze is at the same XY on all three strategies. Does a shared
   step (last-level floor, entry) emit it?
6. The CLI keeps the checkpoints, so a CLI run may publish
   `triage.coverage` today, and it would test the core measure before the
   GUI fix. I did not run it: it is outside the surface that my brief
   names.

## N11 / G-BACKTRAVEL: the second half of "2 Back Rough" (wanaka200 ContourParallel)

Operator observation (15:45, live GUI): "second half of Back Rough 2 is
doing huge travel across the board where it could do one waterline-like
sweep."

Sources:
- `inspect_spans` on the generated op (this GUI, 405cbd23).
- The Phase 0 export `phase0/gcode/wanaka200_contour_parallel.nc`. The
  export of this run was refused (gouge finding), so I used the Phase 0
  file. It is the same toolpath: 14180 moves in both, and move 5147
  ends at (217.0, 55.0) with G-code Z −16.38 (setup Z 8.62, Z0 at the
  stock top 25 mm), the `link_cuts_stock` point of this run. Only the
  modulated feeds differ.
- Move i of the op is G-code motion line i + 1 of the section (lines
  80–14261, after `M3 S20000`; the `links_entries.py` rule).
- Scripts: `nc_moves.py`, `backtravel.py`, `bt2.py` and `bt3.py` in my
  scratchpad. I did not change the project.

Structure (`inspect_spans`): 4 DepthPass spans and no Region span. The
op has `region_ordering` global and ContourParallel.

| Span | Moves | G0 runs | Rapid XY (mm) | Fed XY (mm) |
|---|---|---|---|---|
| DepthPass Z 19.54 | 0–277 | 13 | 1 041 | 18 525 |
| DepthPass Z 14.08 | 277–563 | 17 | 1 177 | 18 525 |
| DepthPass Z 8.62 | 563–12674 | 1 581 | 115 226 | 34 275 |
| WaterlineCleanup | 12674–12719 | 2 | 211 | 90 |
| DepthPass Z 4.0 | 12719–14125 | 109 | 6 709 | 2 255 |
| WaterlineCleanup | 14125–14180 | 9 | 287 | 70 |

- RapidOrderBarrier spans sit only at the level boundaries (0, 277, 563,
  12674, 12719, 14125).
- The op has 1 726 G0 runs in all, which matches the narration's "1726
  round trip(s)".
- First half against second half (by move index, 0–7090 and
  7090–14180):
  - G0 runs: 547 against 1 179.
  - Rapid XY: 45.6 m against 79.1 m.
  - Fed XY: 62.4 m against 11.3 m.
- Retract heights (G-code Z of the top of each G0 run):
  - First half: 340 runs to Z +5.0 and 206 hops to the planes of the
    levels above (Z −4.46, −9.92, −15.38).
  - Second half: 623 to Z +5.0, 555 hops and 1 run to Z +10.0.
  - Z +5.0 is 5 mm above the stock top, so each of these runs leaves the
    stock completely.

Where it starts: inside DepthPass Z 8.62, at about move 3563.

| Moves (Z 8.62) | G0 runs | Rapid XY (mm) | Fed XY (mm) |
|---|---|---|---|
| 563–1563 | 21 | 1 262 | 6 372 |
| 1563–2563 | 8 | 520 | 5 743 |
| 2563–3563 | 10 | 338 | 5 003 |
| 3563–4563 | 105 | 6 940 | 3 007 |
| 4563–5563 | 138 | 11 831 | 2 332 |
| 5563–6563 | 151 | 13 935 | 2 140 |
| 6563–7563 | 162 | 14 335 | 1 614 |
| 7563–8563 | 135 | 12 638 | 1 969 |
| 8563–9563 | 183 | 13 672 | 1 705 |
| 9563–10563 | 208 | 14 640 | 1 581 |
| 10563–11563 | 208 | 13 883 | 1 489 |
| 11563–12563 | 226 | 11 232 | 1 173 |

What the planner does in the travel phase (moves 3563–12674), from the
G-code:

- Before move 3563, the level is cut along 70 stroke rows 2.0 mm apart
  (the commanded stepover 2.0), with fed moves in both X directions and
  39 G0 runs.
- From move 3563, the level becomes a one-way scanline in +X. It has 412
  stroke rows: 363 row gaps are 0.5 mm and 48 are 0.25 mm. That is a
  quarter or an eighth of the stepover. The fed strokes are 2 254 in +X
  and 486 in −X. The median fed move is 2.24 mm.
- On each row the tool cuts a short piece at or above the level, draped
  over the terrain (example: moves 6009–6019, Y 72.5, X 136.5–163.5).
  It lifts G0 to Z +5.0 and goes G0 in +X to the next piece on the same
  row (move 6007: X 44.5 → 136.5, 92 mm, at Z +5). At the row end it
  goes G0 to the start of the next row. Of the G0 moves, 491 run in +X
  and 370 run diagonally; none runs in −X. So the returns are diagonal
  rapids to the next row start. A few long diagonal G0 runs (up to
  194 mm) jump between areas (`phase0/img/wanaka200_cp_back_second_half_rapids.png`).
- Each re-entry at a new piece steps down through the planes of the old
  levels with 0.5 mm G0 hops. Example: moves 6022–6028 at X 217,
  Y 72.5: G0 to Z 0.5, then G1 to −4.96, G0 to −4.46, G1 to −10.42,
  G0 to −9.92, then G1 to −14.95. These are the N8 / G-HOPRETRACT hops.
  So N8 and N11 come from the same stretch: N11 is the XY travel
  between the pieces, and N8 is the Z hopping at each entry.

What the planner did, in one paragraph:

On the third level (setup Z 8.62) of the back rough, ContourParallel
first clears the level with 2.0 mm rows (moves 563 to about 3563). It
then switches, inside the same DepthPass span and with no Region span
and no barrier, to a dense scanline: one-way +X rows 0.5 mm apart (a
quarter of the stepover) over the whole board, Y about 23 to 222. On
each row it cuts only short pieces. It lifts out of the stock to Z +5
between pieces and travels in G0 along the row, and it goes back
diagonally at the row end. The planner gate for this level reads "pre
0.097 → post 0.053, floor 89209/240981 at-plane (8628 with material)".
So only about 3.6 % of the level's cells hold material, scattered over
the terrain. The scanline visits every row of the board to reach them.
This is not By Area ordering: the op is Global and has no Region spans.
It looks like a cell-row mop-up of the residual material on a 0.5 mm
grid (the row pitch equals a 0.5 mm grid, and the sim cell is also
0.5 mm). I did not find the planner stage in the code, so that cause is
not proven. The operator's reading fits: a few waterline-like loops
around the scattered material pockets would replace about 1 500 G0 runs
and about 110 m of rapid travel on this level.
