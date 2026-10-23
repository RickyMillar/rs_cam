"""C10: the C8 fit test with the proposed fix (scan P from mid-arc toward F
in steps of one cell until the traced start lies on arc a), against the
code as it is, on lobe bands and on every open arc with a parent."""
import math, sys
from collections import Counter
import numpy as np
sys.path.insert(0, sys.argv[1])
import model as M
from model import *


def lead_band_fixed(F, a, b):
    sub_a = subdivide(a, CELL)
    sub_b = subdivide(b, CELL)
    total = plen(sub_b)
    at = total / 2
    last_why = None
    while at < total:
        p, seg, t = point_at(sub_b, at)
        res, info, why = lead_in_onto(F, a, p, t)
        if res is not None:
            radius, lead = res
            start = lead[0]
            iq = int(np.argmin(np.hypot(*(sub_a - start).T)))
            link = np.vstack([sub_a[: iq + 1], start])
            fwd = np.vstack([p, sub_b[seg + 1:]])
            bwd = np.vstack([p, sub_b[: seg + 1][::-1]])
            for poly in (link, lead, fwd, bwd):
                for u, v in zip(poly[:-1], poly[1:]):
                    if not F.seg_inside(u, v):
                        return None, dict(total=total), "region"
            return radius, dict(total=total, at=at, span=math.hypot(*(start - p))), None
        if why in ("no margin", "radius > rho - delta/2"):
            return None, dict(total=total), why
        last_why = why
        at += CELL
    return None, dict(total=total), "no P on the arc: " + str(last_why)


def main():
    for shape in (Star(), Pocket()):
      F = Field(shape)
      _, rows = M.run(shape)
      levels = law_levels(F.dmax)
      K = core_levels(F, levels)
      ch = [c for c in chains_of(F, levels) if c.level >= K + 1]
      # lobe bands again (orientation 0 only, every band read from E = start)
      lobe_rows = [r for r in rows if r["orient"] == 0]
      print(f"== {shape.name}: lobe bands {len(lobe_rows)}, as coded: {dict(Counter(r['why'] for r in lobe_rows))}")
      # all open arcs with a parent, coded vs fixed, both ends as E
      res_c, res_f, res_g = Counter(), Counter(), 0
      n = 0
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
          for flip in (False, True):
              b = c.pts[::-1].copy() if flip else c.pts.copy()
              a = aligned_to(b, best.pts)
              n += 1
              _, _, why = lead_band(F, a, b)
              res_c[why or "LEAD-IN"] += 1
              _, rec, why = lead_band_fixed(F, a, b)
              res_f[why or "LEAD-IN"] += 1
          rho = math.hypot(*(mid - F.pivot(mid)))
          g = growing_bite_span(rho, nearest_on(best.pts, mid)[1])
          res_g += 2 * (g is not None and g < L)
      print(f"   open arcs with a parent x 2 ends = {n}")
      print(f"   as coded : {dict(res_c)}")
      print(f"   fixed    : {dict(res_f)}")
      print(f"   growing bite fits (span < arc): {res_g}")


if __name__ == '__main__':
    main()
