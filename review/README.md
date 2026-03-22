# rs_cam Codebase Review

## How to use

Each review area has a prompt in `prompts/`. Open a new Claude Code session and run:

```
Review area N: [paste prompt or point at file]
```

The orchestrator in that session will spawn agents to do the review, then write findings to `results/<area_name>.md`.

## Progress

### Core Engine

| # | Area | Prompt | Status | Result |
|---|------|--------|--------|--------|
| 1 | Face operation | [prompt](prompts/01_op_face.md) | Done | [result](results/01_op_face.md) |
| 2 | Pocket operation | [prompt](prompts/02_op_pocket.md) | Done | [result](results/02_op_pocket.md) |
| 3 | Profile operation | [prompt](prompts/03_op_profile.md) | Done | [result](results/03_op_profile.md) |
| 4 | Adaptive clearing (2.5D) | [prompt](prompts/04_op_adaptive.md) | Done | [result](results/04_op_adaptive.md) |
| 5 | VCarve operation | [prompt](prompts/05_op_vcarve.md) | Done | [result](results/05_op_vcarve.md) |
| 6 | Rest machining | [prompt](prompts/06_op_rest.md) | Done | [result](results/06_op_rest.md) |
| 7 | Inlay operation | [prompt](prompts/07_op_inlay.md) | Done | [result](results/07_op_inlay.md) |
| 8 | Zigzag / Trace / Drill / Chamfer | [prompt](prompts/08_op_minor_2d.md) | Done | [result](results/08_op_minor_2d.md) |
| 9 | Dropcutter (3D finish) | [prompt](prompts/09_op_dropcutter.md) | Done | [result](results/09_op_dropcutter.md) |
| 10 | Adaptive 3D (rough) | [prompt](prompts/10_op_adaptive3d.md) | Done | [result](results/10_op_adaptive3d.md) |
| 11 | Waterline | [prompt](prompts/11_op_waterline.md) | Done | [result](results/11_op_waterline.md) |
| 12 | Pencil / Scallop | [prompt](prompts/12_op_pencil_scallop.md) | Done | [result](results/12_op_pencil_scallop.md) |
| 13 | Steep/Shallow + 3D finishes (ramp, spiral, radial, horizontal) | [prompt](prompts/13_op_3d_finishes.md) | Done | [result](results/13_op_3d_finishes.md) |
| 14 | Project Curve | [prompt](prompts/14_op_project_curve.md) | Done | [result](results/14_op_project_curve.md) |
| 15 | Tool geometry (5 families) | [prompt](prompts/15_tool_geometry.md) | Done | [result](results/15_tool_geometry.md) |
| 16 | Tri-dexel simulation | [prompt](prompts/16_simulation.md) | Done | [result](results/16_simulation.md) |
| 17 | Feeds & speeds calculator | [prompt](prompts/17_feeds_speeds.md) | Done | [result](results/17_feeds_speeds.md) |
| 18 | Mesh handling & STL import | [prompt](prompts/18_mesh_handling.md) | Done | [result](results/18_mesh_handling.md) |
| 19 | Vector import (SVG + DXF) | [prompt](prompts/19_vector_import.md) | Done | [result](results/19_vector_import.md) |
| 20 | Toolpath IR | [prompt](prompts/20_toolpath_ir.md) | Done | [result](results/20_toolpath_ir.md) |
| 21 | Dressups (all modifiers) | [prompt](prompts/21_dressups.md) | Done | [result](results/21_dressups.md) |
| 22 | G-code output & arc fitting | [prompt](prompts/22_gcode_output.md) | Done | [result](results/22_gcode_output.md) |
| 23 | Feed optimization | [prompt](prompts/23_feed_optimization.md) | Done | [result](results/23_feed_optimization.md) |
| 24 | Collision detection | [prompt](prompts/24_collision.md) | Done | [result](results/24_collision.md) |
| 25 | Polygon operations | [prompt](prompts/25_polygon_ops.md) | Done | [result](results/25_polygon_ops.md) |
| 26 | Depth / boundary / TSP | [prompt](prompts/26_depth_boundary_tsp.md) | Done | [result](results/26_depth_boundary_tsp.md) |
| 27 | Slope / contour analysis | [prompt](prompts/27_slope_contour.md) | Done | [result](results/27_slope_contour.md) |

### GUI Application

| # | Area | Prompt | Status | Result |
|---|------|--------|--------|--------|
| 28 | UI panels (layout, usability, consistency) | [prompt](prompts/28_ui_panels.md) | Done | [result](results/28_ui_panels.md) |
| 29 | State management & undo/redo | [prompt](prompts/29_state_management.md) | Done | [result](results/29_state_management.md) |
| 30 | Compute orchestration | [prompt](prompts/30_compute.md) | Done | [result](results/30_compute.md) |
| 31 | Render pipeline | [prompt](prompts/31_render.md) | Done | [result](results/31_render.md) |
| 32 | Controller & event dispatch | [prompt](prompts/32_controller_events.md) | Done | [result](results/32_controller_events.md) |
| 33 | I/O & persistence | [prompt](prompts/33_io_persistence.md) | Done | [result](results/33_io_persistence.md) |
| 34 | Interaction (input, picking, camera) | [prompt](prompts/34_interaction.md) | Done | [result](results/34_interaction.md) |

### User Flows

| # | Area | Prompt | Status | Result |
|---|------|--------|--------|--------|
| 35 | Import flow (STL/SVG/DXF) | [prompt](prompts/35_flow_import.md) | Done | [result](results/35_flow_import.md) |
| 36 | Tool + Setup + Workholding flow | [prompt](prompts/36_flow_tool_setup.md) | Done | [result](results/36_flow_tool_setup.md) |
| 37 | Operation creation & generation flow | [prompt](prompts/37_flow_operation.md) | Done | [result](results/37_flow_operation.md) |
| 38 | Simulation & export flow | [prompt](prompts/38_flow_sim_export.md) | Done | [result](results/38_flow_sim_export.md) |

### CLI

| # | Area | Prompt | Status | Result |
|---|------|--------|--------|--------|
| 39 | CLI commands & job runner | [prompt](prompts/39_cli.md) | Done | [result](results/39_cli.md) |

### Cross-Cutting

| # | Area | Prompt | Status | Result |
|---|------|--------|--------|--------|
| 40 | Testing coverage & quality | [prompt](prompts/40_testing.md) | Done | [result](results/40_testing.md) |
| 41 | Duplication & abstraction | [prompt](prompts/41_duplication.md) | Done | [result](results/41_duplication.md) |
| 42 | Unwired / partial features | [prompt](prompts/42_unwired_features.md) | Done | [result](results/42_unwired_features.md) |
| 43 | Performance & parallelism | [prompt](prompts/43_performance.md) | Done | [result](results/43_performance.md) |
| 44 | Dependency health | [prompt](prompts/44_dependencies.md) | Done | [result](results/44_dependencies.md) |
| 45 | Architecture conformance | [prompt](prompts/45_architecture.md) | Done | [result](results/45_architecture.md) |
| 46 | Operation consistency | [prompt](prompts/46_op_consistency.md) | Done | [result](results/46_op_consistency.md) |
| 47 | Documentation drift | [prompt](prompts/47_doc_drift.md) | Done | [result](results/47_doc_drift.md) |
| 48 | Error handling audit | [prompt](prompts/48_error_handling.md) | Done | [result](results/48_error_handling.md) |
