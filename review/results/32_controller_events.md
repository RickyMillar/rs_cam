# Review: Controller & Event Dispatch

## Summary

The rs_cam controller implements a functional event-driven architecture where `AppEvent` enums (49 variants) are emitted by UI components and processed through a single `handle_internal_event` method. State mutations are centralized in the controller, compute work is delegated to a pluggable backend, and events can generate follow-up events via a queue. The design is reasonably clean but exhibits maintainability concerns around monolithic event handling, DRY violations in simulation dispatch, and thin test coverage for CRUD and undo/redo paths.

## Findings

### Event System

- **49 distinct `AppEvent` enum variants** defined in `ui/mod.rs`
- **Flat enum** (not nested), making pattern matching straightforward but reducing semantic hierarchy
- **Event categories:** File operations (8), Selection & view (4), Tools/Setups/Fixtures (9), Toolpaths (9), Simulation/workspace (9), Collision/compute (3), Undo/redo/edit (3)
- **Dispatch:** Events flow UI -> controller event queue -> `drain_events()` -> app loop -> `handle_internal_event()` for processing
- **Batch processing:** Events queued in `Vec<AppEvent>` and drained once per frame
- **Self-generating events:** Controller can push new events during handling (e.g., `InspectToolpathInSimulation` generates `SimulationWith` + workspace switch)
- **Dead events:** `AppEvent::ToggleSimToolpath(_)` (line 346) and `AppEvent::RecalculateFeeds(_)` (line 427) have empty match arms

### Controller Logic & State Management

**Architecture:**
```
AppController<B: ComputeBackend> {
  pub state: AppState,                    // Full job/viewport state
  events: Vec<AppEvent>,                  // Event queue
  compute: B,                             // Pluggable compute backend
  pending_upload: bool,                   // Render cache invalidation
  collision_positions: Vec<[f32; 3]>,     // Derived from collision check
  load_warnings: Vec<String>,             // Project load-time warnings
  show_load_warnings: bool,               // UI control
}
```

- Controller owns `AppState` and performs direct field mutation (no setter pattern)
- No business logic leaked into UI layer — correct
- Compute work properly delegated to `ComputeBackend` trait (with `ThreadedComputeBackend` default)
- **High coupling:** Controller directly mutates job/simulation state in 40+ locations
- Complex simulation/setup transformation logic in `run_simulation_with_all()` (lines 445-537) and `run_simulation_with_ids()` (lines 539-651) — tightly coupled to job structure

### IO Handling

- Separate `io.rs` module implementing import/export functions
- **Error propagation:** Results bubble up as `Result<T, String>` to controller, then logged in `app.rs`
- Import sets dirty flag, selection, and pending upload
- Three export methods (`export_gcode`, `export_svg_preview`, `export_setup_sheet_html`) defer to I/O layer — no state mutation
- File dialogs delegated to `rfd` crate + app layer
- **Concern:** Error messages are generic strings — no structured error enum

### Testing

**Test harness:** `tests.rs` (764 lines) with `ScriptedBackend` mock compute backend

**15 tests covering:**
- Simulation result routing and targeted dispatch
- Compute result draining + state update
- Project load warnings
- Fixture project loading (2D and 3D models)
- Full save/load/export smoke test
- Multi-setup simulation state boundaries
- Workspace initialization and transitions
- Simulation staleness tracking
- Playback state reset
- Debug trace metadata propagation

### Code Quality

**Large functions needing decomposition:**

| Function | Lines | Issue |
|----------|-------|-------|
| `handle_internal_event` | 421 | Monolithic match for 49 variants; should split by domain |
| `submit_toolpath_compute` | 168 | Complex setup/mesh transformation; extract `prepare_compute_request` |
| `run_simulation_with_all` | 92 | Setup iteration + group building |
| `run_simulation_with_ids` | 112 | ~70% duplicated with `run_simulation_with_all` |
| `drain_compute_results` | 197 | Three nested match statements; dispatch to separate handlers |

**Other concerns:**
- `unwrap_or(ToolId(0))` and `unwrap_or(ModelId(0))` silently default to nonexistent IDs (lines 207, 214)
- No validation when adding toolpath with missing tool/model — errors only surface at compute time
- `ResetSimulation` clears UI state but doesn't cancel in-flight compute — stale results can overwrite reset state
- Selection can be orphaned when removing setup with selected fixture (partially handled but fragile)

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | High | Monolithic 421-line `handle_internal_event` match should be split by domain | events.rs:21-442 |
| 2 | High | DRY violation: `run_simulation_with_all()` and `run_simulation_with_ids()` are ~70% duplicated | events.rs:445-651 |
| 3 | High | No validation when adding toolpath with missing tool/model; errors only surface at compute time | events.rs:190-228 |
| 4 | Med | `submit_toolpath_compute()` at 168 lines should extract mesh/setup transformation logic | events.rs:684-852 |
| 5 | Med | Three nested match statements in `drain_compute_results()` should dispatch to separate handlers | events.rs:854-1051 |
| 6 | Med | `unwrap_or(ToolId(0))` and `unwrap_or(ModelId(0))` silently default to nonexistent IDs | events.rs:207, 214 |
| 7 | Med | `ResetSimulation` doesn't cancel in-flight compute — stale results can overwrite reset state | events.rs:337-345 |
| 8 | Med | Dead code: `ToggleSimToolpath` and `RecalculateFeeds` have empty match arms | events.rs:346, 427 |
| 9 | Med | Error handling inconsistent: some paths return `Result<Option<T>, String>`, others `Result<T, String>` | io.rs:13-52 |
| 10 | Low | Selection orphaning possible when removing setup with selected fixture | events.rs:89-106 |
| 11 | Low | Coordinate frame transforms spread across controller and state — no single source of truth | events.rs:474-482 |

## Test Gaps

1. **Event routing:** No test verifies that event type X -> handler Y mapping is complete
2. **CRUD operations:** No isolated tests for add/remove/rename setup, tool, fixture, keepout, toolpath
3. **Undo/redo:** No test coverage for undo/redo paths
4. **Export validation:** No tests that exported formats (gcode, svg, html) are well-formed
5. **Concurrency:** No tests for race conditions between compute completion + UI edits
6. **Compute cancellation:** No test that cancelling one compute lane doesn't affect others
7. **Selection cascades:** No tests for selection invariants when entities are removed
8. **Collision check:** No test for collision check event handling and state wiring
9. **Import validation:** No tests for invalid/malformed imports (bad STL winding, etc.)

## Suggestions

### Short-Term
1. **Split `handle_internal_event` by domain** — dispatch to `handle_io_event`, `handle_tree_event`, `handle_compute_event`, `handle_history_event`. Halves function size and surfaces domain layers.
2. **Extract `drain_compute_results` into three handler methods** — `handle_toolpath_result`, `handle_simulation_result`, `handle_collision_result`.
3. **Consolidate simulation dispatch** — extract common `build_simulation_groups()` helper taking a toolpath filter function; use for both `run_simulation_with_all` and `run_simulation_with_ids`.
4. **Remove dead code** — delete or implement `ToggleSimToolpath` and `RecalculateFeeds` handlers.
5. **Validation at creation time** — `AddToolpath` event should validate tool/model exist before accepting.

### Medium-Term
6. **Coordinate frame transforms** — create a `CoordinateFrame` module with single source of truth for setup-local <-> stock-global transforms.
7. **Simulation state staleness** — ensure `ResetSimulation` cancels in-flight compute.
8. **Replace `Result<T, String>` with proper error enum** for structured error handling.

### Test Expansion
9. **Add 8-10 new tests:** One per CRUD operation, undo/redo workflows, export format validation, selection cascading on entity removal.
