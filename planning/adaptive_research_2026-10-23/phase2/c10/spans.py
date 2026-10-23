"""C10: ideal spans on exact circles (prev arc unbounded): the C8 circular
lead-in (traced to one cell, as coded) and the growing bite, per level,
with the star and pocket arc lengths at that level from the model."""
import math, sys
sys.path.insert(0, sys.argv[1])
from model import *

def circ_span(rho, delta=S):
    rl, m, st = lead_in_radius_pivot(rho, delta)
    if rl is None or rl > rho - delta / 2:
        return None, rl
    # P at (rho, 0) about the pivot; lead centre at (rho - rl, 0)
    dth = CELL / rl
    for i in range(1, int(math.floor(math.pi / 2 / dth)) + 1):
        th = i * dth
        x = (rho - rl) + rl * math.cos(th)
        y = rl * math.sin(th)
        if math.hypot(x, y) - (rho - delta) <= CELL:
            return math.hypot(x - rho, y), rl
    return None, rl

arcs = {}
for shape in (Star(), Pocket()):
    F = Field(shape)
    levels = law_levels(F.dmax)
    K = core_levels(F, levels)
    for c in chains_of(F, levels):
        if c.level < K + 1 or c.closed:
            continue
        sub = subdivide(c.pts, CELL)
        L = plen(sub)
        mid = point_at(sub, L / 2)[0]
        rho = math.hypot(*(mid - F.pivot(mid)))
        arcs.setdefault(shape.name, []).append((c.level, rho, L))
    arcs[shape.name + "_levels"] = levels

lv = arcs["star_levels"]
print("level  rho    margin   rho_L   circ_span  grow_span  star arcs (mm)          pocket seed-visible arcs (mm)")
for k in range(8, 20):
    rho = lv[k]
    delta = lv[k] - lv[k - 1]
    m = CD - w_front(rho, delta)
    cs, rl = circ_span(rho, delta)
    g = growing_bite_span(rho, delta) if m > 1e-9 else None
    st = sorted(L for (l, r, L) in arcs["star"] if l == k and abs(r - rho) < 0.5)
    po = sorted(L for (l, r, L) in arcs["pocket"] if l == k and abs(r - rho) < 0.5)
    f = lambda xs: (f"{min(xs):.1f}-{max(xs):.1f} (n {len(xs)})" if xs else "-")
    print(f"{k:5d} {rho:6.2f} {m:+.4f} {fmt(rl):>7} {fmt(cs):>10} {fmt(g):>10}  {f(st):24} {f(po)}")
