#!/usr/bin/env bash
# D-FIX1 runs: rough ops only, AgentSearch control and the geodesic_curv arm.
# Binary: the release CLI built with rs_cam_core/research at cdc9814e in a
# clean detached worktree.
set -u
R=/home/ricky/personal_repos/rs_cam
B=$R/.claude/worktrees/fixture-cdc9814e/target/release/rs_cam_cli
O=$R/planning/adaptive_research_2026-10-23/phase3/fix1
C=$R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_agent_search.toml
W=$R/planning/multitool_2026-08-23/wanaka200_mt1_c12_agent_search.toml
run() { fx=$1; arm=$2; proj=$3; shift 3; d=$O/$fx/$arm; mkdir -p $d; s=$(date +%s.%N)
  if [ "$arm" = agent ]; then env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM timeout 200 $B project $proj --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load "$@" > $d/stdout.txt 2> $d/stderr.txt
  else RS_CAM_RESEARCH_ADAPTIVE3D_ARM=$arm timeout 200 $B project $proj --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load "$@" > $d/stdout.txt 2> $d/stderr.txt; fi
  rc=$?; e=$(echo "$(date +%s.%N) - $s" | bc); echo "$fx $arm rc=$rc ${e}s"; }
run corne agent $C --setup 0 --skip 4
run corne geodesic_curv $C --setup 0 --skip 4
run wanaka200 agent $W --setup 0 --skip 0,2,3,4
run wanaka200 geodesic_curv $W --setup 0 --skip 0,2,3,4
echo RUNSDONE
