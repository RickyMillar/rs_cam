#!/usr/bin/env bash
# D57 analysis of fix38/<A|B>; deletes the simulation.json at the end.
set -u; F=/home/ricky/personal_repos/rs_cam/planning/adaptive_research_2026-10-23/phase3/fix38; cd $F; s=$1; d=$F/$s
python3 -I tpsplit.py $d/simulation.json > tpsplit_$s.txt 2>&1
for tp in "1|2 Back Rough" "7|2 Back Rough [edges, CP rest]"; do IFS='|' read k op <<< "$tp"; python3 -I ../fix26/three_clocks.py fix38/$s $k "$op" 10000 > clocks_${s}_tp$k.json 2>&1; done
python3 -I op7_wanaka.py fix38/$s $s 7 > /dev/null 2>op7_$s.err
python3 -I op7_gcode.py $d/program.nc > op7_gcode_$s.txt 2>&1
python3 -I entry_floor.py fix38/$s $s > /dev/null 2>entry_floor_$s.err
ls op7_$s.json entry_floor_$s.json clocks_${s}_tp7.json > /dev/null && rm -f $d/simulation.json && echo "$s analysed, simulation.json deleted" || echo "$s: incomplete, sim kept"
