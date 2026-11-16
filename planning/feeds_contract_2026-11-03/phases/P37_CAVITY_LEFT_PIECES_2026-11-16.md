# P37: the stock that setup "1 Back" leaves, and the 3D Rough that does not say so (2026-11-16)

Job: `planning/rivmap_block_job/nz_wanaka_21_lead_proposal.toml` at `fe9ed4a7e`, setup "1 Back"
alone. All positions are in the setup frame unless the text says G-code frame
(G-code frame = setup frame + (18.5, 8.5); Z is the same).

Sections 1 to 4 were written before the code. Section 5 holds the results.

## 1. The instrument

`gapmap.py` (written in the scratch folder `p37/`; a copy is in `phases/p37/` with the two maps
`gap_proposal.png`, `gap_proposal_cavity.png`, the arm report `arms_report.py` and
`proposal_cavity.diff`) reads the emitted G-code of the release CLI
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

Measured on 2026-11-16 and 2026-11-17 with the release CLI. The arms of section 5.2 ran on the
build before the export gate; `proposal_cavity.toml` ran again on the last build and its G-code
body is the same file.

### 5.1 The code (patch A)

- `adaptive3d/left_stock.rs` (new): the record (`Unplanned`, `LeftWhy`) and its read on the
  planner stock at the end of the plan (`LeftStock`).
- The rule R-1 was made narrower after the first design, before any measurement of it: a lower
  level of the same op can still cut a dropped piece, so a count of drops is not a count of
  stock that stays. The planner now reads each recorded outline on its own stock at the end of
  the plan. The finding is raised only when a piece still holds material above its level floor,
  and it states the standing pieces, their area in planner cells (each cell once), the tallest
  stock above the level floor and its position. So P-2 and P-3 (the drop counts) stay as
  written, and the finding adds the standing figures, which no prediction gives.
- The channel: `VolumetricOutput::left_stock` to `ctx.record(RoughLeftStockFinding)` to
  `GenerationFindings::rough_left_stock` to `ToolpathStats::rough_left_stock` to
  `from_generation::rough_left_stock` (`geom.rough_left_stock`, Caution, `warn(Danger::Part)`,
  face "Stock found and no cut planned: check the stock"). The reader fact is
  `regions_published`: the rest analysis of the op is on. The session turns it on when an
  enabled `DerivedRestRegions` boundary names the op, and MCP `set_rest_analysis_config` can
  also set it. So the text of section 4 was made weaker: with the flag on it says "This
  operation publishes its rest regions, so a toolpath can read these areas as its boundary";
  with the flag off it says "No toolpath reads these areas as its boundary". The remedy that
  the text names is the measured one (section 5.2): "a Pocket from remaining stock or a
  Profile on the outline of the walls". The position is "in the setup frame".
- P-4 was false as first built: the release CLI of the first build gave no line for the
  finding. The G-code header and `summary.json` hold only the findings of an export gate
  (`diagnostics::Gate`), and `geom.lifted_within_flute` and `geom.profile_loop_split` have
  none. A finding that says "the part is not to size here" must be in the export, so the
  export gate `Gate::RoughLeftStock` was added (`ProjectSession::export_warnings`). Lead ruling
  2026-11-17 (D-4): the gate stays.

| Prediction | Result |
|---|---|
| P-1 same G-code | true: without the two new header lines, the G-code of the proposal file is the same before and after (md5 `1334f86abaaa9f36849e3215aacf4e54` with the WARNING lines taken out of both; before the export gate the whole file had the same md5). The generation of op 2 takes 14.4 s before and 15.1 s after (one run each). |
| P-2 op 2 counts | true: 125 small regions, 7 short forecasts, 23 no-field pieces, 265 dropped cut groups, "No toolpath reads these areas". Not predicted: 149 of the 155 still hold stock on 2 212.0 mm2 (tool-centre area); the tallest stands 21.00 mm above the level floor at X228.10 Y48.70, stock top Z 25.5. That is the slot of section 2 (G-code (246.6, 57.2)). |
| P-3 op 1 counts | true: 8 small regions, 1 dropped cut group. All 8 still hold stock on 258.8 mm2; the tallest stands 4.23 mm above the level floor at X95.50 Y192.00, stock top Z 16.5. The predicted "top 15.5" is not a figure of the finding as built. |
| P-4 header and `summary.json` | false on the first build, true with the gate: one line for op 1 and one for op 2 in the G-code header and in `gcode_export.warnings`. No op is refused. |
| P-5 lib tests | true: `--lib p37` 4 passed. With the record of the small region taken out, `a_small_region_the_plan_drops_is_stated_p37` is red; the code was written before the first run of the test, so this injection is the red state. |
| P-6 goldens, FM1 | section 5.4. FM1 not run: P-1 says that no move changes. |

The export gate on the roughing benchmark (lead ruling: the gate stays):

- `planning/fixtures/rivmap100/rivmap100_live_0925.toml` as it is (its 3D Rough is Contour
  Parallel, which has none of the three drop sites): 2 header warning lines before, 2 after. The
  gate adds 0 lines.
- A scratch copy with `clearing_strategy = "geodesic_adaptive"` (no rivmap100 file uses the
  arm): the gate adds 1 line, for the rough: 34 small regions, all 34 still hold stock on
  131.0 mm2, the tallest 1.87 mm above the level floor (the stock to leave is 0.5), 33 dropped
  cut groups. So a Geodesic Adaptive rough of a terrain gets one line when it drops small
  peaks, also when a finish pass comes after it.

### 5.2 The job arms (measured; the CLI of the first build, the instrument of section 1)

"Fed time" is the fed length of the new or changed op over its emitted F words, with no
acceleration. "Setup time" is `total_runtime_s` of the CLI for the whole setup; it also holds the
effect on the later ops, which read the remaining stock.

| Arm | Cavity wall band mm2 | Corner hole footprint before the Drill | Fed time of the change | Setup time s | Below target after the roughs and the new op mm2 |
|---|---|---|---|---|---|
| Proposal as it is | 3 026.9 | 25.50 | 0 | 9 210 | 21.4 |
| A: Contour each level on op 2 | 1 655.9 | 25.50 | +3 327 s (op 2: 603 to 3 930 s, 40 to 251 m fed) | 15 108 | 165.5 |
| B: Profile, 3.0 mm per pass | 62.1 | 25.50 | +73 s (3.8 m) | 9 099 | 28.2 |
| B2: Profile, 2.25 mm per pass | 62.1 | 25.50 | +97 s (5.1 m) | 9 123 | 26.7 |
| C: rest Pocket, 3.0 mm per pass | 62.1 | 16.50 | +193 s (8.5 m) | 9 155 | 28.2 |
| C2: rest Pocket, 2.25 mm per pass | 62.1 | 16.50 | +230 s (10.2 m, and 8.1 m of rapids) | 9 216 | 26.7 |

- The 62.1 mm2 that stays in the band is not the cavity wall: it is rings at lakes and at the
  hollow near the walls (3.3 to 4.1 mm tall) and 0.9 mm2 of nodes on the wall line. The four
  walls are to size in B, B2, C and C2.
- The 5 to 7 mm2 more below the target in B and C is 168 nodes at G-code X 43.0, the wall line
  itself (the target raster gives the wall top to a node on the line). It is an instrument
  limit, not a cut into the wall.
- P-A is false. Arm A leaves 1 656 mm2 of the band, adds 3 327 s of feed and cuts 144 mm2 more
  below the target. Decision D-7.
- P-B is true: the strips go and the corner patch stays (the corner hole keeps 25.50).
- P-C is true: the corner hole footprint is at 16.50.
- Every op of every arm: 0 collisions, 0 rapid collisions, 0 failed checks; the contact counts
  are the two known ones ("Pin drill, flip pins" flute top 5, "Day strip slit V60" flute top 7).
- Envelope warnings (`project.sim_outside_envelope`) of the new op: B and C at 3.0 mm per pass
  cross the rigidity depth limit (3.00 against 2.70 mm). B2 has no warning. C2 has one: the
  corner coverage width, 5.60 against 5.12 mm, 1 050 of 17 158 samples (the first cut into
  each corner patch). The feed modulation (cap only) lowers the feed there (460 to
  3 165 mm/min).
- The feeds of the new op (3 165 mm/min, plunge 1 347, 10 610 rpm) are the values of "Lake relief
  band 6 mm" (op 25), the slot cut of the same tool in the same job. They are not from Suggest.

Recommended: arm C2, `p37/proposal_cavity.toml` (one new op, id 40, "Cavity rest pocket 6 mm",
between op 2 and the pin drill). It is the one arm that brings the walls and the corners to
size and lets the corner light hole start in a cut cavity. B2 is 133 s shorter in fed time and
has no envelope warning, and it leaves the corner patch (about 1.5 mm across, 9 mm tall) at
each corner; the Drill then cuts it at the corner hole only.

The second check of the corner hole, on the simulated stock: the Drill of the proposal reads
the remaining stock, and its R-plane is 5 mm above the highest simulated stock under the tool
(P36 A1). In the G-code of the proposal as it is the R-planes are 38 x 21.50, 21.79, 22.19,
22.60 and 30.50 (the corner hole: stock at 25.50, 4 pecks). In the G-code of
`proposal_cavity.toml` all 42 are 21.50 (stock at 16.50, 3 pecks). In arm B2 the corner hole
keeps 30.50.

`proposal_cavity.toml` on the last build: the same G-code body as arm C2, the same table row,
0 collisions and 0 rapid collisions on each op. The header keeps the two
`geom.rough_left_stock` lines: ops 1 and 2 still drop what they dropped (the slot is one of
the pieces).

### 5.3 What the coverage reading would give (estimate by the replay, not the product)

Each rough against its own reach surface, bar one cell (0.2 mm): op 1 (R 9, floor 11.0) 6 885 mm2
in 12 islands, tallest 14.5 mm; op 2 (R 3) 8 798 mm2 in 20 islands, tallest 21.0 mm. With a bar
of 1.0 mm: 6 534 mm2 in 12 and 8 368 mm2 in 25. The islands are few and large: on this job the
reading is not cell noise at a stock to leave of 0.

### 5.4 Gates

Run on 2026-11-17 on the tree of patch A. Round 1 is the whole list. Round 2 ran after the last
text edits (the reader sentence, the remedy, "setup frame", the singular of "cut group").

- core `--lib`: 2 842 passed, 1 failed, 13 ignored, in both rounds. The red is
  `feeds::envelope::tests::the_envelope_meets_its_time_budget`, the known timing red.
- adaptive3d sentries, round 1: 16 binaries, all green (`adaptive3d_boundary_clear_parity`,
  `adaptive3d_keep_down_link_f038b`, `adaptive3d_entry_stock_aware`,
  `adaptive3d_entry_coalescing_f038`, `adaptive3d_plan_matches_merged_path_g_plansimgap`,
  `adaptive3d_planner_never_ahead_of_emitted_path`, `adaptive3d_emission_byte_parity`,
  `adaptive3d_plans_every_slice_piece_g_adapt3dpieces`,
  `geodesic_adaptive_keeps_the_research_geometry_s7_4`,
  `a_rough_publishes_the_regions_it_leaves_d44`, `a_derived_rest_rough_cuts_only_the_leftovers_d44`,
  `a_rest_op_holds_one_depth_at_the_band_c4`, `contour_each_level_is_off_by_default_c4`,
  `by_area_walls_cut_each_region_level_c4b`, `a_geodesic_slice_cuts_no_full_slot_sweep_p22f`,
  `adaptive3d_by_area_cells_confine_jobs`).
- other core sentries, green: `findings_transport_join_h21`, `a_job_with_each_finding_exports_and_names_it_k6`
  (3 tests, `a_clean_job_writes_no_warning_line_k6` among them),
  `the_volumetric_wrapper_keeps_its_produced_w6_5`, `every_op_states_its_capabilities_w2_4`,
  `region_cap_honesty_f3`, `gate_population_vacuity_xvac`,
  `a_rough_island_above_the_stock_to_leave_is_flagged_g_roughcov`,
  `depth_beyond_stock_core_g_depthstockcore`; round 2 also
  `every_consumer_reads_one_claimed_band_g1` and `narrate_regions_closed_c8`.
- op golden (`every_op_keeps_its_toolpath_g_opgolden`): red with 46 changed cells. The 46 lines
  are the file `opgolden_known_46_rows.txt`, cell for cell (`diff` gives nothing). No other cell
  moves. Not blessed.
- `rs_cam_mcp`: 39 passed. `rs_cam_cli`: 17 test binaries, all green (81 tests in the
  largest).
- viz `--lib` (`--test-threads 2`, no GUI process): 705 passed, 2 ignored. `mcp_wire_surface_pin`:
  2 passed. `a_collision_is_the_same_warning_on_every_surface_q6`: 2 passed.
- full clippy (`--workspace --all-targets`, the three core features, `-D warnings`): clean in
  both rounds. `rustfmt --check` on the changed files: clean; no other file changed.
- FM1: not run. No move changes (P-1, and the op golden moves no cell outside the known rows).

Not verified:

- No sentry holds the export gate `Gate::RoughLeftStock`. The evidence is the job run and the
  rivmap100 copy. The adapter and the planner record have lib tests.
- The finding was not seen on the GUI or through MCP. It is a row of the one diagnostics list
  (`diagnostics_from_generation`), which both read.
- The generation time of op 2 is one run before and one after.

## 6. Decisions for the lead

- D-1 (planner, needs a ruling): a 3D Rough on remaining stock does not see a strip narrower
  than its radius at a wall (S1), and the wall contour runs only where a level cuts the wall.
  On this job that is most of the 3 027 mm2 band. A fix in the planner (a centre cell is
  material when stock stands anywhere under the tool disc, as `PriorStockTops` reads the
  boundary band) moves the goldens and FM1 of every rest rough. Not done.
- D-2 (the measured reading): `project.rough_leaves_islands` abstains at a stock to leave of 0.
  It is the only reading that sees S1. Section 5.3 says the reading is clean here. Proposal: at
  a stock to leave under one cell, measure with the bar at one cell and say so, in place of
  the abstention. Not done.
- D-3 (the small-region bound): on a rest rough it drops the slot (412.6 mm2 of stock 21 mm
  tall, a centre region of about 110 mm2 under the 144 mm2 bound) at each level. For the
  18 mm tool the bound is 1 296 mm2. The bound was made for terrain peaks on fresh stock. A
  ruling is needed on a rest rough. Not changed; the new finding names it.
- D-4 (the export gate), RULED 2026-11-17: the gate stays; a rough that leaves stock says so
  in the G-code header. Measured on rivmap100: 0 lines more on the file as stored (Contour
  Parallel), 1 line more on a Geodesic Adaptive copy (section 5.1). The body op of the template
  "Geodesic body + edges" also gets the line at each export when it drops a piece, with the
  "publishes its rest regions" sentence.
- D-5 (the job): the other gaps of section 2 are not cut by any arm: the step shelf (690 mm2,
  5 mm), the slot (413 mm2, 21 mm), the mech and slit pocket walls (1 103 mm2, 20.7 mm), the
  interior rings (567 mm2).
- D-6 (the job): "Back rest 6 mm" has a `project.sim_outside_envelope` line in the proposal as
  it is: depth 20.70 mm against the limit 2.75 mm (move 11746).
- D-7 (follow-up): "Contour each level" on a By Area rest rough of this job (arm A) costs
  3 327 s, does not clear the wall and cuts below the target. Not looked into.
- D-8: the finding is not superseded by a measured coverage reading of the same op. When D-2
  lands, the supersession reducer must drop it for the measured answer.
