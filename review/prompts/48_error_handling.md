# Review: Error Handling Audit

## Scope
Audit error handling across the entire codebase: unwrap usage, Result propagation, user-facing error messages.

## What to review

### unwrap() audit
- Search for `.unwrap()` in all core library code (rs_cam_core)
- Per CLAUDE.md: "library code should avoid unwrap()"
- For each unwrap: is it justified? Can it panic in practice?
- Also check: `.expect()`, `panic!()`, `unreachable!()`

### Result propagation
- Do public functions return Result?
- Is `anyhow` used in library code or only CLI/GUI?
- Are error types specific or generic?
- Do errors carry enough context for debugging?

### User-facing errors
- Import errors: what does the user see?
- Compute errors: what does the user see?
- Export errors: what does the user see?
- Are errors actionable (tell user what to fix)?

### Edge case handling
- Divide by zero: any unprotected divisions?
- Empty input: what happens with empty Vec, empty polygon, empty mesh?
- NaN/Inf propagation: any floating point traps?
- Index out of bounds: any unchecked array access?

### Graceful degradation
- If one operation fails, does the rest of the project still work?
- If simulation fails, is the design workspace still usable?
- If a model file is missing on project load, what happens?

## Output
Write findings to `review/results/48_error_handling.md` with: unwrap count per module, high-risk unwraps, and error UX assessment.
