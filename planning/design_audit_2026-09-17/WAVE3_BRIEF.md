# Wave 3 — implementation brief (2026-09-17/18 night)

Four Opus agents. Each agent lands its rows from `SYNTHESIS.md` § Wave 3,
one commit per row, and stops. The operator is asleep; nobody answers a
question, so decide, state the decision in the commit body, and move on.

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
  Pass the message with `-m '...'` in SINGLE quotes (zsh eats backticks
  inside double quotes) or with `git commit -F - -- <paths> <<'EOF'`; never
  from a file in the shared scratchpad. Check `git diff --cached --stat`
  before each commit: a hunk that is not yours means a peer edits that file;
  stop and re-read the ownership table.
- **One owner per file.** In wave 2 two agents edited `catalog/registry.rs`
  at once and each swept the other's hunks into its commit; master was red
  for an hour. The table below names one owner per shared file. If your row
  needs a line in a file you do not own, write the exact edit in your reply
  and leave the file alone.
- Format only your own files: `rustfmt --edition 2024 <file>` per file, then
  `scripts/cargo_lane.sh fmt --all -- --check`. Never `cargo fmt` without a
  file list; it cascades into peers' in-flight files.
- Commit title: `<ID>: <what changed, as a sentence>`. Body: the finding's
  consequence, the break, the sentry, the teeth check. End with your
  co-author trailer.
- Simplified Technical English in prose and comments.
- Do not edit another agent's folder. The assignment table is the contract.

## Assignment

| agent | rows | folders owned | also owns (one owner per file) |
|---|---|---|---|
| core-session | SES-07, SES-01, SES-03, STK-06, and the `retract_strategy = "full"` write at `session/mod.rs:2418` | `session/` (all files) | reads `compute/transform.rs`, does not edit it |
| core-cutting | CUT-03 + CMP-17, CUT-05 (core half), CUT-15, CUT-01 | `dressup/`, `ops/`, `adaptive/`, `adaptive3d/` | `compute/config.rs`, `compute/mod.rs`, `compute/catalog.rs`, `compute/execute/finish_3d.rs`, `rs_cam_viz/src/ui/properties/linking_dressup.rs`, `rs_cam_viz/src/mcp_server.rs` (the `retract_strategy` doc line only), `rs_cam_viz/src/state/toolpath.rs`, `rs_cam_viz/src/state/toolpath/support.rs`, `rs_cam_viz/src/compute/worker/execute/mod.rs`, `rs_cam_viz/tests/snapshots/mcp_wire_surface.json`, `rs_cam_core/tests/adaptive3d_post_tsp_z_monotonicity.rs` |
| cli-mcp | CLI-08, CLI-09, CLI-01 (job half: the `max_stay_down_dist` TOML key and alias), CLI-04, CLI-06, the `retract_strategy` doc example in `rs_cam_mcp/src/server.rs` | `crates/rs_cam_cli/`, `crates/rs_cam_mcp/` | `rs_cam_viz/src/app/mcp/commands.rs` |
| viz-ui | UI-06, UI-01, UI-11 | `rs_cam_viz/src/ui/` | `rs_cam_viz/src/state/runtime.rs`, `rs_cam_viz/src/app/mcp/view.rs` |

Cross-agent dependency: CUT-05 (core) deletes `Adaptive3dParams::max_stay_down_dist`;
CLI-01 (cli) deletes the job key of that name. Each side compiles alone
(the CLI coalesces into `max_stay_down_distance_mm` today), so land in any
order. `retract_strategy` spans three agents by file; each deletes only its
own lines, and the core-cutting agent re-blesses the wire snapshot last.
