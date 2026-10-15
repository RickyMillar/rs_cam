# `app/mcp/` — the MCP request handlers and the MCP host lifecycle

Read the parent `app/CLAUDE.md` first. It holds the file map of this folder
and the rules for MCP mutation, export and screenshots.

## The launch and shutdown lifecycle

- `.mcp.json` launches `scripts/gui_logged.sh --mcp`; the wrapper validates its
  stderr log and `exec`s the release GUI so the gateway owns the real GUI PID.
  Clean MCP stdio completion signals the winit host to exit directly; failures
  exit nonzero. A two-second process watchdog bounds shutdown if winit is
  blocked, and is disarmed only when the host acknowledges immediately before
  `ActiveEventLoop::exit`. The completion guard covers returns and panics that
  unwind the outer `mcp-server` thread closure; it does **not** claim panics in
  detached handler tasks. This path bypasses interactive unsaved-close. Rebuild
  the release binary BEFORE an MCP live test, or the server runs old code.
