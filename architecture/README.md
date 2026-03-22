# Architecture

This directory holds the durable design documents for the current `rs_cam` workspace.

## Documents

| File | Purpose |
|------|---------|
| [`user_stories.md`](user_stories.md) | User goals and contributor-facing expectations |
| [`requirements.md`](requirements.md) | Functional and non-functional requirements |
| [`high_level_design.md`](high_level_design.md) | Current crate layout, data flow, and extension points |
| [`TRI_DEXEL_SIMULATION.md`](TRI_DEXEL_SIMULATION.md) | Tri-dexel volumetric simulation design, rationale, and implementation plan |

## Current system summary

`rs_cam` is a three-crate Rust workspace:

- `rs_cam_core`: CAM engine and shared data model
- `rs_cam_cli`: batch interface and TOML job runner
- `rs_cam_viz`: desktop CAM application and visualization shell

The canonical product-facing docs live at the repo root:

- `README.md`
- `FEATURE_CATALOG.md`
- `CREDITS.md`

Planning material lives in `planning/`. Research and external reference material live in `research/` and `reference/`.
