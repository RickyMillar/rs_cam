# C27: the sequence of fed runs at one level in a box: each maximal run of
# consecutive fed moves of one class, with its start, end and length, and
# the straight cut moves longer than a bound (the diagonal links).
import sys, math
from perlevel import fx, per_level
arm, lev = sys.argv[1], sys.argv[2]; box = list(map(float, sys.argv[3:7]))
inb = lambda p: box[0] <= p[0] <= box[1] and box[2] <= p[1] <= box[3]
d, _ = per_level(arm)
runs = []; cur = None
for i, ((g, a, b), c, l) in enumerate(zip(d["moves"], d["cls"], d["lev"])):
    if l != lev or fx.xy(a, b) < 1e-6:
        continue
    single = c == "cut" and fx.xy(a, b) > 5.0
    kind = "LONG" if single else c
    if cur and cur[0] == kind and kind != "LONG" and cur[2] == a:
        cur[2] = b; cur[3] += fx.xy(a, b); cur[4] += 1
    else:
        if cur: runs.append(cur)
        cur = [kind, a, b, fx.xy(a, b), 1, i]
runs.append(cur)
for r in runs:
    if inb(r[1]) or inb(r[2]):
        print(f'{r[0]:5s} move {r[5]:5d} n{r[4]:3d} ({r[1][0]:7.2f},{r[1][1]:6.2f}) -> ({r[2][0]:7.2f},{r[2][1]:6.2f}) {r[3]:6.1f} mm')
