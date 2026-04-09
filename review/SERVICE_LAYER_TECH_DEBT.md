# Service Layer Tech Debt Review

**Date**: 2026-04-08
**Commit**: 127ff2c
**Scope**: `crates/rs_cam_core/src/compute/`, `session.rs`, `crates/rs_cam_mcp/src/server.rs`, and related viz delegation code

## Summary

The migration to a unified `ProjectSession` in core with shared `execute_operation()` dispatch is structurally sound. The core layer has clean error types, consistent operation dispatch, and reasonable test coverage for the simulation pipeline. However, several categories of tech debt remain:

1. **Dead/unreachable code in diagnostics** -- `total_collision_count` is hardcoded to 0 and never mutated
2. **Parallel operation dispatch in viz** -- 2,100+ lines of viz `SemanticToolpathOp` implementations duplicate core dispatch logic with added semantic tracing
3. **Discarded error context** in cancellation paths (`map_err(|_e| Cancelled)`)
4. **Dead code accumulation** in viz helpers that were superseded by core delegation
5. **Unused semantic recorder** in session's `generate_toolpath`
6. **Missing tests** for core `execute.rs` (zero unit tests) and session mutation methods
7. **Inconsistent MCP error response format** -- some tools return `text()`, others `json_str()`

---

## Findings

### Category 1: Error Handling

#### [HIGH] Diagnostics `total_collision_count` is always zero

- **File**: `crates/rs_cam_core/src/session.rs:1263`
- **Description**: `total_collision_count` is declared as `let total_collision_count: usize = 0;` and never mutated. The verdict logic at line 1309 checks `if total_collision_count > 0` which can never be true, and line 1329 reports it as `collision_count: 0` regardless of actual collisions. Only `total_rapid_collision_count` is incremented.
- **Impact**: Holder/shank collision verdicts ("ERROR: N holder/shank collisions detected") can never fire from `diagnostics()`. The MCP `get_diagnostics` tool will always report `collision_count: 0`.
- **Fix**: Either wire up actual holder collision counting (call `collision_check` per toolpath during diagnostics) or remove the dead branch and document that full collision checks require an explicit `collision_check()` call.

### Fix Plan

1. **What to change**: `crates/rs_cam_core/src/session.rs`, the `diagnostics()` method (line 1263). Change `let total_collision_count: usize = 0;` to `let mut total_collision_count: usize = 0;` and accumulate per-toolpath collision counts from the `per_toolpath` entries. Since full holder/shank collision checking is expensive (requires mesh + tool assembly + spatial index), the lightweight approach is: if the session already has collision results cached (from an explicit `collision_check()` call), sum those; otherwise leave at 0 and adjust the verdict wording. Also update `ToolpathDiagnostic.collision_count` (line 1285, currently hardcoded `0`) to pull from cached collision results when available.
2. **How to change it**: Add an `Option<HashMap<usize, CollisionCheckResult>>` field to `ProjectSession` (populated by `collision_check()`). In `diagnostics()`, look up cached collision results per toolpath and sum them. Remove the dead `if total_collision_count > 0` branch only if no caching approach is taken; otherwise wire it up.
3. **Dependencies**: None -- self-contained.
4. **Estimated scope**: Small (< 50 lines). The struct field addition, cache population in `collision_check()`, and the diagnostic lookup are all compact changes.
5. **Risk**: Low. The `diagnostics()` method is read-only and the collision cache is purely additive state. Existing tests (`diagnostics_empty_session`) still pass because no collisions are cached for an empty session.

#### [MEDIUM] Cancellation errors discard original error context

- **File**: `crates/rs_cam_core/src/compute/execute.rs:446,530,554`
- **File**: `crates/rs_cam_core/src/compute/simulate.rs:244,255`
- **File**: `crates/rs_cam_core/src/compute/collision_check.rs:70`
- **Description**: Six `map_err(|_e| ...::Cancelled)` or `map_err(|_cancelled| ...::Cancelled)` calls discard the original error. These cancellation errors from the underlying functions are unit-type `Cancelled` structs, so the `_e` naming misleadingly suggests there is information being discarded. While the current `Cancelled` type carries no data, naming the parameter `_e` rather than `_` is confusing.
- **Impact**: Low immediate impact since `Cancelled` is a unit type. But if the underlying error type is ever enriched, these will silently discard context.
- **Fix**: Use `|_|` instead of `|_e|` / `|_cancelled|` to make it clear no information is lost. Or implement `From<Cancelled>` for the outer error types.

### Fix Plan

1. **What to change**: Six `map_err` closures across three files:
   - `crates/rs_cam_core/src/compute/execute.rs` lines 446, 530, 554: `|_e|` -> `|_|`
   - `crates/rs_cam_core/src/compute/simulate.rs` lines 244, 255: `|_cancelled|` -> `|_|`
   - `crates/rs_cam_core/src/compute/collision_check.rs` line 70: `|_cancelled|` -> `|_|`
2. **How to change it**: Simple text replacement of the closure parameter names. Each of the `Cancelled` error types (`dropcutter::Cancelled`, `adaptive3d::Cancelled`, `waterline::CancelledError`, `simulate::Cancelled`) is a unit struct, so `|_|` is semantically correct and communicates that no information is discarded.
3. **Dependencies**: None.
4. **Estimated scope**: Small (< 50 lines). Six one-character edits.
5. **Risk**: None. Pure cosmetic/clarity change with no behavioral impact.

#### [MEDIUM] MCP "no project loaded" error format is inconsistent

- **File**: `crates/rs_cam_mcp/src/server.rs`
- **Description**: Tools use two different patterns for "no project loaded" errors:
  - `json_str(serde_json::json!({"error": "No project loaded"}))` (lines 160, 184, 196, 211, 343) -- structured JSON
  - `text("No project loaded")` (lines 355, 377, 401) -- plain text
  - Some query tools return JSON, mutation/export tools return plain text
- **Impact**: MCP clients cannot rely on a single error schema. Parsing errors requires checking both formats.
- **Fix**: Standardize all "no project loaded" responses to use the JSON format. Consider extracting a `require_session()` helper that returns a consistent error.

### Fix Plan

1. **What to change**: `crates/rs_cam_mcp/src/server.rs`. Three tool handlers currently return `text("No project loaded")`:
   - `export_gcode` (line 355)
   - `set_toolpath_param` (line 377)
   - `set_tool_param` (line 401)

   Change them to return `json_str(serde_json::json!({"error": "No project loaded"}))` to match the five other tools that already use this format (lines 160, 184, 196, 211, 343).

2. **How to change it**: Extract a shared helper, e.g.:
   ```rust
   fn no_project_error() -> String {
       json_str(serde_json::json!({"error": "No project loaded"}))
   }
   ```
   Replace all 8 occurrences (5 existing JSON + 3 text) with `no_project_error()`. This eliminates future drift.

3. **Dependencies**: None.
4. **Estimated scope**: Small (< 50 lines). One new 3-line helper + 8 call-site replacements.
5. **Risk**: MCP clients that currently parse the plain-text "No project loaded" string will see a format change. Since no stable client contract exists yet, this is acceptable. The change makes the API more consistent for future clients.

#### [LOW] `serde_json::to_value(...).unwrap_or_default()` silently swallows serialization errors

- **File**: `crates/rs_cam_mcp/src/server.rs:186,198,345`
- **Description**: `list_toolpaths`, `list_tools`, and `get_diagnostics` use `serde_json::to_value(...).unwrap_or_default()` which returns `Value::Null` on serialization failure. The MCP client receives `null` with no indication of failure.
- **Fix**: Use `map_err` to return a proper error response, or log a warning before falling back.

### Fix Plan

1. **What to change**: `crates/rs_cam_mcp/src/server.rs`, three call sites: `list_toolpaths` (line 186), `list_tools` (line 198), and `get_diagnostics` (line 345). Each uses `serde_json::to_value(...).unwrap_or_default()`.
2. **How to change it**: Replace `unwrap_or_default()` with a `match` or `map_err` that returns a JSON error response:
   ```rust
   match serde_json::to_value(session.list_toolpaths()) {
       Ok(v) => json_str(v),
       Err(e) => json_str(serde_json::json!({"error": format!("serialization failed: {e}")})),
   }
   ```
   Alternatively, use `unwrap_or_else(|e| serde_json::json!({"error": format!("serialization failed: {e}")}))`.
3. **Dependencies**: None. Can be bundled with the MCP error format fix above.
4. **Estimated scope**: Small (< 50 lines). Three call-site changes.
5. **Risk**: Very low. The `Serialize` impls for `ToolpathSummary`, `ToolSummary`, and `ProjectDiagnostics` are derived and should never fail. This is defensive programming.

---

### Category 2: Dead Code

#### [HIGH] Parallel operation dispatch in viz remains after core unification

- **File**: `crates/rs_cam_viz/src/compute/worker/execute/operations_2d.rs` (1,174 lines)
- **File**: `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs` (947 lines)
- **Description**: The viz layer has 23 `SemanticToolpathOp` implementations that each re-implement the core operation dispatch with added semantic tracing. It also retains 10+ `run_*` helper functions (e.g. `run_pocket`, `run_drill`, `run_waterline`, `run_dropcutter`) marked `#[allow(dead_code)]` that duplicate the logic now in `core::compute::execute::execute_operation()`. The viz `SemanticToolpathOp` impls call the underlying algorithms directly (e.g. `pocket_toolpath`, `zigzag_toolpath`) rather than delegating to core's `execute_operation`, so the core dispatch exists but is only used by `ProjectSession` -- not by the GUI.
- **Impact**: Any bug fix or behavior change to operation dispatch must be made in two places. The `#[allow(dead_code)]` functions are never called and add 300+ lines of dead code.
- **Fix**: Phase 1 -- delete the `#[allow(dead_code)]` `run_*` functions that are clearly unused. Phase 2 -- evaluate whether `SemanticToolpathOp` impls can delegate to `execute_operation` and add semantic tracing as a wrapper rather than duplicating every operation.

### Fix Plan

1. **What to change**:
   - **Phase 1** (immediate): Delete dead `run_*` functions:
     - `crates/rs_cam_viz/src/compute/worker/execute/operations_2d.rs`: `run_pocket` (line 16, ~38 lines) and `run_drill` (line 332, ~38 lines)
     - `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs`: `run_dropcutter` (line 78, ~25 lines) and `run_waterline` (line 196, ~30 lines)
   - **Phase 2** (future): Refactor the 23 `SemanticToolpathOp` implementations to call `core::compute::execute::execute_operation()` internally, wrapping it with semantic tracing. This would reduce `operations_2d.rs` and `operations_3d.rs` by ~60% each.

2. **How to change it**: Phase 1 is a straight deletion of the four dead functions and their `#[allow(dead_code)]` annotations. Run `cargo clippy` afterward to confirm no new warnings. Phase 2 requires creating a generic wrapper like:
   ```rust
   fn execute_with_semantics(req: &ComputeRequest, semantic: &SemanticContext) -> Result<Toolpath, ComputeError> {
       let core_result = execute_operation(/* convert req fields */);
       // attach semantic annotations
   }
   ```
   Each `SemanticToolpathOp::execute()` impl would then call this wrapper instead of duplicating the algorithm call.

3. **Dependencies**: Phase 1 has no dependencies. Phase 2 depends on core `execute_operation` being extended to accept an optional `ToolpathSemanticContext` parameter (or a trait object for progress/tracing callbacks).
4. **Estimated scope**: Phase 1: Small (< 50 lines deleted per file). Phase 2: Large (200+ lines, touching ~23 `SemanticToolpathOp` impls across two files).
5. **Risk**: Phase 1: None -- the functions are provably dead (`#[allow(dead_code)]`). Phase 2: Medium -- the `SemanticToolpathOp` impls add per-polygon iteration tracing, adaptive runtime annotations, and phase tracking that core's `execute_operation` does not currently support. Incorrectly merging these would lose GUI-specific diagnostic granularity.

#### [MEDIUM] Unused `_semantic_root` in `session.rs` generate_toolpath

- **File**: `crates/rs_cam_core/src/session.rs:1008`
- **Description**: `let _semantic_root = semantic_recorder.root_context();` creates a semantic context that is never used. The semantic recorder is finished at line 1051 and the trace is stored, but no semantic scopes are created during the session's toolpath generation. The underscore prefix suppresses the unused-variable warning but indicates unfinished wiring.
- **Impact**: Session-generated toolpaths have empty semantic traces. The `enrich_traces` call at line 1052 operates on an empty semantic tree.
- **Fix**: Either wire semantic tracing into the session's `generate_toolpath` (pass `_semantic_root` to `execute_operation` or a wrapper), or remove the semantic recorder from session and store `None` for the semantic trace.

### Fix Plan

1. **What to change**: `crates/rs_cam_core/src/session.rs`, the `generate_toolpath()` method, lines 1005-1008 and 1051-1052.
   - **Option A (remove)**: Delete the `semantic_recorder` creation (line 1005-1006), `_semantic_root` (line 1008), `semantic_recorder.finish()` (line 1051), and `enrich_traces` call (line 1052). Store `None` for `semantic_trace` in the `ToolpathComputeResult` (line 1060). This is the simpler path since session-generated toolpaths (CLI/MCP) do not currently need semantic traces.
   - **Option B (wire up)**: Rename `_semantic_root` to `semantic_root`, pass `Some(&semantic_root)` as an additional parameter to `execute_operation` or wrap the call in a semantic scope. This requires `execute_operation` to accept an `Option<&ToolpathSemanticContext>` parameter (currently it does not).

2. **How to change it**: Option A is recommended. Remove 6 lines related to semantic recorder setup and replace `Some(semantic_trace)` with `None` in the result struct. Also remove the error-path `let _ = semantic_recorder.finish();` at line 1070.
3. **Dependencies**: If Option B is chosen, it depends on extending `execute_operation`'s signature in `execute.rs`, which should be coordinated with the Phase 2 viz dispatch unification (see parallel dispatch finding above).
4. **Estimated scope**: Option A: Small (< 50 lines). Option B: Medium (50-200 lines) due to the signature change propagation.
5. **Risk**: Option A: Low. CLI/MCP paths currently produce empty semantic traces anyway, so this just makes that explicit. Option B: Medium. Changing `execute_operation`'s signature affects both session and viz callers.

#### [MEDIUM] Dead code in viz helpers

- **File**: `crates/rs_cam_viz/src/compute/worker/helpers.rs:332-380`
- **Description**: Five functions marked `#[allow(dead_code)]`: `make_depth`, `make_depth_with_finishing`, `make_depth_ext`, `make_depth_from_heights`, `run_collision_check`. These were used before core delegation was added.
- **Fix**: Delete them or move to test-only modules if any tests depend on them.

### Fix Plan

1. **What to change**: `crates/rs_cam_viz/src/compute/worker/helpers.rs`, delete five functions:
   - `make_depth` (line 332-334, 3 lines)
   - `make_depth_with_finishing` (line 337-344, 8 lines)
   - `make_depth_ext` (line 346-356, 11 lines, called by the above two)
   - `make_depth_from_heights` (line 358-367, 10 lines)
   - `run_collision_check` (line 374-380, 7 lines, delegates to `run_collision_check_with_phase`)

2. **How to change it**: Delete all five functions and their `#[allow(dead_code)]` annotations. These are `pub(super)` or private, so no external callers exist. The `SemanticToolpathOp` impls use `req.depth_stepping` (set by the compute request builder), not these helpers. `run_collision_check_with_phase` is the live version of the collision check.
3. **Dependencies**: None. `make_depth_ext` is called by `make_depth` and `make_depth_with_finishing`, but all three are dead.
4. **Estimated scope**: Small (< 50 lines). Roughly 40 lines deleted.
5. **Risk**: None. All five functions are annotated `#[allow(dead_code)]` and have no callers. Run `cargo check -p rs_cam_viz` to confirm.

#### [MEDIUM] `#[allow(dead_code)]` on `run_simulation` in viz

- **File**: `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:109`
- **Description**: `run_simulation` (without phase tracking) is marked dead. Only `run_simulation_with_phase` is called.
- **Fix**: Delete `run_simulation` since `run_simulation_with_phase` with a no-op closure serves the same purpose.

### Fix Plan

1. **What to change**: `crates/rs_cam_viz/src/compute/worker/execute/mod.rs`, line 109-114. Delete the `run_simulation` function (6 lines). It is a trivial wrapper that calls `run_simulation_with_phase(req, cancel, |_| {})`.
2. **How to change it**: Delete the function and its `#[allow(dead_code)]` annotation. Verify no callers exist by searching for `run_simulation(` (excluding `run_simulation_with_phase`).
3. **Dependencies**: None.
4. **Estimated scope**: Small (< 50 lines). 6 lines deleted.
5. **Risk**: None. The function is dead code.

#### [LOW] `#[allow(dead_code)]` on `bind_scope_to_full_toolpath`

- **File**: `crates/rs_cam_core/src/compute/semantic_helpers.rs:99`
- **Description**: Helper function marked dead. Appears to be a utility that was written for future use.
- **Fix**: Remove if not needed within the next milestone.

### Fix Plan

1. **What to change**: `crates/rs_cam_core/src/compute/semantic_helpers.rs`, line 99-101. Delete `bind_scope_to_full_toolpath` (3 lines + annotation).
2. **How to change it**: Delete the function. Callers that need this behavior can inline `scope.bind_to_toolpath(toolpath, 0, toolpath.moves.len())` -- it is a one-liner.
3. **Dependencies**: None.
4. **Estimated scope**: Small (< 50 lines). 4 lines deleted.
5. **Risk**: None. Dead code, no callers.

#### [LOW] `#[allow(dead_code)]` on `machine` field in `ProjectSession`

- **File**: `crates/rs_cam_core/src/session.rs:502`
- **Description**: `machine: MachineProfile` is annotated `// Will be used in Phase 5+ (CLI/MCP wiring)`. The machine profile is loaded but never queried.
- **Fix**: Acceptable if Phase 5 is on the roadmap. Add a tracking issue reference.

### Fix Plan

1. **What to change**: `crates/rs_cam_core/src/session.rs`, line 502. No code change needed now -- the `machine` field will be consumed when CLI/MCP wiring supports machine envelope validation and axis limits.
2. **How to change it**: Add a comment with a tracking reference (e.g. `// TODO(phase-5): Wire machine profile into toolpath validation and G-code post-processing`) instead of the current vague comment. This makes it grep-able.
3. **Dependencies**: Depends on Phase 5 roadmap items landing.
4. **Estimated scope**: Small (< 50 lines). Comment update only.
5. **Risk**: None.

---

### Category 3: TODO/FIXME/HACK

No TODO, FIXME, HACK, or XXX markers were found in:
- `crates/rs_cam_core/src/compute/` (all files)
- `crates/rs_cam_core/src/session.rs`
- `crates/rs_cam_mcp/src/server.rs`

This is clean.

---

### Category 4: Type Conversion Overhead

#### [MEDIUM] Viz-to-core simulation request conversion is a full structural copy

- **File**: `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:119-160`
- **Description**: `build_core_simulation_request` converts a viz `SimulationRequest` to a core `SimulationRequest` by cloning all fields. The viz and core `SimulationRequest` types are structurally identical except the viz version uses `ToolpathId(usize)` where core uses bare `usize`, and the viz `SimGroupEntry` uses viz `ToolConfig` where core uses `ToolDefinition`. The conversion also rebuilds the `ToolDefinition` via `build_cutter` for every toolpath in every group, and then the result is converted back (boundaries `usize -> ToolpathId`, checkpoints re-wrapped).
- **Impact**: For typical projects (5-10 toolpaths), the overhead is negligible. For large projects or tight iteration loops, the double conversion (viz->core->viz) adds allocation pressure.
- **Fix**: Consider having viz use core types directly for simulation, or at minimum avoid the `build_cutter` call during conversion by caching the `ToolDefinition` on the viz side.

### Fix Plan

1. **What to change**: `crates/rs_cam_viz/src/compute/worker/execute/mod.rs`, the `build_core_simulation_request` function (line 119). Currently it calls `build_cutter(&toolpath.tool)` for every toolpath entry during conversion. Cache the `ToolDefinition` (core type) alongside the viz `ToolConfig` in the viz `SimGroupEntry` or compute it once during request building (before this function is called).
2. **How to change it**: Add a `cached_tool_def: Option<ToolDefinition>` field to the viz `SimToolpathEntry`, populated when the compute request is built. In `build_core_simulation_request`, use the cached value instead of calling `build_cutter`. Alternatively, compute a `HashMap<ToolId, ToolDefinition>` once before the loop and look up by tool reference.
3. **Dependencies**: None. This is an optimization that can be done independently.
4. **Estimated scope**: Small (< 50 lines). One new field + one lookup change.
5. **Risk**: Low. The `ToolDefinition` is deterministic from `ToolConfig`, so caching it cannot produce different results. The only risk is stale cache if tool params change between request build and simulation, but the compute pipeline rebuilds requests on every run.

#### [LOW] Collision check request clones toolpath and mesh

- **File**: `crates/rs_cam_viz/src/compute/worker/helpers.rs:393-397`
- **Description**: `run_collision_check_with_phase` creates `core_cc::CollisionCheckRequest` by cloning `(*req.toolpath).clone()` and `(*req.mesh).clone()`. These are potentially large data structures (meshes with 100K+ triangles).
- **Fix**: Consider passing `Arc<Toolpath>` and `Arc<TriangleMesh>` into the core collision check API, or change the core API to take references instead of owned values.

### Fix Plan

1. **What to change**: Two places need coordinated changes:
   - `crates/rs_cam_core/src/compute/collision_check.rs`: Change `CollisionCheckRequest` to hold references (`&Toolpath`, `&TriangleMesh`) instead of owned values, or accept `Arc` wrappers.
   - `crates/rs_cam_viz/src/compute/worker/helpers.rs` line 393-397: Remove `(*req.toolpath).clone()` and `(*req.mesh).clone()`.

2. **How to change it**: The simplest approach is to change core's `CollisionCheckRequest` to hold borrows:
   ```rust
   pub struct CollisionCheckRequest<'a> {
       pub toolpath: &'a Toolpath,
       pub tool: MillingCutter,
       pub mesh: &'a TriangleMesh,
   }
   ```
   The underlying `check_collisions_interpolated_with_cancel` already takes references (line 62-65 of `collision_check.rs`), so the owned fields in the request struct are unnecessary. The viz helper then passes `&*req.toolpath` and `&*req.mesh` without cloning.

3. **Dependencies**: The core `CollisionCheckRequest` is also used by `session.rs` line 1249-1253, which would need its borrows updated. Both callers already have the data as references or `Arc`, so no new cloning is introduced.
4. **Estimated scope**: Small (< 50 lines). Struct definition change + 2 call-site updates.
5. **Risk**: Low. The lifetime parameter propagates to `run_collision_check`, which is called in two places (session and viz). Both have the data in scope for the duration of the call.

---

### Category 5: Inconsistent Patterns

#### [MEDIUM] Operation configs: core vs viz use different config types

- **File**: `crates/rs_cam_core/src/compute/catalog.rs` (core `OperationConfig`)
- **File**: `crates/rs_cam_viz/src/compute/worker/*.rs` (viz config types like `PocketConfig`, `DropCutterConfig`, etc.)
- **Description**: Core defines `OperationConfig` as an enum with 23 variants, each containing operation-specific config structs from `operation_configs.rs`. The viz layer has its own parallel config types (e.g. `PocketConfig` in viz state) that contain the same fields plus GUI-specific additions (e.g. `finishing_passes`, `climb`). The session's `ToolpathConfig.operation` uses core `OperationConfig`, but the viz worker uses viz-specific config types. This means viz does NOT delegate operation execution to core's `execute_operation` -- it calls the algorithms directly with viz-specific params.
- **Impact**: Two parallel config hierarchies that must be kept in sync when adding new parameters.
- **Fix**: Long-term: have viz config types implement a trait that produces a core `OperationConfig`, then delegate to `execute_operation`. Short-term: document the mapping and add integration tests that verify sync.

### Fix Plan

1. **What to change**:
   - **Short-term**: Add a compile-time or test-time check that the viz config types and core `OperationConfig` variants stay in sync. Create a test in `crates/rs_cam_viz/tests/` that iterates over all `OperationType` variants and asserts the viz layer has a matching `SemanticToolpathOp` implementation. This catches drift when new operations are added to core but not viz.
   - **Long-term**: Add a `fn to_core_config(&self) -> OperationConfig` method to each viz config type (e.g. `PocketConfig`, `DropCutterConfig`). Then refactor `SemanticToolpathOp::execute()` to call `execute_operation(self.to_core_config(), ...)` with semantic tracing wrapped around it. This eliminates the parallel dispatch entirely.

2. **How to change it**: Short-term: one new integration test (~30 lines). Long-term: implement `Into<OperationConfig>` on each viz config type, then rewrite the `SemanticToolpathOp` impls as thin wrappers.
3. **Dependencies**: The long-term fix depends on the parallel dispatch unification (Category 2, HIGH finding). They should be done together.
4. **Estimated scope**: Short-term: Small (< 50 lines). Long-term: Large (200+ lines) -- 23 `Into` implementations + 23 `SemanticToolpathOp` rewrites.
5. **Risk**: Short-term: None. Long-term: High -- the viz configs contain GUI-specific fields (e.g. `finishing_passes`, `climb`) that may not map cleanly to core `OperationConfig` variants. Careful mapping and integration testing is needed.

#### [LOW] Naming inconsistency: `ToolpathId` vs bare `usize`

- **File**: `crates/rs_cam_core/src/session.rs` (uses `usize` for toolpath indices)
- **File**: `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` (uses `ToolpathId(usize)`)
- **Description**: Core session uses bare `usize` indices throughout (e.g. `generate_toolpath(index: usize)`). Viz uses a newtype `ToolpathId(usize)`. The MCP server converts between them implicitly.
- **Fix**: Consider adopting `ToolpathId` in core for type safety, or document the intentional difference.

### Fix Plan

1. **What to change**: `crates/rs_cam_core/src/session.rs` and related core types. Introduce a `ToolpathId(usize)` newtype in core (e.g. `crates/rs_cam_core/src/compute/mod.rs` or a dedicated `ids.rs` module) and replace bare `usize` parameters in the public API: `generate_toolpath(index: usize)` -> `generate_toolpath(id: ToolpathId)`, `get_result(index: usize)` -> `get_result(id: ToolpathId)`, etc.
2. **How to change it**: Define `#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub struct ToolpathId(pub usize);` in core. Update session methods. The MCP server would construct `ToolpathId` from the deserialized `usize`. The viz layer could then re-export core's `ToolpathId` instead of defining its own.
3. **Dependencies**: This is a cross-cutting change that touches session, MCP server, and potentially CLI. It should be done in a dedicated PR with no other changes.
4. **Estimated scope**: Medium (50-200 lines). The newtype is small, but the session API has ~15 methods that accept `usize` indices.
5. **Risk**: Medium. Changing public API signatures will break downstream callers (MCP server, CLI). All callers are in-workspace, so the compiler will catch everything, but it is a broad change.

---

### Category 6: Clippy Suppressions

#### [LOW] `#[allow(clippy::too_many_arguments)]` on `execute_operation`

- **File**: `crates/rs_cam_core/src/compute/execute.rs:54`
- **Description**: `execute_operation` takes 12 parameters. This is inherently complex but the suppression is justified given the function's role as a universal operation dispatcher.
- **Fix**: Consider a builder pattern or a `OperationContext` struct to group related params (mesh, index, polygons, tool_def, tool_cfg).

### Fix Plan

1. **What to change**: `crates/rs_cam_core/src/compute/execute.rs`, the `execute_operation` function signature (line 55-68). Group the 12 parameters into a context struct:
   ```rust
   pub struct OperationContext<'a> {
       pub mesh: Option<&'a TriangleMesh>,
       pub index: Option<&'a SpatialIndex>,
       pub polygons: Option<&'a [Polygon2]>,
       pub tool_def: &'a ToolDefinition,
       pub tool_cfg: &'a ToolConfig,
       pub heights: &'a ResolvedHeights,
       pub cutting_levels: &'a [f64],
       pub stock_bbox: &'a BoundingBox3,
       pub prev_tool_radius: Option<f64>,
       pub debug_ctx: Option<&'a ToolpathDebugContext>,
       pub cancel: &'a AtomicBool,
   }
   ```
   Then `execute_operation(op: &OperationConfig, ctx: &OperationContext)`.

2. **How to change it**: Define the struct in `execute.rs`. Update the function signature. Update the two callers: `session.rs` `generate_toolpath()` (line 1014-1027) and the viz `SemanticToolpathOp` impls (if they use `execute_operation` after unification). The `#[allow(clippy::too_many_arguments)]` can then be removed.
3. **Dependencies**: Best done after or alongside the parallel dispatch unification, since both touch `execute_operation`'s signature. If done first, the viz layer callers (which call algorithms directly, not `execute_operation`) are unaffected.
4. **Estimated scope**: Medium (50-200 lines). Struct definition (~15 lines) + 2 call-site refactors.
5. **Risk**: Low. The struct is a mechanical grouping of existing parameters. Both callers construct all these values locally, so wrapping them in a struct is straightforward.

#### [LOW] `#[allow(clippy::needless_pass_by_value)]` on MCP tool parameters

- **File**: `crates/rs_cam_mcp/src/server.rs:369,393,417,483`
- **Description**: Four MCP tool handler functions suppress `needless_pass_by_value` on their `Parameters<T>` arguments. This is likely required by the `rmcp` framework's macro-generated code.
- **Fix**: No action needed -- framework constraint.

### Fix Plan

1. **What to change**: None. The `rmcp` framework's `#[tool]` macro generates handler signatures that take `Parameters<T>` by value. The `#[allow]` annotations are correct and necessary.
2. **How to change it**: No action.
3. **Dependencies**: N/A.
4. **Estimated scope**: N/A.
5. **Risk**: N/A.

All other `#[allow(clippy::indexing_slicing)]` suppressions have SAFETY comments and are justified. Test modules correctly carry the standard test exemptions.

---

### Category 7: Missing Tests

#### [HIGH] Core `execute.rs` has zero tests

- **File**: `crates/rs_cam_core/src/compute/execute.rs`
- **Description**: The unified operation dispatch function `execute_operation` has no unit tests. It is exercised indirectly through integration tests (`param_sweep.rs`) and the session tests, but there are no targeted tests for:
  - Error paths (missing geometry, invalid tool type, cancelled)
  - Dressup pipeline (`apply_dressups`)
  - Edge cases (empty polygon lists, zero-depth operations)
- **Fix**: Add a test module with at least one test per error path and a smoke test for each operation family (2D, 3D, stock-based).

### Fix Plan

1. **What to change**: `crates/rs_cam_core/src/compute/execute.rs` -- add a `#[cfg(test)] mod tests` block at the bottom of the file. Minimum test coverage:
   - **Error path tests** (4 tests):
     - `test_missing_polygons_error`: Call a 2D operation (e.g. `Pocket`) with `polygons: None` and assert `MissingGeometry`.
     - `test_missing_mesh_error`: Call a 3D operation (e.g. `DropCutter`) with `mesh: None` and assert `MissingGeometry`.
     - `test_invalid_tool_vcarve`: Call `VCarve` with an `EndMill` tool type and assert `InvalidTool`.
     - `test_cancelled_operation`: Set `cancel` to `true` before calling `DropCutter` and assert `Cancelled`.
   - **Smoke tests** (3 tests):
     - `test_face_operation`: Execute `Face` on a stock bbox and verify the toolpath has moves.
     - `test_pocket_operation`: Execute `Pocket` with a simple square polygon and verify non-empty output.
     - `test_apply_dressups_noop`: Call `apply_dressups` with all dressups disabled and verify the toolpath is unchanged.

2. **How to change it**: Create test helpers for building minimal `OperationConfig` variants, a dummy `ToolDefinition` (6mm flat endmill), `ResolvedHeights`, and `BoundingBox3`. Use a simple square `Polygon2` for 2D tests. For 3D smoke tests, build a minimal box mesh (12 triangles) with `SpatialIndex`.
3. **Dependencies**: None. The test module is self-contained.
4. **Estimated scope**: Medium (50-200 lines). ~7 tests at ~15-20 lines each, plus ~30 lines of test helpers.
5. **Risk**: Low. These are additive tests. The main risk is test fragility if operation output format changes, but smoke tests should only assert non-empty output, not exact move counts.

#### [MEDIUM] Session mutation methods have no tests

- **File**: `crates/rs_cam_core/src/session.rs`
- **Description**: `set_toolpath_param` and `set_tool_param` are untested. The existing tests cover loading, stock bbox, diagnostics, and tool type parsing -- but no mutation or computation. There are no tests for:
  - `generate_toolpath` / `generate_all`
  - `run_simulation`
  - `set_toolpath_param` / `set_tool_param`
  - `export_gcode`
  - `collision_check`
- **Impact**: These mutation methods are the primary API surface for CLI and MCP, so untested behavior risks regressions.
- **Fix**: Add tests using a minimal project file with at least one 2D toolpath (e.g. a pocket on a square polygon) to exercise the generate -> simulate -> diagnose path.

### Fix Plan

1. **What to change**: `crates/rs_cam_core/src/session.rs`, expand the existing `#[cfg(test)] mod tests` block (currently at line 1617 with 4 tests: `empty_project_loads`, `stock_bbox_from_defaults`, `diagnostics_empty_session`, `tool_type_parsing`). Add tests for mutation and computation methods. Minimum coverage:
   - **`test_set_toolpath_param`**: Build a `ProjectFile` with one pocket toolpath, construct a session, call `set_toolpath_param(0, "stepover", json!(1.5))`, verify the operation config was updated and the cached result was invalidated.
   - **`test_set_toolpath_param_unknown`**: Call `set_toolpath_param(0, "nonexistent", json!(1.0))` and assert `InvalidParam` error.
   - **`test_set_tool_param`**: Build a session with one tool, call `set_tool_param(0, "diameter", json!(8.0))`, verify the tool config was updated.
   - **`test_set_tool_param_unknown`**: Call `set_tool_param(0, "nonexistent", json!(1.0))` and assert `InvalidParam` error.
   - **`test_generate_and_diagnose`**: Build a session with a face operation (simplest -- no geometry required), call `generate_toolpath(0, &cancel)`, verify non-empty result, then call `diagnostics()` and verify per-toolpath stats are populated.

2. **How to change it**: Build `ProjectFile` structs programmatically (the existing tests already do this pattern). For mutation tests, the key is including at least one tool and one toolpath with a valid `OperationConfig`. The `Face` operation is easiest since it only requires a stock bbox, no polygons or mesh.
3. **Dependencies**: None. But this fix is complementary to the `execute.rs` test fix above -- together they cover the two main API surfaces.
4. **Estimated scope**: Medium (50-200 lines). ~5 tests at ~20-30 lines each, plus test helpers for building a `ProjectFile` with a tool + toolpath.
5. **Risk**: Low. Additive tests. The main challenge is constructing valid `ToolpathConfig` and `OperationConfig` structs programmatically (they have many fields). Using `..Default::default()` where possible reduces boilerplate.

#### [LOW] Simulation tests don't cover metric sampling

- **File**: `crates/rs_cam_core/src/compute/simulate.rs:462-673`
- **Description**: The test module has 3 tests: basic simulation, cancellation, and per-setup multi-stock. None enable `metric_options.enabled = true`, so the metric sampling and cut-trace assembly paths are untested at the unit level.
- **Fix**: Add a test with `metric_options.enabled = true` and verify that `cut_trace` is `Some` with non-empty samples.

### Fix Plan

1. **What to change**: `crates/rs_cam_core/src/compute/simulate.rs`, the `#[cfg(test)] mod tests` block (line 462). Add one new test:
   - **`test_simulation_with_metrics`**: Clone the existing `simple_request()` helper, set `metric_options.enabled = true` and `metric_options.sample_interval_mm = 1.0` (or similar), run `run_simulation`, and assert:
     - `result.cut_trace.is_some()`
     - `result.cut_trace.unwrap().summary.total_runtime_s > 0.0`
     - The cut trace has non-empty sample data (tool engagement, MRR, etc.)

2. **How to change it**: The `simple_request()` helper already builds a valid `SimulationRequest` with `metric_options: SimulationMetricOptions::default()`. Modify a copy to set `enabled = true`. The existing `SimulationMetricOptions` struct likely has `enabled: bool` defaulting to `false`.
3. **Dependencies**: None. Self-contained test.
4. **Estimated scope**: Small (< 50 lines). One test, ~15-20 lines.
5. **Risk**: Very low. Additive test. If the metric sampling path has bugs, this test will expose them, which is the point.

---

## Priority Summary

| Severity | Count | Key Items |
|----------|-------|-----------|
| HIGH     | 3     | Diagnostics collision_count always 0; parallel dispatch duplication; no execute.rs tests |
| MEDIUM   | 8     | Unused semantic root; inconsistent MCP errors; dead viz helpers; untested mutation methods; type conversion overhead; dual config hierarchies |
| LOW      | 7     | Naming inconsistency; dead utility functions; framework-required suppressions; machine field; simulation metric tests |
