#!/usr/bin/env bash
# D62: shipped CP jobs on master 67257dc20 (release binary, no research feature, run directly; no env, no override).
# Args: <tag> <job toml> <extra flags...>. The sim is kept for depth_by_level.py.
set -u
R=/home/ricky/personal_repos/rs_cam; WT=$R/.claude/worktrees/fixture-cdc9814e; F=$R/planning/adaptive_research_2026-10-23/phase3/fix43
cd $WT; [ "$(git rev-parse --short=9 HEAD)" = 67257dc20 ] && [ -z "$(git status --short)" ] && echo "tree = clean 67257dc20" || { echo "TREE NOT AS EXPECTED"; exit 1; }
s=$1; job=$2; shift 2; d=$F/$s; rm -rf $d; mkdir -p $d; t=$(date +%s)
env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD -u RS_CAM_RESEARCH_KEEP_DOWN_ROUTE -u RS_CAM_RESEARCH_STEP_SPLIT RUST_LOG=info timeout 560 target/release/rs_cam_cli project $job --output-dir $d --summary --emit-gcode $d/program.nc --sim-artifact "$@" > $d/stdout.txt 2> $d/stderr.txt
echo "$s rc=$? $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1); refusals $(grep -ciE refused $d/stderr.txt)"
sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt; grep -E "Z levels computed" $d/stdout_plain.txt | sed -E 's/^.*generate_toolpath\{index=([0-9])\}: rs_cam_core::[a-z_:0-9]+: /  index=\1 /'
