# Review: Testing Coverage & Quality

## Scope
Audit all tests across the workspace — unit, integration, benchmarks.

## What to review

### Coverage map
For each module in rs_cam_core, rs_cam_viz, rs_cam_cli:
- Does it have tests? How many?
- What do the tests actually verify?
- Are edge cases covered?
- Are there tests that just check "doesn't panic" vs actually asserting correctness?

### Test quality
- Assertion specificity: `assert!(result.is_ok())` vs `assert_eq!(result, expected_value)`
- Test isolation: do tests depend on each other or global state?
- Test naming: descriptive or opaque?
- Fixture management: hardcoded values or generated?

### Integration tests
- `end_to_end.rs`: what does it cover? Is it actually end-to-end?
- GUI automation tests: how do they work? What's `automation.rs`?
- compute/worker/tests.rs: coverage

### Benchmarks
- `perf_suite.rs`: what's benchmarked?
- Are benchmarks run in CI?
- Do they cover the hot paths?

### Gaps
- Which operations have NO tests?
- Which critical paths (G-code output, simulation) lack tests?
- Is there any property-based / fuzz testing?

### Test infrastructure
- How to run: `cargo test -q`
- Are there test fixtures? Sample files?
- CI: what checks are enforced?

## Output
Write findings to `review/results/40_testing.md` with a coverage matrix showing test status per module.
