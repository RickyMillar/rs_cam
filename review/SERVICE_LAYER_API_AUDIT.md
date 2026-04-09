# Service Layer API Audit

**Date**: 2026-04-08
**Commit**: 127ff2c (master)
**Scope**: ProjectSession API surface, MCP tool coverage, CLI parity, GUI delegation

---

## 1. ProjectSession Public API Inventory

All public methods on `ProjectSession` (`crates/rs_cam_core/src/session.rs`):

### Lifecycle
| Method | GUI | CLI | MCP | Notes |
|--------|-----|-----|-----|-------|
| `load(path)` | N/A (uses its own state) | Yes (`project` cmd) | Yes (`load_project`) | |
| `from_project_file(file, base_dir)` | N/A | N/A | N/A | Construction helper, not a user-facing entry |

### Queries
| Method | GUI | CLI | MCP | Notes |
|--------|-----|-----|-----|-------|
| `name()` | Yes | Yes | Yes (`project_summary`) | |
| `stock_config()` | Yes | Yes | N/A | Not directly exposed via MCP |
| `stock_bbox()` | Yes | Yes | Yes (within `project_summary`) | Dimensions exposed, not raw bbox |
| `list_toolpaths()` | Yes | Yes | Yes (`list_toolpaths`) | |
| `list_tools()` | Yes | Yes | Yes (`list_tools`) | |
| `get_toolpath_params(index)` | Yes | Yes | Partial (`get_toolpath_params`) | MCP returns metadata not the full OperationConfig fields |
| `get_tool(id)` | Yes | Yes | N/A | No MCP tool to get full tool details by ID |
| `get_result(index)` | Yes | Yes | N/A | Internal; stats exposed through diagnostics |
| `simulation_result()` | Yes | Yes | N/A | Internal; exposed via `screenshot_simulation` |
| `toolpath_count()` | Yes | Yes | Yes (in `project_summary`) | |
| `setup_count()` | Yes | Yes | Yes (in `project_summary`) | |
| `list_setups()` | N/A | Yes | N/A | **Not exposed via MCP** |
| `get_toolpath_config(index)` | Yes | Yes | Yes (`get_toolpath_params`) | |
| `post_config()` | Yes | Yes | N/A | Not exposed via MCP |

### Mutation
| Method | GUI | CLI | MCP | Notes |
|--------|-----|-----|-----|-------|
| `set_toolpath_param(index, param, value)` | Yes | N/A | Yes (`set_toolpath_param`) | **Not available from CLI** |
| `set_tool_param(index, param, value)` | Yes | N/A | Yes (`set_tool_param`) | **Not available from CLI** |

### Compute
| Method | GUI | CLI | MCP | Notes |
|--------|-----|-----|-----|-------|
| `generate_toolpath(index, cancel)` | N/A (own path) | Indirect | Yes (`generate_toolpath`) | GUI uses its own `SemanticToolpathOp` dispatch |
| `generate_all(skip, cancel)` | N/A | Yes | Yes (`generate_all`) | |
| `run_simulation(opts, cancel)` | Delegates to core | Yes | Yes (`run_simulation`) | |
| `collision_check(index, cancel)` | Yes | Yes | **No** | **Not exposed as MCP tool** |

### Analysis
| Method | GUI | CLI | MCP | Notes |
|--------|-----|-----|-----|-------|
| `diagnostics()` | Yes | Yes | Yes (`get_diagnostics`) | |

### Export
| Method | GUI | CLI | MCP | Notes |
|--------|-----|-----|-----|-------|
| `export_gcode(path, setup_id)` | Yes | Yes | Yes (`export_gcode`) | |
| `export_diagnostics_json(output_dir)` | N/A | Yes | **No** | **Not exposed as MCP tool** |

---

## 2. MCP Tool Coverage

### All MCP Tools (13 tools)

| MCP Tool | ProjectSession Method | Clear? | Defaults? |
|----------|----------------------|--------|-----------|
| `load_project` | `ProjectSession::load()` | Yes | N/A |
| `project_summary` | `name()`, `stock_bbox()`, `setup_count()`, `toolpath_count()`, `list_tools()` | Yes | N/A |
| `list_toolpaths` | `list_toolpaths()` | Yes | N/A |
| `list_tools` | `list_tools()` | Yes | N/A |
| `get_toolpath_params` | `get_toolpath_config()` | Partial | N/A |
| `generate_toolpath` | `generate_toolpath()` | Yes | N/A |
| `generate_all` | `generate_all()` | Yes | N/A |
| `run_simulation` | `run_simulation()` | Yes | resolution=0.5 |
| `get_diagnostics` | `diagnostics()` | Yes | N/A |
| `export_gcode` | `export_gcode()` | Yes | N/A |
| `set_toolpath_param` | `set_toolpath_param()` | Yes | N/A |
| `set_tool_param` | `set_tool_param()` | Yes | N/A |
| `screenshot_simulation` | `simulation_result()` + core render | Yes | 1200x800, last checkpoint |
| `screenshot_toolpath` | `get_result()` + core render | Yes | 1200x800 |

### Missing MCP Tools

| Gap | Severity | Description |
|-----|----------|-------------|
| `collision_check` | **HIGH** | No way to run holder/shank collision check via MCP. CLI `project` command does this automatically; MCP has no equivalent. Agents cannot verify safe stickout. |
| `list_setups` | **MEDIUM** | No way to discover setup names/orientations via MCP. Useful for multi-setup projects (flip jigs, two-sided machining). |
| `export_diagnostics_json` | **LOW** | Not critical since `get_diagnostics` returns the data inline. Disk export is a convenience. |
| `get_tool_details` | **LOW** | `list_tools` gives summary (id, name, type, diameter). Full tool geometry (stickout, shank, holder dims) is only available by reading the project TOML. |
| `stock_config` / `post_config` | **LOW** | No way to query stock origin offset, material, post-processor format, or spindle speed via MCP. |

### Tool Description Quality

Descriptions are generally clear and accurate. Notable issues:

- `get_toolpath_params`: Description says "Get operation parameters" but actually returns metadata (id, name, enabled, operation label, tool_id, model_id) rather than the actual tunable parameters (feed_rate, stepover, depth_per_pass, etc.). **This is misleading for agents trying to discover what to pass to `set_toolpath_param`.**
- `generate_all`: Returns a count string, not structured JSON like other tools. Minor inconsistency.
- `run_simulation`: `skip_ids` in `SimulationOptions` is always empty from MCP. No way to skip toolpaths during simulation.

---

## 3. CLI Coverage

### CLI Commands

| Command | Uses ProjectSession? | MCP Equivalent? |
|---------|---------------------|-----------------|
| `project` | Yes (full pipeline) | Yes (manually: load_project + generate_all + run_simulation + get_diagnostics) |
| `job` | **No** (uses `job.rs` direct core calls) | **No** |
| `drop-cutter` | **No** (direct core calls) | No (but available as an operation type in project files) |
| `pocket` | **No** (direct core calls) | No |
| `profile` | **No** (direct core calls) | No |
| `adaptive` | **No** (direct core calls) | No |
| `vcarve` | **No** (direct core calls) | No |
| `rest` | **No** (direct core calls) | No |
| `adaptive3d` | **No** (direct core calls) | No |
| `waterline` | **No** (direct core calls) | No |
| `ramp-finish` | **No** (direct core calls) | No |
| `steep-shallow` | **No** (direct core calls) | No |
| `inlay` | **No** (direct core calls) | No |
| `pencil` | **No** (direct core calls) | No |
| `scallop` | **No** (direct core calls) | No |
| `sweep` | **No** (uses `job.rs`) | **No** |

### CLI-Only Features Not in MCP

| Feature | Severity | Description |
|---------|----------|-------------|
| Single-operation CLI commands | LOW | 13 standalone operation commands (pocket, profile, etc.) bypass ProjectSession entirely. These are convenience wrappers for quick one-off toolpaths, not the project workflow. MCP correctly targets the project workflow. |
| `job` command | MEDIUM | Legacy TOML job format with its own parser, multi-operation pipeline, diagnostics, and per-phase G-code export. Not accessible via MCP. |
| `sweep` command | LOW | Parameter sweep for benchmarking. Development tool, not a production workflow. |
| Setup filtering (`--setup`) | MEDIUM | CLI `project` command can filter by setup name. MCP has no equivalent; `generate_all` and `run_simulation` process all setups. |
| Skip IDs (`--skip`) | LOW | CLI can skip specific toolpath IDs. MCP `generate_all` and `run_simulation` always process all enabled toolpaths. |
| Collision check per-toolpath | HIGH | CLI `project` command runs `collision_check()` for every toolpath. MCP has no collision_check tool. |

### MCP-Only Features Not in CLI

| Feature | Severity | Description |
|---------|----------|-------------|
| `set_toolpath_param` | MEDIUM | CLI cannot modify toolpath parameters at runtime. It loads a fixed TOML. |
| `set_tool_param` | MEDIUM | CLI cannot modify tool parameters at runtime. |
| `screenshot_simulation` | LOW | CLI has no PNG composite output (it does have `--view` for HTML, and the `project` command writes JSON). |
| `screenshot_toolpath` | LOW | CLI has no per-toolpath visualization output in the `project` workflow. |

---

## 4. GUI to Core Delegation

### Operation Execution: NOT FULLY DELEGATED

The GUI (`crates/rs_cam_viz/src/compute/worker/execute/`) does **not** delegate to `core::compute::execute::execute_operation`. Instead, it maintains a **parallel dispatch** via `SemanticToolpathOp::generate_with_tracing()` for each of the 22 operation types.

Each GUI operation implementation calls the same underlying core algorithms (e.g., `pocket_toolpath`, `batch_drop_cutter`, `adaptive_3d_toolpath_annotated`, etc.) but wraps them with GUI-specific concerns:
- Semantic trace annotation (ToolpathSemanticKind items for rows, levels, etc.)
- Phase tracker integration for progress UI
- Debug span linkage

**This means there are two independent dispatch paths for operations:**
1. `core::compute::execute::execute_operation` (used by `ProjectSession` and MCP/CLI)
2. `viz::compute::worker::execute::semantic_op().generate_with_tracing()` (used by GUI)

Both call the same leaf algorithms, but the dispatch logic and parameter wiring is duplicated.

### Operations Verified in GUI Dispatch (22/23)

All 22 operation types have `SemanticToolpathOp` implementations in the GUI:
- **2D** (12): Face, Pocket, Profile, Adaptive, VCarve, Rest, Inlay, Zigzag, Trace, Drill, Chamfer, AlignmentPinDrill
- **3D** (10): DropCutter, Adaptive3d, Waterline, Pencil, Scallop, SteepShallow, RampFinish, SpiralFinish, RadialFinish, HorizontalFinish, ProjectCurve

**Count check**: The OperationConfig enum has 22 variants. The GUI's `semantic_op()` match covers all 22. Core's `execute_operation` also handles all 22. Consistent.

### Simulation: FULLY DELEGATED

The GUI's `run_simulation_with_phase()` properly delegates to `rs_cam_core::compute::simulate::run_simulation_with_phase()`. It converts the viz request types to core request types, then wraps the result back with viz-specific playback data. This is correct.

### Dressups: INDEPENDENT PATHS

The GUI applies dressups in `run_compute_with_phase_tracker()` using its own inline pipeline. Core has `apply_dressups()` in `compute/execute.rs` which `ProjectSession::generate_toolpath()` calls. Both implement the same logic (entry style, dogbones, lead in/out, link moves, arc fitting) but are separate implementations.

---

## 5. Feature Parity Matrix

### Complete Workflow Parity

| Capability | GUI | CLI | MCP |
|-----------|-----|-----|-----|
| Load project TOML | Yes | Yes (`project`) | Yes (`load_project`) |
| List toolpaths | Yes | Yes (in JSON output) | Yes (`list_toolpaths`) |
| List tools | Yes | Yes (in JSON output) | Yes (`list_tools`) |
| Generate single toolpath | Yes | Implicit | Yes (`generate_toolpath`) |
| Generate all toolpaths | Yes | Yes | Yes (`generate_all`) |
| Modify toolpath params | Yes | No | Yes (`set_toolpath_param`) |
| Modify tool params | Yes | No | Yes (`set_tool_param`) |
| Run simulation | Yes | Yes | Yes (`run_simulation`) |
| Get diagnostics | Yes | Yes (JSON files) | Yes (`get_diagnostics`) |
| Collision check | Yes | Yes | **No** |
| Export G-code | Yes | Yes | Yes (`export_gcode`) |
| Screenshot simulation | Yes (viewport) | No | Yes (`screenshot_simulation`) |
| Screenshot toolpath | Yes (viewport) | No | Yes (`screenshot_toolpath`) |
| List setups | Yes | Yes | **No** |
| Setup filtering | Yes | Yes (`--setup`) | **No** |
| Skip toolpaths | Yes | Yes (`--skip`) | **No** |
| Single-operation commands | N/A | Yes (13 commands) | N/A |
| Parameter sweep | N/A | Yes (`sweep`) | N/A |

### All 22 Operations Available from Each Interface

All interfaces support all 22 operation types when working through the project file format. The CLI standalone commands cover only a subset (13 operations), but these are convenience shortcuts, not the primary workflow.

---

## 6. Documentation and Configuration

### MCP Configuration

The `.mcp.json` at the repo root is **empty** (`"mcpServers": {}`). There is no registered MCP server configuration. Users must manually configure their MCP client to run `rs_cam_mcp`.

The binary is named `rs_cam_mcp` (matching the `[[bin]]` in `crates/rs_cam_mcp/Cargo.toml`).

### Parameter Discovery

There is no MCP tool that lists the available parameters for a given operation type. An agent calling `get_toolpath_params` gets metadata (operation label, tool_id) but not the actual field names they could pass to `set_toolpath_param`. To discover settable fields, an agent would need to:
1. Know the operation type from `get_toolpath_params`
2. Guess or have prior knowledge of the field names for that type

The `set_toolpath_param` description hints at some common params ("feed_rate", "stepover", "depth_per_pass", "plunge_rate") and some config-specific ones ("angle", "min_z", "passes"), but this is incomplete for 22 operation types.

---

## 7. Findings Summary

### HIGH Severity

| # | Gap | Description | Recommended Action |
|---|-----|-------------|--------------------|
| H1 | Missing `collision_check` MCP tool | Holder/shank collision detection is a safety-critical capability. CLI runs it automatically; MCP has no way to invoke it. Agents cannot verify tool assembly clearance. | Add `collision_check` MCP tool wrapping `ProjectSession::collision_check()`. Return collision count, min safe stickout, and top-N collision positions. |
| H2 | GUI operation dispatch duplication | GUI maintains 22 independent `SemanticToolpathOp` implementations parallel to core's `execute_operation`. Changes to operation logic must be made in two places. Risk of drift. | Refactor GUI to delegate to `core::compute::execute::execute_operation` and wrap results with semantic tracing post-hoc, or pass semantic/debug recorders into the core function. |

### Fix Plan: H1 — Missing `collision_check` MCP tool

1. **What to change**
   - `crates/rs_cam_mcp/src/server.rs`: Add a `CollisionCheckParam` struct (fields: `index: usize`) and a new `collision_check` method on `CamServer`.
2. **How to change it**
   - Follow the existing `generate_toolpath` pattern: clone the `Arc<TokioMutex<...>>` session handle, `spawn_blocking`, acquire the lock, call `session.collision_check(index, &cancel)`.
   - On success, serialize a JSON response with `collision_count`, `min_safe_stickout`, `is_clear`, and the first 5 collision positions (from `result.collision_report.collisions`). Each position should include `move_idx`, `position: {x,y,z}`, `penetration_depth`, and `segment` (shank vs holder).
   - On `SessionError::MissingGeometry`, return a non-error message like `"Collision check not applicable (2D operation)"` so agents don't treat it as a hard failure.
   - Register the tool with `#[tool(name = "collision_check", description = "Run holder/shank collision check for a generated toolpath. Requires the toolpath to have been generated and a 3D mesh model. Returns collision count, minimum safe stickout, and top collision positions.")]`.
3. **Dependencies** — None. `ProjectSession::collision_check()` already exists at `session.rs:1224` and returns `CollisionCheckResult { collision_report, collision_positions }`.
4. **Estimated scope** — Small (< 50 lines). The param struct is ~5 lines, the handler ~35 lines.
5. **Risk** — Low. This is a read-only query on existing state. The only edge case is calling it before `generate_toolpath` (returns `ToolpathNotFound`) or on 2D operations (returns `MissingGeometry`), both of which are already handled by `ProjectSession`.

### Fix Plan: H2 — GUI operation dispatch duplication

1. **What to change**
   - `crates/rs_cam_viz/src/compute/worker/execute/operations_2d.rs` and `operations_3d.rs`: Replace 22 `SemanticToolpathOp::generate_with_tracing()` implementations with delegation to `core::compute::execute::execute_operation`.
   - `crates/rs_cam_core/src/compute/execute.rs` (`execute_operation` function): Accept optional `ToolpathDebugContext` and `ToolpathSemanticContext` parameters (these are already partially wired — the session path passes `debug_context` at `session.rs:1025`).
   - `crates/rs_cam_viz/src/compute/worker/execute/mod.rs`: Simplify `run_compute_with_phase_tracker` to call `execute_operation` + apply dressups, wrapping results with the GUI's phase tracker and semantic annotations.
2. **How to change it**
   - **Phase 1 (audit)**: Compare each of the 22 `SemanticToolpathOp` implementations with the corresponding arm in `core::compute::execute::execute_operation`. Identify any GUI-specific parameter wiring that differs from core (e.g., extra rounding, different defaults). This must be done per-operation to avoid regressions.
   - **Phase 2 (core extension)**: Add optional callback hooks to `execute_operation` for progress reporting (the GUI currently uses `ToolpathPhaseTracker` for progress UI). This might mean a `ComputeProgressReporter` trait or a closure parameter.
   - **Phase 3 (GUI migration)**: Replace the match in `semantic_op()` with a single path that calls `execute_operation`, then wraps the resulting `Toolpath` with the semantic trace annotations that the GUI adds (move ranges per row/level, etc.). The GUI's `ToolpathSemanticKind` annotations (rows, levels, entry) are currently assigned *during* generation; they would need to be assigned *after* based on move index ranges from debug traces.
   - **Phase 4 (cleanup)**: Delete the 22 individual `SemanticToolpathOp` impl blocks and their associated helper functions that duplicate core logic.
3. **Dependencies** — L6 (dressup consolidation) should land first or concurrently, since the dressup pipeline is also duplicated and would cause merge conflicts if tackled separately.
4. **Estimated scope** — Large (200+ lines, likely 500+). Each of the 22 operations needs individual verification, core needs progress-reporting hooks, and the semantic annotation post-processing is nontrivial.
5. **Risk** — **High**. This is the highest-risk item in the audit. The GUI operations have accumulated GUI-specific tweaks (phase tracker integration, cancel-check placement, slightly different parameter assembly). A regression would manifest as incorrect toolpaths in the GUI while CLI/MCP remain correct. Mitigations: (a) run the full param sweep suite after each operation migration, (b) add a visual regression test comparing GUI render output before/after for at least 5 representative operations.

### MEDIUM Severity

| # | Gap | Description | Recommended Action |
|---|-----|-------------|--------------------|
| M1 | `get_toolpath_params` is misleading | Returns metadata (name, id, enabled) not actual tunable parameters (feed_rate, stepover, etc.). Agents cannot discover what fields are settable. | Return the full operation config as a JSON object so agents can see current values and field names. |
| M2 | Missing `list_setups` MCP tool | No way to discover setup names, orientations, or which toolpaths belong to which setup. | Add `list_setups` tool returning setup id, name, face_up, and toolpath indices. |
| M3 | No setup filtering in MCP | CLI can filter by setup (`--setup`), MCP processes everything. For multi-setup projects, agents cannot target one setup. | Add optional `setup_id` param to `generate_all` and `run_simulation`. |
| M4 | CLI `job` command bypasses ProjectSession | The legacy `job` command and all 13 standalone operation commands use direct core calls instead of ProjectSession. Two compute paths for similar workflows. | Consider deprecating standalone CLI commands in favor of `project` command, or wiring `job` through ProjectSession. |
| M5 | CLI lacks parameter mutation | CLI cannot modify toolpath or tool parameters. It can only process the project as-is. | Not critical for batch CLI usage, but limits scripting workflows. Consider adding `set-param` subcommand. |

### Fix Plan: M1 — `get_toolpath_params` is misleading

1. **What to change**
   - `crates/rs_cam_mcp/src/server.rs`: The `get_toolpath_params` handler (line ~202-226).
2. **How to change it**
   - Replace the hand-built JSON object with a serde serialization of the full `ToolpathConfig`. Currently the handler calls `session.get_toolpath_config(index)` and extracts only `id`, `name`, `enabled`, `operation.label()`, `tool_id`, `model_id`. Instead, serialize the `operation` field (which is an `OperationConfig` — already implements `Serialize`) to get the full parameter set.
   - Build the response as:
     ```json
     {
       "id": tc.id,
       "name": tc.name,
       "enabled": tc.enabled,
       "tool_id": tc.tool_id,
       "model_id": tc.model_id,
       "operation": <serde_json::to_value(&tc.operation)>
     }
     ```
   - The `OperationConfig` serializes as `{"kind": "pocket", "params": {"stepover": 3.0, "feed_rate": 1500.0, ...}}` due to `#[serde(tag = "kind", content = "params")]`, which gives agents the exact field names and current values they need for `set_toolpath_param`.
   - Update the tool description to: `"Get full operation configuration for a toolpath by index. Returns operation type, all tunable parameters with current values, and metadata (id, name, enabled, tool_id)."`.
3. **Dependencies** — None.
4. **Estimated scope** — Small (< 50 lines). It's a ~10-line change to the handler plus a description update.
5. **Risk** — Low. `OperationConfig` already has `#[derive(Serialize)]`. The only consideration is that the response payload becomes larger. If agents parse the response, the new structure is a superset of the old one.

### Fix Plan: M2 — Missing `list_setups` MCP tool

1. **What to change**
   - `crates/rs_cam_mcp/src/server.rs`: Add a new `list_setups` tool method on `CamServer`.
2. **How to change it**
   - No new param struct needed (no parameters).
   - Call `session.list_setups()` which returns `&[SetupData]` (defined at `session.rs:392`). For each `SetupData`, serialize `id`, `name`, `face_up` (convert to string via `face_up.key()`), and `toolpath_indices`.
   - Return a JSON array of setup objects:
     ```json
     [{"id": 0, "name": "Setup 1", "face_up": "top", "toolpath_indices": [0, 1, 2]}]
     ```
   - `SetupData` is not `Serialize`, so build the JSON manually with `serde_json::json!()` rather than deriving (avoid touching the core struct).
3. **Dependencies** — None.
4. **Estimated scope** — Small (< 50 lines). ~20 lines for the handler.
5. **Risk** — Minimal. `list_setups()` is an existing public method. The `FaceUp` enum has a `key()` method for string conversion.

### Fix Plan: M3 — No setup filtering in MCP

1. **What to change**
   - `crates/rs_cam_mcp/src/server.rs`: Modify `SimulationParam` and `generate_all` to accept optional filtering parameters.
2. **How to change it**
   - Add `setup_id: Option<usize>` and `skip_ids: Option<Vec<usize>>` fields to `SimulationParam`. Rename the struct to something like `GenerateSimParams` or add a separate `GenerateAllParam`.
   - In the `generate_all` handler: if `setup_id` is provided, compute skip IDs by collecting all toolpath IDs that do *not* belong to that setup (same logic as `project.rs:87-99`). Pass the combined skip list to `session.generate_all(&combined_skip, &cancel)`.
   - In the `run_simulation` handler: if `skip_ids` is provided, pass them into `SimulationOptions::skip_ids`. If `setup_id` is provided, compute the skip list the same way.
   - Add a new `GenerateAllParam` struct:
     ```rust
     pub struct GenerateAllParam {
         /// Only generate toolpaths from this setup ID
         pub setup_id: Option<usize>,
         /// Skip these toolpath IDs
         pub skip_ids: Option<Vec<usize>>,
     }
     ```
   - Update tool descriptions to document the new optional params.
3. **Dependencies** — M2 (list_setups) should land first so agents can discover valid setup IDs.
4. **Estimated scope** — Medium (50-200 lines). Two param structs need modification, two handlers need skip-list logic (~30 lines each), plus the setup-to-skip-IDs resolution helper (~15 lines).
5. **Risk** — Low. The skip-IDs mechanism is already wired through `ProjectSession::generate_all()` and `SimulationOptions::skip_ids`. The setup filtering logic is proven in `project.rs:86-99`. The main risk is agents passing invalid setup IDs, which should produce an empty toolpath set (not a crash).

### Fix Plan: M4 — CLI `job` command bypasses ProjectSession

1. **What to change**
   - `crates/rs_cam_cli/src/job.rs`: The `run_job_command` function.
   - Potentially `crates/rs_cam_core/src/session.rs`: May need a `ProjectSession::from_job_file()` constructor if the job format is different enough from the project format.
2. **How to change it**
   - **Option A (conversion layer)**: Write a `job_to_project_file()` function that converts the legacy `JobFile` structure (parsed in `job.rs`) into a `ProjectFile` struct, then construct a `ProjectSession` from it. This preserves backward compatibility with existing `.toml` job files while routing through the unified session.
   - **Option B (deprecation)**: Add a deprecation warning to the `job` command directing users to the `project` format. Keep the current implementation but document that it's legacy.
   - Option A is recommended. The job format uses the same core types (tools, operations, models) but with a flatter structure. The conversion would map job phases to toolpath configs with sequential IDs.
   - The 13 standalone operation commands (`pocket`, `profile`, etc.) should remain as-is — they are developer convenience tools for quick one-off toolpath generation, not project workflows. They intentionally bypass the session layer.
3. **Dependencies** — None.
4. **Estimated scope** — Medium (50-200 lines) for Option A. The `JobFile` → `ProjectFile` translation is the bulk of the work.
5. **Risk** — Medium. The job format has different semantics for some fields (e.g., per-phase tools, per-phase dressups). A conversion layer could introduce subtle differences in tool/operation resolution. Needs regression testing against existing job files.

### Fix Plan: M5 — CLI lacks parameter mutation

1. **What to change**
   - `crates/rs_cam_cli/src/main.rs`: Add a new `SetParam` subcommand to the `Commands` enum.
2. **How to change it**
   - Add a CLI subcommand like:
     ```
     rs_cam set-param <project.toml> --toolpath <index> --param <name> --value <json_value>
     rs_cam set-param <project.toml> --tool <index> --param <name> --value <json_value>
     ```
   - Load the project via `ProjectSession::load()`, call `set_toolpath_param()` or `set_tool_param()`, then optionally regenerate and export. This enables scripted parameter sweeps without the `sweep` command.
   - Alternatively, extend the existing `project` command with `--set-toolpath "0:stepover=2.0"` and `--set-tool "0:stickout=50"` flags, parsed before `generate_all`.
3. **Dependencies** — None.
4. **Estimated scope** — Small (< 50 lines) for a simple subcommand, Medium if integrated into the `project` command with regeneration.
5. **Risk** — Low. Uses existing `ProjectSession` mutation methods.

### LOW Severity

| # | Gap | Description | Recommended Action |
|---|-----|-------------|--------------------|
| L1 | `generate_all` returns text, not JSON | Unlike other tools that return structured JSON, `generate_all` returns a plain text string ("Generated N toolpaths"). | Return JSON with count and per-toolpath status. |
| L2 | No parameter discovery tool | No MCP tool to list available parameters for a given operation type. | Add `list_operation_params` tool, or enhance `get_toolpath_params` to include the full config schema. |
| L3 | Missing `export_diagnostics_json` MCP tool | Not critical since `get_diagnostics` returns data inline. | Add if disk-based artifact generation is needed by agents. |
| L4 | MCP skip_ids not exposed | `run_simulation` and `generate_all` always process all enabled toolpaths via MCP. | Add optional `skip_ids` parameter to both tools. |
| L5 | Empty `.mcp.json` | No server configuration registered at repo level. | Add the rs_cam_mcp server entry so `claude` can auto-discover it. |
| L6 | Dressup pipeline duplicated | GUI and core both implement the dressup pipeline (entry style, dogbones, link moves, arc fitting). | Consolidate on `core::compute::execute::apply_dressups()`. |
| L7 | `stock_config` / `post_config` not in MCP | Cannot query stock material, origin offset, post format, or spindle speed. | Add to `project_summary` or as separate tools. |

### Fix Plan: L1 — `generate_all` returns text, not JSON

1. **What to change**
   - `crates/rs_cam_mcp/src/server.rs`: The `generate_all` handler (line ~267-291).
2. **How to change it**
   - Replace the `text(format!("Generated {n} toolpaths"))` return with a JSON response. After `generate_all` completes, iterate over all toolpath indices and collect per-toolpath status:
     ```json
     {
       "generated_count": 3,
       "toolpaths": [
         {"index": 0, "name": "Roughing", "status": "generated", "move_count": 1234},
         {"index": 1, "name": "Finishing", "status": "generated", "move_count": 5678},
         {"index": 2, "name": "Disabled Op", "status": "skipped"}
       ]
     }
     ```
   - Use `session.get_result(idx)` to check which toolpaths have results, and `session.get_toolpath_config(idx)` for names. The before/after count logic can be preserved to compute `generated_count`.
   - Change the return to use `json_str()` instead of `text()`.
3. **Dependencies** — None.
4. **Estimated scope** — Small (< 50 lines). The iteration and JSON construction adds ~20 lines, replacing ~5 existing lines.
5. **Risk** — Low. Agents that currently parse the text "Generated N toolpaths" would need to handle JSON instead, but since MCP responses are typed, this is an improvement.

### Fix Plan: L2 — No parameter discovery tool

1. **What to change**
   - This is **effectively solved by M1**. Once `get_toolpath_params` returns the full `OperationConfig` serialization, agents can see all field names and current values for any loaded toolpath.
2. **How to change it**
   - If a standalone discovery tool is still desired (e.g., "what params does a `pocket` operation support without loading a project?"), add a `list_operation_params` tool that takes an `operation_type: String` and returns the fields of the corresponding config struct.
   - Implementation: construct a default instance of the operation config variant (each `XxxConfig` derives `Default`), serialize it, and return the field names and types. For example, `OperationConfig::Pocket(PocketConfig::default())` serialized shows all available fields with their defaults.
   - Register as `#[tool(name = "list_operation_params", description = "List available parameters and defaults for a given operation type (e.g. 'pocket', 'drop_cutter', 'adaptive3d')")]`.
3. **Dependencies** — None, but M1 reduces the urgency to "nice to have."
4. **Estimated scope** — Medium (50-200 lines). The main challenge is mapping a string like `"pocket"` to `OperationConfig::Pocket(PocketConfig::default())` — needs a match over all 22 variants (~50 lines).
5. **Risk** — Low. Default instances are safe to construct. The only subtlety is that some defaults may not be representative (e.g., `feed_rate: 0.0`), which could confuse agents. Adding `"note": "defaults shown; override as needed"` in the response mitigates this.

### Fix Plan: L3 — Missing `export_diagnostics_json` MCP tool

1. **What to change**
   - `crates/rs_cam_mcp/src/server.rs`: Add a new `export_diagnostics_json` tool.
2. **How to change it**
   - Add a param struct with `output_dir: String`.
   - In the handler, call `session.export_diagnostics_json(Path::new(&output_dir))` which already exists at `session.rs:1393`. Return a success message with the output path.
   - Use `spawn_blocking` since the method writes files.
3. **Dependencies** — None.
4. **Estimated scope** — Small (< 50 lines).
5. **Risk** — Minimal. The method already exists and handles error cases internally.

### Fix Plan: L4 — MCP skip_ids not exposed

1. **What to change**
   - This is **addressed by M3** (setup filtering). The `skip_ids` parameter is added to both `generate_all` and `run_simulation` as part of that fix.
2. **How to change it** — See M3.
3. **Dependencies** — Part of M3.
4. **Estimated scope** — Included in M3.
5. **Risk** — See M3.

### Fix Plan: L5 — Empty `.mcp.json`

1. **What to change**
   - `/.mcp.json` at the repo root.
2. **How to change it**
   - Populate with:
     ```json
     {
       "mcpServers": {
         "rs-cam": {
           "command": "cargo",
           "args": ["run", "-p", "rs_cam_mcp", "--bin", "rs_cam_mcp"],
           "cwd": "."
         }
       }
     }
     ```
   - Alternatively, if a pre-built binary path is preferred for faster startup:
     ```json
     {
       "mcpServers": {
         "rs-cam": {
           "command": "target/release/rs_cam_mcp",
           "args": []
         }
       }
     }
     ```
   - The `cargo run` variant is more portable for development; the binary variant is faster for production.
3. **Dependencies** — None.
4. **Estimated scope** — Small (< 50 lines). It's a single config file edit.
5. **Risk** — Minimal. The only consideration is that `cargo run` compiles if needed, which can be slow. A note in the README about `cargo build --release -p rs_cam_mcp` first would help.

### Fix Plan: L6 — Dressup pipeline duplicated

1. **What to change**
   - `crates/rs_cam_viz/src/compute/worker/helpers.rs`: The `apply_dressups` function (line 96+).
   - `crates/rs_cam_core/src/compute/execute.rs`: The `apply_dressups` function (line 726+).
2. **How to change it**
   - The GUI's `apply_dressups` wraps each dressup step with semantic/debug tracing via `apply_dressup_with_tracing()`. The core's `apply_dressups` applies the same steps without tracing.
   - **Option A (extend core)**: Add optional `ToolpathDebugContext` and `ToolpathSemanticContext` parameters to `core::compute::execute::apply_dressups()`. When provided, wrap each step with tracing annotations. The GUI then calls the core function with recorders; CLI/MCP calls it with `None`.
   - **Option B (core + post-hoc annotation)**: Keep the core function untraced. Have the GUI call the core function, then use before/after move counts to assign semantic annotations to the dressup-added moves. This is simpler but loses per-step granularity.
   - Option A is recommended for full parity. The tracing overhead is negligible.
   - After migration, delete the GUI's `apply_dressups` and `apply_dressup_with_tracing` in `helpers.rs`.
3. **Dependencies** — Should land before or concurrently with H2 (GUI dispatch consolidation) to avoid merge conflicts.
4. **Estimated scope** — Medium (50-200 lines). The core function gains ~40 lines of optional tracing code. The GUI function (~100 lines) is deleted.
5. **Risk** — Medium. The GUI's dressup function has subtle differences in how it computes `safe_z` (uses `effective_safe_z(req)` which considers height overrides) vs. core's `apply_dressups` which takes `safe_z` as a parameter. These must be reconciled during migration to avoid incorrect retract heights.

### Fix Plan: L7 — `stock_config` / `post_config` not in MCP

1. **What to change**
   - `crates/rs_cam_mcp/src/server.rs`: The `project_summary` handler (line ~156-175).
2. **How to change it**
   - Expand the `project_summary` JSON response to include stock and post-processor details:
     ```json
     {
       "name": "...",
       "stock": {
         "width": 200.0, "depth": 150.0, "height": 25.0,
         "origin_x": 0.0, "origin_y": 0.0, "origin_z": 0.0,
         "material": "hardwood"
       },
       "post": {
         "format": "grbl",
         "spindle_speed": 18000,
         "safe_z": 10.0
       },
       "setup_count": 1,
       "toolpath_count": 5,
       "tools": [...]
     }
     ```
   - Access stock config via `session.stock_config()` (returns `&StockConfig`). Access post config via `session.post_config()` (returns `&ProjectPostConfig`).
   - `StockConfig` and `ProjectPostConfig` are not `Serialize`, so build the JSON manually with `serde_json::json!()` using their individual fields.
3. **Dependencies** — None.
4. **Estimated scope** — Small (< 50 lines). Adding ~15 lines to the existing `project_summary` response.
5. **Risk** — Minimal. Purely additive to the response; no existing fields are changed.
