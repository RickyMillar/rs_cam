# Simulation span coverage audit

## Purpose

Track implementation coverage for structural spans and semantic trace detail that feed simulation diagnostics, UI outline navigation, and MCP tools (`inspect_spans`, `get_cut_trace`, `narrate_toolpath`).

The goal is that every generated toolpath carries useful move-linked structure below the top-level `Operation` span whenever the operation has identifiable passes, holes, chains, rows, slices, rings, or cutting runs.

## Data flow

1. Operation generation returns an `AnnotatedToolpath` from `execute_operation_annotated()`.
2. Dressups remap existing spans and add modifier spans (`Entry`, `LeadOut`, `LinkBridge`, `DressupArtifact`).
3. Boundary clipping remaps spans through the provenance map.
4. Simulation computes `span_paths_by_move()` and stamps each `SimulationCutSample` with the structural `SpanId` path.
5. `SimulationCutTrace` propagates sample span paths to hotspots/issues.
6. UI/MCP consumers use spans for outline navigation and filtering.

Semantic traces are parallel: they provide richer named hierarchy when operation-specific runtime events exist. Structural spans must still be present for simulation-side filtering and per-move selection.

## Coverage status

Legend:

- `Rich`: operation-specific pass/region/hole/ring/span data.
- `Generic`: derived from move structure (cutting runs and/or inferred depth passes).
- `Top only`: only the top-level `Operation` span.

| Operation | Structural span target | Current implementation status | Notes |
|---|---|---:|---|
| Face | depth passes + raster/stripe runs | Generic | derived from cutting runs/depths |
| Pocket | depth passes + contour/zigzag runs | Generic | uses configured cutting levels where available |
| Profile | depth passes + contour runs | Generic | final tabs are preserved by dressup remap |
| Adaptive 2D | depth passes + adaptive cut runs | Generic + semantic | semantic runtime events also exist |
| VCarve | centerline/area cut runs | Generic | no VCarve-specific payload yet |
| Rest | depth passes + residual runs | Generic | uses configured cutting levels |
| Inlay | female/male/run regions | Generic | future: explicit female/male spans |
| Zigzag | depth passes + raster rows | Generic | uses configured cutting levels |
| Trace | depth passes + chain/ring runs | Generic | fixes previous top-only coverage |
| Drill | hole spans + peck/plunge child spans | Rich generic drill derivation | avoids `DepthPass` barriers so hole TSP remains safe |
| AlignmentPinDrill | hole spans + peck/plunge child spans | Rich generic drill derivation | same as Drill |
| Chamfer | contour runs | Generic | no chamfer-specific payload yet |
| DropCutter | raster rows | Generic | no depth pass barriers; operation allows global reorder |
| Adaptive 3D | regions + z levels + barriers | Rich | existing annotation-based spans retained; z-level/region spans now have labels |
| Waterline | inferred z slices + contour runs | Generic | future: explicit waterline slice annotations |
| Pencil | centerline/offset pass annotations | Rich semantic + annotation-labeled structural | event labels converted to `Region` spans |
| Scallop | ring annotations | Rich semantic + annotation-labeled structural | event labels converted to `Region` spans |
| Steep/Shallow | steep/shallow paths | Generic structural + generic semantic | future: explicit steep/shallow classification spans |
| Ramp Finish | ramp annotations | Rich semantic + annotation-labeled structural | event labels converted to `Region` spans |
| Spiral Finish | ring annotations | Rich semantic + annotation-labeled structural | event labels converted to `Region` spans |
| Radial Finish | ray runs | Generic | no explicit angle payload yet |
| Horizontal Finish | shelf/slice runs | Generic | no explicit shelf payload yet |
| Project Curve | projected curve runs | Generic | future: source curve spans |

## Implementation checklist

- [x] Document audit findings and coverage goals.
- [x] Add central structural span derivation helpers in `compute::spans`.
- [x] Add depth-pass + cutting-run spans for depth-stepped 2.5D operations.
- [x] Add hole + peck/plunge spans for drilling operations without disabling global hole reorder.
- [x] Add generic cutting-run spans for operations that previously emitted only `Operation`.
- [x] Keep Adaptive3D annotation-derived spans as the richest path.
- [x] Fix Simulation op-list fallback so operation-only spans no longer hide semantic traces.
- [x] Add explicit annotation-to-structural-span conversion for Pencil/Scallop/Ramp/Spiral labels.
- [x] Add explicit semantic children for Drill (`Hole`, `Cycle`) and Trace (`Chain`) rather than relying only on structural labels.
- [x] Add per-span aggregate summaries to `get_cut_trace` or `SimulationCutTrace`.
- [x] Add end-to-end tests for every operation family's expected span kinds.

## Verification notes

Current targeted tests assert more than the presence of an `Operation` item:

- `cargo test -q -p rs_cam_core all_operation_families_emit_expected_structural_span_kinds`: covers all 23 `OperationType::ALL` families, including system-only `AlignmentPinDrill`, and checks expected span kinds plus representative labels.
- `cargo test -q -p rs_cam_core annotated_output`: checks Trace depth/region spans and Drill hole/plunge spans without `DepthPass` barriers.
- `cargo test -q -p rs_cam_core semantic_trace_has`: checks Trace `DepthLevel`/`Chain` semantics and Drill `Hole`/`Cycle` semantics.
- `cargo test -q -p rs_cam_core compute::spans`: checks central span helper behavior, including labeled runtime events and Adaptive 3D spans.
- `cargo test -q -p rs_cam_viz drill_semantic_trace_records_cycle_children`: checks the GUI worker path preserves Drill semantic children.

Update this document whenever new operation-specific span emitters are added.
