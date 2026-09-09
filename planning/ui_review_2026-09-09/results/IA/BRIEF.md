# Source-led UI/IA pass — execution brief

Started 2026-09-09 18:54 +12; source HEAD 4af7dd96 plus working tree.
Owner: Astra. Four bounded read-only supporting tracks, one synthesis owner.

## Purpose

Reconstruct how the app is arranged and how a person moves through it. This is
NOT a repeat of the generator/safety defect sweep. For every design judgment ask:
**if the known engine and wiring defects were fixed, would this workflow still
be understandable?** Record correctness prerequisites separately from IA changes.

Study actual UI construction → state/selection → AppEvent → handler → feedback.
Do not stop at a wrapper to infer behaviour that may happen in a called session
method. Source explains conditions and transitions; images establish appearance;
neither establishes unaided human comprehension or measured discoverability.

Existing W00/W01/W02/R03 findings are evidence and investigation leads. Old
`planning/ui_audit/` maps/design targets are historical, not the current interface
or a design mandate. Map the current interface before considering old proposals.

## Constraints

- No application/config/test/library/user-project edits, builds, tests, commits,
  process kills or changes to the GUI. Other teams are actively developing.
- All review artifacts stay under `planning/ui_review_2026-09-09/results/IA/`.
- Supporting workers have only read/grep/find/ls tools, no shell, MCP or network.
  The orchestrator saves their outputs. No credentials/configuration inspection.
- No fabricated clicks, timings, novice reactions, widget counts or live results.
- Treat source changes during the pass explicitly; pin evidence to paths/lines
  and record the checkout. Do not claim it matches earlier 8a4df241-dirty screenshots.
- No need to wait for human testing to propose an expert IA design. Label its
  assumptions and turn the most important ones into later validation tasks.

## Four disjoint tracks

1. **T1 Navigation/context:** shell, workspaces, menus, selections, inspector
   dispatch, empty states, camera/context retention and return routes.
2. **T2 Authoring/hierarchy:** toolpath forms, primary/advanced decisions, units,
   editable/current/recommended roles, apply semantics and parameter grouping.
3. **T3 Feedback/recovery:** visible state lifecycle, notifications, diagnostics
   priority, selected/playing/focused scope, issue→correction→refreshed evidence.
4. **T4 Expert workflows:** planner/library/dependencies/export, persistent versus
   temporary ownership, command placement and expert paths through the same app.

Each track returns:
- Current-state capability/home/visibility/action/effect/feedback map.
- Three concrete source-traced interaction sequences.
- At most four structural IA problems that remain after correctness fixes.
- Small proposed changes with alternatives, explicit expert-preservation rules,
  and acceptance journeys. Distinguish proposals from current behaviour.
- Known defect prerequisites, uncertainties and screenshot/human checks to request.

## Astra's synthesis outputs

`SUMMARY.md`: short executive answer and ranked design backlog, separate from
engineering defects. `CURRENT_MAP.md`: unified current journeys/ownership/state
transitions. `PROPOSED_FLOW.md`: before/after diagrams and low-cost wireframes.
`VERIFICATION.md`: reviewed worker claims, evidence/source drift and limitations.
No fix agents or implementation work in this pass.
