#!/usr/bin/env bash
# D34 cycle check: the fix6 CP job file (corne_job_corrected_c12_contour_parallel.toml)
# on the D34 tree (3181ae54f + d34 patch), the same flags as fix6. If the cycle is
# 1564 s, the tree moved fix6's 2022 s; if it is 2022 s, the job file did.
set -u
R=/home/ricky/personal_repos/rs_cam
A=$R/planning/adaptive_research_2026-10-23
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$A/phase3/fix25
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 3181ae54f ] && git diff | cmp -s - $A/phase3/d34/applied.diff && echo "tree = 3181ae54f + D34" || { echo "TREE NOT AS EXPECTED"; exit 1; }
d=$F/cp_fix6job/corne; rm -rf $d; mkdir -p $d; s=$(date +%s)
env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD timeout 1500 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_contour_parallel.toml --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact --setup 0 --skip 4 > $d/stdout.txt 2> $d/stderr.txt
echo "cp_fix6job corne rc=$? $(( $(date +%s)-s ))s"
df -h / | tail -1 | awk '{print "  disk free",$4}'
echo CPJOBDONE
