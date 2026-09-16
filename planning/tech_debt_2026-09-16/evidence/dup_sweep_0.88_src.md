# Semantic duplicate sweep — rs_cam

Threshold: cosine >= 0.88 · chunks: 10808 (rust, >= 240 chars, inline and out-of-line `#[cfg(test)]` modules dropped) · clusters: 31

src pairs: 137 · test pairs: 1017 (omitted, --src-only)

A pair counts as a test pair when either side lives under `tests/`, `benches/` or `src/bin/`, or is an out-of-line `#[cfg(test)]` module file under `src/`.

Near-duplicate candidates across different files. Semantic similarity, not proof: shared boilerplate, trait impls with the same shape, and intentional parallels all appear here.

---

## Cluster 1 — top score 0.9645 (20 files, 22 pairs)

Files:
- `crates/rs_cam_cli/src/project.rs`
- `crates/rs_cam_core/src/compute/stock_config.rs`
- `crates/rs_cam_core/src/diagnostics/adapters/from_tool_load.rs`
- `crates/rs_cam_core/src/drill_op.rs`
- `crates/rs_cam_core/src/feed_modulation.rs`
- `crates/rs_cam_core/src/feedopt.rs`
- `crates/rs_cam_core/src/feeds/geometry.rs`
- `crates/rs_cam_core/src/feeds/mod.rs`
- `crates/rs_cam_core/src/io.rs`
- `crates/rs_cam_core/src/session/command.rs`
- `crates/rs_cam_core/src/session/mod.rs`
- `crates/rs_cam_core/src/session/project_file.rs`
- `crates/rs_cam_core/src/tool_load/chipload.rs`
- `crates/rs_cam_core/src/tool_load/display.rs`
- `crates/rs_cam_core/src/tool_load/power.rs`
- `crates/rs_cam_core/src/tool_load/verdict.rs`
- `crates/rs_cam_mcp/src/server.rs`
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs`
- `crates/rs_cam_viz/src/ui/sim_op_list.rs`
- `crates/rs_cam_viz/src/ui_command.rs`

Strongest pairs:

- **0.9645** `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` L972-982 ↔ `crates/rs_cam_viz/src/ui/sim_op_list.rs` L1119-1129
- **0.9471** `crates/rs_cam_core/src/session/mod.rs` L1177-1246 ↔ `crates/rs_cam_cli/src/project.rs` L48-107
- **0.9287** `crates/rs_cam_core/src/session/mod.rs` L970-994 ↔ `crates/rs_cam_core/src/drill_op.rs` L160-182
- **0.9104** `crates/rs_cam_core/src/feedopt.rs` L70-80 ↔ `crates/rs_cam_core/src/feeds/geometry.rs` L1-16
- **0.8993** `crates/rs_cam_core/src/feed_modulation.rs` L100-122 ↔ `crates/rs_cam_core/src/feeds/mod.rs` L417-443
- … 17 more pairs

## Cluster 2 — top score 0.9547 (5 files, 8 pairs)

Files:
- `crates/rs_cam_core/src/finish_setup.rs`
- `crates/rs_cam_core/src/finish_surface_cache.rs`
- `crates/rs_cam_core/src/geom_cache.rs`
- `crates/rs_cam_core/src/reach_map_cache.rs`
- `crates/rs_cam_core/src/tier_map_cache.rs`

Strongest pairs:

- **0.9547** `crates/rs_cam_core/src/tier_map_cache.rs` L146-170 ↔ `crates/rs_cam_core/src/reach_map_cache.rs` L102-126
- **0.9517** `crates/rs_cam_core/src/tier_map_cache.rs` L238-246 ↔ `crates/rs_cam_core/src/reach_map_cache.rs` L184-192
- **0.9190** `crates/rs_cam_core/src/tier_map_cache.rs` L146-170 ↔ `crates/rs_cam_core/src/geom_cache.rs` L193-213
- **0.9127** `crates/rs_cam_core/src/reach_map_cache.rs` L102-126 ↔ `crates/rs_cam_core/src/geom_cache.rs` L193-213
- **0.8961** `crates/rs_cam_core/src/tier_map_cache.rs` L146-170 ↔ `crates/rs_cam_core/src/finish_surface_cache.rs` L265-289
- … 3 more pairs

## Cluster 3 — top score 0.9517 (10 files, 22 pairs)

Files:
- `crates/rs_cam_core/src/boundary.rs`
- `crates/rs_cam_core/src/compute/config.rs`
- `crates/rs_cam_core/src/compute/execute.rs`
- `crates/rs_cam_core/src/compute/stats.rs`
- `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs`
- `crates/rs_cam_core/src/narrate.rs`
- `crates/rs_cam_core/src/surface_link.rs`
- `crates/rs_cam_core/src/toolpath.rs`
- `crates/rs_cam_core/src/unified_finish.rs`
- `crates/rs_cam_core/src/viz.rs`

Strongest pairs:

- **0.9517** `crates/rs_cam_core/src/compute/config.rs` L1124-1182 ↔ `crates/rs_cam_core/src/unified_finish.rs` L1007-1035
- **0.9133** `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs` L157-202 ↔ `crates/rs_cam_core/src/narrate.rs` L1147-1173
- **0.9097** `crates/rs_cam_core/src/viz.rs` L23-34 ↔ `crates/rs_cam_core/src/narrate.rs` L834-843
- **0.9096** `crates/rs_cam_core/src/unified_finish.rs` L945-969 ↔ `crates/rs_cam_core/src/compute/config.rs` L1070-1106
- **0.9091** `crates/rs_cam_core/src/narrate.rs` L1174-1203 ↔ `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs` L109-156
- … 17 more pairs

## Cluster 4 — top score 0.9434 (20 files, 30 pairs)

Files:
- `crates/rs_cam_core/src/adaptive/mod.rs`
- `crates/rs_cam_core/src/adaptive3d/mod.rs`
- `crates/rs_cam_core/src/compute/annotate.rs`
- `crates/rs_cam_core/src/compute/catalog.rs`
- `crates/rs_cam_core/src/compute/operation_configs.rs`
- `crates/rs_cam_core/src/dropcutter.rs`
- `crates/rs_cam_core/src/feeds/suggest.rs`
- `crates/rs_cam_core/src/horizontal_finish.rs`
- `crates/rs_cam_core/src/pencil.rs`
- `crates/rs_cam_core/src/project_curve.rs`
- `crates/rs_cam_core/src/radial_finish.rs`
- `crates/rs_cam_core/src/ramp_finish.rs`
- `crates/rs_cam_core/src/scallop.rs`
- `crates/rs_cam_core/src/spiral_finish.rs`
- `crates/rs_cam_core/src/steep_shallow.rs`
- `crates/rs_cam_core/src/tier_islands.rs`
- `crates/rs_cam_core/src/tier_map.rs`
- `crates/rs_cam_viz/src/state/toolpath/configs.rs`
- `crates/rs_cam_viz/src/ui/components/compare.rs`
- `crates/rs_cam_viz/src/ui/feeds/compare.rs`

Strongest pairs:

- **0.9434** `crates/rs_cam_core/src/project_curve.rs` L28-39 ↔ `crates/rs_cam_core/src/compute/operation_configs.rs` L1583-1596
- **0.9426** `crates/rs_cam_viz/src/ui/components/compare.rs` L85-92 ↔ `crates/rs_cam_viz/src/ui/feeds/compare.rs` L697-709
- **0.9194** `crates/rs_cam_core/src/spiral_finish.rs` L310-324 ↔ `crates/rs_cam_core/src/ramp_finish.rs` L880-894
- **0.9172** `crates/rs_cam_core/src/compute/catalog.rs` L2591-2652 ↔ `crates/rs_cam_core/src/feeds/suggest.rs` L1620-1637
- **0.9146** `crates/rs_cam_core/src/compute/catalog.rs` L2563-2579 ↔ `crates/rs_cam_core/src/feeds/suggest.rs` L1620-1637
- … 25 more pairs

## Cluster 5 — top score 0.9355 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/render/mesh_render.rs`
- `crates/rs_cam_viz/src/render/sim_render.rs`

Strongest pairs:

- **0.9355** `crates/rs_cam_viz/src/render/sim_render.rs` L15-40 ↔ `crates/rs_cam_viz/src/render/mesh_render.rs` L15-35

## Cluster 6 — top score 0.9280 (10 files, 12 pairs)

Files:
- `crates/rs_cam_core/src/collision.rs`
- `crates/rs_cam_core/src/compute/collision_check.rs`
- `crates/rs_cam_core/src/compute/simulate.rs`
- `crates/rs_cam_core/src/panic_message.rs`
- `crates/rs_cam_core/src/session/compute.rs`
- `crates/rs_cam_core/src/simulation_cut.rs`
- `crates/rs_cam_viz/src/compute/mod.rs`
- `crates/rs_cam_viz/src/compute/worker.rs`
- `crates/rs_cam_viz/src/state/job.rs`
- `crates/rs_cam_viz/src/state/simulation.rs`

Strongest pairs:

- **0.9280** `crates/rs_cam_viz/src/state/simulation.rs` L547-571 ↔ `crates/rs_cam_core/src/compute/simulate.rs` L277-314
- **0.9160** `crates/rs_cam_core/src/compute/simulate.rs` L206-240 ↔ `crates/rs_cam_viz/src/compute/worker.rs` L159-202
- **0.9112** `crates/rs_cam_viz/src/compute/worker.rs` L1519-1529 ↔ `crates/rs_cam_core/src/panic_message.rs` L1-32
- **0.9101** `crates/rs_cam_viz/src/compute/worker.rs` L245-290 ↔ `crates/rs_cam_core/src/compute/simulate.rs` L359-427
- **0.9010** `crates/rs_cam_core/src/collision.rs` L160-177 ↔ `crates/rs_cam_core/src/compute/collision_check.rs` L55-111
- … 7 more pairs

## Cluster 7 — top score 0.9271 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/tool_load/optimize/narrative.rs`
- `crates/rs_cam_core/src/tool_load/optimize/refusal.rs`

Strongest pairs:

- **0.9271** `crates/rs_cam_core/src/tool_load/optimize/narrative.rs` L105-116 ↔ `crates/rs_cam_core/src/tool_load/optimize/refusal.rs` L20-30

## Cluster 8 — top score 0.9258 (2 files, 4 pairs)

Files:
- `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs`
- `crates/rs_cam_viz/src/ui/properties/operations/mod.rs`

Strongest pairs:

- **0.9258** `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` L2435-2459 ↔ `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs` L500-533
- **0.8927** `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` L2392-2434 ↔ `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs` L418-439
- **0.8885** `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` L2435-2459 ↔ `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs` L418-439
- **0.8822** `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` L2392-2434 ↔ `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs` L440-452

## Cluster 9 — top score 0.9218 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/feeds/efficiency.rs`
- `crates/rs_cam_core/src/feeds/force.rs`

Strongest pairs:

- **0.9218** `crates/rs_cam_core/src/feeds/efficiency.rs` L309-351 ↔ `crates/rs_cam_core/src/feeds/force.rs` L271-296

## Cluster 10 — top score 0.9217 (3 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/session/cycle_time.rs`
- `crates/rs_cam_viz/src/ui/components/format.rs`
- `crates/rs_cam_viz/src/ui/readiness.rs`

Strongest pairs:

- **0.9217** `crates/rs_cam_viz/src/ui/readiness.rs` L503-524 ↔ `crates/rs_cam_viz/src/ui/components/format.rs` L24-39
- **0.8983** `crates/rs_cam_core/src/session/cycle_time.rs` L147-194 ↔ `crates/rs_cam_viz/src/ui/readiness.rs` L410-454

## Cluster 11 — top score 0.9165 (3 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/tool_load/optimize/mod.rs`
- `crates/rs_cam_core/src/tool_load/optimize/retarget/power.rs`
- `crates/rs_cam_core/src/tool_load/optimize/strategy/retarget.rs`

Strongest pairs:

- **0.9165** `crates/rs_cam_core/src/tool_load/optimize/strategy/retarget.rs` L50-64 ↔ `crates/rs_cam_core/src/tool_load/optimize/mod.rs` L758-768
- **0.8815** `crates/rs_cam_core/src/tool_load/optimize/retarget/power.rs` L139-238 ↔ `crates/rs_cam_core/src/tool_load/optimize/strategy/retarget.rs` L133-232

## Cluster 12 — top score 0.9158 (4 files, 4 pairs)

Files:
- `crates/rs_cam_core/src/tool/ball.rs`
- `crates/rs_cam_core/src/tool/bullnose.rs`
- `crates/rs_cam_core/src/tool/flat.rs`
- `crates/rs_cam_core/src/tool/tapered_ball.rs`

Strongest pairs:

- **0.9158** `crates/rs_cam_core/src/tool/ball.rs` L30-129 ↔ `crates/rs_cam_core/src/tool/tapered_ball.rs` L101-200
- **0.8951** `crates/rs_cam_core/src/tool/ball.rs` L1-14 ↔ `crates/rs_cam_core/src/tool/flat.rs` L1-14
- **0.8951** `crates/rs_cam_core/src/tool/bullnose.rs` L144-243 ↔ `crates/rs_cam_core/src/tool/ball.rs` L120-192
- **0.8945** `crates/rs_cam_core/src/tool/ball.rs` L30-129 ↔ `crates/rs_cam_core/src/tool/bullnose.rs` L54-153

## Cluster 13 — top score 0.9094 (2 files, 1 pairs)

Files:
- `crates/rs_cam_cli/src/run.rs`
- `crates/rs_cam_cli/src/sweep.rs`

Strongest pairs:

- **0.9094** `crates/rs_cam_cli/src/sweep.rs` L267-276 ↔ `crates/rs_cam_cli/src/run.rs` L315-333

## Cluster 14 — top score 0.9075 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/tool_load/mod.rs`
- `crates/rs_cam_viz/src/app/gpu_upload.rs`

Strongest pairs:

- **0.9075** `crates/rs_cam_core/src/tool_load/mod.rs` L201-276 ↔ `crates/rs_cam_viz/src/app/gpu_upload.rs` L1449-1466

## Cluster 15 — top score 0.9041 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/dexel_stock/playback.rs`
- `crates/rs_cam_core/src/dexel_stock/whole_path.rs`

Strongest pairs:

- **0.9041** `crates/rs_cam_core/src/dexel_stock/playback.rs` L216-239 ↔ `crates/rs_cam_core/src/dexel_stock/whole_path.rs` L258-281
- **0.9002** `crates/rs_cam_core/src/dexel_stock/playback.rs` L330-429 ↔ `crates/rs_cam_core/src/dexel_stock/whole_path.rs` L372-471

## Cluster 16 — top score 0.9041 (5 files, 4 pairs)

Files:
- `crates/rs_cam_core/src/adaptive3d/clearing.rs`
- `crates/rs_cam_core/src/adaptive3d/path.rs`
- `crates/rs_cam_core/src/dressup.rs`
- `crates/rs_cam_core/src/geo.rs`
- `crates/rs_cam_core/src/metrology/spacing.rs`

Strongest pairs:

- **0.9041** `crates/rs_cam_core/src/geo.rs` L252-265 ↔ `crates/rs_cam_core/src/dressup.rs` L858-871
- **0.8909** `crates/rs_cam_core/src/metrology/spacing.rs` L67-79 ↔ `crates/rs_cam_core/src/geo.rs` L340-362
- **0.8833** `crates/rs_cam_core/src/dressup.rs` L858-871 ↔ `crates/rs_cam_core/src/adaptive3d/clearing.rs` L2564-2585
- **0.8800** `crates/rs_cam_core/src/adaptive3d/clearing.rs` L632-731 ↔ `crates/rs_cam_core/src/adaptive3d/path.rs` L865-964

## Cluster 17 — top score 0.9036 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/feeds/predict.rs`
- `crates/rs_cam_core/src/tool_load/deflection.rs`

Strongest pairs:

- **0.9036** `crates/rs_cam_core/src/tool_load/deflection.rs` L72-113 ↔ `crates/rs_cam_core/src/feeds/predict.rs` L397-436
- **0.8944** `crates/rs_cam_core/src/tool_load/deflection.rs` L1-65 ↔ `crates/rs_cam_core/src/feeds/predict.rs` L397-436

## Cluster 18 — top score 0.9033 (3 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/metrology/monge.rs`
- `crates/rs_cam_core/src/reach_map.rs`
- `crates/rs_cam_core/src/session/reach.rs`

Strongest pairs:

- **0.9033** `crates/rs_cam_core/src/metrology/monge.rs` L137-170 ↔ `crates/rs_cam_core/src/reach_map.rs` L881-906
- **0.8839** `crates/rs_cam_core/src/session/reach.rs` L48-147 ↔ `crates/rs_cam_core/src/reach_map.rs` L159-173

## Cluster 19 — top score 0.9015 (3 files, 2 pairs)

Files:
- `crates/rs_cam_viz/src/ui/machine_library_modal.rs`
- `crates/rs_cam_viz/src/ui/properties/mod.rs`
- `crates/rs_cam_viz/src/ui/tool_library_modal.rs`

Strongest pairs:

- **0.9015** `crates/rs_cam_viz/src/ui/machine_library_modal.rs` L24-42 ↔ `crates/rs_cam_viz/src/ui/tool_library_modal.rs` L41-60
- **0.8803** `crates/rs_cam_viz/src/ui/machine_library_modal.rs` L43-81 ↔ `crates/rs_cam_viz/src/ui/properties/mod.rs` L1650-1729

## Cluster 20 — top score 0.8961 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/machine_library.rs`
- `crates/rs_cam_core/src/tool_library.rs`

Strongest pairs:

- **0.8961** `crates/rs_cam_core/src/tool_library.rs` L320-339 ↔ `crates/rs_cam_core/src/machine_library.rs` L178-197
- **0.8837** `crates/rs_cam_core/src/machine_library.rs` L92-116 ↔ `crates/rs_cam_core/src/tool_library.rs` L94-117

## Cluster 21 — top score 0.8959 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/compute/semantic_helpers.rs`
- `crates/rs_cam_core/src/compute/spans.rs`

Strongest pairs:

- **0.8959** `crates/rs_cam_core/src/compute/spans.rs` L381-413 ↔ `crates/rs_cam_core/src/compute/semantic_helpers.rs` L14-50

## Cluster 22 — top score 0.8956 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/conformal_spiral.rs`
- `crates/rs_cam_core/src/spiral_finish_compact.rs`

Strongest pairs:

- **0.8956** `crates/rs_cam_core/src/spiral_finish_compact.rs` L106-123 ↔ `crates/rs_cam_core/src/conformal_spiral.rs` L498-521

## Cluster 23 — top score 0.8942 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/app.rs`
- `crates/rs_cam_viz/src/render/toolpath_render.rs`

Strongest pairs:

- **0.8942** `crates/rs_cam_viz/src/render/toolpath_render.rs` L29-52 ↔ `crates/rs_cam_viz/src/app.rs` L1176-1215

## Cluster 24 — top score 0.8935 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/ui/toolpath_panel.rs`
- `crates/rs_cam_viz/src/ui/toolpath_row_controls.rs`

Strongest pairs:

- **0.8935** `crates/rs_cam_viz/src/ui/toolpath_panel.rs` L412-433 ↔ `crates/rs_cam_viz/src/ui/toolpath_row_controls.rs` L22-108

## Cluster 25 — top score 0.8919 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/material.rs`
- `crates/rs_cam_core/src/material/wood_species_library.rs`

Strongest pairs:

- **0.8919** `crates/rs_cam_core/src/material.rs` L8-25 ↔ `crates/rs_cam_core/src/material/wood_species_library.rs` L1-19

## Cluster 26 — top score 0.8905 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/feeds/vendor_lut.rs`
- `crates/rs_cam_core/src/feeds/vendor_normalize.rs`

Strongest pairs:

- **0.8905** `crates/rs_cam_core/src/feeds/vendor_lut.rs` L228-247 ↔ `crates/rs_cam_core/src/feeds/vendor_normalize.rs` L125-141

## Cluster 27 — top score 0.8905 (2 files, 2 pairs)

Files:
- `crates/rs_cam_viz/src/ui/optimize_modal.rs`
- `crates/rs_cam_viz/src/ui/optimize_project.rs`

Strongest pairs:

- **0.8905** `crates/rs_cam_viz/src/ui/optimize_project.rs` L26-44 ↔ `crates/rs_cam_viz/src/ui/optimize_modal.rs` L35-69
- **0.8878** `crates/rs_cam_viz/src/ui/optimize_modal.rs` L875-924 ↔ `crates/rs_cam_viz/src/ui/optimize_project.rs` L353-389

## Cluster 28 — top score 0.8886 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/ui/components/suggest.rs`
- `crates/rs_cam_viz/src/ui/properties/pills.rs`

Strongest pairs:

- **0.8886** `crates/rs_cam_viz/src/ui/components/suggest.rs` L24-44 ↔ `crates/rs_cam_viz/src/ui/properties/pills.rs` L41-63

## Cluster 29 — top score 0.8858 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/app/mcp.rs`
- `crates/rs_cam_viz/src/mcp_server.rs`

Strongest pairs:

- **0.8858** `crates/rs_cam_viz/src/app/mcp.rs` L6-45 ↔ `crates/rs_cam_viz/src/mcp_server.rs` L1484-1583

## Cluster 30 — top score 0.8827 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/tool_load/optimize/delta.rs`
- `crates/rs_cam_core/src/tool_load/optimize/rank.rs`

Strongest pairs:

- **0.8827** `crates/rs_cam_core/src/tool_load/optimize/rank.rs` L117-216 ↔ `crates/rs_cam_core/src/tool_load/optimize/delta.rs` L350-449

## Cluster 31 — top score 0.8825 (2 files, 1 pairs)

Files:
- `crates/rs_cam_cli/src/job.rs`
- `crates/rs_cam_core/src/compute/cutter.rs`

Strongest pairs:

- **0.8825** `crates/rs_cam_cli/src/job.rs` L339-386 ↔ `crates/rs_cam_core/src/compute/cutter.rs` L1-54

