# Architecture consolidation — status tracker

**Read this before `PLAN.md`.** The audit was taken at `627ef997` on
2026-09-09. Work landed after that date closes part of it. An account that
runs the plan without reading this row set will re-do finished work.

> **Protocol** (same as the June plan): the orchestrator updates THIS table
> only. The body of `AUDIT.md` and `PLAN.md` stays verbatim as produced.
> States: `TODO` / `IN PROGRESS (who)` / `PARTLY CLOSED` / `BLOCKED (on what)` /
> `DONE (commit)`. Never delete a row; mark it `DROPPED (why)`.

## Provenance and one prior mistake to avoid

The pi command `/techdebt-orchestrate` pointed at
`planning/architectural_refactor_2026-06-06_v2.md` until 2026-09-10. **That
plan is COMPLETE** — every work item T0..T16 landed 2026-06-07, verified by
content and not by its tracker (`for_each_op!`, `OpRegistryEntry`,
`CutterKind`, `ExecutionContext`, `GateEnv`, `CutterOpProfile`,
`parse_lenient`, `dressup_policy` all present in the tree). It is also a CORE
refactor, not a GUI one. An account pointed at it would have found nothing to
do. The command now points here.

## Phase status

| Phase | Subject | Status | Note |
|---|---|---|---|
| 0 | Contracts and executable evidence | TODO | The characterization tests it asks for partly exist already — 30+ sentries were added by the UI programme and the core track. Inventory before writing new ones. |
| 1A | One mutation contract | **PARTLY CLOSED** | See audit finding 2 below. |
| 1B | One artifact owner | TODO | See audit finding 3 below. Untouched structurally. |
| 1C | Migrate consumers | TODO | |
| 2 | Centralize export planning | TODO | |
| 3 | Unify the full generation pipeline | TODO | Audit finding 1. The largest phase. |
| 4A | Canonical model import | **PARTLY CLOSED** | See audit finding 6 below. |
| 4B | Canonical parameter contracts | **PARTLY CLOSED** | G-SCHEMAENUM (`df5c27d3`) fixed six advertised enum values no config could hold, and added a generic sentry that round-trips every advertised value. The deeper finding — that the `enum:a\|b\|c` schema string is documentation and nothing validates it — STANDS. |
| 5A | Resolve drilling once | TODO | Audit finding 8. Note F4.8 (`cc85abce`) changed the drill pick path; re-read before planning. |
| 5B | Shared annotated run emission | TODO | |
| 5C | Structured offset outcomes | TODO | |
| 6A | One timing view | TODO | Audit finding 5. Note G-AIRDENOM already rebased cutting times onto one clock; the audit's point about SELECTED summaries still stands (`SimulationSemanticCutSummary`, `SimulationCutHotspot`, `KinematicsSummary::cutting_runtime_s` are still mixed-base). |
| 6B | Explicit vendor lookup use | TODO | |
| 7 | Cache mechanics | TODO | |
| 8 | Whole-system acceptance | TODO | |

## What landed after the audit, per finding

**Finding 2 — invalidation is a convention, not an enforced contract.
PARTLY CLOSED.** G-FRESHSTATE (operator ruling, 2026-09-10) made every stock
edit stale every toolpath on both the GUI and MCP routes, put `tool_id` and
`model_id` into `generation_inputs_signature`, and made a GUI inspector edit
drop the core result. F2.12 (`8cb8aa1d`) gave the holder-clearance verdict the
same edit-counter stamp every other readiness row already had. F2.13
(`cc85abce`'s parent merge) made that verdict declare how much of the job it
examined. F4.7 made `rescale_model` invalidate.
**STILL OPEN, and it is the audit's actual point:** `replace_toolpath_config`
and `apply_toolpath_param_snapshot` still remove only that operation's result;
undo/redo and optimizer application still use the snapshot method; public
mutable accessors still require callers to remember. The transactional
mutation boundary does not exist. F2.5 independently found undo bypassing
`invalidate_tool` in both directions, which is the same defect class.

**Finding 3 — computed results have competing owners. NOT CLOSED.** Results
still live in both `session.results` and `gui.toolpath_rt`. The narration
wording the audit cites as a symptom is still in `CLAUDE.md`. Nothing this
session did changes the ownership split.

**Finding 6 — model import duplicated below the convergence point. PARTLY
CLOSED, AND ONE CLAIM NEEDS RE-CHECKING.** F4.4 (`957f051f`) unified the three
GUI doors that REFRESH a model record in place (`rescale_model`,
`reload_model`, `relink_model`) into `LoadedModel::adopt_geometry`, with an
exhaustive destructure so a new field cannot be silently skipped. That is a
different pair from the one the audit names. **The audit's own pair —
`core/io.rs:25` interactive import versus `core/session/project_file.rs:569`
project load — is NOT consolidated.**
**UNVERIFIED CLAIM TO CHECK FIRST:** the audit states "Interactive STEP loading
applies the requested scale. Project STEP loading does not apply its computed
scale." If true that is a live defect of the G-UNITSRELOAD class, which was the
same pair diverging on SVG/DXF. It was NOT verified by this session. Verify
before scoping Phase 4A.

## Related open items

`planning/ui_fix_2026-09-09/PLAN.md` §11 carries twelve follow-ons opened on
2026-09-10, several of which sit inside this plan's phases: F2.14 (widen the
holder check), F4.10 / F4.11 / F4.12 (drill pick coverage), J8.1 (MCP drops a
pinned Top Z), J8.4 (a diagnostic-id mirror that became a parallel copy).
Read that list before opening new work packages here.
