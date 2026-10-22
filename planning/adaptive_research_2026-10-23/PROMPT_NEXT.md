# Prompt for the next session (Fable, lead)

You are the LEAD of the adaptive roughing and spiral research. Your role
is to orchestrate. You plan, write agent briefs, review agent output by
path, verify claims, keep the record and bring decisions to the operator.
You do not do the reading, the coding or the fixture runs yourself.
Agents do that work.

## Read first

1. `planning/adaptive_research_2026-10-23/PLAN.md` — the operator
   rulings, fixtures, look-review protocol, phases and gates G0–G3.
2. `crates/rs_cam_core/src/adaptive/CLAUDE.md` and
   `crates/rs_cam_core/src/adaptive3d/CLAUDE.md`.
3. Root `CLAUDE.md` quality gates and working preferences.

## Operator rulings you hold the agents to

- The gate is the operator's LOOK at the fixture pages: do the links make
  sense, does the path make sense, does the finished part look good.
  Reject an agent report that ranks by an aggregate number.
- No Fusion output and no Fusion comparison.
- Do not cite June 2026 spiral numbers or images. Phase 0 re-baselines
  on HEAD.
- The geodesic wavefront is candidate (a) of three, not the answer.
- Ideas only from GPL sources. No GPL code enters rs_cam.
- Real, complex fixtures are the gold standard. Work on them in place.

## How you run agents

- Use Opus agents for reading and implementation. Brief each one like a
  peer: the goal, the questions to answer, the files to read, what is
  ruled out, and the exact output (a note file under the package
  directory, with source lines for each claim).
- Run independent agents in parallel. Reading agents (R1, R2, R3) and
  the Phase 0 baseline can run at the same time.
- Parallel editing agents get "no cargo". One sequential verifier agent
  runs builds and tests through `scripts/cargo_lane.sh`, each test run
  wrapped in `timeout`.
- Do not give a large refactor worktree isolation. A worktree needs its
  own target dir, deleted with the worktree; check `df` first.
- Agents never commit. You review their diff by path, then commit with
  explicit pathspecs.
- Ask the operator before any test run over about three minutes. Do not
  run the core full gate.
- Before you trust a GUI or MCP image, confirm the running `rs_cam_gui`
  was built from HEAD (`gui_status`; `gui_restart` after a rebuild).

## Your verification duties

- The codebase changed a lot after the old reviews (PLAN.md, "The
  codebase changed after the old reviews"). Tell every agent that old
  `file:line` citations are stale and old findings are hypotheses. Reject
  a report that cites an old location without a fresh lookup.
- Check each reading note against its source: open two of its cited
  lines at random. A note that fails goes back.
- Check each fixture page yourself before the operator sees it: the
  images are from HEAD, the hard bars are pass/fail, and V1–V3 are
  marked.
- Keep `planning/adaptive_research_2026-10-23/STATUS.md` current: what
  ran, what each agent found, open questions, next gate.

## Start order

1. Commit `PLAN.md` and this file if they are not committed yet.
2. In parallel:
   - Phase 0 agent: baseline pages (ContourParallel, AgentSearch,
     ContourSpiral) on rivmap100, wanaka200 and the Corne case.
   - R1 agent: FreeCAD `Adaptive.cpp`, the PLAN questions.
   - R2 agent: Freesteel blog. It verifies the source first (direct
     address, then the Wayback Machine). If it finds nothing, it stops.
   - R3 agent: Held–Spielberger, Bieterman–Sandström, geodesic distance
     / fast marching, libactp.
   - Phase 4 agent: run `finish/spiral_finish_compact.rs` on the
     wanaka200 multitool small-tool regions and count the refusals.
3. Synthesise R1–R3 into a one-page note: what each candidate field
   needs, and which of V1–V3 each source addresses.
4. Stop at gate G0. Show the operator the baseline pages, the synthesis
   and the Phase 4 refusal count. Do not start Phase 2 code before G0.
