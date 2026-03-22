# Review: Controller & Event Dispatch

## Scope
The event-driven business logic layer between UI and state.

## Files to examine
- `crates/rs_cam_viz/src/controller/mod.rs`
- `crates/rs_cam_viz/src/controller/events.rs`
- `crates/rs_cam_viz/src/controller/io.rs`
- `crates/rs_cam_viz/src/controller/tests.rs`
- Event enum definition (grep for `AppEvent` in ui/mod.rs or similar)

## What to review

### Event system
- How many event types? (~40+ mentioned)
- Are events flat enum variants or nested?
- Event dispatch: big match statement?
- Are events queued and processed in batch or immediate?

### Controller logic
- Does the controller own state mutation or delegate?
- Coupling between controller and state/compute/IO
- Is business logic in controller or leaked into UI?

### IO handling
- Import/export paths through controller
- File dialog integration
- Error propagation from IO to UI

### Testing
- What do controller/tests.rs cover?
- Are events testable in isolation?

### Code quality
- Large functions that should be split
- Coupling concerns

## Output
Write findings to `review/results/32_controller_events.md`.
