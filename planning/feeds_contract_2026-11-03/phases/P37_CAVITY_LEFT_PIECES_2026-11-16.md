# P37: the stock that setup "1 Back" leaves, and the 3D Rough that does not say so (2026-11-16)

Job: `planning/rivmap_block_job/nz_wanaka_21_lead_proposal.toml` at `fe9ed4a7e`, setup "1 Back"
alone. All positions are in the setup frame unless the text says G-code frame
(G-code frame = setup frame + (18.5, 8.5); Z is the same).

Sections 1 to 4 were written before the code. Section 5 holds the results.

## 1. The instrument

`gapmap.py` (scratch folder `p37/`) reads the emitted G-code of the release CLI
(`project --setup "1 Back" --emit-gcode`), not the plan and not the simulated stock.

- The stock is a height map on the nodes `(i x 0.2, j x 0.2)` of the stock box (395 x 460 mm,
  top 25.5), in the G-code frame.
- Each fed move (G1, G2, G3) lowers each node under the tool to the tool surface. The tools are
  the flat tools T42 (R 9), T1 (R 3), T6 (R 1) and the cones T2 (10 deg half angle) and T3
  (30 deg). A rapid cuts nothing. An arc becomes chords with a sagitta of 0.05 mm or less.
- The target is the top of `back_target_s1.stl` at each node. The stock to leave of each op of
  the setup is 0.
- The reach surface is the target closed by a disc of R 3: the lowest surface that a 6 mm flat
  tool can make. Stock between the target and the reach surface is there by design (an inside
  corner, a floor narrower than 6 mm).
- The bar is 0.3 mm: one node step (0.2) plus the 0.1 mm tolerance of the ops.

Check against the simulation: P36 section 1.1 gives the simulated stock top under the drill
footprint at four light holes (25.50, 16.79, 17.19, 17.60). The replay gives 25.50, 16.73,
17.13, 17.52 under a disc of R 3.2. The largest difference is 0.08 mm.

Limits: the replay does not know the simulated stock (the CLI does not write it); it has no
feed modulation effect (a feed does not move a tool); areas are node counts x 0.04 mm2.

## 2. The gap (HEAD, the proposal file as it is)

After the last op of the setup ("Day strip slit V60"), model area 158 158 mm2:

- 8 162.4 mm2 of stock is more than 0.3 mm above the target.
- 2 265.4 mm2 of it is by design (the 6 mm tool cannot reach it).
- 5 897.1 mm2 is above the reach surface: no toolpath cuts it.

| Where (G-code frame) | Area mm2 | Tallest mm | Volume mm3 |
|---|---|---|---|
| Cavity wall band (5 mm or less inside the outline of `cavity_d9.00.dxf`) | 3 026.9 | 12.27 at (93.4, 424.8) | 19 458 |
| Cavity interior (rings at the lakes, the hollow and the rivers) | 664.3 | 9.00 | 2 360 |
| Step shelf (Y 104 to 118) | 690.3 | 5.00 | 3 134 |
| Slot X 243 to 292, Y 53 to 64 (8.6 mm wide, target Z 4.5) | 412.6 | 21.00 | 8 637 |
| Mech and slit pockets (Y < 104) | 1 103.0 | 20.70 | 12 758 |

The cavity wall band: a strip at Z 25.5 (the uncut stock top, 9 mm above the floor) stands at
each of the four walls. Cross sections of the final stock:

- wall X 333.5 at Y 341.5: uncut from X 331.9 (1.6 mm), and 17.2 from X 330.3 to 331.9;
- wall X 24.5 at Y 341.5: uncut to X 25.7 (1.2 mm);
- wall Y 418.5 at X 181.5: uncut from Y 416.1 (2.4 mm);
- the corners: the 18 mm tool leaves its own radius. The corner light hole (327.13, 412.17)
  has 25.5 under its footprint.

The stock below the target (29 106.6 mm2 by more than 0.3 mm) comes from the ops that read
other models: the light-hole Drill (+1 189), the river channel (+23 196), the lake pockets
(+2 072), the V-bit ops (+2 623). The two roughs give 1.5 and 20.0 mm2. No arm of section 4 may
raise the figure of the roughs.

## 3. The cause (measured on the accused path)

The probe is the release CLI with `RUST_LOG=info,rs_cam_core::adaptive3d=debug`, ops 0 to 2.
Probe instrumentation: the `info` line of `leave_piece` now also gives the XY box of the piece.
It changes no motion (prediction P-1).

### 3.1 Op 1, "Back bulk rough 18 mm" (Geodesic Adaptive, fresh, Global)

- Its levels are 15.5 and 11.0. Each level drapes on the model, so the cavity floor (16.5) is
  cut at level 15.5.
- The planner cell is 1.5 mm (265 x 309 cells). A region is a set of tool-centre cells, and the
  geodesic arm opens it on an outline that holds a measured margin of one cell (C31,
  `measured_after_mm=1.510`). The cut edge stops 1.2 to 2.4 mm from each wall.
- The wall contour (`waterline_cleanup`) runs at each level: 3 contours at Z 15.5, 2 at Z 11.0.
  It cuts the section of the model at the level Z. The cavity wall stands from 16.5 to 25.5,
  above both levels, so no contour runs on it. This is candidate (d): the 18 mm rough does not
  go nearer, and its wall contour does not exist at this wall.
- The small-region bound (`(2 x D)^2 x min(1, depth/R)` = 1 296 mm2 for this tool) drops 8
  regions at Z 11.0. The log line is at `debug`. Nothing records them.

### 3.2 Op 2, "Back rest 6 mm" (Geodesic Adaptive, remaining stock, By Area)

Candidate (b), the rest region filter: not active. The op sets no area or width bound, and the
log has no "Rest region filter" line. Candidate (c), the boundary: 581 926 cells are outside
the block outline; the cavity walls are inside it. Neither explains the gap.

The planner drops stock at five places. Four are silent:

| # | Where in `adaptive3d/clearing.rs` | What it drops | Count in op 2 | Recorded |
|---|---|---|---|---|
| S1 | the level grid (`cell_has_material`) | a tool-centre cell is material only when the stock in ITS column is above ITS floor. A strip narrower than R at a wall is in no centre cell. | not countable | no |
| S2 | the small-region bound (144 mm2 for this tool) | a whole region at a level | 125 regions at 7 levels | `debug` line only |
| S3 | F-038 forecast (`min_region_cut_length_mm` 15) | a whole region at a level | 7 | `leave_piece` (`info`) |
| S4 | the geodesic field does not fit ("no field") | one piece of a region | 23 (11 at Z 16.55, 12 at Z 13.5) | `leave_piece` (`info`) |
| S5 | F-038 short groups | a cut group of a plan | 265 of 397 groups | `debug` line and a counter |

The wall contour of op 2 runs at one level only: Z 0.5, 2 contours (By Area with "Contour each
level" off cuts the walls once, at the bottom). The cavity wall has no section at Z 0.5.

Where the pieces are (the new box in the log):

- The 23 "no field" pieces are all at the wall X 333.5, Y 157 to 396 (boxes X 324.5 to 332.85):
  the cusps of the 18 mm rough at 17.2. No piece is at the walls X 24.5 and Y 418.5: the strip
  there is 1.2 to 2.4 mm wide, narrower than R 3, so S1 hides it.
- The 7 forecast pieces: the slit (X 33 to 325, Y 86 to 87, twice), a lake (X 116 to 137,
  Y 189 to 246, twice), the wall X 333.5 at Z 8.55, the wall Y 109.5 (X 169 to 225, twice).
- The slot (section 2) has a centre region of about 2.6 x 42 mm = 110 mm2. It is under the
  144 mm2 bound, so S2 drops it at each level. It has no log line at `info`.

So candidate (a) is true but it is the small part: the op leaves pieces to "the rest pass" and
this job has no such pass. The large part is S1 with the absent wall contour, and S2.

### 3.3 Who is "the rest pass"

Research C38 section 16 and the template "Geodesic body + edges" (`session/op_draft.rs`) name
it: a second 3D Rough (Contour Parallel, "Contour each level" on) whose boundary is the
`DerivedRestRegions` of the first. The session knows when such a reader exists: the rest
analysis of the source op is on only then (`normalise_rest_analysis_demand`). Op 2 has no
reader. Op 1 has none.

### 3.4 The second silence: the measured coverage does not run

`project.rough_leaves_islands` (G-ROUGHCOV) reads the simulated stock of each 3D Rough against
its own reach surface. It abstains when the simulation cell is larger than the stock to leave
(`RoughCoverageAbstention::CellCoarserThanStockToLeave`). Both roughs have a stock to leave of
0, so the reading can never run on them, and its text asks for "a cell at or under 0.000 mm".
The CLI log shows no coverage line. This is the only reading that can see S1, because S1 never
makes a region. Decision D-2 (section 6).

## 4. The rules and the predictions

### R-1: a 3D Rough states the stock that it holds and does not plan

Every region or piece that the level plan holds as material and then does not plan is recorded
once, with its level, its outline and its reason: `SmallRegion` (S2), `ShortForecast` (S3),
`NoField` (S4). The dropped cut groups (S5) are counted. The op gives ONE finding,
`geom.rough_left_stock` (Caution, a warning, never a refusal), through the channel of every
generation finding (`ctx.record` to `GenerationFindings` to `ToolpathStats` to
`diagnostics/adapters/from_generation.rs`): the count by reason, the XY area of the union, the
highest stock top that a piece can hold, the position of the largest piece, and what cuts it.

What the finding can know: whether a toolpath reads the regions of this op as its boundary
(section 3.3). It cannot know what a reader, or any other later toolpath, cuts; only the
simulation knows. So the text states the reader fact and is true in both cases:

- no reader: "No toolpath reads these pieces. They stay unless a later toolpath cuts them."
- a reader: "A toolpath reads them as its boundary."
- both: "Check the simulated stock."

The finding is raised in both cases.

The record does not feed `Produced::rest_regions` or the wall-contour skip: those keep the S3
and S4 pieces only, as today. So no motion changes.

R-1 cannot see S1 (no region exists). The finding does not claim it: it says "the stock that
this operation held and did not plan", not "all the stock that stays".

### Predictions (written before the code)

- P-1: the G-code of setup "1 Back" of the proposal file is the same file, byte for byte,
  before and after the code (the box in the log line and R-1 move no tool).
- P-2: op 2 gets `geom.rough_left_stock` with 125 small regions, 7 short-forecast regions and
  23 no-field pieces (155 in all), 265 dropped cut groups, and "no toolpath reads". The highest
  stock top is 25.5 (a region dropped at the first level, 19.5, holds stock up to the level
  top).
- P-3: op 1 gets the finding with 8 small regions, 1 dropped cut group, top 15.5 (the level
  above 11.0).
- P-4: the finding is in the G-code header and in `summary.json` `gcode_export.warnings` (a
  `warn(Danger::Part)` row). No op is refused.
- P-5 (lib test, red before the code): a 3D Rough on a fixture with one region under the
  small-region bound and one large region records one `SmallRegion` piece, with the area and
  the position of the small region, and the diagnostics adapter gives one
  `geom.rough_left_stock` for it. A fixture with no dropped region gives no finding.
- P-6: no op golden row changes and no FM1 cell changes (no motion changes).

### The job arms (scratch copies; measured with the instrument of section 1)

- Arm A: `contour_each_level = true` on op 2. With By Area it cuts the walls of each region at
  each level (C4b). Prediction P-A: the level contours run on the section of the model at the
  level Z. Levels 19.5 and 16.55 cut the cavity wall, so the strips at the four walls go. The
  corners: one contour pass at R 3 cuts a band of 6 mm from each wall. The 18 mm rough reaches
  the point (327.73, 409.98) at the corner, so a patch near (327.5, 412.5) stays, about 1.5 mm
  across. The corner hole centre (327.13, 412.17) is in it: the footprint of the corner hole
  keeps 25.5.
- Arm B: a Profile (inside, tool 0) on the outline of `cavity_d9.00.dxf` (model 7), floor 16.5,
  from remaining stock, after op 2 and before the drills. Prediction P-B: the strips go; the
  same corner patch stays (one pass, the same 6 mm band); about 1 236 mm of contour per depth
  pass.
- Arm C: a Pocket (tool 0) on the same outline, floor 16.5, from remaining stock (the rest
  level clip cuts only where stock stands). Prediction P-C: the strips and the corner patches
  go; the corner hole footprint is at 16.5.
- Each arm: every collision, rapid collision and contact count per op stays 0, except the known
  flute-top counts ("Pin drill, flip pins" 5, "Day strip slit V60" 7).

## 5. Results

(after the code)

## 6. Decisions for the lead

(after the code)
