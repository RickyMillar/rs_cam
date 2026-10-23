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

# E2 at 3ae1da39

- Date: 2026-10-23, from 19:06 NZDT.
- GUI: branch `adaptive-instruments` 3ae1da39 (A2: the coverage readings
  are computed at adopt, with the abstentions NoCheckpoint / RunNotStored;
  B2: the gouge probe tolerance is the op chord tolerance, Blocking only
  below the model surface, Caution inside the leave band; plus the master
  heights and pocket commits).
- Binary `/home/ricky/personal_repos/rs_cam/target/release/rs_cam_gui`,
  mtime_unix 1792735547.89. The gui_pid is 2538272 (generation 4). The
  previous GUI (1275067) stopped through `gui_restart`, exit code 0. I
  did not restart it myself.
- The same projects and sequence as E. Exports go to
  `phase0/gcode/<name>_p15b.nc`. No project is saved.
- Adopt timing in E (first run), for comparison: Corne CP
  `run_simulation` took ≤ 8.6 s (not tightly timed), Corne AgentSearch
  ≤ 7.6 s, and wanaka200 4.0 s.

## E2 project 1: corne_contour_parallel (the zero control)

- Generate: 8006 and 5347 moves (the same as in E).
- `run_simulation`: stored 0.3 mm, auto. The adopt took about 2.0 s in
  all for the two roughs (from my timestamp to the reply's save time).
  The E figure (≤ 8.6 s) was a loose upper bound, so I cannot read a
  delta from it.
- The reply is now 600 KB, because `triage.coverage` carries every island
  (2117 islands on two ops).

### Coverage readings (`triage.coverage`, state `measured` on both ops)

| Op | Islands | Area above spec | Compared area | Bar | One-cell (0.3 mm wide) | 2.5–10 mm tall | ≥ 10 mm tall |
|---|---|---|---|---|---|---|---|
| op 0 "3D Rough 1" (6 mm) | 711 | 1791.2 mm² | 14 619 mm² | 0.80 mm (leave 0.5 + cell 0.3) | 543 | 151 | 17 |
| op 1 "(copy)" (3 mm) | 1406 | 960.0 mm² | 13 167 mm² | 0.80 mm | 1332 | 57 | 17 |

- Finding: `project.rough_leaves_islands`, severity critical, category
  safety, on both ops. It is in `triage.safety`. Op 0: "129 island(s)
  stand taller than one depth per pass plus one cell (6.30 mm)". Op 1:
  526 taller than 3.30 mm.
- Narration sentences (verbatim):
  - op 0: "coverage: 711 island(s) stand more than 0.80 mm above the
    target (model + 0.50 mm stock to leave), 1791.2 mm2 in all; the
    tallest is 22.50 mm high and 0.30 mm wide at (40.5, -106.6)."
  - op 1: "coverage: 1406 island(s) stand more than 0.80 mm above the
    target (model + 0.50 mm stock to leave), 960.0 mm2 in all; the
    tallest is 17.94 mm high and 1.40 mm wide at (110.1, -10.0)."
- The largest islands (area, height, width, peak):
  - op 0 and op 1: 80.7 mm², 14.50 mm, 1.04 mm, (90.3, −71.8); 61.4 mm²,
    14.50 mm, 1.40 mm, (9.0, −25.3); 24.8 mm², 16.5 mm, 0.55 mm,
    (7.5, −80.2). These are long strips (bbox up to 60 mm long) along the
    wall faces.
  - op 0 only: 22–24 mm², 3.50 mm, 1.5–1.6 mm, peak Z 4.0, bbox about
    9 × 7.8 mm, one in each hex hole (for example (34.8, −68.8),
    (112.8, −43.9), (43.2, −23.8)).

### Eye comparison (`phase0/img/corne_contour_parallel_p15b_islands.png`: op 0 on top, op 1 below; 3× top view; red = one cell wide, yellow = ≥ 10 mm tall, magenta = 2.5–10 mm, cyan = < 2.5 mm)

- Red, one cell wide (543 / 1332): a dotted line along the outer cut
  edge (the edge of the red uncut stock outside the boundary), along the
  foot of the outer wall, and (op 1) around every hex hole on the sides
  that are not parallel to an axis.
  - On the outer cut edge these are full-height (22.5 mm) columns of the
    stock wall outside the boundary. That stock was never meant to go.
    They are inside the population, but the tool never cut them. Cause
    not traced. Candidates: the reach margin (R − cell) is wider than the
    actual cut; the planner's outermost ring stops short of the boundary
    (the wall-strip row check shows the planner stops 0.6–0.9 mm short of
    a wall, so the outer fringe and the wall strips may be the same
    planner behaviour, one off the part and one on it); or dexel
    quantisation at the cut wall. **False positive** for a coverage
    reading of the part. They also fill the whole "tallest first" list of op 0's
    message and its narration sentence.
  - Around the hex holes they are a stair-step of the 0.3 mm grid
    against walls that are not parallel to an axis: real slivers of
    ≤ 0.3 mm against a vertical wall. They read as islands of full plate
    height because the bar is vertical.
- Magenta on op 0 (151): one patch in almost every hex hole, standing at
  plate + leave (Z 4.0) over the through-hole target (0.5). The 6 mm tool
  cannot clean the corners of a hex hole about 9 mm across. **Real
  tool-shape residue** (P15_A risk 1). The top-view colour map cannot
  tell Z 4 from Z 0.5, so the eye cannot confirm it here. Op 1 (3 mm)
  removes most of it: 57 remain, mostly 0.55 mm strips along the hex
  sides parallel to X.
- Yellow (17 on each op): strips 0.55–1.4 mm wide along the wall faces,
  standing at wall top + 0.5. Row check at y = −45 (STL and stock
  pixels): the model wall top (Z 18) spans x 7.0–8.8 and the plate starts
  at x 9.1. The stock shows mid-height colour at x 9.1–9.7 and reaches
  plate colour only at x 10.0–10.3. So about 0.6–0.9 mm of stock stands
  against the inner wall face. **Real radial residue** that the eye
  missed in E at the coarse scale. Op 1 (3 mm) leaves the same strips at
  the same places, so its smaller tool did not cut them either.
- No island class reads a hole core or the outer band. That is correct:
  CP has neither.

### Gouge

- No `safety.cuts_into_model` finding on either op (no Blocking and no
  Caution). The E readings are gone: the 0.064 mm rim overshoot and the
  3.2 mm hex-hole graze. That fits B2 (probe tolerance = chord tolerance
  0.3 mm, larger than the 0.02–0.05 mm graze).
- Export to `phase0/gcode/corne_contour_parallel_p15b.nc`: **succeeded**.
- `link_cuts_stock` is unchanged (moves 2320 and 3153, caution).

### Verdicts, E2 project 1

- Coverage: reads, and gives positions and sizes. Against the expectation
  of "0 islands, except inside-corner residue", it gives a **false
  positive** for the zero control in its class and its headline.
  - The fringe of one-cell full-height columns on the outer cut edge is
    an instrument artifact. It makes the headline "22.50 mm high,
    0.30 mm wide", which names material outside the part.
  - The wall strips and the hole-side slivers are real radial residue.
    But the vertical bar turns any radial residue on a vertical wall into
    an island of full wall height, so they make the class Safety (129 /
    526 islands "taller than one step"). The instrument cannot separate
    a sliver attached to a wall from a free-standing spur (G-SPUR) or a
    hole core (G-HOLECORE).
  - The hex-corner residue of the 6 mm tool is real and expected.
- Gouge: **reads what the eye saw**. Nothing cuts into the model; the
  export succeeds.

## E2 project 2: corne_agent_search (the positive control)

- Generate: 4168 and 6060 moves (the same as in E). My first op 1 call
  was refused ("a generation plan is already running") because I sent it
  in parallel with op 0. It ran when sent alone. This is a caller error,
  not a GUI fault.
- `run_simulation`: stored 0.3 mm. The adopt took about 2.2 s (E:
  ≤ 7.6 s, not tightly timed).

### Coverage readings (state `measured` on both ops)

| Op | Islands | Area above spec | One-cell | Largest |
|---|---|---|---|---|
| op 0 (6 mm) | 379 | 7236.4 mm² | 255 | 3216.6 mm², 22.50 mm, 9.09 mm wide, peak (14.7, −109.3): the band outside the wall at full stock height |
| op 1 (3 mm, final stock) | 1262 | 3695.2 mm² | 1171 | 1312.9 mm², 14.50 mm, 4.54 mm wide, (23.7, −100.6) |

- Finding: `project.rough_leaves_islands`, critical / safety, on both ops
  (op 0: 10 islands taller than 6.30 mm; op 1: 516 taller than 3.30 mm).
- Narration sentences (verbatim):
  - op 0: "coverage: 379 island(s) stand more than 0.80 mm above the
    target (model + 0.50 mm stock to leave), 7236.4 mm2 in all; the
    tallest is 22.50 mm high and 9.09 mm wide at (14.7, -109.3)."
  - op 1: "coverage: 1262 island(s) stand more than 0.80 mm above the
    target (model + 0.50 mm stock to leave), 3695.2 mm2 in all; the
    tallest is 18.00 mm high and 4.54 mm wide at (110.1, -10.0)."

### Eye comparison (final stock, op 1; `phase0/img/corne_agent_search_p15b_islands.png`: green = hole core, yellow = area ≥ 40 mm² with bbox, red = one cell, magenta = other; white rings = the Phase 0 spur position and the 13 mm island)

- **Hole cores (G-HOLECORE).** 42 islands have a centroid over no model
  surface, inside the part, width ≥ 0.9 mm, and area 2.7–4.0 mm². By eye
  on the map, 37 of them are the dark cores in the hex holes, and 5 lie
  in small round holes between the hex holes (likely screw holes; the CP
  map has islands at the same 5 places). E counted about 37 hex cores by
  eye, so the instrument reads **the same number**.
  - Each core is 3.50 mm tall (a few 1.50 or 3.62), stands to Z 4.0 over
    the through-hole target 0.5, and is 1.4–1.86 mm wide. Example cores:
    (28.3, −50.1), (37.0, −55.0), (45.6, −70.0), (106.3, −45.0),
    (114.9, −60.0), (132.2, −40.0).
- **Band (G-OUTERBAND, N7).** It reads as about 8 large islands along the
  outside of the case wall, 3.9–4.65 mm wide and 16.5–18 mm tall:
  701.7 mm² (110.1, −10.0), 628.4 mm² (22.5, −108.7), 191.2 mm²
  (45.0, −99.4), 95.1 mm² (85.5, −3.1), 61.0 mm² (109.5, −3.4),
  56.5 mm² (48.9, −5.5), 45.2 mm² (38.7, −4.6). Also 1312.9 mm² at
  (23.7, −100.6), whose bbox covers the left and bottom of the inner
  perimeter: the wall strips and the band joined. The round holes punched
  in the band split it into these pieces. **It matches the eye.**
- **Spur (G-SPUR).** The island nearest the Phase 0 spur position, world
  (104.81, −23.32), is 13.00 mm tall, 0.30 mm wide and 0.5 mm², peak
  (102.91, −22.32, 17.0), bbox (102.3, −22.3)–(102.9, −21.7). That is
  2.2 mm from the Phase 0 point. Phase 0 measured 13 mm at 7:32 and
  16.5 mm at the end, 0.3–1.0 mm wide, on a 0.1 mm re-simulation and a
  0.2 mm heightmap. **Height and width match the Phase 0 range** (13 mm
  against 13–16.5 mm; 0.3 mm against 0.3–1.0 mm). This run is at 0.3 mm,
  so it sees only the thickest sliver.
  - Over the whole op only 3 islands are ≤ 1.04 mm wide and ≥ 10 mm tall.
    Phase 0 counted 39 free-standing spurs at the end at 0.2 mm. The
    instrument does not see the thinner spurs at a 0.3 mm cell, and it
    merges spurs that touch a wall or the band into those islands.
  - The spur island is not in the "tallest first" list of the finding.
    The band entries (16.5–18 mm) fill the list, so a reader of the
    message does not see the spur.
- **Green mid-height strips (seen in E).** They are inside the wall
  islands (the yellow 1312.9 and 701.7 mm² boxes). They are not
  separate islands.
- The same one-cell fringe as on CP: around the hex sides and along the
  outer cut edge (1171 islands).

### Gouge

- No `safety.cuts_into_model` finding on either op. No finding at move
  1401.
- Export to `phase0/gcode/corne_agent_search_p15b.nc`: **succeeded**.
  The reply also says: "Exceeded without refusing the export, because the
  bound does not gate one: toolpath 1: depth of cut, peak 12.0000 mm,
  limit 9.0000 mm ... toolpath 3: depth of cut, peak 15.0000 mm, limit
  4.5000 mm ... a rule of thumb with no published source".
- `crosses_standing_material` is unchanged (4, peak 15.00 mm, move 1401).

### Verdicts, E2 project 2

- Coverage: **reads what the eye saw** for the hole cores (37, the same
  as the eye count), the band and the spur (13 mm × 0.3 mm, 2.2 mm from
  the Phase 0 point). Limits:
  - The message and the narration headline name only the band.
  - The one-cell fringe adds about 1170 islands of noise.
  - Spurs thinner than one cell, or attached to a wall or the band, do
    not read as their own islands.
- Gouge: reads what the eye saw (no gouge). The export succeeds.

## E2 project 3: corne_contour_spiral

- Generate: 14839 and 5376 moves (the same as in E). `run_simulation`:
  stored 0.3 mm. The adopt took about 2.3 s.

### Coverage readings (state `measured` on both ops)

| Op | Islands | Area above spec | One-cell | Largest |
|---|---|---|---|---|
| op 0 (6 mm) | 686 | 5654.2 mm² | 529 | 3216.6 mm², 22.50 mm, 9.09 mm wide at (14.7, −109.3): the band at full stock height (the same as AgentSearch op 0) |
| op 1 (3 mm, final) | 1227 | 3577.6 mm² | 1166 | 1315.3 mm², 16.50 mm, 4.54 mm wide at (90.9, −11.2) |

- Finding: critical / safety on both ops (op 0: 6 islands taller than
  6.30 mm; op 1: 491 taller than 3.30 mm).
- Narration sentences (verbatim):
  - op 0: "coverage: 686 island(s) stand more than 0.80 mm above the
    target (model + 0.50 mm stock to leave), 5654.2 mm2 in all; the
    tallest is 22.50 mm high and 9.09 mm wide at (14.7, -109.3)."
  - op 1: "coverage: 1227 island(s) stand more than 0.80 mm above the
    target (model + 0.50 mm stock to leave), 3577.6 mm2 in all; the
    tallest is 18.00 mm high and 4.54 mm wide at (110.7, -10.0)."

### Eye comparison (`phase0/img/corne_contour_spiral_p15b_islands.png`, the same legend as AgentSearch)

- **Hole cores.** There are 7 hole-interior islands under 5 mm²:
  - 2 hex-hole cores at (19.6, −84.9) and (54.3, −85.0), each 3.00 mm
    tall and 1.4–1.6 mm wide. E saw exactly 2 by eye, at (19.4, −85.0)
    and (54.0, −85.0), and Phase 0 said "2 hole cores 3 mm tall". **An
    exact match** in count, position and height.
  - 5 in the small round holes, at the same 5 positions as AgentSearch:
    (35.4, −77.2), (49.0, −29.6), (81.8, −69.4), (125.1, −52.3) and
    (125.1, −33.2).
- **Band.** About 8 large islands along the outside of the wall:
  706.8 mm² (110.7, −10.0), 628.4 mm² (12.0, −88.0), 191.5 mm²
  (45.0, −94.9), 97.2, 61.0, 56.6 and 45.4 mm², all 16.5–18 mm tall and
  3.9–4.65 mm wide. **It matches** the band with punched holes that the
  eye sees.
- No spur island (none ≤ 1.04 mm wide and ≥ 10 mm tall apart from wall
  fringe cells). Phase 0 saw only nubs ≤ 3.4 mm here.
- The same one-cell fringe (1166 islands).

### Gouge

- No `safety.cuts_into_model` finding on either op.
- Export to `phase0/gcode/corne_contour_spiral_p15b.nc`: **succeeded**.

### Verdicts, E2 project 3

- Coverage: **reads what the eye saw**: the 2 hole cores (count,
  position and height match), the band, and no spur. It has the same
  limits as AgentSearch: the band fills the headline, and the one-cell
  fringe adds about 1170 islands.
- Gouge: reads what the eye saw. The export succeeds.

## E2 project 4: wanaka200_contour_parallel

- Generate: Pin Drill 68 moves; back rough 14180 moves in about 22.8 s;
  front rough 14049 moves in about 20.7 s. In E they took 20.4 s and
  19.2 s, and in Phase 0 14.4 s and 14.0 s.
- `run_simulation`: stored 0.5 mm. The adopt took about 4.7 s (E: 4.0 s),
  so the readings add about 0.7 s here.
- Export to `phase0/gcode/wanaka200_contour_parallel_p15b.nc`:
  **succeeded** (one file, 920 975 bytes). The reply notes "toolpath 1:
  depth of cut, peak 17.1251 mm, limit 9.0000 mm ... a rule of thumb with
  no published source" and does not gate.

### Gouge and the link plough

- No `safety.cuts_into_model` finding on either rough (E: Blocking
  0.098 mm and 0.180 mm; both are gone, no Caution either).
- `project.link_cuts_stock`, critical, on "2 Back Rough": "link move
  5147 removes 11.26 mm at (216.9, 55.0, 8.63). 20 of 44782 link samples
  on 12 link moves ...". **The link plough is still reported only by
  `link_cuts_stock`**, as expected.

### Coverage

- "2 Back Rough": `{"state": "not_measurable", "reading": {"kind":
  "setup_not_identity"}}`. Narration (verbatim): "coverage: not measured
  (the op's setup is not an identity setup (face up Top, no rotation),
  which this reading needs)."
  - The abstention is right: the back rough is in "Setup 1", face_up
    Bottom. The parenthetical "(face up Top, no rotation)" defines an
    identity setup; it does not name the op's face. The phrasing is
    ambiguous: a reader can take it as the op's face. A clearer form would
    name the op's own face (Bottom).
- "6 3D Rough (front)" (Setup 2, face_up Top, identity): measured. 910
  islands, 14 976.0 mm² above spec of 42 118 mm² compared (36 %), bar
  1.00 mm (leave 0.5 + cell 0.5). Critical / safety: "4 island(s) stand
  taller than one depth per pass plus one cell (4.70 mm)". Narration:
  "coverage: 910 island(s) stand more than 1.00 mm above the target
  (model + 0.50 mm stock to leave), 14976.0 mm2 in all; the tallest is
  9.31 mm high and 2.50 mm wide at (0.0, -2.5)." (From the
  `narrate_toolpath` call on index 5. Its levels are z 2.8, −1.4 and
  −2.314, so the terraces between levels are 4.2 mm.)
- Heights: 830 islands 1–2 mm, 65 2–3 mm, 11 3–4 mm, 1 4–5 mm, 2 5–6 mm,
  1 9.31 mm. Area by height: 1–2 mm 1692 mm², 2–3 mm 1492, 3–4 mm 493,
  4–5 mm 5044, 5–6 mm 4337, 9–10 mm 1917.
- The four islands over 4.70 mm:
  - 9.31 mm, 2.50 mm wide, 1917.2 mm², bbox (−2.5, −2.5)–(202.5, 202.0):
    a frame round the whole terrain at the boundary. Edge fringe, as on
    Corne. **False positive.**
  - 5.41 mm, 5.50 mm wide, 3417.8 mm², bbox x 4–77.5, y 4.5–131.5, and
    4.91 mm, 4.60 mm wide, 5044.2 mm², bbox over the whole terrain: large
    connected networks of terrace residue.
  - 5.04 mm, 3.97 mm wide, 919.2 mm², at (100.5, 96.0): terrace residue.
- Eye (`phase0/img/wanaka200_contour_parallel_p15b_islands.png`; yellow
  = over 4.7 mm with bbox, magenta 2–4.7, cyan < 2, white one cell): the
  stock render shows the terraces that a 6 mm flat tool with 4.2 mm
  levels leaves on the slopes. The islands follow those terraces. The 755
  one-cell islands are scattered along the terrace edges and along the
  edge of the low flat area at the top left.
- Reading: the material is real. It is the normal staircase of a Z-level
  rough on a slope, up to one step (4.2 mm) above model + leave. The bar
  (leave + one cell) makes it 36 % of the area and the class Safety.
  Whether a rough must be judged on terraces is a question for the lead.
  As it stands, every Z-level rough on a terrain fails this bar.

## E2 summary

| Instrument | Corne CP (zero) | Corne AgentSearch (positive) | Corne ContourSpiral | wanaka200 CP |
|---|---|---|---|---|
| Coverage present? | yes, both ops | yes, both ops | yes, both ops | back: `setup_not_identity` (designed abstention); front: measured |
| Coverage against the eye | false positive: 711 / 1406 islands, safety class. The classes are a one-cell edge fringe (false), real wall strips 0.6–1.4 mm, hex-corner residue of the 6 mm tool (real, expected) | reads what the eye saw: 37 hex cores (eye 37), the band, the spur 13.0 × 0.3 mm 2.2 mm from the Phase 0 point; plus 5 small-hole residues and the fringe | reads what the eye saw: 2 hex cores at the eye positions, 3.0 mm (Phase 0: 3 mm), the band; no spur | front: reads real terraces, flagged as 36 % of the area; edge-fringe frame 9.31 mm |
| Narration "coverage:" sentence | present | present | present | present (both) |
| Gouge | none; export OK | none; export OK | none; export OK | none; export OK; link plough only in `link_cuts_stock` |
| Adopt time (run_simulation) | about 2.0 s (E ≤ 8.6 s) | about 2.2 s (E ≤ 7.6 s) | about 2.3 s | about 4.7 s (E 4.0 s) |

### Verdicts at 3ae1da39

- **Gouge (B2): reads what the eye saw.** No finding on any of the eight
  roughs; all four exports succeed. The link plough stays with
  `link_cuts_stock`. I saw no Caution finding, so the "Caution inside the
  leave band" path and the new message fields (depth below limit / below
  surface, at_wall) were not exercised by these projects.
- **Coverage (A2): present everywhere, and it finds the real defects.**
  On the positive control it reads the hole cores, the band and the spur.
  On ContourSpiral it reads the 2 cores exactly. But it has three
  **false-positive classes** that make every rough fail, the zero control
  included:
  1. A one-cell (0.3–0.5 mm) fringe of full-height columns along the
     outer cut edge and the stock edge. Cause not traced (reach margin,
     planner ring stopping short, or dexel quantisation). These columns
     also fill
     the "tallest first" list and the narration headline (CP op 0:
     "22.50 mm high and 0.30 mm wide"; wanaka front: "9.31 mm high").
  2. Radial residue on vertical walls (0.3–1.4 mm wide strips and grid
     stair-steps). The vertical bar turns any radial residue into an
     island of full wall height, so it reads Safety. The material is
     real, but the reading cannot separate it from a spur or a core.
  3. Terrace residue on slopes (wanaka front): the normal staircase of a
     Z-level rough, up to one step, flagged as 36 % of the area.
- The headline problem: on every column the finding's "tallest first"
  list names the band, the fringe or a wall strip, and not the defect the
  eye cares about. The hole cores (3.5 mm) and the spur (13 mm × 0.3 mm)
  appear only in the full island list.
- Wording: the `setup_not_identity` sentence "(face up Top, no
  rotation)" defines an identity setup, but a reader can take it as the
  op's face. Naming the op's own face would be clearer.

### Suggestions for the lead (not tested)

- First trace the cause of fringe class 1. If it is the margin, shrink
  it by one more cell, or drop islands that touch the population edge.
- Tag an island as "attached to a model wall" when it touches cells
  whose model top is above its own top. Report those apart from free
  islands (cores, spurs), or measure their horizontal thickness instead
  of their height.
- For terrace residue, compare against the commanded step (as the Safety
  class already does) instead of the leave, or report terraces as a
  separate class.

# E3 at c97093af

- Date: 2026-10-23, from 20:14 NZDT.
- GUI: branch `adaptive-instruments` c97093af (A3: the coverage target is
  the plan's own reachable floor, eroded by the cutter disc; each island
  carries `touches_wall`; the bar is standing > leave + one cell over
  that target; see the worktree note `P15_A3_coverage_target.md` §6 for
  A3's predictions).
- Binary mtime_unix 1792739600.22, gui_pid 2918218 (generation 5). The
  previous GUI (2538272) stopped through `gui_restart`, exit 0.
- The same four projects and sequence. Exports go to
  `phase0/gcode/<name>_p15c.nc`. Island maps colour `touches_wall`
  islands apart.

## E3 project 1: corne_contour_parallel (the zero control)

- Generate 8006 / 5347 moves (unchanged). Adopt about 2.3 s. Export
  `phase0/gcode/corne_contour_parallel_p15c.nc` succeeded. No
  `cuts_into_model` finding on either op. `link_cuts_stock` unchanged.
- Map: `phase0/img/corne_contour_parallel_p15c_islands.png` (op 0 on top,
  op 1 below; orange = `touches_wall`; for free islands red = one cell,
  magenta = 2.5–10 mm, yellow = ≥ 10 mm, cyan = < 2.5 mm).

| Op | E2 islands | E3 islands | E3 `touches_wall` | E3 free one-cell | E3 free 2.5–10 mm | E3 area (E2) |
|---|---|---|---|---|---|---|
| op 0 (6 mm, step 6) | 711 | 902 | 191 (174 one-cell, 17 ≥ 10 mm) | 627 | 84 | 1373.5 mm² (1791.2) |
| op 1 (3 mm, step 3, final) | 1406 | 1428 | 1387 (1367 one-cell, 15 ≥ 10 mm, 5 mid) | 41 | 0 | 772.9 mm² (960.0) |

- Narration (verbatim):
  - op 0: "coverage: 902 island(s) stand more than 0.80 mm above the
    target (the floor the plan can reach, with 0.50 mm stock to leave),
    1373.5 mm2 in all, 191 against a wall; the tallest is 22.50 mm high and
    0.55 mm wide at (36.0, -109.3)."
  - op 1: "coverage: 1428 island(s) stand more than 0.80 mm above the
    target (the floor the plan can reach, with 0.50 mm stock to leave),
    772.9 mm2 in all, 1387 against a wall; the tallest is 16.61 mm high
    and 0.90 mm wide at (110.4, -9.7)."
- Class: still critical / safety on both ops (op 0: 111 islands taller
  than 6.30 mm; op 1: 411 taller than 3.30 mm). Every island in both
  "tallest first" lists is marked "at a wall".

What each class is (eye, on the map):

- **Wall strips** (17 / 15, ≥ 10 mm, all `touches_wall`): the same strips
  as E2 (87.4, 53.5, 33.5 mm² ...). As A3 predicted.
- **Outer-edge fringe:** still reads, now `touches_wall` (orange dots on
  the outer cut edge and the stock edge). The op 0 headline is one of
  them: 22.50 mm × 0.55 mm at (36.0, −109.3), which is on the stock's
  minimum-y edge. As A3 predicted.
- **op 0, 84 free islands 2.5–10 mm:** one at the centre of almost every
  hex hole, 3.5 mm tall (example 18.7 mm² at (36.3, −68.8)). The new
  target puts the hole floor (0.5) inside the reach of the 6 mm tool, so
  the plate-level stock in the hole reads. A3 predicted that the 6 mm
  hex-corner residue would read nothing (E2: 151). **That prediction
  fails: 84 remain.** They look like holes that op 0 did not enter.
  Op 1 clears them (0 remain on op 1), so they are real residue of op 0,
  not of the job.
- **op 0, 627 free one-cell islands:** a ring of slivers round every hex
  rim. The hex wall is a 3.5 mm step, which is less than op 0's 6 mm
  depth per pass, so `touches_wall` (a target step of more than one depth
  per pass) does not tag them. On op 1 (3 mm step) the same kind of
  slivers are tagged (1367). **The tag depends on the op's depth per
  pass**, not on the wall: one sliver can be "at a wall" for one op and
  free for another.
- **op 1, 41 free one-cell islands:** all 0.8–1.1 mm tall, just over the
  0.80 mm bar, along the foot of the angled walls (for example the
  diagonal at x 6–20, y −72 to −100, and y −93 to −96 at x 39–53). Grid
  slivers at the wall foot that the tag misses by one cell.
- **Screw holes:** no free island at the 5 small-hole positions on op 1.

Verdict, project 1: on the final stock (op 1) the zero control fails on
1387 `touches_wall` islands (the wall strips, the hex-side slivers and
the reachable fringe) plus 41 free slivers 0.8–1.1 mm tall at angled wall
feet. On op 0 it also fails on real uncut hole centres (84) and on 627
hex-rim slivers that the tag misses because the hole wall is shorter
than op 0's step.

## E3 project 2: corne_agent_search (the positive control)

- Generate 4168 / 6060 moves. Adopt about 2.5 s. Export
  `phase0/gcode/corne_agent_search_p15c.nc` succeeded (depth-of-cut rule
  of thumb noted, not gated). No `cuts_into_model` on op 1. I did not read
  op 0's diagnostics in E3; the export shows no Blocking finding there.
  `crosses_standing_material` unchanged.
- Map (final stock, op 1): `phase0/img/corne_agent_search_p15c_islands.png`
  (orange = `touches_wall`, green = hole core, red = free one-cell,
  magenta = free 2.5–10 mm).

| Op | E2 islands | E3 islands | E3 `touches_wall` | E3 free one-cell | E3 free 2.5–10 mm | E3 free ≥ 10 mm | E3 area (E2) |
|---|---|---|---|---|---|---|---|
| op 0 (6 mm) | 379 | 457 | 15 | 361 | 79 | 2 | 6832.7 mm² (7236.4) |
| op 1 (3 mm, final) | 1262 | 1257 | 1175 | 44 | 34 | 0 | 3652.5 mm² (3695.2) |

- Narration (verbatim):
  - op 0: "coverage: 457 island(s) stand more than 0.80 mm above the
    target (the floor the plan can reach, with 0.50 mm stock to leave),
    6832.7 mm2 in all, 15 against a wall; the tallest is 22.50 mm high and
    9.47 mm wide at (14.1, -109.3)."
  - op 1: "coverage: 1257 island(s) stand more than 0.80 mm above the
    target (the floor the plan can reach, with 0.50 mm stock to leave),
    3652.5 mm2 in all, 1175 against a wall; the tallest is 16.50 mm high
    and 3.90 mm wide at (85.5, -3.1)."
- Eye defects on the final stock (op 1):
  - **Hex cores: 37** (E2: 37), all `touches_wall` false, as A3
    predicted. 1.5–3.6 mm tall, 1.40–1.60 mm wide.
  - **Screw-hole islands: 0** (E2: 5). The new target removed them.
  - **Band:** 8 islands of 49–1303 mm², 14.5–16.5 mm tall, all
    `touches_wall` true (1302.7, 749.2, 678.1, 205.2, 103.1, 66.1, 61.0,
    48.7 mm²). As A3 predicted.
  - **Spur:** 13.0 mm tall, 0.30 mm wide, 0.54 mm², peak (102.91, −22.32,
    17.0), `touches_wall` false. The same as E2, as A3 predicted.
  - The free one-cell islands fall from 1171 (E2) to 44; the rest are now
    tagged (1159 one-cell `touches_wall`).
- op 0: 79 free 2.5–10 mm islands of about 51 mm² each, 3.5 mm tall,
  7.59 mm wide: whole hex holes that op 0 did not enter (op 1 clears
  them except the 37 cores). One free 16.5 mm island of 51.5 mm² at
  (105.3, −24.7) is the hex hole at the spur, uncut to Z 17 by op 0.
- The "tallest first" list of op 1 holds only band islands and one 13.04 mm
  fringe cell, all "at a wall". The 37 cores and the spur are free
  islands, but they are not in the list, because they are shorter than
  the band.

Verdict, project 2: **every eye defect still reads**, and each one is
free (`touches_wall` false): 37 cores, the spur (13.0 × 0.3 mm). The band
reads as wall-touching. The screw-hole residue is gone. The headline
still names the band.

## E3 project 3: corne_contour_spiral

- Generate 14839 / 5376 moves. Adopt about 4.3 s. Export
  `phase0/gcode/corne_contour_spiral_p15c.nc` succeeded. No
  `cuts_into_model` on op 0 (op 1 not read; the export shows no Blocking).
- Map (final stock, op 1): `phase0/img/corne_contour_spiral_p15c_islands.png`.

| Op | E2 islands | E3 islands | E3 `touches_wall` | E3 free one-cell | E3 free 2.5–10 mm | E3 area (E2) |
|---|---|---|---|---|---|---|
| op 0 (6 mm) | 686 | 772 | 15 | 699 | 58 | 5271.9 mm² (5654.2) |
| op 1 (3 mm, final) | 1227 | 1246 | 1204 | 40 | 2 | 3532.2 mm² (3577.6) |

- Narration (verbatim):
  - op 0: "coverage: 772 island(s) stand more than 0.80 mm above the
    target (the floor the plan can reach, with 0.50 mm stock to leave),
    5271.9 mm2 in all, 15 against a wall; the tallest is 22.50 mm high and
    9.47 mm wide at (14.1, -109.3)."
  - op 1: "coverage: 1246 island(s) stand more than 0.80 mm above the
    target (the floor the plan can reach, with 0.50 mm stock to leave),
    3532.2 mm2 in all, 1204 against a wall; the tallest is 16.80 mm high
    and 5.10 mm wide at (110.4, -9.7)."
- Eye defects on the final stock (op 1):
  - **Hex cores: 2**, at (19.6, −84.9) and (54.3, −85.0), 3.00 mm tall,
    1.6 and 1.4 mm wide, `touches_wall` false. The same as E2, the eye
    and Phase 0. As A3 predicted.
  - **The free 2.5–10 mm class on op 1 is exactly these 2 cores.**
  - **Screw-hole islands: 0** (E2: 5).
  - **Band:** 8 islands of 49–1305 mm², `touches_wall` true.
  - **Spur:** none (no free island ≥ 10 mm). Correct.
  - Free one-cell: 40 (E2: 1166).
- op 0: 58 free 2.5–10 mm islands of 13–20 mm², 3.5 mm tall: hex holes
  partly uncut by op 0 (op 1 clears them).

Verdict, project 3: **every eye defect still reads** (2 cores, the band),
with no spur. On the final stock the only free islands are the 2 cores
and 40 one-cell slivers.

## E3 project 4: wanaka200_contour_parallel

- Generate 68 / 14180 / 14049 moves. **Adopt about 18.5 s** (E2: 4.7 s).
  This is the one slow call of E3 (over 10 s). It is likely the new
  eroded-floor target on the 0.5 mm grid of a 240 × 250 mm stock (the
  Corne adopts stayed at 2.3–4.3 s).
- Export `phase0/gcode/wanaka200_contour_parallel_p15c.nc` succeeded
  (depth-of-cut rule of thumb noted on toolpath 1, not gated). No
  `cuts_into_model` on the front rough. I did not read the back rough's
  diagnostics; the export shows no Blocking. `link_cuts_stock` (critical,
  move 5147, 11.26 mm) is unchanged in the triage actions.
- Back rough: `not_measurable` / `setup_not_identity`, the same sentence
  as E2.
- Front rough: measured.

| | E2 | E3 |
|---|---|---|
| Islands | 910 | 178 |
| Area above spec | 14 976.0 mm² (36 % of compared) | 955.5 mm² (2.2 % of 42 525 mm²) |
| Tallest | 9.31 mm (boundary frame) | 2.11 mm |
| `touches_wall` | — | 0 |
| Class | critical / safety | caution / quality (in `triage.actions`) |
| One-cell | 755 | 175 |

- Narration (verbatim): "coverage: 178 island(s) stand more than 1.00 mm
  above the target (the floor the plan can reach, with 0.50 mm stock to
  leave), 955.5 mm2 in all, 0 against a wall; the tallest is 2.11 mm
  high and 0.50 mm wide at (105.0, 182.0)."
- Heights: 176 islands of 1–2 mm, 2 of 2–3 mm.
- Eye (`phase0/img/wanaka200_contour_parallel_p15c_islands.png`):
  - The terrace networks of E2 (4–5.4 mm, up to 5044 mm²) read nothing.
    The new target is the floor that the flat tool can reach, so the slope
    shadow of the flat tool is inside the target. **A3 predicted that held
    terraces would still read. None do.** The prediction fails in the good
    direction.
  - The 9.31 mm boundary frame is gone. A3 predicted it would read with
    `touches_wall` true.
  - What remains: one-cell lines 1.1–1.7 mm tall along the terrain border
    (x ≈ 3 and 197–198.5, y ≈ 3 and 193–198; the largest is 307.5 mm²
    along y 181.5–198.5), and scattered one-cell islands on steep slope
    edges and on the edge of the low flat area. None is tagged
    `touches_wall`, because no target step there is larger than one
    depth per pass (4.2 mm).

## E3 summary (E2 → E3)

| Rough | Islands | `touches_wall` | Free one-cell | Free 2.5–10 mm | Eye defects | Class |
|---|---|---|---|---|---|---|
| CP op 0 | 711 → 902 | 191 | 627 (hex-rim slivers; the 3.5 mm hole wall is under the 6 mm step) | 84 (uncut hex holes, real) | none expected | safety |
| CP op 1 (final) | 1406 → 1428 | 1387 | 41 (0.8–1.1 mm, angled wall foot) | 0 | none expected | safety |
| AS op 0 | 379 → 457 | 15 | 361 | 79 (uncut hex holes) | — | safety |
| AS op 1 (final) | 1262 → 1257 | 1175 | 44 | 34 (incl. the 37 cores, some under 2.5 mm) | 37 cores (free), band (wall), spur 13.0 × 0.3 mm (free); screw holes 5 → 0 | safety |
| CS op 0 | 686 → 772 | 15 | 699 | 58 (partly uncut hex holes) | — | safety |
| CS op 1 (final) | 1227 → 1246 | 1204 | 40 | 2 (= the 2 cores) | 2 cores (free), band (wall); screw holes 5 → 0 | safety |
| wanaka back | abstain → abstain | — | — | — | — | — |
| wanaka front | 910 → 178 | 0 | 175 | 0 | terraces gone, frame gone | safety → caution |

Gouge: no `cuts_into_model` finding on any rough I read; all four exports
succeed.

Adopt times: Corne 2.3 / 2.5 / 4.3 s; wanaka200 18.5 s (E2: 4.7 s).

### Verdicts at c97093af

- **Does every eye defect still read? Yes.** 37 hex cores on
  AgentSearch and 2 on ContourSpiral, all free (`touches_wall` false), at
  the eye positions and heights. The spur (13.0 × 0.3 mm) reads free. The
  band reads on both, tagged `touches_wall`. The screw-hole residue is
  gone (5 → 0).
- **Does the zero control now fail ONLY on real wall strips and reachable
  fringe? Almost, on the final stock; no, on op 0.**
  - Final stock (CP op 1): 1387 of 1428 islands are `touches_wall` (the
    wall strips, the hex-side slivers, the reachable outer fringe). The
    other 41 are free one-cell slivers 0.8–1.1 mm tall at the foot of
    angled walls, missed by the tag by about one cell.
  - CP op 0: 627 free one-cell hex-rim slivers and 84 free uncut hex
    holes. The holes are real residue of op 0 (op 1 clears them). The
    slivers are not tagged because `touches_wall` needs a target step of
    more than one depth per pass: the 3.5 mm hole wall is under op 0's
    6 mm step but over op 1's 3 mm step. **The tag depends on the op's
    step, so the same sliver is "at a wall" for one op and free for
    another.**
- **The one-cell edge fringe outside the reach: gone** (wanaka frame
  gone; Corne fringe now tagged `touches_wall` where it is inside the
  reach).
- **wanaka front:** the terrace class is gone. It now reads 178 small
  islands (≤ 2.11 mm) on the border and slope edges, class caution.
- **Headline:** on Corne it still names a wall-touching island (fringe
  or band) every time. The free cores and the spur are not in the
  "tallest first" list. A list of free islands first would put them
  there.
- **Cost:** the wanaka200 adopt grew from 4.7 s to 18.5 s.
