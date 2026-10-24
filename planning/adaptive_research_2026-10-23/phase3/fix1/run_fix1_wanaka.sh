#!/usr/bin/env bash
# D-FIX1 wanaka200 runs (operator approved a 900 s timeout per run), through
# the lane script with `cargo run --release` in the clean cdc9814e worktree.
# The full-resolution simulation ran past 900 s, so the sim cell is 1.0 mm.
set -u
R=/home/ricky/personal_repos/rs_cam
WT=$R/.claude/worktrees/fixture-cdc9814e
O=$R/planning/adaptive_research_2026-10-23/phase3/fix1
W=$R/planning/multitool_2026-08-23/wanaka200_mt1_c12_agent_search.toml
cd $WT
run() { arm=$1; d=$O/wanaka200/$arm; rm -rf $d; mkdir -p $d; s=$(date +%s)
  args=(run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $W --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --resolution 1.0 --setup 0 --skip 0,2,3,4)
  if [ "$arm" = agent ]; then env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM timeout 900 scripts/cargo_lane.sh "${args[@]}" > $d/stdout.txt 2> $d/stderr.txt
  else RS_CAM_RESEARCH_ADAPTIVE3D_ARM=$arm timeout 900 scripts/cargo_lane.sh "${args[@]}" > $d/stdout.txt 2> $d/stderr.txt; fi
  echo "wanaka200 $arm rc=$? $(( $(date +%s)-s ))s"; }
run agent
run geodesic_curv
echo RUNSDONE
