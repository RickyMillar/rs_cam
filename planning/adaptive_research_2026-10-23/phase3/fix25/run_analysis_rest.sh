#!/usr/bin/env bash
# D34: the audit (fix11/audit.py) per op, then entries_d34.py and c39_d34.py.
cd "$(dirname "$0")"
A=audit_d34.py
au() { run=$1; op=$2; tp=$3; nom=$4; t=${tp#tp_}; out=audit_$(echo ${run#fix25/} | tr / _)_tp${t%%_*}; python3 -I $A ../$run --op "$op" --tp "$tp" --nominal-mrr $nom --json $out.json > $out.txt 2>&1; echo "audit $run $op rc=$?"; }
for c in body_cp body_cp_field; do au fix25/$c/corne "3D Rough 1 [edges, CP rest]" "tp_5_*.json" 182.25; done
for c in body_cp body_cp_field; do au fix25/$c/wanaka200 "2 Back Rough [edges, CP rest]" "tp_7_*.json" 182.0; done
python3 -I c39_d34.py > c39_d34.txt 2>&1; echo "c39 rc=$?"
echo ANALYSISDONE
