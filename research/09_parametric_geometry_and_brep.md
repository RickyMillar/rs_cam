# Parametric Geometry & Face-Aware CAM: Reference Document

## Context

rs_cam today imports STL (mesh), SVG, and DXF (2D vectors). All 3D operations work on
`TriangleMesh` — flat indexed triangles with no notion of which triangles belong to which
design surface. This means a user can't click a face and say "contour this" or "pocket
within these bounds." Every professional CAM tool supports this. Understanding how they do
it — and what the viable paths are for rs_cam — requires understanding BREP, geometric
kernels, interchange formats, and the Rust ecosystem for all of the above.

---

## 1. Why Mesh Isn't Enough for CAM

### What CAM operations need that mesh doesn't provide

| Capability | Mesh (STL) | BREP |
|---|---|---|
| Face identity ("select this face") | No — just triangles | Yes — topological faces with IDs |
| Surface type classification (planar, cylindrical, freeform) | Must infer from normals | Stored explicitly per face |
| Exact surface evaluation (point + normal at any UV) | Interpolate triangle normals | Exact parametric equation |
| Edge detection (sharp edges, fillets, chamfers) | Approximate from dihedral angles | Exact edge curves |
| Containment regions ("machine within this boundary") | Manual polygon drawing | Select face loops → automatic boundary |
| Adjacent face queries ("everything connected to this") | Build adjacency ad-hoc | Stored in topology |

**rs_cam already does ad-hoc edge detection** in `pencil.rs` — computing dihedral angles between
adjacent triangles to find concave creases. This works, but it's fragile: it detects mesh artifacts
as features and misses shallow transitions.

### What "select a face and machine it" actually requires

In Mastercam/Fusion 360, when you click a face:
1. The kernel knows that face's **surface type** (plane, cylinder, NURBS, etc.)
2. It knows the face's **boundary loops** (outer edges + inner holes)
3. It knows **adjacent faces** and their types
4. It can evaluate the **exact surface equation** at any point

For a "contour" operation on a face, the CAM engine:
- Gets the face boundary as curves
- Offsets those curves by the tool radius
- Projects onto the surface to get Z values
- Generates toolpath following the offset boundary at correct Z

For "pocket within bounds," it:
- Gets the selected face(s) as a containment region
- Projects to 2D → pocket boundary
- Runs 2.5D pocket strategy within that boundary

---

## 2. BREP: What It Is and How It Works

### Boundary Representation

BREP stores geometry as a **topological graph** of:
- **Vertices** — 3D points
- **Edges** — curves (lines, arcs, NURBS, B-splines) bounded by vertices
- **Wires/Loops** — ordered sequences of edges forming closed boundaries
- **Faces** — surfaces (planes, cylinders, cones, spheres, tori, NURBS) bounded by wires
- **Shells** — connected sets of faces
- **Solids** — closed shells enclosing volume

Each face stores:
- The **surface equation** (parametric: maps (u,v) → (x,y,z))
- One or more **boundary wires** (outer loop + hole loops)
- **Orientation** (which side is "out")
- A **unique ID** stable across operations

### BREP vs mesh: the fundamental difference

Mesh is a **discrete approximation**. A cylinder in mesh is 36 flat strips. In BREP, it's one
face with an exact cylindrical surface equation. The CAM engine can evaluate surface normals
at arbitrary resolution, detect the surface is cylindrical (not just "approximately round"),
and use that knowledge for optimal toolpath strategy.

---

## 3. The STEP File Format

### What's in a STEP file

STEP (ISO 10303) encodes geometry as **EXPRESS entities** in a text file. A typical STEP file
for a machined part contains:

```
#1 = PRODUCT('Widget', 'Widget', '', (#2));
...
#47 = ADVANCED_BREP_SHAPE_REPRESENTATION('', (#48), #200);
#48 = MANIFOLD_SOLID_BREP('', #49);
#49 = CLOSED_SHELL('', (#50, #51, #52, ...));
#50 = ADVANCED_FACE('', (#60), #70, .T.);   ← a face
#60 = FACE_BOUND('', #61, .T.);             ← its boundary loop
#61 = EDGE_LOOP('', (#62, #63, #64, #65));  ← edges forming the loop
#70 = PLANE('', #71);                       ← the surface geometry
```

Every face, edge, and vertex has a unique `#id`. The topology (which faces share which edges)
is explicit. Surface equations are exact.

### Application Protocols

| Protocol | What it carries | Status |
|---|---|---|
| AP203 | Geometry + topology only | Formally withdrawn |
| AP214 | + colors, layers, GD&T | Formally withdrawn |
| AP242 | + tessellation, full PMI, supersedes both | Current standard |

**AP242** is significant because it can carry **both BREP and tessellation** in the same file.
A reader that only handles mesh can extract the tessellation; a full kernel can use the BREP.

### STEP vs IGES

IGES is the older interchange format. Key difference: IGES carries **surfaces** but not
**solid topology**. A face in IGES is just a trimmed surface — there's no guarantee that
faces stitch together into a watertight solid. This causes "healing" problems on import.
STEP was designed specifically to fix this. STEP always wins for CAM interchange.

---

## 4. Geometric Kernels: Why They Matter

A **geometric kernel** is the software library that creates, modifies, queries, and tessellates
BREP geometry. It's the foundation that CAD and CAM software is built on.

### The three commercial kernels

| Kernel | Owner | Used by |
|---|---|---|
| **Parasolid** | Siemens | SolidWorks, Mastercam, Siemens NX, Solid Edge |
| **ACIS** | Dassault (Spatial) | AutoCAD 3D, SpaceClaim |
| **ASM** (ACIS derivative) | Autodesk | Fusion 360, Inventor |

### What a kernel provides to CAM software

1. **STEP/IGES import** — parse file → build internal BREP
2. **Face enumeration** — iterate all faces, get surface type, equation, boundary
3. **Surface evaluation** — given face + (u,v) → exact (x,y,z) + normal + curvature
4. **Tessellation** — BREP → mesh, preserving face identity (each face → a group of triangles)
5. **Boolean operations** — union, intersection, subtraction
6. **Topology queries** — adjacent faces, shared edges, connected shells

**The key insight**: When Mastercam imports a STEP file, it's using Parasolid to build a full
BREP model. When you click a face, Parasolid tells Mastercam what that face is. The CAM
algorithms work against the kernel's exact geometry.

### OpenCascade (OCCT): the open-source kernel

OCCT is the only production-grade open-source geometric kernel. It's what FreeCAD, KiCAD,
and many commercial tools use under the hood.

**What it provides:**
- Full STEP and IGES import/export (AP203, AP214, AP242)
- Complete BREP with face/edge/wire/shell/solid topology
- Surface type classification via `GeomAbs_SurfaceType`:
  `Plane | Cylinder | Cone | Sphere | Torus | BezierSurface | BSplineSurface | ...`
- Per-face tessellation via `BRepMesh_IncrementalMesh` — each face gets its own
  `Poly_Triangulation` object, so **face identity is preserved through tessellation**
- Boolean operations, fillets, chamfers, offsetting
- Written in C++, ~7 million lines, LGPL 2.1 licensed

**This is how FreeCAD's Path workbench does face selection for CAM**: OCCT reads the STEP,
builds BREP, the user clicks a face in the 3D view, FreeCAD resolves the click to a
topological face via OCCT, and the CAM operation gets that face's geometry.

---

## 5. How People Go from Fusion 360 to Mastercam

### The standard interchange path

```
Fusion 360 (ASM kernel) → Export STEP AP214/AP242 → Mastercam (Parasolid kernel)
                                                      ↓
                                               Parasolid reads STEP
                                               Rebuilds BREP internally
                                               Face IDs are new (not from Fusion)
                                               But topology is preserved
```

When Mastercam imports the STEP:
- Every face gets a **new** Parasolid face ID (not the same as Fusion's internal ID)
- But the **topology** is preserved: if two faces shared an edge in Fusion, they still share
  an edge in Mastercam
- Surface types are preserved: a plane stays a plane, a cylinder stays a cylinder
- The user can click any face and get its exact geometry

### Why integrated CAM wins

Fusion 360 Manufacturing (built-in CAM) has an advantage: it uses the **same ASM kernel**
for both CAD and CAM. No STEP export/import cycle. Face IDs are stable. Surface equations
are shared. This is why Fusion's face selection "just works" — there's no conversion.

### What gets lost in STEP interchange

- **Feature history** (extrude, fillet, etc.) — STEP only carries the final BREP
- **Constraints and parameters** — STEP is dumb geometry, not a parametric model
- **Assembly mates** (partially preserved in AP242)
- **Face naming** — each kernel assigns its own IDs; "Face 7" in Fusion ≠ "Face 7" in Mastercam
- **Thread data, surface finish annotations** — partially in AP242 PMI, but spotty support

### The F3D format

Fusion 360's native `.f3d` is a ZIP containing:
- ASM kernel data (proprietary binary BREP)
- Mesh cache (for display)
- Feature history (parametric timeline)
- Metadata

It's **not extractable** without Autodesk's kernel. You can't read BREP from an F3D outside
of Fusion. This is why STEP is the universal interchange format.

---

## 6. The Mesh + Parametric Hybrid Question

### Can one file carry both mesh and parametric data?

| Format | BREP | Mesh | Both in one file? |
|---|---|---|---|
| STEP AP242 | Yes | Yes (tessellation facet) | Yes, but reader support is incomplete |
| 3MF | No | Yes + per-face materials | Mesh only, but with face-level metadata |
| OBJ | No | Yes + face groups (`g`) | Group-annotated mesh |
| STL | No | Yes | No metadata at all |
| glTF | No | Yes + mesh primitives per material | Group-annotated mesh |

### The practical hybrid: "enriched mesh"

The approach that actually works for CAM on enriched mesh:

1. **Import STEP** via a BREP kernel (OCCT, truck)
2. **Tessellate per-face** — each BREP face → a group of mesh triangles
3. **Annotate** each group with metadata:
   - Face ID (stable within this import)
   - Surface type (plane, cylinder, freeform, etc.)
   - Surface equation parameters (plane normal + d, cylinder axis + radius, etc.)
   - Bounding box
   - Adjacent face IDs
4. **Export** as OBJ-with-groups, 3MF, or a custom format
5. **CAM engine** works on the annotated mesh — face selection, containment, strategy choice

This is exactly what `occt-import-js` (github.com/kovacsv/occt-import-js) does:
it outputs JSON with a `brep_faces` array mapping triangle ranges to original BREP face IDs.

### What this means for rs_cam

You don't need a full BREP kernel at runtime. You need:
1. A **converter** (at import time) that reads STEP → enriched mesh
2. An **enriched mesh data structure** in rs_cam that carries face groups + metadata
3. **UI** for face selection (click → identify face group → highlight)
4. **Operations** that accept face groups as containment/selection

The BREP kernel only runs at import time. The CAM engine works on enriched mesh.

---

## 7. The Rust Ecosystem: What Exists Today

### Pure Rust: `truck` (ricosjp/truck)

The most mature pure-Rust CAD kernel.

- **1,400 stars**, actively maintained (last activity: March 2026)
- **11 sub-crates**: base, geotrait, geometry, topology, polymesh, meshalgo, modeling, shapeops, stepio, platform, rendimpl
- **Full BREP topology**: Vertex, Edge, Wire, Face, Shell, Solid
- **STEP I/O**: Import supported since v0.6 via `truck-stepio` — parses B-splines, NURBS, elementary geometries, shells, solids. Assembly reading in progress.
- **Tessellation**: `truck-meshalgo` does adaptive surface tessellation. **Each Face is tessellated independently** — face identity is preserved.
- **Geometry**: B-spline/NURBS curves and surfaces, lines, circles, planes, spheres. Parametric evaluation + derivatives.
- **Gap vs OCCT**: Smaller surface type vocabulary, less battle-tested edge cases, experimental STEP import (not all entity types). No boolean operations in mainline (available in `monstertruck` fork).
- **License**: Apache 2.0

**This could be the pure-Rust path**: `truck-stepio` reads STEP → truck BREP → per-face tessellation → enriched mesh for rs_cam.

### Rust + C++ FFI: `opencascade-rs`

- **227 stars**, wraps OCCT via cxx.rs
- `opencascade-sys` provides low-level bindings; `opencascade` provides higher-level API
- **Can do everything OCCT does**: STEP import, face enumeration, per-face tessellation, surface type queries
- **Build cost**: Links against full OCCT C++ library. First build: 20-30 minutes compiling OCCT from source. Requires CMake + C++ compiler.
- **Maintenance concern**: `opencascade-sys` last updated August 2023; hobby project by one maintainer
- **Verdict**: Industrial-strength geometry handling, but C++ dependency and maintenance risk

### Other Rust crates

| Crate | What | Status |
|---|---|---|
| `fornjot` | Rust CAD kernel | 2,500 stars but **stalled for 1+ year**. No STEP import. Not usable. |
| `vcad` | Rust BREP kernel | 304 stars, working `vcad import-step`, but it's an app not a library |
| `ruststep` | STEP parser (used by truck) | "DO NOT USE FOR PRODUCT" — experimental |
| `iso-10303` | STEP schema codegen | 36 stars, low activity |
| `foxtrot` (Formlabs) | STEP viewer | AP214 proof-of-concept, 26 of 915 entity types. Abandoned. |

### STEP parsing maturity (honest assessment)

**No production-quality pure-Rust STEP parser exists.** The real-world STEP files from Fusion 360,
SolidWorks, etc. use hundreds of EXPRESS entity types. Current Rust parsers handle a fraction.
The `truck-stepio` parser is the most complete but still experimental.

### Mesh libraries with face group support

| Crate | Groups? | Notes |
|---|---|---|
| `tobj` (OBJ) | Yes — each `g` directive → separate `Model` | Most popular OBJ crate (267 stars) |
| `obj-rs` (OBJ) | Yes — explicit `Group` struct | More faithful to OBJ spec |
| `lib3mf-rs` (3MF) | Yes — per-face materials, all 9 extensions | Pure Rust, 90%+ test coverage |

---

## 8. How FreeCAD Does It (The Path Workbench)

FreeCAD's CAM module (Path workbench) is the closest open-source analogue to what rs_cam
wants to become. Here's how face selection works:

1. **Import**: OCCT reads STEP → builds internal BREP (Part::Feature)
2. **Display**: OCCT tessellates for the 3D viewer (Coin3D/OpenGL)
3. **User clicks face**: The 3D viewer resolves the click to a triangle → maps to BREP face via OCCT's `TopExp_Explorer`
4. **Operation creation**: User picks "Profile" or "Pocket" → selects face(s) as the Base Geometry
5. **CAM engine**: Gets the face boundary via OCCT → projects to 2D → runs toolpath algorithm
6. **Face reference**: Stored as `Face6` (index into the shape's topology) in the operation definition

**Key lesson**: FreeCAD's CAM is completely dependent on OCCT for face resolution. Without a
BREP kernel, face selection doesn't work. The "click a face" workflow fundamentally requires
mapping screen coordinates → mesh triangle → BREP face → surface equation.

### Using FreeCAD as a subprocess

FreeCAD can be driven headlessly:
```python
# freecad_convert.py (run via FreeCADCmd)
import FreeCAD, Part, MeshPart
shape = Part.Shape()
shape.read("input.step")
for i, face in enumerate(shape.Faces):
    mesh = MeshPart.meshFromShape(face, MaxLength=0.5)
    # Write as OBJ group or custom format
```

**Pros**: Battle-tested STEP handling, full OCCT power
**Cons**: 500MB+ dependency, Python subprocess, seconds of latency

---

## 9. Integration Paths for rs_cam

### Path A: Pure Rust via `truck`

```
STEP file → truck-stepio → truck BREP → per-face tessellation → EnrichedMesh
```

- **Pros**: Pure Rust, no C++ dependency, Apache 2.0, same build system
- **Cons**: Experimental STEP import, limited entity coverage, may fail on real-world files
- **Risk**: Real STEP files from Fusion 360/SolidWorks may use entity types truck doesn't handle
- **Mitigation**: Test against a corpus of real files; fall back to Path B/C for failures

### Path B: OCCT via Rust FFI (`opencascade-rs`)

```
STEP file → opencascade-sys → OCCT BREP → per-face tessellation → EnrichedMesh
```

- **Pros**: Industrial-strength STEP handling, complete entity coverage
- **Cons**: C++ build dependency (CMake, 20-30 min first build), hobby-maintained bindings, LGPL license considerations
- **Risk**: Binding maintenance; OCCT version drift
- **Mitigation**: Write minimal custom bindings targeting only the APIs needed (STEPControl_Reader, BRepMesh, TopExp_Explorer, BRep_Tool)

### Path C: Subprocess via FreeCAD or build123d

```
STEP file → [FreeCAD | build123d] subprocess → OBJ-with-groups → tobj → EnrichedMesh
```

- **Pros**: Zero Rust compilation complexity, battle-tested, can leverage all of OCCT
- **Cons**: External dependency (FreeCAD ~500MB or Python + build123d), subprocess latency, harder to distribute
- **Risk**: User must install FreeCAD/Python separately
- **Mitigation**: Make it optional; detect at runtime; document setup

### Path D: Hybrid (recommended investigation order)

1. **Start with `truck`** — try pure Rust first. Test against real STEP files from Fusion 360, SolidWorks, FreeCAD. See what works and what breaks.
2. **Fall back to subprocess** for files truck can't handle. FreeCAD or build123d can convert STEP → OBJ-with-groups as a preprocessing step.
3. **If truck proves insufficient** for the majority of real files, evaluate opencascade-rs or custom minimal OCCT bindings.

---

## 10. What rs_cam Would Need to Build

Regardless of which STEP import path is chosen, rs_cam needs these new concepts:

### New data structure: `EnrichedMesh`

```
EnrichedMesh {
    vertices: Vec<P3>,
    triangles: Vec<[u32; 3]>,
    face_groups: Vec<FaceGroup>,       // which triangles belong to which face
    adjacency: Vec<(FaceId, FaceId)>,  // which faces share edges
}

FaceGroup {
    id: FaceId,
    triangle_range: Range<usize>,      // indices into triangles array
    surface_type: SurfaceType,         // Plane, Cylinder, Cone, Sphere, Torus, Freeform
    surface_params: SurfaceParams,     // normal+d for plane, axis+radius for cylinder, etc.
    bbox: BoundingBox3,
    boundary_loops: Vec<Vec<P3>>,      // outer + hole boundaries as polylines
}

enum SurfaceType {
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
    BSpline,
    Unknown,
}
```

### New UI concepts

- **Face picking**: Click in 3D view → ray-triangle intersection → look up `face_groups[triangle_to_face[tri_idx]]`
- **Face highlighting**: Render selected face group's triangles in a different color
- **Face-based operation binding**: "Profile this face" → get face boundary → run profile operation

### New operation inputs

Operations that currently take `BoundingBox3` as containment could accept `FaceId` or `Vec<FaceId>` instead, extracting the containment boundary from the face group's boundary loops.

### New `ModelKind`

```rust
enum ModelKind {
    Stl,        // existing
    Svg,        // existing
    Dxf,        // existing
    Step,       // new — carries EnrichedMesh
}
```

---

## 11. What Other Open-Source CAM Projects Do

| Project | Kernel | Face selection? | STEP import? |
|---|---|---|---|
| **FreeCAD Path** | OCCT (C++) | Yes — full BREP | Yes — via OCCT |
| **Kiri:Moto** | None (mesh only) | No | No (STL/OBJ only) |
| **PyCAM** | None (mesh only) | No | No (STL only) |
| **dCAM** | None (mesh only) | No | No |
| **OpenCAMLib** | None (mesh only) | No | No (STL only) |
| **HeeksCAM** | OCCT (C++) | Yes — face picking | Yes — via OCCT |

**Pattern**: Every open-source CAM tool that supports face selection uses OCCT. Every mesh-only
CAM tool doesn't support face selection. There are no exceptions in the open-source world.

---

## 12. The 3MF Angle: Per-Triangle Metadata Without BREP

PrusaSlicer's "face painting" (multi-material, supports, seam) uses 3MF with per-triangle
material assignments. This proves that per-triangle metadata is useful even without full BREP.

`lib3mf-rs` (pure Rust) can read these files. This could be a simpler stepping stone:
- Import 3MF with per-face materials → treat material groups as "faces"
- User manually paints faces in external tool (Bambu Studio, PrusaSlicer) → import as groups
- Not as powerful as STEP-derived face groups, but zero kernel dependency

---

## 13. Key Decisions Ahead

1. **Pure Rust vs OCCT dependency**: truck is promising but unproven on real files. OCCT is proven but brings C++ complexity.

2. **Import-time conversion vs runtime kernel**: The enriched mesh approach (convert at import, CAM works on annotated mesh) is simpler and keeps the core library free of kernel dependencies. The alternative (keep BREP alive at runtime for exact surface evaluation) is more powerful but dramatically more complex.

3. **Which interchange format for enriched mesh**: OBJ-with-groups (via tobj), 3MF (via lib3mf-rs), or a custom binary format? OBJ is simplest but loses surface type metadata. A custom format preserves everything.

4. **Face selection UX**: Needs ray-triangle picking (already used for some debug features?) mapped to face groups. The 3D renderer needs per-face-group coloring.

5. **Scope of face-aware operations**: Start with 2.5D operations (profile, pocket, face) that only need the face boundary projected to 2D? Or go straight to 3D operations that use surface equations?

---

## Sources

### Standards & Specifications
- ISO 10303 (STEP) — iso.org
- AP203/AP214/AP242 comparison — steptools.com, cax-if.org

### Software & Kernels
- OpenCascade documentation — dev.opencascade.org
- Parasolid — siemens.com/plm/parasolid
- ACIS/Spatial — spatial.com

### Rust Ecosystem
- truck: github.com/ricosjp/truck (1,400 stars, Apache 2.0)
- opencascade-rs: github.com/bschwind/opencascade-rs (227 stars)
- fornjot: github.com/hannobraun/fornjot (2,500 stars, stalled)
- vcad: github.com/ecto/vcad (304 stars)
- ruststep: github.com/ricosjp/ruststep (169 stars)
- foxtrot: github.com/Formlabs/foxtrot (324 stars, abandoned)
- tobj: github.com/Twinklebear/tobj (267 stars)
- lib3mf-rs: github.com/sscargal/lib3mf-rs (6 stars)
- occt-import-js: github.com/kovacsv/occt-import-js

### Open-Source CAM
- FreeCAD Path workbench — wiki.freecad.org/Path_Workbench
- HeeksCAM — github.com/danheeks/HeeksCAM
- OpenCAMLib — github.com/aewallin/opencamlib
- Kiri:Moto — grid.space/kiri
