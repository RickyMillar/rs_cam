#!/usr/bin/env bash
# D47: wanaka200, tree 7bf7738a1 (D43 + D44), RS_CAM_RESEARCH_STEP_SPLIT=stock_depth, arm geodesic_local_spiral_pick,
# --resolution 0.6, job files fix33/jobs/wanaka200_body_cp_edges_d44_s{2p0,3p5}.toml (toolpath 7 on DerivedRestRegions of 1).
# The flags are fix31's (incl. --accept-unmodeled-tool-load, the export override). Each run is analysed, then its simulation.json is deleted.
# Then: the CP-alone control (fix25/cp_alone_0.6, kept sim) is read with the full area oracle and the band method; the renders; the load table.
set -u
R=/home/ricky/personal_repos/rs_cam
A=$R/planning/adaptive_research_2026-10-23
WT=$R/.claude/worktrees/fixture-cdc9814e
F=$A/phase3/fix33
cd $WT
[ "$(git rev-parse --short=9 HEAD)" = 7bf7738a1 ] && [ -z "$(git status --short)" ] && echo "tree = clean 7bf7738a1" || { echo "TREE NOT AS EXPECTED"; exit 1; }
for s in s2p0 s3p5; do
  d=$F/$s; rm -rf $d; mkdir -p $d; t=$(date +%s)
  env -u RS_CAM_RESEARCH_ADAPTIVE3D_ARM -u RS_CAM_RESEARCH_SLAB_FIELD -u RS_CAM_RESEARCH_KEEP_DOWN_ROUTE -u RS_CAM_RESEARCH_STEP_SPLIT RS_CAM_RESEARCH_ADAPTIVE3D_ARM=geodesic_local_spiral_pick RS_CAM_RESEARCH_STEP_SPLIT=stock_depth RUST_LOG=info,rs_cam_core::adaptive3d::clearing=debug timeout 1500 scripts/cargo_lane.sh run --release -q -p rs_cam_cli --features rs_cam_core/research -- project $F/jobs/wanaka200_body_cp_edges_d44_${s}.toml --output-dir $d --summary --emit-gcode $d/program.nc --accept-unmodeled-tool-load --sim-artifact --setup 0 --resolution 0.6 > $d/stdout.txt 2> $d/stderr.txt
  echo "$s rc=$? $(( $(date +%s)-t ))s; $(grep -o 'Toolpaths:.*' $d/stderr.txt | head -1)"; sed -E 's/\x1b\[[0-9;]*m//g' $d/stdout.txt > $d/stdout_plain.txt
  grep -E "refused|OVERRIDE" $d/stderr.txt | head -3 | sed 's/^/  /'
  cd $F
  python3 -I tpsplit.py $d/simulation.json > tpsplit_$s.txt 2>&1
  if [ -f $d/program.nc ]; then
    for tp in "1|2 Back Rough" "7|2 Back Rough [edges, CP rest]"; do IFS='|' read k op <<< "$tp"; python3 -I ../fix26/three_clocks.py fix33/$s $k "$op" 10000 > clocks_${s}_tp$k.json 2>&1; done
    python3 -I ../fix25/audit_d34.py ../fix33/$s --op "2 Back Rough" --tp "tp_1_*.json" --nominal-mrr 182.0 --json audit_${s}_tp1.json > audit_${s}_tp1.txt 2>&1
    python3 -I ../fix25/audit_d34.py ../fix33/$s --op "2 Back Rough [edges, CP rest]" --tp "tp_7_*.json" --nominal-mrr 182.0 --json audit_${s}_tp7.json > audit_${s}_tp7.txt 2>&1
  fi
  python3 -I descent_causes.py fix33/$s $s > /dev/null 2>descent_$s.err
  python3 -I largez_landings.py fix33/$s > /dev/null
  AREA_SCREEN=0 python3 -I area_oracle.py fix33/$s wanaka200 1
  python3 -I op7_wanaka.py fix33/$s $s 7 > /dev/null 2>op7_$s.err
  grep "index=0" $d/stdout_plain.txt | grep -E "C22 split entries|C22 2D re-entry cause" | sed -E 's/^.*rs_cam_core::[a-z_:0-9]+: //' > c22_lines_$s.txt
  ls op7_$s.json descent_causes_$s.json area_fix33_$s.npz clocks_${s}_tp1.json > /dev/null 2>&1 && rm -f $d/simulation.json || echo "  $s: analysis incomplete, simulation.json kept"
  cd $WT
  df -h / | tail -1 | awk '{print "  disk free",$4}'
done
cd $F
echo "CP control: full area read"
AREA_SCREEN=0 python3 -I area_oracle.py fix25/cp_alone_0.6/wanaka200 wanaka200 1
python3 -I load_table.py > load_table.txt 2>&1
python3 -I render_d47.py > render.txt 2>&1
echo D47RUNSDONE
