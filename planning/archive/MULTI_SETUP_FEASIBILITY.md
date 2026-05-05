# Multi-Setup Workflows: Feasibility Analysis

## Problem statement

rs_cam currently models a job as **one stock block, one orientation, N sequential operations**. This works well for single-sided 3-axis work but falls short when:

- A part needs machining on both faces (flip workpiece, re-clamp, machine the back)
- Fixtures/clamps occupy space the tool must avoid
- The operator needs clear per-setup instructions (which side is up, where to probe, what to clamp)
- Remaining-stock simulation needs to carry across orientation changes

This document evaluates what a "complete setup system" would look like, what the current codebase supports, what needs to change, and a phased implementation path.

---

## 1. What exists today

### Stock model
- `StockConfig`: a single axis-aligned box (x, y, z) with an origin offset
- Auto-sizing from model bounding box with padding
- Single `Material` enum attached to the stock
- Heightmap simulation initializes from this box

### Job structure
- `JobState` holds one `StockConfig`, one `PostConfig`, one `MachineProfile`
- `Vec<ToolpathEntry>` — flat ordered list, no grouping
- Each toolpath references a tool and a model by ID
- `StockSource::FromRemainingStock` simulates prior toolpaths sequentially but assumes same orientation

### Heights and boundaries
- 5-level height system (clearance / retract / feed / top / bottom) per operation
- Boundary clipping with containment modes (center / inside / outside)
- No concept of "avoid this region" or "clamp is here"

### Collision detection
- Holder/shank collision against mesh surface
- Rapid-through-stock detection
- No fixture collision checking

### What's missing
- No setup grouping or sequencing
- No coordinate system transforms (rotation, flip, WCS offsets)
- No fixture/clamp geometry or keep-out zones
- No per-setup stock state (what does the stock look like *after* flipping?)
- No alignment/datum concept (where do you probe after flipping?)

---

## 2. Design: The complete setup model

### 2.1 Core concept: Setup as a coordinate frame + stock state

A **Setup** represents one clamping of the workpiece on the machine. It defines:

```
Setup {
    id: SetupId,
    name: String,                          // "Top Face", "Flip — Bottom Face"

    // Coordinate frame
    orientation: Orientation,              // which face is up, how it's rotated
    wcs_offset: Vec3,                      // G54/G55 work offset from machine home

    // Stock state entering this setup
    stock_entering: StockState,            // inherited from prior setup or fresh

    // Workholding
    fixtures: Vec<Fixture>,                // clamps, tabs, vise jaws, tape areas

    // Machining bounds
    machining_boundary: Option<Boundary>,  // XY region the tool is allowed in
    keep_out_zones: Vec<KeepOutZone>,      // regions the tool must avoid

    // Heights (setup-level defaults, operations can override)
    default_heights: HeightsConfig,

    // Operations in this setup
    toolpaths: Vec<ToolpathEntry>,         // ordered within the setup
}
```

### 2.2 Orientation model

For 3-axis wood routing, orientation is simple — the part can only be presented in 6 axis-aligned orientations (which face is up) plus a rotation about Z:

```
Orientation {
    face_up: Face,          // Top, Bottom, Front, Back, Left, Right
    z_rotation_deg: f64,    // rotation about the vertical axis (0, 90, 180, 270 typical)
}
```

This maps to an affine transform that rotates stock geometry and all model geometry into the setup's coordinate frame. For the common flip case:

- **Setup 1 (Top)**: `face_up = Top, z_rotation = 0` → identity transform
- **Setup 2 (Flip)**: `face_up = Bottom, z_rotation = 0` → rotate 180° around X axis

The transform applies to:
- Model mesh/polygon coordinates
- Stock bounding box
- Heightmap (the entering stock state)
- Keep-out zone geometry

### 2.3 Stock state propagation

The key insight for multi-setup: after machining Setup 1, the stock is no longer a box. When you flip it for Setup 2, the previously-cut features are now on the bottom (or wherever).

```
StockState {
    // The stock is always defined in absolute coordinates.
    // Each setup's orientation transform maps it into the setup's working frame.
    bounding_box: BoundingBox3,
    material: Material,

    // Simulation state from prior setups (optional — can be None for first setup)
    heightmap: Option<Heightmap>,

    // Pre-machined features that become the "floor" in subsequent setups
    // (e.g., pockets cut in Setup 1 become visible from below in Setup 2)
}
```

**Propagation rules:**
1. Setup 1: stock is the raw block from `StockConfig`
2. After Setup 1 completes: simulate all toolpaths → produces a heightmap
3. Setup 2: transform that heightmap through the orientation change
4. Operations in Setup 2 see the transformed prior state as their starting stock

For the common flip-on-X case, the heightmap rows stay in place but Z values invert relative to the new top surface.

### 2.4 Fixture and workholding model

Fixtures serve two purposes: (a) tell the operator what to do, (b) generate keep-out geometry for the toolpath planner.

```
Fixture {
    id: FixtureId,
    name: String,
    kind: FixtureKind,
    geometry: FixtureGeometry,
    clearance: f64,              // extra margin around fixture for tool avoidance
}

enum FixtureKind {
    Clamp { jaw_width: f64 },
    DoubleSidedTape { area: Rect },
    ScrewHoldDown { screw_positions: Vec<Point2> },
    VacuumZone { outline: Polygon },
    TabsInStock,                  // tabs left from a prior setup
    Vise { jaw_opening: f64 },
    Custom { description: String },
}

enum FixtureGeometry {
    Box { min: P3, max: P3 },
    Cylinder { center: P2, radius: f64, z_min: f64, z_max: f64 },
    Polygon { outline: Vec<P2>, z_min: f64, z_max: f64 },
}
```

**How fixtures affect toolpaths:**
- Fixture geometry + clearance margin → extruded keep-out zones
- Keep-out zones are subtracted from the machining boundary before toolpath generation
- Collision checker gains a `check_fixture_collisions()` pass
- Setup sheet lists fixtures with placement instructions

### 2.5 Machining bounds and keep-out zones

Beyond fixtures, operators often need:

```
KeepOutZone {
    name: String,
    reason: String,                // "edge clamp", "fragile feature from Setup 1", etc.
    geometry: FixtureGeometry,
    clearance: f64,
}

Boundary {
    outline: Polygon,              // XY boundary (could be stock outline or custom)
    containment: ToolContainment,  // Center / Inside / Outside
    z_range: Option<(f64, f64)>,   // optional Z limits
}
```

The existing `clip_toolpath_to_boundary()` in `boundary.rs` already handles polygon clipping with containment modes. The new system would compose:

1. **Setup-level boundary** (default: stock XY outline)
2. **Minus keep-out zones** (fixtures, fragile areas)
3. **Per-operation boundary override** (existing feature, preserved)

### 2.6 Datum and probing

Each setup needs a probing strategy — how the operator establishes the work coordinate system after clamping:

```
DatumConfig {
    xy_method: XYDatumMethod,
    z_method: ZDatumMethod,
    notes: String,                 // freeform operator instructions
}

enum XYDatumMethod {
    CornerProbe { corner: Corner },   // front-left, front-right, etc.
    CenterProbe,                      // probe X and Y center of stock
    EdgeProbe { edge: Edge },         // probe one edge
    PinLocator { pin_positions: Vec<P2> },  // alignment pins from prior setup
    Manual { description: String },
}

enum ZDatumMethod {
    StockTop,                         // probe top of stock (common for Setup 1)
    MachineTable,                     // probe the spoilboard/table surface
    FixedOffset { z: f64 },           // known Z from machine home
    Manual { description: String },
}
```

---

## 3. Job model restructure

### Current hierarchy
```
Job
├── Stock (single)
├── Post
├── Machine
├── Models[]
├── Tools[]
└── Toolpaths[] (flat list)
```

### Proposed hierarchy
```
Job
├── RawStock (the blank before any machining)
├── Post
├── Machine
├── Models[]
├── Tools[]
└── Setups[] (ordered)
    └── Setup
        ├── Orientation
        ├── WCS offset
        ├── DatumConfig
        ├── Fixtures[]
        ├── KeepOutZones[]
        ├── Boundary (optional)
        ├── Default heights
        └── Toolpaths[] (ordered within setup)
```

**Key changes:**
- Toolpaths move from `Job → Vec<ToolpathEntry>` to `Setup → Vec<ToolpathEntry>`
- Stock becomes "RawStock" (the blank) at the job level
- Each setup derives its entering stock state from the prior setup's simulation
- Models and tools remain shared across all setups (they're resources, not per-setup)

---

## 4. Impact on existing systems

### 4.1 Core library (`rs_cam_core`)

| System | Impact | Notes |
|--------|--------|-------|
| `simulation.rs` | **Medium** | Heightmap transform (flip/rotate) needed for stock propagation. The stamping and simulation logic stays the same. |
| `boundary.rs` | **Low** | Already supports polygon boundary clipping. Needs composition with keep-out zones (polygon subtraction). |
| `collision.rs` | **Medium** | Add fixture collision checking alongside holder/shank checks. Same sampling approach, different geometry. |
| `depth.rs` | **None** | Depth stepping is orientation-agnostic. |
| `dressup.rs` | **None** | Dressups are toolpath-local, no setup awareness needed. |
| `gcode.rs` | **Medium** | Needs setup separators (tool change, WCS change, comments). May need G54/G55 output. |
| `geo.rs` | **Low** | Add `Transform3` and orientation-to-matrix conversion. |
| `pipeline.rs` | **Low** | Cache keys might need setup/orientation awareness. |
| All operations | **None** | Operations receive geometry in their local frame — transforms happen before dispatch. |

### 4.2 GUI state (`rs_cam_viz`)

| System | Impact | Notes |
|--------|--------|-------|
| `state/job.rs` | **High** | JobState restructured: toolpaths move into setups. StockConfig becomes RawStock. |
| `state/toolpath/` | **Low** | ToolpathEntry unchanged — it moves into a setup but its own structure doesn't change. |
| `state/history.rs` | **Medium** | Undo actions need setup-awareness (which setup was modified). |
| `state/simulation.rs` | **Medium** | Simulation needs to know which setup it's running within. |
| `ui/project_tree.rs` | **High** | Tree gains a setup level between job and toolpaths. |
| `ui/properties/` | **High** | New panels: setup properties (orientation, fixtures, datum). |
| `io/project.rs` | **High** | Serialization schema changes (versioned migration from flat to setup-grouped). |
| `io/setup_sheet.rs` | **High** | Setup sheet becomes multi-page (one section per setup with flip instructions). |
| `compute/worker.rs` | **Medium** | ComputeRequest needs setup orientation for geometry transform. |
| `render/stock_render.rs` | **Medium** | Render the stock in the current setup's orientation. Show fixtures. |

### 4.3 CLI (`rs_cam_cli`)

| System | Impact | Notes |
|--------|--------|-------|
| `job.rs` (TOML) | **Medium** | TOML format gains `[[setup]]` sections wrapping `[[operation]]` blocks. |
| `main.rs` | **Low** | Direct commands remain single-setup. Job runner gains setup iteration. |

---

## 5. UI design concepts

### 5.1 Project tree with setups

```
▼ My Part
  ├── Stock: 200 × 150 × 25 mm, Walnut
  ├── Post: GRBL
  ├── Machine: Shapeoko VFD
  ├── Models
  │   ├── part_top.stl
  │   └── part_bottom.stl
  ├── Tools
  │   ├── 6mm Flat End Mill
  │   └── 3mm Ball Nose
  ▼ Setup 1 — Top Face
  │ ├── [Orientation: Top up, 0°]
  │ ├── [Fixtures: 4× clamp]
  │ ├── Adaptive Roughing
  │ ├── Pencil Finish
  │ └── Scallop Finish
  ▼ Setup 2 — Bottom Face (flip on X)
    ├── [Orientation: Bottom up, 0°]
    ├── [Fixtures: double-sided tape]
    ├── Pocket
    └── Profile
```

### 5.2 Setup properties panel

When a setup is selected:

**Orientation section:**
- Face-up selector: visual cube showing which face is oriented up
- Z rotation: 0° / 90° / 180° / 270° buttons
- Preview of the stock in this orientation

**Workholding section:**
- Fixture list with add/remove
- Per-fixture: kind dropdown, position inputs, clearance
- Visual overlay of fixture geometry on stock

**Datum section:**
- XY method dropdown + corner/edge picker
- Z method dropdown
- Free-text operator notes

**Bounds section:**
- Setup boundary toggle (default: stock outline)
- Keep-out zone list with add/remove
- Visual overlay on stock

### 5.3 Viewport enhancements

- **Setup tab bar** or dropdown at top of viewport: switch between setups
- Active setup's orientation applies to the 3D view
- Fixture geometry rendered as semi-transparent colored blocks
- Keep-out zones rendered as red-tinted exclusion volumes
- Stock shows prior-setup machining (flipped heightmap visible as "already cut" regions)

### 5.4 Setup sheet (HTML export)

Multi-page document:
```
┌─────────────────────────────────┐
│ Setup 1 of 2: Top Face          │
│                                 │
│ Stock: 200 × 150 × 25 mm       │
│ Material: Walnut                │
│ Orientation: Top face up        │
│                                 │
│ Workholding:                    │
│   4× edge clamps, 15mm inset   │
│                                 │
│ Datum:                          │
│   XY: Front-left corner probe  │
│   Z: Stock top surface probe   │
│                                 │
│ Operations:                     │
│   1. Adaptive Roughing (6mm)   │
│   2. Pencil Finish (3mm)       │
│   3. Scallop Finish (3mm)      │
│                                 │
│ Output file: part_setup1.nc     │
├─────────────────────────────────┤
│ Setup 2 of 2: Bottom Face       │
│                                 │
│ ⟲ FLIP part on X axis          │
│                                 │
│ Workholding:                    │
│   Double-sided tape, full face  │
│                                 │
│ Datum:                          │
│   XY: Front-left corner probe  │
│   Z: Table surface + 25mm      │
│                                 │
│ Operations:                     │
│   1. Pocket (6mm)              │
│   2. Profile (6mm)             │
│                                 │
│ Output file: part_setup2.nc     │
└─────────────────────────────────┘
```

---

## 6. The hard problems

### 6.1 Heightmap orientation transform

When you flip a part, the heightmap from Setup 1 needs to be transformed into Setup 2's frame. For the simple flip-on-X case:

- The XY grid stays the same (Y rows flip)
- Z values transform: `new_z = stock_thickness - old_z`
- Cut regions become raised features (seen from below)
- Uncut regions become the new "top" surface

This is tractable for axis-aligned flips but gets complex for arbitrary rotations. **Recommendation:** restrict to axis-aligned face-up orientations (6 choices) which are all simple grid transforms.

### 6.2 Keep-out zone integration with toolpath generation

Currently, operations generate toolpaths, then `clip_toolpath_to_boundary()` clips them. Keep-out zones could work the same way (clip to boundary minus keep-out polygons), but this has a subtlety: the tool shouldn't just be clipped *out* of keep-out zones — it should ideally never *plan* paths through them.

**Pragmatic approach:** Use boundary clipping (which already works) as the first pass. This is what most hobbyist CAM does. Full keep-out-aware path planning would be a future enhancement.

### 6.3 G-code output strategy

Options:
- **One file per setup** (simplest, most common for hobby CNC): each setup exports to its own `.nc` file. Operator loads them sequentially.
- **Single file with setup markers**: `M0` (program pause) between setups, with comments. Riskier — operator must physically flip the part during the pause.

**Recommendation:** Default to one file per setup. Optional combined file with `M0` pauses.

### 6.4 Migration from flat toolpath list

Existing projects have a flat `Vec<ToolpathEntry>`. Migration path:

1. On load, if no setups are present, wrap all toolpaths in a single "Default Setup" with identity orientation
2. This is a lossless migration — existing projects work identically
3. Users can then split operations into multiple setups via the UI

---

## 7. Phased implementation

### Phase 1: Setup container (foundation)

**Goal:** Introduce the Setup struct as a grouping container without orientation transforms.

- Add `Setup` struct with id, name, and `Vec<ToolpathEntry>`
- Migrate `JobState.toolpaths` → `JobState.setups[0].toolpaths`
- Update project tree to show setup level
- Update project IO with versioned migration
- Update undo/redo for setup-awareness
- Single setup behaves identically to today

**Estimated scope:** Medium. Mostly state restructuring and UI tree changes. No core library changes.

### Phase 2: Fixtures and keep-out zones

**Goal:** Rich setup definition — workholding, bounds, avoidance zones.

- Add `Fixture` and `KeepOutZone` structs
- Setup properties panel for fixture editing
- Fixture rendering in viewport (colored boxes/cylinders)
- Keep-out zones subtracted from machining boundary
- Fixture collision checking in analysis lane
- Setup sheet fixture documentation

**Estimated scope:** Medium-high. New UI panels, new rendering, boundary composition logic.

### Phase 3: Orientation and stock propagation

**Goal:** Flip-and-machine workflows.

- Add `Orientation` struct with face-up and Z rotation
- Geometry transform pipeline (mesh + heightmap through orientation)
- Heightmap flip/rotate for stock state propagation between setups
- Viewport orientation switching
- Per-setup G-code export
- Datum/probing configuration

**Estimated scope:** High. This is the most technically complex phase — coordinate transforms touching simulation, rendering, and export.

### Phase 4: Polish and advanced features

- Combined G-code with M0 pauses
- Pin locator / alignment feature registration between setups
- Stock state visualization (show what was cut in prior setups)
- Setup sheet multi-page layout
- CLI TOML `[[setup]]` sections

---

## 8. Feasibility assessment

### What makes this tractable

1. **Operations are already frame-agnostic**: Every operation receives geometry in a local coordinate frame. If we transform geometry *before* passing it to the operation, all 22 operations work without modification.

2. **Boundary clipping exists**: The keep-out zone approach (subtract from boundary, clip toolpaths) builds on `boundary.rs` which is already robust.

3. **Heightmap simulation is in place**: Stock propagation is "just" a heightmap transform between setups, which is a grid operation.

4. **The UI is event-driven and extensible**: The `AppEvent` enum, selection-driven property panels, and project tree are all designed to grow. Adding `Selection::Setup(SetupId)` and `AppEvent::AddSetup` follows established patterns.

5. **Project IO is versioned**: The `format_version` field in `ProjectFile` enables lossless migration.

### What makes this hard

1. **State restructuring is pervasive**: Moving toolpaths into setups touches job state, undo, selection, compute dispatch, simulation, export, and project IO. It's a refactor of the central data model.

2. **Heightmap transforms for non-trivial orientations**: Flipping a heightmap around X is straightforward. Arbitrary 90° rotations require grid re-sampling. The 6 axis-aligned cases are manageable but each needs explicit handling.

3. **Fixture UI is new territory**: rs_cam has no precedent for placing 3D geometry interactively in the viewport. Fixture editing would likely start with numeric inputs (position, size) rather than drag-to-place.

4. **Testing complexity**: Multi-setup workflows multiply the test matrix. Each operation × each orientation × each fixture configuration.

### Overall verdict

**Feasible, with Phase 1 being the key enabler.** The Setup container (Phase 1) is a moderate refactor that unlocks everything else. Phases 2-4 can be delivered incrementally. The architecture doesn't fight this — it's a natural extension of the existing job model.

The biggest risk is Phase 1's pervasiveness — it touches most of the GUI state layer. But it's a mechanical refactor (move toolpaths one level deeper), not an algorithmic challenge.

**Recommended starting point:** Phase 1, with a single default setup that preserves backward compatibility. This can be merged independently and validates the data model before committing to transforms and fixtures.
