# Wave 1 — implementation brief (2026-09-17 evening)

Seven Opus agents, one folder group each, no shared file. Each agent lands
its rows from `SYNTHESIS.md` § Wave 1 (plus FIN-01, ruled in), one commit per
row, and stops.

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
- **Foreign dirty files, never touch, never stage:** `.mcp.json`, `.pi/`,
  `crates/rs_cam_core/src/compute/tool_config.rs`,
  `crates/rs_cam_core/src/feeds/predict.rs`,
  `crates/rs_cam_core/src/feeds/suggest/invariants.rs`,
  `crates/rs_cam_core/tests/the_bending_diameter_knows_the_flute_count_g_bendeq.rs`.
  A clippy or test error that names only one of those files is not yours.
- Commit with explicit paths: `git add -- <files>` then
  `git commit -m ... -- <files>`. Never `git add -A`, never a bare
  `git commit`, never `git stash`, never `git reset --hard`. Check
  `git diff --cached --stat` before each commit.
- Commit title: `<ID>: <what changed, as a sentence>`. Body: the finding's
  consequence, the break, the sentry, the teeth check. End with your
  co-author trailer.
- Simplified Technical English in prose and comments.
- Do not edit another agent's folder. The assignment table is the contract.

## Assignment

| agent | rows | folders owned | also owns |
|---|---|---|---|
| core-cutting | CUT-08, CUT-02, CUT-12, CUT-07 (doc) | `dressup/`, `adaptive3d/` | `compute/execute/dressup_apply.rs` import line only |
| core-edges | EDG-01, EDG-03, EDG-04, EDG-05 | `material/`, `io/`, `geo.rs`, `metrology/`, `machine/`, `mesh.rs` | — |
| core-fields | FLD-03, FLD-04, FLD-05 | `trace/`, `maps/`, `surface/` | — |
| core-stock | STK-08, STK-04, STK-05 | `stock/`, `dexel_stock/` | `rs_cam_viz/src/app/mcp/simulation.rs` (the one field site), `crates/rs_cam_core/tests/perf_golden_sim_metrics.rs` |
| viz-ui | UI-08, UI-13 | `rs_cam_viz/src/ui/` | `rs_cam_viz/src/app.rs` (the close-guard site) |
| cli-mcp | CLI-05, CLI-07 | `crates/rs_cam_cli/` | — |
| core-finish | FIN-01 | `finish/` | `crates/rs_cam_core/Cargo.toml` (the feature and `required-features` rows), the six research test targets |
