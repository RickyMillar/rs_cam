# Prompt for the B6 session (Kc per material, code package)

Paste this as the first message of a new session.

---

You implement package **B6 (G7, material physics: Kc per material)** of the
rs_cam extrapolation programme, from plan to commit. The orchestrator
session is "rs-cam-2f" (ListAgents). It now works on G10 in
`crates/rs_cam_core/src/feeds/` (mod.rs step 8, ramp.rs, a new plunge.rs,
provenance.rs, rationale.rs, suggest/*) and in the viz Feeds card. A
second session, "rs-cam-e2", owns compute/*, session/*, adaptive3d/* and
dressup/*.

Read first:
- `planning/extrapolation_2026-09-24/RULINGS.md` §B6 (the ruling): solid
  wood takes the Curti density law, MDF takes Goli 2018, and plywood,
  particleboard and plastics refuse a power figure. The force anchor
  (`LIT_KS`, `LIT_FEDGE`) is an MDF fit applied to hardwood; it goes under
  either form.
- `planning/extrapolation_2026-09-24/EXTRAPOLATION_G7.md` (all of it; §3 is
  the Phase 3 input) and `fetch/G7/`.
- `PLAN.md` §4 (the claim design) and one landed package as the model:
  `B5_PLAN.md` with `EXTRAPOLATION_G6.md` §5, or `P2_PLAN.md` with
  `EXTRAPOLATION_G2.md` §5.
- `crates/rs_cam_core/src/feeds/force.rs`, `feeds/efficiency.rs`,
  `material/mod.rs` (Kc, Janka, density), the power ladder in
  `feeds/mod.rs` (step 6) and `feeds/suggest/invariants.rs` (pass 10), and
  `crates/rs_cam_core/src/feeds/CLAUDE.md` (the sentries).

Do:
1. Write `planning/extrapolation_2026-09-24/B6_PLAN.md` with a Plan agent:
   the forms, the files, what refuses, the card and MCP lines, the tests,
   and a prediction of the FM1 moves (power_kw, the power-ladder cells,
   refusals). Put your decisions in it. Ask the operator only a question
   that the ruling does not answer.
2. Implement it with editor agents that get "no cargo". You run every
   build and test yourself.
3. Verify: the B6 sentry you add (name style `a_..._b6`), the sentries in
   `feeds/CLAUDE.md`, `adaptive_feed_modulation_pipeline_f036b`,
   `arc_fit_disposition_a5`,
   `a_rescaled_feed_stays_inside_the_power_ceiling_g_t15`,
   `literature_matrix`, `wanaka_suggest_integration`, the core `--lib`,
   the viz Feeds-card tests, workspace clippy and fmt. Then run FM1
   (`--test feeds_matrix_instrument_fm1 -- --ignored`, about 150 s) and
   diff the CSV against HEAD.
4. Write a §5 landing record in `EXTRAPOLATION_G7.md`.

Rules (all learned the hard way):
- Run every cargo command through `scripts/cargo_lane.sh <args>`, one job
  at a time. Another two sessions build on this machine. Never share
  `CARGO_TARGET_DIR` between worktrees.
- No large test gates (no full core gate, no whole-workspace `cargo
  test`). Ask the operator before any run over three minutes.
- **The FM1 CSVs (`planning/feeds_matrix_2026-09-23/*`) are shared with
  rs-cam-2f.** Before you commit them, `git pull` is not used here: run
  `git log -1 -- planning/feeds_matrix_2026-09-23/` and, if rs-cam-2f
  committed them after your FM1 run, re-run FM1 on the current tree. Tell
  rs-cam-2f (SendMessage) before you start FM1 and after you commit.
- Tell rs-cam-2f before you edit `feeds/mod.rs`, `feeds/provenance.rs`,
  `feeds/rationale.rs`, `feeds/suggest/*` or `rs_cam_viz/src/ui/feeds/why.rs`
  (it has G10 editors in those files); edit disjoint regions only.
- Commit only your own files, by explicit path; run
  `git diff --cached --name-only` before every commit. Never `git add -A`,
  `git stash` or `git reset`. Do not touch `.mcp.json` or `.pi/`.
- Simplified Technical English in prose. Commits end with the
  Co-Authored-By line for the model you run on.
- The operator's standing rules: no invisible calculation (every scale and
  rule on the Feeds card and in MCP `basis`); a claim ships only with its
  evidence and range and refuses outside it; no legacy shims (breaking
  changes are fine, state them). Nothing is pushed.
- Keep token use small: one planner, one or two editors, no workflow.
- When done, send rs-cam-2f a short summary: commits, FM1 moves, open
  questions.
