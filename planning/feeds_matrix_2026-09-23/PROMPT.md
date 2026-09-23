# Prompt for the feeds-matrix session

Paste this as the first message of a new session on Opus 5.5.

---

Read `planning/feeds_matrix_2026-09-23/PLAN.md` in full, then
`crates/rs_cam_core/src/feeds/CLAUDE.md`, `crates/rs_cam_core/src/tool_load/CLAUDE.md`
and `crates/rs_cam_core/src/feeds/INTEGRATION.md`.

Step zero, before any phase: triage the orphaned tree. On 2026-09-23 the
checkout carried about forty uncommitted files from a session that was
cut off: the bandless feed-modulation fix
(`planning/wanaka200_feeds_check_2026-09-19/IMPLEMENTATION_PLAN.md` work
item A), its sentry `tests/plunge_guard_bandless_p3.rs`, the wanaka200
docs, `load_model_2026-09-16/MACHINIST_REFERENCE_CHECK.md`, and unrelated
viz edits (setup deletion modal, MCP lifecycle). Build core and viz
through the lane, run the sentries that plan names (`plunge_guard_p3`,
`dressup_span_invariants`, `constrained_max_modulation_f039`,
`lead_in_out_feed_rates_f040`, `plunge_guard_bandless_p3`) plus the viz
tests the modified viz files name, and commit what is green by explicit
path in small commits that name the origin. Commit the planning docs as
they are. Report what stays red and leave it in the tree. If the operator
says the other session is still alive, skip this step and read those
files without editing them.

Phase 2 is the one phase to run as a workflow (say "use a workflow"): one
research agent per firing cell class, a verifier per finding that opens
the URL and confirms the table row, one reconciler that writes
`EVIDENCE.md`. Every other phase is ordinary orchestration.

Run the plan as a workflow of read-only agents first and editors last.
The rule for the whole programme: **no fix lands before Phase 3's
rulings.** An agent that finds a defect records it in `EVIDENCE.md` with
the cell, the number, the published figure and the URL. It does not
change a number to close a gap.

Phase 0 (one editor, small): the `FeedsSupport` declaration of PLAN §5.
One enum, one `OperationSpec` field, one `FeedsError::Unbacked` variant,
three doors surface it (GUI Suggest, MCP `apply_feeds`, CLI). Sentry:
every `OperationType × ToolType` has an arm; no cell with a vendor row
resolves to `Refuse`. Commit by explicit path through `scripts/cargo_lane.sh`.

Phase 1 (one editor): the matrix instrument of PLAN §4. A core
integration test behind `#[ignore]` that walks `ToolType × OperationType ×
{hardwood, softwood, mdf, plywood_hardwood}`, calls `suggest_params`, the
static checks and, for the subset in §6, a short simulation, and writes
`matrix_<date>.csv` plus a summary. It adds no arithmetic. Run it once;
commit the CSV.

Phase 2 (two or three read-only research agents in parallel, then one
that reconciles): for every cell that refuses, ships formula-only, or
fires a `feeds.*`/`geom.*`/`load.*` id, find the published row or rule it
should agree with. Start from `load_model_2026-09-16/MACHINIST_REFERENCE_CHECK.md`
and `data/vendor_lut/source_manifest.json`; go to the open web only for
cells those do not cover, and record every URL and access date. Run the
two worked examples in PLAN §6 first. Output: `EVIDENCE.md`, one row per
cell, verdict per row.

Phase 3: stop and ask the operator for the five rulings in PLAN §7, with
the evidence rows that bear on each. Give a recommendation per ruling.

Phase 4 (editors, after the rulings): fixes inside the rulings only. Each
fix names the cells it moves and re-blesses the matrix sentry with the
cause. Update `CREDITS.md` and the source manifest for any new source.
Update `FEATURE_CATALOG.md` where a visible surface changes.

Constraints that hold throughout: every cargo command through
`scripts/cargo_lane.sh`; sentries and focused suites only, no heavy gate,
ask before a run over three minutes; stage by explicit file path, never a
directory, `git diff --cached --name-only` before every commit; do not
touch `.mcp.json`, `.pi/`, or the peer's uncommitted files; Simplified
Technical English in prose; commits end with the attribution line the
session reminder gives.

Report at the end of each phase: what was measured, what was found, what
is blocked on a ruling. Nothing is pushed until the operator says so.
