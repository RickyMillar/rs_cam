# Review: Import Flow (End-to-End)

## Scope
The complete user flow from "Import file" to "model visible in viewport" for all 3 formats.

## What to review

### STL import flow
1. File → Import STL → file dialog → path
2. `import_stl()` → stl_io parse → TriangleMesh
3. Winding check → winding report
4. Unit scaling (mm/inch/m/cm)
5. Stock auto-size update
6. LoadedModel creation → job.models
7. Selection update → properties panel
8. GPU upload → viewport display

### SVG import flow
Same structure but: usvg parse → path flattening → Vec<Polygon2>

### DXF import flow
Same structure but: dxf parse → entity extraction → Vec<Polygon2>

### Cross-cutting concerns
- Error messages: are they user-friendly or technical?
- Can user cancel mid-import?
- Re-import (update model after file changed on disk)?
- Multiple models: how does the project tree handle many models?
- Model deletion: does it clean up references in toolpaths?

### Gaps
- No STEP/IGES import — is this documented as out of scope?
- No mesh repair — is this needed for wood routing STLs?

## Output
Write findings to `review/results/35_flow_import.md`.
