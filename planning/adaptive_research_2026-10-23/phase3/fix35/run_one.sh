#!/usr/bin/env bash
# D51 (copy of fix34/run_one.sh, adds wanaka_s2p0; writes into fix35). D49: one run on tree 453d52571 (G-RESTSTALE fix 13caf4b29 + G-RESTFRAME fix). Args: <wanaka_s3p5|corne_B_s2p1> [override].
# No export override unless the second argument is "override" (then the run dir gets the suffix _ovr; measure only).
# wanaka_s3p5: fix33/jobs/wanaka200_body_cp_edges_d44_s3p5.toml, --setup 0 --resolution 0.6 (as fix33).
# corne_B_s2p1: d44/k_body_cp_edges_s2p1_d44_rest_regions.toml, --setup 0 --skip 4 (as fix31 B_s2p1).
# Both: arm geodesic_local_spiral_pick, RS_CAM_RESEARCH_STEP_SPLIT=stock_depth.
set -u
R=/home/ricky/personal_repos/rs_cam
A=$R/planning/adaptive_research_2026-10-23
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$A/phase3/fix35
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 453d52571 ] && [ -z "$(git status --short)" ] && echo "tree = clean 453d52571" || { echo "TREE NOT AS EXPECTED"; exit 1; }
name=$1; ovr=""; [ "${2:-}" = override ] && { ovr="--accept-unmodeled-tool-load"; name=${name}_ovr; }
case $1 in
  wanaka_s2p0) job=$A/phase3/fix33/jobs/wanaka200_body_cp_edges_d44_s2p0.toml; extra="--setup 0 --resolution 0.6";;
  wanaka_s3p5) job=$A/phase3/fix33/jobs/wanaka200_body_cp_edges_d44_s3p5.toml; extra="--setup 0 --resolution 0.6";;
  corne_B_s2p1) job=$A/phase3/d44/k_body_cp_edges_s2p1_d44_rest_regions.toml; extra="--setup 0 --skip 4";;
esac
d=$F/$name; rm -rf $d; mkdir -p $d; t=$(date +%s)
env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD -u RS_CAM_RESEARCH_KEEP_DOWN_ROUTE -u RS_CAM_RESEARCH_STEP_SPLIT RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_spiral_pick RS_CAM_RESEARCH_STEP_SPLIT=stock_depth RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug timeout 560 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $job --output-dir $d --summary --emit-gcode $d/program.nc $ovr --sim-artifact $extra > $d/stdout.txt 2> $d/stderr.txt
echo "$name rc=$? $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
grep -iE "refused|OVERRIDE|error" $d/stderr.txt | head -5 | sed 's/^/  /'; ls $d
