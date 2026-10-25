# C27: the share of each arm's "cut" length that retraces an earlier fed
# move of the same level backward (the keep-down return emitted as a fed
# move with no trace tag), and the share that retraces forward.
import collections
from perlevel import fx, per_level
def key(p): return (round(p[0], 3), round(p[1], 3), round(p[2], 3))
def run(arm):
    d, rows = per_level(arm)
    seen = set()
    back = collections.Counter(); total = collections.Counter()
    for (g, a, b), c, l in zip(d["moves"], d["cls"], d["lev"]):
        if g == 0 or l is None: continue
        L = fx.xy(a, b)
        if L < 1e-6: continue
        if c == "cut":
            total[l] += L
            if (key(b), key(a)) in seen: back[l] += L
        seen.add((key(a), key(b)))
    return total, back
for arm in ("geodesic_local_zigzag", "geodesic_local_zigzag_v2"):
    t, b = run(arm)
    print(arm, {l: (round(t[l]), round(b[l])) for l in t}, "sum", round(sum(t.values())), round(sum(b.values())))
