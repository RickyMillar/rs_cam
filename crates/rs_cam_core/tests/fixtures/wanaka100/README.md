# wanaka100: the operator's Wanaka terrain models

Until 2026-09-25 these files lived only on the operator's machine
(`~/Downloads/wanaka100/`), and the Wanaka fixtures named them by that
absolute path. On any other checkout the models loaded with no bounding
box, and a test that needs one (for example the stepover back-off in
`suggest_feed_matches_final_geometry`, deleted in feeds contract P5) read
red or skipped.

| File | Read by |
|---|---|
| `rivmap_export/terrain.stl` (11 MB) | the three Wanaka projects; `wanaka_z_layer_render` (`agent_search_axial_doc` left with AgentSearch in removal step R4) |
| `rivmap_export/holes.dxf`, `rivmap_export/rivers.dxf`, `lakes.dxf` | the three Wanaka projects |
| `wanaka_full_tuned.toml` | `feed_modulation_cycle_time_f036c` (not ignored: it skipped off the operator's machine, now it runs), `kinematics_histogram`, `machine_kinematics_cycle_time_f034`, `wanaka_axial_doc`, `wanaka_boundary_diag`, `wanaka_e2e_chipload_gate`, `wanaka_step4_fa_revalidation`, `wanaka_step5_mc_revalidation` |

The dated snapshots one folder up (`wanaka_2026-08-16_afc6ecd4.toml`,
`wanaka_airrun_2026-06-01_bb713634.toml`) read these models by the relative
path `wanaka100/...`. A project path resolves against the project file.
The model paths changed first (the names kept their old hash suffix). Removal
step R1 (2026-10-28) then changed only `clearing_strategy` `agent_search` →
`geodesic_adaptive`, because the strategy was removed, and renamed each file
to its new sha256 prefix: `wanaka_2026-08-16_afc6ecd4.toml` (derived from
`f530995a`) and `wanaka_airrun_2026-06-01_bb713634.toml` (derived from
`2c908dca`).

This terrain is not `tests/fixtures/terrain.stl` (a different sha256).
