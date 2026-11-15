# P31: four operator rulings of 2026-11-15

Written 2026-11-15 at HEAD `7c28a48c5`. Each task has a predictions section, written before its
code patch, and a results section. "Measured" is a number from a run in this step. "Reasoned" is
read from the code, not run.

## A. The clamp boxes of setup "3 Front" (no code)

The operator ruled that the corrected boxes of `P29-8e_OBSTACLE_FRAME.md` section 5 go into the
two project files. Commit `7c28a48c5` wrote them (operator file sha256
`ee6869be015b633b424df0fadbf13a38874e0ef1e703c8c148ccdeeaa9d29666`). This task verifies the
toolpaths. It changes no code and no project file.

### A: predictions (before the run)

Method: the reviewer's probe (`review/zz_review_probe.rs.txt`, with a `collision_check` call for
each op and no export) generates operator ops idx 0, 4 to 13 and 17 at HEAD with no simulation,
`--release`. Each dump is compared with the reviewer's dump of commit `109ed6644`
(`review/now_<idx>.csv`).

- Idx 0 and 4 to 11 (setups "1 Back" and "2 Clamp pads"): byte-equal. A difference is a STOP.
- Idx 12, 13 and 17 (setup "3 Front") can differ. Idx 14, 15 and 16 are in that setup too; they
  take minutes each and are not generated.
- Idx 17 (Outline profile with tabs): the clip re-entries go from 42 to about 4. The frame note
  predicted "1560 moves, 4 EntryPlunge, no gap above the floor" from an in-memory probe at an
  older commit, before the re-entry ramps of `910ded4b4`. So the move count is not expected to
  equal 1560: each of the 4 re-entries can be a fold ramp, not a plunge. The cycle time falls.
- The remaining re-entries are at the floor level, 2 on each side in X, where the head band
  (radius 12.5 mm) reaches the grown clamp when the tip is below Z 0.5.
- No obstacle hit on a final toolpath (`ProjectSession::collision_check`).

### A: results (measured)

Run: release build of HEAD `7c28a48c5`, 12 ops generated in one session, no simulation. The dumps
and the probe are in the agent scratchpad (`p31/A/now_<idx>.csv`, `p31/A/zz_p31_probe_a.rs.txt`,
`p31/A/analyze.py`).

- Idx 0, 4, 5, 6, 7, 8, 9, 10, 11 and 13: byte-equal to the review dumps (`cmp`). No op of
  another setup moves. Idx 13 (setup "3 Front") does not move.
- Idx 12 and idx 17 move.

| | idx 12 Face front to T | idx 13 Engrave day labels V60 | idx 17 Outline profile with tabs |
|---|---|---|---|
| Moves, before / after | 371 / 361 | 314 / 314 | 1840 / 1566 |
| Clip re-entries, before / after | 2 / 0 | 0 / 0 | 42 / 4 |
| Straight plunges into stock (`EntryPlunge`), before / after | 2 / 0 | 31 / 31 (the op's own plunge entries, no change) | 0 / 2 |
| Obstacle hits on the final toolpath (`collision_check`) | 0 | 0 | 0 |
| Cycle time (s), before / after | 286.31 / 281.06 | 31.66 / 31.66 | 740.54 / 672.65 |

"Before" for idx 17 is the review dump of `109ed6644`, which holds the re-entry ramps of
`910ded4b4`: its 42 clip re-entries are fold ramps, not plunges. Count: the review dump has 78
entry runs and the new dump has 40. Each has 20 level starts and 16 tab descents (8 tabs on each of
the two lowest levels). The rest are clip re-entries: 78 - 36 = 42 and 40 - 36 = 4.

**Idx 12.** Before, 15 rows stopped at the old boxes (X 1.62 to 356.38, the frame of setup 1) and
two short pieces at X 401 to 404 had a plunge each. After, the 16 rows near the clamps run
X 20.12 to 374.88 (20.12 + 374.88 = 395: symmetric about the stock centre X 197.5), every other row
runs X -9 to 404, and no plunge is left.

**Idx 17.** Above the floor level no pass is clipped: each level from Z 23.75 to Z 1.25 is one
loop with its tab ramps. At the floor level (Z 0) four re-entries are left, two on each X side:

| # | X | Y | Z | Kind | Feed (mm/min) |
|---|---|---|---|---|---|
| 1 | 15.5 | 185.67 | 0 | fold ramp (air lead to Z 1.75) | 3165 |
| 2 | 15.5 | 327.25 | 0.5 | straight plunge from Z 2.375 | 1582 (the plunge rate) |
| 3 | 379.5 | 274.33 | 0 | fold ramp (air lead to Z 1.75) | 3165 |
| 4 | 379.5 | 132.75 | 0.5 | straight plunge from Z 2.375 | 1582 (the plunge rate) |

The reason is the head zone at the floor: the default head band (radius 12.5 mm) reaches the
grown clamp when the tip is below Z 0.5, on both X sides. The positions agree with the frame note
(X 15.5 at Y 185.67 and 327.25; X 379.5 at Y 274.33 and 132.75). The note predicted 4 `EntryPlunge`
and 1560 moves; with the re-entry ramps two of the four are fold ramps and the op has 1566 moves.

Each straight plunge is at the start of a tab. The kept piece there is the upper part of the
vertical tab lift (Z 0.5 to Z 2.0), so the tool plunges to Z 0.5 and then lifts over the tab. The
level before it cut that position to Z 1.25, so the plunge enters 0.75 mm of stock.

**For the operator: the floor level is not cut next to the clamps.** On each X side the floor pass
(Z 1.25 to Z 0) has two gaps, each 52.92 mm long and next to a tab: Y 132.75 to Y 185.67 and
Y 274.33 to Y 327.25, on X 15.5 and on X 379.5 (symmetric about Y 230). In each gap a skin of
1.25 mm stays; on the 9.5 mm next to one tab of each side the clipped tab ramp leaves 0.5 mm to
1.25 mm. So the part stays attached there after the op, as it does at a tab.

**Verdict.** The corrected boxes do what the frame note said. Only ops of setup "3 Front" move
(idx 12 and idx 17 of the ops that were generated). No obstacle hit. Not generated: idx 1, 2, 3,
14, 15 and 16 (idx 14, 15 and 16 are in setup "3 Front" and can move).

## B. Entry rubbing warns in the drawer only

The operator, 2026-11-15: "Drawer only". An entry rubbing row stays red in the Limits drawer. It
writes no export warning. The export warnings stay for the load limits (chip maximum, power,
deflection, plunge rate, depth table).

### B: the rule as it is built

- One typed rule in core, next to `judged_only_before_the_simulation`:
  `tool_load::groups::stated_row_warns_at_export(limit, cut)`. It is `false` only for
  `LimitId::RubbingFloor` on `CutKind::Entry(_)` (a plunge, a ramp, a helix or a peck ladder). Each
  match names every variant, so a new limit or a new cut kind does not compile until it states its
  answer.
- `ProjectSession::stated_cut_over_a_limit` (the door of `export.stated_cut_over_limit`) keeps a
  stated row with the gating tone only where the rule is `true`, under both covers (no fresh
  simulation, fresh simulation). The memo holds the cut of each row.
- Not changed: the row, its tone, the drawer, the face line of the Passes card, the warning id
  and its face.
- An air lead (`EntryApproach`) and a lead-in (`LeadIn`) are in the entry GROUP but are not entry
  cuts of the ruling; the rule keeps their warning. No stated row of the four files has one of
  these cuts with the gating tone (measured, the probe below).

### B: predictions (before the code)

Instrument: a throwaway probe at HEAD (`p31/B/zz_p31_probe_b.rs`: every stated row with the gating
tone, with its limit and its cut, and the words of `stated_cut_over_a_limit` under both covers),
READ-ONLY on the four project files, no simulation, and a script that applies the rule
(`p31/B/predict.py`; it first checks that the rule of HEAD gives the words of HEAD: 0 mismatches).

"No simulation" is the warning of an export with no simulation. "Fresh simulation" is the warning
after a simulation that is fresh for the op. An op that is not in a table has no stated-cut warning
before or after.

`nz_wanaka_21_operator.toml`: no warning changes. Idx 1, 2, 7, 8, 14 and 15 keep their warning
with the same words (chip maximum rows; no entry rubbing row has the gating tone on this file).

`nz_wanaka_21.toml`:

| idx | State | Before | After |
|---|---|---|---|
| 2 | no simulation | stated point · moves 78 % over limit; 1 more stated row over a limit | stated point · moves 78 % over limit (no "more") |
| 2 | fresh simulation | ramp entry · rubbing 48 % under floor | no warning |
| 16 | no simulation | body · chip max 50 % over limit · until modulated | the same |
| 17 | no simulation | ramp entry · rubbing 48 % under floor | no warning |
| 17 | fresh simulation | ramp entry · rubbing 48 % under floor | no warning |

`rivmap100.toml`:

| idx | State | Before | After |
|---|---|---|---|
| 0 Face 5 | both | ramp entry · rubbing 44 % under floor | no warning |
| 1 3D Rough | both | helix entry · rubbing 44 % under floor | no warning |
| 2 Scallop Finish | both | body · rubbing 17 % under floor | the same |
| 3 Project Curve | both | body · rubbing 17 % under floor; 1 more stated row over a limit | the same |

`rivmap100_live_0925.toml`:

| idx | State | Before | After |
|---|---|---|---|
| 0 Face 5 | both | ramp entry · rubbing 44 % under floor | no warning |
| 1 3D Rough ladder | no simulation | stated point · depth 33 % over vendor depth cap | the same |
| 2 Scallop Finish | both | body · rubbing 17 % under floor | the same |
| 3 Project Curve | both | body · rubbing 17 % under floor; 1 more stated row over a limit | the same |

Totals with no simulation: operator 6 warnings to 6; `nz_wanaka_21` 3 to 2; rivmap100 4 to 2;
rivmap100_live 4 to 3. A body rubbing row (Scallop Finish, Project Curve) is not an entry row and
keeps its warning.

**The chip minimum of an entry.** The operator named only the rubbing floor, so the rule leaves a
`ChipMin` row as it is. Count of stated `ChipMin` rows of an entry cut that exceed: operator 7,
`nz_wanaka_21` 7, rivmap100 5, rivmap100_live 5. Every one has the advisory tone (the band floor of
an entry is a reference, `AdvisoryFloor`), so none writes an export warning: 0 of 24 warn, before
and after. They are caution rows of the drawer. A ruling is needed only if the operator wants
these rows to change.

**Tests that change.**

- `a_job_with_each_finding_exports_and_names_it_k6`: the mirror case becomes
  `a_ramp_that_rubs_is_red_in_the_drawer_and_writes_no_export_warning_k6`. It asserts the one
  stated row with the gating tone from `limit_rows` (rubbing floor, ramp entry, face "ramp entry ·
  rubbing 44 % under floor"), no export warning and no `WARNING` header line.
- `a_row_softens_only_under_a_current_modulation_p30_2`: claims 2 and 3 (after a simulation) assert
  that the ramp rubbing row keeps the gating tone and that the export has no
  `export.stated_cut_over_limit` warning. Claim 1 (no simulation) asserts the literal words
  "stated point · feed cap 125 % over limit; 2 more stated rows over a limit; no current
  simulation measured this op" (3 more before: the ramp rubbing row leaves the count).
- `a_stated_cut_over_any_limit_warns_review_f4` (R4 and R8): the plunge-rate row of a plunge entry
  still warns, with and without a simulation. This is the proof that an entry row of another limit
  keeps its warning.

**Op golden, FM1.** Golden arms run no simulation. The header line of the warning prints the face
of the id, not the detail, so `gcode_hash` moves only on an arm that loses its warning: an arm
whose only stated rows with the gating tone are entry rubbing rows. No other field moves. The list
is measured with the code in place (results). FM1 reads no export warning: no cell moves.

### B: results (measured)

- **Warnings.** The probe ran again with the rule in place. Every warning word of the four files
  equals the prediction: 88 compared (44 toolpaths, two covers), 0 mismatches. Every row line of the
  four dumps (limit, cut, tone, face) is equal before and after: no row moves.
- **Entry `ChipMin` rows that warn:** 0 of 24, before and after (all have the advisory tone).
- **Tests.**
  - `a_ramp_that_rubs_is_red_in_the_drawer_and_writes_no_export_warning_k6`: the one stated row
    with the gating tone is (rubbing floor, ramp entry, "ramp entry · rubbing 44 % under floor");
    the export has no warning and no `WARNING` line.
  - `a_row_softens_only_under_a_current_modulation_p30_2`: claim 1 holds the literal words with "2
    more stated rows"; claims 2 and 3 hold a red ramp rubbing row and no stated-cut warning.
  - `a_stated_cut_over_any_limit_warns_review_f4`: the two plunge-rate cases keep their literal
    detail ("plunge entry · plunge rate 294 % over limit"), with and without a simulation. A new
    test, `only_the_rubbing_floor_of_an_entry_cut_is_drawer_only_p31`, holds the rule as a table: of
    28 limits x 18 cuts, exactly 4 cells are `false` (the rubbing floor on each of the 4 entry
    kinds).
- **Op golden.** The known 46 rows, and `gcode_hash` on 26 arms. No other field of an arm moves.
  Each of the 26 arms loses its one stated-cut warning line (its only stated rows with the gating
  tone are entry rubbing rows). The arms:
  `adaptive/each_face`, `adaptive/fresh`, `adaptive/rest`, `adaptive3d/by_area_helix_cp`,
  `adaptive3d/ramp_geodesic`, `bore/rest`, `chamfer/rest`, `drill/rest`, `face/fresh`, `face/rest`,
  `pocket/each_face`, `pocket/each_face_flipped`, `pocket/extend_open`, `pocket/fresh`,
  `pocket/rest`, `pocket/zigzag`, `profile/each_face`, `profile/fresh`, `profile/inside`,
  `profile/on_closed`, `profile/on_open`, `profile/rest`, `trace/rest`, `zigzag/each_face`,
  `zigzag/fresh`, `zigzag/rest`. Not blessed here: the field-only bless of `gcode_hash` on these
  arms is the lead's step (the command is in `ORDER.txt`).
  Header-only proof: the programs of the arms were dumped (`RS_CAM_OPGOLDEN_DUMP_GCODE`). With the
  line "(WARNING Tool export.stated_cut_over_limit <op>: Stated cut over a limit: no measured
  gate)" put back in the header of each op that lost it, each program hashes to its stored value:
  26 of 26 arms, 31 lines (5 `rest` arms hold two ops and lose two lines).
- **FM1.** 2 passed. No cell moves, as predicted.
- **Wire changes.** None: no id, no key and no face changes. The export of a project can hold
  fewer `export.stated_cut_over_limit` warnings, and the count of "more stated rows" in a detail
  can fall.
- **Gates.** On the B tree, one cargo job at a time: fmt check clean. Core sentries:
  `a_row_softens_only_under_a_current_modulation_p30_2` (4), `a_job_with_each_finding_exports_and_names_it_k6`
  (3), `a_stated_cut_over_any_limit_warns_review_f4` (3), `one_record_on_every_surface_p29_10` (10),
  `every_limit_judges_every_declared_cut_p29_7`, `every_declared_cut_is_judged_at_the_stated_point_p29_10`,
  `the_exemption_list_only_shrinks_p29_3`, `no_limit_reads_the_op_type_p29_last`,
  `the_record_states_what_the_gcode_runs_p30_1`, `folder_maps_name_real_files_f140`,
  `command_registry_completeness`: pass. The 24 core binaries whose names hold `modulat`, `export`
  or `rubbing`: pass, but for `_litmatrix_rubbing_floor_clamp` (the known red) and
  `feed_modulation_cycle_time_f036c` (not run: 256 s by its `Cargo.toml` comment, over three
  minutes). Core `--lib`: 2822 passed, 1 failed in the parallel run
  (`the_envelope_meets_its_time_budget`, the known timing red; alone: 2 of 3 runs pass, 1 fails on
  the time budget while another session ran a release build). `rs_cam_mcp` 39; `rs_cam_cli` all
  pass. Viz `--lib` 704 passed (no GUI ran); the 13 viz binaries whose names hold `export`,
  `modulat`, `readiness`, `k6`, `sim` or `mcp_wire` and the 7 named viz binaries: pass. FM1: 2
  passed. Op golden: the known 46 rows and the 26 `gcode_hash` rows above.
- **Full clippy is red at HEAD, in a file this step does not touch:**
  `crates/rs_cam_core/tests/rubbing_floor_diameter_scaling_measurement.rs:613`, "field `ae_mm` is
  never read" (the file of commit `729b5b919`). With `--keep-going` it is the only finding of the
  workspace. Not changed here; the lead decides.
