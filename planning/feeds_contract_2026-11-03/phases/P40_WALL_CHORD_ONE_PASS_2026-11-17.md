# P40: the wall chord of an entry (A), and the one-pass rule again (B) (2026-11-17)

Job: `planning/rivmap_block_job/nz_wanaka_21_lead_proposal.toml` at `0a1f60fe1` (HEAD), on
scratch copies. Setup "1 Back". Positions are in the G-code frame (setup frame + (18.5, 8.5))
unless the text says otherwise.

Paths: `p40/...`, `p39/...` and `p38/...` with no `phases/` in front are the scratch dirs of
the session (`/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/ce846d39-41d4-4e44-be56-d5dd5295a423/scratchpad/`).
They are not in the repo. `phases/p40/` in the repo holds `meas.py`, `left.py` and
`not_landed/P40_B.patch`. The two scripts run from a folder that holds `gapmap.py` of P37 and
the outputs of a run (`out_<tag>.nc`, `out_<tag>.npz`, `out_<tag>/summary.json`).

Part A is a safety fix: a ramp or helix entry cuts the top edge of a wall of the model. Part B
is a measurement of the one-pass rule of P38 on top of part A. Part B lands no code.

Sections A1 to A4 were written before a line of the fix was written. The reproduction (A2) ran
red at HEAD first. Section A5 and later hold the results.

## 0. What failed, and what is not verified

- NOT MEASURED: setup "1 Back" of the proposal, for part A and for the prototype of part B.
  The machine had less than 18 GB available (`free -g`) from 18:40 with no job of this session
  in work; the rule for a release CLI run of the job is 18 GB or more. So the measures of task
  items A5, B3 (the job table) and B4 are open. `p40/cli_A` and `p40/cli_B` are kept, and
  `p40/jobA.sh <cli> <tag>` makes the run and its measures (A6.1).
- Part A is proven in the crate: the lib test at the emitters (red at HEAD) and one sentry test
  through the 3D Rough planner and emitter (red with the entry lift off, A5). The rivmap100 copy
  moves in 34 helix places. It is not proven on the Wanaka back: the 33 nodes of P39 section
  2.4 were not read again.
- The first form of the rule of part A was wrong (the reduced tool as the lift probe: 0.671 mm
  under the limit). The rule changed after that run (A5).
- A tool with a centre core and the rest ladder: the fix covers the two doors; no test
  reproduces the defect there (A2).
- The strict read that the task asked for in part B fails the lib test of the rule (11.5 mm).
  Five of the eight red tests of P38 stay red, one more test is red, and the cause of the
  planner parity red is not proven (B2.4, cause 4).
- Predictions that were false: PA-12 (rivmap100 moves), PB-4 (five of eight), PB-5 (the golden
  row), PB-11 (the lib test).

## A1. The defect and the instrument

- P39 section 2.4: an entry leg that the model clip lifts ships a chord between two clip
  samples. When the two samples stand on the two sides of a wall of the model, the chord goes
  through the top edge of the wall. On the Wanaka back at HEAD: 33 replay nodes of one entry of
  id 2 are under the target (lowered over one node), to 1.72 mm.
- The clip of a ramp (`dressup::entry_descent::clip_polyline_to_floor`, and for a tool with a
  centre core `replan_descent_at_limit`) reads the drop-cutter floor each `ENTRY_CLIP_SPACING_MM`
  (0.5 mm). The clip of a helix (`emit_helix`) reads it each 10 degrees of turn. No door reads
  the floor between two samples.
- The product does not see the cut. The model gouge guard (`compute::model_gouge::measure`)
  samples a move at a quarter of the tool diameter, both ends included. A chord 0.5 mm long gets
  its two ends, and the two ends are on the floor. The HEAD run of setup "1 Back" has no
  `safety.cuts_into_model` line (`p39/out_final/summary.json`: the string is not in the file).
- The instrument of the crate test is the rule of that guard for a 3D Rough (`Allowance`), with
  a finer sample step:
  - the probe is the tool less the chord tolerance of the op on its radius
    (`allowance_probe_radial_tolerance_mm`);
  - the limit is `CL Z + leave - chord tolerance`;
  - a sample is a gouge when it is more than `MODEL_GOUGE_Z_TOL_MM` (0.02 mm) under the limit;
  - the sample step is `MODEL_GOUGE_TOL_MM` (0.01 mm), not a quarter of the diameter.
- The job instrument is `p39/entries.py` on the nodes of `p37/gapmap.py` (0.2 mm), not changed.

## A2. The reproduction (red at HEAD)

`--lib an_entry_chord_over_a_model_wall_holds_the_leave_p40` (`dressup/tests.rs`). The fixture
is a floor at Z 0 and a top at Z 12 with a wall at X 10. A 6 mm tool stands on the floor while
its centre is under X 7. Each entry starts on the floor part and goes over the line X 7. The
chord tolerance of the fixture op is 0.1 mm (the tolerance of the P39 sentry fixture).

The run at HEAD (`p40/red_A_lib.log`, `p40/red_A_lib2.log`), the deepest point of an entry chord
under the limit:

| Door | Under the limit |
|---|---|
| `emit_ramp`, the clip, legs of the other doors (`RampLegs::along`) | 0.253 mm |
| `emit_ramp`, the clip, planned legs of the 3D Rough (`whole_profile`) | 5.105 mm |
| `emit_helix` (radius 2.5 mm, pitch 3 mm) | 0.504 mm |
| `emit_ramp`, a tool with a centre core (`replan_descent_at_limit`) | not reproduced |

- The planned legs are the worst case. `whole_profile` ships the sample before a lifted run, so
  the chord over the wall is 0.5 mm long and 10 mm tall. The clip of the other doors ships a
  long chord from the leg start to the first lifted sample, and that chord is near the top when
  it comes to the wall.
- A tool with a centre core: not reproduced on a single wall. `replan_descent_at_limit` takes a
  direction only when each lap lowers the start column at the limit slope. On the 12 mm wall it
  took the direction -X. On a step of 0.3 mm (the second run) no sample was lifted: the out leg
  of the last lap is above the step, and a lap that the step lifts does not come down again. By
  the reading of the code a chord between two of its samples is still not read, so terrain with
  more than one rise can give the case. The fix covers the door with the same call.
- The rest ladder of a rest-driven surface pass (`pencil::plan_entry_ramp`) was not probed. Its
  laps run between the entry and one far point of a window; no floor is read between them.

## A3. The rule

R-A. Every chord of an entry that an emitter ships over a surface probe passes the chord lift
of the cuts and the links of a 3D Rough (`lift_chords_to_leave`, G-DRAPEGAP).

- The function moves to `surface/chord_lift.rs`, so that `dressup/` can call it. No copy.
- ONE function in `dressup/entry_descent.rs` (`hold_entry_chords`) owns the lifted profile of an
  entry. It takes the tool position before the entry and the points that the emitter is about
  to ship. It gives the points back with each chord lifted. It only raises a point, and it
  inserts a point where a chord is more than half the tolerance under the rest height. With no
  chord under the rest height the points are the same points (the same bytes).
- The inputs of the lift are a new field of `EntrySafety` (`chord_lift`: the probe tool and the
  tolerance). `None`: no lift.
  - The 3D Rough gives the reduced tool of the guard (`Adaptive3dGeometry::gouge_probe`) and its
    chord tolerance: the inputs of the lift of its cuts and links. With no reduced tool (a test
    fixture, a tool that cannot shrink) it gives `None`, as the cut pipeline does.
  - The dressup door (`apply_entry`, the surface ops) gives the tool of the probe and
    `2 x MODEL_GOUGE_TOL_MM`. Derivation: the guard of such an op reads the tool less
    `MODEL_GOUGE_TOL_MM` on its radius. The lift puts a point within half its tolerance (at
    least 0.01 mm) of a jump of the rest height. With the full tool and a tolerance of
    `2 x MODEL_GOUGE_TOL_MM` that distance is the radial tolerance of the guard, so the reduced
    tool of the guard reads no chord under its floor.
- The doors (each one is a call of the ONE function):
  1. `emit_ramp`, the clip (`clip_polyline_to_floor`): planned legs and other legs;
  2. `emit_ramp`, a tool with a centre core (`replan_descent_at_limit`). A raised point can make
     the next piece steeper than the core limit. So after a lift that changed a point the path
     passes `CoreZone::clamp_descents` (the order of the cut pipeline: lift, then clamp);
  3. `emit_ramp`, the rest ladder (`pencil::plan_entry_ramp`, a rest-driven surface pass);
  4. `emit_helix`: the turns, the spiral to the centre, the floor lap and the return.
- Not a door: the fold of a 2.5D ramp and the legs with no probe have no model surface. The
  peck ladder stays at the entry XY, which is draped.
- `whole_profile` (P39 D-5) is not part of this rule. The lift holds the model with it and
  without it.
- The planner stamp of a 3D Rough entry is the moves that `entry_ramp::emit` and
  `emit_bore_helix` write, so the stamp follows with no more code.

## A4. Predictions (written before the fix)

- PA-1 (the lib test): the four cases read at most 0.02 mm under the limit.
- PA-2 (the job, `entries.py`): the ramp nodes of id 2 under the lowered target fall from 33
  to 0. The ramp nodes under the raw target fall from 106, and not to 0 (the raw target reads
  the top of a wall on the node of the wall line).
- PA-3 (cut below the target after the two roughs): with the target lowered over one node it
  falls from 1.3 mm2 to at most 0.1 mm2 (33 nodes are 1.32 mm2). The raw area falls from 14.7
  mm2 by at most 4.2 mm2 (106 nodes).
- PA-4 (the depth rows of id 2): entry at most 2.5 mm (1.99 today); body 20.7; link 6.0;
  cleanup 3.0, each within 0.05 mm.
- PA-5 (time): id 2 stays within 2 s of 585 s. The setup stays within 5 s of 9 186 s. Id 2
  gets at most 200 more moves (3 lifted entries).
- PA-6 (collisions): 0, 0, 0 on each op. The two contact counts and the 24 export warnings
  stay.
- PA-7 (wood left): each zone of the gap map moves by less than 1 mm2.
- PA-8 (id 1, the 18 mm tool with a centre core): its G-code moves only in entries that go over
  a rise of the model. Its time stays within 5 s.
- PA-9 (goldens): no cell moves outside the 46 known cells and the 6 P39 cells of
  `adaptive3d/ramp_geodesic`. The 6 cells keep their P39 values (no leg of that fixture crosses
  a wall).
- PA-10 (FM1): no cell moves.
- PA-11 (sentries): the P39 sentry, `adaptive3d_entry_stock_aware`,
  `adaptive3d_emission_byte_parity` and the parity sentries stay green (their fixtures give no
  reduced tool, so no lift). The K5 session tests (the pocket, the plate, the bump) stay green:
  the lift only raises, and the clamp runs after it.
- PA-12 (rivmap100, the Geodesic Adaptive copy): the same G-code, byte for byte (no bore of it
  is lifted).

## A5. The rule as built (changed after the first run with the fix)

The rule of A3 was built as written, and the lib test stayed red in one case
(`p40/first_fix_reduced_probe.log`): the planned legs read 0.671 mm under the limit (5.105 mm
before). The other two cases were green.

- Cause: the lift puts a point within half the tolerance of a jump of the rest height that its
  own probe reads. With the reduced tool of the guard as the probe, that point stands up to
  0.05 mm past the jump that the guard reads. A chord that falls 7 mm in 0.3 mm is 0.67 mm
  under the floor of the guard there.
- The change: the entry lift reads the rest height with the cutter of the surface probe, the
  tool that the entry clip reads (`EntrySurfaceProbe::floor_z`). The jump of the full tool is
  one radial tolerance before the jump of the guard tool, so a point within half the chord
  tolerance of it holds the floor of the guard. `ChordLift` is the tolerance only.
- The hazard that the cut pipeline names for the full tool (a cut tangent to a wall reads the
  wall top) is not new for an entry: the clip already lifts each sample to the floor of the full
  tool.
- The 3D Rough turns the entry lift on when its cut lift is on (`EntryFrame::chord_lift`: the
  op has `gouge_probe`), with the chord tolerance of its cuts (`emission_chord_tolerance`). The
  emitter and the planner stamp both call that one function. A fixture with no `gouge_probe`
  has no lift on its cuts and none on its entries.
- The dressup door: `ChordLift::SURFACE_OP` (`2 x MODEL_GOUGE_TOL_MM`, A3).
- The same finding applies to the cut pipeline by reading: `emitted_cut_path` gives the lift the
  reduced tool, so a steep cut chord can sit under the floor of the guard within half the chord
  tolerance of a wall. It is not measured and not changed here (decision DA-3).

The second run (`p40/green_A_lib.log`): `--lib -- _p40 _p39 clamp_changes_only g_drapegap
g_centrecut two_cut_pieces dressup::`: 169 passed, 0 failed.

The 3D Rough door, end to end (added after the first hand-off check, because no adaptive3d
fixture states a `gouge_probe` and the job run was not possible):
`a_lifted_ramp_leg_over_a_wall_holds_the_model_p40` in the P39 sentry
(`tests/a_ramp_entry_keeps_to_its_planned_footprint_p39.rs`).

- The fixture: the ledge of that sentry with a centre boundary that is a strip along X (X -5.9
  to 15, Y -1 to 1). Inside the strip no direction has an unlifted run of 3 mm, so `plan`
  takes rule 3 (the longest run that the centre may take): +X, over the ledge. The op states
  the reduced tool of the guard, as the session does.
- Contour Parallel: 418 ramp moves, 346 end on the ledge; the deepest chord is 0.000 mm under
  the limit of the guard; the one-lap bar of P39 holds with the lift on (1.83 mm).
- Red proof (`EntryFrame::chord_lift` gives `None`; `p40/door_red.log`): 1.109 mm under the
  limit, on the fall of move 457 from (-2.835, -0.99, 10.0) to (-3.321, -0.99, 0.127).
- Geodesic Adaptive emits no move inside the strip, so the test runs Contour Parallel only.
- The planner stamp calls the same emitter; no test compares the stamp with the emitted path
  with the lift on (the parity sentries state no `gouge_probe`).

The K5 test (task item A4): `a_centre_cutting_tool_keeps_the_surface_slope` is removed from the
sentry. `--lib the_descent_clamp_changes_only_a_tool_with_a_centre_core_k5` (`adaptive3d/tests.rs`)
holds the claim on `clamp_descents_to_limit`: a path that goes down 5 mm in 5 mm comes back
bit for bit for a centre-cutting tool, with an empty lead; the same tool with a centre core
gets it raised to its limit slope. The injected defect (a clamp that gives every tool the core
of the probe tool) makes it red (`p40/red_k5_injection.log`).

## A6. Part A: measured (2026-11-17)

### A6.1 Setup "1 Back" (task item A5): NOT MEASURED

The run of setup "1 Back" with the part A CLI (`p40/cli_A`, md5
`a20b8651564bca0f2eece5891e364b4f`) was not made. From 18:40 to the end of this work `free -g`
showed 12 to 17 GB available with no job of this session in work (the rule is 18 GB or more).
A Python process of another project (`k1-stats/preview.py`, PID 230942) grew from 7.2 to 9.6
GB in that time. So these are not measured after the fix: the ramp nodes under the target, the
cut below the target, the entry depth rows of id 2, the time, the collisions and the wood left
per zone. The "before" values (HEAD) are in P39 section 6.1, column "After", and
`p40/meas.py head` reads them again from the HEAD run: 14.7 mm2 under the raw target after the
two roughs, 1.3 mm2 (32 nodes, to 1.72 mm) under the target lowered over one node; id 2 585 s.

To make the run when the machine has the memory (one setup per run):

```
p40/jobA.sh p40/cli_A A        # writes out_A.*, entries_A.txt, meas_A.txt, lim_A.txt, rep_A.txt
```

`jobA.sh` is a scratch script. Its steps, for a run with no scratch dir (`target/release/rs_cam_cli`
is the same binary as `p40/cli_A`):

1. `rs_cam_cli project <proposal> --setup "1 Back" --output-dir out_A --emit-gcode out_A.nc --summary`
2. `python3 phases/p37/gapmap.py replay out_A.nc out_A.npz`
3. `python3 phases/p39/entries.py out_A.nc "Back rest 6 mm" out_A/tp_2_*.json 3.0 entries_A.json 6.0`
   (the line "ramp nodes under the target (lowered over one node)": 33 at HEAD)
4. `python3 phases/p40/meas.py A` (the cut below the target, raw and lowered, and the times)

### A6.2 rivmap100, the Geodesic Adaptive copy (0.2 GB, not the job)

`p40/cli_A` on `p38/riv_geo.toml` against the HEAD file (`p39/out_riv_after.nc`): the G-code is
NOT the same. 34 places differ, all in the helix bores of "3D Rough ladder 8 > 2": 6 710 to
6 780 moves, `total_runtime_s` 1 227 to 1 229, 0 collisions in both. At each place a helix step
between two floor reads gets a lift point; the lowest Z of a changed place rises by 0.063 mm at
most. So the helix door had the defect on terrain (a chord of 10 degrees under the model plus
the leave by more than half the chord tolerance), and P39 section 2.7 ("a fitted bore stays in
the region, so a lift is not expected") does not hold on this job.

### A6.3 Gates of the part A tree

Crate tree `2d72aae36528bad868a02e0fd1a4e96aa346aa98` (scratch index). Logs `p40/gate_A1_*.log`
(the first set, before part B) and `p40/gate_A2_*.log` (the second set, on the tree after the
prototype was reversed; section A8).

- adaptive3d sentries (32 binaries, `test-support,heavy-tests`, `--no-fail-fast`): all green.
  The K5 sentry has 4 tests (the red test is removed, item A4).
- dressup sentries (13 binaries): all green.
- op golden: red with 52 changed cells: the 46 known cells and the 6 P39 cells of
  `adaptive3d/ramp_geodesic`, each with its P39 value (line for line the list of the P39 gate
  run). No other cell.
- FM1: 2 passed. No cell moves.

### A6.4 `whole_profile` on every door (P39 D-5, task item A3)

Not turned on. The measured reason:

- Part A does not need it. With the chord lift the legs of the other doors (`RampLegs::along`,
  the clip that ships no sample before a lifted run) hold the limit of the guard on the wall
  fixture (0.253 mm under the limit before, at most 0.02 mm after; the lib test asserts it).
- What it would change is the load of a lap, not the model: P39's lib test holds that the legs
  of the other doors take more than 6 mm in one piece on the ledge fixture, and the planned legs
  2 mm.
- Not measured: the golden cells that move with `whole_profile` on for every door, and a job
  through the dressup door. The doors that would move are the surface ops with a stated ramp
  entry (`EntryProbeRule::SurfaceFixed`, `SurfaceFromConfig`); their defaults state no ramp
  (`finishing_defaults_have_no_ramp_entry_r10`).

## A7. Predicted against measured (part A)

| Prediction | Result |
|---|---|
| PA-1 the lib test | TRUE after the change of A5. As written in A3 (the reduced tool as the probe) it was FALSE for the planned legs: 0.671 mm. |
| PA-2 to PA-8 (the job) | NOT MEASURED (A6.1). |
| PA-9 goldens | TRUE: 46 known cells and the 6 P39 cells with their P39 values. |
| PA-10 FM1 | TRUE. |
| PA-11 sentries | TRUE: all green. |
| PA-12 rivmap100 copy, the same bytes | FALSE: 34 helix places move (A6.2). |

## B1. The one-pass rule on top of P39 and part A: what changes, and the predictions

Written before a run of the prototype. The prototype is `P38_W1.patch` (made against
`c99a5ac37`) ported onto the tree after part A. It does not land. "Before" in part B is the
tree after part A.

### B1.1 What W1 must change

1. The entries. P38 section 5.3: W1 read an entry at its own column, and the emitter shipped two
   legs along +X that no part of the planner read. P39 closed that: `entry_ramp::plan` chooses
   the legs, `plan_entry` reads the stock over their swept footprint, and the ramp starts above
   it, so a lap takes one lap of stock (2 mm). So W1 needs no entry code of its own. Its read
   at the entry column stays: it is the read of the first point of the cut that follows. An
   entry that W1 adds for a kept piece is a `Rapid` segment, and `plan_entry` plans its ramp
   as it plans every entry.
2. The read of the disc edge. W1 counted each cell with its centre inside the disc, the edge
   included (`d_sq > r_sq` is false at one radius). That is wrong beside a wall of the model,
   by reading of the code:
   - a tool that is tangent to a wall has the cells of the wall line at about one radius;
   - the planner stamp lowers such a cell by its covered part only (`ray_blend_above` with the
     coverage of the cell, about one half on the edge), so the cell keeps a part of its height
     at each level;
   - at the second level the cell stands more than one Depth/Pass plus the clearance above the
     cutter, and W1 keeps the tangent move off. P38 section 10 saw this on the fixtures: "at the
     edge of the mesh footprint", stock to 15 mm above the cutter.
3. The strict read. A cell counts when its centre is at least HALF A CELL inside the edge of the
   disc. Derivation: a cell is a square of one cell size around its centre, so its side is half
   a cell from its centre. With the centre half a cell inside the edge, the tool covers the cell
   to its far side along the radius. A cell with its centre nearer to the edge holds the edge
   of the disc; beside a wall it is the wall line. No constant: `0.5 x cell_size`
   (`one_pass::edge_inset_mm`), 0.1 mm on this job (cell 0.2 mm).
4. The limit of the strict read, by reading of the simulation: the depth of a sample
   (`max_penetration`) is the largest length that one stamp removes from a cell with its centre
   inside the disc, and a cell on the edge loses its covered part of its height. So a cell row
   that the strict read does not count can still give a depth reading: the covered part (one
   half to less than one) times the height of the stock above the cutter. An inset cannot
   remove this: each inset leaves a row that is partly covered.
5. A probe switch of the prototype only (`P40_ONE_PASS_READ=coverage`): the read counts each
   cell with its centre inside the disc at its covered part times its height. That is the
   quantity of the simulation. On a wall line it converges to one Depth/Pass (the residue of a
   cell with covered part `f` under level steps of `d` goes to `d / f`, and `f x d / f = d`),
   so it does not keep a tangent move off. One more run of the setup measures it.

### B1.2 Predictions (the strict read, unless the line names the switch)

- PB-1 (the body row of id 2): it falls from 20.7 mm, and it does NOT hold 6.5 mm. The rows
  that the strict read does not count give readings between 6.5 and 19.4 mm (15/16 of 20.7).
  With the switch `coverage` the body row is at most 6.5 mm.
- PB-2 (the entry row of id 2): at most 2.5 mm (P39 holds it; 1.99 mm before).
- PB-3 (the link and cleanup rows of id 2): 6.0 and 3.0 mm, within 0.5 mm.
- PB-4 (the 8 red tests of P38 section 10). Each fixture has stock beside the mesh footprint
  that no centre takes, so its cells at one radius are the case of point 2:
  1. `--lib planner_sim_dexel_parity_geodesic_adaptive`: goes green;
  2. `adaptive3d_boundary_clear_parity`, `boundary_clear_parity_geodesic_adaptive`: goes green;
  3. `adaptive3d_by_area_matches_global_stock`, `by_area_final_stock_equals_global`: goes green;
  4. `adaptive3d_emission_byte_parity`, `contour_parallel_by_area`: goes green;
  5. the same, `geodesic_adaptive`: goes green;
  6. the same, `geodesic_adaptive_boundary`: goes green;
  7. `adaptive3d_plans_every_slice_piece_g_adapt3dpieces`, `geodesic_adaptive_clears_both_lobes`
     (a rapid through stock): not predicted; its cause was not known. If it stays red, the move
     and the cause are read;
  8. `geodesic_adaptive_keeps_the_research_geometry_s7_4`, "island pocket, two levels": goes
     green.
- PB-5 (the golden cells): `adaptive3d/ramp_geodesic` keeps the values of the tree after part A
  (W1 moved 9 cells of it). No other new cell.
- PB-6 (cut below the target after the two roughs): it does not rise against the tree after
  part A by more than 0.5 mm2 (raw) and 0.1 mm2 (lowered one node). W1 added 6.8 mm2 with its
  new entries; the entries are now planned and their chords lifted.
- PB-7 (time of id 2): between 0 and 80 s more than the tree after part A (W1: +58 s).
- PB-8 (the count of new entries): between 5 and 50 more ramp entries than the tree after part
  A (W1: +49). The strict read keeps fewer moves off than W1 (955): between 100 and 600.
- PB-9 (what the op leaves): the mech and slit pockets keep at most 60 mm2 more (W1: +48.2).
  The other zones move by less than 5 mm2.
- PB-10 (collisions): 0, 0, 0 on each op; the contact counts stay.
- PB-11 (`--lib _p38`): the three tests of W1 stay green with the strict read (the rib of the
  fixture is 1.5 mm wide, three cells; the tool meets it head on).
- PB-12 (FM1): no cell moves.

## B2. The prototype and its results

### B2.1 The port

- `P38_W1.patch` applies onto the tree after part A with `patch -p1` (offsets and fuzz only; one
  hunk of `adaptive3d/CLAUDE.md` by hand). No planner code of W1 had to change for P39.
- The change of B1.1: `one_pass.rs`, `edge_inset_mm` (half a cell) and the read radius
  `R - edge_inset_mm`; the probe switch `P40_ONE_PASS_READ=coverage` (`covered_part`).
- `P40_B.patch` (on top of `P40_A.patch`). The prototype was built and gated on the crate tree
  `c978e3eb50cc15da13032502ac6bb19c1de2894e` (scratch index): the part A tree of the first gate
  set plus the patch, before the two late test-file edits of part A (A8). A copy is in
  `phases/p40/not_landed/`.

### B2.2 The lib tests (`--lib -- _p38 _p37 _p39 _p40 planner_sim_dexel_parity adaptive3d::`)

| Read | Result |
|---|---|
| strict (half a cell) | 81 passed, 2 failed: `a_level_does_not_cut_what_the_levels_above_left_p38` ("a sample cuts 11.50 mm deep; one Depth/Pass and the clearance are 3.5 mm") and `planner_sim_dexel_parity_geodesic_adaptive` (66 interior cells `planner_higher`, 0 `sim_higher`) |
| `coverage` switch | 82 passed, 1 failed: `planner_sim_dexel_parity_geodesic_adaptive` (67 interior cells) |

- PB-11 is FALSE, and it is the case of B1.1 point 4. The strict read does not count the cell
  row with its centre between `R - cell / 2` and `R`. A pass beside the rib covers 50 to 100 %
  of that row, and the simulation takes that part of the 11.5 mm column in one sample. So the
  strict read, as the lead stated it, does not hold the rule on the fixture of the rule.
- The `coverage` read holds the three `_p38` tests.

### B2.3 The five sentries that were red in P38 (both reads give the same reds)

| Test | P38 W1 | Prototype | Prediction PB-4 |
|---|---|---|---|
| `--lib planner_sim_dexel_parity_geodesic_adaptive` | red (70 cells) | RED (66; 67 with `coverage`) | FALSE |
| `adaptive3d_boundary_clear_parity`, `boundary_clear_parity_geodesic_adaptive` | red | green | TRUE |
| `adaptive3d_by_area_matches_global_stock`, `by_area_final_stock_equals_global` | red (9 cells) | RED (9 cells, the worst 1.5 mm) | FALSE |
| `adaptive3d_emission_byte_parity`, `contour_parallel_by_area` | red | RED (382 to 417 moves; 425 with `coverage`) | FALSE |
| the same, `geodesic_adaptive` | red | RED (1 455 to 1 485; 1 470) | FALSE |
| the same, `geodesic_adaptive_boundary` | red | RED (864 to 905; 895) | FALSE |
| `adaptive3d_plans_every_slice_piece_g_adapt3dpieces`, `geodesic_adaptive_clears_both_lobes` | red | RED (1 rapid collision) | not predicted |
| `geodesic_adaptive_keeps_the_research_geometry_s7_4`, "island pocket, two levels" | red | green | TRUE |

So the cells at one radius were the cause of two of the eight reds, not of all. PB-4 is FALSE
for five.

### B2.4 The cause of each red, read with a probe

The probe (`P40_PROBE=1`, temporary code, not in a patch) printed each move that the rule keeps
off, with the worst cell of the read, and the moves of the emitted toolpath at a failure.
Logs: `p40/probe_lobes2.log`, `probe_byarea.log`, `probe_byte.log`, `probe_parity.log`.

1. `geodesic_adaptive_clears_both_lobes` (SAFETY: a rapid through stock).
   - The move: move 995 of the toolpath, a rapid (`Linking`) straight down from
     (-7.920, -3.966, 15.0) to (-7.920, -3.966, 8.819). Move 996 is the `EntryPlunge` to
     Z 6.575. Moves 993 and 994 are the retract and the rapid from the end of the piece before.
   - It is the entry of a kept piece. The rule split one cut of 286 points at level Z 4.5 into
     6 pieces (23 read points kept off; the worst cell stands 4.79 mm above the cutter at
     (-4.75, 4.75), top 9.29). The piece that starts at (-7.920, -3.966) got a new entry.
   - The cause: the planner reads the stock for the rapid of an entry on the cell centres
     inside the tool disc (`plan_entry`: conservative top 8.319, plus the 0.5 mm clearance =
     8.819). The planner stock within one cell past the disc stands at 9.225 (the probe:
     "top in R 8.211, in R+cell 9.225"), 0.4 mm above the end of the rapid. The simulation
     reads more cells than the planner: `stock::collision::RapidClearanceCheck::strikes` has a
     second channel over "every visited cell" of the disc, less a rim tolerance (read in the
     code). That the cell at 9.225 is the cell that the check struck is inferred from these two
     reads; the check was not probed.
   - The read is on the planned points, at Z 4.50. The emitter ships the points of that
     place at Z 4.999 to 6.469 (moves 987 to 992: the drape and the leave). The worst cell is
     4.79 mm over the planned Z and 2.82 mm over the highest shipped Z. The Depth/Pass of the
     fixture is the rule value of the tool and was not read, so it is not settled that this
     split fires only because of the planned Z.
   - Why the rule makes this: an entry of a kept piece stands at the first read point that is
     not kept off. So it stands one read step (one cell) from stock that is too tall, by
     construction. An entry of HEAD does not stand there.
2. `by_area_final_stock_equals_global` (Contour Parallel, By Area against Global).
   - The test makes two runs in the By Area order and then one in the Global order. The probe
     log holds two blocks, the same line for line, and no third block. So the rule fires in
     the By Area order only: 3 cut splits in each run (117, 283 and 381 read points kept off;
     for example a cut of 376 points at Z 2.0, the worst cell (9.00, 5.00), top 8.00, 2.750 mm
     from the read point (9.00, 7.75, 2.00), with a strict radius of 2.75 mm). In the Global
     order no move is kept off.
   - So the stock is not the same in the two orders (9 owned cells, to 1.5 mm): By Area brings
     a level to stock that the Global order has cut before that level comes.
   - Not read: what the cell (9.00, 5.00) is against the walls of the pockets, and the Z at
     which the kept-off points ship.
3. `adaptive3d_emission_byte_parity` (three cases): the rule fires in each fixture (51 cut
   splits and 6 dropped entries in the six runs of the probe log), so the count of moves
   rises. The worst cells are 1.55 to 1.75 mm from the read point with a tool radius of 2.0 mm
   (the strict radius is 1.75): cells of the rim of the fixture stock (X or Y at 12.5 and 13.0)
   that the levels above lowered in part, standing 3.5 to 6.3 mm above the cutter (the limit
   is 3.5). The sentry holds the bytes of the motion; it is red wherever the rule fires.
4. `planner_sim_dexel_parity_geodesic_adaptive`: the 66 interior cells where the simulation
   removed more than the planner are 3.19 to 3.68 mm from the moves that lower them (tool
   radius 3.175 mm; cell 0.53 mm): cells on the edge of the disc. The moves are an
   `EntryPlunge` and the first cut moves after it, at Z 7.5 and 10.5, on the rim of the fixture
   (Y -20.0 to -21.6): the entries and the first moves of pieces that the rule made (4 cut
   splits, 3 dropped entries). The mechanism (why the planner stamp of those moves lowers an
   edge cell less than the simulation) is NOT proven.

What the reds have in common: the rule reads a PLANNED point and a planner cell, and three
other parts of the system read the same place another way:

- the emitter drapes the point (cause 1: planned Z 4.50, shipped Z 4.999 to 6.469);
- an entry reads its rapid floor on cell centres inside the disc, and the rule puts the entry
  one cell from tall stock (cause 1);
- the simulation takes the covered part of an edge cell (B2.2, and probably cause 4).

### B2.5 The other gates of the prototype (the strict read), and the table

| | Before (the tree after part A) | Prototype |
|---|---|---|
| `--lib -- _p38 _p37 _p39 _p40 ..` | no `_p38` test; the rest green | 2 red (B2.2) |
| adaptive3d sentries (32 binaries, `--no-fail-fast`) | 72 passed, 0 failed, 2 ignored | 66 passed, 6 failed, 2 ignored |
| op golden, cells outside the 46 known | 6 of `adaptive3d/ramp_geodesic` (the P39 values) | 9 of `adaptive3d/ramp_geodesic` |
| FM1 | 2 passed | 2 passed |
| rivmap100 Geodesic Adaptive copy | 6 780 moves | the same G-code, byte for byte (both reads) |
| Setup "1 Back": the rows of id 2, the time, the entries, the cut below the target, the wood left | NOT MEASURED (A6.1) | NOT MEASURED |

The 6 red sentry tests: the five of B2.3 and one more,
`adaptive3d_entry_stock_aware`, `dome_entries_are_stock_safe_on_every_style`: "dome, Plunge:
rapids enter material: [(900, 0.418), (927, 0.418)]". Two rapids go 0.42 mm into material. It is
the class of cause 1 of B2.4 (the rapid of an entry of a kept piece; 0.4 mm there). It was not
read with the probe. P38 did not list it.

The golden row (not blessed; the tree after part A in the middle column):

| `adaptive3d/ramp_geodesic` | golden | part A | prototype |
|---|---|---|---|
| `arc_count` | 13 | 13 | 10 |
| `cutting_length_mm` | 2283.957 | 2296.031 | 2703.516 |
| `linear_count` | 1091 | 1089 | 1222 |
| `move_count` | 1113 | 1111 | 1241 |
| `rapid_length_mm` | 114.908 | 114.908 | 112.738 |
| `span_count` | 92 | 92 | 143 |
| `gcode_hash` | `2ae632031de60cab` | `1bfe3e7a254d48a3` | `d71d9cc9b23264d1` |
| `moves_hash` | `be39e8dff5872a09` | `30814f8ee1004814` | `610d8b0b2bdc9ffb` |
| `spans_hash` | `3398534541b08672` | `8875c9681caa5486` | `220f12d41f3b1714` |

Predicted against measured:

| Prediction | Result |
|---|---|
| PB-1 the body row of id 2 | NOT MEASURED. The lib fixture shows the predicted case: 11.5 mm with the strict read. |
| PB-2, PB-3 the entry, link and cleanup rows | NOT MEASURED. |
| PB-4 the 8 red tests of P38 | TRUE for 2 (boundary clear parity, the research geometry), FALSE for 5, one not predicted and still red. One more test is red. |
| PB-5 the golden row keeps the part A values | FALSE: 9 cells move; the cut length grows by 18 %. |
| PB-6 to PB-10 (the job) | NOT MEASURED. |
| PB-11 `--lib _p38` green | FALSE (B2.2). |
| PB-12 FM1 | TRUE. |

### B2.6 Which later op meets the wood that the rule leaves (task item B4)

NOT measured for the P40 prototype: no run of setup "1 Back" was possible (section 0). The
only measured data is the W1 run of P38 (`p38/out_w1`, the read of P38 on the code before P39),
read with `p40/left.py` (the replay nodes where the stock after id 2 stands more than 0.3 mm
above the stock of the run before the rule) and with the depth rows of `summary.json`:

- W1 left 202.3 mm2 (5 058 nodes, 1 693 mm3, to 20.70 mm) above the HEAD run after id 2.
- "Cavity rest pocket 6 mm" (id 40) lowers 55.6 mm2 of it. The tallest column that it takes
  there is 8.74 mm (stock 25.24 to 16.50 at G-code (351.0, 118.8)), in its own passes: its body
  row stays 2.25 mm and its entry row 2.25 mm.
- "River channel 6 mm, pass 7 of 7" (id 11) lowers 1.6 mm2 (a column of 4.83 mm); its body row
  stays 1.84 mm.
- "Day strip slit V60" (id 16) lowers 0.6 mm2 (a column of 4.17 mm at G-code (98.2, 92.0));
  its body row goes from 4.60 to 4.77 mm (the bound is 4.51 mm, advisory).
- 144.6 mm2 still stands at the end, to 20.70 mm, in the mech and slit pockets. No later op of
  the setup cuts there.

So on the W1 data no later op meets that wood deeper than its own pass, except the slit V-bit
by 0.17 mm; most of the wood stays in the part. These numbers are for W1, not for the
prototype of this note.

## B3. Recommendation: (b), the rule belongs in the region grid

The evidence, each point measured in B2:

1. The segment door reads the planned point. The emitter then drapes the point. On the
   two-lobe fixture the points of the split ship 0.5 to 1.97 mm above the planned Z (planned
   4.50, shipped 4.999 to 6.469). To read the point that ships, the door must split a path
   after drape, simplify and blend, and that path is built from the tool position, which the
   split changes.
2. A piece that the rule keeps starts one read step from stock that is too tall, and it needs
   an entry there. The entry read (`plan_entry`, cell centres inside the disc) then gives a
   rapid through stock in the simulation (the two-lobe fixture, move 995). The door would need
   a second rule for where a kept piece may start.
3. No inset from the disc edge agrees with the judge. The half-cell inset fails the test of the
   rule itself (11.5 mm). The read that agrees (the covered part of a cell times its height)
   needs the coverage of the stamp in the planner read.
4. The result of the rule changes with the order of the cuts (the rule fires in the By Area
   order and not in the Global order: 9 cells, to 1.5 mm).
5. The rule changes the emitted motion on three byte fixtures and (P38) on a golden row, with
   more entries (P38: 134 to 183 on the job). Each extra entry is a new place for causes 1 to 3.

In the region grid the same rule should have none of these problems. This is by reading of
the code; nothing of (b) is built or measured:

- the grid knows the Z that ships for each centre (`intended_floor::level_floor_z`: every level
  drapes), so the read is on the shipped height;
- a centre whose disc meets stock more than one Depth/Pass above the cutter is not a region
  cell of that level. The 2D planner never plans the move, so no cut is split and no entry is
  added; a region entry stands inside its region, away from the edge;
- the region outline is inset and smoothed (0.3 + 0.3 mm), so the path keeps off the edge cell;
- the read can be the read of the material test of the grid (P38 section 8.1, point 1: one read
  for "material" and "too tall"), with the coverage of the stamp.

If the lead takes (a), the door, this remains: read the shipped path (point 1); an entry rule
for kept pieces, or a read of `plan_entry` that covers the cells that the disc covers in part
(point 2); the `coverage` read in place of the inset (point 3); a ruling on the order
dependence (point 4); the byte fixtures, the golden row and the planner parity test (points 4
and 5, and B2.4 cause 4, which is not proven).

What part B does not answer: the body row of id 2 with the prototype (PB-1), the time, the
entries and the wood left on the job (PB-6 to PB-10). They need the run that was not possible.

## A8. The last gate set of part A, on the tree after the prototype was reversed

`git apply -R P40_B.patch`; the crate tree on a scratch index is again
`2d72aae36528bad868a02e0fd1a4e96aa346aa98` (the tree of the first gate set), and
`adaptive3d/one_pass.rs` is gone. Logs `p40/gate_A2_*.log`, `p40/chainA2.log`.

- core `--lib`: 2 845 passed, 1 failed, 13 ignored. The red is the known one
  (`feeds::envelope::tests::the_envelope_meets_its_time_budget`).
- other core sentries (14 binaries): 86 passed.
- `rs_cam_mcp`: 39 passed. `rs_cam_cli`: 17 binaries, 113 passed.
- viz `--lib` (`--test-threads 2`, no GUI process): 705 passed, 2 ignored.
  `mcp_wire_surface_pin`: 2 passed.
- full clippy (`--workspace --all-targets`, the three core features, `-D warnings`), first run:
  ONE error: "this lint expectation is unfulfilled", `clippy::indexing_slicing` in the K5 sentry
  file (the test that item A4 removed was its only use). The expectation is removed. The crate
  tree is then `e076f4bb2996ebdf4b6a21901691eb7c66101ef0`; it differs from the gate tree by that
  one line of one test file. On that tree the K5 sentry ran
  again (4 passed) and the full clippy ran again: clean (`p40/gate_A3_*.log`).
- After that the P39 sentry got its fourth test (A5, the 3D Rough door) and
  `adaptive3d/CLAUDE.md` one clause (crate tree `897f839bc48f5647f405d1631fb6149cfc8d238b`). On that last tree: the P39 sentry 4 passed, clippy on the
  two changed test files clean, `folder_maps_name_real_files_f140` green
  (`p40/door_final.log`, `door_clippy.log`, `door_f140.log`). No `.rs` file under `src/`
  changed after the gate sets, and the release CLI still has the md5 of `p40/cli_A`.
- `rustfmt --check` on each changed file: clean.
- The release CLI, built again from the tree after the reverse: md5
  `a20b8651564bca0f2eece5891e364b4f`, the md5 of `p40/cli_A` (built before the prototype). So
  `target/release/rs_cam_cli` is the part A build. The job was not run with it (section 0).
- With the first gate set (A6.3): adaptive3d sentries 72 passed, 2 ignored; dressup sentries
  83 passed; op golden 52 cells (46 known and 6 of P39); FM1 2 passed.

## 9. Decisions for the lead

Part A:

- DA-1 (the job): run setup "1 Back" with `p40/cli_A` when the machine has 18 GB (`jobA.sh`),
  and read `entries_A.txt` (the ramp nodes under the lowered target: 33 at HEAD, 0 predicted)
  and `meas_A.txt`, before the fix is called proven on the job.
- DA-2 (the gate of the entry lift): the 3D Rough turns the entry lift on with its cut lift
  (`gouge_probe` present), so a fixture with no `gouge_probe` keeps its bytes. The other choice
  is always on for a 3D Rough; then the byte parity fixtures and goldens can move wherever an
  entry chord was under the model. The safety half: `rough_3d` gives no `gouge_probe` when the
  reduced tool does not build (`build_cutter` fails on the tool less its tolerance). The entry
  lift is then off in the product, though it reads the full cutter and does not need the
  reduced tool. Ruling wanted; "always on" closes that case.
- DA-3 (the cut pipeline, by reading, not measured): `emitted_cut_path` and `emitted_link_path`
  give `lift_chords_to_leave` the reduced tool of the guard. The lift stops within half the
  chord tolerance of a jump that this same tool reads, so a steep cut chord can be under the
  floor of the guard there (the entry case measured 0.67 mm on a 7 mm fall). The guard samples a
  move at a quarter of the diameter and does not see it. It needs its own probe.
- DA-4 (the guard): `compute::model_gouge::measure` reads the two ends of a move shorter than a
  quarter of the diameter. It did not see the wall chord of P39 on the job. A finer sample
  step for a steep move is a design question (cost: one drop-cutter query per sample).
- DA-5 (`whole_profile` on every door, P39 D-5): left off (A6.4). It is a load question of the
  surface ops with a stated ramp entry, and it moves their G-code.
- DA-6 (the dressup door): its lift tolerance is `2 x MODEL_GOUGE_TOL_MM` (0.02 mm), so a
  ramp or helix entry of a surface op over terrain gets a point wherever a chord of 0.5 mm sags
  more than 0.01 mm. No golden row moved and the dressup sentries are green; no job was run
  through that door. Cost: the 13 dressup sentry binaries took 2.7 s of test time in the P39
  gate run and 6.8 s in this one (not at the same machine load; not separated by cause).
- DA-7 (the golden row): `adaptive3d/ramp_geodesic` keeps its 6 P39 cells; bless with P39's
  ruling (P39 D-2), nothing new from part A.

Part B:

- DB-1: take (b), the region grid, for the one-pass rule (B3). The segment door has five
  measured problems; three of them are reads of the same place by another part of the system.
- DB-2: if (a) is wanted, the list of B3 is the work, and the planner parity red needs its
  cause first.
- DB-3: the job numbers of the prototype (the body row, the entries, the time, the wood left)
  need a run of `p40/cli_B` (`jobA.sh p40/cli_B B`, and with `P40_ONE_PASS_READ=coverage` as a
  third argument for the second read). The binary is a prototype: it is in the scratch dir
  only, and `target/release/rs_cam_cli` is the part A build.

## Lead addendum, 2026-11-18: setup "1 Back" measured with part A

The lead made the run of A6.1 after a restart of the machine (the scratch files of P38 to P40
were lost; the "before" values are those of P39 section 6.1, column "After").
CLI md5 `a20b8651...`, 100 s, 12.5 GB.

| | Before (P39) | After (part A) |
|---|---|---|
| Ramp nodes of id 2 under the target (lowered one node) | 32, to 1.72 mm | 0 |
| Cut below the target after the two roughs, raw | 14.7 mm2 | 12.6 mm2 |
| The same, target lowered over one node | 1.3 mm2 | 0.2 mm2 (5 nodes, to 1.00 mm; not ramp nodes) |
| Entries of id 2 that the clip touched | 3 of 136 | 3 of 136 |
| Time of id 2 | 585 s | 585 s |

Not read again in this run: the wood left per zone and the entry depth rows.
