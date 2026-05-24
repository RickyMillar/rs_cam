# F-015 — Op-precondition static validation missing (rest, drill, project_curve)

- **Stage:** substrate
- **Severity:** medium
- **Status:** landed (2026-05-25)
- **First found in:** round-01 (2026-05-24)
- **Effort:** M (~150 LOC, new diagnostics adapter)
- **Linked PRs:** —
- **Source audits:** smoke-run evidence

## Evidence

Three op kinds have generate-time preconditions that are NOT surfaced
in static validation. The user (or agent) sees the error only when
`generate_toolpath` runs, leaving the GUI / MCP in a wedged state
where setting params returns OK but generation errors.

Confirmed cases:

- **Rest** (AS006): "Rest machining requires an earlier enabled
  operation in the same setup using the previous tool on the same
  model". `set_toolpath_param(prev_tool_id=1)` returns OK with no
  warning; the precondition is checked at generate time only.
- **Drill** (user-flagged): "drill operation has that rest error
  still" — similar precondition pattern needs identification.
- **project_curve** (AS018): requires both a source curve model and
  a target surface model. Currently no validation when the project
  only has one model.

## Acceptance test

1. **Unit test (rest)**: add a rest op without a prior tool of the
   correct kind to the setup. `add_toolpath` MutationResult must
   include a `warning` / `diagnostic_delta` entry pointing at the
   missing prior op. Same on `set_toolpath_param(prev_tool_id=N)`
   where toolpath N is missing/wrong-tool.
2. **Unit test (project_curve)**: add a project_curve op in a project
   with only one model. `add_toolpath` MutationResult must include a
   warning that the op needs a second model.
3. **Unit test (drill)**: identify the equivalent precondition and
   surface it.
4. **Smoke verification**: re-run AS006 rest. The precondition error
   must appear in the `add_toolpath` MutationResult, not only at
   generate.

## Files

- **New file**: `crates/rs_cam_core/src/diagnostics/adapters/from_preconditions.rs`
- Existing: similar shape to
  `crates/rs_cam_core/src/diagnostics/adapters/from_static_checks.rs`
  (the source of `geom.plunge_exceeds_feed`)
- Existing: `crates/rs_cam_core/src/rest.rs` — generate-time
  precondition check; extract into the static-validation adapter
- Existing: `crates/rs_cam_core/src/drill.rs` (or wherever the drill
  precondition lives — needs identification)
- Existing: `crates/rs_cam_core/src/project_curve*.rs` — needs the
  surface-model check

## Fix shape

Mirror the existing `geom.plunge_exceeds_feed` rule shape. Each
adapter rule:

```rust
pub fn check_rest_preconditions(toolpath: &Toolpath, setup: &Setup) -> Vec<Diagnostic> {
    // Static check: prev_tool_id is set AND points at an enabled toolpath
    // in the same setup with the right tool.
}
```

Adapter outputs feed into `mutation_result_envelope` via the existing
`diagnostic_delta` plumbing.

## Risk

S-M. Mostly mechanical — the existing static_check adapter is the
template. The hard part is enumerating preconditions across the 22
op kinds (likely only 3–5 need this).

## Notes

- **Out of scope:** validating preconditions across setup ordering
  (e.g. "this rest comes before its prior tool in the setup
  ordering"). That's a separate finding if it turns out to bite.
- **Defensive secondary fix considered:** make generate-time errors
  bubble up through the mutation envelope too. Reject — better to
  catch them statically.
