#!/usr/bin/env bash
# D18 fixture run at f32401326 (C29: the 3D-scored pick): Corne
# geodesic_local_zigzag_pick, RUST_LOG=info, --sim-artifact, the D-FIX2 flags,
# timeout 900, through the lane script.
set -u
R=/home/ricky/personal_repos/rs_cam
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$R/planning/adaptive_research_2026-10-23/phase3/fix5
C=$R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_agent_search.toml
cd $WT
timeout 1800 scripts/cargo_lane.sh build --release -q -p rs_cam_cli --features rs_cam_core/research > $F/build.log 2>&1; echo "build rc=$? $(git log --oneline -1 | cut -c1-9) $(stat -c %y target/release/rs_cam_cli | cut -c1-19)"
arm=geodesic_local_zigzag_pick; d=$F/corne/$arm; rm -rf $d; mkdir -p $d; s=$(date +%s)
RUST_LOG=info RS_CAM_RESEARCH_ADAPTIVE3D_ARM=$arm timeout 900 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $C --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact --setup 0 --skip 4 > $d/stdout.txt 2> $d/stderr.txt
echo "corne $arm rc=$? $(( $(date +%s)-s ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; df -h / | tail -1 | awk '{print "  disk free",$4}'
echo RUNSDONE
