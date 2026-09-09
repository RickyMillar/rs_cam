# Shared review protocol

Applies to R01–R09. Read the assigned brief and [PLAN.md](PLAN.md) first.
This is a review, not permission to implement fixes or operate a CNC machine.

## 1. Evidence before verdicts

Start from an outcome, not instructions naming the correct menu or operation.
At each decision ask:

- What am I trying to achieve?
- What would I try from this screen? Can I see the relevant action?
- What do I expect it to change? How will I know it worked?
- What do I believe is current, selected, verified and saved?
- If it fails, can I understand and recover without reading source code?

Use two passes:

1. **Unaided task:** a human new to the route acts without implementation notes,
   MCP state inspection or coaching. Record the route and their predictions.
2. **Diagnostic pass:** inspect state, code and tool replies to explain what
   happened. Repeat edge cases and capture precise evidence.

An agent that already read the source can do a cognitive walkthrough, but not
claim to be a novice participant. Label it **expert walkthrough**. If no human
or pointer/keyboard tool is available, mark interaction/discoverability outcomes
**untested** and produce a provisional code/screen review instead.

### Evidence labels

| Label | Establishes | Does not establish |
|---|---|---|
| HUMAN | Observed task attempt, prediction, confusion, recovery | General population prevalence from one person |
| DESKTOP | Actual pointer/keyboard/focus/drag interaction in the app | A novice’s natural choice of route |
| MCP-VIEW | Full GUI screenshot after controlled state navigation | Menu discovery, click reach, focus or native-dialog behaviour |
| MCP-STATE | Backend state, metrics or mutation response | GUI availability or identical defaults/side effects |
| CODE / TEST | Implementation path or narrowly exercised invariant | Live usability, rendering or production fixture outcome |
| HISTORICAL | Earlier report or screenshot | Current behaviour |
| HYPOTHESIS / BLOCKED | A proposed risk or missing observation | A verified finding |

Capture the **full window** for context, then crop/annotate if useful. Scene-only
renders supplement it; they are not evidence of panel wording. Read every image
you rely on. Record build, fixture hash, resolution, capture settings, logical
window size/DPI, selection, workspace, operation enabled state and changes made.

Do not infer that a documented GUI capability is live. Trace UI → event/controller
→ session/worker → result → presentation for suspicious controls. Do not convert
a code-path risk into a measured user impact without the intermediate evidence.

## 2. Task states and transitions

Each package covers its happy path plus at least one relevant refusal/error and
one edit/recovery transition. Across the programme, explicitly cover:

- blank, imported-but-unconfigured, partial and valid setup;
- no tool, unsuitable tool, wrong model/selection scope;
- pending, computing, waiting-on-prior-stock, disabled, failed and done;
- current result, stale result, not simulated, metrics not captured/unmeasurable;
- recommended value, manual edit, override, preview, apply, cancel and undo;
- saved/reopened, missing external model and library snapshot;
- long-running, cancelled and retried work;
- partial and multi-setup simulation/export.

Only seed artificial errors in disposable copies. State exactly what was
manufactured and verify the error actually appears. A seed from an old bug is
not automatically a failing fixture in the current build.

## 3. Decision quality and expert capability

For each major decision, record the minimum information needed now, the advanced
information needed on demand, and where each currently lives. Evaluate default
behaviour, explanation, scope and reversibility together.

Keep an explicit list of capabilities and efficient routes that should survive
any simplification. Multiple launch points are not automatically duplication:
a shortcut, contextual action and menu can legitimately invoke the same command.
The problem is conflicting meanings, settings or ownership, not raw button count.

Do not prescribe a wizard, mode split, command palette, renamed shell or hidden
advanced settings in advance. Propose them only against demonstrated task costs
and show what the expert gains/loses. Safety-critical limitations must not depend
on finding an obscure tooltip or advanced disclosure.

## 4. Diagnostic honesty

Read the relevant current caveats in `CLAUDE.md` and `FEATURE_CATALOG.md` before
interpreting metrics. These are reviewer safeguards, not knowledge we may assume
an operator already has. Ask whether the necessary limitation is visible at the
point where the user makes a decision.

Mandatory probes for R04/R06/R07/R09:

- Generated ≠ simulated ≠ measured Within ≠ physical safety certification.
- Absent, not measured, unmodeled, stale, zero and Within are different states.
  Check gate population and scope before treating green as evidence.
- Rapid collisions are profile-aware and resolution-toleranced on stamped ops;
  zero is not proof of no contact. Do not compare counts across cell sizes.
- Advance per tooth is not chip thickness. Vendor, derived, inferred and
  no-data recommendations must not appear equally authoritative.
- Air-cut percentages have different denominators. Keep time basis and operation
  family explicit; prefer absolute air seconds for cross-arm comparisons.
- Kinematic utilization may describe planned or emitted feeds. Check provenance,
  including GUI/MCP narration’s pre-modulation path; do not infer it from timing.
- Reach, remaining stock, rest regions, tier territory and simulated deviation
  answer different questions. Reach has a sampled-grid floor, unmeasured areas,
  an upper-estimate bias and a surface-area basis; it is not achieved finish.
- A displayed deviation colour is not the unaveraged metrology instrument.
  Equal nominal stepover/cusp is not necessarily equal delivered finish.
- Drill metrics use a separate population. Engagement not-applicable does not
  mean the drill is unverified; known drill tool-divisor limits remain relevant.
- Capability limits (such as lateral workholding refusal and face-selection
  restrictions) require explanation, not invented support.

Do not benchmark new algorithms, recalibrate gates, or change numeric thresholds
as a substitute for examining how evidence is communicated.

## 5. Findings and trace format

`results/Rxx/REPORT.md`:

1. Scope, build, fixture/state manifest and evidence limitations.
2. Five-line summary: completion, largest obstruction, largest confidence risk.
3. Task outcome table: completed unaided / assisted / blocked / not tested;
   active time, compute wait, wrong turns, assistance and interpretation errors.
4. Ranked findings with the record below.
5. Strengths and expert capabilities to preserve.
6. Suggested improvements, alternatives/tradeoffs and acceptance tasks.
7. Cross-package handoffs, known-issue matches, historical claims now false.
8. Untested questions and follow-up evidence needed.

Each finding:

```text
ID: UX-Rxx-NNN
Task / starting state:
Type: behaviour defect | interaction/IA | guidance | capability gap | evidence gap
Impact: S0 misleading confidence/data-loss risk | S1 blocks supported task |
        S2 substantial detour or uncertainty | S3 local friction/polish
Evidence label(s) and confidence: confirmed / probable / needs live test
Observed: exact label, action, before → after state; screenshot/trace and code refs
Expected: user expectation and why it is reasonable
Consequence: incorrect decision, blocked outcome or work imposed (not speculation)
Scope: format, operation, setup, size, experience level; reproducibility
Known issue / related finding / primary owner:
Proposal: smallest useful change; alternative; capability to preserve
Acceptance: concrete user task and visible evidence after change
Effort: copy/local | interaction/wiring | cross-layer programme | unknown
```

Keep severity separate from confidence and effort. A hypothetical safety concern
is high-priority to verify, not a confirmed S0. Avoid aggregating unlike concerns
into a single unexplained score.

`trace.md`: step, goal, visible cue, predicted result, actual action, actual
result, feedback latency, wait time, assistance, screenshot and state reference.
For corrections capture **before → warning → correction → refreshed result**,
not just the final healthy screen.

## 6. Completion rule

A package is complete when required scenarios have evidence or an explicit
BLOCKED/NOT TESTED row, its findings have reproducible starting states, and its
handoffs have named owners. It is not complete because every panel has a PNG.
A blocked package must not declare the untested workflow easy or safe.

Reviewers write only their own report/evidence paths and scratch projects.
Commit nothing. Leave production code, seeds, live user projects and personal
libraries unchanged. Share blockers early; do not spend an entire session trying
to make a large fixture generate.
