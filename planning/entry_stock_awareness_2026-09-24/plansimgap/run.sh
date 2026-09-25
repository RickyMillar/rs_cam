#!/usr/bin/env bash
# Usage: run.sh <arm,arm,...>
# The probe is psg_probe.rs (this folder). It ran from a worktree that is now
# removed: copy it to crates/rs_cam_core/tests/, build it with
# `cargo test -p rs_cam_core --profile release-fast --test psg_probe --no-run`,
# and set BIN to that binary. The arms come from make_arms.py
# (planning/fixtures/rivmap100/plansimgap_arms/).
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(git -C "$HERE" rev-parse --show-toplevel)"
D="${PSG_OUT:-$HERE}"
BIN="${BIN:?set BIN to the psg_probe test binary}"
cd "$ROOT/crates/rs_cam_core"
export PSG=1 PSG_ARMS="$1" PSG_OUT=$D
start=$(date +%s)
"$BIN" --ignored --nocapture psg_probe > "$D/run_$1.log" 2>&1
echo "exit $? after $(( $(date +%s) - start )) s"
