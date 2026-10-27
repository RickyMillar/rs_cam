#!/usr/bin/env bash
# D35: fixture worktree at 7b7a8df6c (= 3181ae54f + the D34 patch + its two new files).
# 1. body_cp/corne again (fix25's flags), for the copy op's time split (fix25's simulation.json is deleted).
# 2. Four Corne CP runs: fix6's job file with one field moved to the phase0 file (jobs/s1..s4), fix6's flags.
set -u
R=/home/ricky/personal_repos/rs_cam
A=$R/planning/adaptive_research_2026-10-23
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$A/phase3/fix26
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 7b7a8df6c ] && [ -z "$(git status --short)" ] && echo "tree = clean 7b7a8df6c" || { echo "TREE NOT AS EXPECTED"; exit 1; }
run() { name=$1; proj=$2; envs=$3; d=$F/$name; rm -rf $d; mkdir -p $d; s=$(date +%s)
  env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD $envs RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug timeout 1500 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $proj --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact --setup 0 --skip 4 > $d/stdout.txt 2> $d/stderr.txt
  echo "$name rc=$? $(( $(date +%s)-s ))s"; sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt; df -h / | tail -1 | awk '{print "  disk free",$4}'; }
run body_cp_corne $R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_body_cp_edges.toml "RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_spiral_pick"
for j in s1_stock_box s2_aggressiveness s3_op_params s4_waterline_off; do run cp_$j $F/jobs/$j.toml ""; done
echo RUNSDONE
