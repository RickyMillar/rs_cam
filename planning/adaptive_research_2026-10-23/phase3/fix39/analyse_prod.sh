#!/usr/bin/env bash
# D58 analysis of fix39/<run> against R's op 1 (args: <run> <R program.nc under phase3>). Deletes the simulation.json.
set -u; F=/home/ricky/personal_repos/rs_cam/planning/adaptive_research_2026-10-23/phase3/fix39; cd $F; s=$1; Rnc=$2; d=$F/$s
case $s in corne*) o1="3D Rough 1"; rap=4000; part=corne; mrr=182.25;; wanaka*) o1="2 Back Rough"; rap=10000; part=wanaka200; mrr=182.0;; esac
python3 -I tpsplit.py $d/simulation.json > tpsplit_$s.txt 2>&1
python3 -I ../fix26/three_clocks.py fix39/$s 1 "$o1" $rap > clocks_${s}_tp1.json 2>&1
python3 -I ../fix25/audit_d34.py ../fix39/$s --op "$o1" --tp "tp_1_*.json" --nominal-mrr $mrr --json audit_${s}_tp1.json > audit_${s}_tp1.txt 2>&1
AREA_SCREEN=0 python3 -I area_oracle.py fix39/$s $part 1
python3 -I geom_diff.py ../$Rnc $d/program.nc "$o1" > geom_diff_$s.txt 2>&1
ls area_fix39_$s.npz clocks_${s}_tp1.json > /dev/null && rm -f $d/simulation.json && echo "$s analysed, simulation.json deleted" || echo "$s: incomplete, sim kept"
