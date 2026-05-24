# Acceptance loop — how this works

This directory is the **active workstream** for getting `rs_cam`'s
Suggest / Sim / Optimize calculations to a state we can trust. It runs
as a repeatable audit-fix-audit loop until every acceptance bar in
`planning/SUGGEST_SIM_OPTIMIZE_ACCEPTANCE.md` is met.

Any agent working in this repo — auditor, implementer, or one-off
fixer — starts here. `STATE.md` is the single source of truth for what
round we're on, what's open, what's claimed, and what's done.

## Roles

**Auditor.** Pure read + propose. Runs the smoke acceptance suite,
diffs verdicts against the prior round, opens new findings, closes
verified ones, and re-prioritises the queue. Never writes code or
ships fixes. Usually a fresh / post-compact Claude session.

**Implementer.** Pure write code + tests. Pulls the top unclaimed
finding from `STATE.md`, lands one PR that includes the acceptance
test embedded in the finding file, updates the finding's status. Does
NOT run the full smoke. Does NOT claim "verified" — that's the
auditor's call in the next round. Usually a sub-agent or a dedicated
implementation session.

## Round = one full cycle

```
[Auditor]
  read STATE.md → run smoke → diff vs last baseline →
    for each in-flight finding: verified | reopen
    for each new symptom:        open
  update STATE.md, write round-NN/baseline.md + delta.md
  hand off →

[Implementer(s)]
  pull top unclaimed finding → land PR with embedded acceptance test →
  update finding Status: open → in_flight → landed
  update STATE.md "In flight" → next round's "Closed by implementer"
  hand back →

[Auditor]
  ... next round
```

A round ends when every In-flight finding has either landed or
explicitly deferred. The next round begins with a fresh smoke run.

## Files in this directory

| File | Owner | Purpose |
|---|---|---|
| `README.md` | invariant | this file — how the loop works |
| `STATE.md` | both | current state. Auditor writes, Implementer updates fields. **Always read this first.** |
| `audit_runbook.md` | Auditor | step-by-step for what to do each round |
| `implementer_contract.md` | Implementer | what an implementer MUST do and return |
| `findings/F-XXX-*.md` | both | one file per finding, stable ID, never renamed or deleted |
| `rounds/round-NN-YYYY-MM-DD/baseline.md` | Auditor | the smoke results for that round |
| `rounds/round-NN-YYYY-MM-DD/delta.md` | Auditor | diff vs prior round |
| `rounds/round-NN-YYYY-MM-DD/fix-prompts/` | Auditor | optional per-fix agent prompts for handoff |
| `archive/` | one-way | superseded plans / shipped prompts (never deleted, just retired) |
| `handoff_prompts/` | Auditor | reusable prompts for next implementer / next auditor session |

## Finding lifecycle

```
open  →  in_flight  →  landed  →  verified
                                ↘
                                  reopen → in_flight → ...
                       ↘
                         deferred (waiting on upstream / not now)
                       ↘
                         wont_fix (with reason)
```

- IDs are stable. `F-001` is `F-001` forever, even after it's verified.
- A regression after `verified` REOPENS the original finding — do not
  invent a new ID for the same root cause.
- New root cause same area → new ID.

## Severity ladder

- **blocker** — acceptance bar cannot be met until this is fixed
- **high** — visible bad output in the smoke suite
- **medium** — confirmed bug or duplication, smoke doesn't fully expose it
- **low** — code-smell or doc drift; no functional impact

## Effort ratings

`S` < 50 LOC, `M` 50-300, `L` 300-1000, `XL` > 1000 or multi-crate.

## Exit criteria for the whole loop

Loop ends when ALL of:

- Suggest first-shot landing rate ≥ 90% on covered cells in `cases_agent_smoke.csv`
- Sim chipload calibration ≥ 95% agreement with goal rows (per-stage in `SUGGEST_SIM_OPTIMIZE_ACCEPTANCE.md`)
- Sim deflection calibration ≥ 95% (after `peak_axial_doc_mm` split lands)
- Optimizer honest-improvement rate ≥ 95% on tested cases, refusal rate 100% on n/a cases
- Export gate: 100% blocking on unsafe Exceeds
- No finding has been Open longer than 2 rounds without principled closure

Until then, keep looping.
