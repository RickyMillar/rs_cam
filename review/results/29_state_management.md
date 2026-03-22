# Review: State Management & Undo/Redo

## Summary

AppState is a clean single struct with ID-based sub-state references and no circular dependencies. Mutation is nominally centralized through AppController, but UI panels also mutate state directly (stock sliders, machine presets, toolpath params). The undo/redo system is structurally complete for 5 action types but **only Stock changes are actually recorded** — Tool, Post, Machine, and ToolpathParam undo handlers exist as dead code. No tests exist for the history system.

## Findings

### State Shape

- **Single unified struct** `AppState` (state/mod.rs:26-35) with 7 fields: workspace, job, selection, viewport, simulation, history, show_preflight
- **ID-based references** throughout: ModelId, ToolId, ToolpathId, SetupId, FixtureId, KeepOutId
- **Hierarchical ownership**: JobState.setups contains Vec<ToolpathEntry>, Vec<Fixture>, Vec<KeepOutZone>
- **No Arcs for ownership** — Arc used only for simulation results (SimulationResults.playback_data) and debug trace artifacts
- **No circular references**

### Serialization

- **Partial**: Individual config types (ToolConfig, StockConfig, PostConfig, ToolType) derive Serialize/Deserialize
- **NOT serializable**: LoadedModel, Setup, ToolpathEntry (holds Arc<Toolpath>), AppState itself
- **Custom save/load** in `io/project.rs` — manually serializes/reconstructs JobState as JSON
- Simulation results are NOT persisted; recomputed on load

### Consistency Risks

- **Orphaned ID references**: ToolpathEntry has tool_id and model_id that may not exist after deletion — no foreign key enforcement (job.rs:1187-1191)
- **Stale selection**: Selection::Fixture(setup_id, fixture_id) persists if parent setup deleted — cleanup only fires when the exact deleted item matches (controller/events.rs:76-80, 91-100)
- **Simulation staleness**: Results reference toolpath boundaries by ID; tracked via edit_counter comparison (simulation.rs:259-260) but stale results persist until re-run
- **Semantic index orphans**: SimulationDebugState.semantic_indexes HashMap not cleaned on toolpath deletion

### Mutation Patterns

- **Controller (intended path)**: controller/events.rs (1267 lines) handles AppEvent with 28 call sites to `mark_edited()`
- **UI direct mutations** (bypass):
  - properties/mod.rs:36 — directly mutates `state.job.stock` in slider loop
  - properties/mod.rs:127 — finds and mutates toolpath via `state.job.find_toolpath_mut(id)`
  - properties/mod.rs:452 — directly assigns `state.job.machine = presets[i].1.clone()`
  - sim_timeline.rs — mutates `state.simulation.playback.current_move` directly
- **No atomic transactions**: Multiple edits can occur in one frame without transaction boundaries; partial mutation on panic is possible

### Dirty Tracking

- `mark_edited()` (job.rs:1305-1308): Sets `dirty = true` and increments `edit_counter`
- 28 call sites in controller/events.rs, 2 in ui/properties/mod.rs, 1 in controller/tests.rs
- Import operations set `dirty = true` directly without incrementing counter (intentional)
- **Staleness detection**: `SimulationState.is_stale(current_edit_counter)` compares counters
- **Toolpath debounce**: `ToolpathEntry.stale_since: Option<Instant>` tracks when params last changed

### Selection Model

- **Single-item enum** (selection.rs:4-17): 10 variants (None, Stock, PostProcessor, Machine, Model, Tool, Setup, Fixture, KeepOut, Toolpath)
- **No multi-select support**
- **Reactive dispatch**: AppEvent::Select → controller mutates state.selection → properties panel reads it
- **Stale cleanup**: Only clears selection when the exact deleted item was selected — no cascading cleanup for child deletions

### Undo/Redo System

- **5 action types** defined in UndoAction enum (history.rs:6-31): StockChange, PostChange, ToolChange, ToolpathParamChange, MachineChange
- **Only StockChange is actually recorded** — push happens in properties/mod.rs:31-54 via snapshot pattern
- **Tool/Post/Machine/ToolpathParam**: Handlers exist in Controller::undo() and Controller::redo() (controller/events.rs:1054-1130) but these actions are **never pushed** — dead code
- **Stack depth**: 100 max (history.rs:53-55), oldest removed on overflow via `Vec::remove(0)`
- **Redo invalidation**: New push clears redo stack (history.rs:50-52) — standard behavior
- **Memory model**: Full clones of state objects per action, no incremental diffs
- **Keyboard shortcuts**: Ctrl+Z / Ctrl+Shift+Z (ui/menu_bar.rs:7-21)
- **Snapshot lifecycle issue**: stock_snapshot in UndoHistory persists if user leaves stock panel mid-drag without releasing

### Undo Edge Cases

- **Undo during compute**: No synchronization — undo modifies job state while in-flight compute uses captured snapshot; results may overwrite subsequent redo
- **Toolpath deletion**: Not undoable — deleted toolpath lost permanently even though subsequent stock edits can be undone
- **Tool/Post/Machine changes**: Undo handlers exist but are never triggered (dead code)

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | High | Only StockChange records undo — Tool/Post/Machine/ToolpathParam handlers are dead code | history.rs, properties/mod.rs:31-54, controller/events.rs:1054-1130 |
| 2 | High | UI panels directly mutate state, bypassing controller | properties/mod.rs:36, 127, 452 |
| 3 | Med | Orphaned ID references after tool/model deletion — no FK validation | job.rs:1187-1191 |
| 4 | Med | No undo for add/remove toolpath, tool, setup, fixture, or import | controller/events.rs:57-281 |
| 5 | Med | Stale selection persists when referenced item's parent is deleted | controller/events.rs:76-100 |
| 6 | Med | Undo during active compute can cause state inconsistency | history.rs, compute/worker.rs |
| 7 | Med | No atomic transactions — partial mutation possible on panic | properties/mod.rs, controller/events.rs |
| 8 | Low | Stock snapshot persists if user leaves panel mid-drag | properties/mod.rs:33-34 |
| 9 | Low | `Vec::remove(0)` for undo stack overflow is O(n) — could use VecDeque | history.rs:54 |
| 10 | Low | SimulationDebugState.semantic_indexes not cleaned on toolpath deletion | simulation.rs:137 |

## Test Gaps

- **No tests for history system** — no unit tests in history.rs, no undo/redo tests in controller/tests.rs
- No tests for selection cleanup on cascading deletion
- No tests for orphaned ID reference detection
- No tests for concurrent undo + compute interaction
- State module has some tests in simulation.rs (9 test functions at line 1610+) but none for job, selection, or history

## Suggestions

1. **Wire undo recording for all 5 types**: Add snapshot + push patterns in tool, post, machine, and toolpath param property editors (matching the stock pattern)
2. **Route all mutations through controller**: Replace direct state mutations in properties with AppEvent emission — controller methods handle the actual mutation + mark_edited() + undo push
3. **Add FK cleanup on deletion**: When a tool/model is deleted, validate all ToolpathEntry references and clear/warn on orphans
4. **Add cascading selection cleanup**: On any deletion, check if Selection references the deleted item or any of its children
5. **Add history unit tests**: Test push/pop/redo-invalidation, stack overflow, and snapshot lifecycle
6. **Consider VecDeque for undo stack**: Replace Vec with VecDeque to make `remove(0)` O(1)
7. **Clear stock snapshot on selection change**: When selection moves away from Stock, drop any held snapshot
