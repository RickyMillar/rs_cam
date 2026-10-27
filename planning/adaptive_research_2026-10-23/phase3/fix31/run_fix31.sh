#!/usr/bin/env bash
# D45: Corne, tree 7bf7738a1 (D43 7c17dcb08 + D44), the C12 job files phase3/d44/k_body_cp_edges_s{1p2,2p1}_d44_rest_regions.toml
# (fix27's files with op 5's boundary = DerivedRestRegions of toolpath 1). A: default split; B: RS_CAM_RESEARCH_STEP_SPLIT=stock_depth.
# No export override. Each run is analysed, then its simulation.json is deleted.
set -u
R=/home/ricky/personal_repos/rs_cam
A=$R/planning/adaptive_research_2026-10-23
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$A/phase3/fix31
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 7bf7738a1 ] && [ -z "$(git status --short)" ] && echo "tree = clean 7bf7738a1" || { echo "TREE NOT AS EXPECTED"; exit 1; }
for name in A_s1p2 A_s2p1 B_s1p2 B_s2p1; do
  s=${name#*_}; split=""; [ ${name%%_*} = B ] && split="RS_CAM_RESEARCH_STEP_SPLIT=stock_depth"
  d=$F/$name; rm -rf $d; mkdir -p $d; t=$(date +%s)
  env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD -u RS_CAM_RESEARCH_KEEP_DOWN_ROUTE -u RS_CAM_RESEARCH_STEP_SPLIT RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_spiral_pick $split RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug timeout 1500 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $A/phase3/d44/k_body_cp_edges_${s}_d44_rest_regions.toml --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact --setup 0 --skip 4 > $d/stdout.txt 2> $d/stderr.txt
  echo "$name rc=$? $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
  grep -E "refused|OVERRIDE" $d/stderr.txt | head -3 | sed 's/^/  /'
  cd $F
  python3 -I tpsplit.py fix31/$name/simulation.json > tpsplit_$name.txt 2>&1 || python3 -I tpsplit.py $d/simulation.json > tpsplit_$name.txt 2>&1
  if [ -f $d/program.nc ]; then
    for tp in "1|3D Rough 1" "5|3D Rough 1 [edges, CP rest]" "3|3D Rough 1 [copy]"; do IFS='|' read k op <<< "$tp"; python3 -I ../fix26/three_clocks.py fix31/$name $k "$op" 4000 > clocks_${name}_tp$k.json 2>&1; done
    python3 -I ../fix25/audit_d34.py ../fix31/$name --op "3D Rough 1" --tp "tp_1_*.json" --nominal-mrr 182.25 --json audit_${name}_tp1.json > audit_${name}_tp1.txt 2>&1
    python3 -I ../fix25/audit_d34.py ../fix31/$name --op "3D Rough 1 [edges, CP rest]" --tp "tp_5_*.json" --nominal-mrr 182.25 --json audit_${name}_tp5.json > audit_${name}_tp5.txt 2>&1
  fi
  python3 -I descent_causes.py fix31/$name $name > /dev/null 2>descent_$name.err
  python3 -I largez_landings.py fix31/$name > /dev/null
  AREA_SCREEN=0 python3 -I area_oracle.py fix31/$name corne 1
  python3 -I op5_analysis.py fix31/$name $name > /dev/null 2>op5_$name.err
  grep "index=0" $d/stdout_plain.txt | grep -E "C22 split entries|C22 2D re-entry cause" | sed -E 's/^.*rs_cam_core::[a-z_:0-9]+: //' > c22_lines_$name.txt
  ls op5_$name.json descent_causes_$name.json area_fix31_$name.npz > /dev/null 2>&1 && rm -f $d/simulation.json || echo "  $name: analysis incomplete, simulation.json kept"
  cd $WT
  df -h / | tail -1 | awk '{print "  disk free",$4}'
done
echo D45RUNSDONE
