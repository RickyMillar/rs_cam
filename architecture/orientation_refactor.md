# Orientation & Height System Refactor Design

## Problem Statement

The current orientation/height/simulation pipeline has ~100+ direction-dependent code paths spread across 4 phases (generation, simulation, rendering, IO). This has been a persistent source of bugs:

- Simulation stamping had `tip + h` vs `tip - h` wrong for bottom-entry tools
- 2D operations were hardcoded to Z=0 instead of using heights.top_z
- Trace had double depth-stepping (internal + external)
- Height context used global frame instead of setup-local frame

The root cause: **orientation awareness is scattered across the entire codebase** instead of being resolved once at the boundary.

## Current Architecture (What's Wrong)

```
User sets FaceUp::Bottom
    │
    ├─► compute.rs: transforms mesh/polygons to local frame
    ├─► compute.rs: builds HeightContext in local frame
    ├─► compute.rs: builds stock_bbox in local frame
    │       │
    │       ▼
    │   Operation generates toolpath in LOCAL frame
    │       │
    │       ▼
    ├─► simulation.rs: transforms toolpath BACK to global frame
    ├─► simulation.rs: maps FaceUp → StockCutDirection
    │       │
    │       ▼
    │   Simulation stamps with from_high branching in GLOBAL frame
    │       │
    │       ▼
    └─► gpu_upload.rs: 13 use_local_frame checks for rendering
```

**Three independent orientation representations:**
1. `FaceUp` + `ZRotation` (user-facing, 24 combinations)
2. `StockCutDirection` (simulation, 6 variants)
3. `from_high: bool` (stamping, 2 branches)

These are manually mapped (`face_up_to_direction`, `cuts_from_high_side`), creating coupling points where bugs hide.

## Target Architecture

### Core Principle: "Simulate in local frame, always from top"

```
User sets FaceUp::Bottom
    │
    ├─► compute.rs: transforms mesh/polygons to local frame (UNCHANGED)
    ├─► compute.rs: builds HeightContext in local frame (UNCHANGED)
    ├─► compute.rs: pre-computes cutting_levels as Vec<f64> (NEW)
    │       │
    │       ▼
    │   Operation generates toolpath in LOCAL frame (UNCHANGED)
    │       │
    │       ▼
    ├─► simulation.rs: simulates in LOCAL frame, always FromTop (NEW)
    │   - Each setup gets its own TriDexelStock in local coords
    │   - No toolpath transform needed (already in local frame)
    │   - No StockCutDirection branching
    │       │
    │       ▼
    │   Mesh extracted in local frame, transformed to global for compositing
    │       │
    │       ▼
    └─► gpu_upload.rs: renders from composited global mesh (SIMPLIFIED)
```

### What Changes

| Component | Current | Proposed |
|-----------|---------|----------|
| **Toolpath generation** | Local frame | Local frame (unchanged) |
| **Simulation stock** | Single global stock, 6 directions | Per-setup local stock, always FromTop |
| **Toolpath→sim transform** | local→global via inverse_transform | None needed (already local) |
| **Stamping math** | `if from_high { tip+h } else { tip-h }` | Always `tip+h` (FromTop) |
| **StockCutDirection** | 6 variants in hot path | Eliminated from stamping |
| **Mesh extraction** | Global frame | Per-setup local, transformed to global |
| **Depth stepping** | Per-operation construction | Pre-computed `cutting_levels: Vec<f64>` |
| **Height system** | Resolved in controller, consumed inconsistently | Resolved once, consumed as immutable levels |

## Detailed Design

### 1. Per-Setup Simulation

**Current** (`simulation.rs:build_simulation_groups`):
```rust
let stock_bbox = BoundingBox3::new(0,0,0, stock.x, stock.y, stock.z); // global
for setup in setups {
    let direction = face_up_to_direction(setup.face_up);
    let transformed = transform_toolpath_to_stock_frame(&tp, setup, stock); // local→global
    groups.push(SetupSimGroup { toolpaths, direction });
}
// Single sim with direction branching per group
```

**Proposed:**
```rust
for setup in setups {
    let (w, d, h) = setup.effective_stock(&stock);
    let local_bbox = BoundingBox3::new(0,0,0, w, d, h);
    // Toolpath already in local frame — no transform needed
    groups.push(SetupSimGroup {
        toolpaths,          // local frame, as-is
        stock_bbox: local_bbox,
        direction: StockCutDirection::FromTop, // always
    });
}
```

**In `run_simulation_with_phase`:**
```rust
for group in &req.groups {
    let mut group_stock = TriDexelStock::from_bounds(&group.stock_bbox, req.resolution);
    for tp in &group.toolpaths {
        group_stock.simulate_toolpath_with_lut_cancel(
            &tp.toolpath, &lut, radius,
            StockCutDirection::FromTop, // always
            &cancel,
        )?;
    }
    let group_mesh = dexel_stock_to_mesh(&group_stock);
    // Transform mesh to global frame for compositing
    let global_mesh = transform_stock_mesh(&group_mesh, &group.inverse_transform);
    composite_mesh(&mut final_mesh, &global_mesh);
}
```

### 2. Eliminate StockCutDirection from Stamping

Once all simulation uses FromTop, the `from_high` parameter becomes dead:

```rust
// Current: 3 stamping functions × 2 branches each = 6 paths
fn stamp_point_on_grid(grid, lut, radius, cu, cv, tip_depth, from_high: bool) {
    if from_high { ray_subtract_above(ray, tip_depth + h); }
    else         { ray_subtract_below(ray, tip_depth - h); }
}

// Proposed: 3 functions × 1 path each
fn stamp_point_on_grid(grid, lut, radius, cu, cv, tip_depth) {
    ray_subtract_above(ray, tip_depth + h);
}
```

`StockCutDirection` can be kept for the `decompose()` function (mapping XYZ to grid axes) but `cuts_from_high_side()` is eliminated from the hot path.

### 3. Unified Depth System

**Pre-compute cutting levels in the controller:**

```rust
// In ComputeRequest (new fields):
pub struct ComputeRequest {
    // ... existing fields ...
    pub cutting_levels: Vec<f64>,  // pre-computed absolute Z levels
}
```

**Build in controller:**
```rust
let cutting_levels = if operation.uses_depth_stepping() {
    let ds = DepthStepping {
        start_z: heights.top_z,
        final_z: heights.bottom_z,
        max_step_down: operation.depth_per_pass(),
        // ...
    };
    ds.all_levels()
} else {
    vec![]
};
```

**Consume uniformly in operations:**
```rust
// ALL 2D ops follow this pattern:
fn run_pocket(req: &ComputeRequest, cfg: &PocketConfig) -> Result<Toolpath> {
    let mut out = Toolpath::new();
    for (i, &z) in req.cutting_levels.iter().enumerate() {
        let tp = pocket_at_level(polygon, z, cfg);
        if i > 0 { out.final_retract(safe_z); }
        out.moves.extend(tp.moves);
    }
    Ok(out)
}
```

**Trace becomes a normal 2D op:**
```rust
// trace.rs: single-level function only
pub fn trace_ring_at_z(polygon: &Polygon2, z: f64, params: &TraceParams) -> Toolpath;

// operations_2d.rs: caller handles depth stepping
fn run_trace(req, cfg) {
    for &z in &req.cutting_levels {
        let tp = trace_ring_at_z(polygon, z, &params);
        out.moves.extend(tp.moves);
    }
}
```

### 4. Future Multi-Axis Extension

The per-setup-local-frame design naturally extends:

```rust
// Current: FaceUp enum (6 discrete orientations)
// Future: arbitrary orientation matrix
pub struct SetupOrientation {
    /// Transform from world to setup-local frame.
    /// For 3-axis: one of 24 discrete rotations (FaceUp × ZRotation).
    /// For 4th axis: rotation about A or B axis by arbitrary angle.
    /// For 5-axis: full rotation matrix.
    pub world_to_local: Matrix4,
    pub local_to_world: Matrix4,
}

// Simulation doesn't care — it always gets local-frame inputs
// The orientation only matters at the boundary (compute.rs)
```

## Migration Path

### Phase 1: Fix Trace (small, immediate)
- Remove internal DepthStepping from trace.rs
- Make trace a single-level function called from outer depth loop
- **Files:** trace.rs, operations_2d.rs
- **Risk:** Low — trace is isolated

### Phase 2: Pre-compute cutting_levels (medium)
- Add `cutting_levels: Vec<f64>` to ComputeRequest
- Build in controller after height resolution
- Migrate 2D ops one-by-one to iterate req.cutting_levels
- Remove make_depth/make_depth_with_finishing helpers
- **Files:** compute.rs (controller), worker.rs, operations_2d.rs, helpers.rs
- **Risk:** Medium — touches all 2D ops but each is independent

### Phase 3: Per-setup simulation (large, core refactor)
- Refactor simulation orchestration: per-group stock creation
- Remove transform_toolpath_to_stock_frame (toolpaths stay in local frame)
- Remove StockCutDirection from stamping functions (always FromTop)
- Add mesh compositing: per-group mesh → global frame → union
- Handle checkpoint/playback across setup boundaries
- **Files:** simulation.rs, dexel_stock.rs, execute/mod.rs, gpu_upload.rs
- **Risk:** High — core simulation change, needs thorough testing

### Phase 4: SetupOrientation matrix (future)
- Replace FaceUp + ZRotation with Matrix4
- Keep enum UI for common orientations, expose matrix for advanced
- Extend to 4th-axis rotary
- **Files:** job.rs, compute.rs, project.rs, setup UI
- **Risk:** Medium — mostly a representation change

## What We Keep

- `HeightsConfig` / `HeightMode` / `HeightReference` — user-facing height system (well-designed)
- `DepthStepping` — internal utility for computing Z levels (well-designed)
- `TriDexelStock` — the tri-dexel simulation engine (well-designed, just simplify its interface)
- Multi-setup project structure — setups, toolpaths, fixtures
- All existing operations — their core algorithms are frame-agnostic

## What We Remove

- `StockCutDirection::cuts_from_high_side()` — no more high/low branching
- `from_high: bool` parameter on stamping functions
- `transform_toolpath_to_stock_frame()` — toolpaths stay in local frame
- `face_up_to_direction()` — no mapping needed
- `make_depth()` / `make_depth_with_finishing()` — replaced by pre-computed levels
- Internal DepthStepping in trace.rs — trace becomes a single-level function
- 13 `use_local_frame` branches in gpu_upload.rs — simplified to per-setup mesh rendering

## Estimated Impact

| Metric | Current | After Refactor |
|--------|---------|----------------|
| Orientation-dependent code paths | ~100+ | ~30 (mostly FaceUp/ZRotation core) |
| Stamping branches per move | 2 (from_high) | 0 |
| Depth construction sites | 12 | 1 (controller) |
| Frame conversions per toolpath | 2 (local→global→sim) | 0 (stays local) |
| Lines of direction-branching code | ~150 | ~20 (decompose only) |

## Implementation Status

### Completed
- **Phase 1**: `trace_polygon_at_z` extracted as single-level function. `run_trace` uses `depth_stepped_toolpath` externally like other 2D ops.
- **Phase 2**: `cutting_levels: Vec<f64>` added to `ComputeRequest`, computed via `OperationConfig::cutting_levels(top_z)`. All 2D ops consume pre-computed levels. `make_depth`/`make_depth_with_finishing` helpers removed. `toolpath_at_levels` added to `depth.rs`.
- **Phase 3**: Per-setup simulation implemented. `SetupSimGroup` carries `local_stock_bbox` + `SetupTransformInfo`. `build_simulation_groups` passes toolpaths untransformed. `run_simulation_with_phase` creates per-group stocks, always stamps FromTop, transforms mesh vertices to global, composites via `StockMesh::append_transformed`. `face_up_to_direction()` and `transform_toolpath_to_stock_frame()` removed from controller.

### Partially implemented
- **StockCutDirection from stamping**: `from_high` branching remains in dexel_stock.rs stamping functions (kept for playback compatibility). Simulation always uses FromTop, but the code path still exists for playback re-simulation.
- **Playback**: Uses global-frame toolpaths + direction for backward scrub. Backward scrub always resets to fresh stock (no checkpoint optimization for multi-setup jobs) to avoid frame mismatches.

### Not yet started
- **Phase 4**: SetupOrientation matrix (future multi-axis extension)
- **Checkpoint optimization**: Per-setup checkpoints are stored but only used for mesh display, not for playback resume (frame mismatch). Future work: store global-frame checkpoint stocks or implement per-setup playback.
