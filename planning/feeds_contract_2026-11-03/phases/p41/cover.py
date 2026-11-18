"""P41: each fed piece of an op whose disc covers one point, in G-code order. usage: cover.py <nc> <op prefix> <x> <y> [R]"""
import sys, os, math
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "p37"))
from gapmap import *
nc, pref, x, y = sys.argv[1], sys.argv[2], float(sys.argv[3]), float(sys.argv[4])
k = 0; last=None
for op, tool, p0, p1 in moves(nc):
    if op is None or not op.startswith(pref):
        continue
    R = TOOLS[tool][1] if len(sys.argv) < 6 else float(sys.argv[5])
    dx, dy = p1[0]-p0[0], p1[1]-p0[1]; L2 = dx*dx+dy*dy
    t = 0 if L2 < 1e-12 else max(0, min(1, ((x-p0[0])*dx+(y-p0[1])*dy)/L2))
    d = math.hypot(p0[0]+t*dx-x, p0[1]+t*dy-y)
    if d <= R:
        z = p0[2]+t*(p1[2]-p0[2])
        key=(round(z,1))
        if key!=last:
            print(f"piece {k}: Z {z:.2f} centre distance {d:.2f}  ({p0[0]:.1f},{p0[1]:.1f}) -> ({p1[0]:.1f},{p1[1]:.1f})")
            last=key
    k += 1
