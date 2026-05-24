# F-014 — Documentation drift: three audit docs describe dead architecture

- **Stage:** docs
- **Severity:** low
- **Status:** landed
- **First found in:** round-01 (2026-05-24)
- **Effort:** S (~30 LOC delete / annotate)
- **Linked PRs:** —
- **Source audits:** unification

## Evidence

`review/SERVICE_LAYER_OWNERSHIP_AUDIT.md:36` and
`review/results/41_duplication.md:1-17` describe a
"Phase 2 not achieved — viz reimplements dispatch" state and cite
"Operation Dispatch Match Arms (~200 LOC)" and "SemanticToolpathOp
Tracing Setup (~440 LOC)" that no longer exist. The execute path is
now actually unified at `crates/rs_cam_core/src/compute/execute.rs:201
execute_operation` with viz delegating
(`crates/rs_cam_viz/src/compute/worker/execute/mod.rs:18, 132, 624`
including a comment confirming removal).

Three docs describing architecture that has been fixed but the docs
have not been updated.

## Acceptance test

1. Read each of the three docs. Either:
   - Mark the resolved sections with "(resolved YYYY-MM-DD — execute.rs:201 unified)" or
   - Move the doc to `archive/` with a note pointing at the current state

## Files

- `review/SERVICE_LAYER_OWNERSHIP_AUDIT.md`
- `review/results/41_duplication.md`
- Any sibling docs in `review/results/` describing dispatch dup

## Fix shape

Pure docs work. Either annotate or archive. Don't delete content
silently — historical context has value if marked clearly.

## Risk

None.

## Notes

- **Out of scope:** any other doc audit. This is specifically the
  service-layer/dispatch-duplication docs.
