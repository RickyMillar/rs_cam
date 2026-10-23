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
