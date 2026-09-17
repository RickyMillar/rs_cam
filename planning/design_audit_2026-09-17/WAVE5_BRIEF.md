# Waves 4 + 5 — implementation brief (2026-09-18, overnight, sequential)

One Opus agent at a time, in the slot order below. Every row here re-paths
files outside its own folder, so no two slots overlap in time. The operator
is asleep; nobody answers a question, so decide, state the decision in the
commit body, and move on. A row whose guard sentry cannot prove the change
(bit-identical grids, byte-identical G-code, a green folder sentry) is
SKIPPED with the reason in the reply, not landed on hope.

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
- **You are the only agent editing.** The power session may still commit
  to `feeds/`, `tool_load/`, `tool/` and `planning/load_model_2026-09-16/`;
  those stay foreign. Everything else is yours for your slot, but stay
  inside the files your rows need.
- Format only your own files: `rustfmt --edition 2024 <file>` per file, then
  `scripts/cargo_lane.sh fmt --all -- --check`. Never `cargo fmt` without a
  file list; it cascades into peers' in-flight files.
- Commit title: `<ID>: <what changed, as a sentence>`. Body: the finding's
  consequence, the break, the sentry, the teeth check. End with your
  co-author trailer.
- Simplified Technical English in prose and comments.
- Do not edit another agent's folder. The assignment table is the contract.

## Slots (run in this order, one agent each)

| slot | agent | rows | extra rows from earlier follow-ups |
|---|---|---|---|
| 1 | core-finish | FIN-14, FIN-02, FIN-12, FIN-13, FIN-05, FIN-06 | `ramp_finish_toolpath` (sixth FIN-11-shape wrapper) |
| 2 | core-stock | STK-02, STK-14, STK-11, STK-10, STK-03 + STK-07, STK-09, STK-12, STK-01 (only under `assert_grids_bit_identical`) | move `HolderCollisionCheck` from `compute::collision_check` to `stock::collision` (removes the upward import). STK-13 is NOT ruled: skip. |
| 3 | core-compute | CMP-12; CMP-21 + CMP-28; CMP-03 + CMP-04 + CMP-06 + CMP-07 + CMP-11 + CMP-13; CMP-02; CMP-18; CMP-20 + CUT-06 + CUT-11; CMP-23 + CMP-25 | — |
| 4 | viz-shell | CMP-19, SHL-02, SHL-03, SHL-06 | `inspect_spans` detail mode emits `spans_truncated`/`spans_total_matching` and `mcp_server.rs` description follows; the GUI's MCP project JSON emits `collision_checks_failed`; `compute/CLAUDE.md`'s revision line narrowed for the collision lane |
| 5 | viz-ui | UI-02, UI-03, UI-04, UI-05, UI-07, UI-09, UI-10, UI-12 | — |
| 6 | core-fields | FLD-01, FLD-02, FLD-06 | the four caches' `reset_stats`/`cache_len`/`clear` test-only doors behind `test-support` |
| 7 | core-cutting | CUT-04, CUT-09, CUT-13, CUT-14 | — |
| 8 | core-session | SES-04, SES-06 | a mesh-less toolpath still runs the fixture half of the holder check (CMP-14 follow-up) if it is one change |
| 9 | core-edges | EDG-02, EDG-07 | `material/CLAUDE.md` (≤ 40 lines) |

Not run: every `FDS-*` and `TLD-*` row (power session); STK-13 (not ruled);
FIN-08 (needs a paired A/B the operator runs).
