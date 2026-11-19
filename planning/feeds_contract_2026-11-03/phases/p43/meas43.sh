#!/bin/bash
# P43: the measures of one run of setup "1 Back". usage: meas43.sh <tag> [tag before, for left.py]
# Needs out_<tag>, out_<tag>.nc and, beside this file, p37/gapmap.py, p40/meas.py, p40/left.py,
# p39/entries.py, deep.py, lim.py, rows.py, classes.py.
S=$(cd "$(dirname "$0")" && pwd); cd $S; t=$1
python3 rows.py out_$t 1 2 > rows_$t.txt 2>&1
python3 p37/gapmap.py replay out_$t.nc out_$t.npz > replay_$t.txt 2>&1
python3 deep.py out_$t.nc "Back bulk" deep1_$t.npz 4.966 > deep1_$t.txt 2>&1
python3 classes.py out_$t.nc "Back bulk" deep1_$t.npz 4.966 21.334 17.168 13.002 11.0 | tail -1 >> deep1_$t.txt 2>&1
python3 deep.py out_$t.nc "Back rest" deep2_$t.npz 6.3 > deep2_$t.txt 2>&1
PYTHONPATH=$S/p37 python3 p40/meas.py $t > meas_$t.txt 2>&1
python3 p39/entries.py out_$t.nc "Back rest 6 mm" out_$t/tp_2_*.json 3.0 entries_$t.json 6.0 > entries_$t.txt 2>&1
python3 - > rep_$t.txt 2>&1 <<PY
import sys; sys.path.insert(0, "$S/p37")
from gapmap import *
d = np.load("out_$t.npz"); names = list(d["names"])
k = [i for i, n in enumerate(names) if n.startswith("Back rest")][0]
report("out_$t.npz", "$t after id 2", snap=k, png=False)
report("out_$t.npz", "$t end", png=False)
PY
[ -n "$2" ] && PYTHONPATH=$S/p37 python3 p40/left.py $2 $t > left_$t.txt 2>&1
cat rows_$t.txt; head -2 deep1_$t.txt; tail -1 deep1_$t.txt; head -2 deep2_$t.txt; cat meas_$t.txt rep_$t.txt; tail -6 entries_$t.txt; [ -n "$2" ] && cat left_$t.txt
grep -a -c "WARNING" out_$t.nc
