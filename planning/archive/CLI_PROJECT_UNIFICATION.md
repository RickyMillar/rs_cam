# CLI Project Format Unification & Analysis Surface

## Goal

Add a `project` subcommand to `rs_cam_cli` that loads the GUI project TOML format (format_version=3) and runs toolpaths with full diagnostics — simulation metrics, semantic tracing, collision detection — producing agent-parseable JSON output. This closes the gap between what the GUI computes and what the CLI can analyze.

## Why

The CLI and GUI use completely different TOML formats and execution pipelines:
- **CLI** (`job.rs`): `[tools.name]` string keys, `[[operation]]` flat params, no setups, no dressups, no heights, 6 ops only
- **GUI** (`io/project.rs`): `[[tools]]` with numeric IDs, `[[setups]]` containing `[[setups.toolpaths]]`, full operation/dressup/heights/boundary configs, 22 ops

An AI agent cannot analyze a real project through the CLI. The CLI's `adaptive3d` execution also skips semantic annotation, so even operations it CAN run produce incomplete diagnostic data.

## Architecture Decision

Do NOT rewrite the CLI's existing `job` command. Add a NEW `project` subcommand alongside it. The existing `job` command serves a different use case (simple batch processing with a minimal config).

The `project` subcommand should:
1. Parse the GUI project TOML (same serde types as `io/project.rs`)
2. Build `ComputeRequest`-equivalent data for each toolpath
3. Execute operations through core functions with full tracing
4. Run tri-dexel simulation with cut metrics
5. Run collision detection
6. Output structured JSON diagnostics

## Scope

### Must Have
- Parse GUI project format (format_version=3)
- Execute all 22 operation types (reuse core functions)
- Apply dressups (entry, arc fitting, link moves, rapid order)
- Apply height system (resolve clearance/retract/feed/top/bottom)
- Apply depth stepping
- Full semantic trace annotation (not empty)
- Full debug trace
- Cut metrics simulation (SimulationCutTrace)
- Collision detection (CollisionReport)
- JSON output: per-toolpath diagnostics artifact with all traces
- Support `--setup` flag to run a single setup
- Support `--skip` flag to skip specific toolpath IDs
- Support `--output-dir` for all artifacts

### Nice to Have
- Prior stock carry-forward between operations (TriDexelStock)
- Multi-setup coordinate transforms
- Boundary clipping
- Feed optimization dressup

### Out of Scope
- GUI rendering
- Interactive features
- Project file writing/modification

---

## Implementation Guide

### File: `crates/rs_cam_cli/src/project.rs` (new)

This module handles parsing and executing GUI project files.

#### Step 1: Project TOML Parsing

The GUI project format is defined in `crates/rs_cam_viz/src/io/project.rs`. The serde types live in `crates/rs_cam_viz/src/state/` — these are GUI-side and cannot be imported into the CLI crate (wrong dependency direction).

**Approach**: Define minimal serde-compatible structs in the CLI that mirror the project format. Only parse what's needed for execution — skip GUI-specific fields (status, stale_since, result, etc.).

Key structures to mirror:

```
ProjectFile {
    format_version: u32,
    job: ProjectJob,
    tools: Vec<ProjectTool>,        // [[tools]] with id, type, diameter, cutting_length, etc.
    models: Vec<ProjectModel>,      // [[models]] with id, path, kind, units
    setups: Vec<ProjectSetup>,      // [[setups]] with toolpaths inside
}

ProjectJob {
    name: String,
    stock: StockDef,                // x, y, z, origin_*, material, etc.
    post: PostDef,                  // format, spindle_speed, safe_z
    machine: MachineDef,            // Optional: max_feed, spindle range, chip_load
}

ProjectSetup {
    id: u32,
    name: String,
    face_up: String,                // "top" or "bottom"
    toolpaths: Vec<ProjectToolpath>,
}

ProjectToolpath {
    id: u32,
    name: String,
    type_: String,                  // "adaptive3d", "pocket", "steep_shallow", etc.
    enabled: bool,
    tool_id: u32,
    model_id: u32,
    operation: ProjectOperation,    // kind + params (nested)
    dressups: DressupDef,
    heights: HeightsDef,
    boundary: BoundaryDef,
    stock_source: String,
    feeds_auto: FeedsAutoDef,
}
```

Reference `crates/rs_cam_viz/src/io/project.rs` lines 50-400 for the exact field names and nesting. The TOML keys use snake_case and match the GUI's serde output exactly — the user's `job.toml` IS the format.

#### Step 2: Tool Construction

Use the project's explicit tool dimensions (not CLI's heuristic defaults):

```rust
fn build_tool_from_project(tool: &ProjectTool) -> ToolDefinition {
    // Mirror crates/rs_cam_viz/src/compute/worker/helpers.rs:15-44
    // Use tool.cutting_length, tool.stickout, tool.shank_diameter, etc. directly
}
```

Key difference from CLI's `build_tool`: the project format has explicit `cutting_length`, `stickout`, `holder_diameter`, `shank_diameter`, `shank_length` — use them as-is.

#### Step 3: Model Loading

```rust
fn load_model(model: &ProjectModel, project_dir: &Path) -> Result<LoadedModel> {
    let path = project_dir.join(&model.path);
    match model.kind.as_str() {
        "stl" => { /* TriangleMesh::from_stl_scaled */ },
        "dxf" => { /* load_dxf with unit scaling */ },
        "svg" => { /* load_svg */ },
        "step" => { /* load_step if available */ },
    }
}
```

Reference `crates/rs_cam_viz/src/io/import.rs:168-184` for model loading with unit scaling.

For DXF: `rs_cam_core::dxf_input::load_dxf` applies INSUNITS scaling internally. The project's `[models.units]` field provides an additional scale factor via `ModelUnits::scale_factor()`. When units is `"millimeters"`, scale = 1.0 (no additional scaling beyond INSUNITS).

#### Step 4: Height Resolution

The GUI resolves heights from the `HeightsConfig` which has modes: `auto`, `manual`, `from_reference`. For the CLI, implement a simplified resolver:

```rust
fn resolve_heights(heights: &HeightsDef, stock: &StockDef, model_bbox: &BoundingBox3) -> ResolvedHeights {
    // "auto" mode: derive from stock dimensions
    // "from_reference" mode: stock_top + offset
    // "manual" mode: absolute value
}
```

Reference `crates/rs_cam_viz/src/state/toolpath/support.rs` — search for `resolve` or `HeightsConfig` to find the resolution logic.

#### Step 5: Operation Execution with Full Tracing

This is the critical part. For each operation, create both debug AND semantic recorders, and wire them through:

```rust
fn execute_toolpath(
    toolpath_def: &ProjectToolpath,
    tool: &ToolDefinition,
    model: &LoadedModel,
    stock: &StockDef,
    project_dir: &Path,
) -> Result<ToolpathExecutionResult> {
    let tp_name = format!("tp_{}", toolpath_def.id);
    let op_label = &toolpath_def.name;
    
    // Create BOTH recorders
    let debug_recorder = ToolpathDebugRecorder::new(&tp_name, op_label);
    let semantic_recorder = ToolpathSemanticRecorder::new(&tp_name, op_label);
    let debug_root = debug_recorder.root_context();
    let semantic_root = semantic_recorder.root_context();  // <-- CLI currently skips this!
    
    // Create operation semantic scope
    let op_scope = semantic_root.begin_operation(op_label, move_count);
    
    let toolpath = match toolpath_def.operation.kind.as_str() {
        "adaptive3d" => execute_adaptive3d(toolpath_def, tool, model, &debug_root, &op_scope)?,
        "steep_shallow" => execute_steep_shallow(...)?,
        "pocket" => execute_pocket(...)?,
        // ... all 22 types
        _ => bail!("Unsupported operation: {}", toolpath_def.operation.kind),
    };
    
    // Finish and enrich traces
    let mut debug_trace = debug_recorder.finish();
    let mut semantic_trace = semantic_recorder.finish();
    enrich_traces(&mut debug_trace, &mut semantic_trace);
    
    Ok(ToolpathExecutionResult { toolpath, debug_trace, semantic_trace })
}
```

**For adaptive3d specifically**, the semantic gap fix is:

```rust
fn execute_adaptive3d(...) -> Result<Toolpath> {
    // 1. Run annotated version (returns annotations)
    let (tp, annotations) = adaptive_3d_toolpath_structured_annotated_traced_with_cancel(
        mesh, &si, &cutter, &params, &never_cancel, Some(&debug_root),
    )?;
    
    // 2. CRITICAL: Annotate semantic trace with runtime data
    // This is what the GUI does in operations_3d.rs:592-623
    // and what the CLI currently SKIPS
    annotate_adaptive3d_runtime_semantics(
        Some(&op_scope),
        &tp,
        &annotations,
        detect_flat_areas,
        region_ordering,
    );
    
    Ok(tp)
}
```

The function `annotate_adaptive3d_runtime_semantics` lives in `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs`. It's GUI-side code. You'll need to either:
- Move it to `rs_cam_core` (preferred — it only depends on core types)
- Or duplicate the logic in the CLI

Look at what it does (search for the function in `operations_3d.rs`) — it walks the `Vec<Adaptive3dRuntimeAnnotation>` and creates semantic items for each depth level, region, and pass.

#### Step 6: Simulation & Cut Metrics

After all toolpaths execute, run tri-dexel simulation with metrics:

```rust
fn run_simulation(
    toolpaths: &[ToolpathExecutionResult],
    tools: &[ToolDefinition],
    stock: &StockDef,
    spindle_rpm: u32,
    resolution: f64,
) -> Result<SimulationCutArtifact> {
    let stock_bbox = BoundingBox3 { /* from stock config */ };
    let mut stock = TriDexelStock::from_bounds(&stock_bbox, resolution);
    let mut all_samples = Vec::new();
    
    for (idx, result) in toolpaths.iter().enumerate() {
        let samples = stock.simulate_toolpath_with_metrics_with_cancel(
            &result.toolpath,
            &tools[idx],
            StockCutDirection::FromTop,
            idx,
            spindle_rpm,
            tools[idx].flute_count(),
            3000.0,  // rapid feed
            resolution,
            Some(&result.semantic_trace),  // <-- Link semantic items to cut samples!
            &|| false,
        )?;
        all_samples.extend(samples);
    }
    
    let trace = SimulationCutTrace::from_samples_with_semantics(resolution, all_samples, semantic_traces);
    Ok(SimulationCutArtifact::new(resolution, ...))
}
```

The critical detail: pass `Some(&result.semantic_trace)` to `simulate_toolpath_with_metrics_with_cancel` so that each `SimulationCutSample` gets a `semantic_item_id` linking it to the logical structure. Without this, all issues report `semantic_item_id: null` (the current CLI behavior).

#### Step 7: Collision Detection

```rust
fn run_collisions(
    toolpaths: &[ToolpathExecutionResult],
    tools: &[ToolDefinition],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
) -> Vec<CollisionReport> {
    toolpaths.iter().zip(tools).map(|(result, tool)| {
        let assembly = ToolAssembly {
            cutter_radius: tool.radius(),
            cutter_length: tool.cutting_length(),
            shank_diameter: tool.shank_diameter,
            shank_length: tool.shank_length,
            holder_diameter: tool.holder_diameter,
            holder_length: 40.0,
        };
        let report = check_collisions_interpolated(
            &result.toolpath, &assembly, mesh, index, 2.0,
        );
        let rapid_report = check_rapid_collisions(
            &result.toolpath, &assembly, &stock_bbox,
        );
        // Merge both reports
        report
    }).collect()
}
```

#### Step 8: JSON Output

Produce one JSON file per toolpath + one summary:

```
output_dir/
  summary.json          # Project-level: op count, total time, collision status, go/no-go
  tp_2_pin_drill.json   # Per-toolpath: ToolpathTraceArtifact + cut metrics + collisions
  tp_6_drill.json
  tp_3_adaptive3d.json
  tp_1_project_curve.json
  simulation.json       # Full SimulationCutArtifact with all samples
```

Each per-toolpath JSON should contain:
```json
{
  "toolpath_id": 3,
  "toolpath_name": "3D Adaptive Rough 4",
  "operation_type": "adaptive3d",
  "tool": "6mm End Mill",
  "debug_trace": { /* ToolpathDebugTrace */ },
  "semantic_trace": { /* ToolpathSemanticTrace with items */ },
  "cut_summary": { /* SimulationToolpathCutSummary */ },
  "collision_report": { /* CollisionReport */ },
  "issues": [ /* filtered issues for this toolpath */ ]
}
```

The summary JSON should contain:
```json
{
  "project": "Untitled",
  "setup_count": 2,
  "toolpath_count": 6,
  "total_runtime_s": 1234.5,
  "total_cutting_distance_mm": 56789.0,
  "collision_count": 0,
  "rapid_collision_count": 0,
  "air_cut_percentage": 12.3,
  "average_engagement": 0.35,
  "issue_summary": {
    "air_cuts": 234,
    "low_engagement": 89,
    "rapid_collisions": 0,
    "holder_collisions": 0
  },
  "per_toolpath": [
    { "id": 3, "name": "...", "status": "ok|warning|error", "issues": 5 }
  ],
  "verdict": "WARNING: 47.5% air cutting on adaptive3d"
}
```

### File: `crates/rs_cam_cli/src/main.rs` changes

Add the new subcommand:

```rust
/// Analyze a GUI project file with full diagnostics
Project {
    /// Path to the project .toml file (GUI format, format_version=3)
    input: PathBuf,

    /// Output directory for diagnostic artifacts
    #[arg(long, default_value = "diagnostics")]
    output_dir: PathBuf,

    /// Run only this setup (by name or ID)
    #[arg(long)]
    setup: Option<String>,

    /// Skip these toolpath IDs (comma-separated)
    #[arg(long)]
    skip: Option<String>,

    /// Simulation resolution in mm
    #[arg(long, default_value = "0.5")]
    resolution: f64,

    /// Print human-readable summary to stderr
    #[arg(long)]
    summary: bool,
}
```

---

## Key Files to Read

| File | What to learn |
|------|--------------|
| `crates/rs_cam_cli/src/job.rs` | Current CLI execution — understand what exists |
| `crates/rs_cam_cli/src/main.rs` | CLI structure, collision check, diagnostics report |
| `crates/rs_cam_viz/src/io/project.rs` | Project TOML serde format (the source of truth) |
| `crates/rs_cam_viz/src/compute/worker.rs:76-100` | ComputeRequest structure |
| `crates/rs_cam_viz/src/compute/worker/execute/operations_3d.rs` | How GUI builds params + does semantic annotation |
| `crates/rs_cam_viz/src/compute/worker/execute/mod.rs` | Operation dispatch and context threading |
| `crates/rs_cam_viz/src/compute/worker/helpers.rs` | Tool building from ToolConfig |
| `crates/rs_cam_viz/src/state/toolpath/support.rs` | DressupConfig, height resolution |
| `crates/rs_cam_viz/src/state/toolpath/entry.rs` | ToolpathEntry fields |
| `crates/rs_cam_viz/src/state/job.rs` | JobState, StockConfig, ToolConfig, Setup |
| `crates/rs_cam_core/src/simulation_cut.rs` | SimulationCutSample, trace, artifacts |
| `crates/rs_cam_core/src/collision.rs` | ToolAssembly, CollisionReport |
| `crates/rs_cam_core/src/semantic_trace.rs` | ToolpathSemanticRecorder, context |
| `crates/rs_cam_core/src/debug_trace.rs` | ToolpathDebugRecorder, context |
| `crates/rs_cam_core/src/adaptive3d.rs` | annotated_traced_with_cancel signature |

## Verification

1. `cargo clippy --workspace --all-targets -- -D warnings` (zero warnings)
2. `cargo test -q` (all pass)
3. Run against the test project:
   ```bash
   cargo run -p rs_cam_cli -- project /home/ricky/Downloads/wanaka100/lakes-no-riv/rivmap_export/job.toml \
     --output-dir /tmp/wanaka_diagnostics \
     --skip 7 \
     --summary
   ```
   (skip ID 7 = steep/shallow, too slow for testing)
4. Verify `summary.json` has per-toolpath verdicts
5. Verify adaptive3d trace has non-empty `semantic_trace.items`
6. Verify `simulation.json` has `semantic_item_id` populated on samples

## Lint Policy

This repo has strict clippy. See `CLAUDE.md` for the full lint table. Key ones:
- No `.unwrap()` or `.expect()` — use `?` or `#[allow]` with SAFETY comment
- No `arr[i]` — use iterators, `.get()`, or `#[allow]` with SAFETY comment  
- No `println!` — use `tracing` (info!, debug!, warn!) or `eprintln!` (CLI has `#[allow(clippy::print_stderr)]`)
- Run `/lint-fix` skill if stuck on a lint
