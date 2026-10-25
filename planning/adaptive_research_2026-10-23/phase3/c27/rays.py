# C27: crossings of fed cut moves along 8 rays from the starter centre, per
# level; the crossing positions show where v2 adds passes.
import math, sys
from perlevel import fx, per_level
def cuts(arm, lev):
    d, _ = per_level(arm)
    rings = [p for i, p in d["rings"] if d["lev"][i] == lev]
    segs = [(a, b) for (g, a, b), c, l in zip(d["moves"], d["cls"], d["lev"]) if g and c == "cut" and l == lev and fx.xy(a, b) > 1e-6]
    return rings, segs
def cross(o, ang, segs, L=80):
    ux, uy = math.cos(ang), math.sin(ang); out = []
    for a, b in segs:
        dx, dy = b[0]-a[0], b[1]-a[1]
        den = ux*dy - uy*dx
        if abs(den) < 1e-12: continue
        ax, ay = a[0]-o[0], a[1]-o[1]
        t = (ax*dy - ay*dx)/den; s = (ax*uy - ay*ux)/den
        if 0 < t < L and 0 <= s <= 1: out.append(t)
    out.sort(); res = []
    for t in out:
        if not res or t - res[-1] > 0.05: res.append(t)
    return res
lev = sys.argv[1] if len(sys.argv) > 1 else "Z 5.00"
rz, sz = cuts("geodesic_local_zigzag", lev); rv, sv = cuts("geodesic_local_zigzag_v2", lev)
o = (51.05, 58.55)
print("rings zz", [tuple(round(c, 1) for c in p[:2]) for p in rz][:12])
for k in range(8):
    a = k * math.pi / 4
    cz, cv = cross(o, a, sz), cross(o, a, sv)
    print(f'ray {k*45:3d}: zigzag {len(cz):3d} v2 {len(cv):3d}  zz gaps median {sorted(b-a for a,b in zip(cz,cz[1:]))[len(cz)//2-1] if len(cz)>2 else 0:.2f}  v2 {sorted(b-a for a,b in zip(cv,cv[1:]))[len(cv)//2-1] if len(cv)>2 else 0:.2f}')
