# Feeds Integration

This note describes the current end-to-end feeds/speeds integration between `rs_cam_viz` and `rs_cam_core`.

## Entry point

The active GUI integration lives in:

- `crates/rs_cam_viz/src/ui/properties/mod.rs`

The core calculator lives in:

- `crates/rs_cam_core/src/feeds/mod.rs`

The calculator is fed from:

- tool geometry and metadata
- stock material
- machine profile
- operation family and pass role
- optional vendor LUT observations
- setup context such as tool overhang and workholding rigidity

## Current GUI flow

1. Map the selected GUI operation to a `feeds::OperationFamily` and `PassRole`.
2. Extract optional axial/radial/scallop hints from the operation config.
3. Build `FeedsInput`.
4. Call `rs_cam_core::feeds::calculate`.
5. Auto-write enabled fields back into the operation config.
6. Cache the `FeedsResult` on the `ToolpathEntry`.

## Operation mapping

| GUI operation | Family | Pass role | Special hint |
|---------------|--------|-----------|--------------|
| Face | `Pocket` | `Roughing` | none |
| Pocket | `Pocket` | `Roughing` | none |
| Profile | `Contour` | `Finish` | none |
| Adaptive | `Adaptive` | `Roughing` | none |
| VCarve | `Trace` | `Finish` | `max_depth` as axial hint |
| Rest | `Pocket` | `Roughing` | none |
| Inlay | `Trace` | `Finish` | none |
| Zigzag | `Pocket` | `Roughing` | none |
| Trace | `Trace` | `Finish` | none |
| Drill | `Pocket` | `Roughing` | none |
| Chamfer | `Trace` | `Finish` | none |
| DropCutter | `Parallel` | `Finish` | none |
| Adaptive3d | `Adaptive` | `Roughing` | none |
| Waterline | `Parallel` | `Finish` | `z_step` as axial hint |
| Pencil | `Trace` | `Finish` | none |
| Scallop | `Scallop` | `Finish` | `scallop_height` as scallop hint |
| SteepShallow | `Parallel` | `Finish` | `z_step` as axial hint |
| RampFinish | `Parallel` | `Finish` | `max_stepdown` as axial hint |
| SpiralFinish | `Scallop` | `Finish` | none |
| RadialFinish | `Parallel` | `Finish` | none |
| HorizontalFinish | `Parallel` | `Finish` | none |
| ProjectCurve | `Trace` | `Finish` | none |

## Current known gaps

- workholding rigidity is still hardcoded to `Medium` in the GUI integration
- vendor-source labels are still raw observation IDs
- there is no GUI flow for loading additional LUT directories
- not every operation exposes a perfect DOC/WOC hint, so some operations still rely on calculator defaults

## Provenance

The calculator should be attributed directly to its integrated sources:

- vendor-source observations recorded in `crates/rs_cam_core/data/vendor_lut/source_manifest.json`
- direct material-property and formula references summarized in `CREDITS.md`

When a new external data source is introduced, update `CREDITS.md` and the source manifest in the same change.
