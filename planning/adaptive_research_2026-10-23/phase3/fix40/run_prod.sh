#!/usr/bin/env bash
# D59 (copy of fix39/run_prod.sh, writes into fix40; job files from fix39/jobs). D58: the product strategy (clearing_strategy = "geodesic_adaptive"), tree 26d901943 (geodesic-integration), release build
# WITHOUT the research feature, no env variables, no export override. Arg: corne_B_s2p1 | wanaka_s2p0 | wanaka_s3p5.
# Flags as R: Corne --setup 0 --skip 4; wanaka --setup 0 --resolution 0.6. The sim is kept for analyse_prod.sh.
set -u
R=/home/ricky/personal_repos/rs_cam; A=$R/planning/adaptive_research_2026-10-23; WT=$R/.claude/worktrees/fixture-cdc9814e; F=$A/phase3/fix40
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 26d901943 ] && [ -z "$(git status --short)" ] && echo "tree = clean 26d901943" || { echo "TREE NOT AS EXPECTED"; exit 1; }
s=$1; case $s in corne*) extra="--setup 0 --skip 4";; wanaka*) extra="--setup 0 --resolution 0.6";; esac
d=$F/$s; rm -rf $d; mkdir -p $d; t=$(date +%s)
env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD -u RS_CAM_RESEARCH_KEEP_DOWN_ROUTE -u RS_CAM_RESEARCH_STEP_SPLIT RUST_LOG=info timeout 500 scripts/cargo_lane.sh run --release -q -p rs_cam_cli -- project $A/phase3/fix39/jobs/${s}_geodesic_adaptive.toml --output-dir $d --summary --emit-gcode $d/program.nc --sim-artifact $extra > $d/stdout.txt 2> $d/stderr.txt
echo "$s rc=$? $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
echo "  refusals: $(grep -ciE refused $d/stderr.txt)"; ls $d
