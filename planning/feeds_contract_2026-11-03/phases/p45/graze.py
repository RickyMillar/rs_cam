"""P45: the pieces of an op with a node over the bar, with how far inside the piece edge the nodes stand.
usage: graze.py <nc> <op prefix> <bar>"""
import sys, os, math
sys.path.insert(0, "/home/ricky/personal_repos/rs_cam/planning/feeds_contract_2026-11-03/phases/p37")
from gapmap import *
nc, pref, bar = sys.argv[1], sys.argv[2], float(sys.argv[3])
top = np.full((NY, NX), STOCK_TOP); k = 0
for op, tool, p0, p1 in moves(nc):
    if op is None or not op.startswith(pref):
        if k: break
        stamp(top, tool, p0, p1); continue
    kind, R, half = TOOLS[tool]
    ia = max(0, int(math.ceil((min(p0[0], p1[0]) - R) / CELL))); ib = min(NX - 1, int(math.floor((max(p0[0], p1[0]) + R) / CELL)))
    ja = max(0, int(math.ceil((min(p0[1], p1[1]) - R) / CELL))); jb = min(NY - 1, int(math.floor((max(p0[1], p1[1]) + R) / CELL)))
    before = top[ja:jb + 1, ia:ib + 1].copy(); stamp(top, tool, p0, p1)
    dd = before - top[ja:jb + 1, ia:ib + 1]; deep = dd > bar
    if deep.any():
        X = np.arange(ia, ib + 1) * CELL; Y = (np.arange(ja, jb + 1) * CELL)[:, None]
        dx, dy = p1[0] - p0[0], p1[1] - p0[1]; L2 = dx * dx + dy * dy
        t = np.clip(((X - p0[0]) * dx + (Y - p0[1]) * dy) / L2, 0, 1) if L2 > 1e-18 else np.zeros(dd.shape)
        ins = R - np.hypot(X - (p0[0] + t * dx), Y - (p0[1] + t * dy))
        print(f"piece {k}: {tuple(round(v, 3) for v in p0)} -> {tuple(round(v, 3) for v in p1)}: {int(deep.sum())} nodes over the bar, lowered {dd[deep].min():.2f}..{dd[deep].max():.2f} mm, inside the edge {ins[deep].min():.4f}..{ins[deep].max():.4f} mm, tops before {before[deep].min():.2f}..{before[deep].max():.2f}")
    k += 1
