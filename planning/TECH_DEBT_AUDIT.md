# Tech Debt Audit — Post Service Layer + MCP Refactor

**Date**: 2026-04-11
**Auditor**: Claude (6 specialist agents, full codebase read)
**Codebase**: 110,758 lines of Rust across 4 crates, 186 source files

## Executive Summary

The service-layer refactor (adding `ProjectSession` + MCP) was architecturally successful:
the compute layer has **zero production code duplication**, the MCP layers contain **no
business logic**, and the algorithm test suite is excellent (705+ inline tests, parameter
sweep infrastructure, property tests).

However, the refactor was done "inside-out" — the new session API was built and the MCP
wired to it, but the **existing GUI controller was never fully migrated**. The result is a
clean core with a partially-migrated GUI that still bypasses the session layer in 11+
places, skipping cache/simulation invalidation. Additionally, the entire session API and
MCP server layer shipped with **zero test coverage** and **zero tracing instrumentation**.

---

## Table of Contents

1. [CRITICAL: Session Layer Bypasses](#1-critical-session-layer-bypasses)
2. [CRITICAL: Test Coverage Gaps](#2-critical-test-coverage-gaps)
3. [CRITICAL: Tracing Coverage](#3-critical-tracing-coverage)
4. [HIGH: Data Structure Duplication](#4-high-data-structure-duplication-core-vs-viz)
5. [HIGH: Oversized Modules](#5-high-oversized-modules)
6. [HIGH: MCP API Asymmetry](#6-high-mcp-api-asymmetry)
7. [MODERATE: Simulation Group Duplication](#7-moderate-simulation-group-building-duplication)
8. [MODERATE: Test Helper Duplication](#8-moderate-test-helper-code-duplicates-core-logic)
9. [MODERATE: Module Boundary Issues](#9-moderate-module-boundary-issues)
10. [LOW: Minor Items](#10-low-minor-items)
11. [What's Working Well](#11-whats-working-well)
12. [Recommended Fix Order](#12-recommended-fix-order)

---

## 1. CRITICAL: Session Layer Bypasses

### Problem

`ProjectSession` was designed as the single entry point for all state mutations. Its
mutation methods (`set_toolpath_enabled`, `remove_tool`, `set_stock_config`, etc.)
perform cache invalidation (`self.results.remove(&index)`, `self.simulation = None`)
to keep computed state consistent.

The GUI controller bypasses these methods via `_mut()` accessors, directly mutating
internal state without invalidation. This means:
- Simulation state can become stale without the GUI knowing
- Cached toolpath results aren't cleared when their inputs change
- If the session API adds logging, validation, or undo tracking later, the GUI won't
  get those benefits

### Root Cause

`ProjectSession` exposes mutable accessors for convenience:

```rust
// crates/rs_cam_core/src/session/mod.rs:586-668
pub fn stock_mut(&mut self) -> &mut StockConfig { &mut self.stock }
pub fn machine_mut(&mut self) -> &mut MachineProfile { &mut self.machine }
pub fn tools_mut(&mut self) -> &mut Vec<ToolConfig> { &mut self.tools }
pub fn models_mut(&mut self) -> &mut Vec<LoadedModel> { &mut self.models }
pub fn toolpath_configs_mut(&mut self) -> &mut Vec<ToolpathConfig> { &mut self.toolpath_configs }
pub fn setups_mut(&mut self) -> &mut Vec<SetupData> { &mut self.setups }
```

These bypass the session's invalidation contract. The GUI uses them extensively.

### All Bypass Instances

#### Bypass 1: ToggleToolpathEnabled

**File**: `crates/rs_cam_viz/src/controller/events/mod.rs:79-84`

```rust
AppEvent::ToggleToolpathEnabled(tp_id) => {
    if let Some((idx, _tc)) = self.state.session.find_toolpath_config_by_id(tp_id.0)
        && let Some(tc) = self.state.session.toolpath_configs_mut().get_mut(idx)
    {
        tc.enabled = !tc.enabled;
    }
}
```

**Should call**: `self.state.session.set_toolpath_enabled(idx, !tc.enabled)` — which
exists in `mutation.rs:127-139` and invalidates simulation.

**Impact**: Enabling/disabling a toolpath doesn't invalidate simulation. Stale sim
results may show the wrong toolpaths.

---

#### Bypass 2: ToggleFaceSelection

**File**: `crates/rs_cam_viz/src/controller/events/mod.rs:120-145`

```rust
AppEvent::ToggleFaceSelection { toolpath_id, model_id: _, face_id } => {
    if let Some((idx, _)) = self.state.session.find_toolpath_config_by_id(toolpath_id.0) {
        if let Some(tc) = self.state.session.toolpath_configs_mut().get_mut(idx) {
            let faces = tc.face_selection.get_or_insert_with(Vec::new);
            if let Some(pos) = faces.iter().position(|f| *f == face_id) {
                faces.remove(pos);
            } else {
                faces.push(face_id);
            }
            if faces.is_empty() {
                tc.face_selection = None;
            }
        }
        // Does mark stale_since on GUI runtime — partial fix
    }
}
```

**Missing session method**: No `set_face_selection()` method exists on `ProjectSession`.
Needs to be added. The direct mutation skips `self.results.remove(&index)` and
`self.simulation = None`.

**Impact**: Changing BREP face selection doesn't clear cached toolpath results. The user
must manually regenerate to see the change take effect (the GUI `stale_since` marker is
set, which helps, but cached results remain).

---

#### Bypass 3: MoveToolpathToSetup

**File**: `crates/rs_cam_viz/src/controller/events/toolpath.rs:222-247`

```rust
pub(crate) fn handle_move_toolpath_to_setup(&mut self, tp_id, setup_id, _idx) {
    if let Some((tp_idx, _)) = self.state.session.find_toolpath_config_by_id(tp_id.0) {
        // Remove from source setup's toolpath_indices
        for setup in self.state.session.setups_mut() {
            setup.toolpath_indices.retain(|&i| i != tp_idx);
        }
        // Add to target setup's toolpath_indices
        if let Some(target) = self.state.session.setups_mut().iter_mut()
            .find(|s| s.id == setup_id.0)
        {
            target.toolpath_indices.push(tp_idx);
        }
        self.pending_upload = true;
        self.state.gui.mark_edited();
    }
}
```

**Missing session method**: No `move_toolpath_to_setup()` method exists on
`ProjectSession`. Needs to be added. The direct mutation:
- Does NOT invalidate simulation
- Does NOT clear cached toolpath result (setup transform may have changed)
- Does NOT validate the target setup exists

**Impact**: Moving a toolpath between setups (e.g., top-to-bottom) doesn't trigger
re-computation with the new setup's coordinate transforms. The cached result is from the
old setup's orientation.

---

#### Bypass 4: RemoveTool

**File**: `crates/rs_cam_viz/src/controller/events/model.rs:76-101`

```rust
pub(crate) fn handle_remove_tool(&mut self, tool_id) {
    let in_use = self.state.session.toolpath_configs().iter()
        .any(|tc| tc.tool_id == tool_id.0);
    if in_use {
        // warn and return
    } else {
        self.state.session.tools_mut().retain(|tool| tool.id != tool_id);
        // ...
    }
}
```

**Should call**: `self.state.session.remove_tool(index)` — which exists in
`mutation.rs:223-241` and performs the same in-use check via `SessionError::ToolInUse`.

**Impact**: Low risk since the logic is replicated, but the GUI version finds by `ToolId`
while the session method finds by index. If the session API later adds cleanup logic
(e.g., updating ID counters), this bypass will miss it.

---

#### Bypass 5: RemoveModel

**File**: `crates/rs_cam_viz/src/controller/events/model.rs:327-358`

```rust
pub(crate) fn handle_remove_model(&mut self, model_id) {
    let in_use = self.state.session.toolpath_configs().iter()
        .any(|tc| tc.model_id == model_id.0);
    if in_use {
        // warn and return
    } else {
        self.state.session.models_mut().retain(|m| m.id != model_id.0);
        // ...
    }
}
```

**Should use session API**: Similar pattern to RemoveTool. Duplicates the in-use check
and bypasses session's `remove_model()` method.

---

#### Bypass 6: RenameSetup

**File**: `crates/rs_cam_viz/src/controller/events/model.rs:197-207`

```rust
pub(crate) fn handle_rename_setup(&mut self, setup_id, name) {
    if let Some(setup) = self.state.session.setups_mut().iter_mut()
        .find(|s| s.id == setup_id.0)
    {
        setup.name = name;
        self.state.gui.mark_edited();
    }
}
```

**Missing session method**: No `rename_setup()` on `ProjectSession`. Low impact (naming
doesn't affect compute) but breaks the API contract.

---

#### Bypass 7: Fixture + KeepOut CRUD (4 handlers)

**File**: `crates/rs_cam_viz/src/controller/events/model.rs:210-323`

- `handle_add_fixture()` — directly pushes to `setup.fixtures` via `setups_mut()`
- `handle_remove_fixture()` — directly retains via `setups_mut()`
- `handle_add_keep_out()` — directly pushes to `setup.keep_out_zones` via `setups_mut()`
- `handle_remove_keep_out()` — directly retains via `setups_mut()`

**Missing session methods**: No fixture/keep-out CRUD methods on `ProjectSession`.

**Impact**: Fixtures and keep-out zones affect boundary clipping during toolpath
generation. Adding/removing them should invalidate toolpath results and simulation.
Currently doesn't.

---

#### Bypass 8: AddSetup (partial)

**File**: `crates/rs_cam_viz/src/controller/events/model.rs:104-118`

```rust
pub(crate) fn handle_add_setup(&mut self) {
    let idx = self.state.session.add_setup("".to_owned(), FaceUp::default());
    // Then immediately accesses setups_mut() to rename it:
    if let Some(s) = self.state.session.setups_mut().get_mut(idx) {
        s.name = format!("Setup {}", s.id + 1);
    }
}
```

**Issue**: Correctly calls `add_setup()` but then mutates the name directly. Could be
solved by passing the name to `add_setup()` instead.

---

#### Bypass 9: AlignmentPinDrill sync

**File**: `crates/rs_cam_viz/src/controller/events/model.rs:460-464`

```rust
if let Some(tc) = self.state.session.toolpath_configs_mut().get_mut(idx)
    && let OperationConfig::AlignmentPinDrill(ref mut cfg) = tc.operation
{
    cfg.holes = new_holes;
}
```

**Issue**: Directly mutates operation config to update pin drill holes. Should go through
`set_toolpath_param()` or a dedicated session method. Doesn't invalidate cached result.

---

#### Bypass 10: UI Property Panels (stock, machine, tool)

**File**: `crates/rs_cam_viz/src/ui/properties/mod.rs`

```rust
// Line 194: Stock panel — direct mutation
stock::draw(ui, state.session.stock_mut(), has_flipped_setup, events);

// Line 234: Tool panel — direct mutation
if let Some(t) = state.session.tools_mut().iter_mut().find(|t| t.id == id) {
    tool::draw(ui, t);
}

// Line 803: Machine preset — direct mutation
*state.session.machine_mut() = presets[i].1.clone();

// Line 851: Safety factor slider — direct mutation
egui::Slider::new(&mut state.session.machine_mut().safety_factor, 0.60..=0.95)

// Line 879: Workholding rigidity — direct mutation
let rigidity = &mut state.session.stock_mut().workholding_rigidity;
```

**Impact**: All of these skip `set_stock_config()`, `set_tool_param()`, `set_machine()`.
Stock and tool changes should invalidate simulation and cached results. The UI does fire
`StockChanged` / `MachineChanged` events, so some invalidation happens through the event
handler — but the session itself doesn't know its state was modified.

### Fix Strategy

1. Add missing session methods: `move_toolpath_to_setup()`, `set_face_selection()`,
   `rename_setup()`, fixture/keep-out CRUD
2. Migrate all GUI handlers to call session methods instead of `_mut()` accessors
3. Deprecate or restrict the `_mut()` accessors (make `pub(crate)` or add
   `#[deprecated]`)
4. For UI property panels: either route through session methods on each frame change, or
   accept direct mutation for interactive editing with a commit-on-release pattern that
   calls the session method

---

## 2. CRITICAL: Test Coverage Gaps

### Overview

| Layer | Files | Inline tests | Coverage |
|-------|-------|-------------|----------|
| Core algorithms | 50+ | 705 | Excellent |
| Core compute | 15 | ~30 | Partial |
| Core session | 5 | 3 | Near zero |
| Core integration | 5 files | ~18 | Good |
| Viz controller | 3 | 77 | Good |
| MCP server | 2 | 0 | **Zero** |
| CLI | 2 | ~10 | Adequate |

### Untested Critical Code

#### Session API — 2,297 LOC, 3 tests

The session API is the single entry point for all consumers. Its tests:

**File**: `crates/rs_cam_core/src/session/mod.rs` (bottom, in `#[cfg(test)]`)

Only 3 basic smoke tests exist:
- `empty_project_loads` — creates empty session, checks defaults
- `stock_bbox` — verifies stock bounding box
- `diagnostics_empty` — checks diagnostics on empty project

**Completely untested**:

| File | LOC | What's untested |
|------|-----|----------------|
| `session/mutation.rs` | 346 | All CRUD: `add_toolpath`, `remove_toolpath`, `reorder_toolpath`, `add_tool`, `remove_tool`, `add_setup`, `remove_setup`, `set_dressup_config`, `set_heights_config`, `set_boundary_config`, `replace_tools`, `replace_toolpath_config`, `set_stock_config`, `set_post_config`, `set_machine` |
| `session/compute.rs` | 909 | `set_toolpath_param` (serde round-trip!), `set_tool_param`, `generate_toolpath`, `generate_all`, `run_simulation`, `collision_check`, `export_gcode`, `diagnostics` |
| `session/project_file.rs` | 815 | TOML deserialization, model loading, format version handling, all default value application |
| `session/save.rs` | 227 | TOML serialization, atomic file write, all enum serialization |

**What tests are needed (priority order)**:

1. **Mutation CRUD tests** — add/remove/reorder toolpaths with index validation, tool
   reference checking, setup ownership. Verify `self.simulation = None` is set. Verify
   result cache invalidation.
2. **set_toolpath_param serde round-trip** — this uses JSON serialize→merge→deserialize
   to set arbitrary operation params. Edge cases: unknown param name, wrong value type,
   Optional fields, nested structs.
3. **Project file round-trip** — load a TOML, save it, load again, compare. Test all
   tool types, operation types, setup orientations, fixtures, keep-out zones.
4. **generate_toolpath** — at minimum: missing geometry error, tool-not-found error, and
   one successful 2D + one successful 3D generation.
5. **set_tool_param** — all 11 parameter names, invalid param name, wrong type.

#### MCP Server — 1,173 LOC, 0 tests

**File**: `crates/rs_cam_mcp/src/server.rs`

42 MCP tool handlers with zero test coverage. This is the external API surface that
Claude (and potentially other AI tools) uses to control the CAM system.

**What's untested**: Every single tool handler — `load_project`, `list_toolpaths`,
`add_tool`, `remove_tool`, `add_toolpath`, `set_toolpath_param`, `generate_toolpath`,
`run_simulation`, `get_diagnostics`, `export_gcode`, `set_boundary_config`, etc.

**Testing approach**: The standalone MCP server wraps `ProjectSession` behind a
`TokioMutex`. Tests can construct a `ProjectSession` directly, call the handler
functions, and verify JSON responses.

#### Compute Configuration — ~850 LOC, 0 tests

| File | LOC | What's untested |
|------|-----|----------------|
| `compute/operation_configs.rs` | 1,238 | Operation config construction, parameter bounds |
| `compute/tool_config.rs` | 176 | Tool config parsing, enum conversion |
| `compute/stock_config.rs` | 201 | Stock geometry, alignment pin config, auto-bbox |
| `compute/transform.rs` | 296 | FaceUp/ZRotation coordinate transforms |
| `compute/cutter.rs` | 37 | Tool type → ToolDefinition dispatch |
| `compute/annotate.rs` | 492 | Runtime annotation conversion |

---

## 3. CRITICAL: Tracing Coverage

### Overview

**11 of 186 source files** (5.9%) have any tracing instrumentation. Only 5 functions in
the entire codebase use `#[instrument]`.

| Layer | Files with tracing | Total files | Coverage |
|-------|-------------------|-------------|----------|
| Session API | 0 | 5 | **0%** |
| Core algorithms | 7 | 50+ | 14% |
| Compute layer | 0 | 15 | **0%** |
| IO (import/export) | 1 | 10+ | 10% |
| Viz controller | 5 | 20 | 25% |
| CLI | 4 | 5 | 80% |
| MCP | 1 | 3 | 33% |

### Files With Tracing (the 11)

**Core** (7): `adaptive3d.rs` (19 trace points, excellent), `scallop.rs` (3),
`pencil.rs` (5), `ramp_finish.rs` (4), `steep_shallow.rs`, `mesh.rs`,
`session/project_file.rs`

**CLI** (3): `main.rs`, `job.rs`, `project.rs`, `sweep.rs`

**MCP** (1): `main.rs` (subscriber setup only)

### What Needs Tracing

**Phase 1 — Session layer (every public method)**:

```rust
// Example of what session/compute.rs:generate_toolpath should look like:
#[tracing::instrument(skip(self, cancel), fields(op = %tc_label))]
pub fn generate_toolpath(&mut self, index: usize, cancel: &AtomicBool)
    -> Result<&ToolpathComputeResult, SessionError> {
    // ...
}
```

Methods to instrument:
- `session/mod.rs`: `load()`, `from_project_file()`, `new_empty()`
- `session/mutation.rs`: all 15 public CRUD methods
- `session/compute.rs`: `set_toolpath_param()`, `set_tool_param()`,
  `generate_toolpath()`, `generate_all()`, `run_simulation()`, `collision_check()`,
  `export_gcode()`, `diagnostics()`
- `session/save.rs`: `save()`

**Phase 2 — Algorithm entry points** (high-line-count, long-running):

| File | Lines | Current tracing |
|------|-------|----------------|
| `adaptive.rs` | 2,754 | None |
| `waterline.rs` | 424 | None |
| `pushcutter.rs` | 553 | None |
| `dropcutter.rs` | 743 | None |
| `zigzag.rs` | ~500 | None |
| `pocket.rs` | ~300 | None |
| `profile.rs` | ~300 | None |
| `rest.rs` | ~400 | None |
| `depth.rs` | ~300 | None |

**Phase 3 — IO and compute**:

All import functions (`dxf_input.rs`, `svg_input.rs`, `step_input.rs`), G-code export
(`gcode.rs`), and all compute modules.

### Silent Error Swallowing

5+ instances where errors are silently dropped:

```rust
// session/compute.rs:104 — serde serialization failure silently dropped
let check = serde_json::to_value(&new_op).ok();

// session/compute.rs:755 — missing field gets silent default
.unwrap_or_default();

// simulation_cut.rs:751-752 — file cleanup failure silent
std::fs::remove_file(path).ok();
std::fs::remove_dir(dir).ok();

// session/project_file.rs — multiple .map_err() that lose the original error context
.map_err(|e| SessionError::TomlSerialize(e.to_string()))?
```

### `#[instrument]` Usage

Only 5 functions currently:
- `adaptive3d::adaptive_3d_toolpath()` — with structured fields (tool_radius, stepover)
- `adaptive3d::adaptive_3d_toolpath_annotated()`
- `scallop.rs` — 1 function
- `step_input.rs` — `load_step()`
- `pencil.rs` / `ramp_finish.rs` — 1 each

`adaptive3d.rs` is the gold standard. Its pattern should be replicated across all
algorithm files:

```rust
#[tracing::instrument(skip(mesh, index, cutter, params),
    fields(tool_radius = params.tool_radius, stepover = params.stepover))]
pub fn adaptive_3d_toolpath(...) -> ... {
    debug!("Building spatial grid");
    // ...
    info!(z_level = level, "Starting level {}/{}", level_idx, total_levels);
    // ...
}
```

---

## 4. HIGH: Data Structure Duplication (Core vs Viz)

### LoadedModel — defined twice

**Core**: `crates/rs_cam_core/src/session/mod.rs:142-159`

```rust
pub struct LoadedModel {
    pub id: usize,
    pub name: String,
    pub mesh: Option<Arc<TriangleMesh>>,
    pub polygons: Option<Arc<Vec<Polygon2>>>,
    pub path: PathBuf,
    pub kind: Option<ModelKind>,
    pub units: Option<ModelUnits>,
    pub enriched_mesh: Option<Arc<EnrichedMesh>>,
    pub winding_report: Option<f64>,
    pub load_error: Option<String>,
}
```

**Viz**: `crates/rs_cam_viz/src/state/job.rs:22-35`

```rust
pub struct LoadedModel {
    pub id: ModelId,       // type alias, same underlying type
    pub path: PathBuf,
    pub name: String,
    pub kind: ModelKind,   // non-optional vs core's Option<ModelKind>
    pub mesh: Option<Arc<TriangleMesh>>,
    pub polygons: Option<Arc<Vec<Polygon2>>>,
    pub enriched_mesh: Option<Arc<EnrichedMesh>>,
    pub units: ModelUnits, // non-optional vs core's Option<ModelUnits>
    pub winding_report: Option<f64>,
    pub load_error: Option<String>,
}
```

**Differences**: Viz has non-optional `kind` and `units` where core has `Option<_>`.
Otherwise identical fields. The viz version also adds `bbox()` and `placeholder()` methods.

**Fix**: Viz should re-export and extend core's type (wrapper struct or just use core's
type with helper methods in an extension trait), not redefine it. Currently, converting
between them requires mapping every field.

### Fixture and KeepOutZone — different field structures

**Core**: `crates/rs_cam_core/src/session/mod.rs:184-267`

```rust
pub struct Fixture {
    pub id: FixtureId,
    pub name: String,
    pub kind: FixtureKind,
    pub enabled: bool,
    pub origin_x: f64, pub origin_y: f64, pub origin_z: f64,
    pub size_x: f64, pub size_y: f64, pub size_z: f64,
    pub clearance: f64,
}
```

**Viz re-exports core's types** (confirmed in `state/job.rs` re-exports). This is
actually fine — the audit initially flagged this as a duplication but the viz version
was removed during the refactor. The re-exports at `state/job.rs:12-18` correctly
point to core.

**Remaining issue**: Only `LoadedModel` is genuinely duplicated.

---

## 5. HIGH: Oversized Modules

Files over 1,500 lines that contain multiple distinct concerns and should be split into
sub-modules:

### adaptive3d.rs — 4,736 lines

**Location**: `crates/rs_cam_core/src/adaptive3d.rs`

**Current contents** (single file):
- Type definitions (RegionOrdering, ClearingStrategy3d, EntryStyle3d, Adaptive3dParams)
- Search direction logic (SearchDirection3dResult)
- Path bounds calculation (path_bounds_3d)
- Material grid management
- Engagement calculation
- Main adaptive algorithm loop
- Tracing instrumentation (the best in the codebase)
- Tests

**Recommended split**:
```
adaptive3d/
  mod.rs       — public API, Adaptive3dParams, re-exports
  search.rs    — direction search algorithm
  clearing.rs  — clearing strategy implementations
  entry.rs     — entry style implementations
  engagement.rs — engagement calculation (shareable with 2D adaptive)
```

### adaptive.rs — 2,754 lines

**Location**: `crates/rs_cam_core/src/adaptive.rs`

**Recommended split**:
```
adaptive/
  mod.rs          — public API, AdaptiveParams
  material_grid.rs — MaterialGrid struct + grid operations
  search.rs       — direction search loop
  path.rs         — path generation + linking
```

### dexel_stock.rs — 1,776 lines

**Location**: `crates/rs_cam_core/src/dexel_stock.rs`

**Recommended split**:
```
dexel_stock/
  mod.rs          — TriDexelStock type, public API
  cut_direction.rs — StockCutDirection enum + axis decomposition
  stamping.rs     — tool stamping + material removal methods
  simulation.rs   — simulation integration (SimulationCutSample)
```

### Other oversized files (lower priority)

| File | Lines | Notes |
|------|-------|-------|
| `dexel_mesh.rs` | 1,165 | Mesh extraction from stock; split z_grid/colors/cavity |
| `simulation_cut.rs` | 1,220 | Metrics + hotspot detection; split hotspot/issues |
| `compute/execute.rs` | 1,329 | Giant match over 23 ops; acceptable as orchestration hub |
| `feeds/mod.rs` | 1,375 | Dense vendor LUT; acceptable as reference data |

---

## 6. HIGH: MCP API Asymmetry

Two MCP implementations exist:

1. **Embedded** (`crates/rs_cam_viz/src/app/mcp.rs`, 1,873 lines) — runs inside the GUI
2. **Standalone** (`crates/rs_cam_mcp/src/server.rs`, 1,173 lines) — headless server

### Tools missing from standalone MCP

| Tool | Available in embedded | Available in standalone | GUI-specific? |
|------|----------------------|----------------------|---------------|
| `set_setup_face` | Yes | **No** | No |
| `add_setup` | Yes | **No** | No |
| `move_toolpath_to_setup` | Yes | **No** | No |
| `import_model` | Yes | **No** | No |
| `inspect_model` | Yes | **No** | No |
| `inspect_stock` | Yes | **No** | No |
| `inspect_machine` | Yes | **No** | No |
| `inspect_brep_faces` | Yes | **No** | No |
| `add_alignment_pin` | Yes | **No** | No |
| `remove_alignment_pin` | Yes | **No** | No |
| `sim_jump_to_move` | Yes | No | Yes |
| `sim_scrub_toolpath` | Yes | No | Yes |
| `sim_jump_to_start` | Yes | No | Yes |
| `sim_jump_to_end` | Yes | No | Yes |
| `sim_jump_to_toolpath_start` | Yes | No | Yes |
| `sim_jump_to_toolpath_end` | Yes | No | Yes |

The 6 `sim_*` tools are legitimately GUI-only. The other 10 should be available in the
standalone server since they operate on `ProjectSession` state that exists in both
contexts.

### Duplicated parsing logic

Boundary source/containment parsing is duplicated between the two MCP implementations:

**Embedded**: `crates/rs_cam_viz/src/app/mcp.rs:1407-1426`
**Standalone**: `crates/rs_cam_mcp/src/server.rs:1049-1068`

Both have identical match statements. The shared utilities (`parse_operation_type`,
`parse_tool_type`) are correctly factored to `rs_cam_mcp/src/server.rs` and re-imported
by the embedded MCP. `parse_boundary_source` and `parse_boundary_containment` should
follow the same pattern.

---

## 7. MODERATE: Simulation Group Building Duplication

### Problem

Simulation group building (converting session state into per-setup simulation requests)
is implemented in two places:

1. **GUI**: `crates/rs_cam_viz/src/controller/events/simulation.rs:101-183`
   (`build_simulation_groups`)
2. **Session**: `crates/rs_cam_core/src/session/compute.rs:558-598`
   (inside `run_simulation`)

Both compute:
- Local stock bounding box per setup (accounting for FaceUp + ZRotation)
- Setup transform (local_to_global mapping)
- Toolpath filtering and result collection

The GUI version accesses `toolpath_rt` (GUI-only runtime state) to get computed results,
which is a legitimate difference — the session stores results in `self.results`, the GUI
stores them in a parallel `toolpath_rt` HashMap. But the setup transform and stock bbox
logic is duplicated.

### Impact

If setup transform logic changes (e.g., adding a new FaceUp variant), both
implementations must be updated. This is a maintenance risk, not a correctness issue
today.

### Fix

Extract the shared logic into a method on `ProjectSession` that returns
`Vec<SetupSimGroup>` without the toolpath results. The GUI can then attach its own
results to each group.

---

## 8. MODERATE: Test Helper Code Duplicates Core Logic

### Problem

Test-only files in the viz crate duplicate operation execution logic from core:

**File**: `crates/rs_cam_viz/src/compute/worker/execute/operations_2d.rs`
(guarded by `#[cfg(test)]`)

Contains `run_profile()`, `run_inlay()`, `run_zigzag()` — each duplicating parameter
building and execution dispatch from `core/compute/execute.rs`.

**File**: `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs`
(guarded by `#[cfg(test)]`)

Contains `run_scallop_annotated()` — duplicating tool validation and parameter building.

### Impact

These are test-only, so no production risk. But they can drift from core's execution
logic, making test results misleading.

### Fix

Replace these helpers with direct calls to `execute_operation_annotated()` from core,
passing the appropriate `OperationConfig` variant. This removes ~200 lines of duplicated
test code.

---

## 9. MODERATE: Module Boundary Issues

### Dexel Hierarchy Encapsulation

`dexel_mesh.rs` accesses `DexelGrid` internals directly (from `dexel.rs`) rather than
going through `TriDexelStock` (from `dexel_stock.rs`). The dependency chain is:

```
dexel.rs (low-level: DexelSegment, DexelRay, DexelGrid)
  ↑ used by
dexel_stock.rs (TriDexelStock — wraps 3 DexelGrids)
  ↑ used by
dexel_mesh.rs (mesh extraction — but also directly uses dexel.rs internals)
```

`dexel_mesh.rs` should only depend on `TriDexelStock`'s public API, not reach into
`DexelGrid` internals.

### Setup Transform Centralization

Z-flip and setup orientation logic appears in multiple places:

- `compute/transform.rs` — defines `FaceUp`, `ZRotation`, `SetupTransformInfo`
- `project_curve.rs` — has its own mesh Z-flip logic and `setup_z_flipped` flag
- `session/compute.rs` — applies transforms during `generate_toolpath()`
- `controller/events/simulation.rs` — applies transforms during simulation group building

A centralized `SetupTransform::apply_to_mesh()` / `apply_to_polygons()` would prevent
bugs like the double Z-flip issue we fixed in `project_curve.rs`.

### Compute Catalog vs Operation Configs

`catalog.rs` (static operation specs) and `operation_configs.rs` (runtime parameters)
have overlapping concerns. Both define per-operation metadata with large match statements.
The boundary should be documented:
- `catalog.rs` = immutable specs (labels, descriptions, geometry requirements, feed families)
- `operation_configs.rs` = user-configurable parameter structs

---

## 10. LOW: Minor Items

### ToolConfig mixes geometry + metadata

`compute/tool_config.rs` (176 lines) combines physical geometry (diameter, corner_radius,
taper_half_angle), feeds metadata (flute_count, tool_material), and G-code metadata
(tool_number) in one struct. Works fine today but makes it harder to version geometry
separately from operational parameters.

### No property-based testing framework

Comment in `property_tests.rs`: "Since proptest and rand are not available as
dev-dependencies." The property tests use hand-coded deterministic shapes instead of
randomized fuzzing. Adding proptest would improve geometric invariant coverage.

### No round-trip serialization tests

No test loads a TOML project file, saves it, loads it again, and compares. This is the
most dangerous gap in the project file code — silent data loss on format changes would
go undetected.

---

## 11. What's Working Well

### Compute Layer Ownership — Clean

Zero production code duplication between `core/compute` and `viz/compute`. The viz
compute layer is purely threading infrastructure and artifact writing. All 23 operations
dispatch through a single `execute_operation_annotated()` call in core. The production
code path is:

```
ComputeRequest → viz/worker → generate_via_core() → core/execute_operation_annotated()
```

### MCP Business Logic — None

Both MCP implementations correctly delegate all algorithm execution to `ProjectSession`.
No CAM algorithms leak into the MCP layer. The shared parsing utilities
(`parse_operation_type`, `parse_tool_type`) are correctly factored.

### Algorithm Test Coverage — Excellent

- 705 inline test functions across all algorithm files
- All 23 operations covered by parameter sweep tests (`param_sweep.rs`)
- Property tests validating geometric invariants across 8 polygon families
- End-to-end tests: STL→drop-cutter→G-code, SVG→pocket→G-code, simulation workflows
- All 6 tool types fully tested (radius, tip, cutting length, offsets)
- Dressup operations tested (ramp, helix, lead-in/out, feed optimization)
- No commented-out tests, no `#[ignore]`d tests, no timing-dependent flaky tests

### Clippy Compliance — Perfect

16 deny-level lints enforced at workspace level. Zero violations in production code.
Test modules carry appropriate `#[allow(...)]` annotations.

### Type Re-exports — Mostly Good

`viz/state/job.rs` properly re-exports core types (`ToolConfig`, `StockConfig`,
`FaceUp`, `ZRotation`, `AlignmentPin`, etc.) from `rs_cam_core::compute::*`. The only
remaining duplication is `LoadedModel`.

---

## 12. Recommended Fix Order

### Phase 1 — Session Integrity (blocks everything else)

1. **Add missing session methods**: `move_toolpath_to_setup()`, `set_face_selection()`,
   `rename_setup()`, fixture CRUD, keep-out CRUD
2. **Migrate GUI handlers**: Replace all `_mut()` accessor usage with session method calls
3. **Restrict `_mut()` accessors**: Make them `pub(crate)` or document them as
   "internal use only — prefer named mutation methods"

### Phase 2 — Test Coverage (highest risk reduction)

4. **Session mutation tests**: All CRUD operations, invalidation verification
5. **set_toolpath_param round-trip tests**: Serde edge cases, unknown params, type errors
6. **Project file round-trip tests**: Load→save→load→compare for all config types
7. **MCP tool handler tests**: At minimum smoke tests for all 42 handlers

### Phase 3 — Tracing (observability)

8. **Session layer**: `#[instrument]` on all public methods
9. **Algorithm entry points**: Replicate adaptive3d's tracing pattern
10. **Error paths**: `tracing::error!()` before returning errors, eliminate `.ok()` drops

### Phase 4 — Structural Cleanup

11. **Split oversized modules**: adaptive3d, adaptive, dexel_stock
12. **Deduplicate LoadedModel**: Viz wraps core type instead of redefining
13. **MCP parity**: Add missing tools to standalone server
14. **Centralize transforms**: SetupTransform utility methods

### Phase 5 — Polish

15. **Simulation group deduplication**: Extract shared setup→group logic
16. **Test helper cleanup**: Replace viz test helpers with core calls
17. **Add proptest**: Randomized geometric property testing
18. **Document compute module boundaries**: catalog.rs vs operation_configs.rs

---

## Appendix: File Index

### Session API
- `crates/rs_cam_core/src/session/mod.rs` — ProjectSession struct, queries, mutable accessors
- `crates/rs_cam_core/src/session/mutation.rs` — CRUD methods (add/remove/reorder toolpath, tool, setup)
- `crates/rs_cam_core/src/session/compute.rs` — set_toolpath_param, generate_toolpath, run_simulation
- `crates/rs_cam_core/src/session/project_file.rs` — TOML deserialization + model loading
- `crates/rs_cam_core/src/session/save.rs` — TOML serialization + atomic file write

### GUI Controller (bypass locations)
- `crates/rs_cam_viz/src/controller/events/mod.rs` — event dispatch, ToggleToolpathEnabled, ToggleFaceSelection
- `crates/rs_cam_viz/src/controller/events/model.rs` — tool/model/setup/fixture/keep-out handlers
- `crates/rs_cam_viz/src/controller/events/toolpath.rs` — add/remove/move/reorder toolpath handlers
- `crates/rs_cam_viz/src/controller/events/simulation.rs` — simulation group building
- `crates/rs_cam_viz/src/ui/properties/mod.rs` — stock/tool/machine property panels

### MCP
- `crates/rs_cam_mcp/src/server.rs` — standalone MCP server (42 tools, 1,173 LOC)
- `crates/rs_cam_viz/src/app/mcp.rs` — embedded MCP handler (1,873 LOC)
- `crates/rs_cam_viz/src/mcp_bridge.rs` — request/response channel types
- `crates/rs_cam_viz/src/mcp_server.rs` — embedded MCP server setup

### Oversized Files
- `crates/rs_cam_core/src/adaptive3d.rs` — 4,736 lines
- `crates/rs_cam_core/src/adaptive.rs` — 2,754 lines
- `crates/rs_cam_core/src/dexel_stock.rs` — 1,776 lines
- `crates/rs_cam_core/src/dexel_mesh.rs` — 1,165 lines
- `crates/rs_cam_core/src/simulation_cut.rs` — 1,220 lines
