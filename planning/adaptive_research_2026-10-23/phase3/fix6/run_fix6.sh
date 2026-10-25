#!/usr/bin/env bash
# D-FIX6: the contour_parallel controls at f32401326 (no research arm
# variable), --sim-artifact, timeout 900, through the lane script.
set -u
R=/home/ricky/personal_repos/rs_cam
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$R/planning/adaptive_research_2026-10-23/phase3/fix6
C=$R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_contour_parallel.toml
W=$R/planning/multitool_2026-08-23/wanaka200_mt1_c12_rough_only_contour_parallel.toml
cd $WT
echo "worktree $(git log --oneline -1 | cut -c1-9), binary $(stat -c %y target/release/rs_cam_cli | cut -c1-19)"
run() { fx=$1; proj=$2; shift 2; d=$F/$fx/contour_parallel; rm -rf $d; mkdir -p $d; s=$(date +%s)
  env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM timeout 900 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $proj --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact "$@" > $d/stdout.txt 2> $d/stderr.txt
  echo "$fx contour_parallel rc=$? $(( $(date +%s)-s ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; df -h / | tail -1 | awk '{print "  disk free",$4}'; }
run corne $C --setup 0 --skip 4
run wanaka200 $W --setup 0 --resolution 1.0
echo RUNSDONE
