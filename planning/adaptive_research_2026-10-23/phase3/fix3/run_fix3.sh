#!/usr/bin/env bash
# D16c fixture re-runs at 6ce373922 (C26: chord cap, held links as Link):
# Corne zigzag, zigzag_v2 and one-way, --sim-artifact, the D-FIX2 flags,
# timeout 900 each, through the lane script.
set -u
R=/home/ricky/personal_repos/rs_cam
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$R/planning/adaptive_research_2026-10-23/phase3/fix3
C=$R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_agent_search.toml
cd $WT
timeout 1800 scripts/cargo_lane.sh build --release -q -p rs_cam_cli --features rs_cam_core/research > $F/build.log 2>&1; echo "build rc=$? $(git log --oneline -1 | cut -c1-9) $(stat -c %y target/release/rs_cam_cli | cut -c1-19)"
for arm in geodesic_local_zigzag geodesic_local_zigzag_v2 geodesic_local_oneway; do
  d=$F/corne/$arm; rm -rf $d; mkdir -p $d; s=$(date +%s)
  RS_CAM_RESEARCH_ADAPTIVE3D_ARM=$arm timeout 900 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $C --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact --setup 0 --skip 4 > $d/stdout.txt 2> $d/stderr.txt
  echo "corne $arm rc=$? $(( $(date +%s)-s ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; df -h / | tail -1 | awk '{print "  disk free",$4}'
done
echo RUNSDONE
