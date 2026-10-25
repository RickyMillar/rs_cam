# C27: fed cut length of one arm that runs where the other arm has no fed
# move (every 0.25 mm sample farther than TOL from all of the other arm's
# fed G1 moves of the level). G2/G3 (the arc-fitted returns) are measured
# by their chord and reported apart. Splits the difference into geometry
# the other arm never cuts (a different wrap: the merged meeting line) and
# travel over the same wraps.
import collections, math, sys
from perlevel import fx, per_level
TOL = 0.3
def segd(p, a, b):
    dx, dy = b[0]-a[0], b[1]-a[1]; L2 = dx*dx+dy*dy
    t = 0 if L2 == 0 else max(0, min(1, ((p[0]-a[0])*dx+(p[1]-a[1])*dy)/L2))
    return math.hypot(a[0]+t*dx-p[0], a[1]+t*dy-p[1])
def index(d):
    g = collections.defaultdict(list)
    for (gc, a, b), c, l in zip(d["moves"], d["cls"], d["lev"]):
        if gc == 1 and c == "cut" and fx.xy(a, b) > 1e-6:
            n = max(1, int(fx.xy(a, b)))
            for k in range(n + 1):
                x = a[0]+(b[0]-a[0])*k/n; y = a[1]+(b[1]-a[1])*k/n
                g[(l, int(x//2), int(y//2))].append((a, b))
    return g
def near(g, l, p):
    for dx in (-1, 0, 1):
        for dy in (-1, 0, 1):
            for a, b in g.get((l, int(p[0]//2)+dx, int(p[1]//2)+dy), ()):
                if segd(p, a, b) <= TOL: return True
    return False
def diff(src, ref):
    gi = index(ref); out = collections.Counter(); arcs = collections.Counter(); where = collections.defaultdict(list)
    for (gc, a, b), c, l in zip(src["moves"], src["cls"], src["lev"]):
        if c != "cut" or gc == 0 or fx.xy(a, b) < 1e-6: continue
        if gc in (2, 3): arcs[l] += fx.xy(a, b); continue
        L = fx.xy(a, b); n = max(1, int(L / 0.25)); miss = 0
        for k in range(n + 1):
            p = (a[0]+(b[0]-a[0])*k/n, a[1]+(b[1]-a[1])*k/n)
            if not near(gi, l, p): miss += 1
        if miss: out[l] += L * miss / (n + 1); where[l].append(((a[0]+b[0])/2, (a[1]+b[1])/2, L*miss/(n+1)))
    return out, arcs, where
A, _ = per_level("geodesic_local_zigzag"); B, _ = per_level("geodesic_local_zigzag_v2")
for name, s, r in (("v2 not in zigzag", B, A), ("zigzag not in v2", A, B)):
    o, arcs, where = diff(s, r)
    print(name, {l: round(o[l]) for l in s["levels"]}, "G2/G3 chords", {l: round(arcs[l]) for l in s["levels"]})
    if len(sys.argv) > 1:
        for l in s["levels"]:
            big = sorted(where[l], key=lambda w: -w[2])[:8]
            print("   ", l, [(round(x), round(y), round(m, 1)) for x, y, m in big])
