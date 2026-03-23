# BREP/STEP Support: System Architecture

Reference: `research/09_parametric_geometry_and_brep.md`

---

## 1. Problems / User Needs

### P1: No face-level interaction with 3D models
When a user imports an STL, rs_cam sees a bag of triangles. They cannot click a flat
top face and say "face this surface," click a pocket floor and say "pocket within these
boundaries," or click a wall edge and say "contour this." Every professional CAM tool
supports this.

### P2: No STEP file import
STEP is the universal interchange format. Users design in Fusion 360 / SolidWorks / FreeCAD,
export STEP, and expect their CAM tool to read it. rs_cam only reads STL (mesh), SVG, and DXF.
Exporting STL throws away all face identity, surface type, and topological information.

### P3: 2.5D operations can't derive boundaries from 3D geometry
Pocket, profile, adaptive all require `Polygon2` from SVG/DXF. If the user has a STEP model
with a planar face defining a pocket boundary, they can't extract it — they must recreate it
in a 2D editor and import separately.

### P4: No surface type awareness
The CAM engine doesn't know a surface is cylindrical, planar, or freeform. Face-aware
operations could detect surface types and choose appropriate strategies, step-overs, and
lead-in patterns.

### P5: Boundary system limited to stock bounding box
`BoundaryContainment` clips toolpaths to the stock bbox. Users cannot select faces as
containment regions — they draw rectangular keep-out zones, which is imprecise.

---

## 2. What This Unlocks

- **STEP import**: read the dominant CAM interchange format
- **Face selection**: click a face in the viewport, highlight it, assign operations to it
- **Automatic boundary extraction**: select planar face(s) → derive 2D Polygon2 boundaries for pocket/profile/adaptive — no SVG/DXF needed
- **Face-based containment**: replace manual keep-out rectangles with face-derived boundaries
- **Surface-aware heuristics**: flat vs. curved face detection for operation suggestions
- **Foundation**: face-level rest machining, face-specific tolerances, feature recognition

---

## 3. Functional Requirements

### FR-1: STEP file import
Import AP203/AP214 STEP files containing solid BREP geometry via the `truck` crate
(pure Rust). Produce tessellated geometry with face group metadata.

### FR-2: Enriched mesh representation
Represent STEP geometry as an `EnrichedMesh`: a `TriangleMesh` augmented with face groups.
Each face group maps a contiguous range of triangles to a BREP face, carrying surface type,
boundary loops, bounding box, and adjacency.

### FR-3: Backward-compatible geometry pipeline
Existing 3D operations (drop cutter, waterline, adaptive3d, pencil, scallop) work unchanged
on STEP-imported geometry by extracting the inner `TriangleMesh`. STEP models satisfy
`GeometryRequirement::Mesh`.

### FR-4: Face picking in the viewport
Click the 3D viewport to select individual BREP faces. Ray cast → triangle intersection →
face group lookup.

### FR-5: Face highlighting
Selected faces render with distinct highlight color using the existing `ColoredMeshVertex`
pipeline.

### FR-6: Face-to-polygon boundary extraction
For planar face groups, project boundary loops to 2D `Polygon2` for use as input to existing
2.5D operations and as machining containment regions.

### FR-7: Face selection persistence
Face selections serialize to project file. Face IDs are stable across re-import of the same
STEP file (topological order).

### FR-8: Graceful degradation
When `truck` can't parse a file, report clearly with actionable suggestions. Support partial
imports where possible.

### FR-9: Feature-gated dependency
The `truck` dependency is behind a Cargo feature flag. The `EnrichedMesh` data structure
lives in core unconditionally (pure data, no external deps).

---

## 4. Sub-Requirements by System Layer

### 4.1 Core Geometry (`rs_cam_core`)

**SR-4.1.1: `EnrichedMesh` data structure**

New file: `crates/rs_cam_core/src/enriched_mesh.rs`

```rust
/// A triangle mesh with BREP face group metadata.
pub struct EnrichedMesh {
    pub mesh: Arc<TriangleMesh>,
    pub face_groups: Vec<FaceGroup>,
    pub triangle_to_face: Vec<u16>,              // tri index → face group index
    pub adjacency: Vec<(FaceGroupId, FaceGroupId)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FaceGroupId(pub u16);

pub struct FaceGroup {
    pub id: FaceGroupId,
    pub surface_type: SurfaceType,
    pub surface_params: SurfaceParams,
    pub triangle_range: Range<usize>,            // contiguous in mesh.triangles
    pub bbox: BoundingBox3,
    pub boundary_loops: Vec<Vec<P3>>,            // 3D polylines (outer + holes)
    pub boundary_loops_2d: Option<Vec<Vec<P2>>>, // only for planar faces
}

pub enum SurfaceType { Plane, Cylinder, Cone, Sphere, Torus, BSpline, Unknown }

pub enum SurfaceParams {
    Plane { normal: V3, d: f64 },
    Cylinder { axis_origin: P3, axis_dir: V3, radius: f64 },
    Cone { apex: P3, axis: V3, half_angle: f64 },
    Sphere { center: P3, radius: f64 },
    Torus { center: P3, axis: V3, major_radius: f64, minor_radius: f64 },
    BSpline,
    Unknown,
}
```

Design notes:
- Triangles within each `FaceGroup` are contiguous in the `TriangleMesh.triangles` array
  (tessellation produces them per-face). This allows GPU draw calls via index sub-ranges.
- `triangle_to_face` is O(1) lookup. At 2 bytes/triangle, 500K triangles = 1 MB.
- `boundary_loops_2d` pre-computed at import time for planar faces only.
- `mesh` is `Arc<TriangleMesh>` so `LoadedModel.mesh` can share the same allocation.

**SR-4.1.2: Mesh extraction**

```rust
impl EnrichedMesh {
    pub fn as_mesh(&self) -> &TriangleMesh { &self.mesh }
    pub fn mesh_arc(&self) -> Arc<TriangleMesh> { Arc::clone(&self.mesh) }
}
```

Existing operations call `require_mesh(req)` → get `&TriangleMesh` → work unchanged.

**SR-4.1.3: Face boundary → Polygon2 extraction**

```rust
impl EnrichedMesh {
    /// Single planar face → Polygon2. Returns None if face is non-planar.
    pub fn face_boundary_as_polygon(&self, id: FaceGroupId) -> Option<Polygon2>;

    /// Multiple coplanar faces → union Polygon2. Returns None if non-coplanar.
    pub fn faces_boundary_as_polygon(&self, ids: &[FaceGroupId]) -> Option<Polygon2>;
}
```

This bridges BREP face selection to existing pocket/profile/adaptive operations.

v1 restriction: only approximately-horizontal faces (|normal.z| > 0.95) produce 2D
boundaries. Non-horizontal faces return `None` with a message: "Face is not horizontal;
use a 3D operation."

**SR-4.1.4: Ray-triangle intersection**

New method on `Triangle`:
```rust
impl Triangle {
    /// Moller-Trumbore ray-triangle intersection. Returns parametric t if hit.
    pub fn ray_intersect(&self, origin: &P3, dir: &V3) -> Option<f64>;
}
```

Brute-force over AABB-filtered triangles is sufficient for picking (one ray per click).

**SR-4.1.5: No kernel dependency in core**

`enriched_mesh.rs` is pure data structures + queries. No `truck` dependency.
Only `step_input.rs` imports truck, behind `#[cfg(feature = "step")]`.

---

### 4.2 Import Pipeline

**SR-4.2.1: STEP import module**

New file: `crates/rs_cam_core/src/step_input.rs`, gated `#[cfg(feature = "step")]`

```rust
pub fn load_step(path: &Path, tolerance: f64) -> Result<EnrichedMesh, StepImportError>
```

Internal flow:
1. `truck_stepio::r#in::parse_step_file(path)` → STEP entities
2. Navigate to `CompressedSolid` or `Shell` topology
3. For each `Face`:
   a. Classify surface → `SurfaceType` + `SurfaceParams`
   b. Tessellate via `truck_meshalgo` → vertices + triangles for this face
   c. Extract boundary wires as polylines
   d. Record triangle range
4. Merge per-face tessellations into single `TriangleMesh` with contiguous face groups
5. Build `triangle_to_face` lookup
6. Build adjacency (faces sharing edge endpoints within tolerance)
7. For planar faces, project boundary loops to 2D

Default tolerance: 0.1 mm (appropriate for wood routing).

**SR-4.2.2: Error handling**

```rust
pub enum StepImportError {
    IoError(std::io::Error),
    ParseError(String),
    NoSolidFound,
    UnsupportedEntities {
        entity_types: Vec<String>,
        faces_loaded: usize,
        faces_failed: usize,
    },
    TessellationFailed(String),
}
```

`UnsupportedEntities` enables partial import — render what parsed, warn about what was lost.

**SR-4.2.3: Viz-layer import function**

In `crates/rs_cam_viz/src/io/import.rs`:
```rust
pub fn import_step(path: &Path, id: ModelId, tolerance: f64) -> Result<LoadedModel, String>
```

Produces `LoadedModel` with:
- `kind: ModelKind::Step`
- `enriched_mesh: Some(Arc::new(enriched))`
- `mesh: Some(enriched.mesh_arc())` ← shares the same `Arc<TriangleMesh>`
- `polygons: None`

**SR-4.2.4: Dependency management**

Workspace `Cargo.toml`:
```toml
truck-stepio = { version = "0.6", optional = true }
truck-topology = { version = "0.6", optional = true }
truck-meshalgo = { version = "0.6", optional = true }
truck-geometry = { version = "0.6", optional = true }
```

Core crate:
```toml
[features]
default = ["parallel"]
step = ["truck-stepio", "truck-topology", "truck-meshalgo", "truck-geometry"]
```

Viz/CLI enable via: `rs_cam_core = { path = "..", features = ["step"] }`

Pin to exact minor versions — truck is research-grade.

**SR-4.2.5: Unit handling**

STEP files carry units (typically mm per ISO 10303). Import reads the unit declaration
and converts to mm. `ModelUnits` records the source unit. Re-import on unit change
re-reads the file, same as STL rescale.

---

### 4.3 State Management (`rs_cam_viz`)

**SR-4.3.1: `ModelKind::Step`**

File: `crates/rs_cam_viz/src/state/job.rs`
```rust
pub enum ModelKind { Stl, Svg, Dxf, Step }
```

**SR-4.3.2: `LoadedModel` enriched mesh field**

```rust
pub struct LoadedModel {
    // ... existing fields ...
    pub enriched_mesh: Option<Arc<EnrichedMesh>>,  // new
}
```

| Model Kind | `mesh` | `polygons` | `enriched_mesh` |
|------------|--------|------------|-----------------|
| Stl | Some | None | None |
| Svg | None | Some | None |
| Dxf | None | Some | None |
| Step | Some (shared Arc) | None | Some |

**SR-4.3.3: Selection variants for faces**

File: `crates/rs_cam_viz/src/state/selection.rs`
```rust
pub enum Selection {
    // ... existing ...
    Face(ModelId, FaceGroupId),
    Faces(ModelId, Vec<FaceGroupId>),
}
```

Click: `Selection::Face`. Shift+click: accumulate into `Selection::Faces`.

**SR-4.3.4: Toolpath face binding**

Add to `ToolpathEntry`:
```rust
pub face_selection: Option<Vec<FaceGroupId>>,
```

When `Some`:
- 2.5D ops: selected planar faces' boundary loops → `Polygon2` input
- 3D ops: full mesh still passed (needed for correct Z), face selection → containment boundary
- Boundary clipping: face-derived polygon replaces stock bbox polygon

**SR-4.3.5: `GeometryRequirement` — no change**

STEP models satisfy `Mesh` directly. Face-aware features are opt-in via
`face_selection.is_some()`, not a new requirement variant. This avoids touching the
operation catalog for all 22 operations.

---

### 4.4 Viewport Rendering & Interaction

**SR-4.4.1: Face-colored mesh rendering**

When `EnrichedMesh` is loaded, render using `ColoredMeshVertex` pipeline (already exists
for sim stock). Each face group gets a deterministic pastel color. Selected faces get a
highlight color (e.g., `[0.3, 0.5, 1.0]`).

New constructor: `from_enriched_mesh(device, enriched_mesh, selected_faces)` builds
`ColoredMeshVertex` arrays using `triangle_to_face` for per-triangle colors.

First iteration can be simpler: render as single-color mesh (existing `MeshGpuData`),
with face colors only on hover/selection via an overlay.

**SR-4.4.2: Face highlight on selection**

When `Selection::Face(model_id, face_id)` is active, re-upload vertex buffer with the
selected face's triangles in highlight color. Matches existing pattern — `SimMeshGpuData`
already re-uploads colors via `update_colors_if_changed`.

**SR-4.4.3: Face hover highlighting**

On mouse move (when face selection mode is active), ray cast to find face under cursor,
render with subtle hover tint. Only active during face selection mode (not always — too
expensive for large meshes on every mouse move).

**SR-4.4.4: Ray-triangle picking in picking pipeline**

File: `crates/rs_cam_viz/src/interaction/picking.rs`

New `PickHit` variant:
```rust
PickHit::ModelFace { model_id: ModelId, face_id: FaceGroupId }
```

Insertion point: after fixture/keep-out/stock picks (line ~134), before toolpath picks.
Flow:
1. Use existing `unproject_ray()` → `(origin, dir)` (already computed at line 69-77)
2. For each model with `enriched_mesh`:
   a. Fast rejection: ray-AABB test against model bbox
   b. Iterate triangles, `Triangle::ray_intersect(origin, dir)`
   c. Find nearest hit triangle
   d. `enriched_mesh.triangle_to_face[hit_idx]` → `FaceGroupId`
3. Return `PickHit::ModelFace` if closer than any previous hit

**SR-4.4.5: Face selection mode UX**

Face selection available when:
- A STEP model with enriched mesh is loaded
- Workspace is `Toolpaths`
- A toolpath is selected in the properties panel

Properties panel shows "Select Faces" button → enters face selection mode.
- Click: select single face
- Shift+click: add/remove from multi-selection
- Escape or "Done": exit mode

---

### 4.5 Operation Binding

**SR-4.5.1: ComputeRequest face data**

Add to `ComputeRequest`:
```rust
pub face_selection: Option<Vec<FaceGroupId>>,
pub enriched_mesh: Option<Arc<EnrichedMesh>>,
```

Controller populates from `ToolpathEntry.face_selection` and `LoadedModel.enriched_mesh`.

**SR-4.5.2: Face-derived polygon input for 2.5D ops**

When `face_selection.is_some()` and operation requires `Polygons`:
1. Worker calls `enriched_mesh.faces_boundary_as_polygon(face_ids)`
2. Result replaces model-level `polygons` in request
3. If faces not planar/coplanar → clear error: "Selected faces are not planar"

This means: import STEP → create pocket → select planar face → pocket uses that face's
boundary. No SVG/DXF needed.

**SR-4.5.3: Face-derived containment for 3D ops**

When `face_selection.is_some()` and operation requires `Mesh`:
- Full mesh still passed (3D ops need complete surface for correct Z values)
- Face selection → projected bounding polygon → used as containment boundary
- Replaces stock-bbox-derived boundary in `effective_boundary()` at
  `compute/worker/execute/mod.rs:~332`

**SR-4.5.4: Interaction with existing boundary system**

Face selection provides an alternative **source polygon** for boundary clipping:
- No face selection: source = stock bbox rectangle (existing behavior)
- Face selection: source = face-derived polygon
- `BoundaryContainment` (center/inside/outside) still applies, offsetting by tool radius
- Keep-out zones still subtract from the source polygon

Code integration point: `execute/mod.rs` line ~332-368 where `stock_poly` is built.
```rust
// Existing:
let stock_poly = Polygon2::rectangle(bbox.min.x, ...);
// With face selection:
let stock_poly = if let Some(faces) = &face_ids {
    enriched.faces_boundary_as_polygon(faces)
        .unwrap_or_else(|| Polygon2::rectangle(bbox.min.x, ...))
} else {
    Polygon2::rectangle(bbox.min.x, ...)
};
```

**SR-4.5.5: STL backward compatibility**

If toolpath references STL model (no enriched mesh), `face_selection` forced to `None`.
Properties panel doesn't show face selection UI. All existing behavior unchanged.

---

### 4.6 Serialization / Project IO

**SR-4.6.1: `ModelKind::Step` in project file**

`ProjectModelSection` already has `kind: Option<ModelKind>`. Adding `Step` to the enum
with `#[serde(rename_all = "snake_case")]` serializes as `"step"`. Old project files
without this variant load correctly.

**SR-4.6.2: Face selection persistence**

Add to `ProjectToolpathSection`:
```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub face_selection: Option<Vec<u16>>,
```

**SR-4.6.3: Face ID stability**

Face group IDs assigned by topological order in the STEP file's `CLOSED_SHELL` entity list.
Deterministic for a given STEP file. If the CAD model changes and is re-exported, face IDs
may change.

On re-import: compare face count/types against saved expectations. If topology changed,
emit `ProjectLoadWarning::FaceSelectionStale { toolpath_name, ... }`, clear stale
selections, mark toolpaths for reconfiguration. Do NOT attempt geometric face matching
(unsolved problem — even Fusion 360 has "lost face references").

**SR-4.6.4: Format version**

Bump `PROJECT_FORMAT_VERSION` from current to +1 when STEP ships. Old files still load —
new fields are `Option` with `#[serde(default)]`.

---

### 4.7 Dev / Build Requirements

**SR-4.7.1: Feature flag architecture**

```
rs_cam_core features:
    default = ["parallel"]
    step = ["truck-stepio", "truck-topology", "truck-meshalgo", "truck-geometry"]

EnrichedMesh, FaceGroup, SurfaceType, FaceGroupId → unconditional (no external deps)
step_input.rs → #[cfg(feature = "step")] only
```

Face selection UI, picking, highlighting, face-based boundary extraction all work
unconditionally. Only the STEP parser is feature-gated.

**SR-4.7.2: truck version pinning**

Pin `=0.6.x` — truck is research-grade, breaking changes between minors.

**SR-4.7.3: Test corpus (pre-implementation gate)**

Before committing to truck, assemble and test:
- Simple shapes from FreeCAD (box, cylinder, multi-face pocket)
- Real files from Fusion 360 (AP214)
- Real files from SolidWorks (AP203/AP214)
- Known-difficult files (freeform NURBS, large face counts)

Run each through `truck-stepio` parser. If >50% of real files fail, evaluate
forking truck or pivoting to subprocess fallback.

**SR-4.7.4: CI**

- Add CI job building with `--features step`
- truck crates are pure Rust — no C++ toolchain needed
- Commit small test STEP fixtures (~10-50KB each)

**SR-4.7.5: Performance budget**

| Operation | Budget | Notes |
|-----------|--------|-------|
| STEP import | <5s | typical part, <100 faces, <500K tris |
| Face pick (ray cast) | <10ms | brute-force + AABB pre-filter |
| Face highlight re-upload | <1ms | vertex buffer color update |
| Memory overhead | ~1MB | per 500K-tri model (triangle_to_face lookup) |

---

## 5. Edge Cases

### 5.1 truck can't read a file
`load_step` returns `StepImportError`. Controller sets `LoadedModel.load_error`, model
appears as broken reference. UI shows error + suggests: "Try exporting as STL (face
selection won't be available)."

### 5.2 Multiple solids in STEP (assemblies)
v1: tessellate all solids into single `EnrichedMesh`. Face adjacency doesn't cross solid
boundaries. If per-solid control is needed, user exports parts separately.

### 5.3 Non-planar face selected for 2.5D operation
Clear error: "Selected faces are not planar; use a 3D operation or select horizontal faces."

### 5.4 Multi-face selection with mixed surface types
- 2.5D op: error if not all coplanar planes
- 3D op: combined face bounding box → containment boundary
- Highlight: all selected faces highlight simultaneously

### 5.5 Future: enriched mesh from STL
`EnrichedMesh` doesn't require STEP. Future work could cluster STL triangles by
normal + dihedral angles into face groups. Data structure is designed to support this.

### 5.6 Future: OBJ/3MF with face groups
`tobj` reads OBJ `g` directives as groups. `lib3mf-rs` reads per-face materials.
Either could produce `EnrichedMesh` without truck. The unconditional data structure
enables this path.

---

## 6. Implementation Phases

### Phase 1: Core data structures + STEP parsing
- `EnrichedMesh`, `FaceGroup`, `SurfaceType` in `rs_cam_core/enriched_mesh.rs`
- `Triangle::ray_intersect` (Moller-Trumbore)
- `step_input.rs` with truck, behind feature flag
- Test against STEP corpus
- Unit tests for face queries, ray intersection, polygon extraction

### Phase 2: Import pipeline + state wiring
- `ModelKind::Step`, `enriched_mesh` field on `LoadedModel`
- `import_step()` in viz import module
- Wire through controller events
- STEP renders using existing `MeshGpuData::from_mesh` (same as STL initially)
- Project IO: Step kind in model section

### Phase 3: Face picking + highlighting
- `PickHit::ModelFace` variant
- Ray-triangle mesh picking in `picking.rs`
- Face highlight via `ColoredMeshVertex` pipeline
- `Selection::Face` / `Selection::Faces` variants
- Properties panel display of selected faces

### Phase 4: Face-bound operations
- `face_selection` on `ToolpathEntry` + `ComputeRequest`
- Face-to-polygon boundary extraction in worker
- Face-derived containment in boundary clipping
- Face selection serialization in project IO
- Integration test: STEP → face select → pocket → G-code

---

## 7. Critical Files

| Purpose | File |
|---------|------|
| New: enriched mesh types | `crates/rs_cam_core/src/enriched_mesh.rs` |
| New: STEP import | `crates/rs_cam_core/src/step_input.rs` |
| Ray-triangle intersection | `crates/rs_cam_core/src/geo.rs` (Triangle) |
| Model kind + LoadedModel | `crates/rs_cam_viz/src/state/job.rs` |
| Selection enum | `crates/rs_cam_viz/src/state/selection.rs` |
| Pick pipeline | `crates/rs_cam_viz/src/interaction/picking.rs` |
| Import dispatch | `crates/rs_cam_viz/src/io/import.rs` |
| Compute request | `crates/rs_cam_viz/src/compute/worker.rs` |
| Boundary clipping | `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` |
| Project serialization | `crates/rs_cam_viz/src/io/project.rs` |
| Toolpath entry | `crates/rs_cam_viz/src/state/toolpath/entry.rs` |
| Operation configs | `crates/rs_cam_viz/src/state/toolpath/configs.rs` |
| ColoredMeshVertex (reuse) | `crates/rs_cam_viz/src/render/sim_render.rs` |
| Mesh GPU upload (reuse) | `crates/rs_cam_viz/src/render/mesh_render.rs` |
| Camera ray casting (reuse) | `crates/rs_cam_viz/src/render/camera.rs` |
