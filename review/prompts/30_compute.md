# Review: Compute Orchestration

## Scope
Dual-lane background compute system for toolpath generation and simulation.

## Files to examine
- `crates/rs_cam_viz/src/compute/mod.rs` (coordinator)
- `crates/rs_cam_viz/src/compute/worker.rs` (thread management)
- `crates/rs_cam_viz/src/compute/worker/execute.rs` (operation execution)
- `crates/rs_cam_viz/src/compute/worker/semantic.rs` (semantic tracing)
- `crates/rs_cam_viz/src/compute/worker/helpers.rs`
- `crates/rs_cam_viz/src/compute/worker/tests.rs`

## What to review

### Architecture
- Two lanes: Toolpath and Analysis — why two?
- Thread management: how many threads? Thread pool or dedicated?
- Channel types: MPSC for results? How are requests submitted?
- Queue management: can multiple toolpaths queue? FIFO?

### Cancellation
- Atomic cancel flag — how is it checked in tight loops?
- What happens to partial results on cancel?
- Can you cancel one toolpath while another runs?

### Progress reporting
- Is there progress feedback during long operations?
- Queue depth display in workspace tab badge

### Error handling
- Thread panics: caught by panic handler?
- Operation errors: how do they flow back to UI?
- Memory pressure: what happens on very large operations?

### Execute dispatch
- The giant match on OperationType — is this maintainable?
- Is each operation's parameter extraction consistent?
- Are dressups applied in the same order for all operations?

### Semantic tracing
- What does the semantic worker do?
- How does it feed the debugger UI?

### Testing
- What do the compute/worker/tests.rs cover?

## Output
Write findings to `review/results/30_compute.md`.
