# F-009 — Five diagnostic views, only viz emits them

- **Stage:** substrate
- **Severity:** medium
- **Status:** open — partially lands with F-005
- **First found in:** round-01 (2026-05-24)
- **Effort:** M (~200 LOC)
- **Linked PRs:** —
- **Source audits:** unification

## Evidence

The mutation envelope (shipped 2026-05-24) introduces five derived
views: `gui_banners`, `warnings`, `diagnostic_delta`,
`stale_defaults`, `get_diagnostics`. The view assembly logic lives at
`crates/rs_cam_viz/src/app/mcp.rs:1883-1936`
(`mcp_mutation_result`), and it builds all five from one
`diagnose_*` snapshot — **but only in viz**. The standalone
`rs_cam_mcp` server returns plain text for the same mutations.

`stale_defaults` is rebuilt in three places:

- `crates/rs_cam_core/src/compute/validate.rs:100`
- `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:2232`
- `crates/rs_cam_viz/src/app/mcp.rs:590`

Three calls, no shared cache. Different consumers can see different
stale-default lists for the same project state.

## Acceptance test

1. **Parity test**: mutation through both MCP servers returns
   identical envelope shapes including `gui_banners`,
   `diagnostic_delta`, and `warnings` payloads.
2. **Cache test**: `stale_defaults` reads from one place only; assert
   via call-count instrumentation that it isn't recomputed N times
   per mutation.
3. **Field consistency**: warnings and diagnostic_delta should not
   double-emit the same diagnostic — assert no shared IDs between the
   two arrays on a single mutation.

## Files

- `crates/rs_cam_viz/src/app/mcp.rs:1883-1936` — the assembler;
  candidates for relocation to core
- `crates/rs_cam_core/src/compute/validate.rs:100` — one
  stale_defaults producer
- `crates/rs_cam_viz/src/ui/properties/operations/mod.rs:2232`,
  `crates/rs_cam_viz/src/app/mcp.rs:590` — two more stale_defaults
  producers

## Fix shape

- Move `mcp_mutation_result` (or its core logic) into core so both
  MCP servers can call it
- Single `stale_defaults` cache on `ProjectSession`, invalidated by
  the same `compute_stale_set` from F-008

## Risk

Medium. Partially lands when F-005 merges the MCP servers. Without
F-005, you'd duplicate the assembler in both servers — possible but
less satisfying.

## Notes

- **Lands cleanly with F-005.** If you have to do this independently,
  extract the assembler to core but keep both MCP fronts calling it.
