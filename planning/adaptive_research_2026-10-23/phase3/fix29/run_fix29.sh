#!/usr/bin/env bash
# D41: op 5 (the CP rest on the edges) of Corne body_cp, at op-1 stepover 1.2 and 2.1
# (fix27/jobs/k_body_cp_edges_s1p2.toml, _s2p1.toml; the wanaka kinematics block). Clean 7b7a8df6c.
set -u
R=/home/ricky/personal_repos/rs_cam
A=$R/planning/adaptive_research_2026-10-23
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$A/phase3/fix29
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 7b7a8df6c ] && [ -z "$(git status --short)" ] && echo "tree = clean 7b7a8df6c" || { echo "TREE NOT AS EXPECTED"; exit 1; }
for s in 1p2 2p1; do
  d=$F/s$s; rm -rf $d; mkdir -p $d; t=$(date +%s)
  env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD -u RS_CAM_RESEARCH_KEEP_DOWN_ROUTE RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_spiral_pick RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug timeout 1500 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $A/phase3/fix27/jobs/k_body_cp_edges_s$s.toml --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact --setup 0 --skip 4 > $d/stdout.txt 2> $d/stderr.txt
  echo "s$s rc=$? $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
done
echo D41RUNSDONE
