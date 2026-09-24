# Prompt for the next extrapolation session (after B4, 2026-09-25)

Paste this as the first message of a new session.

---

You continue the rs_cam extrapolation programme as the orchestrator
("rs-cam-e"). Read, in this order:

1. `planning/extrapolation_2026-09-24/PLAN.md` (the programme) and
   `RULINGS.md` (all rulings blocks, the "Landed after the rulings" list,
   the hand-offs, the ramp-feed approval, work item G10).
2. The landing records, §5 of `EXTRAPOLATION_G1.md`, `_G2`, `_G3`, `_G4`,
   `_G5`, `_G6`, for what the code does now. The per-package plans with the
   orchestrator's decisions: `P1_PLAN.md`, `P2_PLAN.md`, `A2_PLAN.md`,
   `A3_PLAN.md`, `B5_PLAN.md`, `B4_PLAN.md`.
3. `crates/rs_cam_core/src/feeds/CLAUDE.md` (the sentries) and
   `crates/rs_cam_core/src/feeds/extrapolation.rs` with its children
   (`size.rs`, `hardness.rs`, `family.rs`, `drill.rs`): the claim design
   every new group follows.

State at hand-off (master 3ec48005 and after): refusals on the FM1 matrix
512 -> 418; every shipping cell states its basis on the Feeds card and in
MCP `basis`. Nothing is pushed.

Work, in order:

1. **The ramp-feed Suggest write.** The field `ramp_feed_rate:
   Option<f64>` is on master (9887735d, by the compute session; None = the
   plunge rate). Approved design (RULINGS, "Open proposal ... ramp feed"):
   ramp feed = min(cutting feed, axial_chip x rpm x Z / tan(ramp angle)),
   the axial chip from the G6 drill claim (flat end mill, 3.175-6.0 mm);
   elsewhere None (the plunge rate) with a card note. Today
   `FeedsResult::ramp_feed_mm_min` is an unsourced clamp in feeds::calculate
   step 8: replace it. Suggest's apply funnel writes the op's
   `ramp_feed_rate`. The entry code uses it for helix and ramp descents
   through material. Measured by the compute session on rivmap100:
   1090 s -> 734 s with the ramp feed alone.
2. **G10, entry parameters.** Its Phases 0-2 run in another account's
   session (`PROMPT_G10.md`); it sends a summary to "rs-cam-e" when done.
   Then Phases 3-5 here: the operator's rulings, the claims, the card.
3. **B7**: replace the 0.88 / 0.75 long-tool share with the deflection
   model. The deflection envelope must first cover the 2D operations
   (182 of 232 roughing cells have none). `EXTRAPOLATION_G8.md` §3.1.
4. **B6**: Kc per material (Curti density law for solid wood, Goli 2018
   for MDF; refuse a power figure for plywood, particleboard, plastics).
   `EXTRAPOLATION_G7.md` §3.
5. **The Optimize resolution gap**: `tool_load/optimize/outcome.rs` uses a
   per-request auto resolution; the project stores one (G-RESTRES).
6. Clean-ups recorded in B4_PLAN (Q4-Q7), B5 (other Spektra sizes), the
   bull literature cells (see below).

The operator owes three answers; ask them early:
- **B3**: may you run the simulation witness (over three minutes) on
  ROUGHED stock (`claims_reference` = the machined stock) before the 42
  ball-nose finish cells on repo-derived rows (0.13-0.21 of the printed
  band) and the 6 ball-nose MDF cells move?
- **The bull literature cells** (`bull_6mm_adaptive2d_oak`,
  `bull_6mm_pocket_oak`, `bull_12mm_pocket_oak`): banded on flat-end
  charts; re-band from the printed Amana corner-radius chart
  (recommended) or keep the flat proxy?
- **Hardwood V-bit** formula-only cells (Face, Pocket, Profile, Rest,
  Zigzag, ProjectCurve): the formula sits under 0.5x of the Onsrud band;
  refuse?

How to work (all learned the hard way):
- You orchestrate; editor agents write code and get "no cargo". You run
  every build and test through `scripts/cargo_lane.sh`, one job at a time.
  A planner agent (Plan type) for a large package; brief an editor
  directly for a small one. Sonnet editors for mechanical steps save
  tokens.
- After ANY change to a row, a band or a scale, also run
  `adaptive_feed_modulation_pipeline_f036b`, `arc_fit_disposition_a5`,
  `a_rescaled_feed_stays_inside_the_power_ceiling_g_t15`,
  `literature_matrix` and `wanaka_suggest_integration` (it takes seconds
  once built), plus the sentries listed in `feeds/CLAUDE.md`, the core
  `--lib`, the viz Feeds-card tests, workspace clippy and fmt. Then re-run
  FM1 (`--test feeds_matrix_instrument_fm1 -- --ignored`, about 150 s),
  diff the CSV against HEAD, and commit it with the change.
- Predict RPM moves, not only chip moves: a row that prints an RPM
  displacing one that does not changes the feed by the RPM ratio (P2
  lesson).
- In zsh, split a test list into an array: `T+=(--test $t)`, then
  `"${T[@]}"`. `cp` is aliased to `cp -i`: use `/bin/cp -f`.
- Never share `CARGO_TARGET_DIR` between worktrees (cargo links a stale
  rlib). Verify a peer's half-built tree in a detached worktree with its
  own target.
- Another session ("rs-cam-e2", ListAgents) owns session/*, adaptive3d/*,
  compute/*, dressup/entry_descent.rs and does release builds for the
  operator. A third session may run G10 research in planning/. Commit only
  your own files by explicit path; `git diff --cached --name-only` before
  every commit; never `git add -A`, `git stash`, or `git reset`.
- Do not touch `.mcp.json` or `.pi/`. Simplified Technical English in
  prose. Commits end with the Co-Authored-By line for the model you run on.
- The operator's standing rules: no invisible calculation or de-rate (every
  scale on the card); a claim ships only with its evidence and range, and
  refuses outside it; no large test gates; ask before a run over three
  minutes; no legacy shims (breaking changes are fine, state them).
