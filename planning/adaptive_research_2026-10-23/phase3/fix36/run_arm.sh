#!/usr/bin/env bash
# D52 (Q8): one run on tree 453d52571, no export override, RS_CAM_RESEARCH_STEP_SPLIT=stock_depth.
# Args: <wanaka|corne> <arm: geodesic_local_spiral_pick (C35, pick on) | geodesic_local_spiral_headon (C37, pick off, plan B)> <tag>
# wanaka: fix33/jobs/wanaka200_body_cp_edges_d44_s2p0.toml --setup 0 --resolution 0.6; corne: d44/k_body_cp_edges_s2p1_d44_rest_regions.toml --setup 0 --skip 4.
# Then the analysis; the simulation.json is deleted at the end.
set -u
R=/home/ricky/personal_repos/rs_cam; A=$R/planning/adaptive_research_2026-10-23; WT=$R/.claude/worktrees/fixture-cdc9814e; F=$A/phase3/fix36
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 453d52571 ] && [ -z "$(git status --short)" ] && echo "tree = clean 453d52571" || { echo "TREE NOT AS EXPECTED"; exit 1; }
fx=$1; arm=$2; s=$3
case $fx in
  wanaka) job=$A/phase3/fix33/jobs/wanaka200_body_cp_edges_d44_s2p0.toml; extra="--setup 0 --resolution 0.6"; e="7|2 Back Rough [edges, CP rest]"; o1="2 Back Rough"; rap=10000; part=wanaka200;;
  corne) job=$A/phase3/d44/k_body_cp_edges_s2p1_d44_rest_regions.toml; extra="--setup 0 --skip 4"; e="5|3D Rough 1 [edges, CP rest]"; o1="3D Rough 1"; rap=4000; part=corne;;
esac
d=$F/$s; rm -rf $d; mkdir -p $d; t=$(date +%s)
env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD -u RS_CAM_RESEARCH_KEEP_DOWN_ROUTE -u RS_CAM_RESEARCH_STEP_SPLIT RS_CAM_RESEARCH_ADAPTIVE3D_ARM=$arm RS_CAM_RESEARCH_STEP_SPLIT=stock_depth RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug timeout 500 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $job --output-dir $d --summary --emit-gcode $d/program.nc --sim-artifact $extra > $d/stdout.txt 2> $d/stderr.txt
echo "$s rc=$? $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
echo "  refusals: $(grep -ciE 'refused' $d/stderr.txt)"
cd $F
python3 -I tpsplit.py $d/simulation.json > tpsplit_$s.txt 2>&1
IFS='|' read k op <<< "$e"
python3 -I ../fix26/three_clocks.py fix36/$s 1 "$o1" $rap > clocks_${s}_tp1.json 2>&1
python3 -I ../fix26/three_clocks.py fix36/$s $k "$op" $rap > clocks_${s}_tp$k.json 2>&1
python3 -I ../fix25/audit_d34.py ../fix36/$s --op "$o1" --tp "tp_1_*.json" --nominal-mrr 182.0 --json audit_${s}_tp1.json > audit_${s}_tp1.txt 2>&1
AREA_SCREEN=0 python3 -I area_oracle.py fix36/$s $part 1
ls area_fix36_$s.npz clocks_${s}_tp1.json > /dev/null && rm -f $d/simulation.json && echo "  $s analysed, simulation.json deleted" || echo "  $s: analysis incomplete, simulation.json kept"
