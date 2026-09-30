---
runner_session: rs-cam-be (local terminal on Ricky's PC; replaces session_01X9bPqEnCfUkrbNprBjUs3d)
updated: 2026-09-30T21:06:35Z
---
## Summary

The runner moved from the Remote Control session to a local terminal
session. The mailbox protocol in README.md is unchanged.

- The runner polls origin/comms/runner every 60 s.
- The runner cannot send a send_message to the lead yet: the lead session
  is not reachable from this terminal. Until that changes, poll
  results/ yourself.
- The runner now has the rs-cam MCP (rs-cam-capped: the GUI under
  systemd-run MemoryMax=16G). It can drive the GUI for MCP jobs.

Queue: empty. Jobs 001-006 are done.

## 2026-10-01 ~22:20 UTC: new runner task from Ricky

Ricky asked the runner to audit memory feasibility across the whole app
and to draft a memory-budget architecture (config budget, preflight
estimate, queueing, graceful stop, degrade-with-warning, e.g. lighter
toolpath renders). Read-only agents; no code changes. The spec goes to
planning/memory_budget_2026-10-01/ (not committed until the lead agrees);
the path follows here. Tell the runner if this collides with your G-SIMMEM
work.

Re-opened from result 006 addendum 3 (the correction over-withdrew):
after a GUI Generate All on 09-30, RSS fell 8.78 -> 1.48 GB when the sim
started; after MCP generate_all (plan ran 1 simulation) RSS held
8.1-9.2 GB and never fell. Same commit, same 0.2 mm, trace off in both
GUI-button runs. That ~7 GB of retained memory is unexplained and is
audit item 1.
