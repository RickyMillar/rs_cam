"""C10: every wrap piece after the core, with its rho, length, parent,
and the fit of the circular lead-in and of the growing bite."""
import math, sys
import numpy as np
sys.path.insert(0, sys.argv[1])
from model import *

for shape in (Star(), Pocket()):
    F = Field(shape)
    levels = law_levels(F.dmax)
    K = core_levels(F, levels)
    ch = [c for c in chains_of(F, levels) if c.level >= K + 1]
    print(f"== {shape.name}: {len(ch)} pieces after core K={K}")
    print(" lvl closed  len    rho_mid  margin  nparents  circ  grow   circ_result")
    n_open_par = n_circ = n_grow = n_beyond = 0
    for c in ch:
        sub = subdivide(c.pts, CELL)
        L = plen(sub)
        mid = point_at(sub, L / 2)[0]
        rho = math.hypot(*(mid - F.pivot(mid)))
        par = [p for p in ch if p.level + 1 == c.level and c.within(p, REACH)]
        res = "-"
        g = None
        m = None
        if len(par) >= 1 and not c.closed:
            # nearest parent to mid
            best = min(par, key=lambda p: nearest_on(p.pts, mid)[1])
            a = aligned_to(c.pts, best.pts)
            foot, delta = nearest_on(best.pts, mid)
            m = CD - w_front(rho, delta)
            g = growing_bite_span(rho, delta)
            r, rec, why = lead_band(F, a, c.pts)
            res = why or "LEAD-IN"
            n_open_par += 1
            n_beyond += m > 0.0005
            n_circ += why is None
            n_grow += g is not None and g < L
        print(f" {c.level:3d} {str(c.closed):6} {L:6.2f} {rho:7.2f} {fmt(m, '{:+.4f}'):>8} {len(par):3d}"
              f"  {fmt(rec.get('span') if res!='-' else None):>6} {fmt(g):>6}  {res}")
    print(f"   open arcs with a parent {n_open_par}; margin > 0.0005: {n_beyond}; circular lead-in fits {n_circ}; growing bite fits {n_grow}")
