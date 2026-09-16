# P2 — the target layout for `crates/rs_cam_core/src/`

Status: **draft, awaiting operator ratification.** No file moves until the
operator ratifies this table. Programme: `planning/structure_2026-09-17/`,
work package P2.

This document is the complete move order. It assigns every one of the 115
root `.rs` files to one destination. It names every rename, every visibility
change, every path break and every sentry the move must repair.

## 1. The measurement (before)

| Quantity | Value |
|---|---:|
| `.rs` files directly in `crates/rs_cam_core/src/` | 115 |
| Lines in those files | 113 484 |
| `mod` declarations in `lib.rs` | 125 |
| `pub use` declarations in `lib.rs` | 1 (`pub use ids::ToolpathId;`) |
| Existing folders | 12 |

The existing folders are `adaptive/`, `adaptive3d/`, `compute/`,
`dexel_stock/`, `diagnostics/`, `feeds/`, `gcode/`, `material/`,
`metrology/`, `session/`, `tool/` and `tool_load/`. The layout keeps all
twelve and splits none of them. It changes one file inside an existing
folder: `material.rs` becomes `material/mod.rs`, at zero path cost.

`feeds/` and `tool_load/` belong to the power-calcs session. The layout
moves no file into or out of them, and it changes no formula inside them.
It does force a path rewrite on 29 files in those two folders. Section 5.1
measures that collision, and ratification question Q5 asks for the ruling.

Eleven of the twelve folders use `mod.rs`. `material/` is the one
exception: it has a sibling `material.rs` instead.

## 2. The rules this layout follows

1. **One destination per file.** Every root file moves, or the table gives
   the reason it stays.
2. **The root holds the crate spine only.** The spine is the vocabulary
   that every layer names: the geometry types, the toolpath IR, the ids,
   the cancellation contract and the measurement contract. Ten files,
   including `lib.rs`.
3. **A folder is a layer, not a topic.** The folder set follows the layer
   order that the `lib.rs` header already documents: import, tool model,
   operations, dressups, simulation, export.
4. **`folder/mod.rs` is a facade.** It holds `pub mod` lines and nothing
   else. It holds no `pub use` either, because `lib.rs` re-exports exactly
   one item today (`ToolpathId`), and `ids.rs` stays at the root.
5. **No re-export shim at an old path** (operator ruling, 2026-09-16). A
   caller of a moved module changes its path.
6. **A rename happens only where the file stem repeats the folder name.**
   Three files rename. Section 6 lists them.
7. **A dependency points down, not up.** Section 4 states the two places
   where the code forced a file out of its obvious folder.

## 3. The folder table

| Folder | Files | New? | What it holds |
|---|---:|:---:|---|
| root | 10 | — | `lib.rs` plus the crate spine |
| `geometry/` | 14 | new | 2D/3D primitives, grids, regions, boundary |
| `surface/` | 6 | new | Mesh sampling and surface fields |
| `maps/` | 11 | new | Grid-walk maps and their bounded caches |
| `ops/` | 16 | new | 2.5D and drilling operations |
| `finish/` | 20 | new | 3D finishing strategies and the finish planner |
| `dressups/` | 7 | new | Post-generation toolpath transforms |
| `stock/` | 10 | new | Stock simulation adjuncts, cut record, stock meshes |
| `io/` | 7 | new | File import and the on-disk libraries |
| `export/` | 4 | new | Non-G-code output and artifacts |
| `trace/` | 5 | new | Records that describe a generated toolpath |
| `machine/` | 4 | new | Machine profile, kinematics, strategy advice |
| `material/` | 1 | existing | `material.rs` becomes `material/mod.rs` |
| `adaptive/` | 5 | existing | unchanged |
| `adaptive3d/` | 4 | existing | unchanged |
| `compute/` | 21 | existing | unchanged |
| `dexel_stock/` | 9 | existing | unchanged |
| `diagnostics/` | 16 | existing | unchanged |
| `feeds/` | 17 | existing | unchanged — another agent owns it |
| `gcode/` | 7 | existing | unchanged |
| `metrology/` | 7 | existing | unchanged |
| `session/` | 11 | existing | unchanged |
| `tool/` | 6 | existing | unchanged |
| `tool_load/` | 34 | existing | unchanged — another agent owns it |

Root file count after the move: **10** (target: 12 or fewer).
`lib.rs` `mod` declarations after the move: **32** (23 folders + 9 files),
down from 125.

## 4. The per-file table

The columns are:

- **lines** — `wc -l`.
- **tests** — `Y` when the file holds an inline `#[cfg(test)]` module.
- **top `crate::` imports** — the three most frequent `crate::<target>`
  spellings in the file, with their counts.
- **core importers** — how many other files under
  `crates/rs_cam_core/src/` name `crate::<stem>`.
- **out-of-crate files** — how many files under `rs_cam_viz`, `rs_cam_cli`,
  `rs_cam_mcp`, `rs_cam_core/tests` and `rs_cam_core/benches` name
  `rs_cam_core::<stem>`. This is the break cost in files.
- **path edits** — occurrences, not files: `crate::<stem>` inside core /
  `rs_cam_core::<stem>` outside it.

### root

The root holds `lib.rs` and the crate spine. Every one of these nine files
is vocabulary that four or more layers name. `geo`, `polygon`, `mesh` and
`toolpath` carry 821 out-of-crate path occurrences between them. Moving
them buys no navigability: a reader who looks for `P3` or `Toolpath` looks
at the root first. `ids.rs` stays because `lib.rs` re-exports `ToolpathId`
from it. `interrupt.rs` is the cancellation contract that 41 core files
name. `measurement.rs` is the measurement contract that the core
`CLAUDE.md` calls out. `panic_message.rs` and `build_info.rs` are the
error-class and crate-identity files that the brief allows at the root.

The destination column repeats the file name, because these files do not
move.

| file | destination | lines | tests | top `crate::` imports | core importers | out-of-crate files | path edits (core / out) |
|---|---|---:|:---:|---|---:|---:|---:|
| `build_info.rs` | `build_info.rs` | 23 | N | — | 0 | 3 | 0 / 0 |
| `geo.rs` | `geo.rs` | 646 | Y | project_curve(1) pencil(1) crease_paths(1) | 128 | 223 | 0 / 0 |
| `ids.rs` | `ids.rs` | 30 | N | — | 54 | 103 | 0 / 0 |
| `interrupt.rs` | `interrupt.rs` | 65 | N | — | 41 | 1 | 0 / 0 |
| `measurement.rs` | `measurement.rs` | 1098 | Y | toolpath(5) geo(3) tool(2) | 8 | 10 | 0 / 0 |
| `mesh.rs` | `mesh.rs` | 1078 | Y | geo(1) | 57 | 142 | 0 / 0 |
| `panic_message.rs` | `panic_message.rs` | 36 | N | — | 2 | 2 | 0 / 0 |
| `polygon.rs` | `polygon.rs` | 3004 | Y | panic_message(4) scallop(1) pocket(1) | 57 | 102 | 0 / 0 |
| `toolpath.rs` | `toolpath.rs` | 1610 | Y | region_set(4) polygon(3) dressup(3) | 71 | 157 | 0 / 0 |

### `geometry/`

Primitives and the derived geometry that sits on them: polygons of regions, grids, marching squares, distance fields and the machining boundary. `geo.rs`, `polygon.rs` and `mesh.rs` stay at the root as spine, so this folder holds what is built ON the primitives, not the primitives themselves. `nn_order.rs` lands here because four layers share it (`finish/`, `dressups/`, `adaptive3d/`); a neutral folder avoids a layer-crossing dependency.

| file | destination | lines | tests | top `crate::` imports | core importers | out-of-crate files | path edits (core / out) |
|---|---|---:|:---:|---|---:|---:|---:|
| `arc_util.rs` | `geometry/arc_util.rs` | 142 | Y | geo(1) | 6 | 3 | 11 / 4 |
| `boundary.rs` | `geometry/boundary.rs` | 1293 | Y | polygon(8) toolpath_spans(4) mesh(4) | 6 | 10 | 15 / 11 |
| `contour_extract.rs` | `geometry/contour_extract.rs` | 607 | Y | fiber(2) marching_squares(1) geo(1) | 5 | 7 | 6 / 7 |
| `edge_distance.rs` | `geometry/edge_distance.rs` | 692 | Y | polygon(1) geo(1) | 2 | 0 | 2 / 0 |
| `enriched_mesh.rs` | `geometry/enriched_mesh.rs` | 705 | Y | polygon(1) mesh(1) geo(1) | 6 | 14 | 12 / 21 |
| `fiber.rs` | `geometry/fiber.rs` | 374 | Y | geo(1) | 3 | 2 | 4 / 2 |
| `grid2.rs` | `geometry/grid2.rs` | 377 | Y | — | 7 | 1 | 20 / 1 |
| `grid_field.rs` | `geometry/grid_field.rs` | 338 | Y | — | 7 | 9 | 9 / 11 |
| `marching_squares.rs` | `geometry/marching_squares.rs` | 324 | Y | geo(1) | 4 | 0 | 4 / 0 |
| `monotone_cells.rs` | `geometry/monotone_cells.rs` | 573 | Y | tool(1) region_set(1) polygon(1) | 2 | 3 | 10 / 3 |
| `nn_order.rs` | `geometry/nn_order.rs` | 1145 | Y | — | 5 | 0 | 11 / 0 |
| `point_runs.rs` | `geometry/point_runs.rs` | 295 | Y | — | 9 | 0 | 20 / 0 |
| `region_mask.rs` | `geometry/region_mask.rs` | 537 | Y | rest_field(2) polygon(1) grid_field(1) | 7 | 2 | 25 / 2 |
| `region_set.rs` | `geometry/region_set.rs` | 193 | Y | polygon(1) geo(1) boundary(1) | 17 | 17 | 29 / 51 |

### `surface/`

The layer that answers "what does the model surface look like here?": the two cutter walks, the slope and rest fields, the flow routing and the valley-reach policy. Every file in this folder imports downward only. The four surface analysers that lean UP on `finish/` — `classify_probe`, `crest_lines`, `pencil_dihedral`, `direction_field` — sit in `finish/` instead, because each one imports a finishing module (`finish_setup` 10 times, `pencil` twice, `scallop_isofield` three times).

| file | destination | lines | tests | top `crate::` imports | core importers | out-of-crate files | path edits (core / out) |
|---|---|---:|:---:|---|---:|---:|---:|
| `dropcutter.rs` | `surface/dropcutter.rs` | 1164 | Y | mesh(4) tool(3) interrupt(3) | 21 | 24 | 33 / 86 |
| `flow_accum.rs` | `surface/flow_accum.rs` | 402 | N | — | 0 | 4 | 0 / 7 |
| `pushcutter.rs` | `surface/pushcutter.rs` | 731 | Y | mesh(4) tool(2) interrupt(2) | 1 | 2 | 1 / 4 |
| `reach.rs` | `surface/reach.rs` | 1011 | Y | tool(2) compute(1) | 8 | 9 | 55 / 20 |
| `rest_field.rs` | `surface/rest_field.rs` | 2822 | Y | reach(25) grid2(14) measurement(12) | 13 | 11 | 51 / 23 |
| `slope.rs` | `surface/slope.rs` | 1249 | Y | mesh(4) interrupt(4) tool(2) | 13 | 6 | 24 / 7 |

### `maps/`

One grid walk labels every cell; these files build those maps, cut them into islands, render them and memoise them. `grid.rs` and `memo.rs` are private today and both stay private here, because every importer of each lands inside this folder. `tool_shape_key.rs` can narrow from `pub(crate)` to private for the same reason (section 7).

| file | destination | lines | tests | top `crate::` imports | core importers | out-of-crate files | path edits (core / out) |
|---|---|---:|:---:|---|---:|---:|---:|
| `finish_surface_cache.rs` | `maps/finish_surface_cache.rs` | 524 | Y | finish_setup(5) memo(3) tool(2) | 7 | 1 | 8 / 1 |
| `geom_cache.rs` | `maps/geom_cache.rs` | 477 | Y | memo(9) compute(5) boundary(3) | 8 | 3 | 22 / 11 |
| `grid.rs` | `maps/grid.rs` | 103 | N | interrupt(2) tier_map(1) reach_map(1) | 2 | 0 | 5 / 0 |
| `memo.rs` | `maps/memo.rs` | 207 | N | tier_map_cache(1) reach_map_cache(1) mesh(1) | 4 | 0 | 22 / 0 |
| `reach_map.rs` | `maps/reach_map.rs` | 1873 | N | tier_map(11) geo(5) dropcutter(3) | 6 | 10 | 10 / 22 |
| `reach_map_cache.rs` | `maps/reach_map_cache.rs` | 193 | N | memo(5) reach_map(3) tool_shape_key(2) | 2 | 3 | 2 / 4 |
| `rest_heatmap_mesh.rs` | `maps/rest_heatmap_mesh.rs` | 717 | Y | rest_field(3) tier_map(2) tier_islands(2) | 1 | 3 | 2 / 5 |
| `tier_islands.rs` | `maps/tier_islands.rs` | 1512 | Y | region_mask(8) finish_planner(8) tier_map(6) | 5 | 21 | 12 / 26 |
| `tier_map.rs` | `maps/tier_map.rs` | 1116 | Y | tool(5) rest_field(4) dropcutter(4) | 7 | 25 | 29 / 29 |
| `tier_map_cache.rs` | `maps/tier_map_cache.rs` | 300 | Y | memo(5) tier_map(4) tool_shape_key(1) | 7 | 2 | 14 / 2 |
| `tool_shape_key.rs` | `maps/tool_shape_key.rs` | 137 | Y | tool(2) tier_map_cache(1) finish_surface_cache(1) | 3 | 1 | 4 / 1 |

### `ops/`

The 2.5D and drilling operations, with the depth-stepping helper they all use. `adaptive_shared.rs` lands here: it is operation-layer engagement math that `adaptive/` and `adaptive3d/` share. Those two engines stay where they are; a later programme may fold them into `ops/` as `ops/adaptive/` and `ops/adaptive3d/`, and this placement does not block that.

`trace.rs` keeps its name at `ops/trace.rs` (section 6, ratification question Q3).

| file | destination | lines | tests | top `crate::` imports | core importers | out-of-crate files | path edits (core / out) |
|---|---|---:|:---:|---|---:|---:|---:|
| `adaptive_shared.rs` | `ops/adaptive_shared.rs` | 331 | Y | geo(1) feed_modulation(1) | 7 | 1 | 14 / 1 |
| `chamfer.rs` | `ops/chamfer.rs` | 317 | Y | toolpath(2) profile(1) polygon(1) | 1 | 1 | 2 / 1 |
| `depth.rs` | `ops/depth.rs` | 860 | Y | interrupt(3) toolpath(2) zigzag(1) | 8 | 4 | 19 / 5 |
| `drill.rs` | `ops/drill.rs` | 498 | Y | toolpath(2) geo(1) drill(1) | 8 | 9 | 20 / 13 |
| `drill_metrics.rs` | `ops/drill_metrics.rs` | 682 | Y | drill(4) tool_load(3) simulation_cut(3) | 6 | 3 | 15 / 3 |
| `drill_op.rs` | `ops/drill_op.rs` | 182 | N | toolpath_spans(1) material(1) drill(1) | 11 | 34 | 37 / 38 |
| `face.rs` | `ops/face.rs` | 857 | Y | geo(5) zigzag(2) toolpath(2) | 2 | 6 | 3 / 12 |
| `inlay.rs` | `ops/inlay.rs` | 702 | Y | toolpath(9) interrupt(2) zigzag(1) | 1 | 1 | 2 / 2 |
| `pocket.rs` | `ops/pocket.rs` | 1088 | Y | interrupt(3) depth(3) toolpath(2) | 8 | 6 | 9 / 11 |
| `profile.rs` | `ops/profile.rs` | 514 | Y | polygon(3) toolpath(2) depth(2) | 5 | 8 | 9 / 11 |
| `project_curve.rs` | `ops/project_curve.rs` | 400 | Y | toolpath(3) interrupt(2) tool(1) | 3 | 3 | 13 / 7 |
| `rest.rs` | `ops/rest.rs` | 677 | Y | zigzag(7) toolpath(4) rest_field(1) | 2 | 1 | 5 / 2 |
| `trace.rs` | `ops/trace.rs` | 533 | Y | toolpath(3) interrupt(3) polygon(2) | 2 | 1 | 4 / 2 |
| `vcarve.rs` | `ops/vcarve.rs` | 601 | Y | toolpath(9) zigzag(2) interrupt(2) | 2 | 2 | 3 / 3 |
| `waterline.rs` | `ops/waterline.rs` | 531 | Y | geo(5) toolpath(4) interrupt(3) | 5 | 3 | 7 / 4 |
| `zigzag.rs` | `ops/zigzag.rs` | 500 | Y | toolpath(2) polygon(2) depth(2) | 7 | 4 | 22 / 5 |

### `finish/`

The 3D finishing strategies, the unified planner and the surface analysers that the strategies own. Twenty files is under the 25-file threshold the brief set for a further split, so `finish/` does not divide into `finish/` and `rough/`. Roughing has no root files left to collect: `adaptive/` and `adaptive3d/` already hold it.

| file | destination | lines | tests | top `crate::` imports | core importers | out-of-crate files | path edits (core / out) |
|---|---|---:|:---:|---|---:|---:|---:|
| `classify_probe.rs` | `finish/classify_probe.rs` | 1068 | Y | finish_setup(10) tool(2) slope(2) | 3 | 16 | 6 / 18 |
| `conformal_spiral.rs` | `finish/conformal_spiral.rs` | 4401 | Y | direction_field(6) geo(3) scallop_math(2) | 1 | 7 | 5 / 12 |
| `crease_paths.rs` | `finish/crease_paths.rs` | 387 | Y | reach(13) pencil(7) geo(3) | 4 | 1 | 8 / 1 |
| `crest_lines.rs` | `finish/crest_lines.rs` | 1077 | Y | rest_field(2) pencil(2) mesh(2) | 4 | 0 | 12 / 0 |
| `direction_field.rs` | `finish/direction_field.rs` | 2062 | Y | crest_lines(4) scallop_isofield(3) pencil_dihedral(2) | 3 | 3 | 12 / 7 |
| `finish_planner.rs` | `finish/finish_planner.rs` | 1980 | Y | measurement(7) tier_islands(3) geo(3) | 5 | 28 | 20 / 36 |
| `finish_setup.rs` | `finish/finish_setup.rs` | 774 | Y | measurement(2) finish_surface_cache(2) classify_probe(2) | 10 | 29 | 35 / 38 |
| `horizontal_finish.rs` | `finish/horizontal_finish.rs` | 642 | Y | toolpath(8) geo(5) tool(2) | 1 | 1 | 2 / 2 |
| `pencil.rs` | `finish/pencil.rs` | 3627 | Y | rest_field(16) dexel_stock(13) toolpath(10) | 16 | 13 | 77 / 18 |
| `pencil_dihedral.rs` | `finish/pencil_dihedral.rs` | 634 | Y | pencil(2) rest_field(1) mesh(1) | 4 | 0 | 7 / 0 |
| `radial_finish.rs` | `finish/radial_finish.rs` | 581 | Y | toolpath(9) geo(6) mesh(3) | 1 | 1 | 2 / 1 |
| `ramp_finish.rs` | `finish/ramp_finish.rs` | 1310 | Y | measurement(7) toolpath(4) point_runs(4) | 5 | 5 | 10 / 6 |
| `scallop.rs` | `finish/scallop.rs` | 3568 | Y | measurement(21) scallop_math(15) surface_link(9) | 11 | 13 | 44 / 16 |
| `scallop_isofield.rs` | `finish/scallop_isofield.rs` | 599 | Y | slope(1) scallop(1) polygon(1) | 3 | 0 | 9 / 0 |
| `scallop_math.rs` | `finish/scallop_math.rs` | 342 | Y | scallop(1) | 4 | 8 | 20 / 11 |
| `spiral_finish.rs` | `finish/spiral_finish.rs` | 777 | Y | toolpath(9) mesh(3) tool(2) | 3 | 1 | 5 / 2 |
| `spiral_finish_compact.rs` | `finish/spiral_finish_compact.rs` | 947 | Y | conformal_spiral(5) direction_field(4) geo(1) | 0 | 2 | 0 / 3 |
| `steep_shallow.rs` | `finish/steep_shallow.rs` | 1475 | Y | toolpath(17) geo(5) interrupt(4) | 2 | 7 | 4 / 7 |
| `surface_link.rs` | `finish/surface_link.rs` | 1928 | Y | toolpath(32) geo(7) region_set(6) | 10 | 11 | 54 / 37 |
| `unified_finish.rs` | `finish/unified_finish.rs` | 4802 | Y | surface_link(12) compute(12) scallop(10) | 11 | 31 | 47 / 34 |

### `dressups/`

The post-generation transforms: entry and lead strategies, arc fitting, segment conditioning, feed optimisation, feed modulation, rapid ordering and the entry burial audit. The folder is plural so `dressup.rs` moves in unchanged. It is NOT named `post/`, because `gcode/post.rs` already owns that word for the G-code post-processor.

| file | destination | lines | tests | top `crate::` imports | core importers | out-of-crate files | path edits (core / out) |
|---|---|---:|:---:|---|---:|---:|---:|
| `arcfit.rs` | `dressups/arcfit.rs` | 1354 | Y | gcode(2) transform_provenance(1) toolpath_spans(1) | 3 | 5 | 4 / 5 |
| `condition.rs` | `dressups/condition.rs` | 419 | Y | toolpath(3) toolpath_spans(2) arcfit(2) | 4 | 1 | 4 / 1 |
| `dressup.rs` | `dressups/dressup.rs` | 4706 | Y | toolpath(25) tool(13) geo(7) | 10 | 11 | 35 / 14 |
| `entry_audit.rs` | `dressups/entry_audit.rs` | 265 | N | toolpath(1) polygon(1) geo(1) | 0 | 3 | 0 / 3 |
| `feed_modulation.rs` | `dressups/feed_modulation.rs` | 1318 | Y | tool_load(13) feeds(6) kinematic_utilization(4) | 6 | 46 | 19 / 59 |
| `feedopt.rs` | `dressups/feedopt.rs` | 590 | Y | toolpath(7) toolpath_spans(5) kinematic_utilization(5) | 1 | 2 | 2 / 2 |
| `tsp.rs` | `dressups/tsp.rs` | 1292 | Y | toolpath(6) toolpath_spans(2) nn_order(2) | 4 | 3 | 8 / 5 |

### `stock/`

The tri-dexel data types, the cut record, the triage over it and the meshes the simulation emits. The engine itself stays in `dexel_stock/`. `radial_profile.rs` lands here because eleven of its fifteen importers are `dexel_stock/` files. `collision.rs` lands here because it checks a cutter envelope against simulated material. `simulation.rs` is an 18-line re-export facade; see ratification question Q4.

| file | destination | lines | tests | top `crate::` imports | core importers | out-of-crate files | path edits (core / out) |
|---|---|---:|:---:|---|---:|---:|---:|
| `collision.rs` | `stock/collision.rs` | 1226 | Y | geo(13) dexel(5) tool(3) | 8 | 11 | 13 / 16 |
| `dexel.rs` | `stock/dexel.rs` | 1103 | Y | geo(2) dexel(1) | 17 | 9 | 36 / 13 |
| `dexel_mesh.rs` | `stock/dexel_mesh.rs` | 831 | Y | dexel(7) radial_profile(4) drill_op(4) | 5 | 4 | 8 / 5 |
| `dexel_mesh_mc.rs` | `stock/dexel_mesh_mc.rs` | 678 | Y | dexel(2) stock_mesh(1) dexel_stock(1) | 2 | 0 | 3 / 0 |
| `radial_profile.rs` | `stock/radial_profile.rs` | 239 | Y | tool(5) | 15 | 13 | 53 / 28 |
| `sim_measurability.rs` | `stock/sim_measurability.rs` | 569 | Y | simulation_cut(2) dexel_stock(2) ids(1) | 7 | 9 | 18 / 11 |
| `sim_triage.rs` | `stock/sim_triage.rs` | 1745 | Y | toolpath(10) geo(6) simulation_cut(5) | 2 | 5 | 7 / 7 |
| `simulation.rs` | `stock/simulation.rs` | 18 | N | radial_profile(2) arc_util(2) stock_mesh(1) | 0 | 3 | 0 / 8 |
| `simulation_cut.rs` | `stock/simulation_cut.rs` | 3136 | Y | simulation_cut(25) machine_kinematics(9) toolpath_spans(4) | 30 | 69 | 103 / 123 |
| `stock_mesh.rs` | `stock/stock_mesh.rs` | 438 | N | toolpath_spans(5) geo(2) toolpath(1) | 12 | 16 | 21 / 21 |

### `io/`

Every door that reads a file from disk: the three importers, the two TOML libraries and the directory mechanics they share. `named_toml_library.rs` is private today and stays private, because both importers land in this folder. `step_input.rs` carries `#[cfg(feature = "step")]`; the attribute moves onto its `pub mod` line in `io/mod.rs`.

`io.rs` renames to `io/model.rs` (section 6).

| file | destination | lines | tests | top `crate::` imports | core importers | out-of-crate files | path edits (core / out) |
|---|---|---:|:---:|---|---:|---:|---:|
| `dxf_input.rs` | `io/dxf_input.rs` | 1315 | Y | polygon(2) geo(1) | 5 | 16 | 11 / 30 |
| `io.rs` | `io/model.rs` | 289 | Y | svg_input(3) dxf_input(3) step_input(1) | 2 | 8 | 6 / 14 |
| `machine_library.rs` | `io/machine_library.rs` | 238 | Y | named_toml_library(4) tool_library(1) machine(1) | 1 | 5 | 1 / 13 |
| `named_toml_library.rs` | `io/named_toml_library.rs` | 80 | N | tool_library(1) machine_library(1) | 2 | 0 | 8 / 0 |
| `step_input.rs` | `io/step_input.rs` | 456 | N | enriched_mesh(4) geo(1) | 1 | 3 | 1 / 3 |
| `svg_input.rs` | `io/svg_input.rs` | 500 | Y | dxf_input(4) polygon(2) geo(1) | 1 | 3 | 3 / 3 |
| `tool_library.rs` | `io/tool_library.rs` | 523 | Y | named_toml_library(4) compute(2) | 2 | 6 | 2 / 17 |

### `export/`

Output that is not G-code: the SVG/HTML preview, the fingerprint diff, the G-code invariant validator and the shared JSON artifact writer. `gcode/` stays untouched, so the validator sits beside the other output surfaces rather than inside the emitter folder.

| file | destination | lines | tests | top `crate::` imports | core importers | out-of-crate files | path edits (core / out) |
|---|---|---:|:---:|---|---:|---:|---:|
| `artifact_io.rs` | `export/artifact_io.rs` | 119 | Y | simulation_cut(1) | 2 | 0 | 5 / 0 |
| `fingerprint.rs` | `export/fingerprint.rs` | 2042 | Y | geo(13) stock_mesh(8) dexel_stock(6) | 2 | 6 | 2 / 14 |
| `gcode_validator.rs` | `export/gcode_validator.rs` | 893 | Y | gcode(1) | 2 | 5 | 2 / 5 |
| `viz.rs` | `export/viz.rs` | 1331 | N | geo(2) toolpath(1) tool(1) | 2 | 6 | 3 / 10 |

### `trace/`

The records that describe a generated toolpath: the debug trace, the semantic trace, the spans, the narration and the transform provenance contract. This folder is the crate's most expensive move — 322 out-of-crate path occurrences — because `debug_trace` and `toolpath_spans` are named by 86 and 75 out-of-crate files.

| file | destination | lines | tests | top `crate::` imports | core importers | out-of-crate files | path edits (core / out) |
|---|---|---:|:---:|---|---:|---:|---:|
| `debug_trace.rs` | `trace/debug_trace.rs` | 566 | Y | — | 27 | 75 | 56 / 91 |
| `narrate.rs` | `trace/narrate.rs` | 2488 | Y | compute(52) debug_trace(6) simulation_cut(4) | 3 | 4 | 5 / 6 |
| `semantic_trace.rs` | `trace/semantic_trace.rs` | 1713 | Y | toolpath_spans(7) debug_trace(7) transform_provenance(6) | 14 | 20 | 36 / 60 |
| `toolpath_spans.rs` | `trace/toolpath_spans.rs` | 2350 | Y | geo(13) toolpath(6) tsp(2) | 41 | 86 | 115 / 142 |
| `transform_provenance.rs` | `trace/transform_provenance.rs` | 717 | Y | toolpath_spans(5) scallop(4) semantic_trace(3) | 14 | 16 | 26 / 23 |

### `machine/`

The machine profile, the kinematics model, the utilisation instrument and the strategy advisor that reads them.

`machine.rs` renames to `machine/profile.rs` and `machine_kinematics.rs` to `machine/kinematics.rs` (section 6).

| file | destination | lines | tests | top `crate::` imports | core importers | out-of-crate files | path edits (core / out) |
|---|---|---:|:---:|---|---:|---:|---:|
| `kinematic_utilization.rs` | `machine/kinematic_utilization.rs` | 793 | N | machine_kinematics(6) toolpath(1) tool_load(1) | 8 | 8 | 24 / 12 |
| `machine.rs` | `machine/profile.rs` | 461 | Y | machine_kinematics(2) | 31 | 40 | 66 / 59 |
| `machine_kinematics.rs` | `machine/kinematics.rs` | 1776 | Y | toolpath(5) kinematic_utilization(1) ids(1) | 18 | 43 | 64 / 92 |
| `strategy_advisor.rs` | `machine/strategy_advisor.rs` | 430 | Y | machine_kinematics(3) toolpath(2) machine(2) | 2 | 3 | 21 / 3 |

### `material/`

One normalisation, not a move. `material.rs` is the crate's only sibling-file module folder; it becomes `material/mod.rs` and matches the other eleven folders.

`material.rs` becomes `material/mod.rs`. The module path `crate::material` does not change, so this costs zero path edits.

| file | destination | lines | tests | top `crate::` imports | core importers | out-of-crate files | path edits (core / out) |
|---|---|---:|:---:|---|---:|---:|---:|
| `material.rs` | `material/mod.rs` | 2348 | Y | feeds(1) | 33 | 82 | 0 / 0 |


## 5. Break cost

The ruling forbids a re-export shim at an old path. Every site that names a
moved module therefore changes. The counts are occurrences, from
`rg -o 'crate::<stem>\b'` inside `crates/rs_cam_core/src` and
`rg -o 'rs_cam_core::<stem>\b'` over `rs_cam_viz`, `rs_cam_cli`,
`rs_cam_mcp`, `rs_cam_core/tests` and `rs_cam_core/benches`.

| Destination | Files moved | `crate::` edits in core | `rs_cam_core::` edits outside | Total |
|---|---:|---:|---:|---:|
| `finish/` | 20 | 379 | 249 | 628 |
| `trace/` | 5 | 238 | 322 | 560 |
| `stock/` | 10 | 262 | 232 | 494 |
| `machine/` | 4 | 175 | 166 | 341 |
| `surface/` | 6 | 164 | 147 | 311 |
| `ops/` | 16 | 184 | 120 | 304 |
| `geometry/` | 14 | 178 | 113 | 291 |
| `maps/` | 11 | 130 | 101 | 231 |
| `dressups/` | 7 | 72 | 89 | 161 |
| `io/` | 7 | 32 | 80 | 112 |
| `export/` | 4 | 12 | 29 | 41 |
| `material/` | 1 | 0 | 0 | 0 |
| **Total** | **105** | **1 826** | **1 648** | **3 474** |

Files touched: **174** under `crates/rs_cam_core/src/` and **357**
out-of-crate files. `lib.rs` is rewritten once.

**Accuracy of the count.** The regex counts the `X::stem` form only. A
brace-grouped import (`use rs_cam_core::{ arcfit::fit_arcs, … }`) names the
stem on its own line, with no `rs_cam_core::` prefix. There are 16 such
blocks in the workspace. A line-by-line scan of all 16 found exactly **one**
hidden occurrence (a `trace::` line). The true out-of-crate total is
therefore 1 649, not 1 648. The undercount is 0.06 % and does not change any
ranking.

The count excludes each file's own self-references. A per-stem re-run with
the exclusion glob anchored to the full path (`-g
"!crates/rs_cam_core/src/<stem>.rs"`) changed **no** count, so no
same-named file in a subfolder was dropped by accident.

The nine spine files stay at the root. A move of those nine as well would
add **1 156** core edits and **958** out-of-crate edits — a further 2 114
path edits for no navigability gain. See ratification question Q1.

### 5.1 Edits inside `feeds/` and `tool_load/`

The brief forbids an edit inside `crates/rs_cam_core/src/feeds/**` and
`tool_load/**`, because the power-calcs session owns them. Those folders
name moving modules, so the move cannot compile without touching them.
The numbers are measured, and they are already inside the §5 totals:

| Folder | Files | Occurrences | Which stems |
|---|---:|---:|---|
| `feeds/` | 7 | 15 | `machine` 13, `drill` 1, `depth` 1 |
| `tool_load/` | 22 | 128 | `simulation_cut` 51, `machine` 39, `toolpath_spans` 16, `machine_kinematics` 4, `drill_metrics` 4, `debug_trace` 4, `drill_op` 3, `kinematic_utilization` 2, `feed_modulation` 2, `drill` 2, `enriched_mesh` 1 |
| **Total** | **29** | **143** | |

The files in `feeds/` are `suggest.rs`, `explain_payload.rs`, `profile.rs`,
`predict.rs`, `mod.rs`, `efficiency.rs` and `vendor_normalize.rs`. The 22
files in `tool_load/` include `mod.rs`, `power.rs`, `chipload.rs`,
`deflection.rs`, `drill_gates.rs`, `verdict.rs`, `display.rs`,
`locality.rs` and 14 files under `optimize/`.

Every one of the 143 edits is a path rewrite on a `use` line or a call
path. None changes a formula, a threshold or a control flow. The operator
must still rule on it; see ratification question Q5.

## 6. Name collisions and renames

A folder name collides with a file stem in three places. `folder/mod.rs`
must stay a facade (rule 4), so the file takes a new name.

| Old path | New path | Why |
|---|---|---|
| `src/io.rs` | `src/io/model.rs` | `io/io.rs` stutters. The file holds the one model-file reader, so `model` names it. Callers change from `rs_cam_core::io::load_model_file` to `rs_cam_core::io::model::load_model_file` (8 out-of-crate files). |
| `src/machine.rs` | `src/machine/profile.rs` | `machine/machine.rs` stutters. The file defines `MachineProfile`. |
| `src/machine_kinematics.rs` | `src/machine/kinematics.rs` | `machine::machine_kinematics` stutters. The path edit happens anyway, so the rename is free. |

Two further collisions need no rename:

- **`material.rs` + `material/`.** `material.rs` becomes `material/mod.rs`
  and declares `pub mod wood_species_library;`. The module path
  `crate::material` is unchanged, so this costs zero path edits. It is the
  only change this layout makes inside an existing folder.
- **`trace.rs` versus `trace/`.** `trace.rs` is the trace/follow-path
  OPERATION; `trace/` holds the toolpath records. The paths
  `crate::ops::trace` and `crate::trace` are distinct and legal. See
  ratification question Q3.

Three files will be named `profile.rs` after the move: `feeds/profile.rs`,
`ops/profile.rs` and `machine/profile.rs`. Each sits in a different folder,
and each path reads correctly (`feeds::profile`, `ops::profile`,
`machine::profile`).

## 7. Visibility changes

A `mod foo;` in `lib.rs` is private to the crate root, which makes it
visible to every descendant module — that is, to the whole crate. The same
line inside `folder/mod.rs` is visible only inside that folder. Six root
modules are not `pub` today. Each one needs a decision.

| Module | Today | Importers | After the move |
|---|---|---|---|
| `grid` | `mod` | `tier_map`, `reach_map` | `maps/mod.rs`: `mod grid;` — both importers are inside `maps/`. No widening. |
| `memo` | `mod` | `tier_map_cache`, `reach_map_cache`, `finish_surface_cache`, `geom_cache` | `maps/mod.rs`: `mod memo;` — all four are inside `maps/`. No widening. |
| `named_toml_library` | `mod` | `tool_library`, `machine_library` | `io/mod.rs`: `mod named_toml_library;` — both are inside `io/`. No widening. |
| `artifact_io` | `mod` | `semantic_trace` (`trace/`), `simulation_cut` (`stock/`) | `export/mod.rs`: **`pub(crate) mod artifact_io;`** — the importers are outside `export/`. This widening is required or the crate does not compile. |
| `nn_order` | `pub(crate) mod` | `surface_link`, `pencil`, `ramp_finish` (`finish/`), `tsp` (`dressups/`), `adaptive3d/clearing` | `geometry/mod.rs`: `pub(crate) mod nn_order;` — unchanged. |
| `tool_shape_key` | `pub(crate) mod` | `finish_surface_cache`, `reach_map_cache`, `tier_map_cache` | `maps/mod.rs`: `mod tool_shape_key;` — all three are inside `maps/`, so it may **narrow** to private. The narrowing is optional; the move agent may keep `pub(crate)` and narrow it in a separate commit. |

One more attribute moves with its module: `step_input` carries
`#[cfg(feature = "step")]` in `lib.rs` today. The attribute moves onto the
`pub mod step_input;` line in `io/mod.rs`.

## 8. `lib.rs` after the move

`lib.rs` keeps its crate-level documentation, and its module block becomes
32 lines:

```rust
pub mod adaptive;
pub mod adaptive3d;
pub mod compute;
pub mod dexel_stock;
pub mod diagnostics;
pub mod dressups;
pub mod export;
pub mod feeds;
pub mod finish;
pub mod gcode;
pub mod geo;
pub mod geometry;
pub mod ids;
pub mod interrupt;
pub mod io;
pub mod machine;
pub mod maps;
pub mod material;
pub mod measurement;
pub mod mesh;
pub mod metrology;
pub mod ops;
pub mod panic_message;
pub mod polygon;
pub mod session;
pub mod stock;
pub mod surface;
pub mod tool;
pub mod tool_load;
pub mod toolpath;
pub mod trace;
pub mod build_info;

pub use ids::ToolpathId;
```

Sort the final list alphabetically. The single `pub use` survives unchanged,
because `ids.rs` does not move.

## 9. The sentry fix checklist

`planning/tech_debt_2026-09-16/evidence/source_scanning_sentries.txt` lists
79 test files that read source by path. Each one was scanned for a path
needle (`rg -n 'src/[a-z_0-9/]+\.rs'`), for a bare filename literal
(`rg -n '"[a-z_0-9]+\.rs"'`) and for its reading mechanism (`read_to_string`,
`include_str!`, `read_dir`). Two files need an edit.

| Sentry | Site | Needle | Fix |
|---|---|---|---|
| `crates/rs_cam_core/tests/checkpoint_b_resolution_ab.rs` | `only_three_consumers_can_see_the_generation_resolution`, about line 1112 | `assert_eq!(consumers, vec!["ramp_finish.rs", "scallop.rs", "steep_shallow.rs"])`, compared as paths relative to `src` | Change the three literals to `"finish/ramp_finish.rs"`, `"finish/scallop.rs"`, `"finish/steep_shallow.rs"`. **Do not change the `PLUMBING` list** (`"finish_setup.rs"`, `"finish_surface_cache.rs"`): it matches on `file_name()`, which the move does not change. |
| `crates/rs_cam_core/tests/common/adversarial2d.rs` | line 546, doc comment | `` `crates/rs_cam_core/src/pocket.rs` `` | Change to `crates/rs_cam_core/src/ops/pocket.rs`. Prose only; the test does not read this path. |

Every other source-scanning sentry is safe, and the checklist records why:

- **Recursive walkers.** `resolved_gen_inputs_has_one_producer.rs`,
  `gen_inputs_one_assembly_n12.rs`, `stale_set_has_one_answer_wp28.rs`,
  `hatches_are_crate_private_wp7.rs`, `setters_are_crate_private_wp15b.rs`,
  `setters_have_rows_wp15a.rs`, `checkpoint_b_resolution_ab.rs` and
  `src/diagnostics/tests.rs` each push a directory onto a stack and recurse
  (`if path.is_dir() { stack.push(path) }`). They find a moved file at its
  new depth.
- **Roots that do not move.** The scanned roots are `src`, `src/session`,
  `src/diagnostics`, `src/compute/execute.rs`,
  `../rs_cam_viz/src`, `../rs_cam_cli/src` and `../rs_cam_mcp/src`.
- **Pinned files that do not move.** `set_param_refuses_absent_field_n5.rs`
  uses `include_str!("../src/session/compute.rs")`;
  `resolved_gen_inputs_has_one_producer.rs` pins
  `STRUCT_FILE = "src/session/compute.rs"`;
  `loose_executor_is_crate_private_wp12.rs` pins
  `src/compute/execute.rs`.
- **Floors still hold.** `stale_set_has_one_answer_wp28.rs` asserts
  `MIN_FILES_WALKED = 100`. The move changes no file count.
- **viz sentries.** The 44 `rs_cam_viz` sentries name only `rs_cam_viz`
  paths, plus `rs_cam_core/src/session/mod.rs`. None moves.

One further prose fix, outside the sentry list:
`crates/rs_cam_viz/src/state/runtime.rs:127` names
`rs_cam_core::tool_shape_key::ToolShapeKey` in a doc comment. It becomes
`rs_cam_core::maps::tool_shape_key::ToolShapeKey`.

## 10. Documents the move must update

| Document | Rows that name a moving file |
|---|---|
| `planning/AGENT_CODEMAP.md` | lines 27, 39, 40, 41, 50, 51, 52, 53, 82, 83, 84, 85 — `toolpath_spans`, `semantic_trace`, `dressup`, `depth`, `boundary`, `tsp`, `simulation_cut`, `narrate`, `collision`, `stock_mesh`, `dexel_mesh`. Line 27 states that `lib.rs` exports all core modules; rewrite it to name the folder set. |
| `crates/rs_cam_core/CLAUDE.md` | No module table names a root file by path today. Add the folder map from section 3. |
| `crates/rs_cam_core/src/lib.rs` | The crate-level doc comment names `adaptive`, `pocket`, `dropcutter`, `waterline`, `drill`, `gcode`, `viz`. Repoint each to its folder. |
| `architecture/toolpath_spans.md`, `architecture/brep_step_support.md` | Name `src/toolpath_spans.rs`, `src/enriched_mesh.rs`, `src/step_input.rs`, `src/geo.rs`. P3 decides whether `architecture/` survives; if it does, fix these five references. |
| `review/**` | 20 or more prompt files name root paths. P1/P3 decide whether `review/` survives. Do not fix it until that ruling lands. |

## 11. `rs_cam_viz` — measured, no move proposed

| Location | Files at its root | Lines at its root | Subfolders |
|---|---:|---:|---|
| `crates/rs_cam_viz/src/` | 9 | — | 9 (`app`, `bin`, `compute`, `controller`, `interaction`, `io`, `render`, `state`, `ui`) |
| `crates/rs_cam_viz/src/ui/` | 25 | 16 871 | 4 (`components` 18 files / 3 073 lines, `feeds` 6 / 3 405, `overlays` 3 / 2 096, `properties` 13 / 13 547) |

`crates/rs_cam_viz/src/` holds nine files beside nine folders. That root is
already tidy.

`ui/` holds 25 files at its root and 40 files in four subfolders — 38 992
lines in total. The 25 root files are named panels and modals
(`sim_timeline.rs`, `export_wizard.rs`, `menu_bar.rs`), and a reader looking
for a panel finds it by name. Twenty-five is about one fifth of core's 115,
and the four subfolders already absorb the two groups that grew large
(properties and feeds). The numbers do not show a flat-root problem, so this
layout proposes **no viz moves**. The brief asked for a measurement before a
proposal; this is the measurement, and it says no.

## 12. Ratification questions

**Q1 — Does the crate spine stay at the root?**
This layout keeps nine files at the root: `geo`, `polygon`, `mesh`,
`toolpath`, `ids`, `interrupt`, `measurement`, `panic_message`,
`build_info`. The alternative moves them too and leaves `lib.rs` with
folders only. The alternative costs a further **1 156 core + 958
out-of-crate = 2 114 path edits**, and it hides `P3` and `Toolpath` one
level down.
*Recommendation: keep the spine at the root.*

**Q2 — Is `folder/mod.rs` a facade, or may it carry the folder's principal
type?**
This layout applies the brief's preference: `mod.rs` holds `pub mod` lines
only. That forces `io.rs` → `io/model.rs`, `machine.rs` →
`machine/profile.rs`, and the plural folder name `dressups/`.
The alternative makes each of those three files the folder's `mod.rs`
(`io/mod.rs`, `machine/mod.rs`, `dressup/mod.rs`). It needs **no rename**,
it follows the crate's existing `material.rs` precedent, and it **preserves
the caller paths** `rs_cam_core::io::…`, `rs_cam_core::machine::…` and
`rs_cam_core::dressup::…`, which saves **107 core + 87 out-of-crate = 194
path edits**. It costs this: a 461-line `machine/mod.rs` does not show the
folder contents at a glance.
*Recommendation: the facade rule as written. The alternative is real and
cheaper.*

**Q3 — `ops/trace.rs` beside `trace/`?**
`trace.rs` is the trace/follow-path operation. `trace/` holds the toolpath
records (debug trace, semantic trace, spans, narration). The two paths do
not collide for the compiler. They may collide for a reader.
Alternatives: rename the operation to `ops/follow.rs`, or name the record
folder `records/`.
*Recommendation: keep both names; the folder prefix disambiguates.*

**Q4 — Delete `simulation.rs` instead of moving it?**
`src/simulation.rs` is 18 lines and owns no logic. It re-exports three
types. Only one has a reader: `rs_cam_viz::app::gpu_upload` reads
`StockMesh` through it at two production sites. The other two
(`linearize_arc`, `RadialProfileLUT`) have no reader through this path.
Under the no-legacy ruling, a delete plus a repoint of the viewport to
`stock::stock_mesh::StockMesh` is cheaper than a move. A delete is a code
change, not a move, so P2 needs the ruling before it acts.
*Recommendation: delete it.*

**Q5 — May the move edit `feeds/` and `tool_load/`?**
The brief protects those two folders. Section 5.1 measures the collision:
**29 files, 143 path occurrences**, all of them `use` lines or call paths,
none of them a formula. The move cannot compile without them. Three
options:

1. **Permit the edits.** The power-calcs session is down for this
   programme, and `git status` shows no foreign uncommitted file in either
   folder today. The move agent re-checks `git status` before each wave.
2. **Defer the stems those folders name.** `machine`, `simulation_cut`,
   `toolpath_spans`, `machine_kinematics`, `drill_metrics`, `debug_trace`,
   `drill_op`, `kinematic_utilization`, `feed_modulation`, `drill`,
   `depth` and `enriched_mesh` stay at the root. Twelve files stay, the
   root grows to 22, and `trace/`, `stock/` and `machine/` lose their
   principal contents. This option removes the point of P2.
3. **One flagged commit.** Every `feeds/` and `tool_load/` edit lands in a
   single commit named in the P2 summary, so the power-calcs session can
   review it as one diff.
*Recommendation: option 1, with the edits carried in option 3's single
flagged commit.*

---

Everything not listed above is decided, not asked. In particular:

- `adaptive_shared.rs` goes to `ops/`.
- `fingerprint.rs` goes to `export/`.
- `nn_order.rs` goes to `geometry/`.
- `radial_profile.rs` goes to `stock/`.
- `collision.rs` goes to `stock/`.
- `reach.rs` goes to `surface/` and not to `maps/`, because `reach_map.rs`
  does not name `crate::reach` at all.
- `classify_probe`, `crest_lines`, `pencil_dihedral` and `direction_field`
  go to `finish/` and not to `surface/`. Each one imports UP into a
  finishing module: `classify_probe` names `finish_setup` 10 times,
  `crest_lines` and `pencil_dihedral` name `pencil`, and `direction_field`
  names `scallop_isofield`. The placement keeps `surface/` free of an
  upward dependency.
