"""P43 round 2: the fed pieces of one op that pass under the target (the model top at a
node), with how far the node stands inside the disc edge while the tool is under it.
A flat tool only. usage: gouge.py <nc> <op prefix> [depth bar=0.3] [edge=0.1]
For each node: depth = target - tool Z (the most over the piece), inside = the most that the
node stands inside the disc edge while the tool is more than 0.05 mm under the target."""
import sys, os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "p37"))
from gapmap import *
nc, pref = sys.argv[1], sys.argv[2]
bar = float(sys.argv[3]) if len(sys.argv) > 3 else 0.3
edge = float(sys.argv[4]) if len(sys.argv) > 4 else 0.1
T = target(); k = 0; rows = []
for op, tool, p0, p1 in moves(nc):
    if op is None or not op.startswith(pref):
        if rows or k: 
            if k: break
        continue
    k += 1
    kind, R, half = TOOLS[tool]
    if kind != "flat": continue
    ia = max(0, int(math.ceil((min(p0[0], p1[0]) - R) / CELL))); ib = min(NX - 1, int(math.floor((max(p0[0], p1[0]) + R) / CELL)))
    ja = max(0, int(math.ceil((min(p0[1], p1[1]) - R) / CELL))); jb = min(NY - 1, int(math.floor((max(p0[1], p1[1]) + R) / CELL)))
    if ib < ia or jb < ja: continue
    sub = np.full((jb - ja + 1, ib - ia + 1), np.inf); stamp_into = sub
    # the lowest tool Z over each node: the stamp of gapmap on an empty map
    big = np.full((NY, NX), np.inf) if False else None
    X = np.arange(ia, ib + 1) * CELL; Y = (np.arange(ja, jb + 1) * CELL)[:, None]
    dx, dy = p1[0] - p0[0], p1[1] - p0[1]; L2 = dx * dx + dy * dy
    if L2 < 1e-18:
        d2 = (X - p0[0]) ** 2 + (Y - p0[1]) ** 2; h = np.where(d2 <= R * R, min(p0[2], p1[2]), np.inf)
    else:
        L = math.sqrt(L2); t = ((X - p0[0]) * dx + (Y - p0[1]) * dy) / L2
        perp2 = ((X - p0[0]) * dy - (Y - p0[1]) * dx) ** 2 / L2
        ok = perp2 <= R * R; ht = np.sqrt(np.where(ok, R * R - perp2, 0.0)) / L
        ta = np.clip(t - ht, 0, 1); tb = np.clip(t + ht, 0, 1); ok &= (t + ht >= 0) & (t - ht <= 1)
        te = tb if p1[2] < p0[2] else ta
        h = np.where(ok, p0[2] + te * (p1[2] - p0[2]), np.inf)
    Tsub = T[ja:jb + 1, ia:ib + 1]
    g = np.isfinite(Tsub) & (Tsub - h > bar)
    if not g.any(): continue
    for j, i in zip(*np.nonzero(g)):
        nx, ny = (ia + i) * CELL, (ja + j) * CELL; tt = np.linspace(0, 1, 801)
        cx, cy, cz = p0[0] + tt * dx, p0[1] + tt * dy, p0[2] + tt * (p1[2] - p0[2])
        ins = R - np.hypot(nx - cx, ny - cy); under = (cz < Tsub[j, i] - 0.05) & (ins >= 0)
        rows.append((k, nx, ny, float(Tsub[j, i] - h[j, i]), float(ins[under].max()) if under.any() else 0.0, p0, p1))
nodes = {}
for r in rows:
    key = (round(r[1], 1), round(r[2], 1))
    if key not in nodes or r[3] > nodes[key][3]: nodes[key] = r
real = [r for r in nodes.values() if r[4] >= edge]; thin = [r for r in nodes.values() if r[4] < edge]
print(f"{nc} '{pref}': nodes that a fed piece passes more than {bar} mm under the target: {len(nodes)} ({len(nodes)*CELL*CELL:.2f} mm2)")
print(f"   {edge} mm or more inside the disc edge: {len(real)}, deepest {max([r[3] for r in real], default=0):.2f} mm; nearer to the edge: {len(thin)}, deepest {max([r[3] for r in thin], default=0):.2f} mm, the most inside {max([r[4] for r in thin], default=0):.3f} mm")
print("   nodes by how far inside the disc edge (mm): " + ", ".join(f"{lo:.2f}-{hi:.2f}: {sum(1 for r in nodes.values() if lo <= r[4] < hi)}" for lo, hi in ((0, 0.01), (0.01, 0.05), (0.05, 0.1), (0.1, 9))))
for r in sorted(nodes.values(), key=lambda r: (-r[4], -r[3]))[:6]:
    print(f"   most inside: node ({r[1]:.1f},{r[2]:.1f}) depth {r[3]:.2f} inside {r[4]:.3f} piece {r[0]} {tuple(round(v,3) for v in r[5])} -> {tuple(round(v,3) for v in r[6])}")
for r in sorted(nodes.values(), key=lambda r: -r[3])[:10]:
    print(f"   node ({r[1]:.1f},{r[2]:.1f}) depth {r[3]:.2f} inside {r[4]:.3f} piece {r[0]} {tuple(round(v,3) for v in r[5])} -> {tuple(round(v,3) for v in r[6])}")
