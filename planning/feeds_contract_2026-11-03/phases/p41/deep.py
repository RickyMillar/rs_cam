"""P41 (the instrument of P38 written again): the replay of gapmap.py, and for each fed piece
of one op the most that the piece lowers a node (before - after).
usage: deep.py <nc> <op prefix> <out.npz> [bar]
Writes per piece: p0, p1, depth, count of nodes lowered more than the bar, and the index.
"""
import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "p37"))
from gapmap import *
nc, pref, out = sys.argv[1], sys.argv[2], sys.argv[3]
bar = float(sys.argv[4]) if len(sys.argv) > 4 else 6.3
top = np.full((NY, NX), STOCK_TOP)
rows = []; k = 0
for op, tool, p0, p1 in moves(nc):
    if op is None or not op.startswith(pref):
        if rows:
            break
        stamp(top, tool, p0, p1); continue
    kind, R, half = TOOLS[tool]
    ia = max(0, int(math.ceil((min(p0[0], p1[0]) - R) / CELL))); ib = min(NX - 1, int(math.floor((max(p0[0], p1[0]) + R) / CELL)))
    ja = max(0, int(math.ceil((min(p0[1], p1[1]) - R) / CELL))); jb = min(NY - 1, int(math.floor((max(p0[1], p1[1]) + R) / CELL)))
    before = top[ja:jb + 1, ia:ib + 1].copy()
    stamp(top, tool, p0, p1)
    d = before - top[ja:jb + 1, ia:ib + 1]
    m = float(d.max()) if d.size else 0.0
    n = int((d > bar).sum())
    jj, ii = np.unravel_index(np.argmax(d), d.shape) if d.size else (0, 0)
    rows.append((k, *p0, *p1, m, n, (ia + ii) * CELL, (ja + jj) * CELL, float(before[jj, ii]) if d.size else 0.0))
    k += 1
a = np.array(rows)
np.savez_compressed(out, pieces=a, stock=top.astype(np.float32))
cut = a[a[:, 7] > 0.01]
deep = a[a[:, 7] > bar]
L = np.hypot(np.hypot(deep[:, 4] - deep[:, 1], deep[:, 5] - deep[:, 2]), deep[:, 6] - deep[:, 3])
print(f"pieces {len(a)}, cutting {len(cut)}, deeper than {bar}: {len(deep)} pieces, {L.sum():.0f} mm of feed; deepest {a[:,7].max():.2f}")
desc = deep[(deep[:, 6] < deep[:, 3] - 1e-6)]
print(f"  of them descending pieces: {len(desc)}; between 6.0 and {bar}: {((a[:,7]>6.0)&(a[:,7]<=bar)).sum()}; nodes lowered > bar: {int(deep[:,8].sum())}")
order = np.argsort(-L)[:12]
for i in order:
    r = deep[i]
    print(f"   piece {int(r[0])}: ({r[1]:.1f},{r[2]:.1f},{r[3]:.2f}) -> ({r[4]:.1f},{r[5]:.1f},{r[6]:.2f}) len {L[i]:.1f} depth {r[7]:.2f} nodes {int(r[8])} worst node ({r[9]:.1f},{r[10]:.1f}) top before {r[11]:.2f}")
