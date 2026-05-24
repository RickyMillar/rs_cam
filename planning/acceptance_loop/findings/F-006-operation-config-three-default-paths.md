# F-006 — Default `OperationConfig` produced via three paths

- **Stage:** suggest
- **Severity:** medium
- **Status:** open — partially absorbed by F-003
- **First found in:** round-01 (2026-05-24)
- **Effort:** M (~200 LOC)
- **Linked PRs:** —
- **Source audits:** suggest-pipeline + unification

## Evidence

Same `OperationType::Pocket` (or any other variant) can produce three
different fully-populated configs depending on entry point:

- `crates/rs_cam_mcp/src/server.rs:1348` — uses
  `OperationConfig::new_default` directly
- `crates/rs_cam_viz/src/io/project.rs:806, 1093` and
  `crates/rs_cam_viz/src/state/toolpath/entry.rs` — builds through
  `ToolpathEntry::for_operation`, which can override via UI defaulting
- `crates/rs_cam_cli/src/job.rs` — `parse_job_file` parses its own

Canonical defaults exist as 23 `impl Default` blocks in
`crates/rs_cam_core/src/compute/operation_configs.rs:92-834`, wrapped
by `OperationConfig::new_default`. But the viz path layers stock /
machine context that the MCP path doesn't, and the CLI path doesn't
share the wrapping at all.

## Acceptance test

1. **Equivalence test**: for each op kind, construct a default
   `OperationConfig` via each of the three paths against the same
   project / tool / machine context. Assert all three produce
   identical configs.

## Files

- `crates/rs_cam_mcp/src/server.rs:1348`
- `crates/rs_cam_viz/src/io/project.rs:806, 1093`
- `crates/rs_cam_viz/src/state/toolpath/entry.rs`
- `crates/rs_cam_cli/src/job.rs`
- `crates/rs_cam_core/src/compute/operation_configs.rs:92-834` — the
  canonical `impl Default` blocks

## Fix shape

Once F-003's `suggest_params()` lands, all three paths should call
it. The CLI's `parse_job_file` still has its own vocabulary to deal
with (job vs project), but the resulting `OperationConfig` should
come from the same function.

## Risk

Medium. Likely cleans up as a side effect of F-003 and F-005. Worth
keeping as a separate finding to verify equivalence in tests.

## Notes

- **Partially absorbed by F-003** — the viz and core paths collapse
  when `suggest_params` ships. CLI path remains separate until F-005
  (merge MCP servers / unify CLI plumbing).
- Re-evaluate this finding's status after F-003 and F-005 land.
