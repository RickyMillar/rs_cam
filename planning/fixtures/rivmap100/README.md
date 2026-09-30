# rivmap100: the roughing benchmark project

The operator's terrain project. The roughing stream (3D Rough, By Area,
entries, G-PLANSIMGAP) measures every change on it. Until 2026-09-25 it
lived only on the operator's machine (`~/Downloads/aspiring/rivmap100/`)
and in a session scratchpad; it is here so that every session, local or
cloud, reads the same file.

| File | What |
|---|---|
| `rivmap100.toml` | The operator's project as saved (2026-09-24). |
| `rivmap100_live_0925.toml` | The copy most 2026-09-25 measurements use (`planning/entry_stock_awareness_2026-09-24/`, `planning/by_area_merge_tree_2026-09-25/`). Rough = toolpath 1, finish (Scallop) = toolpath 2. |
| `rivmap100_live_0925_notsp.toml` | The same, one setting changed for an A/B arm (diff it against the live copy). |
| `rivmap100_ladder_demo.toml` | The step-ladder demo (the ladder is removed; the census probe still reads this file). |
| `rivmap100_single_step.toml` | The single-step arm of the ladder measurement. |
| `rivmap100_memory_small.toml` | G-SIMMEM baseline (2026-09-30): the live copy with the Scallop on and fresh stock, Face heights auto. 2.0 M cut samples before the fix. |
| `rivmap100_memory_repro.toml` | G-SIMMEM repro: the terrain scaled x3.5 (`units = custom 3.5`, 350 x 350 x 42 mm), stock 380 x 510 x 46 mm (760 x 1020 columns at 0.5 mm). Before the fix it aborts under an 8 GiB cap on a 9.5 GB allocation. Run it ONLY under a cap (`planning/sim_memory_2026-09-30/RESULTS.md`). |
| `rivmap100_tiered_finish.toml` | Tiered-finish benchmark (2026-09-30, `planning/tiered_finish_2026-09-30/`): the live copy plus tool 12, the R2.0 tapered ball of the wanaka library (`crates/rs_cam_core/tests/fixtures/t3b_r10_scallop_islands_2026-09-08_e4d817e9.toml`, tool 4), and the two tier ops `plan_multitool_finishing` emits for the ladder [12, 6] at tolerance 0.15, every other dial at the planner default. The operator's 500 x 500 project is not in the repo; this is the proxy. Rebuild the tier ops with the ignored test `write_rivmap100_tiered_finish_fixture` in `crates/rs_cam_core/tests/tier_band_overlap_g_overlapfill.rs`. |
| `rivmap_export/terrain.stl`, `rivers_aligned.dxf`, `machinable_edge_band.dxf` | The models the projects import (relative paths). |
| `rivmap_export/rivmap_data.toml` | Metadata from the terrain generator. rs_cam does not read it; its image paths point at the operator's machine. |
| `arm.sh`, `fmt.py` | One-line `rough-score` summary of toolpath 1. |

Typical run (release CLI, resolution 0.5 mm, depth per pass 8 mm):

```
cargo build --release -p rs_cam_cli
F=rivmap100_live_0925.toml planning/fixtures/rivmap100/arm.sh global \
  --set 1.depth_per_pass=8 --set 1.region_ordering=global
```

`--set` takes `<index>.<param>=<value>`; the index is the toolpath position
in the session. In `rivmap100_live_0925.toml` the order is: 0 "Face 5" (id 7),
1 the 3D Rough (id 1), 2 "Scallop Finish" (id 2), 3 "Project Curve" (id 3).
`arm.sh` scores `--toolpath 1` (the rough, by id). Reference
times (2026-09-25, before G-PLANSIMGAP): Global 778 s, By Area 728 s (one
region), dpp 8.
