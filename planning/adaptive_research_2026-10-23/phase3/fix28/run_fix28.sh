#!/usr/bin/env bash
# D38: why Corne body_cp op 1 has 1050 descents. fix26/jobs/k_body_cp_edges.toml (the wanaka kinematics block),
# clean 7b7a8df6c, fix25 flags; router off (the default) and RS_CAM_RESEARCH_KEEP_DOWN_ROUTE=cut_feed (Part B).
set -u
R=/home/ricky/personal_repos/rs_cam
A=$R/planning/adaptive_research_2026-10-23
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$A/phase3/fix28
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 7b7a8df6c ] && [ -z "$(git status --short)" ] && echo "tree = clean 7b7a8df6c" || { echo "TREE NOT AS EXPECTED"; exit 1; }
run() { name=$1; envs=$2; d=$F/$name; rm -rf $d; mkdir -p $d; t=$(date +%s)
  env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD -u RS_CAM_RESEARCH_KEEP_DOWN_ROUTE RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_spiral_pick $envs RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug timeout 1500 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $A/phase3/fix26/jobs/k_body_cp_edges.toml --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact --setup 0 --skip 4 > $d/stdout.txt 2> $d/stderr.txt
  echo "$name rc=$? $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt; }
run route_default_cut_feed ""   # renamed after the run: the research arm defaults to CutFeed
run route_cut_feed "RS_CAM_RESEARCH_KEEP_DOWN_ROUTE=cut_feed"
cmp $F/route_off/program.nc $A/phase3/fix26/k_body_cp_corne/program.nc && echo "route_off program == fix26 k_body_cp_corne"
echo D38RUNSDONE
