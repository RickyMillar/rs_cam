# Review: Testing Coverage & Quality

## Summary
Strong core library coverage (691 unit tests across 54 modules, plus 9 benchmark groups) but critical gaps in CLI (0 tests), dropcutter (3 tests), and simulation_cut (2 tests). No property-based or fuzz testing. The viz layer has 81 tests covering compute workers and controller state. Integration testing is minimal (2 end-to-end tests). Test quality is generally good with specific assertions but some weak "doesn't panic" checks.

## Findings

### Coverage by Crate

#### rs_cam_core: 691 tests, 54 modules
Well-tested (>20 tests): adaptive (38), adaptive3d (31), dexel (32), dexel_stock (21), polygon (25), depth (24), simulation (24), dressup (24), feeds/geometry (20), feeds/mod (20)

Moderately tested (7-15): arcfit (14), boundary (15), collision (11), fiber (14), geo (12), pencil (9), pushcutter (11), radial_finish (11), ramp_finish (12), rest (12), scallop_math (12), slope (13)

**Undertested critical modules:**
- dropcutter.rs: **3 tests** (core surface generation algorithm)
- face.rs: **4 tests**
- horizontal_finish.rs: **4 tests**
- pipeline.rs: **4 tests**
- simulation_cut.rs: **2 tests**

Tool submodule: bullnose (20), tapered_ball (18), vbit (16), ball (7), flat (4)

#### rs_cam_viz: 81 tests
- compute/worker/tests.rs: 20 (worker lane execution, debug tracing)
- controller/tests.rs: 15 (app state, project I/O, simulation events)
- io/setup_sheet.rs: 14 (HTML generation)
- io/presets.rs: 9 (tool/setup presets)
- io/project.rs: 7 (project save/load)
- compute/worker/execute.rs: 1

#### rs_cam_cli: **0 tests**

### Test Quality

**Assertion patterns:**
- 311 `assert_eq!()` (value equality — good)
- 1101 `assert!()` (generic boolean — mixed quality)
- 1 `assert_ne!()`

**Strengths:**
- G-code tests use specific string matching: `assert!(gcode.contains("G0 X0.000 Y0.000 Z10.000"))`
- Numeric tests use epsilon tolerance: `assert!((center_cl.z - 20.0).abs() < 1.0)`
- Toolpath tests check move counts, positions, feed rates
- Worker tests use `assert_toolpaths_match()` for move-by-move equivalence

**Weaknesses:**
- Some tests use bare `assert!(result.is_ok())` without capturing error details
- Some only verify "doesn't panic" vs correct output (e.g., grid assertions just check `rows > 5`)
- Floating-point comparisons sometimes lack specified tolerance

### Integration Tests
- `end_to_end.rs`: **2 tests only** — load STL → dropcutter → G-code; hemisphere → dropcutter → G-code
- No integration tests for: DXF/SVG input, multi-operation sequencing, simulation, CLI-to-file

### Benchmarks (perf_suite.rs)
9 criterion groups: batch_drop_cutter, point_drop_cutter, spatial_index, stamp_tool, waterline, polygon_ops, arc_fitting, stamp_linear_segment, simulate_toolpath

**Missing benchmarks:** adaptive clearing, dressups, collision checking, 3D surface operations, fine-resolution (<0.25mm) simulation

### Test Infrastructure
- Fixtures: `fixtures/terrain_small.stl` (1.9M), demo SVGs, 7 viz test fixtures
- CI: `.github/workflows/ci.yml` — `cargo test -q`, separate viz regression job
- Dev deps: `approx`, `criterion` (no proptest, quickcheck, or cargo-fuzz)
- Hand-written mocking via `ScriptedBackend` trait

## Issues Found

| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | High | CLI crate has zero tests | crates/rs_cam_cli/ |
| 2 | High | Dropcutter (core surface generation) has only 3 tests | dropcutter.rs |
| 3 | High | simulation_cut.rs has only 2 tests | simulation_cut.rs |
| 4 | Med | No property-based or fuzz testing for geometric algorithms | Entire codebase |
| 5 | Med | Only 2 end-to-end integration tests | tests/end_to_end.rs |
| 6 | Med | Face milling has only 4 tests | face.rs |
| 7 | Med | Pipeline (operation sequencing) has only 4 tests | pipeline.rs |
| 8 | Low | No benchmark regression detection in CI | .github/workflows/ci.yml |
| 9 | Low | Only 1 real-world mesh fixture | fixtures/ |

## Test Gaps
- **Zero test types:** CLI arg parsing, fuzz testing, property-based testing, visual regression, snapshot tests for G-code
- **Undertested critical paths:** dropcutter grid accuracy, simulation cut analytics, face milling generation, pipeline sequencing
- **Missing integration scenarios:** multi-operation on same stock, simulation-then-export, DXF/SVG import flows, undo/redo, cancellation behavior, error recovery
- **No stress test fixtures:** complex geometries, sharp corners, thin walls, deep pockets, large meshes

## Suggestions

### High Priority
1. **Add CLI tests** — bash/integration harness for batch processing
2. **Expand dropcutter tests** — geometric edge cases, grid accuracy verification, various tool types
3. **Add property-based tests** for geometric invariants (e.g., toolpath stays within bounds, no NaN in output)

### Medium Priority
4. **Expand integration tests** — multi-op sequencing, simulation round-trip, import→generate→export
5. **Add fuzz testing** for file parsers (DXF/SVG/STL malformed input)
6. **Add complex geometry fixtures** — sharp corners, thin walls, deep pockets
7. **Integrate benchmark regression detection** into CI

### Low Priority
8. **Add snapshot tests** for G-code output (detect unintended format changes)
9. **Add code coverage reporting** (tarpaulin or llvm-cov)
