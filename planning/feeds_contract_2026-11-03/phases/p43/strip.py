"""P43: for one op, per Z band of fed pieces, the least X of a fed point in a Y window, and
the target top profile along a line. usage: strip.py <nc> <op prefix> y0 y1 xmax"""
import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "p37"))
from gapmap import *
nc, pref = sys.argv[1], sys.argv[2]; y0, y1, xmax = map(float, sys.argv[3:6])
by = {}
allz = {}
for op, tool, p0, p1 in moves(nc):
    if op is None or not op.startswith(pref): continue
    for p in (p0, p1):
        zk = round(p[2], 2)
        allz[zk] = allz.get(zk, 0) + 1
        if y0 <= p[1] <= y1 and p[0] <= xmax:
            a = by.setdefault(zk, [1e9, 0, None])
            a[1] += 1
            if p[0] < a[0]: a[0] = p[0]; a[2] = p
print("Z counts (top 14):", sorted(allz.items(), key=lambda kv: -kv[1])[:14])
for z in sorted(by, reverse=True):
    if by[z][1] >= 5: print(f"Z {z:6.2f}: n {by[z][1]:5d} min X {by[z][0]:.2f} at {by[z][2]}")
t = target()
j = int(round(((y0 + y1) / 2) / CELL))
print("target top along Y", j * CELL, [(round(i * CELL, 1), round(float(t[j, i]), 2)) for i in range(int(20 / CELL), int(70 / CELL), 5)])
