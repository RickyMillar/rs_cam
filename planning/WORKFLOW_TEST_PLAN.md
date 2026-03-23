# Workflow Integration Test Plan

Invariants derived from code-level workflow tracing. Each test exercises a
user workflow through `AppController` and asserts "what should now be true"
at every step.

## Priority key

- **P0**: Guards against data corruption or silent wrong output
- **P1**: Guards against broken user workflows
- **P2**: Regression coverage for known-working paths

---

## W1. STL Import → 3D Operation → Export (P1)

### Steps and invariants

1. **Import STL**
   - `job.models.len() == 1`
   - `model.mesh.is_some()`, `model.polygons.is_none()`, `model.enriched_mesh.is_none()`
   - `model.kind == ModelKind::Stl`
   - `stock.x/y/z >= mesh.bbox` when `auto_from_model == true`
   - `selection == Selection::Model(model.id)`
   - `job.dirty == true`

2. **Add tool**
   - `job.tools.len() == 1`
   - `tool.diameter > 0`, `tool.tool_number == 1`
   - `selection == Selection::Tool(tool.id)`

3. **Add 3D toolpath (DropCutter)**
   - `setup.toolpaths.len() == 1`
   - `tp.model_id == model.id`, `tp.tool_id == tool.id`
   - `tp.status == ComputeStatus::Pending`
   - `tp.heights` all `HeightMode::Auto`

4. **Generate**
   - `tp.status == ComputeStatus::Computing` immediately after submission
   - After completion: `tp.status == ComputeStatus::Done`, `tp.result.is_some()`
   - Result moves are non-empty
   - All move Z values respect resolved `top_z` and `bottom_z`
   - All rapid Z values >= `retract_z`

5. **Export G-code**
   - Output file is valid UTF-8
   - Contains G-code header from post-processor
   - Contains tool change (if tool_number set)
   - No Z values below resolved `bottom_z`

---

## W2. SVG Import → 2.5D Pocket → Depth Verification (P1)

### Steps and invariants

1. **Import SVG**
   - `model.polygons.is_some()`, `model.mesh.is_none()`
   - `model.kind == ModelKind::Svg`

2. **Add pocket toolpath**
   - `tp.operation == OperationConfig::Pocket(_)`
   - Validation passes (polygons available, tool assigned)

3. **Set depth = 6mm, depth_per_pass = 2mm**

4. **Generate**
   - Result has 3 depth passes (at -2, -4, -6)
   - All XY cutting moves lie within or near the polygon boundary
   - Stepover <= tool diameter

---

## W3. STEP Import → Face Selection → Pocket at Face Z (P0)

### Steps and invariants

1. **Import STEP**
   - `model.enriched_mesh.is_some()`
   - `model.mesh.is_some()` — shares same `Arc` as `enriched_mesh.mesh`
   - `model.kind == ModelKind::Step`
   - `enriched.face_groups.len() > 0`
   - `enriched.triangle_to_face.len() == mesh.triangles.len()`

2. **Add pocket toolpath**

3. **Toggle face selection on a horizontal face at known Z**
   - `tp.face_selection == Some(vec![face_id])`
   - `selection == Selection::Toolpath(tp.id)` (stays on toolpath, not Face)
   - `tp.stale_since.is_some()`

4. **Generate**
   - Polygon derived from face boundary (not stock bbox)
   - Resolved `top_z` == face Z (not 0.0)
   - Resolved `bottom_z` == face Z - op_depth
   - Toolpath moves within face polygon boundary

5. **Toggle face off**
   - `tp.face_selection == None`

6. **Toggle non-horizontal face**
   - Polygon derivation returns None
   - Status message set warning about non-horizontal

---

## W4. Face Selection Undo/Redo (P1)

### Steps and invariants

1. **Import STEP, add toolpath, select toolpath**
2. **Toggle face A on**
   - `face_selection == Some([A])`
3. **Toggle face B on**
   - `face_selection == Some([A, B])`
4. **Deselect toolpath (triggers undo snapshot flush)**
5. **Undo**
   - `face_selection` reverts to state before step 2 (None)
6. **Redo**
   - `face_selection == Some([A, B])`

---

## W5. Project Save/Load Round-Trip (P0)

### Steps and invariants

1. **Build project**: import STL + STEP, add 2 tools, add 3 toolpaths
   (one with face_selection, one with manual heights, one with dressups)

2. **Save**
   - File exists on disk
   - `job.dirty == false`

3. **Load saved file**
   - `job.models.len() == 2`, both re-imported from disk
   - `job.tools.len() == 2`, params match saved
   - All 3 toolpaths restored with correct params
   - Face selection restored (valid IDs against enriched mesh)
   - Manual heights preserved
   - Dressup config preserved
   - `job.dirty == false`
   - `selection == Selection::None`

4. **Load with modified STEP file (fewer faces)**
   - `FaceSelectionStale` warning generated
   - Face selection cleared on affected toolpath

---

## W6. Height System Invariants (P0)

### Steps and invariants

1. **Auto heights with safe_z = 10.0, op_depth = 5.0**
   - `clearance_z == 20.0` (retract + 10)
   - `retract_z == 10.0` (safe_z)
   - `feed_z == 8.0` (retract - 2)
   - `top_z == 0.0`
   - `bottom_z == -5.0` (-op_depth)
   - Ordering: clearance > retract > feed > top > bottom

2. **Auto heights with face_top_z = 15.0**
   - `top_z == 15.0`
   - `bottom_z == 10.0` (15 - op_depth)

3. **Manual override: set top_z = 3.0**
   - face_top_z ignored (manual takes precedence)
   - `top_z == 3.0`

4. **Change safe_z → auto heights shift accordingly**
   - Retract, clearance, feed all shift
   - Manual values don't change

---

## W7. Multi-Setup Simulation (P1)

### Steps and invariants

1. **Create two setups: Top and Bottom**
2. **Add toolpaths to each**
3. **Generate all**
4. **Run simulation**
   - Simulation processes both setups
   - Setup 2 starts from Setup 1's modified stock
   - Through-holes (empty dexel rays) persist across setups
   - Checkpoints exist at each toolpath boundary

---

## W8. Tool Change → Toolpath Staleness (P2)

### Steps and invariants

1. **Add tool (6mm), add toolpath, generate**
2. **Change tool diameter to 3mm**
   - `tp.stale_since` should be set (currently **NOT** — this is a gap)
   - Toolpath result is stale — moves were computed with 6mm tool

---

## W9. Dressup Application Order (P2)

### Steps and invariants

1. **Generate pocket with: ramp entry + lead-in + boundary clip**
2. **Verify**:
   - No vertical plunges in output (ramp replaced them)
   - Lead-in arc present before first cut
   - All moves within boundary polygon
   - Order: operation → entry → lead → link → arc → feed → tsp

---

## W10. Compute Status State Machine (P1)

### Steps and invariants

1. **Add toolpath (no model)** → status == Error("No 3D mesh")
2. **Add model, generate** → status transitions: Pending → Computing → Done
3. **Cancel during compute** → status == Pending, result == None
4. **Modify params while computing** → stale_since set, auto-regen queued

---

## Coverage heat map

| Workflow | Currently tested | Gap severity |
|----------|:---:|:---:|
| W1. STL → 3D → Export | Partial (core ops only) | Medium |
| W2. SVG → 2.5D → Verify | Partial (depth tests only) | Medium |
| W3. STEP → Faces → Pocket | **None** | **High** |
| W4. Face Selection Undo | **None** | **High** |
| W5. Save/Load Round-trip | Partial (no STEP) | High |
| W6. Height Invariants | **None** | **High** |
| W7. Multi-Setup Sim | **None** | Medium |
| W8. Tool → Staleness | **None** | Low (known gap) |
| W9. Dressup Order | **None** | Low |
| W10. Status State Machine | **None** | Medium |

## Implementation priority

1. **W3 + W6** — face Z and height invariants (caught 3 bugs this session)
2. **W5** — save/load with STEP (caught validation bug in review)
3. **W4** — undo for face selection (just implemented, needs coverage)
4. **W10** — status machine (foundational invariant)
5. **W1 + W2** — STL/SVG full workflows (regression coverage)
6. **W7-W9** — deeper workflows (lower priority)

## Cross-cutting gaps found during research

- **Undo missing coverage**: heights, boundary, coolant, pre/post gcode, stock_source not in undo snapshot
- **Tool edit doesn't mark dependent toolpaths stale**: changing tool diameter doesn't set stale_since on toolpaths using that tool
- **SVG/DXF imports skip stock auto-size**: intentional? (2D has no Z) but undocumented
