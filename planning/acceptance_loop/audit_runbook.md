# Audit runbook

You are the auditor for this round of the acceptance loop. Your job:
run the smoke suite, diff verdicts vs the last baseline, decide what
opens / closes / reopens, hand off to implementers. You do **not**
write product code.

Read `README.md` first if unfamiliar with the loop.

> **Running the loop without a human in the loop?** Read
> `handoff_prompts/autonomous_auditor.md`. It extends this runbook
> with the orchestration rules (spawning implementer agents in
> background, MCP triage triggers, when to ask the user). This
> runbook is still the per-round procedure either way.

## Step 0 — Inventory before you touch anything

1. Read `STATE.md` end-to-end. Note:
   - Current round number
   - In-flight findings and who claimed them
   - Implementation-log entries since the last verified baseline
2. **MCP triage — DO THIS BEFORE ANYTHING ELSE.** Confirm the MCP
   `rs-cam` server is reachable:
   - Check that `mcp__rs-cam__*` tools appear in your available tools.
   - Call `mcp__rs-cam__project_summary` on a known scratch project.
   - Call `mcp__rs-cam__get_operation_schema` for `"pocket"`.

   **STOP and ask the user if:**
   - `rs-cam` tools are missing from the tool list (MCP disconnected).
   - Any of those calls error or time out.
   - A previously-clean smoke case (e.g. AS001 from round-01)
     returns `harness_error` this run with no code change explaining
     it (stale MCP).
   - Tool schemas differ from what `STATE.md` or finding files
     describe (rebuilt MCP with changed schema).

   The user is the only one who can reconnect or rebuild the MCP.
   When they say it's fixed, **re-run this triage** before continuing.
3. `git status --short` — note any pre-existing dirty state so you
   don't blame the previous round for it. Do not reset or stash.

## Step 1 — Run the smoke acceptance suite

Follow `planning/SUGGEST_SIM_OPTIMIZE_AGENT_RUN_PROMPT.md` for the
mechanics. Inputs:

- `planning/toolpath_acceptance/cases_agent_smoke.csv` (18 cases)
- `planning/toolpath_acceptance/seed_goal_matrix.csv`
- `planning/toolpath_acceptance/toolpath_catalog.csv`

Output goes to
`target/acceptance_sweeps/agent_smoke_<YYYYMMDD_HHMM>/results.csv`.

If a case from prior rounds is now blocked by an issue not on the
finding list (e.g. a new precondition error), don't skip it silently
— open a new finding.

If runtime gets pathological (a single op > 5 min sim), record the
runtime, fail the case as `harness_error`, open a finding, and move
on. Don't fight the GUI.

When verifying landed findings, prefer running the same smoke probe
the user-facing surface takes (MCP `run_simulation`, GUI Generate
button, etc.) — implementation-layer tests can pass even when the
production path bypasses the fix. See round-04 three-rebuild saga
in `rounds/round-04-2026-05-25/delta.md`.

## Step 2 — Diff vs last baseline

For each case in this round's `results.csv`, compare to the same case
in the prior round's `baseline.md` table.

Categorise each delta:

- **Fix verified** — case moved from fail → warn/pass on the predicted
  axis. Mark the in-flight finding `verified`.
- **Regression** — case moved from pass/warn → fail or surfaced a new
  Exceeds verdict. Reopen the closest matching finding OR open a new
  one.
- **No change, expected** — verdict shape unchanged because the fix
  didn't target this axis. Note in delta.md but no action.
- **No change, unexpected** — fix was supposed to move this case and
  didn't. Investigation needed; finding stays in_flight with a note.

Write `rounds/round-NN-YYYY-MM-DD/delta.md` with:

- Headline numbers (cases attempted / generated / simulated, pass / warn / fail counts)
- Findings closed this round (with verifying case IDs)
- Findings reopened
- New findings opened (link to `findings/F-XXX-*.md`)
- Acceptance-bar snapshot vs prior round

## Step 3 — Hunt new findings (only if needed)

Don't spawn research agents unless a symptom can't be explained from
the live tool output. The MCP overhaul gave us `diagnostic_delta`,
`gui_banners`, `warnings`, `operation_schema`, `param_schema` — use
these before chasing source.

When you DO spawn a research agent (see `Agent` tool with
`general-purpose` subagent type), write the prompt to:

- State the symptom with concrete case IDs and evidence
- Point at known starting files (catalog, session, tool_load, dexel_stock)
- Ask for a structured report under 700 words, file:line cites mandatory
- Be explicit about what's out of scope ("don't grade unrelated code")

When the agent returns, distill its findings into one or more
`F-XXX-*.md` files. Don't paste the whole agent report — extract the
load-bearing claims with citations.

## Step 4 — Update STATE.md

Rewrite the queue:

- Move `verified` items into round-NN's "Closed this round"
- Move `landed` items to in-flight or verified depending on whether
  acceptance test passed
- Re-rank the open queue (severity → effort)
- Update "Last verified baseline" pointer to this round's baseline
- Refresh the acceptance bars table with current snapshot
- Bump "Current round" to round-(N+1) with "audit pending"

## Step 5 — Hand off to implementers

If there are no unclaimed findings: write a note in STATE.md saying so
and stop. The loop is paused until either a new finding surfaces or
the user wants you to run the smoke again.

If there are unclaimed findings:

- Choose the top 1–3 from the queue
- Either:
  - Point an existing implementer agent at `STATE.md` + the specific
    F-IDs (preferred — they self-serve from there), or
  - Write a per-fix handoff prompt to `handoff_prompts/` if the
    finding needs more context than the findings file carries

Stop. Don't write code. Don't claim verified-by-anything-other-than-
a-completed-baseline.

## What you are NOT allowed to do

- Write product code
- Run the optimizer or sim across the full project tree (use the smoke
  CSV's subset; that's the calibration set)
- Edit a finding file's `Status` field directly — `Status` changes
  follow the lifecycle in README.md, driven by smoke runs and PR
  merges, not by hand
- Delete files in `archive/`
- Reassign a finding's ID — IDs are stable forever

## When to stop and ask the user

- **MCP unreachable, stale, or rebuilt with a schema change** — see
  Step 0 triage. The user is the only one who can reconnect/rebuild
  the MCP. Mark the round partial in STATE.md and stop.
- A finding's acceptance test, when run, contradicts the smoke evidence
  (means either the test is wrong or the finding's framing is wrong)
- An implementer marked a finding `landed` but the verifying smoke case
  still fails the original acceptance test
- The smoke run itself starts failing for harness reasons (MCP timeouts,
  GUI freezes) you can't work around in 3 retries
- Acceptance bars are met — the loop is ready to exit
