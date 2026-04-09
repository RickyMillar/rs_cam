# Service Layer Extraction -- Fidelity Audit

**Baseline**: commit 443a613 (`/tmp/rs_cam_baseline/`)
**Current**: commit 127ff2c (`/home/ricky/personal_repos/rs_cam/`)
**Date**: 2026-04-08

## Summary

The refactoring moved operation execution, simulation, type definitions, and
helper functions from `rs_cam_viz` to `rs_cam_core`. The vast majority of the
transplant is faithful. Out of 23 operations, most have identical parameter
mapping. However, several behavioral differences were found -- two HIGH
severity, several MEDIUM, and a handful of LOW informational items.

**Findings**: 2 HIGH, 6 MEDIUM, 4 LOW

---

## HIGH Severity

### H1. DropCutter slope filtering dropped

**Old**: `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs:496-549`
The old `DropCutter` operation computed per-cell slope angles from the
drop-cutter grid and filtered out points outside the `[slope_from, slope_to]`
range. Points outside the range were skipped with retract/re-engage moves,
effectively limiting the 3D raster finish to a user-defined slope band.

**New**: `crates/rs_cam_core/src/compute/execute.rs:428-452`
The new code calls `raster_toolpath_from_grid` directly, ignoring the
`slope_from` and `slope_to` fields in `DropCutterConfig`. All cells are
processed regardless of slope angle.

**Impact**: Projects using DropCutter with non-default slope filtering
(`slope_from > 0` or `slope_to < 90`) will now cut areas they previously
skipped. The `DropCutterConfig` still serializes/deserializes these fields, so
saved projects retain the settings -- they are just silently ignored.

**Files**:
- Old: `/tmp/rs_cam_baseline/crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs:496-549`
- New: `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/compute/execute.rs:428-452`
- Config: `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/compute/operation_configs.rs` -- `DropCutterConfig` still has `slope_from` / `slope_to` fields

### Fix Plan

1. **What to change**:
   - `crates/rs_cam_core/src/dropcutter.rs` -- add a `compute_grid_slopes(grid: &DropCutterGrid) -> Vec<f64>` function (port from `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs:26-56`)
   - `crates/rs_cam_core/src/toolpath.rs` -- add a new `raster_toolpath_from_grid_filtered(grid, slope_angles, slope_from, slope_to, feed_rate, plunge_rate, safe_z) -> Toolpath` function that skips cells outside the slope range with retract/re-engage moves
   - `crates/rs_cam_core/src/compute/execute.rs:428-452` -- in the `DropCutter` match arm, after computing the grid, call `compute_grid_slopes` when `slope_from > 0.01 || slope_to < 89.99`, then call the filtered raster function instead of plain `raster_toolpath_from_grid`

2. **How to change it**:
   - Port `compute_grid_slopes` verbatim from `operations_3d.rs:26-56` into `dropcutter.rs`. It uses finite differences (dz/dx, dz/dy from neighbors) to compute per-cell slope angles, which only depends on `DropCutterGrid` (already in core).
   - Create `raster_toolpath_from_grid_filtered` by combining the logic from the old `operations_3d.rs:504-559` raster loop (which checks `slope_angles.get(idx)` before emitting each point, inserting retract/re-engage at slope-filtered gaps) with the existing `raster_toolpath_from_grid` structure in `toolpath.rs:288-325`.
   - In `execute.rs`, replace the current `raster_toolpath_from_grid` call with a branch: if slope filtering is active, use the new filtered function; otherwise use the existing unfiltered one.

3. **Dependencies**: None. Self-contained change.

4. **Estimated scope**: Medium (~100 lines). The `compute_grid_slopes` function is ~30 lines, the filtered raster builder is ~50 lines, and the dispatch change in `execute.rs` is ~10 lines.

5. **Risk**: Low. The slope computation is pure math on the existing `DropCutterGrid` struct. The only behavioral risk is if the old slope filtering had subtle bugs (e.g. boundary cells getting incorrect angles); the old code is well-tested and can be ported as-is. No impact on other operations.

### H2. HeightsConfig::resolve semantics changed

**Old**: `crates/rs_cam_viz/src/state/toolpath/support.rs:184-208`
```rust
let retract = self.retract_z.resolve_value(ctx.stock_top_z + ctx.safe_z, ctx);
let feed_ideal = top + 2.0;
let feed_default = if feed_ideal < retract { feed_ideal } else { (top + retract) / 2.0 };
// ...
top_z: self.top_z.resolve_value(ctx.stock_top_z, ctx),
bottom_z: self.bottom_z.resolve_value(ctx.stock_bottom_z, ctx),
```

**New**: `crates/rs_cam_core/src/compute/config.rs:177-186`
```rust
let retract = self.retract_z.resolve_value(ctx.safe_z, ctx);
ResolvedHeights {
    clearance_z: self.clearance_z.resolve_value(retract + 10.0, ctx),
    retract_z: retract,
    feed_z: self.feed_z.resolve_value(retract - 2.0, ctx),
    top_z: self.top_z.resolve_value(0.0, ctx),
    bottom_z: self.bottom_z.resolve_value(-ctx.op_depth.abs(), ctx),
}
```

**Differences**:
1. **retract_z auto default**: Old = `stock_top_z + safe_z` (absolute world Z). New = `safe_z` (raw value, no stock offset). For a stock at Z=25 with safe_z=10, old retract=35, new retract=10.
2. **feed_z auto default**: Old = `stock_top + 2.0` (with smart clamping). New = `retract - 2.0` (simple offset below retract).
3. **top_z auto default**: Old = `stock_top_z`. New = `0.0`.
4. **bottom_z auto default**: Old = `stock_bottom_z`. New = `-op_depth`.

**Impact**: Any code path using `HeightsConfig::resolve` with `Auto` mode will
produce different Z values. The new code uses a simpler coordinate system
(likely assuming stock top at Z=0), while the old code used absolute world
coordinates anchored to stock geometry. The new tests confirm the new behavior
(e.g. retract=10, top=0, bottom=-5), so this appears intentional but is a
breaking semantic change for callers migrated from the old system.

Also, `HeightContext::simple` changed: old set `stock_top_z = op_depth`, new
sets `stock_top_z = 0.0` and `stock_bottom_z = -op_depth`.

**Files**:
- Old: `/tmp/rs_cam_baseline/crates/rs_cam_viz/src/state/toolpath/support.rs:184-208`
- New: `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/compute/config.rs:177-186`

### Fix Plan

1. **What to change**:
   - `crates/rs_cam_core/src/compute/config.rs:177-186` -- `HeightsConfig::resolve()` method
   - `crates/rs_cam_core/src/compute/config.rs:92-100` -- `HeightContext::simple()` constructor

2. **How to change it**:
   This is the highest-risk finding because the new coordinate convention (stock top at Z=0) may be intentional for the new unified system. Two approaches:

   **Option A -- Restore old semantics** (if the old world-coordinate system is correct):
   - Change `retract_z` auto default from `ctx.safe_z` to `ctx.stock_top_z + ctx.safe_z`
   - Change `feed_z` auto default: compute `let feed_ideal = ctx.stock_top_z + 2.0; let feed_default = if feed_ideal < retract { feed_ideal } else { (ctx.stock_top_z + retract) / 2.0 };` and use that
   - Change `top_z` auto default from `0.0` to `ctx.stock_top_z`
   - Change `bottom_z` auto default from `-ctx.op_depth.abs()` to `ctx.stock_bottom_z`
   - Revert `HeightContext::simple` to set `stock_top_z = op_depth` instead of `0.0`

   **Option B -- Keep new semantics but document and audit callers** (if the new Z=0-origin convention is intentional):
   - Verify all callers of `HeightsConfig::resolve()` in both core and viz construct their `HeightContext` with the new convention (`stock_top_z = 0.0`)
   - Ensure the GUI's `HeightContext` builder matches -- search for all `HeightContext` constructions in `rs_cam_viz` and confirm they set `stock_top_z` and `stock_bottom_z` relative to Z=0
   - Add a doc comment on `HeightContext` explaining the coordinate convention
   - Update any tests that assumed old world-coordinate defaults

   **Recommendation**: Audit which callers exist first. If the GUI still passes world coordinates (e.g. `stock_top_z = 25.0`), then the old semantics are needed (Option A). If the GUI has been updated to pass `stock_top_z = 0.0`, the new semantics are correct and only documentation is needed (Option B).

3. **Dependencies**: Must be resolved before any other height-related fixes. All operations use `ResolvedHeights`, so this is foundational.

4. **Estimated scope**: Small for Option B (~20 lines of docs/audit). Medium for Option A (~50 lines of logic changes + test updates).

5. **Risk**: HIGH. Changing height resolution affects every single operation's Z coordinates. Incorrect fix could shift all cutting depths. Must be validated with a full parameter sweep (`cargo test --test param_sweep`) after any change. The existing core tests at `config.rs:370+` explicitly assert the new defaults (retract=10, top=0, bottom=-5), so Option A would require updating those tests to reflect world-coordinate expectations.

---

## MEDIUM Severity

### M1. Face operation: FaceDirection::OneWay silently ignored

**Old**: `crates/rs_cam_viz/src/compute/worker/execute/operations_2d.rs:436-451`
The old `FaceConfig::generate_with_tracing` used `zigzag_lines` + custom row
iteration, reversing odd-indexed lines when `direction == FaceDirection::OneWay`
to ensure all passes cut in the same direction.

**New**: `crates/rs_cam_core/src/compute/execute.rs:75-88`
The new code calls `crate::face::face_toolpath` which delegates to
`zigzag_toolpath`. The `direction` field is stored in `FaceParams` but never
read -- `face_toolpath` always produces zigzag (bidirectional) passes.

**Impact**: OneWay face cuts will produce bidirectional passes instead. The
extra rapid-return between rows (characteristic of one-way facing) is lost.

**Files**:
- Old: `/tmp/rs_cam_baseline/crates/rs_cam_viz/src/compute/worker/execute/operations_2d.rs:442-449`
- New: `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/face.rs:51-96` (FaceDirection never checked)
- New dispatch: `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/compute/execute.rs:75-88`

### Fix Plan

1. **What to change**:
   - `crates/rs_cam_core/src/face.rs:51-96` -- `face_toolpath()` function

2. **How to change it**:
   The `face_toolpath` function currently delegates to `zigzag_toolpath` which always produces alternating-direction rows. To support `FaceDirection::OneWay`:
   - After calling `zigzag_toolpath` (or `depth_stepped_toolpath` with `zigzag_toolpath` closure), post-process the result when `params.direction == FaceDirection::OneWay`
   - **Simpler approach**: Replace the `zigzag_toolpath` call with direct use of `zigzag_lines` (already public in `crate::zigzag`), then iterate rows manually. For `OneWay`, un-reverse odd rows by swapping endpoints (exactly as the old code did at `operations_2d.rs:448-450`: `if i % 2 != 0 { line.swap(0, 1); }`), then emit rapid-return moves between rows instead of continuous feed.
   - The depth-stepping branch (lines 73-94) needs the same treatment: wrap the `zigzag_toolpath` in a helper that respects direction.
   - Alternatively, add a `direction` field to `ZigzagParams` and handle it in `zigzag_toolpath` itself, but that would be a larger change affecting all zigzag callers.

3. **Dependencies**: None.

4. **Estimated scope**: Small (~40 lines). The core change is adding a conditional row-reversal and rapid-return insertion to `face_toolpath`, matching the old viz code's 8-line block.

5. **Risk**: Low. Only affects Face operations with `FaceDirection::OneWay`. The zigzag (default) path is unchanged. Add a test for one-way direction to `face.rs::tests` that verifies all cutting rows go in the same X direction.

### M2. Adaptive / Adaptive3D: initial_stock always None

**Old**: `crates/rs_cam_viz/src/compute/worker/execute/operations_2d.rs:143`
```rust
initial_stock: req.prior_stock.clone(),
```
**Old 3D**: `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs:165`
```rust
initial_stock: req.prior_stock.clone(),
```

**New**: `crates/rs_cam_core/src/compute/execute.rs:193,516`
```rust
initial_stock: None,
```

**Impact**: Both 2D Adaptive and 3D Adaptive3D operations no longer receive the
prior stock state. This means remaining-stock-aware clearing (where the
adaptive algorithm avoids re-cutting already-cleared areas) is disabled.
Projects using `StockSource::FromRemainingStock` for Adaptive operations will
see the algorithm start from scratch each time.

**Files**:
- Old 2D: `/tmp/rs_cam_baseline/crates/rs_cam_viz/src/compute/worker/execute/operations_2d.rs:143`
- Old 3D: `/tmp/rs_cam_baseline/crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs:165`
- New: `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/compute/execute.rs:193,516`

### Fix Plan

1. **What to change**:
   - `crates/rs_cam_core/src/compute/execute.rs:55-68` -- `execute_operation()` function signature: add an `initial_stock: Option<&TriDexelStock>` parameter
   - `crates/rs_cam_core/src/compute/execute.rs:193` -- Adaptive match arm: pass `initial_stock.cloned()` instead of `None`
   - `crates/rs_cam_core/src/compute/execute.rs:516` -- Adaptive3d match arm: pass `initial_stock.cloned()` instead of `None`
   - `crates/rs_cam_core/src/session.rs` -- `generate_all()` or `generate_single()`: pass `None` for now (the session API doesn't yet track per-toolpath prior stock state)
   - All callers of `execute_operation` in `rs_cam_viz` and `rs_cam_core` -- thread through the new parameter

2. **How to change it**:
   - Add `initial_stock: Option<&TriDexelStock>` to `execute_operation`'s parameter list (after `prev_tool_radius`). The `TriDexelStock` type is already in `crate::dexel_stock`.
   - In the `Adaptive` arm (line 193), change `initial_stock: None` to `initial_stock: initial_stock.cloned()`.
   - In the `Adaptive3d` arm (line 516), change `initial_stock: None` to `initial_stock.cloned()`.
   - Update all call sites to pass the correct value. The viz `ComputeRequest` already has `prior_stock: Option<TriDexelStock>`, so the viz worker can pass `req.prior_stock.as_ref()`. The CLI/session path should pass `None` until prior-stock tracking is implemented there.

3. **Dependencies**: None for the parameter threading. Full prior-stock support in `ProjectSession` (tracking stock state between sequential toolpath generations) is a separate, larger task.

4. **Estimated scope**: Small (~15 lines). Adding one parameter and changing two `None` literals to `initial_stock.cloned()`. The call-site updates are mechanical.

5. **Risk**: Low. Passing `None` preserves current behavior. Passing a valid stock only affects Adaptive operations where `StockSource::FromRemainingStock` is set. The `AdaptiveParams.initial_stock` and `Adaptive3dParams.initial_stock` already accept `Option<TriDexelStock>` -- this just threads the value through.

### M3. Waterline Z range: mesh bbox vs heights system

**Old**: `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs:218-225`
```rust
waterline_toolpath_with_cancel(
    mesh, &index, &cutter,
    req.heights.top_z,
    req.heights.bottom_z,
    cfg.z_step,
    ...
```

**New**: `crates/rs_cam_core/src/compute/execute.rs:544-553`
```rust
crate::waterline::waterline_toolpath_with_cancel(
    m, idx, tool_def,
    m.bbox.max.z,
    m.bbox.min.z,
    cfg.z_step,
    ...
```

**Impact**: The old waterline used the heights system (stock-relative, user-
configurable), but the new code uses the raw mesh bounding box. If a user set
custom top/bottom Z values via the heights system, those are now ignored --
waterline always covers the full mesh Z range.

**Files**:
- Old: `/tmp/rs_cam_baseline/crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs:218-225`
- New: `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/compute/execute.rs:544-553`

### Fix Plan

1. **What to change**:
   - `crates/rs_cam_core/src/compute/execute.rs:544-553` -- Waterline match arm

2. **How to change it**:
   Replace `m.bbox.max.z` / `m.bbox.min.z` with `heights.top_z` / `heights.bottom_z`:
   ```rust
   crate::waterline::waterline_toolpath_with_cancel(
       m,
       idx,
       tool_def,
       heights.top_z,    // was: m.bbox.max.z
       heights.bottom_z,  // was: m.bbox.min.z
       cfg.z_step,
       &params,
       &(|| cancel.load(Ordering::SeqCst)),
   )
   ```
   This restores the old behavior where waterline Z range comes from the heights system (`ResolvedHeights.top_z` / `bottom_z`), which is stock-relative and user-configurable. The `waterline_toolpath_with_cancel` signature already takes `start_z: f64, final_z: f64` -- no function signature changes needed.

3. **Dependencies**: Depends on H2 being resolved first. If the `HeightsConfig::resolve` defaults are wrong (H2), then `heights.top_z` and `heights.bottom_z` will be wrong here too. If H2 is confirmed as intentional (stock-top at Z=0), then `heights.top_z = 0.0` and `heights.bottom_z = -depth` should still be correct for waterline Z range relative to the mesh coordinate system.

4. **Estimated scope**: Small (~5 lines). Literally changing two arguments.

5. **Risk**: Low-Medium. If the mesh is positioned with its top surface at a Z value different from `heights.top_z`, the waterline range will be wrong. This is the same risk the old code had -- but the old code relied on callers setting heights correctly. Need to verify that the heights system is producing values consistent with the mesh coordinate space. A simple test: load a project with a waterline operation and compare the Z range used.

### M4. Dressup pipeline: air-cut filter and feed optimization removed

**Old**: `crates/rs_cam_viz/src/compute/worker/helpers.rs:253-343`
The old dressup pipeline had 7 steps + air-cut filtering:
1. Entry style
2. Dogbones
3. Lead in/out
4. Link moves
5. Arc fitting
6. **Air-cut filter** (removes toolpath segments that don't engage stock)
7. **Feed optimization** (stock-aware engagement-based feed rate adjustment)
8. TSP rapid ordering

**New**: `crates/rs_cam_core/src/compute/execute.rs:726-806`
The new pipeline has 6 steps:
1. Entry style
2. Dogbones
3. Lead in/out
4. Link moves
5. Arc fitting
6. TSP rapid ordering

Air-cut filtering and feed optimization are absent.

**Impact**:
- Air-cut filtering required a `prior_stock` reference, which the new unified
  `apply_dressups` doesn't receive.
- Feed optimization required a mutable heightmap and stock-aware parameters.
- Both features reduce cycle time for remaining-stock operations. Without them,
  toolpaths will contain unnecessary air-cutting moves and use constant feed
  rates.

**Files**:
- Old: `/tmp/rs_cam_baseline/crates/rs_cam_viz/src/compute/worker/helpers.rs:253-343`
- New: `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/compute/execute.rs:726-806`

### Fix Plan

1. **What to change**:
   - `crates/rs_cam_core/src/compute/execute.rs:726-806` -- `apply_dressups()` function: add air-cut filtering and feed optimization steps

2. **How to change it**:
   Both `filter_air_cuts` (in `crate::dressup`) and `optimize_feed_rates` (in `crate::feedopt`) already live in `rs_cam_core`. The functions are fully available -- they were just not wired into the core `apply_dressups`.

   **Air-cut filtering**:
   - Add `prior_stock: Option<&TriDexelStock>` parameter to `apply_dressups`
   - After the arc fitting step (step 5) and before TSP rapid ordering (step 6), add:
     ```rust
     if let Some(stock) = prior_stock {
         tp = crate::dressup::filter_air_cuts(tp, stock, tool_radius, safe_z, 0.1);
     }
     ```
   - The `DressupConfig` does not need a new field -- air-cut filtering is automatic when `prior_stock` is available (matching old behavior).

   **Feed optimization**:
   - Add `feed_optimization: bool`, `feed_max_rate: f64`, `feed_ramp_rate: f64` fields to `DressupConfig` (if not already present)
   - Add a `tool_config: &ToolConfig` parameter to `apply_dressups` (needed to build the cutter for feed optimization)
   - After air-cut filtering and before TSP ordering, add the feed optimization step. This requires building a `TriDexelStock` from the stock bbox, building a cutter from the tool config, and calling `crate::feedopt::optimize_feed_rates`.
   - The viz code at `helpers.rs:243-294` shows the full pattern including error handling via `feed_optimization_stock()`.

3. **Dependencies**: M2 (initial_stock threading) should land first, since air-cut filtering and initial_stock both depend on `prior_stock` being available in the core execution path.

4. **Estimated scope**: Medium (~80 lines). Air-cut filter integration is ~10 lines. Feed optimization is ~50 lines (stock construction, cutter building, error handling, the call). Parameter additions to `apply_dressups` signature and call-site updates are ~20 lines.

5. **Risk**: Medium. Feed optimization creates a temporary `TriDexelStock` which has memory/performance cost. The viz code has a `feed_optimization_unavailable_reason` guard that checks operation type and stock source before constructing the stock -- that guard logic would need to be replicated or exposed from core. Additionally, `apply_dressups` currently only takes `DressupConfig` + scalars; adding `prior_stock` and `tool_config` changes its signature at all call sites (session.rs, viz worker, tests).

### M5. Scallop ball-tip validation removed

**Old**: `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs:268-271`
```rust
if !req.tool.tool_type.has_ball_tip() {
    return Err(OperationError::InvalidTool(
        "Scallop operation requires a Ball Nose or Tapered Ball Nose tool".into(),
    ));
}
```

**New**: `crates/rs_cam_core/src/compute/execute.rs:574-591`
No tool-type validation. The scallop operation will proceed with any tool type.

**Impact**: Using a flat endmill or V-bit for scallop will fail deeper in the
core algorithm with a less informative error, rather than with a clear
`InvalidTool` error at the dispatch level.

**Files**:
- Old: `/tmp/rs_cam_baseline/crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs:268-271`
- New: `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/compute/execute.rs:574-591`

### Fix Plan

1. **What to change**:
   - `crates/rs_cam_core/src/compute/execute.rs:574-591` -- Scallop match arm

2. **How to change it**:
   Add the ball-tip validation check at the top of the `OperationConfig::Scallop` arm, before building params:
   ```rust
   OperationConfig::Scallop(cfg) => {
       if !tool_cfg.tool_type.has_ball_tip() {
           return Err(OperationError::InvalidTool(
               "Scallop operation requires a Ball Nose or Tapered Ball Nose tool".into(),
           ));
       }
       let m = require_mesh(mesh)?;
       // ... rest unchanged
   ```
   The `has_ball_tip()` method already exists on `ToolType` in `crate::compute::tool_config` and `tool_cfg` is available via the `tool_cfg: &ToolConfig` parameter.

3. **Dependencies**: None.

4. **Estimated scope**: Small (~5 lines). Just adding the guard clause.

5. **Risk**: Very low. This is a pure validation check that returns an error early. No impact on valid tool combinations. The same pattern is already used for VCarve, Inlay, and Chamfer tool-type validation in the same function.

### M6. Simulation: global stock uses zero-origin bbox (intentional but different)

**Old**: `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:147`
```rust
let mut global_stock = TriDexelStock::from_bounds(&req.stock_bbox, req.resolution);
```

**New**: `crates/rs_cam_core/src/compute/simulate.rs:197-205`
```rust
let global_bbox = BoundingBox3 {
    min: P3::new(0.0, 0.0, 0.0),
    max: P3::new(
        request.stock_bbox.max.x - request.stock_bbox.min.x,
        request.stock_bbox.max.y - request.stock_bbox.min.y,
        request.stock_bbox.max.z - request.stock_bbox.min.z,
    ),
};
let mut global_stock = TriDexelStock::from_bounds(&global_bbox, request.resolution);
```

**Impact**: The global stock grid now uses zero-origin coordinates instead of
world coordinates. The code comment says this is intentional ("local_to_global
returns stock-relative coordinates (0->stock_x, 0->stock_y, 0->stock_z),
NOT world coordinates with origin offsets"). However, this changes the
coordinate space of checkpoint stock data. The `checkpoints[].stock` will have
different coordinates than before, which could affect playback resume if the
GUI still expects world coordinates.

**Files**:
- Old: `/tmp/rs_cam_baseline/crates/rs_cam_viz/src/compute/worker/execute/mod.rs:147`
- New: `/home/ricky/personal_repos/rs_cam/crates/rs_cam_core/src/compute/simulate.rs:197-205`

### Fix Plan

1. **What to change**:
   - Likely no code change needed -- this may be intentional and correct.

2. **How to change it**:
   The zero-origin global stock was introduced deliberately (comment at `simulate.rs:194-196` explains: "local_to_global returns stock-relative coordinates"). The fix is to **verify that all consumers of checkpoint data use consistent coordinates**:
   - Search for all reads of `SimCheckpointMesh.stock` in viz code (playback resume, stock display, prior-stock construction)
   - Verify that the viz code transforms from zero-origin back to world coordinates when needed for display
   - If any consumer expects world coordinates, add a coordinate offset there (translate by `stock_bbox.min`)
   - Add a doc comment on `SimCheckpointMesh.stock` stating the coordinate convention: "Stock grid uses zero-origin coordinates (0 to stock_width, 0 to stock_height, 0 to stock_depth), not world coordinates."

3. **Dependencies**: None.

4. **Estimated scope**: Small (~10 lines of documentation + audit). If consumers need updating, Medium.

5. **Risk**: Low if this is confirmed as intentional (which the code comment suggests). The main risk is if the viz playback code builds a `prior_stock` from checkpoint data and passes it to operations that expect world coordinates -- but since M2/M4 show that prior_stock is not currently threaded through core, this is not an active bug yet. It becomes relevant once M2 and M4 are fixed.

---

## LOW Severity

### L1. Simulation: playback_data and cut_trace_path not in core result

**Old**: `SimulationResult` included `playback_data` (Vec of transformed
toolpaths + tool configs + directions) and `cut_trace_path` (filesystem path
for the written artifact).

**New**: Neither field exists on the core `SimulationResult`.

**Impact**: These are GUI-specific concerns. The GUI presumably builds its own
playback data from the core result. The cut trace artifact writing was moved
to the GUI layer. This is a correct architectural separation.

### Fix Plan

1. **What to change**: Nothing. This is working as designed.

2. **How to change it**: N/A. The GUI layer (`rs_cam_viz`) constructs playback data from the core `SimulationResult.boundaries` and `checkpoints`. The cut trace artifact is written by `helpers.rs:473-534` (`build_simulation_cut_artifact`). Both are GUI-layer concerns correctly separated from core.

3. **Dependencies**: None.

4. **Estimated scope**: None (no code change needed).

5. **Risk**: None.

### L2. Old entry-style dressup passed tool_radius as plunge_rate (old bug)

**Old**: `crates/rs_cam_viz/src/compute/worker/helpers.rs:165`
```rust
|tp| apply_entry(tp, core_entry, tool_radius),
```
The third argument to `apply_entry` is named `plunge_rate`, but the old code
passed `tool_radius` (which is typically 1-12mm vs plunge rate of 400-500 mm/min).

**New**: `crates/rs_cam_core/src/compute/execute.rs:757`
```rust
tp = apply_entry(tp, EntryStyle::Ramp { max_angle_deg: cfg.ramp_angle }, plunge_rate);
```
Correctly passes the plunge rate.

**Impact**: This is a bug fix. The old code's ramp/helix entries were using
the tool radius (a very small number like 3.175) as the plunge rate, producing
extremely slow entry moves. The new code uses the actual plunge rate.

### Fix Plan

1. **What to change**: Nothing in core. The core code is correct.

2. **How to change it**: The old viz code at `helpers.rs:135` passes `tool_radius` as the third arg to `apply_entry`, but the function's third parameter is `plunge_rate`. The core code at `execute.rs:757` correctly passes `plunge_rate`. **However**, the viz code at `helpers.rs:135` still has the old bug -- it should be fixed there too to ensure the viz worker path matches core behavior while the viz dressup path is still in use.
   - `crates/rs_cam_viz/src/compute/worker/helpers.rs:135` -- change `tool_radius` to the actual plunge rate (derive from `req` or the toolpath's feed rate)

3. **Dependencies**: None.

4. **Estimated scope**: Small (~2 lines in viz).

5. **Risk**: Very low. Fixes a longstanding bug. Entry moves will use a realistic plunge rate instead of an absurdly low value.

### L3. DropCutter min_z clamping added (improvement)

**Old**: Used `cfg.min_z` directly (default -50.0).
**New**: Clamps to `cfg.min_z.max(stock_bbox.min.z - 1.0)`.

**Impact**: Prevents the drop-cutter grid from generating points far below the
stock bottom, which previously caused simulation artifacts. This is an
improvement, not a regression.

### Fix Plan

1. **What to change**: Nothing. This is an improvement.

2. **How to change it**: N/A. The clamping `cfg.min_z.max(stock_bbox.min.z - 1.0)` at `execute.rs:436` prevents the drop-cutter grid from emitting points far below the stock, which caused `ray_subtract_above` to incorrectly destroy stock material during simulation. The 1mm margin below stock bottom ensures the grid still reaches the full stock depth.

3. **Dependencies**: None.

4. **Estimated scope**: None (no code change needed).

5. **Risk**: None.

### L4. CLI project.rs: complete rewrite using ProjectSession

**Old**: 2754 lines of inline TOML parsing, operation dispatch, simulation,
collision checking, and diagnostic output.

**New**: 337 lines delegating to `ProjectSession::load`, `generate_all`,
`run_simulation`, `collision_check`, and `diagnostics`.

**Impact**: The CLI now uses the same code paths as the GUI. All old CLI
features (per-toolpath JSON, simulation artifact, summary JSON, collision
checks, human-readable output) are preserved. The output format is identical
-- same JSON structure for diagnostics, summary, and simulation artifacts.

The old CLI had its own duplicate implementations of height resolution,
dressup application, and operation dispatch. These are now eliminated in favor
of the shared service layer. This is the intended architectural outcome.

### Fix Plan

1. **What to change**: Nothing. This is the intended architectural outcome.

2. **How to change it**: N/A. The CLI rewrite from 2754 lines to 337 lines using `ProjectSession` is correct and produces identical output. All old CLI features are preserved through the shared service layer.

3. **Dependencies**: None.

4. **Estimated scope**: None (no code change needed).

5. **Risk**: None. The CLI inherits fixes from the core service layer automatically.

---

## Type Mapping Verification

All 23 operation config types are present in both old and new codebases with
identical fields, serde attributes, and default values:

| Config | Fields Match | Serde Match | Defaults Match |
|--------|:-----------:|:-----------:|:--------------:|
| FaceConfig | YES | YES | YES |
| PocketConfig | YES | YES | YES |
| ProfileConfig | YES | YES | YES |
| AdaptiveConfig | YES | YES | YES |
| VCarveConfig | YES | YES | YES |
| RestConfig | YES | YES | YES |
| InlayConfig | YES | YES | YES |
| ZigzagConfig | YES | YES | YES |
| TraceConfig | YES | YES | YES |
| DrillConfig | YES | YES | YES |
| ChamferConfig | YES | YES | YES |
| AlignmentPinDrillConfig | YES | YES | YES |
| DropCutterConfig | YES | YES | YES |
| Adaptive3dConfig | YES | YES | YES |
| WaterlineConfig | YES | YES | YES |
| PencilConfig | YES | YES | YES |
| ScallopConfig | YES | YES | YES |
| SteepShallowConfig | YES | YES | YES |
| RampFinishConfig | YES | YES | YES |
| SpiralFinishConfig | YES | YES | YES |
| RadialFinishConfig | YES | YES | YES |
| HorizontalFinishConfig | YES | YES | YES |
| ProjectCurveConfig | YES | YES | YES |

Supporting types (enums, dressup config, boundary config, height system,
stock source, etc.) are all present with matching serde attributes.

**Project file compatibility**: A project file saved with the old code will
deserialize correctly with the new code. All serde tags, rename attributes,
and default values are preserved.

---

## Faithful Transplants (no issues)

The following operations were transplanted with identical logic:

- Pocket (contour and zigzag patterns, depth stepping)
- Profile (depth levels, tab application, retract between levels)
- VCarve (V-bit validation, half-angle calculation)
- Rest (previous tool radius validation, depth stepping)
- Inlay (female/male separation, V-bit validation)
- Zigzag (depth stepping)
- Trace (depth stepping, compensation)
- Drill (centroid calculation, cycle mapping)
- Chamfer (V-bit validation, half-angle calculation)
- AlignmentPinDrill (stock depth calculation, cycle mapping)
- Pencil (all parameters mapped)
- SteepShallow (all parameters mapped)
- RampFinish (all parameters mapped)
- SpiralFinish (all parameters mapped)
- RadialFinish (all parameters mapped)
- HorizontalFinish (all parameters mapped)
- ProjectCurve (direction mapping, per-polygon iteration)

Helper functions transplanted faithfully:
- `build_cutter` -- identical implementation
- `compute_stats` -- identical implementation
- Simulation core loop (per-setup stock, metric collection, rapid collision scan, deviation computation) -- structurally identical with the noted differences above
