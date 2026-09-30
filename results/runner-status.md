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
