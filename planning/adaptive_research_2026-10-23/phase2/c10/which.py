"""C10: the pocket arcs where the fixed C8 test plans a lead-in, and the
growing-bite fits, with level, rho, arc length and span."""
import math, sys
sys.path.insert(0, sys.argv[1])
from model import *
from fixed import lead_band_fixed  # noqa: E402 (runs fixed's report too)
for shape in (Pocket(), Star()):
    F = Field(shape)
    levels = law_levels(F.dmax)
    K = core_levels(F, levels)
    ch = [c for c in chains_of(F, levels) if c.level >= K + 1]
    print(f"== {shape.name}")
    for c in ch:
        if c.closed:
            continue
        par = [p for p in ch if p.level + 1 == c.level and c.within(p, REACH)]
        if not par:
            continue
        sub = subdivide(c.pts, CELL)
        L = plen(sub)
        mid = point_at(sub, L / 2)[0]
        best = min(par, key=lambda p: nearest_on(p.pts, mid)[1])
        rho = math.hypot(*(mid - F.pivot(mid)))
        g = growing_bite_span(rho, nearest_on(best.pts, mid)[1])
        for flip in (False, True):
            b = c.pts[::-1].copy() if flip else c.pts.copy()
            a = aligned_to(b, best.pts)
            r, rec, why = lead_band_fixed(F, a, b)
            if why is None or (g is not None and g < L):
                print(f"  level {c.level} rho {rho:.2f} arc {L:.2f} parents {len(par)} flip {flip}: "
                      f"circular {why or 'LEAD-IN'} span {fmt(rec.get('span'))} P at {fmt(rec.get('at'))}; growing bite {fmt(g)}")
