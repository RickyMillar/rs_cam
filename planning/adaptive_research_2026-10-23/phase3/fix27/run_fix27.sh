#!/usr/bin/env bash
# D37: the op-1 stepover sweep, Corne body_cp with the wanaka kinematics block (fix26/jobs/k_body_cp_edges.toml),
# op 1 stepover 1.2 x 1.0 / 1.25 / 1.5 / 1.75 (jobs/k_body_cp_edges_s*.toml; nothing else changed).
# Also the CP reference (s = cp: fix26/jobs/k_contour_parallel.toml, no research variable).
# The area oracle reads with AREA_SCREEN=0 (every clearing cutting sample), for the percentiles.
# Fixture worktree at clean 7b7a8df6c (waits for D36 to restore it). Per run: the area oracle on op 1, the audit
# of op 1, then the simulation.json is deleted.
set -u
R=/home/ricky/personal_repos/rs_cam
A=$R/planning/adaptive_research_2026-10-23
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$A/phase3/fix27
until grep -q D36DONE $A/phase3/d36/run_d36.log 2>/dev/null; do sleep 20; done
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 7b7a8df6c ] && [ -z "$(git status --short)" ] && echo "tree = clean 7b7a8df6c" || { echo "TREE NOT AS EXPECTED"; exit 1; }
for s in cp 1p2 1p5 1p8 2p1; do
  job=$F/jobs/k_body_cp_edges_s$s.toml; arm="RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_spiral_pick"
  [ $s = cp ] && { job=$A/phase3/fix26/jobs/k_contour_parallel.toml; arm=""; }
  d=$F/s$s; rm -rf $d; mkdir -p $d; t=$(date +%s)
  env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD $arm RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug timeout 1500 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $job --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact --setup 0 --skip 4 > $d/stdout.txt 2> $d/stderr.txt
  echo "s$s rc=$? $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
  (cd $F && AREA_SCREEN=0 python3 -I area_oracle.py fix27/s$s corne 1 && python3 -I ../fix25/audit_d34.py ../fix27/s$s --op "3D Rough 1" --tp "tp_1_*.json" --nominal-mrr 182.25 --json audit_s${s}_tp1.json > audit_s${s}_tp1.txt 2>&1 && python3 -I ../fix26/three_clocks.py fix27/s$s 1 "3D Rough 1" 4000 > clocks_s${s}_tp1.json && { [ $s = cp ] || python3 -I ../fix26/three_clocks.py fix27/s$s 5 "3D Rough 1 [edges, CP rest]" 4000 > clocks_s${s}_tp5.json; } && rm -f $d/simulation.json) || echo "  analysis failed for s$s (simulation.json kept)"
  df -h / | tail -1 | awk '{print "  disk free",$4}'
done
echo D37RUNSDONE
