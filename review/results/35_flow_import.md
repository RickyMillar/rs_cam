# Review: Import Flow

## Summary

The import flow in rs_cam supports three formats (STL, SVG, DXF) with largely complete UI wiring and cross-format parsing logic. STL imports include winding consistency checking and auto-scaling by unit type. The flow successfully integrates loaded models into the project state, GPU rendering pipeline, and stock auto-sizing. However, several issues exist: SVG/DXF imports don't trigger viewport camera fitting, re-import/update workflows are unsupported, there's no model deletion UI, and hardcoded parse tolerances lack user control.

## Findings

### STL Import

- Full pipeline: File dialog -> `import_stl()` -> `TriangleMesh::from_stl_scaled()` -> `stl_io` binary/ASCII auto-detection
- Winding consistency check with automatic repair (BFS propagation from topmost normal) at >5% inconsistency threshold
- Stores winding report percentage for user visibility in properties panel
- Unit scaling supported with 4 presets (mm/inch/cm/m) plus custom scale factor
- Stock auto-sizing from mesh bounding box when `stock.auto_from_model` is true
- GPU upload flagged correctly via `pending_upload = true`
- Selection auto-updates to newly imported model
- Error messages wrapped in `Result<LoadedModel, String>` format
- Scale factor hardcoded to 1.0 on first import; user must manually rescale via `rescale_model()` event. No unit selection dialog at import time (controller/io.rs:15)
- Automatic winding flip logic may overcorrect: mesh.rs:106-109 uses a fixed 5% threshold to trigger `fix_winding()` without user control
- No re-import/refresh workflow: if STL file is modified on disk, there's no "Update Model" button

### SVG Import

- Closed path flattening with configurable Bezier tolerance (currently 0.1 mm)
- Containment detection: inner shapes automatically become holes
- Explicit open-path rejection (edges without Z/close command)
- Automatic CCW winding normalization via `ensure_winding()`
- Comprehensive unit tests covering rectangles, circles, curves, multi-path, holes (svg_input.rs:168-332)
- Hardcoded tolerance of 0.1 mm (svg_input.rs:45) not user-configurable. For large designs (> 1000mm), this may create excessive vertices
- No camera fit or viewport centering after SVG import
- SVG coordinates are dimensionless; code treats them as mm (svg_input.rs:59) but real SVGs often use px (96 DPI ~= 0.265 mm/px). No user warning
- Empty SVG handling returns `Ok(Vec::new())` silently

### DXF Import

- Support for LwPolyline, Polyline, Circle, Ellipse entities
- Bulge arc tessellation with angular tolerance (currently 5.0 degrees)
- Closed entity filtering (ignores open paths)
- Containment detection matching SVG flow with CCW winding enforcement
- Reasonable test coverage of rectangles, circles, bulge arcs, multi-entity (dxf_input.rs:242-408)
- Hardcoded arc tessellation tolerance of 5.0 degrees (dxf_input.rs:67). For tight curves or large circles, this may produce visible faceting (~72 vertices for r=100mm circle)
- No DXF unit header (INSUNIT) handling; assumes coordinates are in mm (dxf_input.rs:23-27)
- 3D polylines flattened to Z=0; partial ellipse sweeps forced to full circle (dxf_input.rs:124-130, 210-216)
- No error context for malformed DXF: `Io(dxf::DxfError)` variant provides only the underlying crate's message

### Cross-cutting

- **GPU Upload**: `pending_upload = true` flagged on STL import only; SVG/DXF don't set it (bug — controller/io.rs:29-45)
- **Model State**: LoadedModel holds optional mesh/polygons and preserves load_error for broken round-trips
- **Toolpath Reference Safety**: Toolpaths reference model_id by value; if model is missing, toolpath status set to error (controller/events.rs:~640-650). No cascade cleanup on deletion (but deletion UI doesn't exist)
- **File Dialog**: Uses `rfd::FileDialog` with format filters; path persisted in `LoadedModel::path`
- **Project Persistence**: Models serialized in ProjectFile with path and metadata. On load, missing files trigger `ProjectLoadWarning::MissingModelFile`; import errors trigger `ProjectLoadWarning::ModelImportFailed`. Warnings shown in UI modal (controller/io.rs:96-114)
- **Error Handling**: All import functions return Result types. Errors propagate to controller, logged as `tracing::error!`. No user-facing error dialog; errors only appear in logs and optionally in toolpath status
- **No model deletion UI**: models can only be hidden via toolpath visibility, not removed from project
- **No re-import/update workflow**: modifying source file requires delete+re-import (impossible without deletion UI)
- **STEP/IGES**: Not supported. Not listed in FEATURE_CATALOG.md. Explicitly out of scope for wood routing
- **Mesh repair**: STL import doesn't validate manifoldness, fill holes, or remove degenerate triangles. OK for wood routing (typically simple blocks/plates)

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | Medium | SVG/DXF imports don't set `pending_upload = true`, so GPU mesh not refreshed; also no camera fit for 2D models | controller/io.rs:29-45 |
| 2 | Medium | SVG flatten tolerance (0.1 mm) and DXF arc tolerance (5 deg) are hardcoded; no user control | svg_input.rs:45, dxf_input.rs:67 |
| 3 | Medium | No model deletion UI exists; models can only be hidden via toolpath visibility | project_tree.rs (no RemoveModel event) |
| 4 | Medium | No re-import/update workflow; modifying source file requires delete+re-import (impossible without deletion UI) | No controller method |
| 5 | Low | STL always imports with scale=1.0; unit selection dialog missing at import time | controller/io.rs:15 |
| 6 | Low | DXF ignores INSUNIT header; assumes coordinates are in mm | dxf_input.rs:23-27 |
| 7 | Low | DXF flattens 3D polylines to Z=0; partial ellipse sweeps forced to full circle | dxf_input.rs:124-130, 210-216 |
| 8 | Low | SVG units undefined; treated as mm without user warning (px != mm) | svg_input.rs:59 |
| 9 | Low | Automatic mesh winding flip at >5% inconsistency may overcorrect intentional flips | mesh.rs:106-109 |
| 10 | Low | Empty SVG/DXF allowed (no geometry); no warning or indication | svg_input.rs:254-256 |
| 11 | Low | No error context wrapping for `dxf::DxfError` or usvg parse failures | dxf_input.rs:24, svg_input.rs:33 |

## Test Gaps

- No integration test for STL -> GPU upload -> viewport display (pending_upload flow not tested)
- No test for re-import scenario (file on disk modified; project reloaded)
- No test for model deletion cascade (if deleting a model, are toolpath references cleaned?)
- No test for project load with missing model file (ProjectLoadWarning coverage is UI-only)
- No test for mixed import scenario (STL + SVG + DXF in same project)
- No unit selection at import time (currently no UI for it; scale=1.0 always)
- No SVG viewBox/unit inference test (coordinates treated as mm without validation)
- No DXF INSUNIT parsing test (hardcoded mm assumption untested)

## Suggestions

1. **Add GPU upload flag for SVG/DXF** (`pending_upload=true` in controller/io.rs:29-45). Also add camera fit for 2D models by returning a bounding box from the import function
2. **Make tolerances configurable** via UI or per-operation config (SVG flatten tolerance, DXF arc tolerance). Default to current values but allow per-import override
3. **Add model deletion + cascade cleanup**: `AppEvent::RemoveModel(model_id)` -> remove from `job.models` -> set error status on toolpaths referencing deleted model
4. **Support re-import workflow**: `AppEvent::ReloadModel(model_id)` -> call `import_model()` with stored path/units -> preserve model_id, update in-place
5. **Add unit selection dialog at import**: for STL show 4-choice menu (mm/in/cm/m); for SVG/DXF offer px->mm conversion or warning
6. **Warn on empty geometry**: if `polygons.is_empty()` after SVG/DXF parse, log warning and set `load_error`
7. **Add DXF INSUNIT parsing**: read header INSUNIT, compute scale factor, apply before tessellation
8. **Add import tracing spans**: wrap import in `tracing::info_span!("import_stl", path=?)` to log import time/size
