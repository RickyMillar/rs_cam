# Review: Compute Orchestration

## Summary

The dual-lane compute system (Toolpath + Analysis) is well-architected with clean separation, atomic cancellation, progress reporting via phase tracking, and a sophisticated trait-based dispatch for 22 operation types. The semantic tracing infrastructure is excellent. Main concerns are: no thread panic handling (crash cascades through mutex poisoning), 10+ `.expect("lane mutex poisoned")` calls, `execute.rs` at 2492 lines, and test coverage gaps for most operation types and error paths.

## Findings

### Architecture — Two Lanes

- **Toolpath Lane** (`ComputeLane::Toolpath`): 22 operation types dispatched via `SemanticToolpathOp` trait (execute.rs:23-28)
- **Analysis Lane** (`ComputeLane::Analysis`): Simulation and collision detection
- **Why two?** Independent isolation — simulations don't block toolpath generation and vice versa
- Defined at compute/mod.rs:11-14, instantiated at worker.rs:285-286

### Thread Management

- **2 dedicated threads** spawned via `std::thread::spawn()` at worker.rs:428 and 487
- Infinite loops waiting on `Condvar` — threads live for application lifetime
- No thread pool, no dynamic scaling
- **No `catch_unwind()`** — thread panics are unrecoverable

### Channel & Queue Design

- **Results**: Unbuffered `mpsc::channel::<ComputeMessage>()` for results back to UI
- **Requests**: `Mutex<LaneInner<Request>>` with `VecDeque` + `Condvar` wakeup (not channels)
- **Toolpath queue**: FIFO with dedup — `queue.retain()` removes old entries with same toolpath_id (worker.rs:322); if already running, flags cancel + Cancelling state
- **Analysis queue**: Latest-wins — `inner.queue.clear()` on any new submit (worker.rs:396); only one request at a time
- Queue depth exposed via `LaneSnapshot.queue_depth`, displayed in status bar as `"q{depth}"` (status_bar.rs:106)

### Cancellation

- **Per-lane `AtomicBool`** cancel flag with `SeqCst` ordering (worker.rs:199, 208)
- Core library functions accept `&cancel` reference and poll periodically in tight loops
- If cancel flag set and result is Ok, converts to `Err(ComputeError::Cancelled)` (worker.rs:453)
- **Partial traces preserved** — debug/semantic data from cancelled work is still sent to UI (worker.rs:475-477)
- Each lane cancels independently — can cancel toolpath while simulation runs
- `cancel_all()` marks both lanes cancelling (worker.rs:380-385)

### Progress Reporting

- **Lane state machine**: Idle → Queued → Running → Cancelling → Idle (compute/mod.rs:16-53)
- **LaneSnapshot** exposes: state, queue_depth, current_job name, current_phase string, started_at time
- **ToolpathPhaseTracker** (worker.rs:226-282) updates phase via scoped guards
- Example phases: "Initialize stock", "Simulate {name}", "Compute stats", "Apply dressups"
- UI drains results every frame via `drain_compute_results()` (controller/events.rs:854)

### Execute Dispatch

- **Trait-based** (`SemanticToolpathOp`) rather than a monolithic match — excellent design (execute.rs:23-28)
- `OperationConfig::semantic_op()` match (execute.rs:30-56): 27 lines mapping 22 operation types
- 22 separate `impl SemanticToolpathOp` blocks (execute.rs:1521-2477)
- **Test validates exhaustiveness**: `semantic_dispatch_covers_all_operation_types` (execute.rs:2486-2490)
- Each operation follows consistent pattern: extract params → validate inputs → build scope → generate toolpath → annotate structure → bind scopes → return Result

### Dressup Application

- **Single `apply_dressups()` function** (helpers.rs:47-304) called uniformly after all operations
- **Fixed order**: Entry style → Dogbones → Lead in/out → Link moves → Arc fitting → Feed optimization → Rapid ordering
- Each phase creates debug + semantic scopes for tracing
- Order is consistent for all 22 operation types — no per-operation overrides

### Semantic Tracing

- **CutRun analysis** (semantic.rs:8-46): Identifies contiguous cutting sequences, detects closed loops, tracks Z bounds
- **Rich annotation**: Operation labels, parameter snapshots, tool summaries, cut run structures
- **Debugger integration**: Traces stored in `ComputeExecutionOutcome`, artifacts written to `target/toolpath_debug/` as JSON
- **Adaptive3d special case** (execute.rs:1327-1519): Captures runtime algorithm state (pass indices, entry points, exit reasons) via label parsing

### Error Handling

- **Operation errors**: Wrapped as `ComputeError::Message(String)`, sent through channel, mapped to `ComputeStatus::Error` in UI (controller/events.rs:872)
- **Cancellation**: `ComputeError::Cancelled` → `ComputeStatus::Pending` (controller/events.rs:868)
- **Thread panics**: NOT caught — no `catch_unwind()` in either thread
- **Mutex poisoning**: 10+ `.expect("lane mutex poisoned")` calls cascade — one panic poisons all locks, crashing the app
- **Result channel**: `result_tx.send()` failures silently dropped with `let _`
- **Dressup errors**: Feed optimization failure logged as warning (helpers.rs:264-268), other dressup errors silently swallowed

### Helpers

- `build_cutter()` (helpers.rs:7-28): Factory for all 5 tool types
- `require_polygons()` / `require_mesh()` (helpers.rs:34-45): Input validation with descriptive errors
- `make_depth()` / `make_depth_with_finishing()` (helpers.rs:322-359): Depth stepping
- `run_collision_check_with_phase()` (helpers.rs:362-410): Collision detection wrapper
- Annotation helpers in execute.rs: `annotate_operation_scope()`, `annotate_cut_runs()`, `annotate_full_toolpath_item()` — reduce duplication across operations

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | High | No `catch_unwind()` — thread panic kills lane permanently, poisons all mutexes | worker.rs:428, 487 |
| 2 | High | 10+ `.expect("lane mutex poisoned")` — cascade crash on any panic | worker.rs:213, 255, 319, 353, 364, 395, 431, 438, 459, 490+ |
| 3 | High | `execute.rs` is 2492 lines — 22 operation impls + dispatch + helpers in one file | compute/worker/execute.rs |
| 4 | Med | `apply_dressups()` is 258 lines — linear but long | helpers.rs:47-304 |
| 5 | Med | Dressup errors silently swallowed (except feed optimization warning) | helpers.rs:47-304 |
| 6 | Med | `.expect("row has points")` could panic if guard logic changes | execute.rs:2106 |
| 7 | Med | No backpressure on toolpath queue — unlimited growth possible | worker.rs:314-336 |
| 8 | Med | `result_tx.send()` failures silently dropped | worker.rs:475+ |
| 9 | Low | Annotation boilerplate repeated across 22 operation impls | execute.rs:1521-2477 |
| 10 | Low | Parameter extraction + error wrapping pattern duplicated 15+ times | execute.rs |
| 11 | Low | `run_simulation()` marked `#[allow(dead_code)]` — used only via wrapper | execute.rs:60 |

## Test Gaps

- **Only 4 of 22 operation types tested** (Pocket, DropCutter, Waterline, Adaptive3d) — 18 operations untested
- No tests for dressup application order or consistency
- No tests for individual dressup isolation or incompatible combinations
- No tests for semantic trace correctness (move ranges, parameter capture)
- No tests for thread panic recovery or mutex poisoning
- No tests for lane state machine transitions (Cancelling→Idle exhaustiveness)
- No tests for rapid successive requests or large queue depths
- No tests for boundary clipping edge cases (complex polygons, keep_out_footprints)
- No tests for simulation metrics (deviations, rapid_collisions, cut_trace)
- Feed optimization and error handling partially tested (3 tests)
- **Well-tested areas**: Dual-lane independence, cancellation, multi-setup simulation, checkpoint restoration

### Test Summary

| Category | Coverage | Quality |
|----------|----------|---------|
| Toolpath Lane | 90% | High |
| Analysis Lane | 85% | High |
| Cancellation | 95% | High |
| Error Handling | 40% | Medium |
| Dressups | 20% | Low |
| Multi-Setup | 95% | High |
| Operation Types | 18% (4/22) | Low |

## Suggestions

1. **Add panic handler**: Wrap thread loops in `catch_unwind()` with recovery (reset lane to Idle, log error, continue)
2. **Replace `.expect()` with `.ok()`/logging**: Mutex lock failures should degrade gracefully, not crash
3. **Split `execute.rs`**: Move operation impls to `execute/operations_2d.rs`, `execute/operations_3d.rs`, `execute/operations_finish.rs`; keep dispatch + main flow in `execute/mod.rs`
4. **Add integration tests for untested operations**: At minimum Face, Profile, Adaptive, VCarve — the most-used 2.5D operations
5. **Add dressup test matrix**: Test each dressup in isolation and common combinations
6. **Surface dressup errors**: Return warnings alongside results rather than silently swallowing
7. **Add queue depth limit**: Cap toolpath queue at reasonable max (e.g., 50) and reject/warn on overflow
8. **Extract dressup phase helper**: Create `apply_dressup_with_tracing()` to eliminate the repeated debug scope + semantic scope pattern
9. **Document Adaptive3d runtime label format**: The string parser at execute.rs:1243-1313 is critical for the debugger but undocumented
