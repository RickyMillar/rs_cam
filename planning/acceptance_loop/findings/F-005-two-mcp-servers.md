# F-005 — Two MCP server implementations of the same 40+ tool surface

- **Stage:** substrate
- **Severity:** medium
- **Status:** deferred — wait for F-001…F-005 of unification plan to land first
- **First found in:** round-01 (2026-05-24)
- **Effort:** XL (multi-week, multi-crate)
- **Linked PRs:** —
- **Source audits:** unification

## Evidence

Two MCP server implementations of the same surface:

- `crates/rs_cam_mcp/src/server.rs` (2151 LOC) — standalone, talks
  directly to `ProjectSession`. `add_toolpath` at line 1317 calls
  `session.add_toolpath` directly.
- `crates/rs_cam_viz/src/mcp_server.rs` (983 LOC) — viz-routed,
  forwards to `McpRequestKind` bridge in
  `crates/rs_cam_viz/src/app/mcp.rs`. `add_toolpath` at line 599
  sends `McpRequestKind::AddToolpath`.

Tools double-implemented (non-exhaustive): `add_toolpath`,
`get_diagnostics`, `load_project`, `screenshot_*`, `save_project`,
`inspect_model`, `list_setups`, `get_tool_load_report`. Descriptions
and param shapes will drift over time without coordination.

The recent MCP overhaul (2026-05-24, shipped) added
`get_operation_schema`, `param_schema_hints`,
`mutation_result_envelope` to one or both — needs verification that
both servers carry the new surface identically.

## Acceptance test

1. **Schema test**: enumerate all MCP tool names from both servers,
   assert identical sets and identical param shapes per tool.
2. **Behavior test**: for a fixed project, the same MCP call sequence
   (load → add_toolpath → set_param → generate → simulate →
   get_tool_load_report) must return byte-for-byte identical JSON
   from both servers.
3. **Smoke test**: re-run the agent smoke suite against
   `cargo run -p rs_cam_mcp -- --headless` (currently this binary
   may not exist; that's part of the fix). Verdicts must match the
   viz-MCP run.

## Files

- `crates/rs_cam_mcp/src/server.rs` — likely to be deleted or
  converted into a thin headless wrapper
- `crates/rs_cam_viz/src/mcp_server.rs` — likely stays as the
  canonical front
- `crates/rs_cam_viz/src/app/mcp.rs` — the bridge / request dispatch
- New: `crates/rs_cam_service/` (optional) — if extracting a service
  crate

## Fix shape — two options

**Option A (recommended):** keep viz's `mcp_server.rs` as the canonical
front; add a `--headless` flag so it can serve without the GUI window.
Delete `rs_cam_mcp/src/server.rs`.

**Option B:** extract a new `rs_cam_service` crate. Both the standalone
CLI MCP and the GUI MCP front delegate to it. Cleaner separation,
heavier change.

**Recommend A** unless we're about to add a second consumer (web
client, language bindings).

## Risk

XL. Coordinated change across 3–4 crates. Coordinated tests across
both servers. Forces F-004 (project loader unification) to be
addressed simultaneously.

## Sub-fixes that come along for free

- F-004 — there will be one loader because there's one server
- F-006 — default `OperationConfig` paths collapse from three to one
- F-009 — diagnostic-delta / gui_banners / warnings emission unifies
  because both servers use the same mutation envelope builder

## Notes

- Blocked by F-001…F-005 of `planning/CODEBASE_UNIFICATION_PLAN.md`
  (the algorithm-substrate fixes). Land those first; come back to
  this one once the substrate is stable.
- **Out of scope:** changing MCP tool semantics. This is purely
  consolidation.
