#!/usr/bin/env bash
# Usage: run.sh <arm,arm,...>
D=/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/61c87c01-64ec-4b8d-a255-a09cc23316a1/scratchpad/plansimgap
BIN=$(ls -t /home/ricky/personal_repos/rs_cam/.claude/worktrees/agent-a9d705ebaf3dee576/target/release-fast/deps/psg_probe-* | grep -v '\.d$' | head -1)
cd /home/ricky/personal_repos/rs_cam/.claude/worktrees/agent-a9d705ebaf3dee576/crates/rs_cam_core
export PSG=1 PSG_ARMS="$1" PSG_OUT=$D
start=$(date +%s)
"$BIN" --ignored --nocapture psg_probe > "$D/run_$1.log" 2>&1
echo "exit $? after $(( $(date +%s) - start )) s"
