#!/usr/bin/env bash
# D49 analysis of one run under fix34 (args: <run> <wanaka|corne>). Then the run's simulation.json is deleted.
set -u
F=/home/ricky/personal_repos/rs_cam/planning/adaptive_research_2026-10-23/phase3/fix34; cd $F; s=$1; fx=$2; d=$F/$s
python3 -I tpsplit.py $d/simulation.json > tpsplit_$s.txt 2>&1
if [ $fx = wanaka ]; then
  for tp in "1|2 Back Rough" "7|2 Back Rough [edges, CP rest]"; do IFS='|' read k op <<< "$tp"; python3 -I ../fix26/three_clocks.py fix34/$s $k "$op" 10000 > clocks_${s}_tp$k.json 2>&1; done
  python3 -I ../fix25/audit_d34.py ../fix34/$s --op "2 Back Rough" --tp "tp_1_*.json" --nominal-mrr 182.0 --json audit_${s}_tp1.json > audit_${s}_tp1.txt 2>&1
  python3 -I descent_causes.py fix34/$s $s > /dev/null 2>descent_$s.err
  AREA_SCREEN=0 python3 -I area_oracle.py fix34/$s wanaka200 1
  python3 -I op7_wanaka.py fix34/$s $s 7 > /dev/null 2>op7_$s.err
  python3 -I op7_gcode.py $d/program.nc > op7_gcode_$s.txt 2>&1
  ok="op7_$s.json descent_causes_$s.json area_fix34_$s.npz"
else
  for tp in "1|3D Rough 1" "5|3D Rough 1 [edges, CP rest]" "3|3D Rough 1 [copy]"; do IFS='|' read k op <<< "$tp"; python3 -I ../fix26/three_clocks.py fix34/$s $k "$op" 4000 > clocks_${s}_tp$k.json 2>&1; done
  python3 -I ../fix25/audit_d34.py ../fix34/$s --op "3D Rough 1" --tp "tp_1_*.json" --nominal-mrr 182.25 --json audit_${s}_tp1.json > audit_${s}_tp1.txt 2>&1
  python3 -I ../fix25/audit_d34.py ../fix34/$s --op "3D Rough 1 [edges, CP rest]" --tp "tp_5_*.json" --nominal-mrr 182.25 --json audit_${s}_tp5.json > audit_${s}_tp5.txt 2>&1
  python3 -I corne_descent_causes.py fix34/$s $s > /dev/null 2>descent_$s.err
  AREA_SCREEN=0 python3 -I area_oracle.py fix34/$s corne 1
  python3 -I corne_op5.py fix34/$s $s > /dev/null 2>op5_$s.err
  ok="op5_$s.json descent_causes_$s.json area_fix34_$s.npz"
fi
python3 -I largez_landings.py fix34/$s > /dev/null
grep "index=0" $d/stdout_plain.txt | grep -E "C22 split entries|C22 2D re-entry cause" | sed -E 's/^.*rs_cam_core::[a-z_:0-9]+: //' > c22_lines_$s.txt
ls $ok > /dev/null && rm -f $d/simulation.json && echo "$s analysed, simulation.json deleted" || echo "$s: analysis incomplete, simulation.json kept"
