#!/usr/bin/env bash
# The page runs again with --sim-artifact (the per-sample engagement), the
# same binary (31935b89) and flags; program.nc must be byte-equal to the
# page's run, so the plans do not change.
set -u
R=/home/ricky/personal_repos/rs_cam
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$R/planning/adaptive_research_2026-10-23/phase3
C=$R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_agent_search.toml
W=$R/planning/multitool_2026-08-23/wanaka200_mt1_c12_rough_only.toml
cd $WT
run() { fx=$1; arm=$2; ref=$3; proj=$4; shift 4; d=$F/fix2/simart/$fx/$arm; rm -rf $d; mkdir -p $d; s=$(date +%s)
  args=(run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $proj --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact "$@")
  if [ "$arm" = agent ]; then env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM timeout 900 scripts/cargo_lane.sh "${args[@]}" > $d/stdout.txt 2> $d/stderr.txt
  else RS_CAM_RESEARCH_ADAPTIVE3D_ARM=$arm timeout 900 scripts/cargo_lane.sh "${args[@]}" > $d/stdout.txt 2> $d/stderr.txt; fi
  rc=$?; cmp -s $d/program.nc $ref/program.nc && same=same || same=DIFFERENT
  echo "$fx $arm rc=$rc $(( $(date +%s)-s ))s program.nc $same as $ref; sim.json $(du -h $d/simulation.json 2>/dev/null | cut -f1)"; df -h / | tail -1 | awk '{print "  disk free",$4}'; }
run corne agent $F/fix1/corne/agent $C --setup 0 --skip 4
run corne geodesic_local_zigzag $F/fix2/corne/geodesic_local_zigzag $C --setup 0 --skip 4
run corne geodesic_local_oneway $F/fix2/corne/geodesic_local_oneway $C --setup 0 --skip 4
run wanaka200 agent $F/fix2/wanaka200/agent $W --setup 0 --resolution 1.0
run wanaka200 geodesic_local_zigzag $F/fix2/wanaka200/geodesic_local_zigzag $W --setup 0 --resolution 1.0
echo RUNSDONE
