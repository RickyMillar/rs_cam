#!/usr/bin/env bash
# D57: wanaka s2.0 (fix33/jobs/wanaka200_body_cp_edges_d44_s2p0.toml, --setup 0 --resolution 0.6), StockDepth split, arm
# geodesic_local_spiral_pick, no export override. Arg: A (clean f2724a5bd, D55) | B (f2724a5bd + d56/tree_vs_f2724a5bd_WIP.patch).
# The simulation.json is kept; the analysis deletes it.
set -u
R=/home/ricky/personal_repos/rs_cam; A=$R/planning/adaptive_research_2026-10-23; WT=$R/.claude/worktrees/fixture-cdc9814e; F=$A/phase3/fix38
PATCH=$A/phase3/d56/tree_vs_f2724a5bd_WIP.patch
cd $WT; run=$1
[ "$(git rev-parse --short=9 HEAD)" = f2724a5bd ] || { echo "HEAD NOT f2724a5bd"; exit 1; }
if [ $run = A ]; then [ -z "$(git status --short)" ] && echo "tree = clean f2724a5bd" || { echo "TREE NOT CLEAN"; exit 1; }
else git apply --reverse --check $PATCH && [ "$(git status --short | wc -l)" = "$(grep -c '^diff --git' $PATCH)" ] && echo "tree = f2724a5bd + D56 WIP patch" || { echo "TREE NOT f2724a5bd + PATCH"; exit 1; }; fi
d=$F/$run; rm -rf $d; mkdir -p $d; t=$(date +%s)
env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD -u RS_CAM_RESEARCH_KEEP_DOWN_ROUTE -u RS_CAM_RESEARCH_STEP_SPLIT RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_spiral_pick RS_CAM_RESEARCH_STEP_SPLIT=stock_depth RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug,rs_cam_core::adaptive3d=debug timeout 500 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $A/phase3/fix33/jobs/wanaka200_body_cp_edges_d44_s2p0.toml --output-dir $d --summary --emit-gcode $d/program.nc --sim-artifact --setup 0 --resolution 0.6 > $d/stdout.txt 2> $d/stderr.txt
echo "$run rc=$? $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
echo "  refusals: $(grep -ciE refused $d/stderr.txt)"
