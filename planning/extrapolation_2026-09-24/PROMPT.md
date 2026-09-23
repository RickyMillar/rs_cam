# Prompt for the extrapolation session

Paste this as the first message of a new session.

---

Read `planning/extrapolation_2026-09-24/PLAN.md` in full, then
`planning/feeds_matrix_2026-09-23/RULINGS.md` (every round),
`planning/feeds_matrix_2026-09-23/FORMULA_BACKING_v2.md`,
`crates/rs_cam_core/src/feeds/CLAUDE.md` and
`crates/rs_cam_core/src/feeds/support.rs`.

The question of the whole session is the operator's: "How can we make a
reasonable claim, to fill gaps our data does not, within reasonable limits,
for each circumstance we need to fill?" Answer it group by group (PLAN §2),
with a trend, an equation with a range, a second witness where one exists,
and an implementation only after the operator's ruling (PLAN §3–§5).

Rules for the session:

- Every cargo command through `scripts/cargo_lane.sh`; one job at a time;
  sentries and focused suites only; no heavy gate; ask before a run over
  three minutes. The FM1 instrument (`--test feeds_matrix_instrument_fm1 --
  --ignored`) takes about 150 s and is allowed.
- Editors get "no cargo"; the orchestrator verifies. In zsh, split a path
  list with `${=LIST}`. Stage by explicit path; `git diff --cached
  --name-only` before every commit. Do not touch `.mcp.json` or `.pi/`.
- Simplified Technical English in prose; commits end with
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Nothing is
  pushed until the operator says so.
- No fix lands before Phase 4's rulings. A fit is a document until then.
- Every claim that ships is on the Feeds card with its rule, range and
  residual (the operator's rule: no invisible calculation or de-rate).
- Phase 1 (fetch) and Phase 2 (trends) run as a workflow (say "use a
  workflow"): one research agent per group, URL verifiers per finding, one
  reconciler per group file. Everything else is ordinary orchestration.

Deliverables: `EXTRAPOLATION_<group>.md` per group (G1–G8), a `RULINGS.md`
for the session, the `Extrapolation` trait and its arms with sentries, the
matrix CSV per landing, `FEATURE_CATALOG.md` and the memory note. The
acceptance case for G1 is the wanaka "3D Finish 6" pass (a 1 mm tip tapered
ball) getting a recipe with a stated range instead of a refusal.
