#!/usr/bin/env bash
# D35 part b, fixture worktree at clean 7b7a8df6c:
# 1. the field-on runs again (fix25 flags; Corne and wanaka200 at 0.6) for the CP rest op audit with the fixed line map;
# 2. a sensitivity check (a hypothetical machine): Corne body_cp and fix6's CP job with the wanaka200
#    machine's [job.machine.kinematics] block (Shapeoko XXL, accel 500/500/270, jd 0.02), so the summary
#    clock includes acceleration.
set -u
R=/home/ricky/personal_repos/rs_cam
A=$R/planning/adaptive_research_2026-10-23
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$A/phase3/fix26
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 7b7a8df6c ] && [ -z "$(git status --short)" ] && echo "tree = clean 7b7a8df6c" || { echo "TREE NOT AS EXPECTED"; exit 1; }
run() { name=$1; proj=$2; envs=$3; extra=$4; d=$F/$name; rm -rf $d; mkdir -p $d; s=$(date +%s)
  env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD $envs RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug timeout 1500 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $proj --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact $extra > $d/stdout.txt 2> $d/stderr.txt
  echo "$name rc=$? $(( $(date +%s)-s ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt; df -h / | tail -1 | awk '{print "  disk free",$4}'; }
ARM="RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_spiral_pick"
run field_corne $R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_body_cp_edges.toml "$ARM RS_CAM_RESEARCH_SLAB_FIELD=on" "--setup 0 --skip 4"
run field_wanaka200 $R/planning/multitool_2026-08-23/wanaka200_mt1_c12_rough_only_body_cp_edges.toml "$ARM RS_CAM_RESEARCH_SLAB_FIELD=on" "--setup 0 --resolution 0.6"
run k_body_cp_corne $F/jobs/k_body_cp_edges.toml "$ARM" "--setup 0 --skip 4"
run k_cp_fix6job_corne $F/jobs/k_contour_parallel.toml "" "--setup 0 --skip 4"
echo RUNSDONE
