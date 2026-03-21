# rs_cam Agent Notes

## What this repo is

`rs_cam` is a Rust CAM workspace for 3-axis wood routers.

It has three product layers:

- `crates/rs_cam_core`: CAM engine and shared data model
- `crates/rs_cam_cli`: batch CLI
- `crates/rs_cam_viz`: desktop CAM app (`rs_cam_gui`)

## Architecture guardrails

- keep the core library independent from GUI concerns
- treat the toolpath IR as the boundary between planning and post-processing/output
- keep import, tool modeling, operation generation, dressups, simulation, and export as distinct layers
- prefer extending the existing core + worker + UI wiring path instead of creating parallel one-off flows

## Current doc map

- product overview: `README.md`
- capability surface: `FEATURE_CATALOG.md`
- attribution and source lineage: `CREDITS.md`
- design docs: `architecture/`
- research notes: `research/`
- status and backlog: `planning/`

## Session workflow

1. Read `planning/PROGRESS.md`.
2. Check `FEATURE_CATALOG.md` before making claims about shipped functionality.
3. Update docs when the visible product surface changes.
4. Keep `CREDITS.md` current when adding external datasets, formulas, or algorithm references.

## Dependency reality

Use the actual manifests as source of truth:

- workspace: `Cargo.toml`
- core: `crates/rs_cam_core/Cargo.toml`
- CLI: `crates/rs_cam_cli/Cargo.toml`
- GUI: `crates/rs_cam_viz/Cargo.toml`

Do not document or rely on crates that are not currently in those manifests.

## Implementation expectations

- tests live close to the code they validate
- library code should avoid `unwrap()`
- if GUI state adds a field, audit setup-sheet, project-IO, and any test initializers for required updates
- if a feature is only present in UI/state and not end-to-end wired, document that honestly
