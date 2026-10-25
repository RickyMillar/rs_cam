# C27: the G2/G3 moves of "3D Rough 1" per level: count and true arc length
# (from I, J), per arm. The one-way returns of v2 are arc-fitted (G2/G3).
import collections, math, os
from perlevel import fx, per_level, BASE
def arcs(arm):
    d, _ = per_level(arm)
    lines = open(os.path.join(BASE, arm, "program.nc")).read().splitlines()
    cmds = fx.section(lines, "3D Rough 1")[1:1 + len(d["moves"])]
    n = collections.Counter(); L = collections.Counter(); g = 0
    for cmd, (gc, a, b), l in zip(cmds, d["moves"], d["lev"]):
        w = dict((k, float(v)) for k, v in fx.WORD.findall(cmd))
        if gc in (2, 3):
            cx, cy = a[0] + w.get("I", 0), a[1] + w.get("J", 0)
            r = math.hypot(a[0]-cx, a[1]-cy)
            t0 = math.atan2(a[1]-cy, a[0]-cx); t1 = math.atan2(b[1]-cy, b[0]-cx)
            sweep = (t1 - t0) % (2*math.pi) if gc == 3 else (t0 - t1) % (2*math.pi)
            n[l] += 1; L[l] += r * sweep
    return d, n, L
for arm in ("geodesic_local_zigzag", "geodesic_local_zigzag_v2"):
    d, n, L = arcs(arm)
    print(arm, {l: (n[l], round(L[l])) for l in d["levels"]}, "sum", sum(n.values()), round(sum(L.values())))
