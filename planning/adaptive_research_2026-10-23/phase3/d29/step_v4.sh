#!/usr/bin/env bash
# D21 step runner: step.sh <name> <timeout> <cargo args...>; output to phase3/d21/<name>.txt
cd /home/ricky/personal_repos/rs_cam/.claude/worktrees/fixture-cdc9814e
D=/home/ricky/personal_repos/rs_cam/planning/adaptive_research_2026-10-23/phase3/d29
n=$1; to=$2; shift 2; s=$(date +%s)
timeout $to scripts/cargo_lane.sh "$@" 2>&1 | sed -E 's/\x1b\[[0-9;]*m//g' > $D/v4_$n.txt; rc=${PIPESTATUS[0]}
echo "$n rc=$rc $(( $(date +%s)-s ))s"; grep -E '^test result|^error(\[|:)|^warning: [^t]|FAILED|panicked at|^Diff in' $D/v4_$n.txt | head -12
