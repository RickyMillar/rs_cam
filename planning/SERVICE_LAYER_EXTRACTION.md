# Unified Service Layer + MCP Server: Implementation Plan

## Vision

One compute engine, three interfaces. A `ProjectSession` in `rs_cam_core` owns all machining logic. The GUI renders it, the CLI batches it, and an MCP server lets AI agents interact with a running session in real-time.

```
                           ┌─────────────────────┐
                           │    rs_cam_core       │
                           │                      │
                           │  compute/            │
                           │    config.rs          │  ← Config types (OperationConfig, etc.)
                           │    execute.rs         │  ← execute_toolpath()
                           │    simulate.rs        │  ← run_simulation()
                           │    transform.rs       │  ← FaceUp, ZRotation, setup transforms
                           │                      │
                           │  session.rs           │  ← ProjectSession API
                           └──────────┬───────────┘
                                      │
                    ┌─────────────────┼─────────────────┐
                    │                 │                  │
              ┌─────┴─────┐   ┌──────┴──────┐   ┌──────┴──────┐
              │  rs_cam_  │   │  rs_cam_    │   │  rs_cam_    │
              │  viz      │   │  cli        │   │  viz/mcp    │
              │           │   │             │   │             │
              │  GUI      │   │  Batch CLI  │   │  MCP Server │
              │  (egui)   │   │  (clap)     │   │  (rmcp)     │
              └───────────┘   └─────────────┘   └─────────────┘
```

---

## Current State Analysis

### What Exists Today

| Component | Location | Lines | Purpose |
|-----------|----------|-------|---------|
| GUI compute worker | `rs_cam_viz/src/compute/worker/` | ~4500 | Operation execution, simulation, collision |
| GUI state types | `rs_cam_viz/src/state/toolpath/` | ~1700 | OperationConfig, DressupConfig, HeightsConfig |
| GUI state types | `rs_cam_viz/src/state/job.rs` | ~1200 | JobState, ToolConfig, Setup, FaceUp, etc. |
| GUI controller | `rs_cam_viz/src/controller/` | ~1200 | Event routing, compute submit, undo |
| CLI job executor | `rs_cam_cli/src/job.rs` | ~846 | Duplicate operation execution (6 ops only) |
| CLI project executor | `rs_cam_cli/src/project.rs` | ~2754 | Duplicate execution (22 ops), duplicate configs |

### Why They're Out of Sync

The CLI was built first as a minimal batch tool with its own TOML format. The GUI then built a parallel pipeline in `rs_cam_viz` with richer state. Nobody refactored them to converge. Result: every fix goes in one place but not the other. The CLI `project.rs` duplicates ~1400 lines of operation execution and ~1000 lines of config types that already exist in the GUI.

### Key Finding: Zero GUI Dependencies in Compute

The entire `compute/worker/` module has **zero imports from egui, wgpu, or any rendering code**. All imports are either `rs_cam_core::*` (pure computation) or `crate::state::*` (pure data types). This means the extraction boundary is clean.

---

## Phase Plan

### Phase 1: Move Config Types to Core (~1 session)

**Goal:** All operation/toolpath configuration types live in `rs_cam_core`. Both GUI and CLI import from core.

**Create:** `crates/rs_cam_core/src/compute/config.rs` (~1600 lines)

**Move from `rs_cam_viz/src/state/toolpath/`:**

| Type | Source | Lines |
|------|--------|-------|
| `OperationType` enum (23 variants) + `OperationSpec` | `catalog.rs` | ~180 |
| `OperationConfig` enum (23 variants) + all methods | `catalog.rs` | ~200 |
| `OperationParams` trait | `catalog.rs` | ~20 |
| All 23 config structs (FaceConfig → AlignmentPinDrillConfig) | `catalog.rs` | ~600 |
| `GeometryRequirement`, `OperationFamily`, `UiProcessRole`, etc. | `catalog.rs` | ~80 |
| `DressupConfig` + `DressupEntryStyle` + `RetractStrategy` | `support.rs` | ~120 |
| `HeightsConfig` + `HeightMode` + `ResolvedHeights` + `HeightContext` | `support.rs` | ~130 |
| `BoundaryConfig` + `BoundarySource` + `BoundaryContainment` | `support.rs` | ~70 |
| `StockSource`, `ToolpathId`, `ToolpathStats`, `ComputeStatus` | `support.rs` | ~30 |
| `FeedsAutoMode` | `support.rs` | ~20 |
| `ToolpathResult` | `entry.rs` | ~10 |

**Move from `rs_cam_viz/src/state/job.rs`:**

| Type | Lines |
|------|-------|
| `ToolConfig` + `ToolType` + `ToolMaterial` + `BitCutDirection` | ~80 |
| `StockConfig` + `Material` + `WorkholdingRigidity` | ~60 |
| `PostConfig` + `PostFormat` | ~30 |
| `FaceUp` enum + transform methods | ~120 |
| `ZRotation` enum + transform methods | ~90 |
| `Setup` struct (data portion only) | ~40 |
| `ModelId`, `ToolId`, `SetupId` newtypes | ~10 |

**In `rs_cam_viz`:** Replace moved types with `pub use rs_cam_core::compute::config::*`. All existing GUI code continues to work via re-exports.

**In `rs_cam_cli`:** Delete the ~1000 lines of duplicate config structs in `project.rs`. Import from `rs_cam_core::compute::config`.

**Verification:**
- `cargo test -q` — all pass
- `cargo clippy --workspace` — zero warnings
- GUI smoke test — open app, load project, verify UI works
- Serde round-trip: save project → reload → compare (field names MUST match exactly)

**Risk:** Serde attribute alignment. The GUI's config structs use `#[serde(tag="kind", content="params")]` etc. These must be preserved exactly or project files break.

---

### Phase 2: Move Execution Logic to Core (~2 sessions)

**Goal:** One `execute_toolpath()` function in core that both GUI and CLI call.

**Create:** `crates/rs_cam_core/src/compute/execute.rs` (~2000 lines)

**Move from `rs_cam_viz/src/compute/worker/`:**

| Function/Module | Source | Lines | Notes |
|----------------|--------|-------|-------|
| `build_cutter()` | `helpers.rs` | ~30 | ToolConfig → ToolDefinition |
| `apply_dressups()` | `helpers.rs` | ~200 | 7-step dressup pipeline |
| `compute_stats()` | `helpers.rs` | ~10 | Move count/distance |
| All 2D op runners (10) | `operations_2d.rs` | ~1100 | pocket, profile, adaptive, etc. |
| All 3D op runners (12) | `operations_3d.rs` | ~950 | dropcutter, adaptive3d, waterline, etc. |
| `SemanticToolpathOp` trait + impls | `operations_2d.rs` + `operations_3d.rs` | ~500 | Semantic annotation for each op |
| All semantic annotation funcs (11) | `execute/mod.rs` | ~400 | `annotate_adaptive3d_runtime_semantics()` etc. |
| `CutRun` + semantic helpers | `semantic.rs` | ~150 | cutting_runs(), contour_toolpath() |

**Create the unified entry point:**

```rust
// rs_cam_core/src/compute/execute.rs

pub struct ComputeRequest {
    pub toolpath_name: String,
    pub operation: OperationConfig,
    pub dressups: DressupConfig,
    pub tool: ToolConfig,
    pub heights: ResolvedHeights,
    pub cutting_levels: Vec<f64>,
    pub polygons: Option<Arc<Vec<Polygon2>>>,
    pub mesh: Option<Arc<TriangleMesh>>,
    pub surface_mesh: Option<Arc<TriangleMesh>>,   // For ProjectCurve
    pub stock_bbox: Option<BoundingBox3>,
    pub prior_stock: Option<TriDexelStock>,         // For feed optimization
    pub boundary: Option<BoundaryConfig>,
    pub keep_out_footprints: Vec<Polygon2>,
    pub prev_tool_radius: Option<f64>,              // For rest machining
}

pub struct ComputeResult {
    pub toolpath: Toolpath,
    pub stats: ToolpathStats,
    pub debug_trace: Option<ToolpathDebugTrace>,
    pub semantic_trace: Option<ToolpathSemanticTrace>,
}

pub fn execute_toolpath(
    req: &ComputeRequest,
    cancel: &AtomicBool,
    debug_ctx: Option<&ToolpathDebugContext>,
    semantic_ctx: Option<&ToolpathSemanticContext>,
) -> Result<ComputeResult, ComputeError>
```

**What stays in GUI:** `ThreadedComputeBackend` (threading), `ToolpathPhaseTracker` (UI progress), file I/O (artifact dirs).

**GUI becomes:**
```rust
fn run_compute(&self, req: ComputeRequest) -> ComputeResult {
    let phase = self.phase_tracker.start("Generating");
    let result = rs_cam_core::compute::execute_toolpath(&req, &cancel, debug, semantic);
    phase.finish();
    result
}
```

**CLI becomes:**
```rust
let result = rs_cam_core::compute::execute_toolpath(&req, &never_cancel, debug, semantic);
```

**Verification:**
- All 915+ tests pass
- GUI: generate toolpath → verify same moves, distances
- CLI: `cargo run -p rs_cam_cli -- project job.toml --summary` → same results as before

---

### Phase 3: Move Simulation & Collision to Core (~1 session)

**Goal:** Simulation and collision checking through core functions.

**Create:** `crates/rs_cam_core/src/compute/simulate.rs`

**Move:**

| Function | Source | Lines |
|----------|--------|-------|
| Simulation orchestration | `execute/mod.rs:120-333` | ~200 |
| Setup group simulation | `execute/mod.rs` | ~100 |
| Collision checking wrapper | `helpers.rs` | ~50 |
| Rapid collision checking | `helpers.rs` | ~30 |

**Create:** `crates/rs_cam_core/src/compute/transform.rs`

| Type | Source | Lines |
|------|--------|-------|
| `SetupTransformInfo` | `worker.rs:139-241` | ~100 |
| `SetupSimGroup`, `SetupSimToolpath` | `worker.rs` | ~40 |
| Transform methods (local_to_global, etc.) | `worker.rs` | ~60 |

**Entry point:**
```rust
pub fn run_simulation(
    groups: &[SetupSimGroup],
    stock_bbox: &BoundingBox3,
    resolution: f64,
    metric_options: SimulationMetricOptions,
    cancel: &AtomicBool,
) -> Result<SimulationResult, ComputeError>

pub fn run_collision_check(
    toolpath: &Toolpath,
    tool: &ToolDefinition,
    mesh: &TriangleMesh,
    stock_bbox: &BoundingBox3,
    cancel: &AtomicBool,
) -> CollisionResult
```

---

### Phase 4: Create ProjectSession (~1 session)

**Goal:** A single API that owns project state + compute, usable by GUI, CLI, and MCP.

**Create:** `crates/rs_cam_core/src/session.rs` (~500 lines)

```rust
pub struct ProjectSession {
    // Project data
    project: ProjectData,       // name, stock, post, machine
    models: Vec<LoadedModel>,   // geometry with Arc<TriangleMesh> / Arc<Vec<Polygon2>>
    tools: Vec<ToolConfig>,
    setups: Vec<SetupData>,     // setup orientation + toolpath configs
    
    // Computed state
    results: HashMap<usize, ComputeResult>,
    simulation: Option<SimulationResult>,
    diagnostics: Option<ProjectDiagnostics>,
}

impl ProjectSession {
    // Lifecycle
    pub fn load(path: &Path) -> Result<Self>;
    pub fn from_project_file(project: ProjectFile, base_dir: &Path) -> Result<Self>;
    
    // Queries
    pub fn list_toolpaths(&self) -> Vec<ToolpathSummary>;
    pub fn list_tools(&self) -> Vec<ToolSummary>;
    pub fn get_toolpath_params(&self, id: usize) -> Option<&OperationConfig>;
    pub fn get_tool(&self, id: usize) -> Option<&ToolConfig>;
    pub fn stock_config(&self) -> &StockConfig;
    pub fn stock_bbox(&self) -> BoundingBox3;
    
    // Compute
    pub fn generate_toolpath(&mut self, id: usize, cancel: &AtomicBool) -> Result<&ComputeResult>;
    pub fn generate_all(&mut self, skip: &[usize], cancel: &AtomicBool) -> Result<()>;
    
    // Analysis
    pub fn run_simulation(&mut self, opts: SimulationOptions, cancel: &AtomicBool) -> Result<&SimulationResult>;
    pub fn collision_check(&self, id: usize) -> Option<CollisionResult>;
    pub fn diagnostics(&self) -> ProjectDiagnostics;
    
    // Mutation
    pub fn set_parameter(&mut self, id: usize, param: &str, value: serde_json::Value) -> Result<()>;
    pub fn set_tool_parameter(&mut self, id: usize, param: &str, value: serde_json::Value) -> Result<()>;
    
    // Export
    pub fn export_gcode(&self, path: &Path, setup_id: Option<usize>) -> Result<()>;
    pub fn export_diagnostics_json(&self, output_dir: &Path) -> Result<()>;
}

pub struct ProjectDiagnostics {
    pub total_runtime_s: f64,
    pub air_cut_percentage: f64,
    pub average_engagement: f64,
    pub collision_count: usize,
    pub rapid_collision_count: usize,
    pub per_toolpath: Vec<ToolpathDiagnostic>,
    pub verdict: String,
}
```

**How it builds ComputeRequests internally:**

```rust
impl ProjectSession {
    fn build_compute_request(&self, tp_id: usize) -> Result<ComputeRequest> {
        let tp = self.find_toolpath(tp_id)?;
        let tool = self.find_tool(tp.tool_id)?;
        let setup = self.find_setup_for_toolpath(tp_id)?;
        let model = self.find_model(tp.model_id)?;
        
        // Apply setup orientation transform to mesh
        let mesh = if setup.face_up == FaceUp::Bottom {
            model.mesh.map(|m| Arc::new(transform_mesh_for_setup(m, setup, &self.stock)))
        } else {
            model.mesh.clone()
        };
        
        // Resolve heights in setup-local frame
        let heights = tp.heights.resolve(&self.height_context(tp, setup));
        
        Ok(ComputeRequest {
            operation: tp.operation.clone(),
            dressups: tp.dressups.clone(),
            tool: tool.clone(),
            heights,
            mesh,
            polygons: model.polygons.clone(),
            stock_bbox: Some(self.effective_stock_bbox(setup)),
            // ...
        })
    }
}
```

---

### Phase 5: Rewire CLI (~1 session)

**Goal:** CLI uses `ProjectSession` instead of duplicate execution code.

**`project.rs` becomes (~300 lines):**
```rust
pub fn run_project_command(input: &Path, output_dir: &Path, ...) -> Result<()> {
    // 1. Load session
    let mut session = ProjectSession::load(input)?;
    
    // 2. Generate toolpaths
    session.generate_all(&skip_ids, &AtomicBool::new(false))?;
    
    // 3. Run simulation
    let sim_opts = SimulationOptions { resolution, skip: skip_ids.clone() };
    session.run_simulation(sim_opts, &AtomicBool::new(false))?;
    
    // 4. Write diagnostics
    session.export_diagnostics_json(output_dir)?;
    
    // 5. Print summary
    let diag = session.diagnostics();
    eprintln!("Air cutting: {:.1}%", diag.air_cut_percentage);
    // ...
}
```

**Delete:** ~1400 lines of operation execution, ~200 lines of dressup application, ~150 lines of simulation code, ~1000 lines of duplicate config structs from `project.rs`.

**`job.rs` similarly simplified** — parse legacy TOML → build ProjectSession → call same functions.

---

### Phase 6: Rewire GUI (~2 sessions, can parallel with Phase 5)

**Goal:** GUI's compute worker calls core functions instead of owning them.

**`compute/worker/execute/mod.rs` becomes:**
```rust
pub fn run_compute_with_phase(req: ComputeRequest, cancel: &AtomicBool, ...) -> ComputeResult {
    // Phase tracking wrapper around core function
    phase.set("Generating");
    let result = rs_cam_core::compute::execute_toolpath(&req, cancel, debug, semantic);
    phase.set("Complete");
    result
}
```

**What stays in `rs_cam_viz/src/compute/`:**
- `ThreadedComputeBackend` — worker thread management, mpsc channels
- `ToolpathPhaseTracker` — UI progress updates
- Request building from GUI state (`submit_toolpath_compute()` in controller)
- Artifact file I/O
- The worker loop with panic recovery

**What's deleted from viz:** All operation execution code (~2000 lines), all operation-specific semantic annotation functions (~500 lines), all dressup application code (~200 lines). Replaced with one call to `rs_cam_core::compute::execute_toolpath()`.

---

### Phase 7: Add MCP Server (~2 sessions)

**Goal:** AI agents interact with a running GUI session through MCP tools.

**Dependencies:** `rmcp` v1.3.0 (official Rust MCP SDK), `axum`, `tokio`

**Add to `crates/rs_cam_viz/Cargo.toml`:**
```toml
rmcp = { version = "1.3.0", features = ["server", "transport-streamable-http-server"] }
axum = "0.8"
tokio = { version = "1", features = ["full"] }
tokio-util = { version = "0.7", features = ["rt"] }
schemars = "0.8"
```

**Create:** `crates/rs_cam_viz/src/mcp_server.rs`

The MCP server wraps a `ProjectSession` (shared with the GUI via `Arc<Mutex<>>`):

```rust
#[derive(Clone)]
pub struct CamMcpServer {
    session: Arc<Mutex<ProjectSession>>,
    tool_router: ToolRouter<CamMcpServer>,
}

#[tool_router]
impl CamMcpServer {
    // ── Project Queries ──
    
    #[tool(description = "Get project summary: setups, toolpaths, tools, stock dimensions")]
    async fn project_summary(&self) -> Result<CallToolResult, McpError>;

    #[tool(description = "List all toolpaths with status, tool info, and move counts")]
    async fn list_toolpaths(&self) -> Result<CallToolResult, McpError>;

    #[tool(description = "List all tools with geometry and dimensions")]
    async fn list_tools(&self) -> Result<CallToolResult, McpError>;

    #[tool(description = "Get all parameters for a specific toolpath")]
    async fn get_toolpath_params(&self, #[tool(param)] id: usize) -> Result<CallToolResult, McpError>;

    // ── Compute ──
    
    #[tool(description = "Generate toolpath for a specific operation. Returns move count and distances.")]
    async fn generate_toolpath(&self, #[tool(param)] id: usize) -> Result<CallToolResult, McpError>;

    #[tool(description = "Generate all toolpaths, optionally skipping specific IDs")]
    async fn generate_all(&self, #[tool(param)] skip: Vec<usize>) -> Result<CallToolResult, McpError>;

    // ── Analysis ──
    
    #[tool(description = "Run tri-dexel stock simulation and return diagnostics: air cutting %, engagement, collisions, per-toolpath issues")]
    async fn run_simulation(&self, #[tool(param)] resolution: Option<f64>) -> Result<CallToolResult, McpError>;

    #[tool(description = "Get collision report for a toolpath: holder/shank events, rapid-through-stock, min safe stickout")]
    async fn get_collision_report(&self, #[tool(param)] id: usize) -> Result<CallToolResult, McpError>;

    #[tool(description = "Get cut trace diagnostics: air cut time, engagement distribution, chipload, issues")]
    async fn get_diagnostics(&self) -> Result<CallToolResult, McpError>;

    // ── Mutation ──
    
    #[tool(description = "Set a toolpath parameter (e.g. stepover, depth_per_pass, feed_rate). Marks toolpath as stale.")]
    async fn set_toolpath_param(
        &self, #[tool(param)] id: usize,
        #[tool(param)] param: String,
        #[tool(param)] value: f64,
    ) -> Result<CallToolResult, McpError>;

    #[tool(description = "Set a tool parameter (e.g. diameter, stickout, flute_count)")]
    async fn set_tool_param(
        &self, #[tool(param)] id: usize,
        #[tool(param)] param: String,
        #[tool(param)] value: f64,
    ) -> Result<CallToolResult, McpError>;

    // ── Export ──
    
    #[tool(description = "Export G-code to a file path")]
    async fn export_gcode(&self, #[tool(param)] path: String) -> Result<CallToolResult, McpError>;
}
```

**MCP server startup** (in GUI app initialization):

```rust
// In app.rs or main.rs, after creating the GUI app:
fn start_mcp_server(session: Arc<Mutex<ProjectSession>>) {
    let server = CamMcpServer::new(session);
    tokio::spawn(async move {
        let service = StreamableHttpService::new(
            move || Ok(server.clone()),
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig::default(),
        );
        let router = axum::Router::new().nest_service("/mcp", service);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:8808").await.unwrap();
        axum::serve(listener, router).await.unwrap();
    });
}
```

**State sync:** When the MCP server mutates state (set_parameter, generate), it needs to notify the GUI to re-render. Use a shared `Arc<AtomicBool>` flag or channel that the GUI's render loop polls.

---

### Phase 8: Claude Code Integration (~0.5 session)

**Create** `.mcp.json` in repo root:
```json
{
  "mcpServers": {
    "rs-cam": {
      "type": "http",
      "url": "http://127.0.0.1:8808/mcp"
    }
  }
}
```

**Update** `.claude/agents/sim-diagnostics.md` to reference MCP tools instead of file reads.

**Update** `AI_MACHINIST_ANALYSIS_REFERENCE.md` to document the MCP tools.

**Agent workflow becomes:**
```
Agent: [calls tool: project_summary]
→ "Stock: 120x120x8mm softwood. 2 setups, 6 toolpaths. Setup 1: bottom-up."

Agent: [calls tool: generate_all { skip: [7] }]
→ "Generated 5 toolpaths. Pin Drill: 24 moves. Adaptive: 8707 moves. ..."

Agent: [calls tool: run_simulation { resolution: 0.5 }]
→ "Air cutting: 12.3%. Avg engagement: 0.35. 0 collisions. Verdict: OK"

Agent: [calls tool: set_toolpath_param { id: 3, param: "stepover", value: 1.5 }]
→ "Toolpath 3 marked stale. Regenerate to apply."

Agent: [calls tool: generate_toolpath { id: 3 }]
→ "Regenerated. 12,400 moves (was 8707). Cutting: 28,500mm."

Agent: [calls tool: run_simulation]
→ "Air cutting: 8.2%. Engagement: 0.42. Verdict: OK"
// User sees the GUI update live with the new toolpath
```

---

## Phase Dependencies

```
Phase 1 (config types)
    │
    ├── Phase 2 (execution logic) ──┐
    │                                ├── Phase 4 (ProjectSession)
    └── Phase 3 (simulation) ───────┘        │
                                             ├── Phase 5 (CLI rewire)
                                             ├── Phase 6 (GUI rewire) 
                                             │        │
                                             │        └── Phase 7 (MCP server)
                                             │                  │
                                             │                  └── Phase 8 (Claude Code)
                                             │
                                             └── Phase 5 can start immediately
```

Phases 5+6 can run in parallel. Phase 7 depends on Phase 6 (MCP lives in viz).

## Effort Estimate

| Phase | Sessions | Lines Changed | Risk |
|-------|----------|--------------|------|
| 1. Config types | 1 | ~1600 move + re-export | Medium (serde compat) |
| 2. Execution logic | 2 | ~2500 move + refactor | High (biggest phase) |
| 3. Simulation | 1 | ~400 move | Low |
| 4. ProjectSession | 1 | ~500 new | Medium (API design) |
| 5. CLI rewire | 1 | ~1400 delete, ~300 new | Low |
| 6. GUI rewire | 2 | ~2500 delete, ~200 new | Medium (threading) |
| 7. MCP server | 2 | ~500 new + deps | Medium (new deps) |
| 8. Claude Code | 0.5 | ~50 config | Low |
| **Total** | **~10.5** | | |

## Key Files to Read Before Starting

| File | Lines | What to Learn |
|------|-------|--------------|
| `rs_cam_viz/src/state/toolpath/catalog.rs` | 801 | OperationConfig enum, all 23 configs |
| `rs_cam_viz/src/state/toolpath/support.rs` | 571 | DressupConfig, HeightsConfig, BoundaryConfig |
| `rs_cam_viz/src/state/job.rs` | 1200 | ToolConfig, StockConfig, Setup, FaceUp, ZRotation |
| `rs_cam_viz/src/compute/worker.rs` | ~400 | ComputeRequest, SetupTransformInfo |
| `rs_cam_viz/src/compute/worker/execute/mod.rs` | 1856 | Main dispatch, simulation, semantic annotation |
| `rs_cam_viz/src/compute/worker/execute/operations_2d.rs` | 1172 | 2D operation execution pattern |
| `rs_cam_viz/src/compute/worker/execute/operations_3d.rs` | 947 | 3D operation execution pattern |
| `rs_cam_viz/src/compute/worker/helpers.rs` | 553 | build_cutter, apply_dressups |
| `rs_cam_viz/src/controller/events/compute.rs` | 560 | How GUI builds ComputeRequests |
| `rs_cam_cli/src/project.rs` | 2754 | Current CLI duplicate (delete target) |
| `rs_cam_core/src/session.rs` | N/A | Will be created |

## Verification Protocol

After each phase:
1. `cargo fmt --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test -q` (915+ tests)
4. GUI smoke: load project → generate toolpath → run simulation → export G-code
5. CLI smoke: `cargo run -p rs_cam_cli -- project job.toml --output-dir /tmp/test --summary --skip 7`
6. Serde round-trip: load project → save → reload → verify identical

## Lint Policy

See `CLAUDE.md` for the full table. Critical rules:
- No `.unwrap()` / `.expect()` — use `?` or `#[allow]` with SAFETY comment
- No `arr[i]` — use iterators, `.get()`, or `#[allow]` with SAFETY comment
- No `println!` — use `tracing` macros
- Run `/verify` before committing each phase
