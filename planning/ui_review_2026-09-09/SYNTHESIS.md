# Synthesis brief — turn the reviews into one improvement programme

Owner: **Astra**. Run after the normal-path checkpoint, then update after the
expert/interaction reviews. This is a design/review task, not implementation.
Read PLAN, PROTOCOL, all completed reports and their evidence limitations.

## 1. Do not merge by concatenation

Build one journey map spanning import, setup, authoring, verification, correction,
export and return-to-work. Place every finding at the decision/transition it
changes. Track blocked/untested coverage separately from observed difficulty.

Merge findings by root cause and affected state, retaining original IDs and
screenshots. A stale result in a graph, readiness banner and export modal may
be one state-lifetime issue with three consequences. Similar-looking symptoms
can also have different producers: prove the shared cause before consolidating.

Separate:

- behaviour/wiring defects;
- incorrect or excessive confidence;
- task flow, navigation and control hierarchy;
- terminology and guidance;
- unsupported capability and its explanation;
- missing evidence/tooling, which may prevent a reliable recommendation.

## 2. Rank decisions, not screenshots

Safety-relevant misleading confidence and data-loss risks receive immediate
triage, but distinguish confirmed from hypothetical cases. Then prioritize
blocked supported workflows, frequent avoidable detours and local friction.

For each candidate change, state:

- affected task(s) and user lens;
- observed frequency in the sessions (not invented market prevalence);
- error/consequence or effort removed;
- confidence and missing validation;
- dependencies and likely implementation scope;
- expert capability/shortcut retained and any new cost;
- a repeatable acceptance task.

Do not mechanically multiply ordinal severity/effort numbers into a spurious
ROI score. Publish a ranked top ten **across the programme**, not ten per panel.
Preserve a clear “keep” list so successful powerful tools are not collateral damage.

## 3. Resolve design tradeoffs with task evidence

Explicitly consider, without prejudging:

- guided starting path versus free-order expert work;
- contextual primary actions versus a canonical menu/home;
- progressive disclosure versus hiding essential caveats;
- automatic recomputation versus predictability and resource control;
- inherited/default settings versus explicit scope and overrides;
- summary verdicts versus explainable evidence and uncertainty;
- simple manual authoring versus planner proposals and ownership;
- fast direct export versus equivalent, understandable verification.

Where two reports recommend incompatible solutions, compare them on the same
baseline task. Prefer a small shared interaction rule over nine one-off fixes.
Do not require “one concern, one home” to mean “only one way to launch a command.”

## 4. Validate before implementation

Produce low-cost sketches for the highest-leverage flow changes, not a polished
mockup of every panel. Use the same goal cards from the baseline:

1. New-to-rs_cam user imports an SVG and authors a held pocket/profile job.
2. User prepares a small 3D cut and explains tool/detail tradeoffs.
3. User interprets a consequential warning and an unknown state, corrects the
   relevant input and finds refreshed evidence.
4. User returns to an edited/reopened job and explains what remains current.
5. Expert revises a dependent multi-tool plan and exports the intended setup.

Ask for predictions and teach-back, not “do you like this screen?”. Log assistance,
wrong turns, active effort and confidence mistakes. Ideally include a CAM-literate
person unfamiliar with this app alongside Ricky. If only expert review is
available, label validation provisional rather than manufacturing novice results.

Compare proposed and existing routes on the same outcome. Improvements must not
trade fewer clicks for an incorrect default, weaker warning or lost override.

## 5. Output

Write `results/SYNTHESIS.md` with:

- an executive summary and observed end-to-end journey map;
- coverage ledger, participant/tool limitations and known-issue reconciliation;
- consolidated root causes with source finding IDs;
- ranked top ten and strengths to preserve;
- proposed normal and expert routes, including correction/reopen loops;
- design decisions, alternatives rejected and reasons;
- implementation tranches and dependencies;
- acceptance journeys and remaining human-validation questions.

Proposed tranche types, assigned only once findings exist:

1. **Truth and working controls:** fix misleading/no-effect behaviour and
   incorrect state or claims before cosmetic changes.
2. **Normal-path flow:** next actions, selection/scope continuity, parameter
   hierarchy and error-to-fix routes.
3. **Expert acceleration:** chain/planner/comparison/library work and reusable
   shortcuts without obscuring consequences.
4. **Interaction polish and regression:** visuals, keyboard/focus/window sizes,
   targeted UI automation and durable task-based sentries.

Estimate concrete changes after inspecting their implementation paths. Do not
promise a “one-day” fix because its label is short. If source changes follow,
use the repository's core → controller/worker → GUI architecture, impact tools,
focused tests and documented verification gates. Those changes require a separate
implementation request; this review programme commits nothing.
