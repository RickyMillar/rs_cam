"""P41: the replay stock and the target on a line of nodes before one piece of an op.
usage: sect.py <nc> <op prefix> <piece k> x|y <fixed coord> <from> <to>
Also prints each earlier fed piece of the op whose tool disc comes within 4.5 mm of the line middle (Z and nearest distance)."""
import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "p37"))
from gapmap import *
nc, pref, K, ax, fixed, a, b = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4], float(sys.argv[5]), float(sys.argv[6]), float(sys.argv[7])
T = target()
top = np.full((NY, NX), STOCK_TOP); k = 0
mid = (fixed, (a + b) / 2) if ax == "x" else ((a + b) / 2, fixed)
near = {}
for op, tool, p0, p1 in moves(nc):
    if op is not None and op.startswith(pref):
        if k == K:
            print("piece", k, p0, p1); break
        # nearest distance of the piece to mid
        dx, dy = p1[0]-p0[0], p1[1]-p0[1]; L2 = dx*dx+dy*dy
        t = 0 if L2 < 1e-12 else max(0, min(1, ((mid[0]-p0[0])*dx+(mid[1]-p0[1])*dy)/L2))
        d = math.hypot(p0[0]+t*dx-mid[0], p0[1]+t*dy-mid[1])
        if d < 6.0:
            z = round(p0[2]+t*(p1[2]-p0[2]), 2)
            if z not in near or d < near[z][0]:
                near[z] = (d, k)
        k += 1
    stamp(top, tool, p0, p1)
vals = np.arange(int(round(a / CELL)), int(round(b / CELL)) + 1)
f = int(round(fixed / CELL))
print("coord  target  stock")
for v in vals:
    j, i = (v, f) if ax == "x" else (f, v)
    print(f"{v*CELL:7.1f} {T[j,i]:7.2f} {top[j,i]:7.2f}")
print("earlier pieces of the op near", mid, ": Z -> (nearest centre distance, piece)")
for z in sorted(near, reverse=True):
    print(f"   Z {z:6.2f}: {near[z][0]:.2f} mm (piece {near[z][1]})")
