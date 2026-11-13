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
