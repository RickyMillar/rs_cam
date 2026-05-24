# F-008 — Stale-flag propagation has four authors, no `compute_stale_set` helper

- **Stage:** substrate
- **Severity:** medium
- **Status:** landed
- **First found in:** round-01 (2026-05-24)
- **Effort:** S (~80 LOC)
- **Linked PRs:** —
- **Source audits:** unification

## Evidence

At least four code paths invalidate / mark-stale toolpaths
independently:

1. `crates/rs_cam_core/src/session/compute.rs` —
   `set_toolpath_param_invalidates_result` (test at line 2261) and
   `set_tool_param_invalidates_toolpath_results` (test at line 2331).
   The session does the bookkeeping internally.
2. `crates/rs_cam_viz/src/app/mcp.rs:1828` —
   `mcp_mark_stale_toolpaths`. Called from 8 viz mutation sites
   (lines 2022/2337/2379/2604/2662/2692/2727/2794).
3. `crates/rs_cam_viz/src/controller/events/simulation.rs:22` —
   `invalidate_simulation`. A third path.
4. `crates/rs_cam_mcp/src/server.rs` — the standalone MCP. **Does
   not propagate stale_toolpaths at all** in its mutation responses
   (it pre-dates the mutation envelope).

There is no `compute_stale_set(session, mutation_kind) -> StaleSet`
helper that all four can call into. Each path computes its own
dependent-toolpath set ad-hoc.

## Acceptance test

1. **Unit test**: `set_tool_param(index=0, "diameter", 6.0)` returns
   a `MutationResult.stale_toolpaths` containing every toolpath that
   references tool 0. Test in both `rs_cam_viz` MCP and
   `rs_cam_mcp` standalone MCP.
2. **Parity test**: the same mutation through both MCP servers
   returns identical `stale_toolpaths` arrays.
3. **Regression**: the existing
   `set_toolpath_param_invalidates_cached_result` test at
   `crates/rs_cam_core/src/session/compute.rs:1456` still passes.

## Files

- **New function** in `crates/rs_cam_core/src/session/compute.rs`:
  ```rust
  pub fn compute_stale_set(
      session: &ProjectSession,
      mutation_kind: MutationKind,
  ) -> StaleSet
  ```
  where `MutationKind` enumerates `ToolDiameterChanged{tool_id}`,
  `ToolpathParamChanged{toolpath_id, param}`, `StockChanged`,
  `MaterialChanged`, etc.
- **Replace** 8 viz call sites of `mcp_mark_stale_toolpaths`
- **Replace** `set_toolpath_param_invalidates_result` and
  `set_tool_param_invalidates_toolpath_results` to delegate
- **Add** to standalone MCP `rs_cam_mcp/src/server.rs` — call on
  every mutation, finally emitting `stale_toolpaths` in its
  envelopes

## Fix shape

Mechanical refactor: extract the existing logic from one of the
viz call sites into a core function. Wire the other call sites and
the standalone MCP to use it.

## Risk

Small. Existing tests already cover the behavior — they just need
to point at the unified function.

## Notes

- **Out of scope:** changing the staleness rules themselves. Just
  centralize who computes them.
