#!/usr/bin/env bash
# D69: one product run of a play-project copy on the release binary of the fixture worktree (HEAD + any prototype diff,
# recorded). Keeps simulation.json for the analysis step (analyse.sh deletes it). Args: <label> <job toml> [extra cli args]
set -u
R=/home/ricky/personal_repos/rs_cam; WT=$R/.claude/worktrees/fixture-cdc9814e; F=$R/planning/adaptive_research_2026-10-23/phase3/d69
label=$1; job=$2; shift 2; cd $WT
d=$F/$label; rm -rf $d; mkdir -p $d
{ git rev-parse HEAD; git diff | md5sum; git diff --stat; } > $d/tree.txt
t=$(date +%s)
env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD -u RS_CAM_RESEARCH_KEEP_DOWN_ROUTE -u RS_CAM_RESEARCH_STEP_SPLIT RUST_LOG=info timeout 590 target/release/rs_cam_cli project $job --output-dir $d --summary --emit-gcode $d/program.nc --sim-artifact --setup 0 "$@" > $d/stdout.txt 2> $d/stderr.txt
echo "$label rc=$? $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1); refusals $(grep -ciE refused $d/stderr.txt)"
sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
