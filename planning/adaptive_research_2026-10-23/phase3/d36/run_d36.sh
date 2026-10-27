#!/usr/bin/env bash
# D36: C22 §18.9 slab read counter. Fixture worktree: clean 7b7a8df6c + d36/tree_vs_7b7a8df6c_counter.patch
# (md5 b4ab9db4769f46172c7980db60b86c50). fix25's body_cp_field Corne job (field on), fix25 flags.
# The worktree is restored to clean 7b7a8df6c at the end.
set -u
R=/home/ricky/personal_repos/rs_cam
A=$R/planning/adaptive_research_2026-10-23
WT=$R/.claude/worktrees/fixture-cdc9814e
D=$A/phase3/d36
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 7b7a8df6c ] && [ -z "$(git status --short)" ] || { echo "TREE NOT CLEAN 7b7a8df6c"; exit 1; }
[ "$(md5sum < $D/tree_vs_7b7a8df6c_counter.patch | cut -d' ' -f1)" = b4ab9db4769f46172c7980db60b86c50 ] || { echo "PATCH MD5 WRONG"; exit 1; }
git apply $D/tree_vs_7b7a8df6c_counter.patch && git diff > $D/applied.diff && echo "applied: $(git diff --stat | tail -1)"
d=$D/field_corne; rm -rf $d; mkdir -p $d; s=$(date +%s)
env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_spiral_pick RS_CAM_RESEARCH_SLAB_FIELD=on RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug timeout 1500 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $R/planning/corne_case_analysis_2026-09-18/corne_job_corrected_c12_body_cp_edges.toml --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact --setup 0 --skip 4 > $d/stdout.txt 2> $d/stderr.txt
echo "d36 field_corne rc=$? $(( $(date +%s)-s ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
cmp $d/program.nc $A/phase3/fix25/body_cp_field/corne/program.nc && echo "program.nc == fix25 body_cp_field" || echo "PROGRAM DIFFERS"
git checkout -- . && git status --short && echo "restored: $(git rev-parse --short=9 HEAD), $(git status --short | wc -l) changes"
echo D36DONE
