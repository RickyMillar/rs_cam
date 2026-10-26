#!/usr/bin/env bash
# D21c step 12: the fixtures with C35 at 3cc257c1f (adaptive-head; the
# commit equals the D21c diff), arm
# geodesic_local_spiral_pick, as D20 (fix7): RUST_LOG=info, --sim-artifact,
# timeout 900, lane script. Arg 1: corne | wanaka200.
set -u
R=/home/ricky/personal_repos/rs_cam
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$R/planning/adaptive_research_2026-10-23/phase3/fix12
C=$R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_agent_search.toml
W=$R/planning/multitool_2026-08-23/wanaka200_mt1_c12_rough_only.toml
cd $WT
# Runs in the fixture worktree, checked out at 3cc257c1f (adaptive-head
# took new C38 work during the first Corne run; that run is in corne_adaptive_head/).
# The lead committed the D21c diff as 3cc257c1f (git diff b37cad1a4 3cc257c1f equals d21c/start.diff).
[ "$(git rev-parse --short=9 HEAD)" = 3cc257c1f ] && [ -z "$(git status --short)" ] && echo "HEAD 3cc257c1f, clean" || { echo "HEAD or tree not as expected"; git log --oneline -1; git status --short; exit 1; }
arm=geodesic_local_spiral_pick
run() { fx=$1; proj=$2; shift 2; d=$F/$fx; rm -rf $d; mkdir -p $d; s=$(date +%s)
  RUST_LOG=info RS_CAM_RESEARCH_ADAPTIVE3D_ARM=$arm timeout 900 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $proj --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact "$@" > $d/stdout.txt 2> $d/stderr.txt
  echo "$fx $arm rc=$? $(( $(date +%s)-s ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; echo "  binary $(stat -c %y target/release/rs_cam_cli | cut -c1-19), stamp $(strings target/release/rs_cam_cli | grep -o '0\.1\.0[0-9a-f]\{9\}' | head -1)"; df -h / | tail -1 | awk '{print "  disk free",$4}'; }
case $1 in
  corne) run corne $C --setup 0 --skip 4 ;;
  wanaka200) run wanaka200 $W --setup 0 --resolution 1.0 ;;
esac
echo RUNDONE
