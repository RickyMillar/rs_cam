# C27: fed "cut" moves that run over an earlier fed move of the same level
# (every 0.25 mm sample within TOL of an earlier move at the same Z): the
# keep-down returns and any other travel over cut stock that the trace does
# not tag. TOL is the 3D RDP tolerance (cut_simplify_tolerance 0.1 mm).
import collections, math
from perlevel import fx, per_level
TOL = 0.1
def segd(p, a, b):
    ax, ay = a[0], a[1]; dx, dy = b[0]-ax, b[1]-ay; L2 = dx*dx+dy*dy
    t = 0 if L2 == 0 else max(0, min(1, ((p[0]-ax)*dx+(p[1]-ay)*dy)/L2))
    return math.hypot(ax+t*dx-p[0], ay+t*dy-p[1])
def run(arm):
    d, rows = per_level(arm)
    grid = collections.defaultdict(list); H = 1.0
    total = collections.Counter(); over = collections.Counter()
    for (g, a, b), c, l in zip(d["moves"], d["cls"], d["lev"]):
        if g == 0 or l is None: continue
        L = fx.xy(a, b)
        if L < 1e-6: continue
        if c == "cut":
            total[l] += L
            n = max(1, int(L / 0.25))
            ok = True
            for k in range(n + 1):
                p = (a[0]+(b[0]-a[0])*k/n, a[1]+(b[1]-a[1])*k/n)
                cell = (int(p[0]//H), int(p[1]//H))
                hit = False
                for dx in (-1,0,1):
                    for dy in (-1,0,1):
                        for (q, r, z) in grid[(cell[0]+dx, cell[1]+dy)]:
                            if abs(z - b[2]) < 1e-3 and segd(p, q, r) <= TOL: hit = True; break
                        if hit: break
                    if hit: break
                if not hit: ok = False; break
            if ok: over[l] += L
        # index this move
        n = max(1, int(L / H)); 
        cells = {(int((a[0]+(b[0]-a[0])*k/n)//H), int((a[1]+(b[1]-a[1])*k/n)//H)) for k in range(n+1)}
        for cl in cells: grid[cl].append((a, b, b[2]))
    return total, over
if __name__ == "__main__":
    for arm in ("geodesic_local_zigzag", "geodesic_local_zigzag_v2"):
        t, o = run(arm)
        print(arm, {l: (round(t[l]), round(o[l])) for l in t}, "sum", round(sum(t.values())), round(sum(o.values())))
