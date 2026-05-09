# Agent codemap — rs_cam

Generated with SocratiCode graph tooling on 2026-05-08.

## SocratiCode status

- Code graph built: 241 files, 36 file-dependency edges, 0 circular dependency chains.
- Symbol graph built: 4,545 symbols, 30,092 call edges, 80.8% unresolved.
- Semantic index: started/resumed; see `codebase_status` before semantic search. At generation time it was still completing in the background.
- Context artifacts: none configured (`.socraticodecontextartifacts.json` absent).

Graph caveat: Rust `mod`/crate dependency extraction is sparse in the current graph (many files appear as graph orphans), so use the graph for quick orientation and symbol lookup, but rely on the module map below plus targeted file reads for exact Rust wiring.

## Product layers

| Layer | Path | Purpose |
|---|---|---|
| Core engine | `crates/rs_cam_core` | Geometry, imports, tool modeling, operation configs/generation, dressups, simulation, feeds/speeds, tool-load optimizer, G-code, session API. |
| CLI | `crates/rs_cam_cli` | Batch commands and TOML job execution. Delegates to core/session where possible. |
| GUI | `crates/rs_cam_viz` | Desktop `egui`/`wgpu` app, controller, worker lanes, viewport/UI, embedded MCP bridge. |
| Standalone MCP | `crates/rs_cam_mcp` | Headless/project-session MCP server tools. |

## Core engine map

### Public module root

- `crates/rs_cam_core/src/lib.rs` exports all core modules.
- Operation modules are mostly one file per operation: `pocket.rs`, `profile.rs`, `adaptive/`, `adaptive3d/`, `dropcutter.rs`, `waterline.rs`, `scallop.rs`, etc.

### Data model and operation catalog

| Path | Role |
|---|---|
| `crates/rs_cam_core/src/compute/catalog.rs` | `OperationType`, `OperationConfig`, operation metadata/specs, shared `OperationParams` accessors. |
| `crates/rs_cam_core/src/compute/operation_configs.rs` | Concrete config structs/defaults for the operation variants. |
| `crates/rs_cam_core/src/compute/tool_config.rs` | GUI/session-facing tool config model. |
| `crates/rs_cam_core/src/compute/stock_config.rs` | Stock/material/workholding configuration. |
| `crates/rs_cam_core/src/compute/config.rs` | Shared compute/dressup/boundary/height config. |
| `crates/rs_cam_core/src/toolpath.rs` | Toolpath IR/move representation; boundary between planning and output. |
| `crates/rs_cam_core/src/toolpath_spans.rs` | Structural spans attached to generated toolpaths. |
| `crates/rs_cam_core/src/semantic_trace.rs` | Semantic trace items for operation-level diagnostics. |

### Generation pipeline

| Path | Role |
|---|---|
| `crates/rs_cam_core/src/compute/execute.rs` | Central operation dispatch. SocratiCode symbols: `execute_operation`, `execute_operation_annotated`, `apply_dressups`, validation helpers. |
| `crates/rs_cam_core/src/compute/spans.rs` | Generic span derivation helpers for depth runs, cut runs, drill holes, labeled runtime events. |
| `crates/rs_cam_core/src/compute/annotate.rs` | Semantic annotation helpers for trace/drill/depth regions. |
| `crates/rs_cam_core/src/compute/semantic_helpers.rs` | Helpers for semantic trace construction. |
| `crates/rs_cam_core/src/dressup.rs` | Entry/link/lead/dogbone/arcfit/feed optimization dressups. |
| `crates/rs_cam_core/src/depth.rs` | Depth stepping helpers. |
| `crates/rs_cam_core/src/boundary.rs` / `polygon.rs` | Boundary and polygon utilities. |
| `crates/rs_cam_core/src/tsp.rs` | Rapid-order optimization. |

Typical flow:

1. `ProjectSession::generate_toolpath` in `session/compute.rs` builds context.
2. `compute::execute_operation_annotated` dispatches by `OperationConfig`.
3. Operation module produces `Toolpath` or annotated result.
4. `compute::execute` applies dressups and span/semantic annotation.
5. Result is cached in session/GUI runtime state.

### Session API

| Path | Role |
|---|---|
| `crates/rs_cam_core/src/session/mod.rs` | Session type and module facade. SocratiCode graph imports `compute.rs`, `mutation.rs`, `save.rs`. |
| `crates/rs_cam_core/src/session/compute.rs` | High-level compute API. SocratiCode symbols include `set_toolpath_param`, `generate_toolpath`, `generate_all`, `run_simulation`, `collision_check`, `narrate_toolpath`, `diagnostics`, `export_gcode`, `tool_load_report`. |
| `crates/rs_cam_core/src/session/execution.rs` | Execution/result types and session-side runtime state. |
| `crates/rs_cam_core/src/session/loading.rs` | Project load/import paths. |
| `crates/rs_cam_core/src/session/mutation.rs` | Session mutation methods and invalidation. |
| `crates/rs_cam_core/src/session/save.rs` / `project_file.rs` | Project persistence. |

Use `ProjectSession` APIs when adding CLI/MCP/headless features. Avoid bypassing invalidation by directly mutating lower state.

### Simulation and diagnostics

| Path | Role |
|---|---|
| `crates/rs_cam_core/src/compute/simulate.rs` | Core simulation orchestration, `SimulationRequest`, setup groups, toolpath boundaries/checkpoints. |
| `crates/rs_cam_core/src/dexel_stock/` | Tri-dexel stock engine, stamping, metric sample generation. |
| `crates/rs_cam_core/src/simulation_cut.rs` | Cut trace schema/samples/provenance/summaries. |
| `crates/rs_cam_core/src/narrate.rs` | Agent-readable toolpath narration. |
| `crates/rs_cam_core/src/collision.rs` / `compute/collision_check.rs` | Holder/shank/rapid collision checks. |
| `crates/rs_cam_core/src/stock_mesh.rs`, `dexel_mesh.rs` | Mesh output from stock simulation. |

Typical flow:

1. Session/GUI constructs `SimulationRequest` with generated annotated toolpaths and tools.
2. `compute::simulate` runs setup groups and calls dexel stock stamping/metric paths.
3. Cut samples carry toolpath IDs, semantic IDs, span paths, chipload/power/engagement metrics.
4. GUI/MCP diagnostics consume `SimulationCutTrace` and summaries.

### Tool load, feeds/speeds, optimizer

| Path | Role |
|---|---|
| `crates/rs_cam_core/src/feeds/` | Feeds/speeds calculator, vendor LUT loading/normalization/lookup, chip thinning geometry. |
| `crates/rs_cam_core/data/vendor_lut/` | Embedded vendor observations. |
| `crates/rs_cam_core/src/tool_load/mod.rs` | Tool-load report entrypoint. SocratiCode symbol: `evaluate_toolpath`. |
| `crates/rs_cam_core/src/tool_load/chipload.rs` | Chipload guardrail and steady-state sample filtering. |
| `crates/rs_cam_core/src/tool_load/power.rs` | Power guardrail. |
| `crates/rs_cam_core/src/tool_load/deflection.rs` | Force-aware tip-deflection guardrail. |
| `crates/rs_cam_core/src/tool_load/verdict.rs` | Current shared verdict/confidence/reason types. |
| `crates/rs_cam_core/src/tool_load/optimize/mod.rs` | Optimizer orchestration and public outcome/candidate types. |
| `crates/rs_cam_core/src/tool_load/optimize/policy.rs` | Search policy/provenance values. |
| `crates/rs_cam_core/src/tool_load/optimize/axes.rs` | Search axes, axis context/bindings, optimization surface. |
| `crates/rs_cam_core/src/tool_load/optimize/bounds.rs` / `space.rs` | Axis bounds and search-space construction. |
| `crates/rs_cam_core/src/tool_load/optimize/patches.rs` | Candidate/axis patch application helpers. |
| `crates/rs_cam_core/src/tool_load/optimize/retarget/` | Per-gate retargeter implementations. |
| `crates/rs_cam_core/src/tool_load/optimize/strategy/` | Search strategies: headroom, grid, retarget. |

Optimizer flow today:

1. `optimize_toolpath(session, baseline_trace, toolpath_index, cancel)` builds context from session.
2. Baseline verdict comes from `evaluate_toolpath` over current sim trace.
3. Strategy layer emits candidate patches for headroom/grid/retarget paths.
4. Patch helpers apply candidates to cloned `OperationConfig`s.
5. Candidate sims run through the same session/simulation/gate path, then outcome ranking/tiers select recommendations.

### Import/export

| Path | Role |
|---|---|
| `svg_input.rs`, `dxf_input.rs`, `step_input.rs`, `mesh.rs`, `enriched_mesh.rs` | Geometry import/parsing and mesh/BREP enrichment. |
| `gcode.rs` | G-code generation/post output. |
| `io.rs` | Shared IO helpers. |
| `fingerprint.rs` | Toolpath fingerprinting and sweep comparison support. |

## GUI map (`rs_cam_viz`)

| Path | Role |
|---|---|
| `crates/rs_cam_viz/src/app/` | App shell and embedded MCP handler (`app/mcp.rs`). |
| `crates/rs_cam_viz/src/controller/` | Controller-first event handling. Look under `controller/events/` for toolpath, simulation, project, UI events. |
| `crates/rs_cam_viz/src/compute/worker/` | Worker lanes and core bridging. `compute/worker/execute/mod.rs` converts GUI requests to core execution/simulation. |
| `crates/rs_cam_viz/src/state/` | GUI runtime state, project/job config, toolpath runtime entries, simulation state. |
| `crates/rs_cam_viz/src/ui/` | Egui UI panels, properties, diagnostics, timeline, operation list. |
| `crates/rs_cam_viz/src/render/` | Viewport/rendering code. |
| `crates/rs_cam_viz/src/io/` | GUI project/export/setup-sheet IO. |
| `crates/rs_cam_viz/src/mcp_server.rs` | Embedded MCP server tool descriptions/registration. |

GUI compute flow:

1. Controller event updates session/state and submits work to worker lane.
2. Worker delegates operation generation/simulation to `rs_cam_core` helpers.
3. Results return into GUI runtime state; UI panels render state and diagnostics.
4. Embedded MCP tools read/mutate the same GUI/session state.

When GUI state adds fields, audit project IO, setup sheet, worker test initializers, and MCP serialization/descriptions.

## CLI and standalone MCP

| Path | Role |
|---|---|
| `crates/rs_cam_cli/src/main.rs` | CLI entrypoint. SocratiCode graph marks it as a high-level entry point. |
| `crates/rs_cam_cli/src/job.rs` | TOML job execution flow. |
| `crates/rs_cam_cli/src/helpers.rs` | CLI helper utilities. |
| `crates/rs_cam_mcp/src/server.rs` | Standalone MCP ProjectSession-backed tool implementations. |
| `crates/rs_cam_mcp/src/main.rs` | MCP server binary entrypoint. |

Keep GUI embedded MCP and standalone MCP behavior aligned where tools overlap.

## Tests and validation map

| Area | Path / command |
|---|---|
| Core tests | Inline tests across `crates/rs_cam_core/src/**`; run `cargo test -q -p rs_cam_core <filter>`. |
| Optimizer smoke | `crates/rs_cam_core/tests/optimize_smoke.rs`. |
| Param sweeps | `crates/rs_cam_core/tests/param_sweep.rs`; `cargo test --test param_sweep`. |
| GUI worker/renderless tests | `crates/rs_cam_viz/src/compute/worker/tests.rs` and renderless harness. |
| CLI integration | `crates/rs_cam_cli/tests/integration.rs`. |
| Full local gates | `cargo fmt --check`, `cargo test -q`, `cargo clippy --workspace --all-targets -- -D warnings`. |

## SocratiCode navigation recipes for agents

Use these before raw file reading when possible:

| Question | SocratiCode tool |
|---|---|
| Where does a feature live? | `codebase_search { query: "conceptual feature words" }` after indexing completes. |
| What depends on this file? | `codebase_graph_query { filePath: "relative/path.rs" }`. |
| What symbols are in a file? | `codebase_symbols { file: "relative/path.rs" }`. |
| Who calls / what calls this function? | `codebase_symbol { name: "symbol", file: "optional/path.rs" }`. |
| What breaks if I change a symbol/file? | `codebase_impact { target: "symbol_or_path" }`. |
| What does an entrypoint do? | `codebase_flow { entrypoint: "name", file: "optional/path.rs" }`. |
| Are there cycles? | `codebase_graph_circular`. |
| Is the index ready? | `codebase_status`. |

Recommended starting points:

- Generation pipeline: search/symbol `execute_operation_annotated`, then read `compute/execute.rs` and the operation module.
- Session-level changes: inspect `session/compute.rs`, `session/mutation.rs`, and `session/save.rs` together.
- Simulation issues: inspect `compute/simulate.rs`, `dexel_stock/stamping.rs`, `dexel_stock/simulation.rs`, `simulation_cut.rs`.
- Tool-load/optimizer work: inspect `tool_load/mod.rs`, criterion files, `tool_load/optimize.rs`, and `feeds/vendor_lookup.rs`.
- GUI behavior: start from `controller/events/*`, then worker bridge, then relevant `ui/*` panel.

## Validation smoke test

SocratiCode usefulness was tested after indexing with four realistic navigation queries:

- "where does operation generation dispatch apply dressups spans semantic trace" found `compute/execute.rs::apply_dressup_traced`, GUI worker bridge code, and span planning docs. Useful for generation/dressup orientation, though old review docs can rank above current code.
- "how are per operation spindle rpm overrides passed into simulation cut samples" found the UI spindle override row, `compute/simulate.rs::SimToolpathEntry.spindle_rpm`, core fallback tests, and `effective_spindle_rpm`. This was highly useful.
- "optimizer policy search axes retarget strategy candidate patches" found current optimizer split files (`patches.rs`, strategy/retarget, `optimize/mod.rs`) plus G16 docs. Useful for ongoing optimizer work.
- "MCP optimize_toolpath tool load report GUI embedded handler" found embedded MCP server setup and optimizer project code, but less directly than expected; use exact symbol search or `rg` for MCP handler names.

Symbol tools were also tested:

- `codebase_symbols` on `tool_load/optimize/mod.rs` produced a good function inventory.
- `codebase_symbol optimize_toolpath` found local callees but no external callers; treat caller lists as incomplete.
- `codebase_impact` on `tool_load/optimize/policy.rs` returned zero impacted files even though `optimize/mod.rs` imports it; file-impact is weak for this Rust module layout.

## Known SocratiCode graph limitations observed

- File graph has low Rust dependency density for this repo: 36 edges over 245 files, with many important modules reported as orphans.
- Symbol graph is useful for listing symbols and local callees, but many call edges are unresolved. Treat symbol call results as hints, not exhaustive proof.
- Semantic search is strong for conceptual navigation and recently edited code, but planning/review docs may outrank implementation files. Add `fileFilter` or use exact symbol search/`rg` when needed.
- Use `rg` for exact strings and Rust module declarations when SocratiCode graph output is sparse.
