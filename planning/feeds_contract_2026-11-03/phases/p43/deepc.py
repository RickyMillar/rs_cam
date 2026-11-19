"""P43 round 2: the replay of deep.py with the corrected class (lead ruling 2026-11-20).
A node counts for a piece only when the piece lowers it by more than the bar AND the node
stands the cut tolerance or more inside the disc edge of the piece: tool radius less the
distance from the node to the piece (in XY) >= tol. A node nearer to the edge is a graze.
usage: deepc.py <nc> <op prefix> <bar> [tol=0.1] [piece index to print by path ...]
"""
import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "p37"))
from gapmap import *
nc, pref, bar = sys.argv[1], sys.argv[2], float(sys.argv[3])
tol = float(sys.argv[4]) if len(sys.argv) > 4 else 0.1
show = {int(v) for v in sys.argv[5:]}
top = np.full((NY, NX), STOCK_TOP)
k = 0; res = []
for op, tool, p0, p1 in moves(nc):
    if op is None or not op.startswith(pref):
        if res: break
        stamp(top, tool, p0, p1); continue
    kind, R, half = TOOLS[tool]
    ia = max(0, int(math.ceil((min(p0[0], p1[0]) - R) / CELL))); ib = min(NX - 1, int(math.floor((max(p0[0], p1[0]) + R) / CELL)))
    ja = max(0, int(math.ceil((min(p0[1], p1[1]) - R) / CELL))); jb = min(NY - 1, int(math.floor((max(p0[1], p1[1]) + R) / CELL)))
    before = top[ja:jb + 1, ia:ib + 1].copy()
    stamp(top, tool, p0, p1)
    d = before - top[ja:jb + 1, ia:ib + 1]
    deep = d > bar
    if deep.any():
        X = np.arange(ia, ib + 1) * CELL; Y = (np.arange(ja, jb + 1) * CELL)[:, None]
        dx, dy = p1[0] - p0[0], p1[1] - p0[1]; L2 = dx * dx + dy * dy
        t = np.clip(((X - p0[0]) * dx + (Y - p0[1]) * dy) / L2, 0, 1) if L2 > 1e-18 else np.zeros(d.shape)
        dist = np.hypot(X - (p0[0] + t * dx), Y - (p0[1] + t * dy))
        inside = R - dist                      # how far inside the disc edge the node stands
        real = deep & (inside >= tol)
        res.append((k, p0, p1, int(deep.sum()), int(real.sum()), float(d[deep].max()), float(d[real].max()) if real.any() else 0.0, float(inside[deep].max())))
        if k in show:
            jj, ii = np.nonzero(deep)
            print(f"piece {k}: {tuple(round(v,3) for v in p0)} -> {tuple(round(v,3) for v in p1)}; tool radius {R}")
            for j, i in zip(jj, ii):
                print(f"   node ({(ia+i)*CELL:.1f},{(ja+j)*CELL:.1f}) top before {before[j,i]:.2f} lowered {d[j,i]:.2f} mm; distance to the piece {dist[j,i]:.4f}; inside the disc edge by {inside[j,i]:.4f} mm; counts {bool(real[j,i])}")
    k += 1
allp = len(res); realp = [r for r in res if r[4] > 0]
print(f"{nc} '{pref}' bar {bar} tol {tol}: pieces with a node over the bar {allp} ({sum(r[3] for r in res)} nodes); of them with a node {tol} mm or more inside the disc edge: {len(realp)} pieces, {sum(r[4] for r in realp)} nodes, deepest {max([r[6] for r in realp], default=0):.2f} mm")
for r in sorted(realp, key=lambda r: -r[4])[:8]:
    print(f"   piece {r[0]}: {tuple(round(v,2) for v in r[1])} -> {tuple(round(v,2) for v in r[2])}: {r[4]} of {r[3]} nodes count, deepest {r[6]:.2f}, the deepest node inside the edge {r[7]:.3f} mm")
