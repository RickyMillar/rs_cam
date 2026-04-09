# Phase 4f: Remove JobState — Handoff Document

## Status

Phases 1-6 (minus 4f) of the unified state architecture are complete.
The GUI now loads projects through `ProjectSession` and syncs state
before compute/simulation. The MCP server has full CRUD. Both paths
converge at `execute_operation_annotated()`.

**What remains:** Replace `JobState` with `ProjectSession` in the GUI,
eliminating the dual-state model and the `sync_session_from_job()` bridge.

## Current Architecture (post Phase 4e)

```
AppController
  ├── state: AppState
  │     ├── job: JobState          ← TO BE REMOVED
  │     ├── selection: Selection   ← stays (GUI-only)
  │     ├── viewport: ViewportState ← stays (GUI-only)
  │     ├── simulation: SimulationState ← stays (GUI-only)
  │     └── history: UndoHistory   ← stays (GUI-only)
  ├── session: Option<ProjectSession>  ← becomes the source of truth
  └── compute: ComputeBackend      ← stays
```

## Target Architecture

```
AppController
  ├── state: AppState
  │     ├── session: ProjectSession     ← single source of truth
  │     ├── ui_toolpaths: HashMap<usize, ToolpathUiState>
  │     ├── selection: Selection
  │     ├── viewport: ViewportState
  │     ├── simulation: SimulationState
  │     ├── history: UndoHistory
  │     ├── dirty: bool
  │     ├── edit_counter: u64
  │     └── file_path: Option<PathBuf>
  └── compute: ComputeBackend
```

Where `ToolpathUiState` holds:
- `visible: bool`
- `locked: bool`
- `auto_regen: bool`
- `status: ComputeStatus`
- `stale_since: Option<Instant>`
- `result: Option<ToolpathResult>`
- `feeds_result: Option<FeedsResult>`

## Scope

- **32 files** to change
- **~360 references** to `state.job` / `JobState` / `ToolpathEntry`
- **~2500-3500 lines** of changes
- **No new logic** — purely mechanical replacement of data access paths

## New Accessors ProjectSession Needs

Currently `ProjectSession` fields are `pub(crate)`. The GUI needs:

### Mutable accessors (for property panel editing):
- `stock_mut() -> &mut StockConfig`
- `post_mut() -> &mut ProjectPostConfig`
- `machine_mut() -> &mut MachineProfile`
- `tools_mut() -> &mut Vec<ToolConfig>`
- `setups_mut() -> &mut Vec<SetupData>`
- `setup_mut(index) -> Option<&mut SetupData>`
- `find_toolpath_config_mut(index) -> Option<&mut ToolpathConfig>`

### Lifecycle methods (currently on JobState):
- `set_name(String)`
- `mark_edited()` — increment edit counter, set dirty
- `sync_next_ids()` — update ID counters after bulk changes

### Iteration:
- `all_toolpath_configs() -> impl Iterator<Item = &ToolpathConfig>`
- `all_toolpath_configs_mut() -> impl Iterator<Item = &mut ToolpathConfig>`
- `setup_of_toolpath(index) -> Option<usize>`

## Migration Batches (recommended order)

### Batch 1: Serialization (LOW RISK) — 3 files
- `io/project.rs` — already decoupled, rename types
- `io/export.rs` — operates on ToolpathEntry only
- `io/setup_sheet.rs` — minimal usage

### Batch 2: State Definition — 2 files
- `state/job.rs` → delete, replaced by ProjectSession
- `state/mod.rs` → replace `job: JobState` with `session: ProjectSession`

### Batch 3: Test Fixtures — 3 files
- `controller/tests.rs` — 52 refs, update `sample_controller()` helper
- `controller/workflow_tests.rs` — 17 refs
- `ui/properties/operations/mod.rs` — 3 refs

### Batch 4: Event Handlers (HIGH RISK) — 4 files
- `controller/events/model.rs` — 49 refs (setup/fixture/tool mutations)
- `controller/events/toolpath.rs` — 24 refs (toolpath CRUD)
- `controller/events/compute.rs` — 16 refs (compute state mutations)
- `controller/events/mod.rs` — 7 refs (event dispatch)

### Batch 5: UI Binding (HIGHEST RISK) — 6 files
- `ui/properties/mod.rs` — 47 refs (2941-line file, tight binding)
- `ui/setup_panel.rs` — 14 refs
- `ui/toolpath_panel.rs` — 10 refs
- `ui/project_tree.rs` — 12 refs
- `ui/properties/pocket.rs` — 3 refs
- `ui/properties/stock.rs` — 2 refs

### Batch 6: I/O and Controllers — 3 files
- `controller/io.rs` — 27 refs (load/save)
- `controller.rs` — 10 refs (init, sync_session removed)
- `app.rs` + `app/viewport.rs` — 16 refs total

### Batch 7: Remaining Readers — 11 files
- `ui/menu_bar.rs`, `ui/status_bar.rs`, `ui/workspace_bar.rs`,
  `ui/preflight.rs`, `ui/sim_op_list.rs`, `ui/sim_timeline.rs`,
  `ui/sim_diagnostics.rs`, `interaction/picking.rs`, etc.

## Verification Per Batch

```bash
# After each batch:
cargo clippy --workspace --all-targets -- -D warnings
cargo test -q  # 144 pass, 2 pre-existing fail

# After Batch 4 (event handlers):
# Manual: load project, add/remove toolpath, generate, verify

# After Batch 5 (UI binding):
# Manual: edit stock, edit tool, edit operation params, undo/redo

# After Batch 6 (I/O):
# Manual: save, reload, verify round-trip

# Final:
cargo run -p rs_cam_viz --bin rs_cam_gui --release
# Full workflow: load → edit → generate → simulate → export
```

## Key Gotchas

1. **Nested mutations**: `job.setups[].fixtures[].origin_x = value` is
   scattered throughout. ProjectSession needs mutable access to nested
   setup fields.

2. **mark_edited()**: 20+ call sites. Either expose on ProjectSession
   or auto-trigger on mutation.

3. **Undo/Redo**: UndoHistory stores clones of StockConfig, ToolConfig,
   OperationConfig, DressupConfig. These types are already from core,
   so undo still works. But the snapshot/restore paths need to write
   through to ProjectSession.

4. **ToolpathEntry split**: Currently a single struct with both config
   and runtime state. The migration separates them:
   - Persisted → `session.toolpath_configs[i]` (ToolpathConfig)
   - Runtime → `ui_toolpaths[i]` (ToolpathUiState)
   - UI code that reads both needs two lookups

5. **Test helpers**: `sample_controller()` creates a whole JobState.
   Replace with `ProjectSession::from_project_file()` using a test
   ProjectFile, then construct ToolpathUiState for each toolpath.

## Files Reference (with ref counts)

| File | state.job refs | Difficulty |
|------|---------------|------------|
| controller/events/model.rs | 49 | HARD |
| controller/tests.rs | 52 | HARD |
| ui/properties/mod.rs | 47 | VERY HARD |
| controller/io.rs | 27 | HARD |
| controller/events/toolpath.rs | 24 | MEDIUM |
| controller/workflow_tests.rs | 17 | HARD |
| controller/events/compute.rs | 16 | MEDIUM |
| ui/setup_panel.rs | 14 | MEDIUM |
| app/viewport.rs | 13 | EASY |
| ui/project_tree.rs | 12 | EASY |
| controller.rs | 10 | EASY |
| ui/toolpath_panel.rs | 10 | EASY |
| controller/events/simulation.rs | 8 | EASY |
| controller/events/mod.rs | 7 | EASY |
| ui/preflight.rs | 5 | EASY |
| app.rs | 3 | EASY |
| ui/status_bar.rs | 3 | EASY |
| ui/workspace_bar.rs | 3 | EASY |
| ui/properties/pocket.rs | 3 | EASY |
| ui/properties/stock.rs | 2 | EASY |
| ui/menu_bar.rs | 2 | EASY |
| ui/sim_op_list.rs | 2 | EASY |
| controller/events/undo.rs | 2 | EASY |
| interaction/picking.rs | 2 | EASY |
| io/setup_sheet.rs | 1 | EASY |
| ui/sim_timeline.rs | 1 | EASY |
| ui/sim_diagnostics.rs | 1 | EASY |
