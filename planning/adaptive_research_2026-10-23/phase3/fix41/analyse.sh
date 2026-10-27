#!/usr/bin/env bash
# D60 analysis of fix41/<run> (corne_* or wanaka_*); deletes the simulation.json at the end.
set -u; F=/home/ricky/personal_repos/rs_cam/planning/adaptive_research_2026-10-23/phase3/fix41; cd $F; s=$1; d=$F/$s
python3 -I tpsplit.py $d/simulation.json > tpsplit_$s.txt 2>&1
case $s in
 corne*) for tp in "1|3D Rough 1" "5|3D Rough 1 [edges, CP rest]" "3|3D Rough 1 [copy]"; do IFS='|' read k op <<< "$tp"; python3 -I ../fix26/three_clocks.py fix41/$s $k "$op" 4000 > clocks_${s}_tp$k.json 2>&1; done
   python3 -I corne_op5.py fix41/$s $s > /dev/null 2>op5_$s.err; python3 -I corne_reach.py $d/program.nc > corne_reach_$s.txt 2>&1; ok="op5_$s.json";;
 wanaka*) for tp in "1|2 Back Rough" "7|2 Back Rough [edges, CP rest]"; do IFS='|' read k op <<< "$tp"; python3 -I ../fix26/three_clocks.py fix41/$s $k "$op" 10000 > clocks_${s}_tp$k.json 2>&1; done
   python3 -I op7_wanaka.py fix41/$s $s 7 > /dev/null 2>op7_$s.err; python3 -I op7_gcode.py $d/program.nc > op7_gcode_$s.txt 2>&1; ok="op7_$s.json";;
esac
python3 -I cap_audit.py $d $s > /dev/null 2>cap_$s.err
ls $ok cap_audit_$s.json clocks_${s}_tp1.json > /dev/null && rm -f $d/simulation.json && echo "$s analysed, simulation.json deleted" || echo "$s: incomplete, sim kept"
