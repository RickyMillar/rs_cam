# Semantic duplicate sweep — rs_cam

Threshold: cosine >= 0.88 · chunks: 15123 (rust, >= 240 chars, inline and out-of-line `#[cfg(test)]` modules dropped) · clusters: 45

src pairs: 168 · test pairs: 1449 (omitted, --src-only)

A pair counts as a test pair when either side lives under `tests/`, `benches/` or `src/bin/`, or is an out-of-line `#[cfg(test)]` module file under `src/`.

Near-duplicate candidates across different files. Semantic similarity, not proof: shared boilerplate, trait impls with the same shape, and intentional parallels all appear here.

---

## Cluster 1 — top score 0.9721 (5 files, 7 pairs)

Files:
- `crates/rs_cam_core/src/finish/finish_setup.rs`
- `crates/rs_cam_core/src/maps/finish_surface_cache.rs`
- `crates/rs_cam_core/src/maps/geom_cache.rs`
- `crates/rs_cam_core/src/maps/reach_map_cache.rs`
- `crates/rs_cam_core/src/maps/tier_map_cache.rs`

Strongest pairs:

- **0.9721** `crates/rs_cam_core/src/maps/tier_map_cache.rs` L187-200 ↔ `crates/rs_cam_core/src/maps/reach_map_cache.rs` L184-197
- **0.9538** `crates/rs_cam_core/src/maps/tier_map_cache.rs` L259-267 ↔ `crates/rs_cam_core/src/maps/reach_map_cache.rs` L246-254
- **0.9414** `crates/rs_cam_core/src/maps/finish_surface_cache.rs` L306-318 ↔ `crates/rs_cam_core/src/maps/tier_map_cache.rs` L187-200
- **0.9358** `crates/rs_cam_core/src/maps/finish_surface_cache.rs` L306-318 ↔ `crates/rs_cam_core/src/maps/reach_map_cache.rs` L184-197
- **0.8966** `crates/rs_cam_core/src/maps/geom_cache.rs` L252-262 ↔ `crates/rs_cam_core/src/maps/tier_map_cache.rs` L259-267
- … 2 more pairs

## Cluster 2 — top score 0.9650 (5 files, 5 pairs)

Files:
- `crates/rs_cam_cli/src/project.rs`
- `crates/rs_cam_core/src/session/diagnostics_types.rs`
- `crates/rs_cam_viz/src/ui/sim_diagnostics.rs`
- `crates/rs_cam_viz/src/ui/sim_op_list.rs`
- `crates/rs_cam_viz/src/ui/sim_timeline.rs`

Strongest pairs:

- **0.9650** `crates/rs_cam_viz/src/ui/sim_op_list.rs` L1248-1258 ↔ `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` L999-1009
- **0.9489** `crates/rs_cam_core/src/session/diagnostics_types.rs` L25-99 ↔ `crates/rs_cam_cli/src/project.rs` L51-110
- **0.9273** `crates/rs_cam_viz/src/ui/sim_op_list.rs` L792-812 ↔ `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` L2268-2275
- **0.8944** `crates/rs_cam_viz/src/ui/sim_timeline.rs` L978-1003 ↔ `crates/rs_cam_viz/src/ui/sim_op_list.rs` L792-812
- **0.8800** `crates/rs_cam_viz/src/ui/sim_op_list.rs` L1069-1130 ↔ `crates/rs_cam_cli/src/project.rs` L1292-1357

## Cluster 3 — top score 0.9595 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/feeds/extrapolation/plunge.rs`
- `crates/rs_cam_core/src/feeds/plunge.rs`

Strongest pairs:

- **0.9595** `crates/rs_cam_core/src/feeds/extrapolation/plunge.rs` L345-353 ↔ `crates/rs_cam_core/src/feeds/plunge.rs` L246-254
- **0.8905** `crates/rs_cam_core/src/feeds/extrapolation/plunge.rs` L1-38 ↔ `crates/rs_cam_core/src/feeds/plunge.rs` L1-46

## Cluster 4 — top score 0.9565 (18 files, 25 pairs)

Files:
- `crates/rs_cam_core/src/adaptive3d/mod.rs`
- `crates/rs_cam_core/src/compute/annotate.rs`
- `crates/rs_cam_core/src/compute/execute/curve_engrave.rs`
- `crates/rs_cam_core/src/compute/execute/finish_3d.rs`
- `crates/rs_cam_core/src/compute/operation_configs.rs`
- `crates/rs_cam_core/src/finish/horizontal_finish.rs`
- `crates/rs_cam_core/src/finish/pencil.rs`
- `crates/rs_cam_core/src/finish/radial_finish.rs`
- `crates/rs_cam_core/src/finish/ramp_finish.rs`
- `crates/rs_cam_core/src/finish/scallop.rs`
- `crates/rs_cam_core/src/finish/scallop/research.rs`
- `crates/rs_cam_core/src/finish/scallop_math.rs`
- `crates/rs_cam_core/src/finish/steep_shallow.rs`
- `crates/rs_cam_core/src/maps/tier_islands.rs`
- `crates/rs_cam_core/src/maps/tier_map.rs`
- `crates/rs_cam_core/src/ops/inlay.rs`
- `crates/rs_cam_core/src/ops/project_curve.rs`
- `crates/rs_cam_core/src/surface/dropcutter.rs`

Strongest pairs:

- **0.9565** `crates/rs_cam_core/src/compute/operation_configs.rs` L2558-2573 ↔ `crates/rs_cam_core/src/ops/project_curve.rs` L48-65
- **0.9135** `crates/rs_cam_core/src/compute/operation_configs.rs` L2327-2345 ↔ `crates/rs_cam_core/src/finish/ramp_finish.rs` L227-244
- **0.9135** `crates/rs_cam_core/src/finish/scallop.rs` L1124-1162 ↔ `crates/rs_cam_core/src/finish/scallop/research.rs` L119-151
- **0.9134** `crates/rs_cam_core/src/ops/project_curve.rs` L124-140 ↔ `crates/rs_cam_core/src/compute/operation_configs.rs` L2700-2718
- **0.9130** `crates/rs_cam_core/src/surface/dropcutter.rs` L16-43 ↔ `crates/rs_cam_core/src/maps/tier_map.rs` L609-633
- … 20 more pairs

## Cluster 5 — top score 0.9486 (44 files, 58 pairs)

Files:
- `crates/rs_cam_core/src/compute/catalog.rs`
- `crates/rs_cam_core/src/compute/execute.rs`
- `crates/rs_cam_core/src/compute/execute/dressup_apply.rs`
- `crates/rs_cam_core/src/compute/execute/findings.rs`
- `crates/rs_cam_core/src/compute/obstacle.rs`
- `crates/rs_cam_core/src/compute/sim_prefix.rs`
- `crates/rs_cam_core/src/compute/simulate.rs`
- `crates/rs_cam_core/src/compute/stats.rs`
- `crates/rs_cam_core/src/compute/stock_change.rs`
- `crates/rs_cam_core/src/compute/toolpath_stats.rs`
- `crates/rs_cam_core/src/diagnostics/adapters/from_generation.rs`
- `crates/rs_cam_core/src/diagnostics/adapters/from_tool_load.rs`
- `crates/rs_cam_core/src/dressup/feed_modulation.rs`
- `crates/rs_cam_core/src/dressup/feedopt.rs`
- `crates/rs_cam_core/src/export/viz.rs`
- `crates/rs_cam_core/src/feeds/geometry.rs`
- `crates/rs_cam_core/src/feeds/mod.rs`
- `crates/rs_cam_core/src/feeds/operating_point.rs`
- `crates/rs_cam_core/src/feeds/suggest/apply.rs`
- `crates/rs_cam_core/src/finish/surface_link.rs`
- `crates/rs_cam_core/src/finish/unified_finish.rs`
- `crates/rs_cam_core/src/machine/head.rs`
- `crates/rs_cam_core/src/ops/pocket.rs`
- `crates/rs_cam_core/src/session/command.rs`
- `crates/rs_cam_core/src/session/rest_fresh.rs`
- `crates/rs_cam_core/src/stock/cut_as.rs`
- `crates/rs_cam_core/src/surface/rest_field.rs`
- `crates/rs_cam_core/src/tool_load/chipload.rs`
- `crates/rs_cam_core/src/tool_load/display.rs`
- `crates/rs_cam_core/src/tool_load/power.rs`
- `crates/rs_cam_core/src/tool_load/verdict.rs`
- `crates/rs_cam_core/src/trace/narrate.rs`
- `crates/rs_cam_mcp/src/server.rs`
- `crates/rs_cam_viz/src/app/simulation.rs`
- `crates/rs_cam_viz/src/compute/mod.rs`
- `crates/rs_cam_viz/src/state/simulation.rs`
- `crates/rs_cam_viz/src/state/simulation/issue_triage.rs`
- `crates/rs_cam_viz/src/state/simulation/playback_state.rs`
- `crates/rs_cam_viz/src/state/toolpath/configs.rs`
- `crates/rs_cam_viz/src/ui/feeds/compare.rs`
- `crates/rs_cam_viz/src/ui/properties/mod.rs`
- `crates/rs_cam_viz/src/ui/properties/obstacles.rs`
- `crates/rs_cam_viz/src/ui/properties/stock_changes.rs`
- `crates/rs_cam_viz/src/ui_command.rs`

Strongest pairs:

- **0.9486** `crates/rs_cam_core/src/stock/cut_as.rs` L255-269 ↔ `crates/rs_cam_core/src/tool_load/verdict.rs` L564-576
- **0.9246** `crates/rs_cam_mcp/src/server.rs` L1092-1107 ↔ `crates/rs_cam_core/src/machine/head.rs` L48-63
- **0.9171** `crates/rs_cam_core/src/compute/stock_change.rs` L51-80 ↔ `crates/rs_cam_mcp/src/server.rs` L1393-1410
- **0.9093** `crates/rs_cam_core/src/session/command.rs` L1862-1875 ↔ `crates/rs_cam_mcp/src/server.rs` L1507-1518
- **0.9076** `crates/rs_cam_core/src/ops/pocket.rs` L81-105 ↔ `crates/rs_cam_core/src/compute/execute/findings.rs` L106-122
- … 53 more pairs

## Cluster 6 — top score 0.9457 (5 files, 5 pairs)

Files:
- `crates/rs_cam_core/src/compute/catalog/capabilities.rs`
- `crates/rs_cam_core/src/compute/execute/territory.rs`
- `crates/rs_cam_core/src/feeds/vendor_lut.rs`
- `crates/rs_cam_core/src/feeds/vendor_normalize.rs`
- `crates/rs_cam_viz/src/ui/feeds/shared.rs`

Strongest pairs:

- **0.9457** `crates/rs_cam_core/src/compute/execute/territory.rs` L77-97 ↔ `crates/rs_cam_core/src/compute/catalog/capabilities.rs` L283-306
- **0.9232** `crates/rs_cam_viz/src/ui/feeds/shared.rs` L99-116 ↔ `crates/rs_cam_core/src/feeds/vendor_lut.rs` L158-178
- **0.9098** `crates/rs_cam_viz/src/ui/feeds/shared.rs` L88-98 ↔ `crates/rs_cam_core/src/feeds/vendor_lut.rs` L144-157
- **0.9021** `crates/rs_cam_core/src/feeds/vendor_normalize.rs` L75-105 ↔ `crates/rs_cam_core/src/compute/catalog/capabilities.rs` L320-341
- **0.8905** `crates/rs_cam_core/src/feeds/vendor_normalize.rs` L147-163 ↔ `crates/rs_cam_core/src/feeds/vendor_lut.rs` L266-285

## Cluster 7 — top score 0.9397 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/session/generation_plan.rs`
- `crates/rs_cam_viz/src/controller/generate_all.rs`

Strongest pairs:

- **0.9397** `crates/rs_cam_core/src/session/generation_plan.rs` L118-139 ↔ `crates/rs_cam_viz/src/controller/generate_all.rs` L136-153

## Cluster 8 — top score 0.9391 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/session/cycle_time.rs`
- `crates/rs_cam_viz/src/ui/readiness.rs`

Strongest pairs:

- **0.9391** `crates/rs_cam_viz/src/ui/readiness.rs` L476-492 ↔ `crates/rs_cam_core/src/session/cycle_time.rs` L220-235
- **0.8867** `crates/rs_cam_core/src/session/cycle_time.rs` L358-405 ↔ `crates/rs_cam_viz/src/ui/readiness.rs` L493-543

## Cluster 9 — top score 0.9355 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/render/mesh_render.rs`
- `crates/rs_cam_viz/src/render/sim_render.rs`

Strongest pairs:

- **0.9355** `crates/rs_cam_viz/src/render/sim_render.rs` L15-40 ↔ `crates/rs_cam_viz/src/render/mesh_render.rs` L15-35

## Cluster 10 — top score 0.9258 (3 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/io/mod.rs`
- `crates/rs_cam_core/src/ops/drill_op.rs`
- `crates/rs_cam_core/src/session/mod.rs`

Strongest pairs:

- **0.9258** `crates/rs_cam_core/src/session/mod.rs` L956-980 ↔ `crates/rs_cam_core/src/ops/drill_op.rs` L162-184
- **0.8936** `crates/rs_cam_core/src/io/mod.rs` L126-162 ↔ `crates/rs_cam_core/src/session/mod.rs` L400-499

## Cluster 11 — top score 0.9231 (2 files, 2 pairs)

Files:
- `crates/rs_cam_viz/src/ui/overlays/panel.rs`
- `crates/rs_cam_viz/src/ui/viewport_overlay.rs`

Strongest pairs:

- **0.9231** `crates/rs_cam_viz/src/ui/overlays/panel.rs` L216-232 ↔ `crates/rs_cam_viz/src/ui/viewport_overlay.rs` L599-613
- **0.8981** `crates/rs_cam_viz/src/ui/viewport_overlay.rs` L453-480 ↔ `crates/rs_cam_viz/src/ui/overlays/panel.rs` L331-358

## Cluster 12 — top score 0.9218 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/feeds/efficiency.rs`
- `crates/rs_cam_core/src/feeds/force.rs`

Strongest pairs:

- **0.9218** `crates/rs_cam_core/src/feeds/efficiency.rs` L314-356 ↔ `crates/rs_cam_core/src/feeds/force.rs` L206-231

## Cluster 13 — top score 0.9204 (3 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/machine/kinematic_utilization.rs`
- `crates/rs_cam_core/src/machine/kinematics.rs`
- `crates/rs_cam_core/src/machine/mod.rs`

Strongest pairs:

- **0.9204** `crates/rs_cam_core/src/machine/kinematic_utilization.rs` L464-492 ↔ `crates/rs_cam_core/src/machine/kinematics.rs` L970-997
- **0.8876** `crates/rs_cam_core/src/machine/mod.rs` L244-258 ↔ `crates/rs_cam_core/src/machine/kinematics.rs` L531-550

## Cluster 14 — top score 0.9184 (2 files, 4 pairs)

Files:
- `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs`
- `crates/rs_cam_viz/src/ui/properties/operations/validate.rs`

Strongest pairs:

- **0.9184** `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs` L560-593 ↔ `crates/rs_cam_viz/src/ui/properties/operations/validate.rs` L263-287
- **0.8976** `crates/rs_cam_viz/src/ui/properties/operations/validate.rs` L220-262 ↔ `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs` L481-502
- **0.8890** `crates/rs_cam_viz/src/ui/properties/operations/validate.rs` L263-287 ↔ `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs` L481-502
- **0.8861** `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs` L503-515 ↔ `crates/rs_cam_viz/src/ui/properties/operations/validate.rs` L220-262

## Cluster 15 — top score 0.9158 (4 files, 3 pairs)

Files:
- `crates/rs_cam_core/src/tool/ball.rs`
- `crates/rs_cam_core/src/tool/bullnose.rs`
- `crates/rs_cam_core/src/tool/flat.rs`
- `crates/rs_cam_core/src/tool/tapered_ball.rs`

Strongest pairs:

- **0.9158** `crates/rs_cam_core/src/tool/tapered_ball.rs` L100-199 ↔ `crates/rs_cam_core/src/tool/ball.rs` L30-129
- **0.8951** `crates/rs_cam_core/src/tool/ball.rs` L1-14 ↔ `crates/rs_cam_core/src/tool/flat.rs` L1-14
- **0.8945** `crates/rs_cam_core/src/tool/ball.rs` L30-129 ↔ `crates/rs_cam_core/src/tool/bullnose.rs` L54-193

## Cluster 16 — top score 0.9121 (4 files, 4 pairs)

Files:
- `crates/rs_cam_core/src/dexel_stock/band_batch.rs`
- `crates/rs_cam_core/src/dexel_stock/playback.rs`
- `crates/rs_cam_core/src/dexel_stock/swept.rs`
- `crates/rs_cam_core/src/dexel_stock/whole_path.rs`

Strongest pairs:

- **0.9121** `crates/rs_cam_core/src/dexel_stock/swept.rs` L1041-1140 ↔ `crates/rs_cam_core/src/dexel_stock/band_batch.rs` L318-385
- **0.9050** `crates/rs_cam_core/src/dexel_stock/whole_path.rs` L1-92 ↔ `crates/rs_cam_core/src/dexel_stock/band_batch.rs` L1-57
- **0.8987** `crates/rs_cam_core/src/dexel_stock/whole_path.rs` L168-188 ↔ `crates/rs_cam_core/src/dexel_stock/band_batch.rs` L88-113
- **0.8853** `crates/rs_cam_core/src/dexel_stock/whole_path.rs` L1-92 ↔ `crates/rs_cam_core/src/dexel_stock/playback.rs` L1-76

## Cluster 17 — top score 0.9117 (4 files, 4 pairs)

Files:
- `crates/rs_cam_viz/src/ui/machine_library_modal.rs`
- `crates/rs_cam_viz/src/ui/properties/head.rs`
- `crates/rs_cam_viz/src/ui/properties/machine_panel.rs`
- `crates/rs_cam_viz/src/ui/tool_library_modal.rs`

Strongest pairs:

- **0.9117** `crates/rs_cam_viz/src/ui/machine_library_modal.rs` L44-82 ↔ `crates/rs_cam_viz/src/ui/properties/machine_panel.rs` L28-111
- **0.9097** `crates/rs_cam_viz/src/ui/machine_library_modal.rs` L83-142 ↔ `crates/rs_cam_viz/src/ui/properties/machine_panel.rs` L28-111
- **0.9015** `crates/rs_cam_viz/src/ui/tool_library_modal.rs` L42-61 ↔ `crates/rs_cam_viz/src/ui/machine_library_modal.rs` L25-43
- **0.8821** `crates/rs_cam_viz/src/ui/properties/machine_panel.rs` L252-321 ↔ `crates/rs_cam_viz/src/ui/properties/head.rs` L146-236

## Cluster 18 — top score 0.9105 (4 files, 3 pairs)

Files:
- `crates/rs_cam_core/src/adaptive/search.rs`
- `crates/rs_cam_core/src/compute/alignment_pins/walls.rs`
- `crates/rs_cam_core/src/compute/execute/standing_material.rs`
- `crates/rs_cam_core/src/geo.rs`

Strongest pairs:

- **0.9105** `crates/rs_cam_core/src/geo.rs` L391-413 ↔ `crates/rs_cam_core/src/compute/execute/standing_material.rs` L762-773
- **0.8924** `crates/rs_cam_core/src/compute/alignment_pins/walls.rs` L180-191 ↔ `crates/rs_cam_core/src/compute/execute/standing_material.rs` L762-773
- **0.8845** `crates/rs_cam_core/src/adaptive/search.rs` L842-854 ↔ `crates/rs_cam_core/src/compute/execute/standing_material.rs` L762-773

## Cluster 19 — top score 0.9087 (3 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/session/compute.rs`
- `crates/rs_cam_core/src/session/compute/params.rs`
- `crates/rs_cam_core/src/session/compute/simulation.rs`

Strongest pairs:

- **0.9087** `crates/rs_cam_core/src/session/compute.rs` L2149-2199 ↔ `crates/rs_cam_core/src/session/compute/simulation.rs` L37-136
- **0.8934** `crates/rs_cam_core/src/session/compute.rs` L1478-1577 ↔ `crates/rs_cam_core/src/session/compute/params.rs` L622-696

## Cluster 20 — top score 0.9069 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/feeds/predict.rs`
- `crates/rs_cam_core/src/tool_load/deflection.rs`

Strongest pairs:

- **0.9069** `crates/rs_cam_core/src/tool_load/deflection.rs` L1-65 ↔ `crates/rs_cam_core/src/feeds/predict.rs` L582-619
- **0.8997** `crates/rs_cam_core/src/tool_load/deflection.rs` L72-113 ↔ `crates/rs_cam_core/src/feeds/predict.rs` L582-619

## Cluster 21 — top score 0.9052 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/state/pin_view.rs`
- `crates/rs_cam_viz/src/ui/setup_panel.rs`

Strongest pairs:

- **0.9052** `crates/rs_cam_viz/src/ui/setup_panel.rs` L518-537 ↔ `crates/rs_cam_viz/src/state/pin_view.rs` L29-47

## Cluster 22 — top score 0.9029 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/adaptive3d/clearing.rs`
- `crates/rs_cam_core/src/adaptive3d/path.rs`

Strongest pairs:

- **0.9029** `crates/rs_cam_core/src/adaptive3d/path.rs` L375-500 ↔ `crates/rs_cam_core/src/adaptive3d/clearing.rs` L2882-2981

## Cluster 23 — top score 0.9022 (4 files, 3 pairs)

Files:
- `crates/rs_cam_core/src/compute/config.rs`
- `crates/rs_cam_core/src/geometry/boundary.rs`
- `crates/rs_cam_core/src/geometry/contour_extract.rs`
- `crates/rs_cam_core/src/toolpath.rs`

Strongest pairs:

- **0.9022** `crates/rs_cam_core/src/geometry/boundary.rs` L15-22 ↔ `crates/rs_cam_core/src/compute/config.rs` L402-411
- **0.8949** `crates/rs_cam_core/src/geometry/boundary.rs` L665-674 ↔ `crates/rs_cam_core/src/toolpath.rs` L90-122
- **0.8808** `crates/rs_cam_core/src/geometry/contour_extract.rs` L256-339 ↔ `crates/rs_cam_core/src/geometry/boundary.rs` L833-981

## Cluster 24 — top score 0.9017 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/settings/mod.rs`
- `crates/rs_cam_viz/src/io/app_settings.rs`

Strongest pairs:

- **0.9017** `crates/rs_cam_viz/src/io/app_settings.rs` L1-18 ↔ `crates/rs_cam_core/src/settings/mod.rs` L1-60

## Cluster 25 — top score 0.8987 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/material/force_line.rs`
- `crates/rs_cam_core/src/material/wood_species_library.rs`

Strongest pairs:

- **0.8987** `crates/rs_cam_core/src/material/wood_species_library.rs` L90-111 ↔ `crates/rs_cam_core/src/material/force_line.rs` L295-310

## Cluster 26 — top score 0.8984 (3 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/budget/guard.rs`
- `crates/rs_cam_core/src/compute/collision_check.rs`
- `crates/rs_cam_viz/src/compute/worker.rs`

Strongest pairs:

- **0.8984** `crates/rs_cam_viz/src/compute/worker.rs` L298-322 ↔ `crates/rs_cam_core/src/compute/collision_check.rs` L16-56
- **0.8813** `crates/rs_cam_core/src/budget/guard.rs` L76-89 ↔ `crates/rs_cam_viz/src/compute/worker.rs` L395-429

## Cluster 27 — top score 0.8964 (3 files, 3 pairs)

Files:
- `crates/rs_cam_core/src/compute/alignment_pins/placement.rs`
- `crates/rs_cam_viz/src/controller/events/pins.rs`
- `crates/rs_cam_viz/src/ui/properties/pins/mod.rs`

Strongest pairs:

- **0.8964** `crates/rs_cam_viz/src/ui/properties/pins/mod.rs` L19-35 ↔ `crates/rs_cam_viz/src/controller/events/pins.rs` L135-144
- **0.8885** `crates/rs_cam_core/src/compute/alignment_pins/placement.rs` L1-14 ↔ `crates/rs_cam_viz/src/ui/properties/pins/mod.rs` L241-261
- **0.8836** `crates/rs_cam_viz/src/ui/properties/pins/mod.rs` L19-35 ↔ `crates/rs_cam_viz/src/controller/events/pins.rs` L106-134

## Cluster 28 — top score 0.8961 (2 files, 2 pairs)

Files:
- `crates/rs_cam_core/src/compute/stock_config.rs`
- `crates/rs_cam_core/src/session/project_file.rs`

Strongest pairs:

- **0.8961** `crates/rs_cam_core/src/session/project_file.rs` L496-513 ↔ `crates/rs_cam_core/src/compute/stock_config.rs` L98-115
- **0.8913** `crates/rs_cam_core/src/session/project_file.rs` L463-495 ↔ `crates/rs_cam_core/src/compute/stock_config.rs` L66-97

## Cluster 29 — top score 0.8940 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/ui/toolpath_panel.rs`
- `crates/rs_cam_viz/src/ui/toolpath_row_controls.rs`

Strongest pairs:

- **0.8940** `crates/rs_cam_viz/src/ui/toolpath_panel.rs` L842-863 ↔ `crates/rs_cam_viz/src/ui/toolpath_row_controls.rs` L22-108

## Cluster 30 — top score 0.8933 (4 files, 3 pairs)

Files:
- `crates/rs_cam_core/src/finish/conformal_spiral.rs`
- `crates/rs_cam_core/src/finish/conformal_spiral/spiral_build.rs`
- `crates/rs_cam_core/src/finish/spiral_finish_compact.rs`
- `crates/rs_cam_core/src/tool_load/distribution.rs`

Strongest pairs:

- **0.8933** `crates/rs_cam_core/src/finish/spiral_finish_compact.rs` L106-123 ↔ `crates/rs_cam_core/src/finish/conformal_spiral.rs` L503-526
- **0.8846** `crates/rs_cam_core/src/finish/conformal_spiral.rs` L2545-2567 ↔ `crates/rs_cam_core/src/finish/conformal_spiral/spiral_build.rs` L158-166
- **0.8836** `crates/rs_cam_core/src/tool_load/distribution.rs` L299-305 ↔ `crates/rs_cam_core/src/finish/conformal_spiral.rs` L1885-1893

## Cluster 31 — top score 0.8932 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/stock/simulation_cut.rs`
- `crates/rs_cam_core/src/stock/simulation_cut/accumulate.rs`

Strongest pairs:

- **0.8932** `crates/rs_cam_core/src/stock/simulation_cut.rs` L904-935 ↔ `crates/rs_cam_core/src/stock/simulation_cut/accumulate.rs` L27-101

## Cluster 32 — top score 0.8902 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/io/machine_library.rs`
- `crates/rs_cam_core/src/io/tool_library.rs`

Strongest pairs:

- **0.8902** `crates/rs_cam_core/src/io/tool_library.rs` L73-94 ↔ `crates/rs_cam_core/src/io/machine_library.rs` L63-87

## Cluster 33 — top score 0.8899 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/export/gcode_validator.rs`
- `crates/rs_cam_viz/src/io/export.rs`

Strongest pairs:

- **0.8899** `crates/rs_cam_core/src/export/gcode_validator.rs` L619-659 ↔ `crates/rs_cam_viz/src/io/export.rs` L73-129

## Cluster 34 — top score 0.8874 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/ui/components/suggest.rs`
- `crates/rs_cam_viz/src/ui/properties/pills.rs`

Strongest pairs:

- **0.8874** `crates/rs_cam_viz/src/ui/components/suggest.rs` L24-44 ↔ `crates/rs_cam_viz/src/ui/properties/pills.rs` L46-69

## Cluster 35 — top score 0.8865 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/dressup/mod.rs`
- `crates/rs_cam_core/src/ops/waterline.rs`

Strongest pairs:

- **0.8865** `crates/rs_cam_core/src/dressup/mod.rs` L2269-2285 ↔ `crates/rs_cam_core/src/ops/waterline.rs` L346-357

## Cluster 36 — top score 0.8862 (2 files, 1 pairs)

Files:
- `crates/rs_cam_cli/src/memory_budget.rs`
- `crates/rs_cam_core/src/budget/settings.rs`

Strongest pairs:

- **0.8862** `crates/rs_cam_cli/src/memory_budget.rs` L1-34 ↔ `crates/rs_cam_core/src/budget/settings.rs` L1-36

## Cluster 37 — top score 0.8862 (2 files, 1 pairs)

Files:
- `crates/rs_cam_cli/src/job.rs`
- `crates/rs_cam_core/src/compute/cutter.rs`

Strongest pairs:

- **0.8862** `crates/rs_cam_core/src/compute/cutter.rs` L1-52 ↔ `crates/rs_cam_cli/src/job.rs` L840-887

## Cluster 38 — top score 0.8861 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/budget/estimate.rs`
- `crates/rs_cam_core/src/dexel_stock/simulation.rs`

Strongest pairs:

- **0.8861** `crates/rs_cam_core/src/budget/estimate.rs` L335-364 ↔ `crates/rs_cam_core/src/dexel_stock/simulation.rs` L1739-1796

## Cluster 39 — top score 0.8857 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/app/mcp/commands.rs`
- `crates/rs_cam_viz/src/mcp_bridge.rs`

Strongest pairs:

- **0.8857** `crates/rs_cam_viz/src/app/mcp/commands.rs` L2980-3079 ↔ `crates/rs_cam_viz/src/mcp_bridge.rs` L678-692

## Cluster 40 — top score 0.8856 (2 files, 1 pairs)

Files:
- `crates/rs_cam_viz/src/state/viewport.rs`
- `crates/rs_cam_viz/src/ui/overlays/registry.rs`

Strongest pairs:

- **0.8856** `crates/rs_cam_viz/src/state/viewport.rs` L287-307 ↔ `crates/rs_cam_viz/src/ui/overlays/registry.rs` L1816-1840

## Cluster 41 — top score 0.8854 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/feeds/suggest.rs`
- `crates/rs_cam_core/src/feeds/suggest/adaptive_entry.rs`

Strongest pairs:

- **0.8854** `crates/rs_cam_core/src/feeds/suggest/adaptive_entry.rs` L375-474 ↔ `crates/rs_cam_core/src/feeds/suggest.rs` L568-667

## Cluster 42 — top score 0.8852 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/compute/execute/composed.rs`
- `crates/rs_cam_core/src/finish/composed.rs`

Strongest pairs:

- **0.8852** `crates/rs_cam_core/src/finish/composed.rs` L1-45 ↔ `crates/rs_cam_core/src/compute/execute/composed.rs` L1-37

## Cluster 43 — top score 0.8844 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/selection/item.rs`
- `crates/rs_cam_core/src/selection/view.rs`

Strongest pairs:

- **0.8844** `crates/rs_cam_core/src/selection/view.rs` L336-358 ↔ `crates/rs_cam_core/src/selection/item.rs` L422-521

## Cluster 44 — top score 0.8833 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/adaptive/mod.rs`
- `crates/rs_cam_core/src/adaptive/path.rs`

Strongest pairs:

- **0.8833** `crates/rs_cam_core/src/adaptive/path.rs` L637-667 ↔ `crates/rs_cam_core/src/adaptive/mod.rs` L312-327

## Cluster 45 — top score 0.8824 (2 files, 1 pairs)

Files:
- `crates/rs_cam_core/src/stock/simulation_cut/reporting.rs`
- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs`

Strongest pairs:

- **0.8824** `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` L266-298 ↔ `crates/rs_cam_core/src/stock/simulation_cut/reporting.rs` L201-218

