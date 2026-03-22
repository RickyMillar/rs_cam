# Review: State Management & Undo/Redo

## Scope
The canonical app state shape, mutation patterns, undo/redo stack.

## Files to examine
- `crates/rs_cam_viz/src/state/mod.rs` (coordinator)
- `crates/rs_cam_viz/src/state/job.rs` (project data)
- `crates/rs_cam_viz/src/state/toolpath.rs` + sub-files
- `crates/rs_cam_viz/src/state/simulation.rs`
- `crates/rs_cam_viz/src/state/viewport.rs`
- `crates/rs_cam_viz/src/state/selection.rs`
- `crates/rs_cam_viz/src/state/history.rs` (undo/redo)

## What to review

### State shape
- Is AppState a single struct or split across sub-states?
- How do sub-states reference each other? (IDs, indices, Arcs?)
- Are there any circular references or inconsistency risks?
- Is the state shape serializable? (for project save/load)

### Mutation patterns
- Who mutates state? Only controller? Or do UI panels mutate directly?
- Are mutations atomic or can partial updates leave inconsistent state?
- Dirty tracking: what triggers `mark_edited()`?

### Undo/redo
- What actions are undoable? (Stock, Post, Tool, ToolpathParam, Machine)
- What's NOT undoable? (add/remove toolpath, import, setup changes?)
- Stack depth: 100 — is that sufficient?
- Redo invalidation: does a new edit clear the redo stack?
- Memory cost of undo snapshots

### Selection
- Selection model: single item, enum-based
- Multi-select support?
- Selection ↔ properties panel sync

### Testing & code quality

## Output
Write findings to `review/results/29_state_management.md`.
