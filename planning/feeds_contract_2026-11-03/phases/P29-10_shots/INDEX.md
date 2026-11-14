# P29-10 screenshots: the index

Every "after" shot comes from ONE build: `rs_cam_gui` release, built from the
P29-10 review tree (HEAD `4a6050c6b` plus the review patch), binary mtime
2026-11-14 02:19. `gui_status` read "binary not newer than the process" before
the shots. The operator file and the live file were loaded read-only; nothing
was saved. Three GUI processes of that one binary took the shots: S1 (the
first process, which the kernel OOM killer stopped later), S1b to S9 (the
second), S10 (the third). The window is 1600 x 1400 points (2200 or 2400
points high for an open drawer). The simulation views read the simulation that
`generate_all` left (its fixpoint ran 3 simulations; the GUI read "18 traces",
"live", Readiness "Simulation: Up to date · 18/18 operations"). A separate
`run_simulation` was not used (see "Not shot"), so these views are not proven
equal to the views after a `run_simulation`.

Commits: 10b `2853ab8b7`, 10c `ac23e7a4e`, 10e `aa60b4da8`, 10f `a381097c7`,
10g `de37ede1c`; "review" is the review patch of 2026-11-14 (not committed).

The "before" files were shot on a GUI built from `ef25930d6` (commit
`9750a44dc`). A before file named `*_10g` is the 10g before set.

| Shot | File before | File after | Op | What to look at | Commit |
|---|---|---|---|---|---|
| S1 | `before/S1_idx1_passes.png` | `after/S1_idx1_passes.png` | operator idx 1, Back bulk rough 18 mm | the SAFE CUT header has no `MayOccur` sentence; ONE face line "stated point · depth 387 % over force cap" in caution | `2853ab8b7`, `a381097c7` |
| S1b | `before/drawer_idx1_open_10g.png` | `after/S1b_idx1_limits_open.png` | operator idx 1 | "As the op stands": the stated 10 x 5.5 mm rows; "If you apply ⚡ · 5 over": the proposal rows by cut; a quiet cut is one closed line, elided with the full text on hover | `a381097c7`, `de37ede1c`, review |
| S2 | `before/S2_idx14_passes.png` | `after/S2_idx14_passes.png` | operator idx 14, Front bulk rough 18 mm | one face line "stated point · depth 73 % over force cap" | `a381097c7` |
| S2 | none | `after/S2_idx14_limits_open.png` | operator idx 14 | the drawer by cut; the ramp entry rows carry "worst case" in the cut title | `a381097c7`, `de37ede1c` |
| S2 | `before/S2_idx15_passes.png` | `after/S2_idx15_passes.png` | operator idx 15, Front 3D rough | one face line "stated point · depth 57 % over force cap" | `a381097c7` |
| S2 | none | `after/S2_idx15_limits_open.png` | operator idx 15 | the drawer by cut (taller than 2400 points: the tail is under the frame) | `a381097c7`, `de37ede1c` |
| S3 | `before/S3_idx0_passes.png` | `after/S3_idx0_passes.png` | operator idx 0, Face back to T1 | no face line (no stated row exceeds); "Limits · 51 · 2 over" | `a381097c7` |
| S3 | none | `after/S3_idx0_limits_open.png` | operator idx 0 | the entry chip rows read "chip min < 0.1 % under floor" in caution (advisory), not danger | `a381097c7` |
| S3 | `before/S3_idx12_passes.png` | `after/S3_idx12_passes.png` | operator idx 12, Face front to T | as idx 0 | `a381097c7` |
| S3 | none | `after/S3_idx12_limits_open.png` | operator idx 12 | as idx 0 | `a381097c7` |
| S4 | `before/S4_idx16_passes.png` | `after/S4_idx16_passes.png` | operator idx 16, Front scallop finish | the SAFE WIDTH bar; "Limits · 39 · 2 over" | `a381097c7` |
| S4 | none | `after/S4_idx16_limits_open.png` | operator idx 16 | the rows with no number fold per cut; the helix entry chip rows in caution | `a381097c7`, `de37ede1c` |
| S4 | `before/S4_idx5_passes.png` | `after/S4_idx5_passes.png` | operator idx 5, River channel 6 mm | the caution line "Depth, power and deflection: judged after the simulation" (elided) | `a381097c7` |
| S4 | none | `after/S4_idx5_limits_open.png` | operator idx 5 | "Limits · 26": the stated part is one "no number · 8" line; the cuts fold | `a381097c7`, `de37ede1c` |
| S5 | `before/S5_sim_idx1_metrics.png` | `after/S5_sim_idx1_metrics.png` | operator idx 1, playhead at its start | "Other limits": body and link depth rows; "Cut groups · 21" open with each group a row, `NotJudged` drill rows | `aa60b4da8` |
| S5 | `before/S5_sim_idx2_metrics.png` | `after/S5_sim_idx2_metrics.png` | operator idx 2, playhead at its start | the entry group of idx 2 is its own row ("entry · depth 383 % over force cap") | `aa60b4da8` |
| S6 | `before/S6_readiness.png` | `after/S6_readiness.png` | operator, whole run | the tool load block: the worst rows by the one rank | `aa60b4da8` |
| S7 | `before/S7_sim_idx2_warnings.png` | `after/S7_sim_idx2_warnings.png` | operator idx 2 | the Warnings tab (578 safety findings fill the list) | `ac23e7a4e` |
| S8 | `before/S8_idx6_passes.png` | `after/S8_idx6_simulated_passes.png` | operator idx 6, Lake pocket 2 mm (the simulation was present) | the slot ring mark at width D beside the proposal | `de37ede1c` |
| S8 | none | `after/S8_idx6_simulated_limits_open.png` | operator idx 6 (the simulation was present) | "slot ring · 6 within · 1 no vertical feed" as one closed line | `de37ede1c` |
| S9 | `before/S9_sim_idx1_10g.png` | `after/S9_sim_idx1_chart.png` | operator idx 1, playhead at its start | the width x depth chart: the planned marks at the stated point over the samples; the 100 % budget line in caution | `de37ede1c` |
| S9 | `before/S9_sim_idx16_10g.png` | `after/S9_sim_idx16_chart.png` | operator idx 16, playhead at its start | the field draws with faint 25 / 50 / 75 % lines; no "budget, no limit here" line on this op (the field stays under 100 %) | `de37ede1c` |
| S10 | `before/S10_live_idx1_passes_10g.png` | `after/S10_live_idx1_passes.png` | live file, 3D Rough ladder 8 > 2 | the proposal ⚡ 2.58 (2.585 mm); the face line "stated point · depth 33 % over vendor depth cap" in danger | `a381097c7`, `de37ede1c` |
| S10 | none | `after/S10_live_idx1_limits_open.png` | live file, 3D Rough ladder 8 > 2 | the "first slot" and "planner cap" rows with "chip max .. over limit" in danger; "proposal 2.59 x 1.20 mm" | `a381097c7`, `de37ede1c` |
| S11 | 10i or 10h shots (deleted) | `after/S11_idx1_passes.png`, `after/S11_idx1_limits_open.png` | operator idx 1, 10z (rulings Q2, R, T) | face line: planner cap · depth 387 % over force cap (caution where there is one); ruling R: the planner cap always occurs, so it ranks before the first slot (420 %, may not occur); part one opens with the title "the stated point" (ruling T); "chip max 21 % over limit" (body) and "24 %" (planner cap) read the caution chip "advisory" (Q2, before modulation) | release build 2026-11-14 06:48 of the 10z tree (`gui_status`: binary not newer than the process) |
| S11 | 10i or 10h shots (deleted) | `after/S11_idx2_passes.png`, `after/S11_idx2_limits_open.png` | operator idx 2, 10z (rulings Q2, R, T) | face line: ramp entry · depth 100 % over force cap · worst case (caution where there is one); ruling R: the ramp entry always occurs and is further over than the planner cap (74 %); the planner cap "chip max 2.9 % over limit" is advisory (Q2) | release build 2026-11-14 06:48 of the 10z tree (`gui_status`: binary not newer than the process) |
| S11 | 10i or 10h shots (deleted) | `after/S11_idx7_passes.png`, `after/S11_idx7_limits_open.png` | operator idx 7, 10z (rulings Q2, R, T) | face line: body · chip max 27 % over limit · before modulation (caution where there is one); Q2: the body chip max row now ranks, in caution; the ramp chip min row (-76 %) may not occur, so the body ranks first (ruling R) | release build 2026-11-14 06:48 of the 10z tree (`gui_status`: binary not newer than the process) |
| S11 | 10i or 10h shots (deleted) | `after/S11_idx8_passes.png`, `after/S11_idx8_limits_open.png` | operator idx 8, 10z (rulings Q2, R, T) | face line: body · chip max 27 % over limit · before modulation (caution where there is one); Q2: before, no face line (the chip max rows were held off the face); the body, slot ring, ramp and helix chip max rows read advisory | release build 2026-11-14 06:48 of the 10z tree (`gui_status`: binary not newer than the process) |
| S11 | 10i or 10h shots (deleted) | `after/S11_idx14_passes.png`, `after/S11_idx14_limits_open.png` | operator idx 14, 10z (rulings Q2, R, T) | face line: ramp entry · depth 78 % over force cap · worst case (caution where there is one); ruling R, as idx 2; the planner cap "chip max 3.4 % over limit" is advisory | release build 2026-11-14 06:48 of the 10z tree (`gui_status`: binary not newer than the process) |
| S11 | 10i or 10h shots (deleted) | `after/S11_idx16_passes.png`, `after/S11_idx16_limits_open.png` | operator idx 16, 10z (rulings Q2, R, T) | face line: body · chip min 16 % under floor (caution where there is one); ruling R: the helix entry (chip min 91 % under, may not occur) ranks after the body | release build 2026-11-14 06:48 of the 10z tree (`gui_status`: binary not newer than the process) |
| S11 | 10i or 10h shots (deleted) | `after/S11_idx6_passes.png`, `after/S11_idx6_limits_open.png` | operator idx 6, 10z (rulings Q2, R, T) | face line: ramp entry · chip min 76 % under floor · worst case (caution where there is one); no change from 10i | release build 2026-11-14 06:48 of the 10z tree (`gui_status`: binary not newer than the process) |
| S11 | 10i or 10h shots (deleted) | `after/S11_idx17_passes.png`, `after/S11_idx17_limits_open.png` | operator idx 17, 10z (rulings Q2, R, T) | face line: plunge entry · plunge rate 17 % over limit · may not occur (caution where there is one); no change from 10i; the face line elides at the rail width ("may not occur ...") | release build 2026-11-14 06:48 of the 10z tree (`gui_status`: binary not newer than the process) |
| S11 | 10i or 10h shots (deleted) | `after/S11_idx0_passes.png`, `after/S11_idx0_limits_open.png` | operator idx 0, 10z (rulings Q2, R, T) | face line: none (caution where there is one); no change from 10i; the stated-point title over three rows | release build 2026-11-14 06:48 of the 10z tree (`gui_status`: binary not newer than the process) |
| S11 | 10i or 10h shots (deleted) | `after/S11_idx3_passes.png`, `after/S11_idx3_limits_open.png` | operator idx 3, 10z (rulings Q2, R, T) | face line: none (caution where there is one); no change from 10i; the body and the drill cycle fold as before | release build 2026-11-14 06:48 of the 10z tree (`gui_status`: binary not newer than the process) |
| S12 | none (new) | `after/S12_idx7_passes.png`, `after/S12_idx7_limits_open.png` | operator idx 7, P30-1 and P30-2, no simulation | face line: danger "body · chip max 27 % over limit · until modulated" (was caution "before modulation"); drawer: the body and slot ring chip max rows read "exceeds"; the feeds line "runs 1258 mm/min · 0 of 144 above 1258 · as stated" in the muted colour | release build 2026-11-14 23:54 of the P30-2 tree (`gui_status`: binary not newer than the process) |
| S12 | none (new) | `after/S12_idx8_passes.png`, `after/S12_idx8_limits_open.png` | operator idx 8, P30-1 and P30-2, no simulation | face line: danger "body · chip max 27 % over limit · until modulated"; drawer: body and slot ring "exceeds", the ramp and helix chip max rows stay "advisory" (worst case, may not occur); feeds line "runs 1258 mm/min · 0 of 200 above 1258 · as stated" | release build 2026-11-14 23:54 of the P30-2 tree (`gui_status`: binary not newer than the process) |
| S12 | none (new) | `after/S12_idx1_passes.png`, `after/S12_idx1_limits_open.png` | operator idx 1, P30-2, no result | face line: danger "planner cap · chip max 24 % over limit · until modulated" (was caution "planner cap · depth 387 % over force cap"); drawer: body 21 % and planner cap 24 % chip max "exceeds", first slot 31 % stays "advisory" (may not occur); no feeds line (no result) | release build 2026-11-14 23:54 of the P30-2 tree (`gui_status`: binary not newer than the process) |
| S12 | none (new) | `after/S12_idx0_passes.png`, `after/S12_idx0_limits_open.png` | operator idx 0, P30-1, no simulation | face line: none; feeds line "runs 6488 mm/min · 0 of 60 above 6488 · as stated" (muted) | release build 2026-11-14 23:54 of the P30-2 tree (`gui_status`: binary not newer than the process) |
| S12 | none (new) | `after/S12_readiness.png` | operator file, Readiness | no check state changes; the new export warning is not a readiness row | release build 2026-11-14 23:54 of the P30-2 tree (`gui_status`: binary not newer than the process) |
| S12 | none (new) | `after/S12_rivmap100_idx0_simulated_passes.png` | rivmap100 Face 5 after one simulation with modulation (the other three ops disabled, cell 0.5 mm) | two quiet lines under the Limits row: "runs 2333 to 3000 mm/min · 77 of 77 above 2200" in the caution colour, then "feeds: modulated, run 1" muted | release build 2026-11-15 00:11 of the final P30-2 tree (`gui_status`: binary not newer than the process) |
| S12 | none (new) | `after/S12_rivmap100_idx0_sim_inspector.png`, `after/S12_rivmap100_idx0_sim_inspector_superseded.png` | rivmap100 Face 5, Simulation inspector: current (run 1), then after a second simulation at 0.4 mm | the same two lines under the Operation row; after the second run the stage line reads "feeds: modulated, run 1, not current" in the caution colour | release build 2026-11-15 00:11 of the final P30-2 tree (`gui_status`: binary not newer than the process) |

## Not shot


- The tool panel (`tool_panel_helix_10e2.png` in the 10e set): `set_ui_view`
  cannot select a tool, and no input tool can click the GUI on this Wayland
  session. The review patch does not change the tool panel.
- The Passes card at 240 points (review item 1): in the Toolpaths workspace a
  700-point window keeps the inspector at 340 points (the viewport dock
  overlaps it), and MCP has no door to resize the panel. The headless test
  `inspector_width_is_tab_independent_up4` holds the 240-point layout.
- S8 before the simulation: the simulation of `generate_all` was present for
  every Toolpaths shot, so no S8 shot shows the generated state alone.
- A fresh `run_simulation`: the first call OOM-killed the GUI (kernel OOM,
  15 GB anonymous memory, about 14 GB free on the machine). The simulation
  shots read the full run of `generate_all`.

## Seen on the shots, outside the five review items

- S7: 578 "The shank meets the stock" safety findings fill the Warnings list,
  so the headline and the entry-group findings of idx 2 are not in view.
- S9 idx 16: the cut force card reads "Width 1 to 154 % of D · peak 154 %": a
  sample wider than the tool diameter.
- S5, S9 (1400 points high, shot after a 2200-point shot): a black band about
  60 points high under the timeline. The window resize leaves it.
- S1, S1b: the hover text of "Generate All" covers the "1 Back" setup header
  (the pointer rests on the button).
- S1: the transient "Computing faces — 0 s" job and the pick bar text
  "Computing faces..." (the shot came right after the load).
- S4 idx 16 and idx 5: the status bar reads "RCH running" (the reach job) and
  the dock shows "Cancel all jobs".
- At 340 points the longer quiet-cut headers of the Limits drawer elide
  ("plunge entry · may not occur · 2 within · 5 not mod..."); the full text is
  on the hover (review item 1).
- S3: the Face entry chip rows read "chip min < 0.1 % under floor" with no
  "stored feed" word; section 7 of the design expects "· stored feed".
- S11 (10z): under the new title "the stated point" the engagement floor rows
  read large shares ("depth 12000 % of min engage floor", "width 40000 % of
  min engage floor" on idx 3 and idx 0). They are within; the share of a
  floor that holds by a wide margin reads as noise. 10z does not change them.
- S11 (10z) idx 17: the face line "plunge entry · plunge rate 17 % over limit
  · may not occur" (58 characters) elides at the 340-point rail.
- S12 (P30): a first build printed the feeds line as one line, and after a
  simulation it elided its stage ("... above 2200 · modul..."). The final build
  prints the stage on its own line after a feed stage; with the stated feeds
  the one line fits ("... · as stated").
- S12 (P30): on rivmap100, a second simulation with modulation at another
  cell size (0.2 to 0.5 mm, and again 0.5 to 0.4 mm) left Face 5 "modulated,
  run 1, not current". The reason was not traced; a likely cause is that the
  second pass changed no feed, so the slot kept the tag of run 1 with its old
  stock record. The export names it in the feeds caution.
