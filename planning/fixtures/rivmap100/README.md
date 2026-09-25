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
