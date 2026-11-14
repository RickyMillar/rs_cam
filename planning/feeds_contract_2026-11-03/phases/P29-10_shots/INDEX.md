# P29-10 screenshots: the index

## The build

Every "after" file comes from ONE build and ONE GUI process: `rs_cam_gui`
release, built from HEAD `f3d9645cb` plus the review patch R8 (not
committed), binary mtime 2026-11-15 03:55:34. `gui_status` read "binary not
newer than the process" after `gui_restart`. The window is 1600 x 1400
points, 2400 points high for an open drawer. No project was saved; the
operator file hash stayed
`f6a2b3880410b10288ce21ebf976c2405ee78250a5957595132fd72d599e4e41`.

- The operator file `planning/rivmap_block_job/nz_wanaka_21_operator.toml`:
  loaded only; no `generate_all`, no `generate_toolpath`, no simulation.
  After the load the GUI showed results for 11 of 18 ops ("Toolpaths
  11/18"). These are the 11 ops that `list_toolpaths` gave as `stale`
  right after the load (idx 0, 3, 4, 6, 7, 8, 10, 11, 12, 13, 17), so the
  GUI regenerate sweep (`skips_automatic_generate`) most likely generated
  them. The cause is inferred from that match, not traced; the GUI log
  holds no line for them.
- The simulated shots use `planning/fixtures/rivmap100/rivmap100.toml`:
  `generate_all` (its plan ran 3 simulations at 0.2 mm), then ONE
  `run_simulation`, then one `export_gcode`. GUI RSS 1.4 GB. The old
  simulated shots of the operator file (S5, S6, S7, S9) are REPLACED by
  these rivmap100 shots; the operator file was not simulated.
- S10: `planning/fixtures/rivmap100/rivmap100_live_0925.toml`, no
  simulation.

The "before" files were shot on older builds (see the git history of this
file); a before file of S5, S6, S7 or S9 shows the operator file, so it is
not the same project as its after file.

## The shots

| Shot | File before | File after | Project, op | What to look at |
|---|---|---|---|---|
| S1 | `before/S1_idx1_passes.png` | `after/S1_idx1_passes.png` | operator idx 1, Back bulk rough 18 mm | danger face line "planner cap · chip max 24 % over limit · until modulated"; "Limits · 153 · 14 over" |
| S1b | `before/drawer_idx1_closed_10g.png` | `after/S1b_idx1_limits_open.png` | operator idx 1 | the stated point, body, planner cap (chip max 24 % exceeds), first slot rows; the drawer is longer than 2400 points: "If you apply" is under the frame |
| S2 | `before/S2_idx14_passes.png` | `after/S2_idx14_passes.png` | operator idx 14, Front bulk rough 18 mm | danger face line "planner cap · chip max 3.4 % over limit · until modulated..." (elided) |
| S2 | none | `after/S2_idx14_limits_open.png` | operator idx 14 | the ramp entry rows carry "worst case" on the depth and power rows only (R2) |
| S2 | `before/S2_idx15_passes.png` | `after/S2_idx15_passes.png` | operator idx 15, Front 3D rough | danger face line "planner cap · chip max 3.1 % over limit · until modulated" |
| S2 | none | `after/S2_idx15_limits_open.png` | operator idx 15 | the drawer by cut; the tail is under the frame |
| S3 | `before/S3_idx0_passes.png` | `after/S3_idx0_passes.png` | operator idx 0, Face back to T1 | no face line; "Limits · 81"; the muted line "runs 6488 mm/min · 0 of 60 above 6488 · as stated" |
| S3 | none | `after/S3_idx0_limits_open.png` | operator idx 0 | every stated row within; cuts fold to one line each |
| S3 | `before/S3_idx12_passes.png` | `after/S3_idx12_passes.png` | operator idx 12, Face front to T | as idx 0 ("0 of 62 above 6488") |
| S3 | none | `after/S3_idx12_limits_open.png` | operator idx 12 | as idx 0 |
| S4 | `before/S4_idx16_passes.png` | `after/S4_idx16_passes.png` | operator idx 16, Front scallop finish | caution face line "body · chip min 16 % under floor"; the SAFE WIDTH bar |
| S4 | none | `after/S4_idx16_limits_open.png` | operator idx 16 | helix entry chip min 91 % and rubbing 73 % under floor, advisory (may not occur) |
| S4 | `before/S4_idx5_passes.png` | `after/S4_idx5_passes.png` | operator idx 5, River channel 6 mm | caution line "Depth, power and deflection: judged after the simulation..." (elided) |
| S4 | none | `after/S4_idx5_limits_open.png` | operator idx 5 | "Limits · 39"; the cuts fold; "plunge entry · 1 within · 4 not modelled" |
| S5 | `before/S5_sim_idx1_metrics.png` (operator) | `after/S5_sim_idx0_metrics.png`, `after/S5_sim_idx0_metrics_groups_open.png` | rivmap100 idx 0 Face 5, playhead at its start | caution "runs 2333 to 3000 mm/min · 77 of 77 above 2200", "feeds: modulated, run 1"; Cut groups · 21 open |
| S5 | `before/S5_sim_idx2_metrics.png` (operator) | `after/S5_sim_idx1_metrics.png`, `after/S5_sim_idx1_metrics_groups_open.png` | rivmap100 idx 1 3D Rough | "runs 559 to 3000 mm/min · 2563 of 5787 above 2400"; the cleanup and link groups have their own rows |
| S5 | none | `after/S5_sim_idx2_metrics.png`, `after/S5_sim_idx2_metrics_groups_open.png` | rivmap100 idx 2 Scallop Finish | "Cut force · no limit"; depth rows "no limit here"; Width 1 to 123 % of D |
| S5 | none | `after/S5_sim_idx3_metrics.png`, `after/S5_sim_idx3_metrics_groups_open.png` | rivmap100 idx 3 Project Curve | "runs 3000 mm/min · 478 of 478 above 1000"; Chipload "100 % below floor (rubbing)" under a green check |
| S6 | `before/S6_readiness.png` (operator) | `after/S6_readiness.png` | rivmap100 after the simulation, Readiness | Simulation up to date 4/4; tool load block: the worst rows by the one rank, all within |
| S7 | `before/S7_sim_idx2_warnings.png` (operator) | `after/S7_sim_warnings.png` | rivmap100, Simulation Warnings tab | 6 findings in caution, then the blue "wasted around" rows; each line elides |
| S8 | `before/S8_idx6_passes.png` | `after/S8_idx6_passes.png` | operator idx 6, Lake pocket 2 mm (no simulation) | caution face line "ramp entry · chip min 76 % under floor · may not occur"; "runs 990 to 1010 mm/min · 0 of 27580 above 1010 · as stated" |
| S8 | none | `after/S8_idx6_limits_open.png` | operator idx 6 | "slot ring · 6 within · 1 no vertical feed" as one closed line; ramp and helix rubbing 46 % under floor, advisory |
| S9 | `before/S9_sim_idx1_10g.png` (operator) | in `after/S5_sim_idx1_metrics.png` | rivmap100 idx 1, Width x depth chart | the field with 25 / 50 / 75 % lines, "100 % budget" in caution, the planned marks |
| S9 | `before/S9_sim_idx16_10g.png` (operator) | in `after/S5_sim_idx2_metrics.png` | rivmap100 idx 2, Width x depth chart | the field under "Vendor DOC 3xD"; no budget line (no force limit on this cut) |
| S10 | `before/S10_live_idx1_passes.png` | `after/S10_live_idx1_passes.png` | live file idx 1, 3D Rough ladder 8 > 2 | proposal ⚡ 2.58; danger face line "stated point · depth 33 % over vendor depth cap" |
| S10 | none | `after/S10_live_idx1_limits_open.png` | live file idx 1 | the stated point: "depth 33 % over vendor depth cap" exceeds; body, planner cap, first slot rows |
| S11 | none | `after/S11_idx2_passes.png`, `after/S11_idx2_limits_open.png` | operator idx 2, Back rest 6 mm | danger face line "planner cap · chip max 2.9 % over limit · until modulated..." (elided); the planner cap chip max row exceeds |
| S11 | none | `after/S11_idx3_passes.png`, `after/S11_idx3_limits_open.png` | operator idx 3, Pin drill | no face line; the body and the drill cycle fold; "no cutting move · as stated" |
| S11 | none | `after/S11_idx17_passes.png`, `after/S11_idx17_limits_open.png` | operator idx 17, Outline profile with tabs | caution face line "plunge entry · plunge rate 17 % over limit · may not occur ..." (elided) |
| S12 | none | `after/S12_idx7_passes.png`, `after/S12_idx7_limits_open.png` | operator idx 7, Lake pocket 2 mm, 1 lake | danger "body · chip max 27 % over limit · until modulated"; "runs 1258 mm/min · 0 of 144 above 1258 · as stated" |
| S12 | none | `after/S12_idx8_passes.png`, `after/S12_idx8_limits_open.png` | operator idx 8, ANTENNA pocket | danger "body · chip max 27 % over limit · until modulated"; the ramp entry chip max exceeds, the helix one is advisory (may not occur) |
| S12 | none | `after/S12_readiness.png` | operator file, Readiness (no simulation) | Simulation "Not run"; the tool load block reads "no simulation"; the export warning is not a readiness row |
| S12 | none | `after/S12_rivmap100_idx0_simulated_passes.png` | rivmap100 Face 5 after the simulation, Passes | danger "ramp entry · rubbing 44 % under floor"; caution "runs 2333 to 3000 mm/min · 77 of 77 above 2200", "feeds: modulated, run 1" |
| S13 | none | `after/S13_idx4_passes.png`, `after/S13_idx9_passes.png`, `after/S13_idx10_passes.png`, `after/S13_idx11_passes.png`, `after/S13_idx13_passes.png` | operator idx 4, 9, 10, 11, 13 | the face line of the five ops that no other shot shows |
| E1 | none | `after/export_header_rivmap100.txt` | rivmap100 export after the simulation | four `export.stated_cut_over_limit` header lines (R8: rubbing rows, "no measured gate"); the "feeds run ... above the stated" lines of each op |

## The face line of each of the 18 operator ops (no simulation)

Danger is the red line with the X, caution the yellow line with "!".

The face line is the worst stated row that exceeds (the same rows as the
R2 to R5 "after" table of `review_2026-11-15/FIXES.md`). A card note is a
separate line of the card that is not a stated row (idx 5 and idx 9).

| idx | op | face line | other card note |
|---|---|---|---|
| 0 | Face back to T1 | none | none |
| 1 | Back bulk rough 18 mm | danger: planner cap · chip max 24 % over limit · until modulated | none |
| 2 | Back rest 6 mm | danger: planner cap · chip max 2.9 % over limit · until modulated (elided) | caution header: Move budget not met at the depth |
| 3 | Pin drill, flip pins | none | none |
| 4 | Drill light holes 6 mm end mill | none | none |
| 5 | River channel 6 mm, pass 7 of 7 | none | caution: Depth, power and deflection: judged after the simulation (elided) |
| 6 | Lake pocket 2 mm, 11 lakes | caution: ramp entry · chip min 76 % under floor · may not occur | none |
| 7 | Lake pocket 2 mm, 1 lake | danger: body · chip max 27 % over limit · until modulated | none |
| 8 | ANTENNA_D5.00 pocket 2 mm | danger: body · chip max 27 % over limit · until modulated | none |
| 9 | V rivers and lakes | none | caution: Engaged Ø 1.12 mm at DOC 3.17 mm; caution: Depth, power and deflection: judged after the simulatio... |
| 10 | Day strip slit V60 | none | status banner: Cuts below the stock |
| 11 | Face clamp pads 6 mm | caution: ramp entry · chip min 66 % under floor · may not occur | none |
| 12 | Face front to T | none | none |
| 13 | Engrave day labels V60 | none | none |
| 14 | Front bulk rough 18 mm | danger: planner cap · chip max 3.4 % over limit · until modulated (elided) | none |
| 15 | Front 3D rough | danger: planner cap · chip max 3.1 % over limit · until modulated | none |
| 16 | Front scallop finish | caution: body · chip min 16 % under floor | none |
| 17 | Outline profile with tabs | caution: plunge entry · plunge rate 17 % over limit · may not occur (elided) | none |

Ten ops carry a face line: six danger (idx 1, 2, 7, 8, 14, 15) and four
caution (idx 6, 11, 16, 17). As predicted in FIXES R2 to R5.


## What the operator is asked to judge

From `NEXT_SESSION.md` (rows "P29-10 GUI", "P29-11 and P29-last", "P30
modulation"):

1. The red and caution lines on the 18 ops (table above): are ten face
   lines (six danger, four caution) too many? The danger lines are the
   chip maximum "until modulated" (P30-2); the caution lines are entry rows
   that may not occur and a body chip floor. Two more ops show the caution
   note "judged after the simulation".
2. The two-part Limits drawer: "As the op stands" (the stated point and
   each declared cut) and "If you apply ⚡" (the proposal). Do the
   engagement floor shares ("width 40000 % of min engage floor") read as
   noise?
3. The "runs ... above the stated feed" line: after a simulation with
   modulation the exported feeds are above the stated feed (Face 5: 77 of
   77 cutting moves at 2333 to 3000 mm/min against 2200; Project Curve: 478
   of 478 at 3000 against 1000). May a move run faster than the typed feed
   (P30 options A to F)?
4. The export header (`after/export_header_rivmap100.txt`): the warning
   lines and the feeds lines of each op.
5. A row is over before the simulation and within after it, because the
   simulation changes the feed (the modulation design step).

## Seen on the shots (not changed in the product)

- Long Limits drawers (idx 1, 2, 14, 15) run past 2400 points: the "If you
  apply" part is under the frame.
- Face lines elide at the 340-point rail: idx 2, 5, 9, 14, 17.
- Engagement floor shares read as noise: "depth 20000 % of min engage
  floor", "width 40000 % of min engage floor".
- idx 6, 7, 8, 16 and live idx 1: the ramp and helix entry show two rows
  with the same face "depth inside, no bound here".
- idx 3 and idx 4 (drill ops with 118 and 463 moves) read "no cutting move ·
  as stated".
- rivmap100 Project Curve, Metrics: the Chipload card has a green check and
  "peak 35 % of limit" over "100 % below floor (rubbing)"; the end tick of
  the axis ("▸0.076") overlaps a digit. The Cut groups list has no rubbing
  row, and the Readiness tool load block is all within, while the export
  warns for the rubbing row (R8). The cut group "entry · deflection 0 % of
  limit" reads "0 %".
- rivmap100 Scallop Finish: "Width 1 to 123 % of D · peak 123 %" (a sample
  wider than the tool); the "runs ..." line elides.
- rivmap100 Face 5, Passes after the simulation: "Operating point
  (measured) · Achieved peak chip 0.0667 mm/t · ! within band (b..." reads a
  caution chip on a "within" word, elided.
- S7: every Warnings line elides at the inspector width; the line
  "plunge-class motion descends faster than this op's ..." shows twice.
- The status bar reads "Modified" after `generate_all` (not saved).
- In the export, the header of the next op (name and feeds lines) comes
  before its `M5` and `TOOL CHANGE` lines.
- S9 has no file of its own: the chart is in the S5 shots of idx 1 and 2.
- `S5_sim_idx2_metrics.png` (1400 points) ends above "Other limits", so the
  closed "Cut groups" line is not in that shot; the groups-open shot shows
  it.
- A transient "RCH running" job was in the first S4 idx 16 Passes shot; it
  was shot again with no job.
