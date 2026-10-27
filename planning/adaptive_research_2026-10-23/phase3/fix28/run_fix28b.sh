#!/usr/bin/env bash
# D38 part b: the research arm's router DEFAULT is CutFeed (adaptive3d/mod.rs:523-528), so the run with the
# variable unset (route_default_cut_feed) already routes. This run sets RS_CAM_RESEARCH_KEEP_DOWN_ROUTE=off.
# Waits for D37 (fix27) to finish first.
set -u
R=/home/ricky/personal_repos/rs_cam
A=$R/planning/adaptive_research_2026-10-23
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$A/phase3/fix28
until grep -q D37RUNSDONE $A/phase3/fix27/run.log 2>/dev/null; do sleep 20; done
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 7b7a8df6c ] && [ -z "$(git status --short)" ] && echo "tree = clean 7b7a8df6c" || { echo "TREE NOT AS EXPECTED"; exit 1; }
d=$F/route_off; rm -rf $d; mkdir -p $d; t=$(date +%s)
env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD -u RS_CAM_RESEARCH_KEEP_DOWN_ROUTE RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_spiral_pick RS_CAM_RESEARCH_KEEP_DOWN_ROUTE=off RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug timeout 1500 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $A/phase3/fix26/jobs/k_body_cp_edges.toml --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact --setup 0 --skip 4 > $d/stdout.txt 2> $d/stderr.txt
echo "route_off rc=$? $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
echo D38BDONE
