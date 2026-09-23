#!/usr/bin/env bash
# Launch rs_cam_gui with persistent crash logging.
#
# Pi's MCP config (`.mcp.json`) spawns this wrapper with `--mcp`; tracing and
# panic backtraces must go to stderr because stdout is the MCP transport. The
# stderr tee keeps a copy at ~/.rs_cam_logs/gui_stderr.log.
#
# In MCP mode the wrapper `exec`s the GUI. The gateway therefore owns the real
# GUI PID, and closing its stdio reaches the process it launched instead of
# leaving a child behind. Interactive launches still wait and record exit
# status (128+n = killed by signal n).
#
# RUST_BACKTRACE=1 keeps panic backtraces; RUST_LOG is overridable.
set -u
cd "$(git -C "$(dirname "$0")" rev-parse --show-toplevel)" || exit 2

log_dir="$HOME/.rs_cam_logs"
log_file="$log_dir/gui_stderr.log"
if ! mkdir -p "$log_dir"; then
  printf 'rs_cam_gui: cannot create stderr log directory: %s\n' "$log_dir" >&2
  exit 2
fi
# Open the destination before launching the GUI. In particular, an MCP exec
# must not succeed while its only persistent diagnostic sink is unavailable.
if ! : >> "$log_file"; then
  printf 'rs_cam_gui: cannot open stderr log: %s\n' "$log_file" >&2
  exit 2
fi

export RUST_LOG="${RUST_LOG:-info}"
export RUST_BACKTRACE=1

# Tests may point this wrapper at a harmless fake executable. Production leaves
# the override unset and runs the release GUI.
gui_bin="${RS_CAM_GUI_BIN:-target/release/rs_cam_gui}"
mcp_mode=0
for arg in "$@"; do
  if [[ "$arg" == "--mcp" ]]; then
    mcp_mode=1
    break
  fi
done

# MCP transport: stdout passes through untouched. stderr is split so both the
# gateway and the persistent log retain it. `exec` preserves this wrapper's PID.
if ((mcp_mode)); then
  exec "$gui_bin" "$@" 2> >(tee -a "$log_file" >&2)
fi

# Interactive mode keeps the wrapper alive so it can append the final status.
"$gui_bin" "$@" 2> >(tee -a "$log_file" >&2)
status=$?
{
  echo ""
  echo "$(date -Is) rs_cam_gui exited with status ${status}"
  if [ "$status" -gt 128 ]; then
    echo "  (killed by signal $((status - 128)) — from outside the process)"
  elif [ "$status" -eq 0 ]; then
    echo "  (clean exit)"
  else
    echo "  (exited on its own)"
  fi
} >> "$log_file"
exit "$status"
