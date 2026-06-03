# Architecture

This directory holds the durable design documents for the current `rs_cam` workspace.

## Documents

| File | Purpose |
|------|---------|
| [`user_stories.md`](user_stories.md) | User goals and contributor-facing expectations |
| [`requirements.md`](requirements.md) | Functional and non-functional requirements |
| [`high_level_design.md`](high_level_design.md) | Current crate layout, data flow, and extension points |
| [`TRI_DEXEL_SIMULATION.md`](TRI_DEXEL_SIMULATION.md) | Tri-dexel volumetric simulation design, rationale, and implementation plan |
| [`brep_step_support.md`](brep_step_support.md) | BREP / STEP file architecture and face-aware tessellation |
| [`orientation_refactor.md`](orientation_refactor.md) | Orientation / height refactor — Phase 1-3 shipped, Phase 4 deferred |
| [`toolpath_spans.md`](toolpath_spans.md) | Proposed spans IR replacing `OperationAnnotations` |

## Current system summary

`rs_cam` is a four-crate Rust workspace:

- `rs_cam_core`: CAM engine and shared data model
- `rs_cam_cli`: batch interface and TOML job runner
- `rs_cam_viz`: desktop CAM application and visualization shell
- `rs_cam_mcp`: shared MCP parameter struct library

The canonical product-facing docs live at the repo root:

- `README.md`
- `FEATURE_CATALOG.md`
- `CREDITS.md`

Planning material lives in `planning/`. Research and external reference material live in `research/` and `reference/`.
