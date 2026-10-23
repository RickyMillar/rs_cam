# Diagnostic (not on the page): where the (e)/(f) loops read over the ceiling.
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
from geom import R, D, C_FRAC, TOL
from shapes import Shape
from march import Replay
d = np.load(os.path.join(os.path.dirname(os.path.abspath(__file__)), "curves_whole.npz"))


def load(nm, k):
    out = {}
    for key in d.files:
        a, b, i, j = key.split("|")
        if a == nm and b == k:
            out.setdefault(int(i), []).append((int(j), d[key].astype(float)))
    return [[P for _, P in sorted(v, key=lambda t: t[0])] for _, v in sorted(out.items())]


def overs(nm, k):
    sh = Shape(nm)
    rp = Replay(stock0=sh.stock0, mx=sh.mx, my=sh.my, seed=sh.seed)
    rows = []
    cv = load(nm, k)
    for li, lvl in enumerate(cv):
        for P in lvl:
            w = rp.cut(P)[0] / D
            s = np.concatenate([[0], np.cumsum(np.hypot(*np.diff(P, axis=0).T))])
            inner = (s > R) & (s < s[-1] - R)
            for i in np.nonzero(inner & (w > C_FRAC + TOL / D))[0]:
                rows.append((li, w[i], P[i, 0], P[i, 1], float(sh.wall_dist(P[i, 0], P[i, 1]))))
    return len(cv), rows


if __name__ == "__main__":
    for nm, k in (("star", "e"), ("star", "f"), ("pocket", "f")):
        n, rows = overs(nm, k)
        lv = sorted(set(r[0] for r in rows))
        print(nm, k, "levels", n, "over samples", len(rows), "at levels", lv[:20])
        if rows:
            wd = np.array([r[4] for r in rows])
            print("   wall distance of over samples: min %.2f max %.2f" % (wd.min(), wd.max()))
            for r in sorted(rows, key=lambda r: -r[1])[:5]:
                print("   level %d w %.3f at (%.1f, %.1f) wall %.2f" % r)
