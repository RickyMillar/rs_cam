# BREP/STEP Post-Merge Review

Independent code-first findings report for the BREP/STEP merge on `master`.

- **Baseline:** current `master` at `f0a159d`
- **Merge commit:** `c5eb82b` (16 feature-branch commits + 3 post-merge fixups)
- **Scope:** 6,162 lines added, 1,272 removed across 58 files (16 new, 42 modified)
- **Deliverable:** report only, no code changes

---

## Merge Scope Summary

| Category | Files changed | Net lines |
|----------|:---:|---:|
| Core library (`rs_cam_core/src/`) | 14 | +2,055 |
| GUI/Viz (`rs_cam_viz/`) | 24 | +219 |
| CLI (`rs_cam_cli/`) | 3 | −219 |
| Tests + fixtures | 9 | +1,727 |
| Config/build (`Cargo.*`) | 4 | +466 |
| Docs/planning | 4 | +509 |

Key new modules: `enriched_mesh.rs` (587 lines), `step_input.rs` (442 lines).
New dependencies: `truck-stepio`, `truck-polymesh`, `geo` (BooleanOps).

---

## Findings

### High Severity

#### H1. No user-visible import feedback — failures are silent

**Claim:** When STEP import fails, the error is only logged via `tracing::error!` with no toast, modal, or status bar message. The user sees nothing.

**Why it matters:** A user selects a STEP file, the dialog closes, and nothing appears. They cannot distinguish "import in progress" from "import failed" without reading log output.

**Evidence:** `controller/events.rs:43-44` — `tracing::error!("Import failed: {error}")` with no UI notification path. Contrast with project load warnings which display a modal at `app.rs:2118-2137`.

**Subsystem:** User workflow / GUI.

**Direction:** Add an error toast or status bar message on import failure, matching the existing pattern for project load warnings.

---

#### H2. Face selection not cleared on model removal — stale selection references deleted model

**Claim:** `handle_remove_model` only clears `Selection::Model(model_id)`, not `Selection::Face(model_id, _)` or `Selection::Faces(model_id, _)`.

**Why it matters:** After removing a STEP model, the properties panel may attempt to look up face metadata from a model that no longer exists. The `if let` chain at `properties/mod.rs:226` fails gracefully (skips the block), but the selection enum stays set to a deleted entity — confusing state that persists until the user clicks something else.

**Evidence:** `controller/events.rs:403` checks `Selection::Model(model_id)` only. The `Selection::Face` and `Selection::Faces` variants (selection.rs:20-22) are not matched.

**Subsystem:** Architecture / state consistency.

**Direction:** Extend the model-removal selection clear to match on `Face(mid, _) | Faces(mid, _)` where `mid == model_id`.

---

#### H3. No validation of deserialized face_selection IDs against loaded enriched mesh

**Claim:** On project load, `face_selection` IDs from TOML are wrapped into `FaceGroupId` without checking they fall within the loaded enriched mesh's face count.

**Why it matters:** If the STEP file changed between saves (face topology altered in CAD), saved indices may reference wrong faces or exceed the face count. Out-of-range indices passed to `face_for_triangle` would panic. In-range-but-wrong indices silently select the wrong faces — a correctness hazard in a CAM tool where "wrong face" means "wrong machining boundary."

**Evidence:** `project.rs:1060-1064` — raw `u16` → `FaceGroupId` conversion with no bounds check. The architecture doc (SR-4.6.3) specified staleness detection and `ProjectLoadWarning::FaceSelectionStale`, which was never implemented.

**Subsystem:** Architecture / persistence safety.

**Direction:** After model re-import, validate that all face_selection IDs are < `enriched_mesh.face_groups.len()`. Clear invalid selections and emit a load warning.

---

### Medium Severity

#### M1. Face pick event handled in `app.rs`, bypassing controller event system

**Claim:** The `PickHit::ModelFace` handler at `app.rs:1415-1436` directly mutates `ToolpathEntry.face_selection` and `AppState.selection` without routing through `AppEvent` and the controller.

**Why it matters:** The rest of the codebase routes state mutations through controller events (e.g., `AppEvent::GenerateToolpath`, `AppEvent::RemoveModel`). Direct mutation in `app.rs` means: (a) undo cannot capture the change, (b) auto-regen stale detection works only because `stale_since` is manually set, and (c) future hooks or observers on state changes will miss face selection edits.

**Evidence:** `app.rs:1415-1436` — the face toggle writes to `entry.face_selection` and `state.selection` directly. Compare with `PickHit::StockFace` at `app.rs:1394` which goes through the same inline pattern but for a less complex edit.

**Subsystem:** Architecture / controller consistency.

**Direction:** Introduce `AppEvent::ToggleFaceSelection { toolpath_id, face_id }` and handle it in the controller, which would also enable undo tracking.

---

#### M2. No undo for face selection changes

**Claim:** `UndoAction` variants (`history.rs:7-32`) cover stock, post, tool, toolpath params, and machine changes — but `face_selection` is a separate field on `ToolpathEntry`, not part of `OperationConfig` or `DressupConfig`, so the existing `ToolpathParamChange` snapshot does not capture it.

**Why it matters:** A user can accidentally click-toggle the wrong face off their selection with no way to undo it. For complex multi-face selections, this is destructive.

**Evidence:** `history.rs:7-32` — no face_selection variant. `app.rs:1420-1429` — direct toggle without snapshot.

**Subsystem:** User workflow / GUI.

**Direction:** Either (a) extend `ToolpathParamChange` to include `face_selection`, or (b) add a `FaceSelectionChange` undo variant. Most naturally addressed by routing through controller events (see M1).

---

#### M3. Hover highlighting is stubbed — `hovered_face_id()` always returns `None`

**Claim:** The rendering pipeline accepts a `hovered_face` parameter and has the color logic ready (`mesh_render.rs:209,218`), but `app.rs:716-718` contains `// TODO: implement hover tracking` and returns `None`.

**Why it matters:** Users cannot preview which face they are about to click. On a model with many small faces, clicking blind and toggling is frustrating — especially since face selection currently cannot be undone (M2).

**Evidence:** `app.rs:716-718` — `fn hovered_face_id() -> Option<FaceGroupId>` hardcoded to `None`. `mesh_render.rs:218` — hover color logic exists but is never triggered.

**Subsystem:** User workflow / GUI.

**Direction:** Track hovered face from mouse-move + pick-on-hover. The per-frame GPU re-upload at `app.rs:764-773` already runs, so the cost is the ray cast — acceptable for single-click verification. Could be gated on whether a STEP model is loaded and a toolpath is selected.

---

#### M4. "2.5D (from SVG)" and "3D (from STL)" labels in toolpath creation menu

**Claim:** The toolpath creation UI labels 2.5D operations as "from SVG" and 3D operations as "from STL" (`project_tree.rs:328,336`), which is misleading when the geometry source is a STEP model with face selection.

**Why it matters:** A STEP user seeing "from SVG" may not realize they can create a pocket operation from their STEP model. The hint text in the properties panel ("Click faces in viewport to select") partially compensates, but only after the toolpath is already created and selected.

**Evidence:** `project_tree.rs:328` — `"2.5D (from SVG)"`, `project_tree.rs:336` — `"3D (from STL)"`.

**Subsystem:** User workflow / GUI.

**Direction:** Change labels to "2.5D (Boundary)" / "3D (Surface)" or similar geometry-source-agnostic terms, since these operations already work with face-derived boundaries.

---

#### M5. Surface classifier only detects axis-aligned planes — all other BREP surface types become `Unknown`

**Claim:** `classify_face_surface` at `step_input.rs:190-237` only checks if one bounding box axis is negligible to detect flat faces. Cylinders, cones, spheres, torus, and BSpline variants exist in the `SurfaceType` enum but are never produced by the classifier.

**Why it matters:** (a) The BREP Topology panel shows most faces as `Unknown`, reducing the user's ability to select the right geometry. (b) The `face_boundary_as_polygon` method only works for planar faces (relies on `SurfaceType::Plane`), so a tilted planar face that the heuristic misclassifies as Unknown cannot produce a boundary polygon. (c) The truck crate provides exact parametric surface data on the BREP faces, but `classify_face_surface` ignores it and re-derives from vertex positions.

**Evidence:** `step_input.rs:190-237` — checks `extent < 1e-3 * max_extent` for each axis. `enriched_mesh.rs:175-177` — `face_boundary_as_polygon` checks `SurfaceType::Plane` and `|normal.z| > 0.95`.

**Subsystem:** Architecture / information loss.

**Direction:** Extract surface type from truck's parametric representation (`Face::surface()`) instead of heuristic vertex analysis. This is achievable since truck's `NurbsSurface` and `Plane` types are accessible during the tessellation loop.

---

#### M6. No viz-layer tests exercise any BREP workflow

**Claim:** All worker tests pass `enriched_mesh: None` and `face_selection: None`. Controller tests likewise use `None`. Zero test exercises face picking, face selection toggle, enriched mesh rendering, face-derived boundary computation, or face_selection save/load.

**Why it matters:** The entire BREP GUI workflow — the user-facing product — has no automated verification. Core data structures and STEP parsing are tested (14 unit tests + 8 integration tests), but the integration from face pick → selection → compute → toolpath generation is only manually tested.

**Evidence:** `compute/worker/tests.rs` — all `ComputeRequest` instances have `enriched_mesh: None`. `controller/tests.rs` — same pattern.

**Subsystem:** Code quality / test coverage.

**Direction:** Add at least: (1) a worker test with `enriched_mesh + face_selection` verifying polygon injection, (2) a project round-trip test with `face_selection`, (3) a picking test that feeds a synthetic enriched mesh through `pick()`.

---

#### M7. Test STEP fixtures duplicated in two locations

**Claim:** The same 4 OCCT STEP files exist in both `crates/rs_cam_core/tests/fixtures/step/` and `tests/fixtures/step/` (792 lines duplicated).

**Why it matters:** Maintenance burden — updates to test files must happen in both locations or they drift.

**Evidence:** `occt-cube.step`, `occt-cone.step`, `occt-cylinder.step`, `occt-sphere.step` in both directories.

**Subsystem:** Code quality.

**Direction:** Remove one copy and use relative paths from the surviving location.

---

#### M8. Silent fallback when face selection produces no valid polygon

**Claim:** When selected faces are non-planar or non-horizontal, `faces_boundary_as_polygon` returns `None`. The compute path at `execute/mod.rs:349-353` falls back to stock bounding box with no warning to the user.

**Why it matters:** A user selects vertical wall faces, creates a pocket, generates — and gets a toolpath covering the entire stock. Nothing tells them their face selection was effectively ignored.

**Evidence:** `execute/mod.rs:348-353` — `faces_boundary_as_polygon(face_ids).unwrap_or(...)` fallback. `controller/events.rs:1028-1031` — same pattern. Neither path emits a warning or sets a status message.

**Subsystem:** User workflow / error communication.

**Direction:** When face_selection is non-empty but produces no polygon, set a visible warning on the toolpath (similar to existing error messages like "No 2D geometry" at events.rs:1066).

---

### Low Severity

#### L1. FEATURE_CATALOG.md and PROGRESS.md not updated

**Claim:** Neither document mentions BREP, STEP, face selection, or enriched mesh. The Import section of FEATURE_CATALOG lists only STL, SVG, DXF.

**Evidence:** `grep -i "brep\|step\|enriched\|face.select" FEATURE_CATALOG.md PROGRESS.md` → no matches.

**Direction:** Add STEP import and face selection to the shipped surface in FEATURE_CATALOG. Note the BREP merge in PROGRESS.md recent work.

---

#### L2. No face selection mode concept — interaction is implicit

**Claim:** The architecture doc (SR-4.4.5) and plan (Phase 5.2) specified an explicit face selection mode with "Select Faces" button, Escape to exit, and "Done" to confirm. What shipped is implicit: if workspace is Toolpaths and a toolpath is selected, clicks on STEP model faces toggle face selection. There is no explicit enter/exit mode.

**Evidence:** No `face_selection_mode` or `InteractionMode` in any viz source file. `app.rs:1415` — the pick handler fires on any click when conditions match. The properties panel at `mod.rs:1089-1098` shows hint text but no mode toggle button.

**Direction:** The implicit interaction works and is arguably simpler. The hint text partially compensates. Consider whether the explicit mode is needed for discoverability, or whether the hint text is sufficient.

---

#### L3. No u16 overflow guard on face index

**Claim:** Face indices are cast as `face_idx as u16` at `enriched_mesh.rs:273,285` and `step_input.rs:150-151` without checking for overflow.

**Evidence:** Comment at `enriched_mesh.rs:20` documents the 65535-face limit, but no `debug_assert!` or `TryFrom` enforces it.

**Direction:** Add `debug_assert!(face_idx <= u16::MAX as usize)` or use `u16::try_from().expect()` with a descriptive message. Unlikely to trigger for wood routing parts.

---

#### L4. `Selection::Faces` variant exists but is never constructed by UI code

**Claim:** `Selection::Faces(ModelId, Vec<FaceGroupId>)` at `selection.rs:22` is defined but no code path sets it. The face toggle at `app.rs:1434` always sets `Selection::Face` (singular), even when multiple faces are selected on the toolpath.

**Evidence:** `grep "Selection::Faces(" crates/rs_cam_viz/src/` → only match is in `properties/mod.rs:234` (the display handler).

**Direction:** Either wire `Selection::Faces` when `entry.face_selection.len() > 1` for richer properties display, or remove the variant to reduce dead state.

---

#### L5. `ModelId` shown as debug format in face properties panel

**Claim:** The face info panel at `properties/mod.rs:224` displays `format!("Model: {:?}", model_id)`, producing output like `Model: ModelId(0)` instead of the model's friendly name.

**Evidence:** `properties/mod.rs:224` — `{:?}` format on `ModelId`.

**Direction:** Look up `model.name` from `job.models` and display that instead.

---

#### L6. Enriched mesh rendering skips setup-local-frame transform

**Claim:** STL models get `transform_mesh(mesh, setup, stock)` for setup-local-frame rendering, but enriched mesh GPU data upload at `app.rs:764-773` does not apply this transform.

**Evidence:** Compare `app.rs:753-756` (STL transform path) with `app.rs:764-773` (enriched mesh path — no transform call).

**Direction:** Apply the same setup transform to enriched mesh vertices. Without this, STEP models may render in the wrong position when using non-identity setups (e.g., flipped stock).

---

#### L7. Boundary loops are tessellation-derived, not BREP-curve-derived

**Claim:** `extract_boundary_loops` at `step_input.rs:241-323` walks tessellation boundary edges rather than using the original BREP trim curves from truck.

**Evidence:** The function looks for edges that appear in exactly one triangle, then chains them into loops. Boundary precision is limited to the tessellation resolution (0.1mm chord height).

**Direction:** This is acceptable for v1 (boundary precision is within machining tolerance for wood). For future work, extracting BREP trim curves from truck would give exact boundaries independent of tessellation resolution.

---

#### L8. Single-face shell cloning is wasteful for large models

**Claim:** `step_input.rs:75-78` clones the entire shell's vertex and edge arrays for each face to create a single-face shell for per-face tessellation.

**Evidence:** `vertices: cshell.vertices.clone(), edges: cshell.edges.clone()` — full clone per face.

**Direction:** This is a workaround for truck's API. The cost is proportional to `face_count * (vertex_count + edge_count)`. For typical CAM parts (<100 faces, <10K vertices), this is negligible. For large assemblies, it could be significant. Monitor import times and optimize if they become a bottleneck.

---

## User Story Matrix

| # | Story | Score | Key gap |
|---|-------|-------|---------|
| 1 | Import STEP and understand what entered the project | Works with friction | No on-screen success/failure notification (H1) |
| 2 | Inspect model/face info to choose geometry | Works with friction | No hover highlight (M3), limited surface classification (M5), debug-format ModelId (L5) |
| 3 | Select faces and understand selection state | Works with friction | No undo (M2), `Selection::Faces` never constructed (L4), Toolpaths workspace only |
| 4 | Create operation from face selection | Works with friction | "from SVG"/"from STL" labels (M4), silent fallback on non-planar (M8) |
| 5 | Save, reload, continue editing | Works | Face selection persists; enriched mesh reconstructed. No staleness detection (H3) |
| 6 | Recover from invalid states | Works with friction | Silent partial import (catch_unwind + log), no face selection undo (M2), stale selection on model remove (H2) |

No story scores **missing**. All core BREP workflows function. The friction points cluster around feedback, discoverability, and error communication rather than correctness.

---

## Synthesis

### Product readiness

BREP/STEP is **functionally present but not product-ready in the GUI.** The core pipeline — STEP parse → tessellate → enrich → pick faces → derive polygon → generate toolpath → persist — works end-to-end. But the user experience has too many silent failures (H1, M8), undiscoverable interactions (L2, M4), and missing safety nets (H3, M2, H2) to confidently hand to a user who doesn't already know the feature exists.

The threshold for "enough GUI" is whether a user can complete the BREP workflow confidently without hidden knowledge. Currently they cannot: they must know to select a toolpath before clicking faces, they get no feedback when import fails or face selection is ignored, they cannot hover-preview faces, and they cannot undo face selection mistakes.

### Architectural fit

**Good.** The core data model (`EnrichedMesh`, `FaceGroup`, `BrepEdge`) is well-designed, cleanly layered in core with no viz dependencies, and the dual `mesh` + `enriched_mesh` storage on `LoadedModel` is an effective adapter that preserves full backward compatibility. The truck dependency is properly feature-gated. The face-to-polygon pipeline reuses the existing operation system rather than creating parallel paths.

The main architectural concern is the face pick handler bypassing the controller event system (M1). This is a layering violation that prevents undo integration and is inconsistent with how all other user interactions are routed.

### Top reuse opportunities

1. **Import path unification** (~130 lines reducible): filename extraction, controller import methods, file dialog blocks, AppEvent variants, and event dispatch arms are duplicated 4× per format. Unifying through `ModelKind` dispatch would reduce 8 files to 3 for adding a new format.
2. **Operation labels** are format-specific ("from SVG"/"from STL") when they should be geometry-type-generic ("Boundary"/"Surface").
3. The rendering split (smooth STL via `mesh_pipeline` vs. flat STEP via `colored_opaque_pipeline`) is architecturally correct but creates a visual inconsistency — STEP models always appear flat-shaded.

### Highest-risk gaps to address first

1. **H3: Face selection validation on load** — silent wrong-face selection is a CAM correctness risk.
2. **H1: Import failure feedback** — users cannot distinguish failed import from no-op.
3. **H2: Stale selection on model removal** — state consistency bug.
4. **M1 + M2: Controller routing + undo** — addressing M1 naturally enables M2.
5. **M8: Silent polygon fallback** — face selection that does nothing is worse than no face selection.
