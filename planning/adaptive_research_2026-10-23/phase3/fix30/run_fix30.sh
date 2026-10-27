#!/usr/bin/env bash
# D42: detect_flat_areas = true on op 1 only (jobs/*_flat*.toml; from fix26/fix27 k_ files), Corne, clean 7b7a8df6c.
# geo_s2p1 is refused at export (copy op deflection=stiffness); it is rerun with EXTRA=--accept-exceeded-tool-load
# for the measurement only (the G-code is never machined).
set -u
R=/home/ricky/personal_repos/rs_cam
A=$R/planning/adaptive_research_2026-10-23
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$A/phase3/fix30
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 7b7a8df6c ] && [ -z "$(git status --short)" ] && echo "tree = clean 7b7a8df6c" || { echo "TREE NOT AS EXPECTED"; exit 1; }
for name in ${ONLY:-geo_s1p2 geo_s2p1 cp}; do
  case $name in geo_s1p2) job=k_body_cp_edges_flat_s1p2;; geo_s2p1) job=k_body_cp_edges_flat_s2p1;; cp) job=k_contour_parallel_flat;; esac
  arm="RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_spiral_pick"; [ $name = cp ] && arm=""
  d=$F/$name; rm -rf $d; mkdir -p $d; t=$(date +%s)
  env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD -u RS_CAM_RESEARCH_KEEP_DOWN_ROUTE $arm RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug timeout 1500 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $F/jobs/$job.toml --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load $EXTRA --sim-artifact --setup 0 --skip 4 > $d/stdout.txt 2> $d/stderr.txt
  echo "$name rc=$? $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
done
echo D42RUNSDONE
