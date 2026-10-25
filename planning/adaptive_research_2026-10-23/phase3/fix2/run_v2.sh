#!/usr/bin/env bash
# D16b step 4: geodesic_local_zigzag_v2 at 4cceca914 with --sim-artifact
# (one run serves the page and the load share). D-FIX2 flags, timeout 900.
set -u
R=/home/ricky/personal_repos/rs_cam
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$R/planning/adaptive_research_2026-10-23/phase3/fix2
C=$R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_agent_search.toml
W=$R/planning/multitool_2026-08-23/wanaka200_mt1_c12_rough_only.toml
cd $WT
timeout 1800 scripts/cargo_lane.sh build --release -q -p rs_cam_cli --features rs_cam_core/research > $F/build_v2.log 2>&1; echo "build rc=$? $(git log --oneline -1 | cut -c1-9) $(stat -c %y target/release/rs_cam_cli | cut -c1-19)"
run() { fx=$1; proj=$2; shift 2; d=$F/$fx/geodesic_local_zigzag_v2; rm -rf $d; mkdir -p $d; s=$(date +%s)
  RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_zigzag_v2 timeout 900 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $proj --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact "$@" > $d/stdout.txt 2> $d/stderr.txt
  echo "$fx zigzag_v2 rc=$? $(( $(date +%s)-s ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; df -h / | tail -1 | awk '{print "  disk free",$4}'; }
run corne $C --setup 0 --skip 4
run wanaka200 $W --setup 0 --resolution 1.0
echo RUNSDONE
