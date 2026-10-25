# C27: fed "cut" moves that end at a point the cutter visited earlier at the
# same level (within 1 um): the one-way return emitted as one straight fed
# move. Per level: count and length.
import collections
from perlevel import fx, per_level
def key(p): return (round(p[0], 3), round(p[1], 3), round(p[2], 3))
def run(arm):
    d, _ = per_level(arm)
    seen = set(); n = collections.Counter(); L = collections.Counter(); out = []
    for i, ((g, a, b), c, l) in enumerate(zip(d["moves"], d["cls"], d["lev"])):
        if l is None: continue
        if c == "cut" and g != 0 and fx.xy(a, b) > 1e-6 and key(b) in seen:
            n[l] += 1; L[l] += fx.xy(a, b); out.append((i, a, b))
        seen.add(key(a)); seen.add(key(b))
    return n, L, out, d
if __name__ == "__main__":
    for arm in ("geodesic_local_zigzag", "geodesic_local_zigzag_v2"):
        n, L, out, d = run(arm)
        print(arm, {l: (n[l], round(L[l])) for l in d["levels"]}, "sum", sum(n.values()), round(sum(L.values())))
