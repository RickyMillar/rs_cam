# Wave 2 — implementation brief (2026-09-17 late)

Four Opus agents. Each agent lands its rows from `SYNTHESIS.md` § Wave 2
(plus the FLD-04/05 `test-support` row deferred from wave 1), one commit per
row, and stops. The tool_load rows stay with the power session.

## Rules for every agent

- Read the finding block in your group file and the row in `SYNTHESIS.md`
  before you edit. Re-check the anchor with `rg` first; if the code moved or
  the claim is wrong, write that in your reply and skip the row.
- Read the folder `CLAUDE.md` for every folder you edit. Update it when your
  change makes one of its lines false. Keep it at 40 lines or fewer.
- Breaking changes are fine (ruling 2026-09-16). State each break in the
  commit body. No compatibility shims, no aliases.
- **Cargo only through `scripts/cargo_lane.sh <args>`** (it serialises jobs
  and waits for memory). Never call `cargo` directly. Run the smallest
  sentry first, then the folder sentries your `CLAUDE.md` names, then
  `scripts/cargo_lane.sh clippy -p <crate> --all-targets -- -D warnings`.
  Do not run the core `--features heavy-tests` gate or any wanaka test.
- Every new or changed sentry gets a teeth check: inject the guarded defect,
  see the test go red, restore it. Say in the commit body that you did.
- `cargo fmt` cascades across sibling modules: run
  `scripts/cargo_lane.sh fmt --all -- --check`, fix only files you own,
  and leave any other reported file alone.
- **Foreign files, never touch, never stage:** `.mcp.json`, `.pi/`,
  everything under `crates/rs_cam_core/src/feeds/`, `crates/rs_cam_core/src/tool_load/`,
  `crates/rs_cam_core/src/tool/`, `planning/load_model_2026-09-16/`. The power
  session edits them live; its `tool::tests::tip_deflection_*` tests are red
  and that is not yours. Run `git status --short` before each commit and
  stage only your own paths.
- Commit with explicit paths: `git add -- <files>` then
  `git commit -m "..." -- <files>`. Never `git add -A`, never a bare
  `git commit`, never `git stash`, never `git reset --hard`, never
  `git commit --amend` (master is shared; a peer may have committed on top).
  Pass the message with `-m`, not from a file in the shared scratchpad.
  Check `git diff --cached --stat` before each commit.
- Commit title: `<ID>: <what changed, as a sentence>`. Body: the finding's
  consequence, the break, the sentry, the teeth check. End with your
  co-author trailer.
- Simplified Technical English in prose and comments.
- Do not edit another agent's folder. The assignment table is the contract.

## Assignment

| agent | rows | folders owned | also owns |
|---|---|---|---|
| core-compute | CMP-14 + CMP-24, CMP-15, CMP-08 + CMP-10 + CMP-09 (`scallop_height` half), CMP-27, CMP-01, CMP-26, CMP-16, CMP-22 | `compute/` except the three files core-finish owns | `session/compute/diagnostics.rs`, `session/compute/params.rs`, `session/project_file.rs`, `session/mod.rs` (the `ProjectEvidence` type only), `stock/sim_triage.rs`, the CLI/viz consumers of `ProjectEvidence::holder_collisions` if the type change reaches them |
| viz-shell | EDG-06, SHL-01, SHL-04, SHL-05 | `rs_cam_viz/src/{app,compute,controller,state,render,io}/` and the viz crate-root files | — |
| core-finish | FIN-04, FIN-09, FIN-11, FIN-15, CMP-09 (`classification_sampler` half) | `finish/` | `compute/operation_configs.rs`, `compute/execute/finish_3d.rs`, `compute/execute/findings.rs`, `compute/catalog/registry.rs` rows for pencil and unified finish only |
| core-fields | FLD-04 + FLD-05 as one `test-support` feature row | `maps/`, `surface/` | `crates/rs_cam_core/Cargo.toml` (the feature and `required-features` rows), `crates/rs_cam_viz/Cargo.toml` (the dev-dependency feature), the seven binding test targets, `crates/rs_cam_viz/tests/reach_overlay_p5.rs` |

Not in this wave: SES-07, CUT-03 + CMP-17, CUT-05 + CLI-01 (wave 3, per
`SYNTHESIS.md`); every `TLD-*` row (power session).
