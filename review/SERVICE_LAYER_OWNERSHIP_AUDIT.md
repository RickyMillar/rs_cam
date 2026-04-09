# Service Layer Ownership Audit

**Baseline:** commit 443a613 (pre-extraction, no `compute/` module in core)
**Current:** commit 127ff2c (post-extraction)
**Auditor:** automated code review
**Date:** 2026-04-08

---

## Executive Summary

The service layer extraction successfully completed **Phase 1** (config types to core), **Phase 3** (simulation to core), and a **partial Phase 2** (execution helpers to core). However, the critical Phase 2 goal -- "one `execute_toolpath()` function in core that both GUI and CLI call" -- was **not achieved**. The viz crate retains its own complete operation dispatch (~4300 lines) and does NOT delegate to core's `execute_operation()`. As a result, there are two parallel execution pipelines that can diverge.

**Findings by severity:**
- HIGH: 3
- MEDIUM: 4
- LOW: 2

---

## Findings

### HIGH-1: Dual Operation Execution Pipelines (not delegating)

**Severity:** HIGH
**Description:** The core has `execute_operation()` in `compute/execute.rs` (836 lines) that dispatches all 22 operations. The viz crate has its own parallel dispatch via the `SemanticToolpathOp` trait with 22 `generate_with_tracing()` implementations across `operations_2d.rs` (1174 lines) and `operations_3d.rs` (947 lines). The viz code does NOT call core's `execute_operation()` -- it reimplements the same operation dispatch with added semantic tracing and phase tracking.

This means:
- Bug fixes in one pipeline won't propagate to the other
- The core `DropCutter` clamps `min_z` to `stock_bbox.min.z - 1.0` (line 436); viz does not
- The core `Adaptive3d` uses `stock_bbox.max.z` for `stock_top_z`; viz falls back to `cfg.stock_top_z` when `stock_bbox` is absent
- Core's Adaptive passes `initial_stock: None`; viz passes `req.prior_stock.clone()`
- Any new parameter or behavioral fix must be applied in both places

**Files:**
- `crates/rs_cam_core/src/compute/execute.rs` (core's dispatch -- used by `session.rs` only)
- `crates/rs_cam_viz/src/compute/worker/execute/operations_2d.rs` (viz's dispatch -- used by GUI)
- `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs` (viz's dispatch -- used by GUI)
- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` (viz's `semantic_op()` + `run_compute()`)

**Recommended action:** Complete Phase 6 from `SERVICE_LAYER_EXTRACTION.md`: make viz call `rs_cam_core::compute::execute_toolpath()` for operation dispatch, keeping only threading, phase tracking, file I/O, and semantic annotation wrapping in viz. The existing `run_pocket()`, `run_profile()`, etc. functions (many already marked `#[allow(dead_code)]`) should be removed once delegation is complete.

### Fix Plan

**What to change:**

The viz compute pipeline currently dispatches operations through the `SemanticToolpathOp` trait (22 impls across `operations_2d.rs` and `operations_3d.rs`) which call core algorithm functions directly. This needs to be replaced with delegation to `rs_cam_core::compute::execute::execute_operation()`.

Files to modify:
- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` -- change `run_compute_with_phase_tracker()` to call `rs_cam_core::compute::execute::execute_operation()` for the raw toolpath, then wrap the result with semantic annotation post-hoc.
- `crates/rs_cam_viz/src/compute/worker/execute/operations_2d.rs` -- eventually delete most of this file; keep only `run_profile()` (used by tests) and semantic annotation functions.
- `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs` -- eventually delete most of this file; keep only annotation functions.

**How to change it:**

1. Add a bridge function in `execute/mod.rs` that converts viz's `ComputeRequest` into the arguments needed by `rs_cam_core::compute::execute::execute_operation()`. The mapping is:
   - `req.mesh.as_deref()` -> `mesh: Option<&TriangleMesh>`
   - Build `SpatialIndex` from mesh -> `index: Option<&SpatialIndex>`
   - `req.polygons.as_deref()` and flatten -> `polygons: Option<&[Polygon2]>`
   - `req.tool` -> convert via `build_cutter()` -> `tool_def: &ToolDefinition`
   - `req.tool` -> `tool_cfg: &ToolConfig` (already the same type after re-export)
   - `req.heights` -> `heights: &ResolvedHeights`
   - `req.cutting_levels` -> `cutting_levels: &[f64]`
   - `req.stock_bbox` -> `stock_bbox: &BoundingBox3`
   - `req.prev_tool_radius` -> `prev_tool_radius: Option<f64>`
   - Thread debug context through -> `debug_ctx: Option<&ToolpathDebugContext>`
   - Thread cancel flag through -> `cancel: &AtomicBool`

2. In `run_compute_with_phase_tracker()`, replace the current `semantic_op(&req.operation).generate_with_tracing(&exec_ctx)?` call with:
   ```
   let tp = rs_cam_core::compute::execute::execute_operation(...)?;
   ```

3. After getting the raw toolpath from core, apply semantic annotations post-hoc. The current `SemanticToolpathOp` impls interleave generation and annotation; the annotation logic needs to be factored into standalone "annotate after the fact" functions. Many operations (Adaptive3d, Pencil, Scallop, RampFinish, SpiralFinish) already have `_annotated` variants that return `(Toolpath, Vec<RuntimeAnnotation>)` -- core's `execute_operation` would need to return annotations alongside the toolpath, or viz calls the `_structured_annotated` variants directly through a separate path.

4. The hardest part: core's `execute_operation()` currently returns a plain `Toolpath`, discarding structured annotations. To preserve viz's rich semantic tracing, either:
   - **Option A (recommended):** Extend core's `execute_operation()` to accept an optional `ToolpathDebugContext` + `ToolpathSemanticContext` and thread them into operations that support structured annotations. Return an `ExecutionResult { toolpath, annotations }` struct.
   - **Option B:** Have viz call `execute_operation()` for the raw toolpath, then re-derive annotations by calling the `_structured_annotated` variant a second time (wasteful, not recommended).
   - **Option C (incremental):** For operations that don't need rich semantic annotation (most 2D ops, simpler 3D ops), delegate to core immediately. For complex annotated operations (Adaptive3d, Pencil, Scallop, etc.), keep calling the `_structured_annotated_traced_with_cancel` variants directly from viz but remove the `SemanticToolpathOp` trait indirection. This closes the behavioral divergence without requiring core API changes.

5. Fix the specific divergences documented in this finding:
   - Core DropCutter clamps `min_z` to `stock_bbox.min.z - 1.0`; viz does not. Add the same clamping to core (already there) and remove viz's raw grid call.
   - Core Adaptive3d uses `stock_bbox.max.z` for `stock_top_z`; viz falls back to `cfg.stock_top_z`. Add the same fallback to core.
   - Core Adaptive passes `initial_stock: None`; viz passes `req.prior_stock.clone()`. Add an `initial_stock` parameter to core's `execute_operation()`.

**Dependencies:** HIGH-2 (dual dressups) and HIGH-3 (dual OperationError) should land first or concurrently, since the rewired path will need a unified dressup pipeline and error type.

**Estimated scope:** Large (200+ lines of new bridge code, 2000+ lines of deleted viz dispatch code, net reduction ~1500 lines).

**Risk:**
- Semantic tracing regressions: the GUI's toolpath inspection panel relies on the rich annotation tree built by `SemanticToolpathOp` impls. If annotations are applied post-hoc instead of inline, the tree structure or move-range bindings could shift.
- Test breakage: `execute/mod.rs` has integration tests that use `run_compute()` directly. The `run_profile()` and `run_inlay()` functions are imported for tests (`#[cfg(test)]`); those tests need to be updated to use the new delegation path.
- Behavioral changes: the DropCutter `min_z` clamping and Adaptive3d `stock_top_z` fallback differences mean that GUI output will change slightly after unification. This is a correctness fix but may surprise users.

---

### HIGH-2: Dual `apply_dressups()` Implementations

**Severity:** HIGH
**Description:** Both crates have their own `apply_dressups()` with different signatures, capabilities, and step ordering:

| Feature | Core (`execute.rs:726`) | Viz (`helpers.rs:96`) |
|---------|------------------------|----------------------|
| Entry style | Direct match on enum | Uses `cfg.entry_style.to_core(cfg)` helper |
| Air-cut filter | Not included | Included (step between arc fitting and feed opt) |
| Feed optimization | Not included | Included (with prior_stock heightmap) |
| Debug/semantic tracing | None | Full tracing via `apply_dressup_with_tracing()` |
| Plunge rate | Inferred from toolpath moves | Inferred from toolpath moves |

Core's version has 6 steps. Viz's version has 8 steps (adding air-cut filtering and feed optimization). Any dressup fix applied to one version will be missed in the other.

**Files:**
- `crates/rs_cam_core/src/compute/execute.rs:726-806`
- `crates/rs_cam_viz/src/compute/worker/helpers.rs:96-314`

**Recommended action:** Move air-cut filtering and feed optimization into core's `apply_dressups()` (or a richer `apply_dressups_full()` variant). Viz should wrap core's function with tracing only.

### Fix Plan

**What to change:**

- `crates/rs_cam_core/src/compute/execute.rs` -- expand `apply_dressups()` (line 726) to include air-cut filtering and feed optimization steps.
- `crates/rs_cam_viz/src/compute/worker/helpers.rs` -- reduce `apply_dressups()` (line 96) to a thin wrapper that calls core's expanded version, adding only tracing/semantic annotations.

**How to change it:**

1. Extend core's `apply_dressups()` signature to accept optional prior stock and feed optimization parameters:
   ```rust
   pub fn apply_dressups(
       mut tp: Toolpath,
       cfg: &DressupConfig,
       tool_cfg: &ToolConfig,
       safe_z: f64,
       prior_stock: Option<&TriDexelStock>,
       feed_opt_stock: Option<&mut TriDexelStock>,
   ) -> Toolpath
   ```

2. Add air-cut filtering (step 6, after arc fitting) to core's pipeline. The `filter_air_cuts` function already lives in `rs_cam_core::dressup` -- it just needs to be called conditionally when `prior_stock` is `Some`:
   ```rust
   // 6. Air-cut filter (when prior stock available)
   if let Some(prior) = prior_stock {
       tp = filter_air_cuts(tp, prior, tool_radius, safe_z, 0.1);
   }
   ```

3. Add feed optimization (step 7, after air-cut filtering) to core's pipeline. The `optimize_feed_rates` function already lives in `rs_cam_core::feedopt`:
   ```rust
   // 7. Feed optimization (when stock available)
   if cfg.feed_optimization {
       if let Some(stock) = feed_opt_stock {
           let nominal = /* extract from toolpath */;
           let cutter = build_cutter(tool_cfg);
           let params = FeedOptParams { ... };
           tp = optimize_feed_rates(&tp, &cutter, stock, &params);
       }
   }
   ```

4. The `feed_optimization_unavailable_reason()` check currently lives in viz's `helpers.rs::feed_optimization_stock()`. Move the stock-building logic to core (or accept a pre-built stock as a parameter, which is cleaner).

5. In viz's `helpers.rs`, replace the 8-step `apply_dressups()` with:
   ```rust
   pub(super) fn apply_dressups(tp: Toolpath, req: &ComputeRequest, debug, semantic) -> Toolpath {
       let stock = feed_optimization_stock(req).ok();
       let mut stock_mut = stock;
       let result = rs_cam_core::compute::execute::apply_dressups(
           tp, &req.dressups, &req.tool, safe_z,
           req.prior_stock.as_ref(), stock_mut.as_mut(),
       );
       // Wrap with tracing: rebuild semantic trace items by
       // comparing before/after toolpath state for each enabled step.
       result
   }
   ```
   However, the tracing wrapper is complex -- `apply_dressup_with_tracing()` needs before/after move ranges for each step. Two approaches:
   - **Option A (recommended):** Have core's `apply_dressups()` return a `DressupReport` listing which steps ran and their move-range effects, then viz builds semantic scopes from the report.
   - **Option B:** Have core accept a trait object `DressupObserver` with callbacks for each step start/end, which viz implements to drive semantic tracing.

**Dependencies:** None. This fix is independent and can land before HIGH-1.

**Estimated scope:** Medium (50-200 lines). Core changes are ~30 lines of new code in `apply_dressups()`. Viz changes are ~50 lines to rewire the wrapper. The tracing approach (Option A or B) adds ~40-80 lines.

**Risk:**
- Feed optimization requires a `&mut TriDexelStock`, which is a heavier dependency than the current core `apply_dressups()` has. The `TriDexelStock` and `FeedOptParams` types need to be imported into `execute.rs`.
- The `feed_optimization_unavailable_reason()` function references `StockSource`, which is a config type already in core, so no new dependencies there.
- Behavioral change for CLI/session users: they would now get air-cut filtering and feed optimization that they previously lacked. This is the desired outcome, but if the prior stock is not available in the session path, these steps simply won't fire (the parameters are optional).

---

### HIGH-3: Duplicated `OperationError` Enum

**Severity:** HIGH
**Description:** `OperationError` is defined identically in two places:
- `rs_cam_core::compute::execute::OperationError`
- `rs_cam_viz::compute::OperationError`

Both have the same four variants (`MissingGeometry`, `InvalidTool`, `Cancelled`, `Other`). The viz version adds `From<String>` and `From<OperationError> for ComputeError` impls. The viz operations import `crate::compute::OperationError` (the viz one), not core's.

**Files:**
- `crates/rs_cam_core/src/compute/execute.rs:22-45`
- `crates/rs_cam_viz/src/compute/mod.rs:82-120`

**Recommended action:** Delete the viz definition. Have viz re-export `rs_cam_core::compute::execute::OperationError` (or via `rs_cam_core::compute::OperationError`) and add the `From` impls as extension traits or move them to core.

### Fix Plan

**What to change:**

- `crates/rs_cam_core/src/compute/execute.rs` (lines 22-45) -- add `From<String>` impl to core's `OperationError`.
- `crates/rs_cam_core/src/compute/mod.rs` -- add `pub use execute::OperationError;` re-export.
- `crates/rs_cam_viz/src/compute/mod.rs` (lines 82-120) -- delete the duplicate `OperationError` enum and its impls. Replace with `pub use rs_cam_core::compute::OperationError;`. Keep the `From<OperationError> for ComputeError` impl (it references viz's `ComputeError` which stays in viz).
- `crates/rs_cam_viz/src/compute/worker/execute/operations_2d.rs` (line 14) -- update `use crate::compute::OperationError;` -- this will now resolve to the re-exported core type, no change needed if the re-export is in the same module path.
- `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs` (line 19) -- same as above.
- `crates/rs_cam_viz/src/compute/worker/helpers.rs` (line 7) -- same as above.

**How to change it:**

1. In `crates/rs_cam_core/src/compute/execute.rs`, add after line 45:
   ```rust
   impl From<String> for OperationError {
       fn from(s: String) -> Self {
           Self::Other(s)
       }
   }
   ```

2. In `crates/rs_cam_core/src/compute/mod.rs`, add to the Phase 2 re-exports section (around line 55):
   ```rust
   pub use execute::OperationError;
   ```

3. In `crates/rs_cam_viz/src/compute/mod.rs`, delete lines 82-111 (the `OperationError` enum, `Display`, `Error`, and `From<String>` impls). Replace with:
   ```rust
   pub use rs_cam_core::compute::OperationError;
   ```
   Keep lines 113-120 (the `From<OperationError> for ComputeError` impl) since `ComputeError` is viz-local.

4. Verify that all viz files importing `crate::compute::OperationError` continue to compile -- they should, since the re-export preserves the same module path.

**Dependencies:** None. This is the simplest of the three HIGH fixes and should land first.

**Estimated scope:** Small (< 50 lines). ~5 lines added to core, ~30 lines deleted from viz, ~2 lines added to viz.

**Risk:**
- Very low risk. The types are identical. The only behavioral difference is that core's `OperationError` gains `From<String>`, which is a strictly additive change.
- Downstream crates that match on `OperationError` variants will continue to work since the variants are identical.

---

### MEDIUM-1: Duplicated Simulation Types (Intentional Thin Wrappers)

**Severity:** MEDIUM
**Description:** Several simulation-related types exist in both crates:

| Type | Core location | Viz location | Difference |
|------|--------------|-------------|------------|
| `SimulationRequest` | `compute/simulate.rs:53` | `compute/worker.rs:133` | Viz uses `SetupSimGroup`/`SetupSimToolpath` (with `ToolpathId` and `ToolConfig`); core uses `SimGroupEntry`/`SimToolpathEntry` (with `usize` id and `ToolDefinition`) |
| `SimulationResult` | `compute/simulate.rs:85` | `compute/worker.rs:162` | Viz adds `playback_data` and `cut_trace_path` fields |
| `SimBoundary` | `compute/simulate.rs:67` | `compute/worker.rs:146` | Viz uses `ToolpathId`; core uses `usize` |
| `SimCheckpointMesh` | `compute/simulate.rs:78` | `compute/worker.rs:156` | Identical structure |

The viz simulation properly **delegates** to core via `build_core_simulation_request()` and `simulate::run_simulation_with_phase()`, then converts the results back. This is the intended pattern. However, the conversion layer could be thinner.

**Files:**
- `crates/rs_cam_core/src/compute/simulate.rs`
- `crates/rs_cam_viz/src/compute/worker.rs:133-179`
- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:119-265`

**Recommended action:** Consider whether viz can use core's types directly with `ToolpathId(usize)` wrapping at the boundary, rather than maintaining parallel structs. `SimCheckpointMesh` is identical and should be re-exported.

### Fix Plan

**What to change:**

- `crates/rs_cam_viz/src/compute/worker.rs` (lines 156-160) -- delete viz's `SimCheckpointMesh` and re-export core's.
- `crates/rs_cam_viz/src/compute/worker.rs` (lines 146-154) -- keep viz's `SimBoundary` (it uses `ToolpathId` instead of `usize`, which is a meaningful type distinction).
- `crates/rs_cam_viz/src/compute/worker.rs` (lines 162-179) -- keep viz's `SimulationResult` (it has extra fields: `playback_data`, `cut_trace_path`).
- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` (lines 222-231) -- simplify checkpoint conversion to a direct passthrough.

**How to change it:**

1. Delete the viz `SimCheckpointMesh` struct (worker.rs lines 156-160) and replace with:
   ```rust
   pub use rs_cam_core::compute::simulate::SimCheckpointMesh;
   ```

2. In `run_simulation_with_phase()` (execute/mod.rs lines 222-231), remove the checkpoint conversion loop. Since the types are now identical, replace:
   ```rust
   let checkpoints: Vec<SimCheckpointMesh> = core_result.checkpoints
       .into_iter()
       .map(|cp| SimCheckpointMesh { boundary_index: cp.boundary_index, mesh: cp.mesh, stock: cp.stock })
       .collect();
   ```
   with:
   ```rust
   let checkpoints = core_result.checkpoints;
   ```

3. For `SimBoundary`, the `ToolpathId` vs `usize` distinction is intentional (type safety at the viz level). Keep the conversion. However, consider adding a `From<usize>` impl to `ToolpathId` if it doesn't already exist, to simplify the mapping.

4. For `SimulationRequest`, the viz version uses `SetupSimGroup`/`SetupSimToolpath` which carry `ToolConfig` and `ToolpathId` -- these are viz/UI-level types. The conversion via `build_core_simulation_request()` is the correct pattern. No change needed.

**Dependencies:** None.

**Estimated scope:** Small (< 50 lines). Delete ~5 lines of struct definition, simplify ~10 lines of conversion code.

**Risk:**
- Very low. The structs are structurally identical. Any code that accesses `SimCheckpointMesh` fields will work the same way.
- The re-export changes the fully-qualified type path, which could affect downstream type inference in rare cases, but since viz already re-exports the struct through `compute::worker`, consumers won't notice.

---

### MEDIUM-2: Viz DropCutter Has Slope Filtering Not in Core

**Severity:** MEDIUM
**Description:** The viz `SemanticToolpathOp for DropCutterConfig` implementation (operations_3d.rs:457-589) includes slope-based filtering via `compute_grid_slopes()` that the core `execute_operation` for DropCutter does NOT have. This means:
- CLI/session-based DropCutter will NOT respect `slope_from`/`slope_to` parameters
- The viz-only `compute_grid_slopes()` function (lines 26-64) performs finite-difference slope estimation that should be a core capability

**Files:**
- `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs:26-64` (slope computation)
- `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs:497-549` (slope filtering in DropCutter)
- `crates/rs_cam_core/src/compute/execute.rs:428-452` (core DropCutter, no slope filtering)

**Recommended action:** Move `compute_grid_slopes()` and the slope-filtering raster logic to core's DropCutter implementation so CLI gets the same behavior.

### Fix Plan

**What to change:**

- `crates/rs_cam_core/src/dropcutter.rs` -- add `compute_grid_slopes()` as a public method on `DropCutterGrid`.
- `crates/rs_cam_core/src/toolpath.rs` -- modify `raster_toolpath_from_grid()` (line 288) to accept optional slope filtering parameters, or add a new `raster_toolpath_from_grid_filtered()` variant.
- `crates/rs_cam_core/src/compute/execute.rs` (lines 428-452) -- update the `DropCutter` dispatch to use slope filtering when `slope_from`/`slope_to` are active.
- `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs` (lines 26-64, 456-589) -- delete `compute_grid_slopes()` and simplify the `SemanticToolpathOp for DropCutterConfig` to delegate to core.

**How to change it:**

1. Move `compute_grid_slopes()` from viz `operations_3d.rs` (lines 26-64) to `crates/rs_cam_core/src/dropcutter.rs` as a method on `DropCutterGrid`:
   ```rust
   impl DropCutterGrid {
       /// Compute per-cell slope angle (degrees from horizontal) using finite differences.
       pub fn compute_slopes(&self) -> Vec<f64> { ... }
   }
   ```
   The implementation is self-contained -- it only references `self.rows`, `self.cols`, `self.get()`, `self.x_step`, `self.y_step`. No additional dependencies.

2. Add a new function `raster_toolpath_from_grid_with_slope_filter()` in `crates/rs_cam_core/src/toolpath.rs` that accepts slope bounds:
   ```rust
   pub fn raster_toolpath_from_grid_with_slope_filter(
       grid: &DropCutterGrid,
       feed_rate: f64,
       plunge_rate: f64,
       safe_z: f64,
       slope_from: f64,
       slope_to: f64,
   ) -> Toolpath
   ```
   This function integrates the row-by-row rasterization with slope-based point skipping (the logic currently at viz operations_3d.rs lines 514-583). When `slope_from <= 0.01 && slope_to >= 89.99`, it falls back to the existing `raster_toolpath_from_grid()` with no filtering.

3. Update core's `execute_operation()` DropCutter arm (execute.rs line 428) to call the new slope-filtered variant:
   ```rust
   OperationConfig::DropCutter(cfg) => {
       // ... existing grid computation ...
       Ok(raster_toolpath_from_grid_with_slope_filter(
           &grid, feed_rate, plunge_rate, safe_z,
           cfg.slope_from, cfg.slope_to,
       ))
   }
   ```

4. In viz, simplify the `SemanticToolpathOp for DropCutterConfig` to delegate to core (or call the new core function directly), keeping only the semantic annotation wrapping (row scopes, param logging).

**Dependencies:** None. This fix is independent.

**Estimated scope:** Medium (50-200 lines). ~60 lines moved from viz to core (`compute_grid_slopes` + slope-filtered raster logic). ~30 lines of new function signatures and wiring. ~100 lines deleted from viz's DropCutter impl (replaced by delegation).

**Risk:**
- The slope filtering logic uses `#[allow(clippy::indexing_slicing)]` because it indexes into the grid by `row * cols + col`. The core version needs the same allow or should use `.get()` with bounds checks. Since `DropCutterGrid::get()` already exists and handles bounds, prefer using it.
- The viz DropCutter impl builds semantic row-scope annotations inline during rasterization. If core produces the toolpath without these annotations, viz would need to annotate the result post-hoc (labeling row boundaries by detecting retract-rapid patterns in the output). This is slightly less precise but acceptable for a finish operation.
- CLI users will now see slope filtering applied, which is the desired behavior since `slope_from`/`slope_to` already exist on the config type.

---

### MEDIUM-3: Viz-Only `require_polygons`/`require_mesh` Helpers

**Severity:** MEDIUM
**Description:** Both crates have `require_polygons()` and `require_mesh()` helpers, but with different signatures:
- Core: `require_polygons(polygons: Option<&[Polygon2]>)` and `require_mesh(mesh: Option<&TriangleMesh>)`
- Viz: `require_polygons(req: &ComputeRequest)` and `require_mesh(req: &ComputeRequest)` (also builds `SpatialIndex`)

The viz versions operate on the viz `ComputeRequest` struct, while core operates on raw `Option` values. This is a natural consequence of not sharing the `ComputeRequest` type.

**Files:**
- `crates/rs_cam_core/src/compute/execute.rs:810-818`
- `crates/rs_cam_viz/src/compute/worker/helpers.rs:20-37`

**Recommended action:** Once Phase 6 (GUI rewire) is complete and viz delegates to core, these viz helpers become unnecessary.

### Fix Plan

**What to change:**

- `crates/rs_cam_viz/src/compute/worker/helpers.rs` (lines 20-37) -- delete `require_polygons()` and `require_mesh()` once viz delegates to core.

**How to change it:**

1. This fix is a natural consequence of HIGH-1. Once viz's `run_compute_with_phase_tracker()` calls `rs_cam_core::compute::execute::execute_operation()`, the viz-side `require_polygons(req)` and `require_mesh(req)` helpers are no longer called -- core's `execute_operation()` uses its own `require_polygons(polygons)` and `require_mesh(mesh)` internally.

2. After HIGH-1 lands, grep for remaining call sites of the viz helpers:
   ```
   grep -rn "require_polygons\|require_mesh" crates/rs_cam_viz/src/compute/
   ```
   Delete the functions from `helpers.rs` if no callers remain.

3. If some callers remain in test code, either inline the logic or have tests use core's versions directly.

**Dependencies:** HIGH-1 must land first.

**Estimated scope:** Small (< 50 lines). Delete ~20 lines of helper functions and update any remaining imports.

**Risk:** None. These are internal helper functions with no public API surface.

---

### MEDIUM-4: Dead Code Accumulation in Viz Operations

**Severity:** MEDIUM
**Description:** Multiple operation runner functions in viz are marked `#[allow(dead_code)]` because they are superseded by the `SemanticToolpathOp` trait implementations:
- `run_pocket()` (operations_2d.rs:17)
- `run_drill()` (operations_2d.rs:332)
- `run_dropcutter()` (operations_3d.rs:78)
- `run_waterline()` (operations_3d.rs:196)
- `run_simulation()` (execute/mod.rs:109)

These dead functions duplicate logic that exists both in the `SemanticToolpathOp` impls (used in practice) and in core's `execute_operation()`. They add maintenance burden and confusion about which code path is actually executing.

**Files:**
- `crates/rs_cam_viz/src/compute/worker/execute/operations_2d.rs` (lines 17, 332)
- `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs` (lines 78, 196)
- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` (line 109)

**Recommended action:** Remove the dead `run_*` functions. They are not called and serve no purpose once the `SemanticToolpathOp` dispatch is the live path.

### Fix Plan

**What to change:**

- `crates/rs_cam_viz/src/compute/worker/execute/operations_2d.rs` -- delete `run_pocket()` (line 17, marked `#[allow(dead_code)]`), `run_drill()` (line 332, marked `#[allow(dead_code)]`).
- `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs` -- delete `run_dropcutter()` (line 78, marked `#[allow(dead_code)]`), `run_waterline()` (line 196, marked `#[allow(dead_code)]`).
- `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` -- delete `run_simulation()` (line 109, marked `#[allow(dead_code)]`).

**How to change it:**

1. Before deleting, verify each function truly has zero callers:
   ```
   grep -rn "run_pocket\b" crates/rs_cam_viz/src/
   grep -rn "run_drill\b" crates/rs_cam_viz/src/
   grep -rn "run_dropcutter\b" crates/rs_cam_viz/src/
   grep -rn "run_waterline\b" crates/rs_cam_viz/src/
   grep -rn "run_simulation\b" crates/rs_cam_viz/src/
   ```
   Note: `run_simulation()` is called by `run_simulation_with_phase()` only through the non-dead-code path; the `#[allow(dead_code)]` wrapper `run_simulation()` just delegates to `run_simulation_with_phase(req, cancel, |_| {})`. This is truly dead since the threaded backend always uses `run_simulation_with_phase` directly.

2. Delete each function and remove its `#[allow(dead_code)]` attribute.

3. Also remove any related dead helper functions: `make_depth()`, `make_depth_with_finishing()`, `make_depth_ext()`, `make_depth_from_heights()` in `helpers.rs` (lines 332-372) -- all four are marked `#[allow(dead_code)]`.

4. Clean up unused imports that were only needed by the deleted functions.

5. Note: `run_profile()` and `run_inlay()` are NOT dead -- they are used by `#[cfg(test)]` imports in `execute/mod.rs` (line 40-41). Keep these.

**Dependencies:** None. This can land independently and immediately.

**Estimated scope:** Small (< 50 lines net). Delete ~200 lines of dead functions, update ~10 lines of imports. Net deletion of ~190 lines.

**Risk:** Very low. The `#[allow(dead_code)]` annotations confirm these functions are not called. A `cargo test` run will catch any missed callers.

---

### LOW-1: No Backwards Imports (Clean)

**Severity:** LOW (positive finding)
**Description:** Core never imports from viz. The only references to `rs_cam_viz` in core source are documentation comments in `compute/mod.rs` and `feeds/INTEGRATION.md`. This boundary is clean.

**Recommended action:** None needed.

### Fix Plan

No action required. This is a positive finding confirming the dependency boundary is clean.

---

### LOW-2: Re-Export Pattern Is Clean

**Severity:** LOW (positive finding)
**Description:** Viz re-exports core types cleanly through thin re-export modules:
- `state/toolpath/support.rs` -- re-exports `rs_cam_core::compute::config::*`
- `state/toolpath/configs.rs` -- re-exports `rs_cam_core::compute::operation_configs::*`
- `state/toolpath/catalog.rs` -- re-exports `rs_cam_core::compute::catalog::*`
- `state/job.rs` -- re-exports stock config, tool config, and transform types

No unnecessary newtypes or conversion layers. The config types are single-source-of-truth in core.

**Recommended action:** None needed.

### Fix Plan

No action required. This is a positive finding confirming the re-export pattern is clean.

---

## What Moved vs What Stayed vs What's Duplicated

| Component | Before (baseline) | After (current) | Status |
|-----------|-------------------|-----------------|--------|
| **Config types** (OperationConfig, DressupConfig, ToolConfig, HeightsConfig, StockConfig, etc.) | viz only | core only, viz re-exports | MOVED -- clean |
| **Operation catalog** (OperationSpec, OperationType, feed_optimization_unavailable_reason) | viz only | core only, viz re-exports | MOVED -- clean |
| **SetupTransformInfo** (FaceUp, ZRotation, local_to_global) | viz only | core only, viz re-exports | MOVED -- clean |
| **build_cutter()** | viz only | core only, viz re-exports | MOVED -- clean |
| **compute_stats()** | viz only | core only | MOVED -- clean |
| **Semantic helpers** (CutRun, cutting_runs, contour_toolpath, line_toolpath) | viz only | core only | MOVED -- clean |
| **execute_operation()** (22-op dispatch) | viz only | core only (used by session.rs) | MOVED -- but viz has parallel path |
| **apply_dressups()** (dressup pipeline) | viz only | BOTH crates (different versions) | DUPLICATED -- divergent |
| **OperationError** enum | viz only | BOTH crates (identical variants) | DUPLICATED -- should consolidate |
| **SemanticToolpathOp** trait + 22 impls | viz only | viz only (not in core) | STAYED -- should move to core |
| **Semantic annotation functions** (annotate_adaptive3d_runtime_semantics, etc.) | viz only | viz only | STAYED -- should move to core |
| **Simulation orchestration** (run_simulation) | viz only | core (primary), viz (thin wrapper) | MOVED -- clean delegation |
| **SimulationRequest/Result/Boundary/Checkpoint types** | viz only | BOTH (core = canonical, viz = wrapper) | DUPLICATED -- intentional but could thin |
| **Collision checking** | viz only | core (primary), viz (thin wrapper) | MOVED -- clean delegation |
| **compute_grid_slopes() / slope filtering** | viz only | viz only (not in core DropCutter) | STAYED -- should move to core |
| **ToolpathPhaseTracker** | viz only | viz only | STAYED -- correct (UI concern) |
| **ThreadedComputeBackend** | viz only | viz only | STAYED -- correct (threading/UI concern) |
| **Debug artifact I/O** | viz only | viz only | STAYED -- correct (filesystem concern) |

---

## Completion Assessment vs Plan

Per `planning/SERVICE_LAYER_EXTRACTION.md`:

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 1: Config types to core | COMPLETE | All config types moved, viz re-exports cleanly |
| Phase 2: Execution logic to core | PARTIAL | `execute_operation()` and helpers exist in core, but viz doesn't call them. Semantic tracing impls not moved. |
| Phase 3: Simulation & collision to core | COMPLETE | Viz properly delegates to core |
| Phase 4: ProjectSession | COMPLETE | `session.rs` exists and uses core exclusively |
| Phase 5: Rewire CLI | COMPLETE | CLI uses ProjectSession -> core |
| Phase 6: Rewire GUI | NOT STARTED | Viz still has its own 4300-line execution pipeline |
| Phase 7: MCP Server | COMPLETE | Uses ProjectSession |

**Bottom line:** The extraction is ~75% complete. The remaining 25% is Phase 6 (rewiring the GUI to delegate to core for operation execution), which would eliminate ~2500 lines of duplicated logic and close all three HIGH findings.
