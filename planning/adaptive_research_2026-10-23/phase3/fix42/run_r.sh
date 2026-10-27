#!/usr/bin/env bash
# D61: one R run (research arm), the release binary of the checked-out tree run directly (not through cargo).
# Args: <tree: 607e52730 | dabf258d3> <corne_B_s2p1 | wanaka_s3p5>. Arm geodesic_local_spiral_pick, RS_CAM_RESEARCH_STEP_SPLIT=stock_depth,
# no override. Job files as D54: d44/k_body_cp_edges_s2p1_d44_rest_regions.toml (--setup 0 --skip 4),
# fix33/jobs/wanaka200_body_cp_edges_d44_s3p5.toml (--setup 0 --resolution 0.6). The sim is kept for depth_by_level.py.
set -u
R=/home/ricky/personal_repos/rs_cam; A=$R/planning/adaptive_research_2026-10-23; WT=$R/.claude/worktrees/fixture-cdc9814e; F=$A/phase3/fix42
tree=$1; s=$2; cd $WT
[ "$(git rev-parse --short=9 HEAD)" = $tree ] && [ -z "$(git status --short)" ] && echo "tree = clean $tree" || { echo "TREE NOT AS EXPECTED"; exit 1; }
[ target/release/rs_cam_cli -nt .git/$(git rev-parse --git-path HEAD | sed 's|^.git/||') ] 2>/dev/null; stat -c 'binary %y' target/release/rs_cam_cli
case $s in corne*) job=$A/phase3/d44/k_body_cp_edges_s2p1_d44_rest_regions.toml; extra="--setup 0 --skip 4";;
           wanaka*) job=$A/phase3/fix33/jobs/wanaka200_body_cp_edges_d44_s3p5.toml; extra="--setup 0 --resolution 0.6";; esac
d=$F/${tree}_$s; rm -rf $d; mkdir -p $d; t=$(date +%s)
env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD -u RS_CAM_RESEARCH_KEEP_DOWN_ROUTE -u RS_CAM_RESEARCH_STEP_SPLIT RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_spiral_pick RS_CAM_RESEARCH_STEP_SPLIT=stock_depth RUST_LOG=info timeout 560 target/release/rs_cam_cli project $job --output-dir $d --summary --emit-gcode $d/program.nc --sim-artifact $extra > $d/stdout.txt 2> $d/stderr.txt
echo "$tree $s rc=$? $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1); refusals $(grep -ciE refused $d/stderr.txt)"
sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt; grep -E "Z levels computed" $d/stdout_plain.txt | sed -E 's/^.*generate_toolpath\{index=([0-9])\}: rs_cam_core::[a-z_:0-9]+: /  index=\1 /'
