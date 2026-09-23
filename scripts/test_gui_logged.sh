#!/usr/bin/env bash
# Focused process-contract test for gui_logged.sh; does not launch a GUI.
set -u

repo_root="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)" || exit 2
wrapper="$repo_root/scripts/gui_logged.sh"
tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/rs-cam-gui-wrapper.XXXXXX")" || exit 2
mkdir -p "$tmp_root/home" "$tmp_root/bad-home" \
  "$tmp_root/bad-file-home/.rs_cam_logs/gui_stderr.log"
fake="$tmp_root/fake-gui"

cleanup() {
  rm -f "$fake" "$tmp_root"/*.out "$tmp_root"/*.err
  rm -f "$tmp_root/home/.rs_cam_logs/gui_stderr.log"
  rm -f "$tmp_root/bad-home/.rs_cam_logs"
  rmdir "$tmp_root/bad-file-home/.rs_cam_logs/gui_stderr.log" \
    "$tmp_root/bad-file-home/.rs_cam_logs" "$tmp_root/bad-file-home" \
    "$tmp_root/home/.rs_cam_logs" "$tmp_root/home" \
    "$tmp_root/bad-home" "$tmp_root" 2>/dev/null || true
}
trap cleanup EXIT

cat > "$fake" <<'FAKE'
#!/usr/bin/env bash
printf 'pid:%s\n' "$$"
for arg in "$@"; do
  printf 'arg:%s\n' "$arg"
done
printf 'fake stderr\n' >&2
exit "${FAKE_EXIT:-0}"
FAKE
chmod 700 "$fake"

# `tee` runs asynchronously in process substitution, so wait briefly for its
# append rather than assuming it has flushed when the GUI has exited.
wait_for_log_line() {
  local line=$1
  local log_file=$2
  local attempt
  for ((attempt = 0; attempt < 20; attempt++)); do
    if grep -Fx "$line" "$log_file" > /dev/null 2>&1; then
      return 0
    fi
    sleep 0.05
  done
  echo "timed out waiting for log line: $line" >&2
  return 1
}

# A bad log destination must fail before MCP exec, visibly and nonzero.
: > "$tmp_root/bad-home/.rs_cam_logs"
set +e
HOME="$tmp_root/bad-home" RS_CAM_GUI_BIN="$fake" \
  "$wrapper" --mcp > "$tmp_root/bad-log.out" 2> "$tmp_root/bad-log.err"
bad_log_status=$?
set -e
[[ "$bad_log_status" -eq 2 ]]
[[ ! -s "$tmp_root/bad-log.out" ]]
grep -F 'rs_cam_gui: cannot create stderr log directory:' \
  "$tmp_root/bad-log.err" > /dev/null

# A directory at the log-file path reaches the separate open failure.
set +e
HOME="$tmp_root/bad-file-home" RS_CAM_GUI_BIN="$fake" \
  "$wrapper" --mcp > "$tmp_root/bad-file.out" 2> "$tmp_root/bad-file.err"
bad_file_status=$?
set -e
[[ "$bad_file_status" -eq 2 ]]
[[ ! -s "$tmp_root/bad-file.out" ]]
grep -F 'rs_cam_gui: cannot open stderr log:' \
  "$tmp_root/bad-file.err" > /dev/null

# --mcp is deliberately not argv[1]. The fake must inherit the wrapper PID,
# proving that the wrapper used exec rather than orphaning a child.
HOME="$tmp_root/home" RS_CAM_GUI_BIN="$fake" FAKE_EXIT=7 \
  "$wrapper" before --mcp after > "$tmp_root/mcp.out" 2> "$tmp_root/mcp.err" &
wrapper_pid=$!
set +e
wait "$wrapper_pid"
mcp_status=$?
set -e

[[ "$mcp_status" -eq 7 ]]
grep -Fx "pid:$wrapper_pid" "$tmp_root/mcp.out" > /dev/null
grep -Fx 'arg:before' "$tmp_root/mcp.out" > /dev/null
grep -Fx 'arg:--mcp' "$tmp_root/mcp.out" > /dev/null
grep -Fx 'arg:after' "$tmp_root/mcp.out" > /dev/null
grep -Fx 'fake stderr' "$tmp_root/mcp.err" > /dev/null
wait_for_log_line 'fake stderr' "$tmp_root/home/.rs_cam_logs/gui_stderr.log"

# Without --mcp the wrapper remains the parent and records the child status.
HOME="$tmp_root/home" RS_CAM_GUI_BIN="$fake" FAKE_EXIT=9 \
  "$wrapper" interactive > "$tmp_root/normal.out" 2> "$tmp_root/normal.err" &
wrapper_pid=$!
set +e
wait "$wrapper_pid"
normal_status=$?
set -e

[[ "$normal_status" -eq 9 ]]
if grep -Fx "pid:$wrapper_pid" "$tmp_root/normal.out" > /dev/null; then
  echo 'normal mode unexpectedly execed the GUI' >&2
  exit 1
fi
grep -Fx 'arg:interactive' "$tmp_root/normal.out" > /dev/null
grep -F 'rs_cam_gui exited with status 9' \
  "$tmp_root/home/.rs_cam_logs/gui_stderr.log" > /dev/null

printf 'gui_logged.sh process contract: ok\n'
