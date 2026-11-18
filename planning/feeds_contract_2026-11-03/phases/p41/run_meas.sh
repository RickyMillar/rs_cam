#!/bin/bash
# usage: run_meas.sh <tag>   (after the CLI run wrote out_<tag>, out_<tag>.nc)
S=$(cd "$(dirname "$0")" && pwd); cd $S; t=$1
python3 lim.py out_$t 1 2 40 > lim_$t.txt 2>&1
python3 p37/gapmap.py replay out_$t.nc out_$t.npz > replay_$t.txt 2>&1
python3 deep.py out_$t.nc "Back rest" deep_$t.npz > deep_$t.txt 2>&1
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
cat lim_$t.txt deep_$t.txt meas_$t.txt rep_$t.txt; tail -12 entries_$t.txt
