# rs_cam_mcp_proxy instructions

A stdio MCP supervisor. The MCP client starts the proxy, and the proxy
starts `rs_cam_gui --mcp` as its child. The proxy stays up when the GUI
exits, so an agent can restart the GUI (`gui_restart`) with no `/mcp`
reconnect. Read root `CLAUDE.md` first.

## Files

| File | Holds |
|---|---|
| `src/proxy.rs` | routing, handshake replay, child lifecycle, `gui_status` |
| `src/procinfo.rs` | `/proc` reads, the GUI pid walk, signals, OOM evidence |
| `src/tools.rs` | the proxy tool schemas |
| `src/config.rs`, `src/log.rs` | arguments; stderr and `--log` output |
| `src/bin/rs_cam_mcp_fake_child.rs` | test fixture MCP server, not product |
| `tests/proxy.rs` | stdio tests against the fake child |

## Invariants

- The crate depends on `serde_json` only. Do not add `rs_cam_core`,
  `rs_cam_viz` or an MCP SDK: the proxy must build in seconds.
- Stdout is the client transport. Write logs to stderr or `--log` only.
- One writer thread per pipe. The client reader never writes to the child
  pipe directly, and no code holds the state lock while it waits.
- Proxy request ids start with `rs_cam_mcp_proxy:`. Client ids pass through.
- A child that exits on its own is not started again, unless
  `--auto-restart` is set (one restart per 30 s at most).
- An OOM verdict names its evidence (`last_exit.oom_checks`).

## Tests

`cargo test -p rs_cam_mcp_proxy -q` (about 2 s). The OOM test needs user
systemd and returns early without it. Never build `rs_cam_viz` for this
crate.

## Traps

- `systemd-run --scope` replaces itself with the GUI, so the child pid is
  the GUI pid. A launcher that does not exec reports 128+n for signal n.
