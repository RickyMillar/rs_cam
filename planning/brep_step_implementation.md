# BREP/STEP Implementation Plan

Reference: `architecture/brep_step_support.md`

## Context

This plan breaks the BREP/STEP architecture into concrete implementation tasks with
explicit dependencies. The goal is to identify what can be parallelized (worked on by
multiple agents simultaneously) vs what must be sequential.

**This work runs on a git worktree** to avoid conflicts with the parallel tridexel
unification effort (which touches adaptive, dexel_stock, simulation crates).

---

## Worktree Setup

All BREP/STEP work happens on a dedicated branch in a worktree:

```bash
# Create worktree and branch
git worktree add ../rs_cam_brep_step -b feature/brep-step-support

# Work happens in ../rs_cam_brep_step/
cd ../rs_cam_brep_step
```

### Conflict surface with tridexel work

| File | BREP/STEP change | Tridexel change | Risk |
|------|------------------|-----------------|------|
| `worker.rs` | Add 2 fields to ComputeRequest (Phase 6) | Modify sim/compute logic | Low — additive |
| `execute/mod.rs` | Add 5-line if block in boundary section (Phase 6) | Modify operation dispatch | Low — different regions |
| `events.rs` | Add import handling (Phase 3) | Modify sim events | Low — different sections |
| Everything else | New files or files tridexel won't touch | — | None |

**Phases 0–5 have zero conflict risk.** Phase 6–7 have minor additive conflicts
that are trivial to resolve on merge.

### Merge strategy

1. Complete all phases on `feature/brep-step-support` branch
2. After tridexel work merges to main, rebase this branch:
   ```bash
   git fetch origin main
   git rebase origin/main
   ```
3. Resolve any conflicts in worker.rs / execute/mod.rs / events.rs (additive only)
4. PR to main

### Commit strategy

One commit per phase (or per sub-phase for parallel phases):
- `Phase 0: truck STEP parser validation`
- `Phase 1: EnrichedMesh data structures + ray-triangle intersection`
- `Phase 2A: STEP import via truck (feature-gated)`
- `Phase 2B: ModelKind::Step + state/IO wiring`
- `Phase 3: Wire STEP import into GUI pipeline`
- `Phase 4A: Face picking in viewport`
- `Phase 4B: Face-colored mesh rendering`
- `Phase 5: Face selection UX`
- `Phase 6A: Face-bound 2.5D operations`
- `Phase 6B: Face-bound 3D containment`
- `Phase 7: Face selection persistence + polish`

---

## Dependency Graph Overview

```
Phase 0: truck validation (GATE — must pass before anything else)
    │
    ▼
Phase 1: Core data structures (rs_cam_core — no GUI, no truck)
    │
    ├──────────────────────────────┐
    ▼                              ▼
Phase 2A: STEP import           Phase 2B: State + IO wiring
(step_input.rs + truck)         (ModelKind, LoadedModel, project IO)
    │                              │
    └──────────┬───────────────────┘
               ▼
Phase 3: Import integration (wire 2A + 2B together)
    │
    ├──────────────────────────────┐
    ▼                              ▼
Phase 4A: Face picking          Phase 4B: Face-colored rendering
(ray-tri + picking.rs)          (ColoredMeshVertex for enriched mesh)
    │                              │
    └──────────┬───────────────────┘
               ▼
Phase 5: Face selection UX (selection state + properties panel + mode)
    │
    ├──────────────────────────────┐
    ▼                              ▼
Phase 6A: Face-bound 2.5D      Phase 6B: Face-bound 3D
(face → Polygon2 for ops)      (face → containment boundary)
    │                              │
    └──────────┬───────────────────┘
               ▼
Phase 7: Face selection persistence (project IO + staleness detection)
```

---

## Phase 0: truck Validation (GATE)

**Purpose**: Prove that `truck-stepio` can parse real STEP files before building on it.
**Parallel**: No — this is a go/no-go gate.

### Tasks

**0.1** Create a throwaway test crate (`tests/step_validation/`) that depends on
`truck-stepio`, `truck-topology`, `truck-meshalgo`, `truck-geometry`.

**0.2** Gather 5-10 test STEP files:
- 2-3 simple shapes exported from FreeCAD (box, cylinder, pocket plate)
- 2-3 real parts from Fusion 360 (AP214)
- 1-2 from SolidWorks (AP203)
- Commit small ones as test fixtures in `tests/fixtures/step/`

**0.3** Write a test that for each file:
1. Parses via `truck_stepio::r#in::*`
2. Navigates to shells/faces
3. Tessellates each face
4. Prints: face count, triangle count, surface types found, any errors

**0.4** Evaluate results:
- If ≥80% of files parse and tessellate: proceed to Phase 1
- If 50-80%: note which entity types fail, check if fixable
- If <50%: pivot to subprocess fallback or fork truck

**Exit criteria**: At least the FreeCAD files and one Fusion file parse successfully.

### Files
- `tests/step_validation/Cargo.toml` (throwaway, not workspace member)
- `tests/fixtures/step/*.step`

---

## Phase 1: Core Data Structures

**Purpose**: Build the `EnrichedMesh` type and supporting functions in `rs_cam_core`.
No external dependencies. No GUI changes.
**Parallel**: Tasks 1.1 and 1.2 can run in parallel.

### Task 1.1: `EnrichedMesh` data structures

New file: `crates/rs_cam_core/src/enriched_mesh.rs`

Create:
- `FaceGroupId(pub u16)` — newtype, `Debug Clone Copy PartialEq Eq Hash Serialize Deserialize`
- `SurfaceType` enum — `Plane, Cylinder, Cone, Sphere, Torus, BSpline, Unknown`
- `SurfaceParams` enum — variant per surface type with geometric parameters
- `FaceGroup` struct — id, surface_type, surface_params, triangle_range, bbox, boundary_loops, boundary_loops_2d
- `EnrichedMesh` struct — `mesh: Arc<TriangleMesh>`, face_groups, triangle_to_face, adjacency
- `EnrichedMesh::as_mesh()` → `&TriangleMesh`
- `EnrichedMesh::mesh_arc()` → `Arc<TriangleMesh>`
- `EnrichedMesh::face_group(&self, id: FaceGroupId) -> Option<&FaceGroup>`
- `EnrichedMesh::face_for_triangle(&self, tri_idx: usize) -> FaceGroupId`

Add `pub mod enriched_mesh;` to `crates/rs_cam_core/src/lib.rs`.

Unit tests:
- Build a synthetic `EnrichedMesh` from a known simple mesh (e.g., a box with 6 face groups)
- Test triangle_to_face lookup
- Test face_group retrieval
- Test as_mesh() returns correct mesh

### Task 1.2: Ray-triangle intersection + face boundary extraction

**1.2a** — Add `Triangle::ray_intersect(&self, origin: &P3, dir: &V3) -> Option<f64>` to
`crates/rs_cam_core/src/geo.rs`. Moller-Trumbore algorithm. Return parametric `t` (hit point = origin + t * dir).

Tests:
- Ray hitting triangle center
- Ray missing triangle
- Ray parallel to triangle
- Ray hitting edge (degenerate)
- Backface hit (t > 0 but from behind)

**1.2b** — Add face boundary → Polygon2 methods to `EnrichedMesh`:
```rust
pub fn face_boundary_as_polygon(&self, id: FaceGroupId) -> Option<Polygon2>
pub fn faces_boundary_as_polygon(&self, ids: &[FaceGroupId]) -> Option<Polygon2>
```

Only works for planar faces (|normal.z| > 0.95). Projects 3D boundary loops to XY plane.
Returns `None` for non-planar faces.

Tests:
- Horizontal planar face → correct Polygon2
- Tilted face → None
- Multiple coplanar faces → union polygon

### Verification
```bash
cargo test -p rs_cam_core -- enriched_mesh
cargo test -p rs_cam_core -- ray_intersect
```

---

## Phase 2A: STEP Import (can parallel with 2B)

**Purpose**: Build the truck-based STEP parser that produces `EnrichedMesh`.
**Depends on**: Phase 1 (needs `EnrichedMesh` type)
**Parallel with**: Phase 2B

### Task 2A.1: Feature flag + truck dependencies

Edit `Cargo.toml` (workspace):
```toml
truck-stepio = { version = "0.6", optional = true }
truck-topology = { version = "0.6", optional = true }
truck-meshalgo = { version = "0.6", optional = true }
truck-geometry = { version = "0.6", optional = true }
```

Edit `crates/rs_cam_core/Cargo.toml`:
```toml
[features]
default = ["parallel"]
parallel = ["rayon"]
step = ["truck-stepio", "truck-topology", "truck-meshalgo", "truck-geometry"]

[dependencies]
truck-stepio = { workspace = true, optional = true }
truck-topology = { workspace = true, optional = true }
truck-meshalgo = { workspace = true, optional = true }
truck-geometry = { workspace = true, optional = true }
```

Verify: `cargo check -p rs_cam_core` (no step) and `cargo check -p rs_cam_core --features step`

### Task 2A.2: `step_input.rs`

New file: `crates/rs_cam_core/src/step_input.rs`, gated `#[cfg(feature = "step")]`

```rust
pub fn load_step(path: &Path, tolerance: f64) -> Result<EnrichedMesh, StepImportError>
```

Implement:
1. Parse STEP file via truck_stepio
2. Extract shells/solids from the parsed table
3. For each topological face:
   a. Classify surface → SurfaceType + SurfaceParams
   b. Tessellate face → get per-face vertices + triangles
   c. Extract boundary wire curves as polylines
4. Merge all per-face meshes into one contiguous TriangleMesh
5. Build triangle_to_face lookup (u16 per triangle)
6. Build adjacency list (faces sharing edge vertices within tolerance)
7. For planar faces, compute boundary_loops_2d

Define `StepImportError` enum (IoError, ParseError, NoSolidFound, UnsupportedEntities, TessellationFailed).

Add `#[cfg(feature = "step")] pub mod step_input;` to lib.rs.

### Task 2A.3: STEP import tests

Move the Phase 0 test fixtures to `crates/rs_cam_core/tests/fixtures/step/`.
Write integration tests:
- Parse each test file → assert face count > 0
- Verify triangle_to_face covers all triangles
- Verify face_groups have non-empty triangle ranges
- Verify boundary_loops_2d is Some for planar faces

### Verification
```bash
cargo test -p rs_cam_core --features step -- step_input
```

---

## Phase 2B: State + IO Wiring (can parallel with 2A)

**Purpose**: Prepare `rs_cam_viz` state and project IO for STEP models, without the
actual import function (use mock/placeholder until 2A is ready).
**Depends on**: Phase 1 (needs `EnrichedMesh` type)
**Parallel with**: Phase 2A

### Task 2B.1: `ModelKind::Step`

File: `crates/rs_cam_viz/src/state/job.rs`

- Add `Step` variant to `ModelKind` enum
- Add `enriched_mesh: Option<Arc<EnrichedMesh>>` to `LoadedModel`
- Update `LoadedModel::placeholder()` to include `enriched_mesh: None`
- Update any match exhaustiveness for `ModelKind` (search for `ModelKind::` patterns)

### Task 2B.2: Project IO for Step models

File: `crates/rs_cam_viz/src/io/project.rs`

- `ProjectModelSection.kind` already `Option<ModelKind>` — `Step` auto-serializes via serde
- Add `.step` / `.stp` extension detection in `import_model` dispatch for auto-kind
- Update `load_model_from_section` to handle `ModelKind::Step`
  (initially: just set `load_error` saying "STEP not yet wired" until Phase 3)

### Task 2B.3: File dialog filter

File: `crates/rs_cam_viz/src/io/import.rs` (or wherever the file dialog is configured)

- Add `*.step` and `*.stp` to the file type filter list
- Add `import_step` function signature (stub that returns error until Phase 3)

### Verification
```bash
cargo test -p rs_cam_viz -- model_kind
cargo test -p rs_cam_viz -- project  # existing project IO tests still pass
```

---

## Phase 3: Import Integration

**Purpose**: Wire the STEP parser (2A) into the GUI import pipeline (2B).
**Depends on**: Phase 2A + Phase 2B (both must be complete)
**Parallel**: No

### Task 3.1: Wire `import_step` in viz

File: `crates/rs_cam_viz/src/io/import.rs`

Replace the stub with:
```rust
pub fn import_step(path: &Path, id: ModelId, tolerance: f64) -> Result<LoadedModel, String> {
    let enriched = rs_cam_core::step_input::load_step(path, tolerance)
        .map_err(|e| e.to_string())?;
    let mesh_arc = enriched.mesh_arc();
    Ok(LoadedModel {
        id,
        path: path.to_owned(),
        name: file_stem(path),
        kind: ModelKind::Step,
        mesh: Some(mesh_arc),
        polygons: None,
        enriched_mesh: Some(Arc::new(enriched)),
        units: ModelUnits::Millimeters,  // STEP is always mm
        winding_report: None,
        load_error: None,
    })
}
```

### Task 3.2: Wire through controller events

File: `crates/rs_cam_viz/src/controller/events.rs`

- In the import event handler, dispatch `.step`/`.stp` extensions to `import_step()`
- Handle import errors the same as STL (set `load_error`, add to models list)

### Task 3.3: Enable step feature in viz + cli

`crates/rs_cam_viz/Cargo.toml`:
```toml
rs_cam_core = { path = "../rs_cam_core", features = ["step"] }
```

Same for `crates/rs_cam_cli/Cargo.toml`.

### Task 3.4: Manual test

- Build and run the GUI: `cargo run -p rs_cam_viz --bin rs_cam_gui`
- Import a test STEP file
- Verify it renders in the viewport (same as STL — single color mesh)
- Verify model appears in project tree as "Step" kind
- Save and reload project — verify model persists

### Verification
```bash
cargo build -p rs_cam_viz
# Manual: import STEP, verify renders, save/load project
```

---

## Phase 4A: Face Picking (can parallel with 4B)

**Purpose**: Click viewport → identify which BREP face was hit.
**Depends on**: Phase 3 (needs enriched mesh loaded and renderable)
**Parallel with**: Phase 4B

### Task 4A.1: Mesh ray intersection utility

File: `crates/rs_cam_core/src/mesh.rs` or new `crates/rs_cam_core/src/mesh_pick.rs`

```rust
/// Cast a ray against a TriangleMesh, return (triangle_index, t) of nearest hit.
pub fn ray_pick_triangle(
    mesh: &TriangleMesh,
    origin: &P3,
    dir: &V3,
) -> Option<(usize, f64)>
```

Implementation: AABB rejection on mesh bbox first, then brute-force iterate
`mesh.faces` calling `Triangle::ray_intersect`. Return nearest hit.

### Task 4A.2: PickHit::ModelFace variant

File: `crates/rs_cam_viz/src/interaction/picking.rs`

Add variant:
```rust
PickHit::ModelFace {
    model_id: ModelId,
    face_id: FaceGroupId,
    hit_t: f64,
}
```

### Task 4A.3: Wire face picking into pick pipeline

File: `crates/rs_cam_viz/src/interaction/picking.rs`

In `pick()`, after the stock face pick block (line ~134, before toolpath picks):
```rust
// Model face picking (enriched mesh only)
for model in &job.models {
    if let Some(enriched) = &model.enriched_mesh {
        let mesh = enriched.as_mesh();
        if mesh.bbox.ray_intersect(&origin, &dir).is_none() { continue; }
        if let Some((tri_idx, t)) = ray_pick_triangle(mesh, &origin, &dir) {
            if t < best_t {
                best_t = t;
                let face_id = enriched.face_for_triangle(tri_idx);
                best_hit = Some(PickHit::ModelFace {
                    model_id: model.id,
                    face_id,
                    hit_t: t,
                });
            }
        }
    }
}
```

### Verification
```bash
cargo test -p rs_cam_core -- ray_pick
cargo build -p rs_cam_viz  # compiles with new PickHit variant
```

---

## Phase 4B: Face-Colored Rendering (can parallel with 4A)

**Purpose**: Render enriched mesh with per-face-group colors.
**Depends on**: Phase 3
**Parallel with**: Phase 4A

### Task 4B.1: Enriched mesh GPU data builder

File: `crates/rs_cam_viz/src/render/mesh_render.rs`

Add function:
```rust
pub fn enriched_mesh_gpu_data(
    device: &wgpu::Device,
    enriched: &EnrichedMesh,
    selected_faces: &[FaceGroupId],
    hovered_face: Option<FaceGroupId>,
) -> SimMeshGpuData  // reuse the colored mesh type from sim_render
```

Implementation:
- For each triangle in mesh:
  - Look up face group via `triangle_to_face`
  - Assign color:
    - Selected face → highlight blue `[0.3, 0.5, 1.0]`
    - Hovered face → soft cyan `[0.4, 0.7, 0.8]`
    - Default → deterministic pastel from face group id
  - Build `ColoredMeshVertex` with position, flat normal, color
- Upload as vertex + index buffers

### Task 4B.2: Wire into render resources

File: `crates/rs_cam_viz/src/render/mod.rs`

- Add `enriched_mesh_data: Option<SimMeshGpuData>` to `RenderResources`
- In the upload path: if loaded model has enriched_mesh, build via 4B.1
- In the draw path: render `enriched_mesh_data` using `sim_mesh_pipeline` (already exists)
  instead of `mesh_data` when the model is STEP

### Task 4B.3: Dirty tracking for face selection changes

When selection changes (face selected/deselected/hovered), mark enriched mesh GPU data
as dirty for re-upload. Use the existing `edit_counter` pattern or a simple `Option<Instant>`.

### Verification
```bash
cargo build -p rs_cam_viz
# Manual: import STEP, verify faces render with distinct colors
```

---

## Phase 5: Face Selection UX

**Purpose**: Wire face picking + rendering into a usable selection workflow.
**Depends on**: Phase 4A + Phase 4B (both must be complete)
**Parallel**: No — this ties everything together

### Task 5.1: Selection state variants

File: `crates/rs_cam_viz/src/state/selection.rs`

Add:
```rust
Face(ModelId, FaceGroupId),
Faces(ModelId, Vec<FaceGroupId>),
```

### Task 5.2: Face selection mode

Add `face_selection_mode: bool` to app state (or a `InteractionMode` enum).

When active:
- Viewport clicks route to face picking instead of normal selection
- Shift+click toggles face in/out of multi-selection
- Escape exits mode
- The current face selection is stored on the active `ToolpathEntry.face_selection`

### Task 5.3: Handle PickHit::ModelFace in controller

File: `crates/rs_cam_viz/src/controller/events.rs` (or wherever PickHit is dispatched)

When `PickHit::ModelFace` received:
- If in face_selection_mode: update the active toolpath's face_selection
- If not in face_selection_mode: set `Selection::Face(model_id, face_id)` and show
  face info in properties panel

### Task 5.4: Properties panel — face info + select button

File: `crates/rs_cam_viz/src/ui/properties/mod.rs` (or operations.rs)

When a toolpath is selected and model is STEP:
- Show "Select Faces" button → enters face_selection_mode
- Show count of selected faces + surface types
- Show "Clear Faces" button
- When face_selection_mode active: show "Done" button

### Task 5.5: Face hover

On mouse move in viewport (when face_selection_mode active):
- Call pick to get hovered face
- Store as `hovered_face: Option<(ModelId, FaceGroupId)>` in app state
- Trigger enriched mesh GPU re-upload with hover color

### Verification
```bash
cargo build -p rs_cam_viz
# Manual: import STEP → select toolpath → "Select Faces" → click faces
# → verify highlight, shift+click multi-select, Escape exits
```

---

## Phase 6A: Face-Bound 2.5D Operations (can parallel with 6B)

**Purpose**: Selected planar faces → Polygon2 → pocket/profile/adaptive input.
**Depends on**: Phase 5
**Parallel with**: Phase 6B

### Task 6A.1: Add face_selection to ToolpathEntry + ComputeRequest

File: `crates/rs_cam_viz/src/state/toolpath/entry.rs`
- Add `face_selection: Option<Vec<FaceGroupId>>` to `ToolpathEntry` and `ToolpathEntryInit`
- Wire through `from_init`, `duplicate_from`, `clear_runtime_state`

File: `crates/rs_cam_viz/src/compute/worker.rs`
- Add `face_selection: Option<Vec<FaceGroupId>>` to `ComputeRequest`
- Add `enriched_mesh: Option<Arc<EnrichedMesh>>` to `ComputeRequest`

### Task 6A.2: Face-derived polygon injection in controller

File: `crates/rs_cam_viz/src/controller/events.rs`

In `submit_toolpath_compute` (~line 913-1114), when building ComputeRequest:
- If `entry.face_selection.is_some()` and model has `enriched_mesh`:
  - Extract polygon via `enriched_mesh.faces_boundary_as_polygon(face_ids)`
  - Set `req.polygons = Some(Arc::new(vec![polygon]))` — overriding model polygons
  - This lets 2.5D ops (pocket, profile, adaptive) use face-derived boundaries

### Task 6A.3: Test end-to-end

Write integration test:
1. Load a STEP file with a planar pocket face
2. Create pocket operation
3. Set face_selection to the pocket face
4. Run compute
5. Verify toolpath is generated within the face boundary

### Verification
```bash
cargo test -p rs_cam_viz --features step -- face_bound
# Manual: STEP → pocket → select face → generate → verify toolpath follows face boundary
```

---

## Phase 6B: Face-Bound 3D Operations (can parallel with 6A)

**Purpose**: Selected faces → containment boundary for 3D operations.
**Depends on**: Phase 5
**Parallel with**: Phase 6A

### Task 6B.1: Face-derived containment in boundary clipping

File: `crates/rs_cam_viz/src/compute/worker/execute/mod.rs`

In the boundary clipping section (~line 332-368), replace the stock_poly source:
```rust
let stock_poly = if let (Some(face_ids), Some(enriched)) =
    (&req.face_selection, &req.enriched_mesh)
{
    enriched.faces_boundary_as_polygon(face_ids)
        .unwrap_or_else(|| Polygon2::rectangle(bbox.min.x, bbox.min.y, bbox.max.x, bbox.max.y))
} else {
    Polygon2::rectangle(bbox.min.x, bbox.min.y, bbox.max.x, bbox.max.y)
};
```

The rest of the boundary pipeline (keep-out subtraction, BoundaryContainment offset) works unchanged.

### Task 6B.2: Test

Integration test: STEP → waterline → select faces → verify toolpath is bounded to face region.

### Verification
```bash
# Manual: STEP → waterline → select faces → generate → verify containment
```

---

## Phase 7: Persistence + Polish

**Purpose**: Face selections survive save/load. Staleness detection.
**Depends on**: Phase 6A + 6B

### Task 7.1: face_selection in ProjectToolpathSection

File: `crates/rs_cam_viz/src/io/project.rs`

Add to `ProjectToolpathSection`:
```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub face_selection: Option<Vec<u16>>,
```

Wire through `save_toolpath_section` and `load_toolpath_section`.

### Task 7.2: Face ID staleness detection

On project load, after re-importing STEP model:
- If `face_selection` references face IDs beyond the enriched mesh's face count:
  - Clear the stale selection
  - Add `ProjectLoadWarning::FaceSelectionStale { toolpath_name, ... }`
  - Show warning banner in UI

### Task 7.3: Bump format version

Bump `PROJECT_FORMAT_VERSION` to 3. Old v2 files load fine (new fields are `Option` with
`#[serde(default)]`).

### Task 7.4: Update FEATURE_CATALOG.md and docs

Document STEP import and face selection as shipped capabilities.

### Verification
```bash
cargo test -p rs_cam_viz -- project  # IO round-trip tests
# Manual: save project with face selections → reload → verify preserved
# Manual: modify STEP in FreeCAD → re-export → reload project → verify warning
```

---

## Summary: Parallelization Map

```
Sequential gates:       Phase 0 → Phase 1 → Phase 3 → Phase 5 → Phase 7

Can parallel:           Phase 2A ║ Phase 2B     (after Phase 1)
                        Phase 4A ║ Phase 4B     (after Phase 3)
                        Phase 6A ║ Phase 6B     (after Phase 5)

Task-level parallel:    1.1 ║ 1.2              (within Phase 1)
```

**Total work streams**: 3 opportunities for 2-way parallelism, plus task-level
parallelism within Phase 1.

---

## Estimated Scope Per Phase

| Phase | New files | Modified files | Tests |
|-------|-----------|----------------|-------|
| 0 | 1 throwaway crate | 0 | validation script |
| 1 | 1 (enriched_mesh.rs) | 2 (geo.rs, lib.rs) | ~15 unit tests |
| 2A | 1 (step_input.rs) | 2 (Cargo.tomls) | ~8 integration tests |
| 2B | 0 | 3 (job.rs, project.rs, import.rs) | ~5 tests |
| 3 | 0 | 3 (import.rs, events.rs, Cargo.tomls) | manual test |
| 4A | 0-1 | 2 (picking.rs, mesh.rs) | ~5 tests |
| 4B | 0 | 2 (mesh_render.rs, mod.rs) | manual test |
| 5 | 0 | 4 (selection.rs, events.rs, properties, app.rs) | manual test |
| 6A | 0 | 3 (entry.rs, worker.rs, events.rs) | ~3 integration tests |
| 6B | 0 | 1 (execute/mod.rs) | ~2 tests |
| 7 | 0 | 2 (project.rs, FEATURE_CATALOG.md) | ~3 IO round-trip tests |
