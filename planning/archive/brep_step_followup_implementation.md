# BREP Follow-up: Face/Edge-Driven Workflows

## Context

The BREP/STEP foundation is built (Phases 0-7 on feature/brep-step-support).
`enriched_mesh` and `face_selection` flow through to `ComputeRequest` but are
**never consumed in the execute path**. This plan wires face and edge data into
the CAM operations to unlock face-derived boundaries, polygon extraction, edge
tracing, and auto-depth — aligning STEP workflows with STL and SVG/DXF.

Continues on the same `feature/brep-step-support` worktree branch.

---

## What This Unlocks

| Today | After |
|-------|-------|
| STEP renders with face colors but face selection does nothing | Select face → pocket, profile, trace, adaptive just work |
| Machining boundary is always the stock rectangle | Select faces → boundary follows face edges |
| Need separate SVG/DXF for 2.5D operations | Derive polygons directly from STEP faces |
| Pencil detects edges from mesh normals (fragile) | BREP provides exact edges with known dihedral angles |
| Depths are always manual | Face Z value auto-populates depth |
| ProjectCurve needs STL + SVG separately | Single STEP file provides both |

---

## Dependency Graph

```
Phase A: Face polygon injection (THE critical wiring)
    │
    ├───────────────────────┐
    ▼                       ▼
Phase B: Face boundaries   Phase C: BREP edge extraction
(replace stock_bbox rect)  (extract internal edges from CompressedShell)
    │                       │
    ▼                       ▼
Phase D: Auto-depth        Phase E: Edge-driven operations
(face Z → height params)   (pencil, trace, chamfer from BREP edges)
    │                       │
    └───────────┬───────────┘
                ▼
Phase F: Multi-face union + UI polish
```

---

## Phase A: Face Polygon Injection

**Purpose**: When a toolpath has `face_selection` set and the operation needs
polygons, derive them from the selected face boundaries automatically.

**This is the single highest-value change** — it unlocks Pocket, Profile,
Adaptive, VCarve, Rest, Inlay, Zigzag, Trace, Drill, Chamfer for STEP models.

### A.1: Inject face-derived polygons in the controller

File: `crates/rs_cam_viz/src/controller/events.rs` (~line 1025)

Currently:
```rust
let mut polygons = model.and_then(|model| model.polygons.clone());
let mut mesh = model.and_then(|model| model.mesh.clone());
let enriched_mesh = model.and_then(|model| model.enriched_mesh.clone());
let face_selection = face_selection_for_toolpath.clone();
```

Add after this block:
```rust
// If face_selection is set and operation needs polygons, derive from faces
if polygons.is_none() {
    if let (Some(face_ids), Some(enriched)) = (&face_selection, &enriched_mesh) {
        if !face_ids.is_empty() {
            if let Some(poly) = enriched.faces_boundary_as_polygon(face_ids) {
                polygons = Some(Arc::new(vec![poly]));
            }
        }
    }
}
```

This means: STEP model + face selection → 2.5D operations get their polygons
from face boundaries, no SVG/DXF needed.

### A.2: Update error messages in require_polygons

File: `crates/rs_cam_viz/src/compute/worker/helpers.rs`

Change error from `"No 2D geometry (import SVG)"` to
`"No 2D geometry (import SVG/DXF or select STEP faces)"`.

### A.3: Face selection UI on toolpath properties

File: `crates/rs_cam_viz/src/ui/properties/mod.rs`

When a toolpath is selected and its model is STEP:
- Show "Select Faces" button (enters face selection mode)
- Show selected face count + "Clear" button
- Show derived polygon preview (vertex count of extracted boundary)

### A.4: Integration test

Test: STEP cube → select top face → create Pocket → verify polygon extraction
→ verify toolpath generated within face boundary.

### Verification
```
cargo test -p rs_cam_core --features step -- step_import
cargo test -p rs_cam_viz -- face
# Manual: import STEP → create pocket → select face → generate
```

---

## Phase B: Face-Derived Machining Boundaries

**Purpose**: When face_selection is set, use the face boundary as the
containment polygon instead of the stock bounding box rectangle.

### B.1: Replace stock_poly in boundary clipping

File: `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` (~line 343)

Currently:
```rust
let mut stock_poly = Polygon2::rectangle(bbox.min.x, bbox.min.y, bbox.max.x, bbox.max.y);
```

Replace with:
```rust
let mut stock_poly = if let (Some(face_ids), Some(enriched)) =
    (&req.face_selection, &req.enriched_mesh)
{
    enriched.faces_boundary_as_polygon(face_ids)
        .unwrap_or_else(|| Polygon2::rectangle(bbox.min.x, bbox.min.y, bbox.max.x, bbox.max.y))
} else {
    Polygon2::rectangle(bbox.min.x, bbox.min.y, bbox.max.x, bbox.max.y)
};
```

Keep-out subtraction and BoundaryContainment offset continue to work unchanged
on the face-derived polygon.

### B.2: Face boundary for 3D operations

For 3D ops (waterline, adaptive3d, etc.), the face selection constrains the
XY region. The full mesh is still passed (needed for correct Z values), but
the face boundary provides the clipping polygon.

This uses the same code path as B.1 — 3D ops also go through boundary clipping.

### Verification
```
# Manual: STEP → waterline → select face → verify toolpath stays within face boundary
```

---

## Phase C: BREP Edge Extraction

**Purpose**: Extract internal edges from `CompressedShell.edges` that are
currently ignored. These are the feature edges between adjacent faces.

### C.1: Add edge data to EnrichedMesh

File: `crates/rs_cam_core/src/enriched_mesh.rs`

New types:
```rust
pub struct BrepEdge {
    pub id: usize,
    pub face_a: FaceGroupId,
    pub face_b: FaceGroupId,
    pub vertices: Vec<P3>,           // 3D polyline (tessellated edge curve)
    pub vertices_2d: Option<Vec<P2>>, // 2D projection for horizontal edges
    pub is_concave: bool,            // true if faces form a concave crease
    pub dihedral_angle: f64,         // angle between face normals (radians)
}
```

Add to EnrichedMesh:
```rust
pub edges: Vec<BrepEdge>,
```

### C.2: Extract edges during STEP import

File: `crates/rs_cam_core/src/step_input.rs`

After per-face tessellation, extract edges from the adjacency data:

For each pair of adjacent faces:
1. Find shared boundary vertices (vertices that appear in both face tessellations)
2. Chain them into a polyline (these are the edge vertices)
3. Classify concavity from face normal dot product
4. Compute dihedral angle
5. Project to 2D if both adjacent faces are approximately horizontal

This is more precise than the current mesh-based approach in `pencil.rs`
because it uses BREP topology (exact adjacency) instead of triangle edge
scanning (approximate, affected by tessellation).

### C.3: Edge query methods

```rust
impl EnrichedMesh {
    /// Get all concave edges (for pencil-like operations).
    pub fn concave_edges(&self, min_angle: f64) -> Vec<&BrepEdge>;

    /// Get all edges adjacent to a specific face.
    pub fn edges_for_face(&self, face_id: FaceGroupId) -> Vec<&BrepEdge>;

    /// Get edges between two specific faces (e.g., for targeted chamfer).
    pub fn edges_between(&self, a: FaceGroupId, b: FaceGroupId) -> Vec<&BrepEdge>;

    /// Get all edges as 2D polylines (for trace/engrave operations).
    pub fn edge_chains_2d(&self) -> Vec<Vec<P2>>;
}
```

### Verification
```
cargo test -p rs_cam_core --features step -- brep_edge
```

---

## Phase D: Auto-Depth from Face Z Values

**Purpose**: When face_selection is set and selected faces are planar, auto-
populate the operation's depth from the face Z value.

### D.1: Depth suggestion from face selection

File: `crates/rs_cam_viz/src/ui/properties/operations.rs`

When drawing operation params and model has enriched_mesh + face_selection:
- Extract Z from `SurfaceParams::Plane { d }` of selected face
- Show as "Suggested depth: {d} mm" hint text
- Auto-fill depth field if currently at default value

For a pocket on the top face of a box:
- Top face at Z=25mm → suggest depth = stock_top - face_z = stock_top - 25

For a pocket on a recessed face at Z=10mm:
- Depth = stock_top - 10mm → the pocket goes down to Z=10

### D.2: Heights auto-population

When face_selection is set:
- `top_z` = max Z of selected face(s) bbox
- `bottom_z` = min Z of selected face(s) bbox (for planar faces, same as top_z)
- `depth` = stock_top_z - face_z (how deep to cut to reach this face)

### Verification
```
# Manual: STEP → select recessed face → create pocket → verify depth auto-populated
```

---

## Phase E: Edge-Driven Operations

**Purpose**: Use BREP edges for pencil finish, trace, and chamfer operations.

### E.1: BREP-enhanced pencil

File: `crates/rs_cam_core/src/pencil.rs`

Add alternative entry point:
```rust
pub fn pencil_from_brep_edges(
    edges: &[BrepEdge],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &PencilParams,
) -> Toolpath
```

Uses pre-computed concave edge chains instead of re-detecting from mesh.
Falls through to existing mesh-based detection when no BREP edges available.

Wire in the execute path: if `req.enriched_mesh` is available, use
`enriched.concave_edges(params.bitangency_angle)` instead of
`compute_shared_edges() + chain_concave_edges()`.

### E.2: Edge trace operation

Add ability for trace to accept BREP edge chains as input.

Currently trace requires `Polygon2` (closed rings). Two approaches:
a) Extract edge chains as closed loops where possible → feed as Polygon2
b) Add open-path trace support (follow an edge chain without closing)

For (b), add to `trace.rs`:
```rust
pub fn trace_open_path(path: &[P2], params: &TraceParams) -> Toolpath
```

This follows the path without closing it. The path comes from
`BrepEdge.vertices_2d` projected to 2D.

### E.3: Edge-targeted chamfer

When chamfer operation has face_selection:
- Extract edges adjacent to selected faces via `edges_for_face()`
- Filter to edges above a dihedral angle threshold
- Chamfer only those edges (not all polygon edges)

### Verification
```
cargo test -p rs_cam_core -- pencil_brep
# Manual: STEP with creases → pencil → verify edges traced precisely
```

---

## Phase F: Multi-Face Union + UI Polish

### F.1: Fix multi-face polygon union

File: `crates/rs_cam_core/src/enriched_mesh.rs`

Replace the current broken implementation of `faces_boundary_as_polygon()`
(which concatenates points) with proper boolean union via the `geo` crate:

```rust
pub fn faces_boundary_as_polygon(&self, ids: &[FaceGroupId]) -> Option<Polygon2> {
    let polys: Vec<geo::Polygon> = ids.iter()
        .filter_map(|id| self.face_boundary_as_polygon(*id))
        .map(|p| p.to_geo())
        .collect();
    if polys.is_empty() { return None; }
    // Use geo::BooleanOps for union
    let union = polys.into_iter()
        .reduce(|a, b| a.union(&b))
        .map(Polygon2::from_geo)?;
    Some(union)
}
```

### F.2: Face boundary rendering in viewport

When faces are selected, render their 2D boundary loops as colored outlines
in the viewport (like SVG/DXF polygon rendering). Uses the existing line
pipeline with face boundary vertices → `LineVertex` array.

### F.3: Operation menu gating

When model is STEP with face_selection:
- Show polygon-requiring operations as available (they can derive from faces)
- When model is STL (no enriched_mesh): gray out polygon operations
- When model is SVG/DXF: gray out mesh operations

File: `crates/rs_cam_viz/src/ui/toolpath_panel.rs` — filter the operation type
dropdown based on available geometry.

### F.4: STEP metadata in properties (enhance)

Show face-level info when a face is selected:
- Surface type + equation parameters
- Z value for planar faces
- Adjacent face count
- Boundary loop vertex count
- "Can derive polygon: Yes/No" indicator

### Verification
```
cargo test -p rs_cam_core -- faces_boundary
# Manual: shift+click 2 adjacent faces → verify union polygon is correct
# Manual: verify operation menu shows/hides based on geometry
```

---

## Critical Files

| Purpose | File |
|---------|------|
| Polygon injection + enriched_mesh/face_selection wiring | `controller/events.rs` (~line 1025) |
| Boundary clipping face integration | `compute/worker/execute/mod.rs` (~line 343) |
| Face polygon extraction | `enriched_mesh.rs` (face_boundary_as_polygon) |
| BREP edge types + extraction | `enriched_mesh.rs` (new BrepEdge) |
| STEP import edge extraction | `step_input.rs` (new edge extraction) |
| Pencil BREP edge entry point | `pencil.rs` (new function) |
| Trace open path support | `trace.rs` (new function) |
| Auto-depth UI | `ui/properties/operations.rs` |
| Operation menu gating | `ui/toolpath_panel.rs` |
| Multi-face union | `enriched_mesh.rs` (fix faces_boundary_as_polygon) |
| require_polygons error message | `compute/worker/helpers.rs` |
| Face selection UI on toolpath | `ui/properties/mod.rs` |

---

## Parallelization

```
Sequential:  Phase A → Phase B (B uses polygon injection from A)
Parallel:    Phase C ║ Phase D    (after A, independent of each other)
Sequential:  Phase C → Phase E   (E needs edges from C)
After all:   Phase F             (polish, depends on A+B+C)
```

Phase A is the critical path — everything else builds on it.
