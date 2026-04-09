# Service Layer Performance Review

**Baseline**: commit 443a613 (pre-extraction, `/tmp/rs_cam_baseline/`)
**Current**: commit 127ff2c (`/home/ricky/personal_repos/rs_cam/`)

---

## Summary

The service-layer extraction moved simulation orchestration and operation configs to core, added `ProjectSession` as a unified API, and introduced the `rs_cam_mcp` crate. The refactoring is structurally sound with no regressions in the simulation hot path. However, there are avoidable data clones in the session layer, a duplicated operation dispatch between viz and core, and one file (`session.rs`) that has grown beyond its natural scope. Build-time impact from the MCP crate is contained.

---

## Findings

### 1. Toolpath + semantic trace cloned into Arc for every simulation run

**Severity**: MEDIUM

**Location**: `crates/rs_cam_core/src/session.rs` lines 1154, 1158

```rust
toolpath: Arc::new(result.toolpath.clone()),
semantic_trace: result.semantic_trace.as_ref().map(|t| Arc::new(t.clone())),
```

`ToolpathComputeResult` stores `Toolpath` (owned). When building `SimToolpathEntry`, the session clones the entire toolpath (a `Vec<Move>` that can be hundreds of thousands of entries) and wraps it in `Arc`. Same for the semantic trace.

**Impact**: For a 10-toolpath project with 100k moves each, this clones ~10 * 100k * 40 bytes = ~40 MB of move data on every simulation call.

**Recommended action**: Store the toolpath as `Arc<Toolpath>` inside `ToolpathComputeResult` so simulation can `Arc::clone` (pointer copy) instead of deep-cloning. The toolpath is immutable after generation, so shared ownership is safe. Same for `semantic_trace` -- store as `Option<Arc<ToolpathSemanticTrace>>`.

### Fix Plan

1. **What to change**
   - `crates/rs_cam_core/src/session.rs` line 419: change `ToolpathComputeResult.toolpath` from `Toolpath` to `Arc<Toolpath>`
   - `crates/rs_cam_core/src/session.rs` line 422: change `semantic_trace` from `Option<ToolpathSemanticTrace>` to `Option<Arc<ToolpathSemanticTrace>>`
   - `crates/rs_cam_core/src/session.rs` lines 1030-1061 (`generate_toolpath`): wrap the generated toolpath in `Arc::new()` at construction time (after dressups are applied, right before insertion into `self.results`)
   - `crates/rs_cam_core/src/session.rs` line 1154: change `Arc::new(result.toolpath.clone())` to `Arc::clone(&result.toolpath)` (pointer copy)
   - `crates/rs_cam_core/src/session.rs` line 1158: change `result.semantic_trace.as_ref().map(|t| Arc::new(t.clone()))` to `result.semantic_trace.clone()` (Arc clone)
   - All read sites that access `result.toolpath.moves` (lines 1138, 1268, 1374) continue to work via `Deref` -- no changes needed

2. **How to change it**
   - In `generate_toolpath` (line 1030), after `apply_dressups` returns the owned `Toolpath`, wrap it: `let toolpath = Arc::new(toolpath);`
   - Similarly wrap `semantic_trace`: `let semantic_trace = Some(Arc::new(semantic_trace));`
   - Update the `ToolpathComputeResult` struct fields
   - The `GcodePhase` at line 1374 takes `&Toolpath` -- since `Arc<Toolpath>` implements `Deref<Target = Toolpath>`, `&result.toolpath` still produces `&Toolpath`. No change needed.
   - The `check_rapid_collisions(&result.toolpath, ...)` at line 1268 similarly auto-derefs. No change needed.

3. **Dependencies** — None. This is a self-contained change.

4. **Estimated scope** — Small (< 50 lines). Struct field type change + 5-6 call site updates.

5. **Risk** — Low. `Arc<Toolpath>` implements `Deref<Target = Toolpath>`, so all existing `&result.toolpath` and `result.toolpath.moves` sites compile without change. The collision check in Finding 2 still clones the inner `Toolpath` via `result.toolpath.as_ref().clone()` (or equivalent), so that issue remains independently fixable. The only semantic change is that `ToolpathComputeResult` is no longer `Clone`-derivable (if it ever was) without explicit `Arc::clone` on the toolpath field -- but it has no `#[derive(Clone)]` today.

---

### 2. Collision check clones Toolpath and TriangleMesh unnecessarily

**Severity**: MEDIUM

**Location**: `crates/rs_cam_core/src/session.rs` lines 1250-1252

```rust
let request = CollisionCheckRequest {
    toolpath: result.toolpath.clone(),
    mesh: model.as_ref().clone(),
};
```

`CollisionCheckRequest` owns `Toolpath` and `TriangleMesh`, but `run_collision_check` only borrows them (`&CollisionCheckRequest`). The mesh clone is especially expensive (vertices + faces + bounding box data).

**Impact**: Clones an entire mesh (potentially millions of triangles) and toolpath for each collision check invocation. This is a one-shot cost, not per-frame, but still wasteful.

**Recommended action**: Change `CollisionCheckRequest` to hold `&Toolpath` and `&TriangleMesh` (lifetime-parameterized), or at minimum `Arc` references.

### Fix Plan

1. **What to change**
   - `crates/rs_cam_core/src/compute/collision_check.rs` lines 13-17: change `CollisionCheckRequest` to borrow its data:
     ```rust
     pub struct CollisionCheckRequest<'a> {
         pub toolpath: &'a Toolpath,
         pub tool: ToolDefinition,
         pub mesh: &'a TriangleMesh,
     }
     ```
   - `crates/rs_cam_core/src/compute/collision_check.rs` lines 55-57: update `run_collision_check` signature to `fn run_collision_check(request: &CollisionCheckRequest<'_>, cancel: &AtomicBool)`. Internally, `request.toolpath` and `request.mesh` are already used through `&` references (`&request.toolpath`, `&request.mesh`) so the function body needs no changes.
   - `crates/rs_cam_core/src/session.rs` lines 1249-1253: change the call site from:
     ```rust
     let request = CollisionCheckRequest {
         toolpath: result.toolpath.clone(),
         tool: tool_def,
         mesh: model.as_ref().clone(),
     };
     ```
     to:
     ```rust
     let request = CollisionCheckRequest {
         toolpath: &result.toolpath,
         tool: tool_def,
         mesh: model.as_ref(),
     };
     ```
   - `crates/rs_cam_core/src/compute/collision_check.rs` tests (lines 118-152): update test construction to pass `&tp` and `&mesh` instead of moving them into the struct. The tests create `CollisionCheckRequest` with owned values -- change to references.

2. **How to change it**
   - Add lifetime parameter `<'a>` to `CollisionCheckRequest`
   - Change `toolpath: Toolpath` to `toolpath: &'a Toolpath` and `mesh: TriangleMesh` to `mesh: &'a TriangleMesh`
   - Keep `tool: ToolDefinition` as owned (it is constructed fresh per call and consumed)
   - The `run_collision_check` body already accesses everything through references (`&request.toolpath`, `&request.mesh`) so no internal changes are needed
   - If Finding 1 lands first and `result.toolpath` is `Arc<Toolpath>`, then `&result.toolpath` auto-derefs to `&Toolpath` -- still works

3. **Dependencies** — None (works independently of Finding 1, though the two are complementary).

4. **Estimated scope** — Small (< 50 lines). Struct definition + 3 call sites (1 production, 2 tests).

5. **Risk** — Low. The lifetime parameter propagates only to `CollisionCheckRequest` itself, not to `run_collision_check`'s return type. The request is created and consumed within the same function scope in both the session call site and the tests, so lifetime inference is trivial. The only risk is if any downstream code stores `CollisionCheckRequest` across borrow scopes -- no such usage exists today.

---

### 3. Duplicate operation dispatch: viz retains its own full dispatch alongside core's execute.rs

**Severity**: MEDIUM

**Location**:
- Viz: `crates/rs_cam_viz/src/compute/worker/execute/{mod,operations_2d,operations_3d}.rs` (3,754 lines)
- Core: `crates/rs_cam_core/src/compute/execute.rs` (836 lines)

The extraction added `core::compute::execute::execute_operation` (836 lines) covering all 23 operations. However, the viz compute worker still has its own complete dispatch through the `SemanticToolpathOp` trait -- 3,754 lines of operation dispatch code that does the same thing with added semantic tracing and debug instrumentation.

The `ProjectSession` (core) calls `core::compute::execute::execute_operation`. The GUI compute worker does NOT use it -- it uses its own `semantic_op(&req.operation).generate_with_tracing(&exec_ctx)` chain.

**Impact**: Two independent implementations of the same 23-operation dispatch. Any new operation must be added in both places. Any bug fix must be applied twice. Net code increase of ~615 lines vs baseline.

**Recommended action**: Migrate the viz compute worker to delegate to `core::compute::execute::execute_operation`, passing debug/semantic context through. The viz-specific wrapping (phase tracking, debug spans, semantic traces) can be applied as a thin wrapper around the core function rather than reimplementing every operation.

### Fix Plan

1. **What to change**
   - `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` lines 311-348 (`run_compute_with_phase_tracker`): replace the `semantic_op(&req.operation).generate_with_tracing(&exec_ctx)` call with a call to `rs_cam_core::compute::execute::execute_operation(...)`, passing the existing `core_ctx` debug context through. The phase tracking, debug span creation, and dressup application already wrap the call -- those stay.
   - `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` lines 74-107 (`SemanticToolpathOp` trait + `semantic_op` dispatch): delete entirely.
   - `crates/rs_cam_viz/src/compute/worker/execute/operations_2d.rs`: reduce to only the semantic annotation helpers (`annotate_adaptive_runtime_semantics`, `annotate_operation_scope`, etc.) and the `run_*` helper functions used by tests. Delete all 12 `impl SemanticToolpathOp for *Config` blocks.
   - `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs`: same treatment -- delete all 11 `impl SemanticToolpathOp for *Config` blocks. Keep `prepare_mesh_operation`, `compute_grid_slopes`, and the `run_*` helpers used by tests.
   - Add a post-generation semantic annotation pass: after `execute_operation` returns the toolpath, call into per-operation annotation functions to attach semantic trace data. This preserves the rich semantic information without duplicating the operation logic.

2. **How to change it**
   - **Phase 1 (delegation)**: In `run_compute_with_phase_tracker`, replace:
     ```rust
     let tp = semantic_op(&req.operation).generate_with_tracing(&exec_ctx)?;
     ```
     with a call to `execute_operation` that maps from the viz `ComputeRequest` fields to the core function's parameters:
     ```rust
     let tp = rs_cam_core::compute::execute::execute_operation(
         &req.operation,
         req.mesh.as_deref(),
         req.spatial_index.as_ref(),
         req.polygons.as_deref().map(|v| v.as_slice()),
         &build_cutter(&req.tool),
         &req.tool,
         &req.resolved_heights,
         &req.cutting_levels,
         &req.stock_bbox.unwrap_or_default(),  // needs guard
         req.prev_tool_radius,
         core_ctx.as_ref(),
         cancel,
     ).map_err(ComputeError::from)?;
     ```
     This requires ensuring the viz `ComputeRequest` exposes the same fields that `execute_operation` expects (resolved heights, cutting levels, stock bbox). Most of these are already present on `ComputeRequest`; verify each field name maps correctly.
   - **Phase 2 (semantic annotation)**: Extract the semantic trace annotation from each `SemanticToolpathOp` impl into a standalone function `annotate_operation_semantics(op: &OperationConfig, tp: &Toolpath, semantic_root: Option<&SemanticContext>, ...)` that pattern-matches the operation and calls the appropriate `annotate_*` helpers. Call this after `execute_operation` returns.
   - **Phase 3 (cleanup)**: Remove the `SemanticToolpathOp` trait, `semantic_op` function, `OperationExecutionContext` struct, and all `impl SemanticToolpathOp` blocks.

3. **Dependencies**
   - The viz `ComputeRequest` must expose `resolved_heights: ResolvedHeights` and `cutting_levels: Vec<f64>` (or the equivalent). Check whether these are already computed before entering `run_compute_with_phase_tracker` or if they are computed inside individual `SemanticToolpathOp` impls. If the latter, the height resolution logic must be hoisted to the call site first.
   - Some 3D operations in viz pass `phase_tracker` into the operation (e.g., `run_dropcutter` takes `phase_tracker` for sub-phase reporting). The core `execute_operation` does not accept a phase tracker. Sub-phase reporting for 3D ops would be lost unless `execute_operation` is extended to accept an optional phase tracker, or the viz layer adds phase tracking around the call rather than inside it.
   - No dependency on Findings 1 or 2.

4. **Estimated scope** — Large (200+ lines). Net deletion of ~2,500-3,000 lines from viz, addition of ~100-200 lines for the annotation bridge and `ComputeRequest` field mapping. The refactor touches 3 files in viz and potentially `execute_operation`'s signature in core.

5. **Risk** — Medium-high. This is the riskiest change:
   - **Semantic trace fidelity**: The viz `SemanticToolpathOp` impls interleave semantic annotations _during_ toolpath generation (e.g., annotating individual cutting runs, levels, polygons as they are produced). Moving to a post-generation annotation pass means some structural information (which polygon produced which runs, which level produced which moves) may not be available after the fact. Operations like `Face`, `Pocket`, and `Adaptive` build semantic trees while generating -- this would need to be reconstructed from the finished toolpath or the annotation would lose granularity.
   - **Sub-phase reporting**: 3D operations currently report fine-grained phases ("Drop-cutter grid", "Rasterize grid", "Waterline passes") through `phase_tracker` passed into the operation. Delegating to core's `execute_operation` loses this unless the core function is extended.
   - **Test breakage**: Tests in `operations_2d.rs` and `operations_3d.rs` that call `run_*` helpers directly are unaffected, but any test that exercises `SemanticToolpathOp` would need updating.
   - **Recommended mitigation**: Do this in stages. Start with a single simple operation (e.g., `Profile` or `Drill`) to validate the pattern, then migrate the rest incrementally.

---

### 4. session.rs is 1691 lines -- doing too much

**Severity**: LOW

**Location**: `crates/rs_cam_core/src/session.rs`

This file contains:
- TOML deserialization types (ProjectFile, ProjectJobSection, ProjectToolSection, etc.) -- ~375 lines
- Loaded state types (LoadedModel, SetupData, ToolpathConfig, etc.) -- ~100 lines
- ProjectSession struct + lifecycle + queries + mutation + compute + analysis + export -- ~700 lines
- Free functions for loading models and parsing -- ~200 lines
- Serde impls for diagnostics -- ~30 lines
- Tests -- ~75 lines
- Default value functions -- ~100 lines

**Impact**: Not a performance issue but a maintainability concern. The file conflates project file I/O with session state management with compute orchestration.

**Recommended action**: Split into `session/project_file.rs` (deserialization types + loading), `session/mod.rs` (ProjectSession core), and `session/compute.rs` (generate/simulate/collision/export). No urgency.

### Fix Plan

1. **What to change**
   - Create `crates/rs_cam_core/src/session/` directory with three files:
     - `session/project_file.rs` (~475 lines): Move all TOML deserialization types (`ProjectFile`, `ProjectJobSection`, `ProjectStockConfig`, `ProjectPostConfig`, `ProjectToolSection`, `ProjectModelSection`, `ProjectSetupSection`, `ProjectToolpathSection`), their `Default` impls, all `default_*` value functions (lines 113-383), and the free functions `stock_from_project`, `parse_tool_type`, `tool_from_project_section`, `infer_model_kind`, `load_model_geometry`, and the `LoadedGeometry` enum (lines 1442-1580).
     - `session/compute.rs` (~350 lines): Move `generate_toolpath`, `generate_all`, `run_simulation`, `collision_check`, `diagnostics`, and `export_gcode` methods from the `impl ProjectSession` block. These would be in a separate `impl ProjectSession` block that is `pub(crate)` or `pub`.
     - `session/mod.rs` (~450 lines): Keep `SessionError`, `LoadedModel`, `SetupData`, `ToolpathConfig`, `ToolpathComputeResult`, `ToolpathSummary`, `ToolSummary`, `SimulationOptions`, `ToolpathDiagnostic`, `ProjectDiagnostics`, the `ProjectSession` struct definition, and the lifecycle/query/mutation methods (`load`, `from_project_file`, `toolpath_count`, `toolpath_summaries`, `tool_summaries`, `set_toolpath_param`, `find_tool_by_raw_id`, `find_model_by_raw_id`, `find_setup_for_toolpath_index`, `effective_stock_bbox`, `stock_bbox`).
   - Rename existing `crates/rs_cam_core/src/session.rs` to `crates/rs_cam_core/src/session/mod.rs` and extract the other two files.
   - Update `crates/rs_cam_core/src/lib.rs` if it has `mod session;` -- a file-to-directory rename is transparent to `mod session;` as long as the directory has `mod.rs`.
   - The serde impls for `ToolpathDiagnostic` and `ProjectDiagnostics` (lines 1583-1615) go with `mod.rs` (they are on types defined there).
   - Tests (lines 1617-1691) stay in `mod.rs` or move to `compute.rs` depending on what they test.

2. **How to change it**
   - Create the `session/` directory
   - Move types and functions as described, adding `pub(crate)` or `pub(super)` visibility as needed for cross-file access within the module
   - The `project_file.rs` types are all `pub` and used by `mod.rs` (in `from_project_file`), so they get `pub use` re-exports from `mod.rs` to maintain the public API
   - The `compute.rs` methods need access to `ProjectSession` private fields (`results`, `toolpath_configs`, `tools`, `models`, `stock`, `post`, `simulation`). Since they are in the same module (`session`), they can access private fields of the struct defined in `mod.rs` -- Rust's privacy boundary is the module, not the file.

3. **Dependencies** — None. This is a pure refactor with no behavioral change. Should land _after_ Findings 1 and 2 to avoid merge conflicts, since those modify the same lines.

4. **Estimated scope** — Medium (50-200 lines changed, though ~1,200 lines are moved). The actual new code is just `mod` declarations, `use` imports, and `pub use` re-exports.

5. **Risk** — Low. This is a mechanical split with no logic changes. The public API (`ProjectSession` and its methods) is unchanged. The only risk is missing a `use` import or `pub` visibility specifier, which the compiler catches immediately. Run `cargo test -p rs_cam_core` to verify.

---

### 5. Viz simulation request → core request conversion: model_mesh clone

**Severity**: LOW

**Location**: `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` line 158

```rust
model_mesh: req.model_mesh.clone(),
```

The viz `build_core_simulation_request` clones the `Option<Arc<TriangleMesh>>`. Since it's an `Arc::clone`, this is just a reference count bump -- negligible cost. The `name.clone()` and `tool_summary` string clones are similarly cheap (short strings).

The `Arc::clone(&tp.toolpath)` for toolpaths is also just pointer copies, not deep clones.

**Impact**: Negligible. All clones in the conversion function are either `Arc::clone` (pointer copy) or short string copies.

**Recommended action**: None needed.

---

### 6. Simulation hot path: no regressions from extraction

**Severity**: NONE (informational)

**Location**: `crates/rs_cam_core/src/compute/simulate.rs` vs `/tmp/rs_cam_baseline/crates/rs_cam_viz/src/compute/worker/execute/mod.rs`

The core simulation loop is a clean transplant of the baseline viz simulation:
- Same per-group local stock + parallel global stock pattern (pre-existing)
- Same `simulate_toolpath_with_lut_metrics_cancel` inner loop
- Same `dexel_stock_to_mesh` + `transform_stock_mesh_to_global` checkpoint pattern
- No new allocations in the inner loop

The only difference: the new code builds `ToolDefinition` (cutter) directly (`entry.tool` is pre-built) while the old code called `build_cutter(tool_config)` inside the loop. This is a minor improvement since the cutter construction is now done by the caller.

The double simulation (group stock + global stock for checkpoints) is a pre-existing pattern, not introduced by the extraction.

**Impact**: No performance regression in the simulation hot path.

---

### 7. dexel_mesh.rs contour-tiling: complexity is acceptable

**Severity**: NONE (informational)

**Location**: `crates/rs_cam_core/src/dexel_mesh.rs` -- grew from 580 to 1117 lines (+537)

The new code adds internal cavity surface generation (`emit_z_grid_cavity_surfaces`). The algorithmic complexity is:

- Phase 1: O(cells) to collect per-cell gap info -- linear
- Phase 2: O(cells * max_gaps^2) for shared gap detection via `find_matching_gap`
- Phase 3: O(cells * max_gaps^2) for cavity wall edges

Since `max_gaps` is bounded by the number of through-cuts at a cell (typically 1-3, at most ~10 for extreme cases), this is effectively O(cells) in practice. The `find_matching_gap` does a linear scan over gaps but the gap count per cell is tiny (bounded by physical geometry).

**Impact**: No quadratic blowup. The contour-tiling is linear in the number of grid cells.

---

### 8. operation_configs.rs OperationParams trait: worth its weight

**Severity**: NONE (informational)

**Location**: `crates/rs_cam_core/src/compute/operation_configs.rs` (1233 lines) + `catalog.rs`

The `OperationParams` trait provides `feed_rate()`, `set_feed_rate()`, `plunge_rate()`, `set_plunge_rate()`, `stepover()`, `depth_per_pass()` across all 23 operation variants. This enables:
- Generic parameter mutation in `session.rs:set_toolpath_param` without pattern-matching 23 variants
- Feed optimization queries
- MCP parameter setting via the session API

The file is large but mechanical (23 config structs * (Default impl + fields)). No unnecessary complexity.

---

### 9. MCP crate dependencies are properly isolated

**Severity**: NONE (informational)

**Location**: `crates/rs_cam_mcp/Cargo.toml`

The MCP crate depends on `tokio`, `rmcp`, and `tracing-subscriber`, but these are:
- Only in `rs_cam_mcp`'s `[dependencies]`, not in workspace dependencies
- Not leaking into `rs_cam_core` or `rs_cam_viz` (verified by checking their Cargo.toml files)
- `rs_cam_mcp` depends on `rs_cam_core` (path dep) but not `rs_cam_viz`

**Impact**: No build time impact on core or viz. The MCP crate compiles independently.

---

### 10. Arc usage patterns are appropriate

**Severity**: NONE (informational)

The Arc wrapping pattern is consistent:
- `LoadedModel.mesh: Option<Arc<TriangleMesh>>` -- shared between toolpath generations, correct
- `LoadedModel.polygons: Option<Arc<Vec<Polygon2>>>` -- same
- `SimToolpathEntry.toolpath: Arc<Toolpath>` -- shared across simulation/playback/checkpoint, correct
- `SimulationResult.cut_trace: Option<Arc<SimulationCutTrace>>` -- shared with viz, correct

No unnecessary Arc wrapping found. The one improvement is storing `ToolpathComputeResult.toolpath` as `Arc<Toolpath>` (see Finding 1).

---

## Priority Summary

| # | Severity | Finding | Effort |
|---|----------|---------|--------|
| 1 | MEDIUM | Toolpath cloned for each simulation | Low -- change field to Arc |
| 2 | MEDIUM | Collision check clones mesh + toolpath | Low -- take references |
| 3 | MEDIUM | Duplicate operation dispatch (viz + core) | High -- requires viz worker refactor |
| 4 | LOW | session.rs too large | Medium -- straightforward split |
| 5-10 | NONE | No issues found | N/A |

Findings 1 and 2 are the most impactful quick wins. Finding 3 is the largest structural debt but requires careful work to preserve viz-specific tracing/debug instrumentation while delegating to core.
