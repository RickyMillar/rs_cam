# Project-file loader unification

**Status:** backlog (surfaced April 2026 during adaptive remediation Package H)

## Problem

The workspace has **two project-file loaders** that read the same TOML
format and disagree on whether `[[setups.toolpaths.operation]]` is
required:

| Loader | Location | Behavior on missing `operation` |
|---|---|---|
| rs_cam_core | `crates/rs_cam_core/src/session/project_file.rs:745` | **Drops the toolpath silently** |
| rs_cam_viz | `crates/rs_cam_viz/src/io/project.rs:1078` | Falls back to `OperationConfig::new_default(op_type)` |

**Entry points that hit each:**
- MCP `load_project` → `ProjectSession::load` → rs_cam_core (strict)
- GUI "Open Project" → rs_cam_viz (tolerant)
- CLI `job <file.toml>` → separate parser at `crates/rs_cam_cli/src/job.rs` (third vocabulary)

## Symptom

A hand-written or migrated TOML that lists `[[setups.toolpaths]]` with
only `type = "..."` (no nested `[setups.toolpaths.operation]` table)
round-trips cleanly through the GUI but **silently loses all toolpaths**
through the MCP. The user sees `"Loaded 'Name' — N setups, 0 toolpaths"`
with no warning.

## Discovery

Surfaced during Package H of the April adaptive remediation series while
migrating `crates/rs_cam_viz/tests/fixtures/sample_*_project.toml` from
format_version=1 to format_version=3. The migrated fixtures had
`[[setups.toolpaths]]` blocks but only the `type` field, and loaded via
the MCP with 0 toolpaths. Commit `8cf6334` worked around by removing
the toolpath blocks entirely and documenting the issue in each fixture.

## Recommended fix

Unify to the **tolerant** behavior (rs_cam_viz's). Two options:

1. **Extract the tolerant logic into `rs_cam_core`** and have rs_cam_viz
   delegate to it. Pros: one canonical loader. Cons: rs_cam_core would
   need `OperationConfig::new_default` in scope, which it already has.

2. **Keep both loaders but make rs_cam_core default on None** using the
   same fallback logic rs_cam_viz has. Pros: surgical. Cons: two codebases
   to keep in sync forever.

Both options should **emit a warning** when the fallback fires, so the
user knows their TOML was imprecise.

## Estimated effort

- Option 1: ~4 hours (understand the scope of `OperationConfig::new_default`, lift the tolerant logic into core, update the viz loader to delegate)
- Option 2: ~1 hour (add the fallback to `project_file.rs:745`)

## Tests

- Add a unit test in `crates/rs_cam_core/src/session/project_file.rs` that loads a minimal TOML with `[[setups.toolpaths]]` missing `operation` and asserts 1 setup / 1 toolpath (not 0).
- Add a warning-emission assertion via `tracing_test` or subscriber capture.

## Related

- Commit `8cf6334` — Package H fixture migration with the trap documented
- Memory: `project_two_loader_divergence.md`
- April 2026 adaptive review: `planning/adaptive_review_2026-04.md` (not explicitly listed as a finding there — this surfaced later during remediation)
