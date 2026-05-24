# F-004 — Three project-TOML loaders

- **Stage:** substrate
- **Severity:** medium
- **Status:** open — likely lands with F-005
- **First found in:** known-issue prior to round-01; documented in
  `planning/LOADER_UNIFICATION.md` and
  `memory/project_two_loader_divergence.md`. Confirmed by round-01
  audit.
- **Effort:** L (multi-file, ~500-800 LOC)
- **Linked PRs:** —
- **Source audits:** unification

## Evidence

Three project-TOML loaders disagree on missing-operation behavior:

1. `crates/rs_cam_core/src/session/project_file.rs:745` — strict.
   **Drops toolpaths** that lack an `[operation]` sub-table.
2. `crates/rs_cam_viz/src/io/project.rs:441, 806, 1093` — tolerant.
   Falls back to `OperationConfig::new_default` when missing.
3. `crates/rs_cam_cli/src/job.rs:232` — `parse_job_file`, a third
   vocabulary entirely.

The two-loader divergence has bitten the project before (see
`memory/project_two_loader_divergence.md`). The third path
(`cli/job.rs`) makes it a three-way split.

## Acceptance test

1. **Unit test**: load a TOML with a toolpath missing `[operation]`
   through all three entry points; assert identical behavior (either
   all drop, all default — pick one).
2. **Unit test**: load the same valid project through all three;
   assert byte-for-byte identical in-memory `Project` state (sans
   loader-specific metadata).
3. **Round-trip test**: save a project from each entry point and load
   from each; assert all 9 (3×3) combinations round-trip.

## Files

- `crates/rs_cam_core/src/session/project_file.rs:745`
- `crates/rs_cam_viz/src/io/project.rs:441, 806, 1093`
- `crates/rs_cam_cli/src/job.rs:232`
- `planning/LOADER_UNIFICATION.md` — pre-existing design notes

## Fix shape

Single `core::project_io::load_project(path) -> Result<Project, ProjectLoadError>`
in core. Both MCP servers and CLI delegate to it. Default the
tolerant behavior (default missing `[operation]` to `new_default`)
but emit a structured warning so callers can decide whether to
present a banner.

## Risk

Medium. Likely needs to land with F-005 (merge MCP servers) since
the same code paths use both. Decide on tolerant-vs-strict default
before starting.

## Notes

- **Decision needed before implementing:** tolerant or strict on
  missing operation? Surface the question to the user — they were
  burned by silent drop before.
