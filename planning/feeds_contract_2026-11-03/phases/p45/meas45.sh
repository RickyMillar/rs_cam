#!/bin/bash
# P45 (copy of p44/meas44.sh): the measures of one run of setup "1 Back". usage: meas44.sh <scratch p44 dir> <tag>
# Needs out_<tag> and out_<tag>.nc in the scratch dir, and ../p43 beside it (deepc.py, gouge.py with p37/).
S=$1; t=$2; H=$(cd "$(dirname "$0")" && pwd); cd $S
python3 $H/../p43/rows.py out_$t 1 2 > rows_$t.txt 2>&1
python3 $H/../p37/gapmap.py replay out_$t.nc out_$t.npz > replay_$t.txt 2>&1
python3 $H/../p44/places.py out_$t.npz --json places_$t.json > places_$t.txt 2>&1
python3 ../p43/deepc.py out_$t.nc "Back bulk" 4.666 > deepc1_$t.txt 2>&1
python3 ../p43/deepc.py out_$t.nc "Back rest" 6.5 > deepc2_$t.txt 2>&1
python3 ../p43/gouge.py out_$t.nc "Back bulk" > gouge1_$t.txt 2>&1
python3 ../p43/gouge.py out_$t.nc "Back rest" > gouge2_$t.txt 2>&1
python3 - > times_$t.txt 2>&1 <<PY
import sys, json; sys.path.insert(0, "$H/../p37")
from gapmap import op_times
for op, (fed, t, rap) in op_times("out_$t.nc").items():
    if op.startswith(("Back bulk", "Back rest")): print("%-50s fed %.1f mm, feed time %.1f s, rapid %.1f mm" % (op[:50], fed, t, rap))
d = json.load(open("out_$t/summary.json"))
for tp in d["per_toolpath"]:
    if tp["id"] in (1, 2): print(tp["id"], tp["name"][:40], "moves", tp["move_count"], {k: round(v, 1) for k, v in tp.items() if isinstance(v, (int, float)) and ("time" in k or "runtime" in k or "length" in k or k.endswith("_mm"))})
print("setup total_runtime_s", d["total_runtime_s"])
PY
cat rows_$t.txt places_$t.txt times_$t.txt; tail -3 deepc1_$t.txt; tail -3 deepc2_$t.txt; tail -2 gouge1_$t.txt; tail -2 gouge2_$t.txt
grep -a -c "WARNING" out_$t.nc; grep -a "rough_left_stock\|still stands at" out_$t.nc | cut -c1-1500 | head -4
