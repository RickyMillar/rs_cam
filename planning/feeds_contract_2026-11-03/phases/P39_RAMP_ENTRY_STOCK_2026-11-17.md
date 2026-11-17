# P39: the ramp entry of a 3D Rough cuts 14.5 mm deep (2026-11-17)

Job: `planning/rivmap_block_job/nz_wanaka_21_lead_proposal.toml` at `eeb6d3474` (the code of
`c99a5ac37`), on scratch copies. Setup "1 Back", toolpath id 2 "Back rest 6 mm" (3D Rough, rest,
6 mm flat tool, Depth/Pass 6.0, ramp entry at 3 degrees, `entry_clearance_mm` 0.5). Positions
are in the G-code frame (setup frame + (18.5, 8.5); Z is the same) unless the text says
otherwise.

Sections 1 to 4 were written before a line of the fix was written. Section 5 and later hold the
results.

## 1. The instrument

- `p39/entries.py` (new). It replays the emitted G-code on the nodes of `p37/gapmap.py` (0.2 mm).
- An entry is an `entry` item of the semantic trace of the op (`tp_2_*.json`). Its ramp is each
  move from the first fed move that leaves the entry XY to the end of the item.
- The planner read of an entry is `contact_z` = ramp start Z less `entry_clearance_mm`
  (`dressup::entry_descent::rapid_to_entry_top`).
- For each fed piece of a ramp the script reads the most that the piece lowers one node
  (`before - after`).
- The product measure is the same quantity. The depth row reads `axial_engagement_mm`. The
  simulation sets it to "the maximum per-cell material length removed by this stamp"
  (`dexel_stock/stamping.rs`, the doc of the stamp; `dexel_stock/simulation.rs`,
  `apply_subsegment_metrics`). A pure vertical move reads 0 there (its depth goes to
  `plunge_descent_mm`).
- So the 14.5 mm of the entry row is a real axial load: one sample removed a column of wood 14.5
  mm tall. It is not the depth of the slot that the ramp made in earlier laps.
- The run: `p38/out_base` (the CLI `cli_head`, built from the code of HEAD; the proposal copy is
  byte-equal to the file in the repo). Setup "3 Front" was run for this note with the same CLI.

## 2. W0: what the ramp entries of id 2 cut (HEAD)

### 2.1 The code that ships a ramp entry

- `clearing.rs::plan_entry` reads the stock on a disc of the tool radius at the entry XY:
  `rapid_floor_z` (the conservative top) and `contact_z` (the cell-centre top).
- `clearing.rs::stamp_emitted_segment` stamps an entry as a vertical descent at the entry XY with
  the envelope table ("entries stamp the envelope" in the folder file means this column).
- `path.rs::segments_to_toolpath` ships `dressup::emit_ramp(.., dir = (1.0, 0.0), .., None)`.
- `emit_ramp` plans a zigzag along +X. One lap descends `ENTRY_CLEARANCE` (2 mm). A leg is at
  most `ENTRY_CLEARANCE / tan(angle) / 2` = 19.08 mm at 3 degrees.
- The 3D Rough gives `emit_ramp` a surface probe (`EntryFrame::safety`). So the legs pass
  `clip_polyline_to_floor` (G-RAMPTERRAIN): each sample 0.5 mm apart that is under the
  drop-cutter floor goes up to the floor. The clip ships the lifted samples and the end point of
  each leg. It does not ship a sample that is not lifted.
- No part of that path reads the stock along the legs.

So P38 section 5.3 is right in one part (the leg leaves the disc that the planner read, and the
stamp is not the shipped shape). It is wrong in two parts: the leg is at most 19.08 mm long, not
114 mm, and the 3D Rough does not ship "blind" legs: the model clip changes them.

### 2.2 The counts

- 139 entry items. 134 have a ramp (23 after a retract, 111 after a keep-down link). 5 have no
  ramp move.
- 6 102 ramp pieces, 11 314 mm fed. 5 076 mm of that cuts nothing on the replay.
- The ramp descends 0.54 to 17.45 mm. The longest leg is 18.50 mm.
- The model clip touched 58 of the 134 entries (an entry with a flat or a rising piece).

A piece of the ramp as planned lowers a node by one lap or less (2 mm). That is the slot of the
ramp, by design (operator ruling 2026-09-24): class b. No class b piece is deep: 6 035 of the
6 102 pieces take 2.3 mm or less (2.0 plus the replay bar).

The pieces that take more, by cause:

| Class | Cause | Pieces > 6.3 mm | Fed mm | Entries | Deepest mm | Pieces > 2.3 mm | Fed mm | Entries |
|---|---|---|---|---|---|---|---|---|
| a | The entry is not clipped. The leg, at its planned Z, meets stock outside the read disc that stood above the planner read. | 1 | 18.4 | 1 | 7.18 | 4 | 53.8 | 4 |
| l | A flat or rising piece of a clipped entry. The leg rides the model floor (a ledge, the top of a wall) under stock that no lap lowered. | 32 | 101.7 | 7 | 16.16 | 59 | 178.7 | 16 |
| s | A steep piece (it goes down more than it goes along): the fall from a lifted run back to the start column. | 2 | 29.0 | 2 | 13.72 | 4 | 39.2 | 4 |
| r | A descending piece of a clipped entry, deeper than one lap. | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| b | The slot of the ramp, by design. | 0 | 0 | 0 | 2.0 | 0 | 0 | 0 |

- Deep entries (a piece over 6.3 mm = Depth/Pass plus the replay bar): 10 of 134, 35 pieces,
  149.1 mm fed. Over 9 mm: 9 entries. Over 12 mm: 8. Over 14 mm: 4.
- Over one lap (2.3 mm): 20 entries, 67 pieces, 271.7 mm fed.
- The planner read against the shipped footprint: in 51 entries the stock top under the swept
  footprint of the legs is more than 0.3 mm above `contact_z`; in 15 entries more than 6.0 mm;
  the most is 21.44 mm.

### 2.3 Three entries, read in the G-code

- Class a, item 36, entry (101.00, 100.35): the disc reads 16.55 to 18.79. The ramp starts at
  19.29. Its first leg goes to X 119.41 at Z 18.32 and meets stock at 25.50 at (121.4, 102.2):
  7.18 mm.
- Class l, item 77, entry (217.12, 50.45), entry Z 3.5: the disc reads 7.50. The ramp starts at
  8.0. The floor of the model is 11.0 from X 220.6 (a ledge) and 25.5 from X 230.9. The clip
  lifts the leg to 11.0 and the leg rides the ledge for 9.9 mm under stock that stands to 25.50:
  14.50 mm. This is the sample of the product row (14.5 mm). Item 83 is the same case (14.50).
- Item 44, entry (142.15, 43.81): the ramp starts at 26.0, above every column of its footprint
  (the stock top is 25.5). Each out leg ships as ONE chord from the start column up to the first
  lifted sample (158.70, 25.5), because the clip ships no sample that is not lifted. So no lap
  lowers the stock from X 156.7 to 158.2. The lap that goes under Z 11.0 then rides the 11.0
  ledge there and takes 12.7 to 14.05 mm in four pieces of 0.49 mm.

Item 44 proves that the rule "the ramp starts at or above the tallest stock of its swept
footprint" is not enough while the model clip can lift a leg.

### 2.4 Does a ramp cut the model, or leave the centre region?

- The model: yes, on the replay. 507 nodes that a ramp piece lowers are more than 0.3 mm under
  the raw target (20.3 mm2). The raw target reads the top of a wall on the node of the wall
  line. Against the target lowered over one node on each side: 94 nodes (3.8 mm2) in 12
  entries, to 4.13 mm.
- By the class of the piece: s 74 nodes, l 17, r 3. So the cut is made by the chord between two
  clip samples that stand on the two sides of a wall of the model: for example the piece from
  (230.447, 50.45, 11.0) to (230.941, 50.45, 25.5), and the fall from (345.844, 100.35, 25.5) to
  (344.869, 100.35, 18.08). The clip reads the floor at the two ends and not between them. The
  doc of `ENTRY_CLIP_SPACING_MM` covers the sag over a curved surface, not a wall.
- This cut is at HEAD. It is narrow (one sample step, 0.5 mm, at the top edge of a wall). It was
  not proven with a probe in the crate. P38 gives 21.4 mm2 under the target after the two
  roughs; 20.3 mm2 of the nodes of this section are ramp pieces of id 2.
- The centre region: no ramp point is outside the block outline less the tool radius (the
  nearest is 7.12 mm inside). The script does not read the 4 keep-outs of the op. The planner
  does not check a leg against the centre boundary; the session clip does, after generation.

### 2.5 Does the planner stamp equal the shipped ramp?

No.

- The planner stamps a column of the tool at the entry XY. The ramps lower 11 251 mm2 of node
  visits outside the read disc that the planner stock does not record.
- So a later entry of the same op reads stock that is not there. In 22 entries the planner read
  is more than 0.5 mm above the replay top of the disc (the most: 11.92 mm). Their ramps feed
  1 686 mm through air (22 s at the ramp feed).
- The planner is never ahead of the shipped path in this (it keeps more stock than the path
  leaves), so the stamp is safe. It is not the path that ships (G-SIMCORE says it must be).

### 2.6 The instrument against the product

- The replay reads the same value as the product for items 77 and 83: 14.500 mm (the row:
  judged sample 14.500 mm).
- The deepest piece on the replay is 16.16 mm (item 86, a class l piece at (156.6, 40.8)). The
  row says "deepest engaged depth 14.695 mm". NOT reconciled. Probable causes: the replay nodes
  are half a cell from the planner cells (P38 section 5.2), and 2 004 entry samples with a
  depth have no measured width and get no number.

### 2.7 Id 21, and the helix and plunge styles

- Id 21 "Front 3D rough" is in setup "3 Front", not in "1 Back". Its HEAD run (this note,
  `cli_head`): entry row 4.19 mm (Depth/Pass 5.532, the force cap 3.44 mm); body row 6.41 mm.
- On the replay (a flat stock top at 25.5 is assumed; the frame offset is 0): 278 ramps, the
  clip touched 192. No piece is deeper than 5.8 mm. 12 entries have a piece over one lap: class
  a 3 pieces (to 3.62 mm), class l 19 pieces (to 5.19 mm), class r 1 piece (2.34 mm).
- Limit of that read: 16 of the 278 items do not end at their start XY in the script (an item
  that passes its entry XY two times). They are in the counts.
- So id 21 has the same two causes, on terrain, under one Depth/Pass. Its `project.entry_load`
  is the force cap of the entry (3.44 mm), not a cut deeper than one pass.
- Plunge style (read of the code): `emit_peck_plunge` stays at the entry XY. The read disc is
  the footprint and the column stamp is the shipped shape. Not affected.
- Helix style on Geodesic Adaptive (P22b): the planner reads the disc `R + radius`, fits the
  bore to the region and stamps the moves that `emit_bore_helix` writes. Read and stamp cover
  the shipped shape. `emit_helix` lifts a point to the model floor, as the ramp clip does; a
  fitted bore stays in the region, so a lift is not expected. Not measured.
- Helix style on Contour Parallel: the read disc `R + radius` covers the turns. The stamp is the
  column (not the shipped shape, the safe side). A turn can be lifted by the model. Not
  measured. Not changed by this package.
- A helix with no room (`EntryBore::Fallback`) ships the same ramp as a ramp op. It has the
  defect.

## 3. The rule

R-1. A ramp entry of a 3D Rough is ONE shape (`RampLegs`: the direction and the longest leg).
The planner chooses it in `plan_entry`, where the stock, the model and the centre boundary are
in scope. The planner read, the planner stamp and the emitter use that one shape.

- A leg is free where the model floor is at or under the entry Z (so the model clip lifts no
  point of it) and the tool centre is inside the centre boundary. The free run of a direction is
  the distance from the entry to the first sample that is not free, at most the longest leg of
  `emit_ramp`.
- The directions are the eight of `replan_descent_at_limit`, +X first. The planner takes the
  direction with a full free leg and the lowest stock top under its swept footprint; with no
  full leg, the longest free run. A tie keeps the first direction, so an entry with room along
  +X and flat stock keeps its G-code.
- A run shorter than `fold_min_run_mm` (the tool radius, at least 1 mm, and the minimum leg of a
  tool that does not cut at its centre: the gate that the fold of a 2.5D ramp has) is no ramp.
  A tool that cuts at its centre then takes the peck ladder of the op (`emit_peck_plunge`). A
  tool that does not cut at its centre keeps the ramp of HEAD (it cannot plunge); that case is
  not covered.
- The read: `rapid_floor_z` and `contact_z` are read over the swept footprint of the longest leg
  that the shape permits (the disc of the tool along the leg), not over the disc at the entry.
  The ramp starts `entry_clearance_mm` above that read.
- With no lift and a start above every column of the footprint, each piece of the zigzag lowers
  a column by one lap or less (`ENTRY_CLEARANCE`, 2 mm). No new constant.
- The stamp: the planner stamps the moves that the emitter writes for the shape (the pattern of
  the P22b bore), with the cut table.
- The sentry replays the emitted path and holds: no ramp piece of a 3D Rough is flat or goes up,
  and no ramp piece removes a column taller than one lap plus the dexel allowance.

Change against the direction of the lead: the lead's rule (start above the tallest stock of the
footprint) is kept and one condition is added (no leg that the model clip can lift), because
item 44 shows that the first rule alone does not hold.

## 4. Predictions (written before the fix)

- P-1 (the entry row of id 2): the depth of the entry group is at most 2.5 mm (one lap 2.0 plus
  the 0.5 mm allowance). Today 14.5 mm.
- P-2 (the replay, `entries.py`): no ramp piece of id 2 is deeper than 6.3 mm (today 35 pieces
  in 10 entries). No ramp piece is flat or goes up (today 58 entries). At most 5 pieces are over
  2.3 mm (the replay nodes are half a cell from the planner cells).
- P-3 (the body, link and cleanup rows of id 2): the body row stays over 6 mm (20.7 mm today;
  P38 is not in the tree). The link row stays 6.0 and the cleanup row 3.0, within 0.5 mm.
- P-4 (time): the fed time of id 2 stays between 540 and 700 s (603 s today). Less air (1 686
  mm), and a ramp into a wall starts higher. Some entries become peck ladders: at most 20 of
  the 134.
- P-5 (the model): the ramp nodes under the lowered target fall from 94 to 0. The area under
  the target after the two roughs falls from 21.4 mm2 to under 5 mm2.
- P-6 (what the op leaves): the gap map area above the 6 mm reach surface after the last op
  moves by less than 100 mm2 in each zone. A ramp slot that left the region cut stock that the
  op does not cut now.
- P-7 (collisions): 0 collisions and 0 rapid collisions on each op; the contact counts stay the
  two known ones.
- P-8 (rivmap100, the Geodesic Adaptive copy of `rivmap100_live_0925.toml`): the op states a
  helix. Its G-code moves only where a bore falls back to the ramp, and through the planner
  stock after such a ramp. If no bore falls back, the file is the same byte for byte.
- P-9 (goldens): the row `adaptive3d/ramp_geodesic` moves (its entries, and the plan after
  them, because the stamp is now the slot). No other row outside the 46 known rows moves,
  unless its fixture states a ramp or a helix fallback; each such row is listed, not blessed.
- P-10 (FM1): no cell moves.
- P-11 (sentries): `adaptive3d_entry_stock_aware` stays green. A case of
  `adaptive3d_emission_byte_parity` moves only if it states a ramp entry. The parity sentries
  (`planner_never_ahead`, `plan_matches_merged_path`, `boundary_clear_parity`) stay green: the
  stamp is the shipped moves.
- P-12 (id 21 and the front, not run after the fix unless time permits): no ramp piece is flat
  or goes up.

## 5. The rule as built (changed after the first sentry run)

The rule of section 3 was built as written, and one sentry went red:
`adaptive3d_entry_stock_aware`, `session_rough_keeps_the_planner_order_for_its_entries` ("Ramp:
an EntryPlunge at move 2 goes 7.50 mm straight into stock"). On terrain an entry can stand on a
slope of the model: every direction lifts, so the entry had no free run and took the peck
ladder. A ramp op must not plunge. The rule was changed; no test was changed.

The rule that is in the patch:

- R-1a (one shape). `dressup::RampLegs` is the direction and the longest leg.
  `adaptive3d/entry_ramp.rs::plan` chooses it in `plan_entry`. `entry_ramp::emit` is the one
  writer of the moves: the emitter calls it, and the planner stamps the moves that it writes
  with the cut table (the pattern of the P22b bore). The leg cap is one function
  (`dressup::ramp_leg_cap_mm`).
- R-1b (where a leg runs). The tool centre stays inside the centre boundary and over the
  planner stock grid. The planner prefers a leg that the model clip does not lift (the model
  floor at or under the entry Z):
  1. a full leg that is not lifted: the direction with the lowest stock under its swept
     footprint (the shortest ramp); a tie keeps the first direction, +X first;
  2. else the longest run that is not lifted, if it is at least `ramp_min_run_mm` (the gate of
     the 2.5D fold: the tool radius, at least 1 mm, the minimum leg of a tool that does not cut
     at its centre);
  3. else the longest run that the tool centre may take (a lifted leg);
  4. else the peck ladder of the op, or for a tool that does not cut at its centre the +X ramp
     of HEAD (not covered).
- R-1c (the read). `rapid_floor_z` and `contact_z` are read over the swept footprint of the
  longest leg that the shape permits. The ramp starts `entry_clearance_mm` above that read.
- R-1d (a lifted leg). `clip_polyline_to_floor` has a mode `whole_profile`, on for the legs that
  the planner chose: it ships the sample before a lifted run and the sample after it. The path
  is then `max(planned, floor)` at each sample, the samples of each lap stand at the same XY,
  and a lap lowers a column by one lap or less. Every other door of `emit_ramp` ships the
  points that it shipped before (not changed, not measured).
- With R-1c and R-1d each ramp piece lowers a column by one lap or less (2 mm), lifted or not.
  No new constant.
- `--lib a_planned_ramp_leg_over_a_ledge_takes_one_lap_at_most_p39` holds R-1d: the fixture is
  the entry of section 2.3 (item 44). The planned legs read one lap or less. The same legs with
  the clip of the other doors read more than 6 mm, and the test asserts that too, so the
  fixture cannot go empty. (The terrain test of the sentry stays green with `whole_profile`
  off: it does not hold R-1d.)
- A full leg is always a run: the shortest run is `min(ramp_min_run_mm, leg cap)`. Without the
  `min` a 20 degree ramp (leg cap 2.75 mm) on a 6 mm tool had no run and every entry became a
  peck ladder (`one_entry_rule_two_emitters_p29_8` went red on the first gate run).

Not fixed (section 2.4): the chord between two clip samples on the two sides of a wall of the
model. The planner now avoids most lifted legs, so the case is rare; the clip itself is the
same between two samples.

The sentry `a_ramp_entry_keeps_to_its_planned_footprint_p39` (3 tests, both strategies). It
replays the emitted path on a fresh dexel stock. For each `EntryRamp` move it holds: the stock
under the tool along the move stands at most one lap (2 mm, plus 0.25 mm of dexel noise) above
the cutter; the tool centre is inside the centre boundary; on the two fixtures where a direction
has a run that is not lifted, no ramp move is flat or goes up. Red at HEAD (run before the
code): the boundary fixture 6.02 mm (Contour Parallel), the ledge fixture 8.00 mm and 3 530 flat
or rising moves (Geodesic Adaptive). Green with the patch: 1.70 to 1.98 mm, 0 flat or rising
moves on the two fixtures; the terrain fixture has 321 lifted moves (Contour Parallel) and reads
1.98 mm.

Why `adaptive3d_entry_stock_aware` was green at HEAD: it reads the material under the tool at
the END of each fed descent. It reads no flat piece and no stock along a leg.

## 6. W2: measured (2026-11-17)

"Before" is the code of HEAD (`cli_head`). "After" is the first release CLI of the patch
(`cargo build --release -p rs_cam_cli`), built before the shortest-run `min` of section 5. The
release CLI of the final tree gave the same G-code for the two setups, byte for byte (section
7). Same proposal file, one setup per run.

### 6.1 Setup "1 Back", for the operator

| | Before | After |
|---|---|---|
| Deepest cut of "Back rest 6 mm" (id 2) in its ramp entries | 14.5 mm (383 % over the force cap) | 1.99 mm (within the force cap, 66 %) |
| Deepest cut of id 2 in its clearing passes (body) | 20.7 mm | 20.7 mm (NOT changed: P38 is not in the tree) |
| Deepest cut of id 2 in its links, and in its wall cleanup | 6.0 mm, 3.0 mm | 6.0 mm, 3.0 mm |
| Ramp entries of id 2 | 134 | 136 (no entry became a peck ladder) |
| Ramp pieces deeper than 6.3 mm on the replay | 35 in 10 entries | 0 |
| Ramp pieces deeper than one lap (2.3 mm) on the replay | 67 in 20 entries, to 16.16 mm | 0; the deepest is 1.994 mm |
| Ramp entries that the model clip lifted | 58 | 3 |
| Samples of id 2 outside the safe envelope | 6 715 of 57 556 | 6 690 of 57 447 |
| Time of id 2 (feed at the emitted feeds) | 603 s, 40.0 m fed, 19 992 moves | 585 s, 39.7 m fed, 15 166 moves |
| Time of the setup (`total_runtime_s`) | 9 216 s | 9 186 s |
| Wood left, cavity walls (with the rest pocket op) | 62.1 mm2 | 62.1 mm2 |
| Wood left, cavity interior | 566.7 mm2, to 6.39 mm | 570.4 mm2, to 6.39 mm |
| Wood left, step shelf | 690.3 mm2 | 690.3 mm2 |
| Wood left, the 8.6 mm slot | 412.6 mm2, 21.0 mm | 412.6 mm2, 21.0 mm |
| Wood left, mech and slit pockets | 1 103.0 mm2, to 20.70 mm | 1 155.5 mm2, to 20.70 mm |
| Cut below the target after the two roughs (raw target) | 21.4 mm2 | 14.7 mm2 |
| The same, target lowered over one node | 2.2 mm2, to 4.13 mm | 1.3 mm2, to 1.72 mm |
| Ramp nodes of id 2 under the raw target / the lowered target | 507 / 94 | 106 / 33 (one entry) |
| Collisions, rapid collisions, failed checks (each op) | 0, 0, 0 | 0, 0, 0 |
| Contact counts | "Pin drill, flip pins" flute top 5; "Day strip slit V60" flute top 7 | the same |
| Export warnings | 24 lines | the same 24 lines |

Id 21 is not in this setup. Setup "3 Front" (the same two CLIs):

| | Before | After |
|---|---|---|
| Deepest cut of "Front 3D rough" (id 21) in its ramp entries | 4.19 mm (22 % over the force cap) | 1.69 mm judged, 1.88 mm deepest (within, 52 %) |
| Body and link rows of id 21 | 6.41 mm, 0.48 mm | 6.41 mm, 0.48 mm |
| Entry row of "Front bulk rough 18 mm" (id 20) | 1.94 mm | 1.94 mm |
| Ramp entries of id 21, and those that the clip lifted | 278, 192 | 287, 46 |
| Ramp pieces of id 21 over one lap on the replay | 23 in 12 entries, to 5.19 mm | 0; the deepest is 1.881 mm |
| Time of id 21 (feed at the emitted feeds) | 807 s | 870 s (+63 s: a ramp starts above the stock of its whole footprint) |
| Time of the setup | 8 677 s | 8 755 s |
| Collisions, rapid collisions, failed checks | 0, 0, 0 | 0, 0, 0 |

In plain words:

- The ramp entries of the two 6 mm roughs take 2 mm or less in each pass of the ramp. Before,
  an entry of the back rest rough took 14.5 mm.
- The back rest rough is 18 s shorter. The front 3D rough is 63 s longer.
- The clearing passes of the back rest rough still cut 20.7 mm deep. That is the defect of P38,
  and this package does not change it.
- The mech and slit pockets keep 52.5 mm2 more wood: a ramp leg that went out of its region
  before cut that wood by accident, 14 mm deep.

### 6.2 Predicted against measured

| Prediction | Result |
|---|---|
| P-1 entry row of id 2 at most 2.5 mm | TRUE: 1.99 mm. |
| P-2 no ramp piece over 6.3 mm; none flat or rising; at most 5 over 2.3 mm | Part TRUE, part FALSE. No piece is over 6.3 mm, and none is over 2.3 mm. FALSE: 3 entries have flat or rising pieces (the rule changed: a lifted leg is permitted where no other run exists, section 5). |
| P-3 body stays over 6 mm, link 6.0, cleanup 3.0 | TRUE: 20.7, 6.0, 3.0 mm. |
| P-4 time of id 2 in 540 to 700 s; at most 20 peck ladders | TRUE: 585 s; 0 peck ladders. |
| P-5 ramp nodes under the lowered target fall to 0; under 5 mm2 under the raw target after the two roughs | FALSE: 33 nodes stay (one entry, the chord over a wall, section 2.4), and the raw area is 14.7 mm2 (21.4 before). With the lowered target the area is 1.3 mm2 (2.2 before). |
| P-6 each zone of the gap map moves by less than 100 mm2 | TRUE: mech and slit pockets +52.5 mm2, cavity interior +3.7 mm2, the others 0. |
| P-7 collisions and contact counts | TRUE. |
| P-8 rivmap100 Geodesic Adaptive copy | TRUE: the G-code is the same byte for byte (`cmp`); 6 710 moves, 1 227 s. No bore fell back to a ramp. |
| P-9, P-10, P-11 | Section 7. |
| P-12 id 21 has no flat or rising ramp piece | FALSE for the same cause as P-2: 46 of 287 entries are lifted (192 of 278 before). No piece is over one lap. |

### 6.3 Not verified, not covered

- `project.entry_load` itself. The CLI output does not hold the finding (no file of the run
  names it). Its rule (`stock/sim_triage.rs::entry_load_observation`): the deepest entry sample
  (the axial depth or the plunge descent) is over 2 x the median body depth of the op, and the
  finding is Critical from 1.0 mm. The axial entry depth is now 1.99 mm (id 2) and 1.88 mm
  (id 21). The median body depth and the plunge axis were not read. Read the finding on the
  GUI or through MCP.
- The body of id 2 still cuts 20.7 mm deep. Setup "1 Back" is not cleared to cut.
- The chord over a wall of the model (section 2.4): 33 ramp nodes of one entry stay under the
  lowered target, to 1.72 mm. It is at HEAD. No probe in the crate proves it.
- Item 86 (section 2.6): 16.16 mm on the replay against 14.695 mm in the row.
- The other doors of `emit_ramp` (the dressup door of the 2.5D and the surface ops) keep the
  clip that ships no sample before a lifted run. The lib test shows the size of the defect on
  the item 44 shape. No job was measured through that door.
- A tool that does not cut at its centre: `replan_descent_at_limit` tries another direction
  when the probe loses contact or a lap does not go down, so the shipped leg can leave the
  footprint that `plan_entry` read. With no run at all it keeps the +X ramp of HEAD.
- The peck ladder of rule 4 ran on no job and in no test (0 entries on the two setups).
- The helix on Contour Parallel (section 2.7).
- The gouge finding of the ramps of id 1 and id 20 was not read; their entry rows did not move
  (1.97 mm and 1.94 mm).
- `entries.py` reads the centre region as the block rectangle; the 4 keep-outs are not read.

## 7. Gates (the final tree, 2026-11-17)

The first gate run found the shortest-run defect (section 5). The list below is the second run,
on the tree of the patch. After it the lib test of the patch lost three casts that clippy
refused; the lib test and clippy ran again. The release CLI was built again from that tree and
gave the same G-code for setup "1 Back" and setup "3 Front", byte for byte.

- core `--lib`: 2 843 passed, 1 failed, 13 ignored. The red is the known one
  (`feeds::envelope::tests::the_envelope_meets_its_time_budget`).
- adaptive3d sentries (32 binaries, `test-support,heavy-tests`, `--no-fail-fast`): 72 passed,
  1 failed, 2 ignored.
  - RED, from this patch, not edited: `a_rough_plans_its_cut_and_link_descents_to_the_core_limit_k5_centrecut`,
    `a_centre_cutting_tool_keeps_the_surface_slope`: "the Yes tool keeps its steep descents; the
    No tool is held to the limit". The test compares the steepest fed descent of the two tools
    on the pocket block. A probe print (removed) reads 0.05241 for both now: the slope of the 3
    degree ramp. So the steep descents of the Yes tool at HEAD were the falls of its ramp legs
    that the model clip lifted at the pocket wall (class s of section 2.2); the planner now
    keeps the legs in the pocket. This was not measured at HEAD (no HEAD test build). The claim
    of the test (the K5 clamp reads only a `No` fact) is not changed by this patch, but its
    instrument no longer sees a difference on this fixture. The lead must rule (section 8).
  - Green: the P39 sentry, `adaptive3d_entry_stock_aware`, `adaptive3d_emission_byte_parity`
    (no case moved), `adaptive3d_planner_never_ahead_of_emitted_path`,
    `adaptive3d_plan_matches_merged_path_g_plansimgap`, `adaptive3d_boundary_clear_parity`,
    `a_geodesic_layer_opens_with_a_helix_bore_p22b`, and the others of the folder file.
- other core sentries (14 binaries): 86 passed. Dressup sentries (13 binaries, with
  `one_entry_rule_two_emitters_p29_8` and `folder_maps_name_real_files_f140`): 83 passed.
- op golden: red with 52 changed cells: the 46 known cells, cell for cell, and 6 cells of ONE
  row. Not blessed:

  | `adaptive3d/ramp_geodesic` | before | after |
  |---|---|---|
  | `cutting_length_mm` | 2283.957 | 2296.031 |
  | `linear_count` | 1091 | 1089 |
  | `move_count` | 1113 | 1111 |
  | `gcode_hash` | `2ae632031de60cab` | `1bfe3e7a254d48a3` |
  | `moves_hash` | `be39e8dff5872a09` | `30814f8ee1004814` |
  | `spans_hash` | `3398534541b08672` | `8875c9681caa5486` |

  No row of the dressup door moved, so `RampLegs::along` and the leg cap function keep its
  bytes.
- FM1 (`feeds_matrix_instrument_fm1`): 2 passed. No cell moves.
- `rs_cam_mcp`: 39 passed. `rs_cam_cli`: 17 binaries, 113 passed.
- viz `--lib` (`--test-threads 2`, no GUI process): 705 passed, 2 ignored.
  `mcp_wire_surface_pin`: 2 passed.
- full clippy (`--workspace --all-targets`, the three core features, `-D warnings`): clean. `rustfmt --check` on each changed file: clean.

Predictions P-9 to P-11:

| Prediction | Result |
|---|---|
| P-9 goldens: `adaptive3d/ramp_geodesic` moves, no other new row | TRUE. |
| P-10 FM1 | TRUE. |
| P-11 sentries | Part FALSE: the K5 test above went red (not predicted). The rest TRUE: `adaptive3d_entry_stock_aware` is green, no byte parity case moved, the parity sentries are green. |

## 8. Decisions for the lead

- D-1 (the K5 test, red): `a_centre_cutting_tool_keeps_the_surface_slope` needs a ruling: give
  the test an instrument that reads the cut moves only (the claim is about the drape of a cut),
  or delete the claim. An agent does not edit it.
- D-2 (the golden row): bless `adaptive3d/ramp_geodesic` (6 cells) with this patch, or hold.
- D-3 (terrain time): the front 3D rough is 63 s longer (8 %), because a ramp starts above the
  stock of its whole footprint. A shorter leg that stops before taller stock would start
  lower; it is not built. Accept, or ask for it.
- D-4 (the wall chord, section 2.4): the clip can ship a chord over a wall of the model between
  two samples. Rare after this patch on the 3D Rough (33 nodes, 1.72 mm, one entry); the other
  doors were not measured. It needs its own probe and fix.
- D-5 (the other doors of `emit_ramp`): turn `whole_profile` on for every door (their G-code
  moves, the goldens too), or leave it.
- D-6 (a `No` tool with no run, and the peck ladder with no run): not covered and not tested.
- D-7: the body of id 2 (P38) is still open; the entries no longer block it.
