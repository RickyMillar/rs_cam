# Review: Error Handling Audit

## Summary
225+ `unwrap()` calls across the codebase, with the majority in test code. Core library has 164 unwraps (concentrated in dexel_stock.rs: 52, simulation.rs: 19) — most are in `#[cfg(test)]` blocks but reveal API patterns where production callers could panic. The viz layer has 61 unwraps, heavily concentrated in io/project.rs (39). CLI has 0 unwraps (uses anyhow properly). Error types are inconsistent: core uses thiserror for imports, stringly-typed for operations, and ComputeError for 3D ops. No explicit NaN/Inf guards in hot paths.

## Findings

### Unwrap Count by Crate

| Crate | unwrap() | expect() | panic!() | unreachable!() | todo!() | unimplemented!() |
|-------|---------|---------|---------|----------------|---------|-----------------|
| rs_cam_core | 164 | 13 | 0 | 0 | 3 | 1 |
| rs_cam_viz | 61 | 0 | 0 | 0 | 0 | 0 |
| rs_cam_cli | 0 | 0 | 0 | 0 | 0 | 0 |
| **Total** | **225** | **13** | **0** | **0** | **3** | **1** |

### High-Risk Unwraps in Core

#### dexel_stock.rs (52 unwraps) — CRITICAL FILE
- Lines 140, 149: `.as_mut().unwrap()` on lazily-initialized grids — **Safe** (guarded by preceding `if is_none()` check)
- Lines 1140-1391: Multiple `.world_to_cell().unwrap()` — **In tests**, but reveals API pattern where production callers could panic on out-of-bounds queries
- The `world_to_cell()` API returns `Option<(usize, usize)>` — callers must check bounds

#### simulation.rs (19 unwraps) — CRITICAL FILE
- Lines 630, 661, 665, 669: Test-only `.world_to_cell().unwrap()`
- Lines 761, 773-774: `.last().unwrap()` and `.first().unwrap()` on result vectors — **Could panic if empty**
- Production simulation code properly avoids unwrap in hot paths

#### adaptive.rs (8 unwraps) — MEDIUM
- Line 1526: `grid.find_nearest_material().unwrap()` — test only
- Lines 1834, 1839, 2267, 2306: Angle/result calculations — test only

#### Other core files
- arcfit.rs (7), tool/bullnose.rs (11): Geometry calculations, mostly justified after prior checks

### High-Risk Unwraps in Viz

#### io/project.rs (39 unwraps) — CRITICAL FILE
- Line 1312: `fs::create_dir_all(&dir).unwrap()` — File system I/O in tests
- Line 1390: `fs::remove_dir_all(temp_dir).unwrap()` — Cleanup in tests
- Line 1672: `fs::write(&project_path, content).unwrap()` — Test write
- Line 1674: `load_project(&project_path).unwrap()` — Double risk: load already returns Result
- All in test code, but project.rs has 39 unwraps in 1723 lines (2.3% density)

#### app.rs (8 unwraps) — MEDIUM
- GUI state initialization and message passing

### Error Type Strategy

| Layer | Type | Pattern |
|-------|------|---------|
| Core imports | `thiserror` ADTs | `MeshError`, `SvgError` with variants |
| Core operations | Infallible or `String` | Most operations assume valid input |
| Viz 2D ops | `Result<Toolpath, String>` | Stringly-typed errors |
| Viz 3D ops | `Result<Toolpath, ComputeError>` | Structured enum (Cancelled, Message) |
| Viz I/O | `Result<_, String>` | Format strings: "Serialize error: {e}" |
| CLI | `anyhow::Result` | Proper context propagation |

**Gap:** Only ~11 public functions in core return `Result`. Most CAM algorithms are infallible — they assume valid input geometry, no numerical errors, no interruption.

### Edge Case Handling

#### Divide by Zero
- Cell size divisions (`dexel.rs`, `adaptive.rs`): `(u - origin) / cell_size` — **no guard against cell_size == 0.0**
- Feed rate inversions: `1.0 / seg_len_sq` — **no guard against zero-length segments**
- Callers assume valid user input; silent NaN propagation if violated

#### Empty Input
- **Good:** `toolpath.moves.is_empty()` checked before processing; `ray_is_empty()` for dexels; `require_polygons()`, `require_mesh()` fail gracefully
- **Bad:** `points.last().unwrap()` in simulation — panics on empty results; grid bounds unchecked in test patterns

#### NaN/Inf
- **Only 4 NaN checks** found in entire core — minimal explicit handling
- Geometry calculations assume valid floating-point results
- Risk: unfiltered NaN from bad geometry could propagate silently

### User-Facing Error Flow

| Scenario | What User Sees |
|----------|---------------|
| Import failure | ProjectLoadWarning: "Model 'foo' could not be loaded because path was not found" |
| Compute failure | Status in compute lane: ComputeError::Message(string) |
| Cancellation | ComputeError::Cancelled — clean UI feedback |
| Project save failure | Result<(), String> — error message |
| Project load failure | LoadedProject with warnings list (non-fatal; loads with missing tools/models) |

Errors are generally user-visible but not always actionable — string messages don't suggest fixes.

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | High | 164 unwrap() calls in core library (guideline says "avoid unwrap()") | crates/rs_cam_core/src/ |
| 2 | High | dexel_stock.rs has 52 unwraps — highest density in core | dexel_stock.rs |
| 3 | Med | No divide-by-zero guards on cell_size / segment_length | dexel.rs, simulation.rs |
| 4 | Med | Inconsistent error types: String vs ComputeError vs thiserror ADTs | execute.rs, project.rs |
| 5 | Med | Minimal NaN/Inf checking (~4 checks in entire core) | Various |
| 6 | Med | `.last().unwrap()` / `.first().unwrap()` on potentially empty vectors | simulation.rs:761,773 |
| 7 | Low | io/project.rs has 39 unwraps in tests (brittle cleanup) | io/project.rs |
| 8 | Low | 3 todo!() stubs in core (simulation_cut, dropcutter, contour_extract) | Various |

## Test Gaps
- No edge-case tests for: empty stock, zero resolution, degenerate geometry, NaN input
- No tests specifically validating unwrap safety (proving preconditions hold)
- No fuzz testing on file I/O paths (corrupted project files)

## Suggestions

### High Priority (Safety)
1. **Audit all `world_to_cell()` call sites** — Replace `.unwrap()` with proper bounds checking in any non-test code
2. **Add guards against zero cell_size/radius** at grid creation time — return error instead of allowing divide-by-zero
3. **Replace `.first().unwrap()` / `.last().unwrap()`** with `.first().ok_or()` / pattern matching

### Medium Priority (Quality)
4. **Standardize error types** — Define unified `CamError` enum; convert 2D operations from `String` to structured errors
5. **Add NaN validation layer** between viz input and core computation — catch bad geometry early
6. **Replace test unwraps with `.expect("msg")`** for clearer failure diagnostics
7. **Add `#[must_use]` to Result returns** to catch swallowed errors

### Low Priority (Architecture)
8. **Consider returning `Result<Toolpath, CamError>`** from all operation generators
9. **Document preconditions** explicitly (e.g., "cell_size > 0", "polygon non-empty")
10. **Add property-based tests** for NaN/Inf propagation through hot paths
