# Semantic duplicate sweep — rs_cam

Threshold: cosine >= 0.88 · chunks: 10749 (rust, >= 240 chars, inline and out-of-line `#[cfg(test)]` modules dropped) · clusters: 31

src pairs: 118 · test pairs: 1017 (omitted, --src-only)

A pair counts as a test pair when either side lives under `tests/`, `benches/` or `src/bin/`, or is an out-of-line `#[cfg(test)]` module file under `src/`.

Near-duplicate candidates across different files. Semantic similarity, not proof: shared boilerplate, trait impls with the same shape, and intentional parallels all appear here.

---

## Cluster 1 — top score 0.9650 (17 files, 17 pairs)

Files:
- `crates/rs_cam_cli/src/project.rs`
- `crates/rs_cam_core/src/compute/stock_config.rs`
- `crates/rs_cam_core/src/diagnostics/adapters/from_tool_load.rs`
- `crates/rs_cam_core/src/dressup/feed_modulation.rs`
- `crates/rs_cam_core/src/dressup/feedopt.rs`
- `crates/rs_cam_core/src/feeds/geometry.rs`
- `crates/rs_cam_core/src/feeds/mod.rs`
- `crates/rs_cam_core/src/io/mod.rs`
- `crates/rs_cam_core/src/ops/drill_op.rs`
- `crates/rs_cam_core/src/session/mod.rs`
- `crates/rs_cam_core/src/session/project_file.rs`
- `crates/rs_cam_core/src/tool_load/chipload.rs`
- `crates/rs_cam_core/src/tool_load/display.rs`
- `crates/rs_cam_core/src/tool_load/power.rs`
- `crates/rs_cam_core/src/tool_load/verdict.rs`
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs`
- `crates/rs_cam_viz/src/ui/sim_op_list.rs`

Strongest pairs:

- **0.9650** `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` L972-982 ↔ `crates/rs_cam_viz/src/ui/sim_op_list.rs` L1119-1129
- **0.9434** `crates/rs_cam_core/src/session/mod.rs` L1184-1252 ↔ `crates/rs_cam_cli/src/project.rs` L48-102
- **0.9258** `crates/rs_cam_core/src/session/mod.rs` L970-994 ↔ `crates/rs_cam_core/src/ops/drill_op.rs` L160-182
- **0.9075** `crates/rs_cam_core/src/feeds/geometry.rs` L1-16 ↔ `crates/rs_cam_core/src/dressup/feedopt.rs` L70-80
- **0.8957** `crates/rs_cam_core/src/dressup/feed_modulation.rs` L100-122 ↔ `crates/rs_cam_core/src/feeds/mod.rs` L417-443
- … 12 more pairs

## Cluster 2 — top score 0.9535 (3 files, 3 pairs)

Files:
- `crates/rs_cam_core/src/maps/geom_cache.rs`
- `crates/rs_cam_core/src/maps/reach_map_cache.rs`
- `crates/rs_cam_core/src/maps/tier_map_cache.rs`

Strongest pairs:

- **0.9535** `crates/rs_cam_core/src/maps/reach_map_cache.rs` L185-193 ↔ `crates/rs_cam_core/src/maps/tier_map_cache.rs` L239-247
- **0.8969** `crates/rs_cam_core/src/maps/tier_map_cache.rs` L239-247 ↔ `crates/rs_cam_core/src/maps/geom_cache.rs` L233-243
- **0.8836** `crates/rs_cam_core/src/maps/reach_map_cache.rs` L185-193 ↔ `crates/rs_cam_core/src/maps/geom_cache.rs` L233-243

## Cluster 3 — top score 0.9513 (12 files, 21 pairs)

Files:
- `crates/rs_cam_core/src/compute/config.rs`
- `crates/rs_cam_core/src/compute/execute.rs`
- `crates/rs_cam_core/src/compute/stats.rs`
- `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs`
- `crates/rs_cam_core/src/export/viz.rs`
- `crates/rs_cam_core/src/finish/surface_link.rs`
- `crates/rs_cam_core/src/finish/unified_finish.rs`
- `crates/rs_cam_core/src/geometry/boundary.rs`
- `crates/rs_cam_core/src/geometry/contour_extract.rs`
- `crates/rs_cam_core/src/ops/inlay.rs`
- `crates/rs_cam_core/src/toolpath.rs`
- `crates/rs_cam_core/src/trace/narrate.rs`

Strongest pairs:

- **0.9513** `crates/rs_cam_core/src/finish/unified_finish.rs` L1013-1041 ↔ `crates/rs_cam_core/src/compute/config.rs` L1215-1273
- **0.9142** `crates/rs_cam_core/src/finish/unified_finish.rs` L951-975 ↔ `crates/rs_cam_core/src/compute/config.rs` L1161-1197
- **0.9067** `crates/rs_cam_core/src/compute/config.rs` L789-831 ↔ `crates/rs_cam_core/src/compute/stats.rs` L252-335
- **0.9043** `crates/rs_cam_core/src/compute/config.rs` L844-887 ↔ `crates/rs_cam_core/src/compute/execute.rs` L496-532
- **0.9030** `crates/rs_cam_core/src/compute/execute.rs` L452-464 ↔ `crates/rs_cam_core/src/trace/narrate.rs` L1131-1144
- … 16 more pairs

## Cluster 4 — top score 0.9440 (20 files, 29 pairs)

Files:
- `crates/rs_cam_core/src/adaptive3d/mod.rs`
- `crates/rs_cam_core/src/compute/annotate.rs`
- `crates/rs_cam_core/src/compute/catalog.rs`
- `crates/rs_cam_core/src/compute/operation_configs.rs`
- `crates/rs_cam_core/src/feeds/suggest.rs`
- `crates/rs_cam_core/src/finish/horizontal_finish.rs`
- `crates/rs_cam_core/src/finish/pencil.rs`
- `crates/rs_cam_core/src/finish/radial_finish.rs`
- `crates/rs_cam_core/src/finish/ramp_finish.rs`
- `crates/rs_cam_core/src/finish/scallop.rs`
- `crates/rs_cam_core/src/finish/scallop_math.rs`
- `crates/rs_cam_core/src/finish/spiral_finish.rs`
- `crates/rs_cam_core/src/finish/steep_shallow.rs`
- `crates/rs_cam_core/src/maps/tier_islands.rs`
- `crates/rs_cam_core/src/maps/tier_map.rs`
- `crates/rs_cam_core/src/ops/project_curve.rs`
- `crates/rs_cam_core/src/surface/dropcutter.rs`
- `crates/rs_cam_viz/src/state/toolpath/configs.rs`
- `crates/rs_cam_viz/src/ui/components/compare.rs`
- `crates/rs_cam_viz/src/ui/feeds/compare.rs`

Strongest pairs:

- **0.9440** `crates/rs_cam_core/src/compute/operation_configs.rs` L1575-1588 ↔ `crates/rs_cam_core/src/ops/project_curve.rs` L28-39
- **0.9426** `crates/rs_cam_viz/src/ui/components/compare.rs` L85-92 ↔ `crates/rs_cam_viz/src/ui/feeds/compare.rs` L697-709
- **0.9207** `crates/rs_cam_core/src/ops/project_curve.rs` L64-79 ↔ `crates/rs_cam_core/src/compute/operation_configs.rs` L1648-1664
- **0.9172** `crates/rs_cam_core/src/feeds/suggest.rs` L1659-1676 ↔ `crates/rs_cam_core/src/compute/catalog.rs` L2612-2673
- **0.9148** `crates/rs_cam_core/src/compute/operation_configs.rs` L1465-1482 ↔ `crates/rs_cam_core/src/finish/ramp_finish.rs` L217-234
- … 24 more pairs

## Cluster 5 — top score 0.9355 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/render/mesh_render.rs`
- `crates/rs_cam_viz/src/render/sim_render.rs`

Strongest pairs:

- **0.9355** `crates/rs_cam_viz/src/render/sim_render.rs` L15-40 ↔ `crates/rs_cam_viz/src/render/mesh_render.rs` L15-35

## Cluster 6 — top score 0.9280 (8 files, 9 pairs)

Files:
- `crates/rs_cam_core/src/compute/collision_check.rs`
- `crates/rs_cam_core/src/compute/simulate.rs`
- `crates/rs_cam_core/src/session/compute.rs`
- `crates/rs_cam_core/src/stock/collision.rs`
- `crates/rs_cam_viz/src/compute/mod.rs`
- `crates/rs_cam_viz/src/compute/worker.rs`
- `crates/rs_cam_viz/src/state/job.rs`
- `crates/rs_cam_viz/src/state/simulation.rs`

Strongest pairs:

- **0.9280** `crates/rs_cam_viz/src/state/simulation.rs` L553-577 ↔ `crates/rs_cam_core/src/compute/simulate.rs` L279-316
- **0.9265** `crates/rs_cam_viz/src/compute/worker.rs` L157-192 ↔ `crates/rs_cam_core/src/compute/simulate.rs` L208-242
- **0.9007** `crates/rs_cam_core/src/stock/collision.rs` L164-181 ↔ `crates/rs_cam_core/src/compute/collision_check.rs` L55-111
- **0.8984** `crates/rs_cam_core/src/session/compute.rs` L95-126 ↔ `crates/rs_cam_viz/src/state/job.rs` L170-205
- **0.8912** `crates/rs_cam_core/src/session/compute.rs` L2479-2534 ↔ `crates/rs_cam_core/src/compute/simulate.rs` L208-242
- … 4 more pairs

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

- **0.9258** `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` L2430-2454 ↔ `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs` L500-533
- **0.8927** `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` L2387-2429 ↔ `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs` L418-439
- **0.8885** `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` L2430-2454 ↔ `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs` L418-439
- **0.8822** `crates/rs_cam_viz/src/ui/properties/operations/mod.rs` L2387-2429 ↔ `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs` L440-452

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

## Cluster 11 — top score 0.9158 (4 files, 4 pairs)

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

## Cluster 12 — top score 0.9153 (3 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/tool_load/optimize/mod.rs`
- `crates/rs_cam_core/src/tool_load/optimize/retarget/power.rs`
- `crates/rs_cam_core/src/tool_load/optimize/strategy/retarget.rs`

Strongest pairs:

- **0.9153** `crates/rs_cam_core/src/tool_load/optimize/strategy/retarget.rs` L50-64 ↔ `crates/rs_cam_core/src/tool_load/optimize/mod.rs` L762-772
- **0.8820** `crates/rs_cam_core/src/tool_load/optimize/retarget/power.rs` L139-238 ↔ `crates/rs_cam_core/src/tool_load/optimize/strategy/retarget.rs` L133-232

## Cluster 13 — top score 0.9059 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/tool_load/mod.rs`
- `crates/rs_cam_viz/src/app/gpu_upload.rs`

Strongest pairs:

- **0.9059** `crates/rs_cam_core/src/tool_load/mod.rs` L201-276 ↔ `crates/rs_cam_viz/src/app/gpu_upload.rs` L1458-1475

## Cluster 14 — top score 0.9037 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/dexel_stock/playback.rs`
- `crates/rs_cam_core/src/dexel_stock/whole_path.rs`

Strongest pairs:

- **0.9037** `crates/rs_cam_core/src/dexel_stock/playback.rs` L216-239 ↔ `crates/rs_cam_core/src/dexel_stock/whole_path.rs` L258-281
- **0.9007** `crates/rs_cam_core/src/dexel_stock/playback.rs` L330-429 ↔ `crates/rs_cam_core/src/dexel_stock/whole_path.rs` L372-471

## Cluster 15 — top score 0.9027 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/feeds/predict.rs`
- `crates/rs_cam_core/src/tool_load/deflection.rs`

Strongest pairs:

- **0.9027** `crates/rs_cam_core/src/tool_load/deflection.rs` L72-113 ↔ `crates/rs_cam_core/src/feeds/predict.rs` L397-436
- **0.8933** `crates/rs_cam_core/src/tool_load/deflection.rs` L1-65 ↔ `crates/rs_cam_core/src/feeds/predict.rs` L397-436

## Cluster 16 — top score 0.9015 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/ui/machine_library_modal.rs`
- `crates/rs_cam_viz/src/ui/tool_library_modal.rs`

Strongest pairs:

- **0.9015** `crates/rs_cam_viz/src/ui/machine_library_modal.rs` L24-42 ↔ `crates/rs_cam_viz/src/ui/tool_library_modal.rs` L41-60

## Cluster 17 — top score 0.8935 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/ui/toolpath_panel.rs`
- `crates/rs_cam_viz/src/ui/toolpath_row_controls.rs`

Strongest pairs:

- **0.8935** `crates/rs_cam_viz/src/ui/toolpath_panel.rs` L412-433 ↔ `crates/rs_cam_viz/src/ui/toolpath_row_controls.rs` L22-108

## Cluster 18 — top score 0.8933 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/finish/conformal_spiral.rs`
- `crates/rs_cam_core/src/finish/spiral_finish_compact.rs`

Strongest pairs:

- **0.8933** `crates/rs_cam_core/src/finish/spiral_finish_compact.rs` L106-123 ↔ `crates/rs_cam_core/src/finish/conformal_spiral.rs` L498-521

## Cluster 19 — top score 0.8916 (3 files, 3 pairs)

Files:
- `crates/rs_cam_core/src/session/command.rs`
- `crates/rs_cam_mcp/src/server.rs`
- `crates/rs_cam_viz/src/ui_command.rs`

Strongest pairs:

- **0.8916** `crates/rs_cam_mcp/src/server.rs` L224-262 ↔ `crates/rs_cam_viz/src/ui_command.rs` L140-158
- **0.8909** `crates/rs_cam_mcp/src/server.rs` L34-45 ↔ `crates/rs_cam_core/src/session/command.rs` L1217-1225
- **0.8846** `crates/rs_cam_mcp/src/server.rs` L263-275 ↔ `crates/rs_cam_viz/src/ui_command.rs` L159-169

## Cluster 20 — top score 0.8909 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/geo.rs`
- `crates/rs_cam_core/src/metrology/spacing.rs`

Strongest pairs:

- **0.8909** `crates/rs_cam_core/src/metrology/spacing.rs` L67-79 ↔ `crates/rs_cam_core/src/geo.rs` L372-394

## Cluster 21 — top score 0.8905 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/feeds/vendor_lut.rs`
- `crates/rs_cam_core/src/feeds/vendor_normalize.rs`

Strongest pairs:

- **0.8905** `crates/rs_cam_core/src/feeds/vendor_lut.rs` L218-237 ↔ `crates/rs_cam_core/src/feeds/vendor_normalize.rs` L125-141

## Cluster 22 — top score 0.8905 (2 files, 2 pairs)

Files:
- `crates/rs_cam_viz/src/ui/optimize_modal.rs`
- `crates/rs_cam_viz/src/ui/optimize_project.rs`

Strongest pairs:

- **0.8905** `crates/rs_cam_viz/src/ui/optimize_project.rs` L26-44 ↔ `crates/rs_cam_viz/src/ui/optimize_modal.rs` L35-69
- **0.8878** `crates/rs_cam_viz/src/ui/optimize_modal.rs` L875-924 ↔ `crates/rs_cam_viz/src/ui/optimize_project.rs` L353-389

## Cluster 23 — top score 0.8886 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/ui/components/suggest.rs`
- `crates/rs_cam_viz/src/ui/properties/pills.rs`

Strongest pairs:

- **0.8886** `crates/rs_cam_viz/src/ui/components/suggest.rs` L24-44 ↔ `crates/rs_cam_viz/src/ui/properties/pills.rs` L41-63

## Cluster 24 — top score 0.8858 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/app/mcp.rs`
- `crates/rs_cam_viz/src/mcp_server.rs`

Strongest pairs:

- **0.8858** `crates/rs_cam_viz/src/app/mcp.rs` L6-45 ↔ `crates/rs_cam_viz/src/mcp_server.rs` L1484-1583

## Cluster 25 — top score 0.8854 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/material/mod.rs`
- `crates/rs_cam_core/src/material/wood_species_library.rs`

Strongest pairs:

- **0.8854** `crates/rs_cam_core/src/material/mod.rs` L8-25 ↔ `crates/rs_cam_core/src/material/wood_species_library.rs` L1-19

## Cluster 26 — top score 0.8831 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/adaptive/mod.rs`
- `crates/rs_cam_core/src/adaptive/path.rs`

Strongest pairs:

- **0.8831** `crates/rs_cam_core/src/adaptive/path.rs` L83-111 ↔ `crates/rs_cam_core/src/adaptive/mod.rs` L218-233

## Cluster 27 — top score 0.8827 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/tool_load/optimize/delta.rs`
- `crates/rs_cam_core/src/tool_load/optimize/rank.rs`

Strongest pairs:

- **0.8827** `crates/rs_cam_core/src/tool_load/optimize/rank.rs` L117-216 ↔ `crates/rs_cam_core/src/tool_load/optimize/delta.rs` L350-449

## Cluster 28 — top score 0.8825 (2 files, 1 pairs)

Files:
- `crates/rs_cam_cli/src/job.rs`
- `crates/rs_cam_core/src/compute/cutter.rs`

Strongest pairs:

- **0.8825** `crates/rs_cam_cli/src/job.rs` L337-384 ↔ `crates/rs_cam_core/src/compute/cutter.rs` L1-54

## Cluster 29 — top score 0.8820 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/finish/finish_setup.rs`
- `crates/rs_cam_core/src/maps/finish_surface_cache.rs`

Strongest pairs:

- **0.8820** `crates/rs_cam_core/src/maps/finish_surface_cache.rs` L219-227 ↔ `crates/rs_cam_core/src/finish/finish_setup.rs` L128-143

## Cluster 30 — top score 0.8817 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/stock/simulation_cut.rs`
- `crates/rs_cam_viz/src/compute/worker/helpers.rs`

Strongest pairs:

- **0.8817** `crates/rs_cam_core/src/stock/simulation_cut.rs` L1001-1012 ↔ `crates/rs_cam_viz/src/compute/worker/helpers.rs` L84-146

## Cluster 31 — top score 0.8801 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/trace/semantic_trace.rs`
- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs`

Strongest pairs:

- **0.8801** `crates/rs_cam_core/src/trace/semantic_trace.rs` L1180-1190 ↔ `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` L368-393

