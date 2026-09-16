# I07 — Optimize UI twins: ui/optimize_modal.rs vs ui/optimize_project.rs
Verdict: TRUE_DUP
## Evidence
- Pair 1 `format_cycle` (0.9527): `crates/rs_cam_viz/src/ui/optimize_modal.rs:1152` ↔
  `crates/rs_cam_viz/src/ui/optimize_project.rs:537`. `diff` of bodies: **identical**
  (line-index drift only; only difference is a `#[cfg(test)]` boundary and doc comment
  on the modal side). Handles non-finite (`"—"`), mm:ss ≥60s, `{:.1}s` below.
- Pair 2 `format_delta` (0.9523): `optimize_modal.rs:953` ↔ `optimize_project.rs:516`.
  `diff` of bodies: **identical** — same field order (feed/rpm/stepover/DOC), same
  `{:.0}`/`:.2}` formats, same `"—"` for empty, same `", "` join. Modal side has extra
  doc comments.
- The two *files* are intentional sibling surfaces (SIBLING by design): modal is the
  per-toolpath Optimize window (U2 of OPTIMIZER_UX_PLAN.md), project.rs is the
  project-level rollup (U3, OPT-004). Only the two helper functions are duplicated.
- Callers (rg): modal `format_cycle` at optimize_modal.rs:677,840,903; `format_delta`
  at :664,890. project.rs `format_cycle` at :265,268,272,377,667,686; `format_delta`
  at :374,664. All read-only rendering of `ParamDelta` / cycle seconds.
- Sentries: duplicated unit tests exist in BOTH files and assert the same contract:
  modal `optimize_modal.rs:1305-1333` (`format_delta_empty`, `format_cycle_short_seconds`,
  `format_cycle_minutes`, `format_cycle_handles_inf`) and project
  `optimize_project.rs:898-924` (same cases, `format_cycle_minutes_format`, etc.).
  No core sentries involved; UI-only.
## Drift / differences
- (TRUE_DUP) None functional. Only drift is documentation: modal copies carry doc
  comments (`/// Format cycle time in mm:ss...`), project copies are bare.
## Proposed cleanup
- home: new `crates/rs_cam_viz/src/ui/components/optimize_format.rs` (or a small
  `ui/components/format.rs`) exporting `pub(crate) fn format_cycle(f64) -> String` and
  `pub(crate) fn format_delta(&ParamDelta) -> String`; both files import and drop
  local copies + one duplicate test set (keep a single copy of the 4 asserts).
  risk: low — pure functions, no state, no snapshots/MCP involvement.
  proof test: keep one `format_cycle(125.0) == "2:05.0"` / `format_delta` assert in
  the new module's `#[cfg(test)]`; run `cargo test -p rs_cam_viz -q` and
  `cargo clippy -p rs_cam_viz --all-targets -- -D warnings`.
