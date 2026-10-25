#!/usr/bin/env bash
# D-FIX2 runs at 31935b89 (release CLI with rs_cam_core/research in the clean
# fixture worktree), through the lane script, timeout 900 each.
# Corne: the same flags as D-FIX1; the AgentSearch control is D-FIX1's.
# wanaka200: the rough-only copy, setup 0 (2 Back Rough), --resolution 1.0.
set -u
R=/home/ricky/personal_repos/rs_cam
WT=$R/.claude/worktrees/fixture-cdc9814e
O=$R/planning/adaptive_research_2026-10-23/phase3/fix2
C=$R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_agent_search.toml
W=$R/planning/multitool_2026-08-23/wanaka200_mt1_c12_rough_only.toml
cd $WT
run() { fx=$1; arm=$2; proj=$3; shift 3; d=$O/$fx/$arm; rm -rf $d; mkdir -p $d; s=$(date +%s)
  args=(run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $proj --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load "$@")
  if [ "$arm" = agent ]; then env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM timeout 900 scripts/cargo_lane.sh "${args[@]}" > $d/stdout.txt 2> $d/stderr.txt
  else RS_CAM_RESEARCH_ADAPTIVE3D_ARM=$arm timeout 900 scripts/cargo_lane.sh "${args[@]}" > $d/stdout.txt 2> $d/stderr.txt; fi
  echo "$fx $arm rc=$? $(( $(date +%s)-s ))s"; df -h / | tail -1 | awk '{print "  disk free",$4}'; }
run corne geodesic_local_zigzag $C --setup 0 --skip 4
run corne geodesic_local_oneway $C --setup 0 --skip 4
run wanaka200 agent $W --setup 0 --resolution 1.0
run wanaka200 geodesic_local_zigzag $W --setup 0 --resolution 1.0
echo RUNSDONE
