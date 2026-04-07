# BREP/STEP Follow-up: Workflow Alignment & Feature Opportunities

Post-implementation audit of the feature/brep-step-support branch.

## Audit Fixes Applied

These gaps were found and fixed in commit `7512cdd`:

1. **Render pipeline was disconnected** — `enriched_mesh_gpu_data()` was dead code. Now wired:
   STEP models render with per-face pastel colors via `sim_mesh_pipeline`
2. **Reload missed enriched_mesh** — `reload_model()` now copies `enriched_mesh`
3. **BREP metadata display** — properties panel shows face count, adjacency, surface types

## Remaining Feature Opportunities

### Tier 1: Quick wins (low effort, high alignment value)

**1.1 Face-derived polygons for 2.5D operations**
- 13 operations (Pocket, Profile, Adaptive, VCarve, Trace, etc.) require `Polygon2` from SVG/DXF
- `enriched_mesh.face_boundary_as_polygon()` already exists and works for horizontal planar faces
- **Gap**: No UI to trigger "use this face as pocket boundary" — face selection is view-only
- **Fix**: When face is selected on a STEP model, offer polygon-requiring operations in context menu.
  In the compute pipeline, if `face_selection.is_some()`, call `faces_boundary_as_polygon()`
  to populate `req.polygons` automatically. The code path in `events.rs` is partially wired
  but needs the actual polygon injection.

**1.2 BREP metadata in properties**
- Done in audit fix.

**1.3 Multi-face polygon union (fix TODO)**
- `faces_boundary_as_polygon()` currently concatenates all exterior points (incorrect)
- Should use `geo` crate's boolean union for proper multi-face boundary merging
- Impacts shift+click multi-face selection → pocket/profile workflow

### Tier 2: Workflow alignment (medium effort)

**2.1 ProjectCurve with single STEP file**
- ProjectCurve requires Both (Mesh + Polygons) — currently needs separate STL + SVG
- STEP can provide both: mesh from tessellation + polygons from face boundaries
- One STEP file replaces two imports for this workflow

**2.2 Face boundary overlay rendering**
- SVG/DXF polygons render as 2D outlines in the viewport
- When a STEP face is selected, its 2D boundary should render similarly (green outline)
- Uses the existing line_pipeline, just needs boundary loop → LineVertex conversion

**2.3 Operation availability filtering**
- Currently all 22 operations show in the menu regardless of loaded geometry
- Should gray out / hide operations that can't work with the selected model's geometry
- e.g., hide Pocket when only STL is loaded (no polygons), show it when STEP face is selected

**2.4 Face-based stock auto-sizing**
- Find the top-most horizontal face → use its XY bbox for stock width/length
- Use mesh Z-span for stock height
- "Auto Size from Top Face" button in stock properties

### Tier 3: New import formats (EnrichedMesh makes these easy)

**3.1 Auto face-group detection for STL**
- Cluster triangles by normal similarity + dihedral angle threshold
- Produces FaceGroup-like structure without BREP import
- STL gains face selection, face-derived polygons for trace operations
- Algorithm: BFS flood-fill from seed triangles, split at edges where dihedral angle > threshold

**3.2 OBJ import with face groups**
- OBJ natively supports `g` (group) and `usemtl` (material) directives
- `tobj` crate maps groups → separate meshes with names
- Each group → FaceGroup → EnrichedMesh
- Users with Blender/Maya exports get face semantics for free

**3.3 3MF import with per-face materials**
- `lib3mf-rs` (pure Rust) supports all 3MF extensions
- Per-triangle material assignments → face group clustering
- PrusaSlicer "face painting" files could map colors → operation regions

### Tier 4: Advanced features (built on EnrichedMesh foundation)

**4.1 Automatic operation suggestions**
- When selecting a face, analyze surface type:
  - Plane → suggest Pocket, Profile, Face
  - Cylinder → suggest 3D finishing, Waterline
  - Freeform → suggest Scallop, Pencil
- Show as "Recommended" section in face properties panel

**4.2 Face-specific tolerances**
- Different faces could have different scallop height / step-over requirements
- Plane faces: coarser finishing (already flat)
- Freeform: finer finishing (curvature matters)

**4.3 Rest machining by face**
- "Machine only the faces the previous tool couldn't reach"
- Face adjacency graph + tool geometry → automatic rest region detection

## Current Workflow Matrix

| Workflow | STL | SVG | DXF | STEP | Status |
|----------|-----|-----|-----|------|--------|
| 3D finishing (drop cutter, waterline, etc.) | Yes | — | — | Yes | Working |
| 2.5D operations (pocket, profile, etc.) | — | Yes | Yes | Partial | Need face→polygon wiring |
| Face selection & highlighting | — | — | — | Yes | Working (viewport colored) |
| ProjectCurve (mesh + curves) | Need SVG too | Need STL too | Need STL too | Can self-provide | Need wiring |
| Stock auto-sizing | Manual | Manual | Manual | Manual | Opportunity |
| Operation suggestions | — | — | — | — | Opportunity |
