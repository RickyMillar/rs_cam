# UI/IA review — read this first

**Recommendation: keep the four workspaces. Improve the decisions and handoffs
inside and between them; do not start a whole-shell redesign.**

This pass reviewed UI construction, selection, events, feedback and ownership at
source HEAD **4af7dd96**, with four read-only supporting tracks and Astra's
verification/synthesis. It uses existing screenshots as earlier-build evidence.
No new live runs, code/config edits, builds, tests or fix agents.

## What we learned about UX, beyond the bugs

The app has useful work areas, purposeful controls and deep expert capability.
The main problem is that the operator must assemble the workflow themselves:

- **Workspace is not editing context.** Switching to Toolpaths can leave a setup
  inspector open; selecting an op in Simulation moves playback but does not
  select that op for editing. The context is retained, but its meaning changes.
- **Forms expose parameters better than decisions.** Through-cut holding choices
  can be collapsed while less consequential tuning stays visible. Complex
  operations offer many fields without a strong “decide this first” hierarchy.
- **A value has several roles.** Current plan, live recommendation, inherited
  default and emitted feed behaviour need clear separation. Fixing a mislabeled
  number or uncapped Apply does not by itself make their relationship obvious.
- **Diagnosis does not reliably lead back to correction.** Re-run, Jump, graphs
  and Optimize exist. What is missing is a named issue → correct operation/tab →
  recheck → matching fresh evidence route for manual changes.
- **Dialog shape does not predict commitment.** Tool drafts, planner previews,
  catalog edits and export settings differ in what Apply, Close or navigation
  retains. These differences need visible ownership and consequences.
- **Export entry points imply different journeys.** Readiness's Export opens
  preflight; File's Export opens the wizard. Keep the expert shortcut, but make
  scope, checks and settings understandable independently of entry point.

These problems remain even if every known generator/cache/cap defect is fixed.
They are source-backed expert design judgments; user confusion or prevalence has
not been measured.

## Ranked design backlog

Ranks are design priorities, **not machining-risk severity scores**.

| Rank | Change | What “done” means | Likely scope |
|---|---|---|---|
| 1 — D2 | Named actionable findings + scoped Edit/recheck/return | Operator resolves a finding without memorizing its op/move or searching again after regeneration | Cross-workspace interaction/state |
| 2 — D4 | Current plan versus proposed change, explicit Apply scope | Operator predicts changed fields and keeps an intentional manual stepover while accepting speeds | Shared value/apply presentation; correctness prerequisites |
| 3 — D3 | Decision-first operation forms | Target, depth/quality intent and holding/leave consequences are readable without opening every section | Operation-family layouts and contextual summaries |
| 4 — D1 | Persistent context + partial-job next actions | User knows which setup/input/tool/op an action affects, including after workspace changes | Shell/selection guidance; preserve expert context |
| 5 — D5 | Preview/Apply/Close ownership + before-Apply change summary | User names replaced/retained objects and persistence before committing | Planner/library/dialog contracts |
| 6 — D6 | Common export handoff summary, with quick/configured routes | Same intended scope and applicable checks are clear from either export entry | Cross-route presentation and check parity |

Small consistency repairs belong within these, not in a separate redesign:
wrap essential warnings; replace raw “Annotation” counts with named subjects;
include Readiness in the Workspace menu; update stale “project tree” wording.

## Show me, rather than more reports

- [Annotated authoring screen](annotated/01_authoring.png)
- [Annotated current/proposed values screen](annotated/02_values.png)
- [Annotated diagnosis/recovery screen](annotated/03_recovery.png)
- [Proposed journeys and rough wireframes](PROPOSED_FLOW.md)

Annotations are review suggestions on existing screenshots, not implemented UI.
The detailed [current interaction map](CURRENT_MAP.md) and
[verification notes](VERIFICATION.md) are available for whoever picks up the work;
you do not need to read the four raw worker reports.

## Keep

Four workspaces; direct/contextual shortcuts; purpose lines and Tool/Input controls;
reference-based Heights; explicit validation; working stale/Re-run cues; separate
load criteria; span lock and playback; optional deep graphs/vendor evidence;
planner Preview; library snapshots; per-field speed controls and quick export.
No forced wizard, universal Fine slider, fake safety score or blanket Advanced
bucket hiding targets/workholding/limitations.

## Separate engineering prerequisites

Earlier R03/W01/W03 reports contain ramp containment, raw recommendation/cap,
target fallback, stale/export/invalidation and persistence findings. They belong
in a **separate verified defect backlog**, not diluted into visual polish.
Their current fix status must be checked against the changing code/build before
assignment. This pass did not reproduce or re-certify them. In particular, do not
promise reliable single-field Apply, Undo plan, persistence or gate parity until
the underlying paths actually support those promises.

## What next

Approve/refine these six interaction decisions, then turn them into bounded design
and implementation tasks with the acceptance journeys in PROPOSED_FLOW. A short
walkthrough of the sketches can test the uncertain assumptions; we do **not** need
another broad MCP sweep first. Event-based automation helps functional coverage,
not natural discoverability; real input tests hit targets/focus/drag, and people
validate comprehension. None of that blocks this expert IA assessment.

**This pass is complete. No support agents remain running. No fixes were launched.**
