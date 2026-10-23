# Defects filed from the Phase 0 baseline (2026-10-23, operator ruling: file now)

Evidence: `phase0/<fixture>.html`, `phase0/PHASE0_NOTES.md`, the saved
projects in `phase0/projects/` and the exported G-code in `phase0/gcode/`.
All on HEAD 61a8d4b4 (GUI binary mtime 1792706351.82). Move numbers are
op-local (section line i + 1 is move i, see the page header).

| Tag | Defect | Where seen | Evidence |
|---|---|---|---|
| G-LINKPLOUGH | A keep-down link of the DEFAULT 3D Rough (ContourParallel) cuts through standing stock. The simulation triage reports `project.link_cuts_stock`. | wanaka200 "2 Back Rough": CRITICAL, 20 samples on 12 link moves over 5.96 mm; worst link move 5147 removes 11.26 mm at (216.9, 55.0, 8.63). Corne "3D Rough 1": link move 2320 removes 6.50 mm at (21.4, −107.4, 11.0), limit 6.30; "3D Rough 1 (copy)": 3 links, worst move 3153 removes 3.38 mm, limit 3.30. | `phase0/gcode/wanaka200_contour_parallel.nc`, `phase0/gcode/corne_contour_parallel.nc`; projects `phase0/projects/{wanaka200,corne}_contour_parallel.toml` |
| G-SPUR | Standing spurs: thin slivers (0.3–1.0 mm wide) stay at the same XY on every level and stack to 13–16.5 mm. They can snap off. Operator saw them live at ~7 min. AgentSearch only; ContourParallel none; ContourSpiral nubs ≤ 3.4 mm. | Corne "3D Rough 1 (copy)" AgentSearch: wedge at (104.5, 86.0), 13 mm at move 3577 (7:32), 16.5 mm at the end; passes come within 3.2–3.6 mm of the spot with R 3.0. wanaka200 AgentSearch back rough: 15.6 mm at the back edge band. | `phase0/img/corne_agent_search_crop_spurs_gui_420s.png`, `..._spurs_420s_zoom.png`, `phase0/tools/spurs.py`, `phase0/data/*_n5.json` |
| G-HOLECORE | Hex-hole cores left standing: a central pillar remains inside a hex switch hole after the rough. AgentSearch about a third of the holes, ContourSpiral 2 holes, ContourParallel none. A coverage failure visible on the finished part. | Corne "3D Rough 1 (copy)" final stock. | `phase0/img/lead_corne_as_stock_top_zoom.png`, `lead_corne_cs_stock_top_zoom.png`, `lead_corne_cp_stock_top_zoom.png` |
| G-OUTERBAND | The band outside the case wall is left standing with round holes punched into it (N7). AgentSearch and ContourSpiral; ContourParallel clears it. | Corne, both roughs. | `phase0/img/corne_agent_search_3DRough1_crop_2.png` and crop 3 (ContourParallel for comparison) |
| G-AXIALDOC | Peak axial depth of cut above the commanded depth per pass, read by `narrate_toolpath`. | Corne ContourParallel "3D Rough 1": 11.25 mm vs 6.0 commanded. wanaka200 AgentSearch back: 10.92 vs 5.46; front 7.91 vs 4.2. rivmap100 AgentSearch 3.0 vs 2.0; ContourSpiral 2.5 vs 2.0 (move 33879, Z 6). Also `crosses_standing_material` 4 (peak 15.0 mm at move 1401) on Corne AgentSearch copy. | the pages' "Information" and "Bar 3b" rows |
| G-WALLDIP (N6) | The Corne copy op runs along the outer wall at Z −6.0 and dips to Z −7.5 at plunge feed about every 19 mm (vertical ticks on the wall in the copy cut view). | Corne ContourParallel "3D Rough 1 (copy)". | `phase0/gcode/corne_contour_parallel.nc` after the tool change |
| G-HOPRETRACT (N8) | 1 726 G0 runs on one rough, 964 to safe Z and the rest 0.5 mm hops above the level: the links view is a solid wall of hops. | wanaka200 ContourParallel "2 Back Rough". | `phase0/img/wanaka200_contour_parallel_2BackRough_links.png` |
| G-SIMCYCLE | The same toolpath gives two cycle times: 2 022.1 s then 1 563.6 s on a re-run, because the feed modulation reads the previous simulation. | Corne ContourParallel. | `phase0/PHASE0_NOTES.md` "Surprises" |

Not defects but instrument gaps (ruling 2026-10-23: build before Phase 2):

- G-ROUGHCOV: no coverage instrument reaches a 3D Adaptive op through
  MCP; every per-toolpath coverage field is null. `metrology/union_coverage.rs`
  exists (whole-board stock vs model + stock_to_leave) but is test-support only.
- G-ROUGHGOUGE: `ModelGougeRule::Exempt` on `REG_ADAPTIVE3D` (and on every
  other registry entry read on 2026-10-23); no gouge check of emitted moves
  against the model mesh runs for a rough.
- G-THINSTAND: no detector for thin standing material (G-SPUR) in the
  simulation triage; the Phase 0 agent's `phase0/tools/spurs.py` heightmap
  replay is the only measure.

## Handed over by the geometry-selection lead (2026-10-23 15:35), adaptive/ owner = this research

| Tag | Defect | Evidence |
|---|---|---|
| G-MOPLOAD | Sentry `a_clearing_cut_holds_the_pass_load_g_adaptpassload` red since 2026-10-20 (not S11): two "Residue mop" moves (2810, 6159) exceed the pass load by the exact oracle, peak 0.9404 vs bound 0.5460. A REAL kernel finding: the residue mop breaks the `adaptive/` invariant `step_within_pass_load`. The mop is the V1 mechanism, so this is on the research's path. Candidate commits before 10-20: b5dc7ed4, bd7cf9f4, b523905a. | peer's agent, `planning/geometry_selection_2026-10-16/PLAN.md` ~589-591 |
| G-RAPIDLIFTPIN | Sentry `a_rapid_leaves_cut_depth_straight_up_g_adaptrapidlift` red since 2026-10-20: a sim-only flag on move 6930; the exact oracle clears it by 0.2495 mm; the pin `SIM_ONLY_FLAGS=0` sees count 1. Needs a re-pin or a cause hunt. | same |

`added_material_cuts_as_stock_s6` was fixed by the peer in 0e6788b2.

| G-BACKTRAVEL (N11) | Operator, live GUI 2026-10-23 15:45, wanaka200 "2 Back Rough" (ContourParallel): "second half of Back Rough 2 is doing huge travel across the board where it could do one waterline-like sweep." Capture pending (agent E): the level where it starts, retracts and rapid distance per level, region order. Related: G-HOPRETRACT (N8). | `phase0/img/wanaka200_cp_back_second_half_rapids.png` (pending) |

## From the Phase 1.5 evidence run (P15_E_evidence.md, 2026-10-23 15:55)

| Tag | Defect | Evidence |
|---|---|---|
| G-SHELFSCAN (N11 cause) | At the third level of the wanaka200 back rough the ContourParallel planner leaves its 2.0 mm rows and runs a one-way +X scan with 0.5 mm rows over the whole board while 3.6 % of the level holds material; short cuts (median 2.24 mm), a lift to 5 mm and a rapid to the next piece: 1 581 rapid runs, ~115 m rapid travel on that level. The planner step that switches is not yet found. | `phase0/img/wanaka200_cp_back_second_half_rapids.png`, P15_E §N11 |
| G-WALLSTRIP | Green mid-height strips stand inside the Corne case wall on AgentSearch and ContourSpiral; not named in Phase 0. | `phase0/img/corne_agent_search_p15_stock.png` |
| G-ROUGHGOUGE-STRICT | The new Allowance rule blocks export on every rough: wall-graze depth overstated (Z-only), and the blocking class applies inside the leave band. Fix in progress (B2). | P15_E §gouge |
| G-ROUGHCOV-WIRING | `triage.coverage` is `[]` in the GUI: the viz hands the session an empty checkpoint list. Fix in progress (A2). | P15_E §coverage |

| G-WALLSTRIP-PLAN | The ContourParallel 3D Rough leaves real 0.6–1.4 mm strips of stock against vertical walls (E2, Corne zero control: confirmed against the model). Reachable material the planner left; suspected grid-cell inset or the last pass stopping short of the boundary. Coverage reads them honestly. | P15_E_evidence.md §E2, `phase0/img/corne_contour_parallel_p15b_islands.png` |

| G-RESLINKLOAD | In the research harness (geodesic morph columns on the 2D star) the shipped residue-contour cleanup emits a ~27 mm LINK from (48.00, 67.43) to (69.51, 51.19) that reads 0.55–0.97 of D against the 0.3626 pass-load ceiling (17 steps over). The cleanup is shipped `adaptive/path.rs` code; the candidate only changed what residue it meets. Check whether a shipped strategy can emit the same link class (the G-ADAPTLINKLOAD sentry covers links, not this cleanup path?). | P15_D_verify.md §D4 |
