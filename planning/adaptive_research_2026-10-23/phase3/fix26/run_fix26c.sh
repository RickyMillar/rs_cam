#!/usr/bin/env bash
# D35 extra run: fix6's CP job + the wanaka kinematics block + the waterline enabled = false
# (the copy op is the last simulated op), clean 7b7a8df6c. Shows whether the integrator clock
# escapes G-SIMFEED.
set -u
R=/home/ricky/personal_repos/rs_cam
A=$R/planning/adaptive_research_2026-10-23
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$A/phase3/fix26
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 7b7a8df6c ] && [ -z "$(git status --short)" ] && echo "tree = clean 7b7a8df6c" || { echo "TREE NOT AS EXPECTED"; exit 1; }
d=$F/k_cp_fix6job_wl_off; rm -rf $d; mkdir -p $d; s=$(date +%s)
env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug timeout 1500 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $F/jobs/k_contour_parallel_wl_off.toml --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact --setup 0 --skip 4 > $d/stdout.txt 2> $d/stderr.txt
echo "k_cp_fix6job_wl_off rc=$? $(( $(date +%s)-s ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
echo RUNSDONE
