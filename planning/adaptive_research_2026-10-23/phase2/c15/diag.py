# Diagnostic: where the raster replay reads over the ceiling.
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import numpy as np
from geom import *
from march import Replay
from harness_svg import RENDERS
d = np.load(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'curves.npz'))
def load(k):
    out = {}
    for key in d.files:
        kk, i, j = key.split('|')
        if kk == k:
            out.setdefault(int(i), []).append((int(j), d[key].astype(float)))
    return [[P for _, P in sorted(v, key=lambda t: t[0])] for _, v in sorted(out.items())]
first = load('first')
for key in sys.argv[1:]:
    cv = load(key)
    rp = Replay()
    plan = ([(l, False) for l in first] if key not in ('a', 'a0') else []) + [(l, True) for l in cv]
    rows = []
    for li, (lvl, z) in enumerate(plan):
        for ai, P in enumerate(lvl):
            w = rp.cut(P)[0] / D
            hz = hidden(P[:, 0], P[:, 1])
            for k in np.nonzero((w > C_FRAC) & hz)[0]:
                rows.append((w[k], li, ai, k, len(P), P[k, 0], P[k, 1]))
    rows.sort(reverse=True)
    print(key, len(rows))
    pos = np.array([min(r[3], r[4] - 1 - r[3]) for r in rows])
    print(' samples from an arc end: <=3 cells', (pos <= 3).sum(), '<= R/TOL', (pos <= R / TOL).sum(), 'beyond', (pos > R / TOL).sum())
    for r in rows[:12]:
        print('  w %.3f level %d arc %d idx %d/%d at (%.2f, %.2f)' % r)


def probe(key, li_want, ai_want, k_want):
    cv = load(key)
    rp = Replay()
    plan = ([(l, False) for l in first] if key not in ('a', 'a0') else []) + [(l, True) for l in cv]
    for li, (lvl, z) in enumerate(plan):
        for ai, P in enumerate(lvl):
            if li == li_want and ai == ai_want:
                if k_want >= 2:
                    rp.cut(P[:k_want])
                x, y = P[k_want]
                ii, jj, dx, dy, inside = rp._window(x, y)
                mat = rp.stock[np.ix_(ii, jj)] & inside
                t = P[min(k_want + 1, len(P) - 1)] - P[max(k_want - 1, 0)]
                t /= np.hypot(*t)
                n = dx * -t[1] + dy * t[0]
                a = dx * t[0] + dy * t[1]
                print('heading', t, 'cells', mat.sum())
                print(' along range', a[mat].min(), a[mat].max(), ' across range', n[mat].min(), n[mat].max())
                # histogram of across coordinate
                h, e = np.histogram(n[mat], bins=12)
                print(' across hist', h, np.round(e, 2))
                prev = plan[li - 1][0] if li > 0 else []
                for Q in prev:
                    dd = np.hypot(Q[:, 0] - x, Q[:, 1] - y)
                    print(' prev level arc: nearest %.3f at %s, ends %s %s' % (dd.min(), Q[dd.argmin()], Q[0], Q[-1]))
                return
            rp.cut(P)


def residue(key):
    from march import zone_footprint, pocket_boundary_distance, MGX, MGY, SCALLOP, H
    cv = load(key)
    rp = Replay()
    plan = ([(l, False) for l in first] if key not in ('a', 'a0') else []) + [(l, True) for l in cv]
    for lvl, z in plan:
        for P in lvl:
            rp.cut(P)
    res = rp.stock & zone_footprint()
    bd = pocket_boundary_distance(MGX[res], MGY[res])
    sel = bd > SCALLOP + H
    xs, ys = MGX[res][sel], MGY[res][sel]
    from scipy import ndimage
    m = np.zeros_like(res)
    m[res] = sel
    lab, n = ndimage.label(m)
    print(key, 'interior residue blobs', n)
    for i in range(1, n + 1):
        ii = lab == i
        print('  blob %d: %.2f mm2 at (%.1f, %.1f), max wall dist %.2f' % (i, ii.sum() * H * H, MGX[ii].mean(), MGY[ii].mean(), pocket_boundary_distance(MGX[ii], MGY[ii]).max()))


def interior_over(key):
    cv = load(key)
    rp = Replay()
    plan = ([(l, False) for l in first] if key not in ('a', 'a0') else []) + [(l, True) for l in cv]
    for li, (lvl, z) in enumerate(plan):
        for ai, P in enumerate(lvl):
            w = rp.cut(P)[0] / D
            s = np.concatenate([[0], np.cumsum(np.hypot(*np.diff(P, axis=0).T))])
            inner = (s > R) & (s < s[-1] - R)
            for k in np.nonzero(inner & (w > C_FRAC))[0]:
                print('  %s level %d arc %d idx %d/%d w %.3f at (%.2f, %.2f)' % (key, li, ai, k, len(P), w[k], P[k, 0], P[k, 1]))


def end_class(p):
    import math
    from geom import _in_outer
    x, y = p
    if abs(math.hypot(x - ISL_C[0], y - ISL_C[1]) - HOLE_R) < 0.3:
        return 'island'
    for q in ((x + 0.3, y), (x - 0.3, y), (x, y + 0.3), (x, y - 0.3)):
        if not _in_outer(*q):
            return 'wall'
    for a, b in ((P_UP, A_UP), (P_LO, B_LO)):
        u = (b - a) / np.hypot(*(b - a))
        v = np.array(p) - a
        if abs(v[0] * u[1] - v[1] * u[0]) < 0.3:
            return 'cut edge'
    return 'interior'


def ends(key):
    import collections
    cv = load(key)
    c = collections.Counter()
    for lvl in cv:
        for P in lvl:
            c[tuple(sorted((end_class(P[0]), end_class(P[-1]))))] += 1
    return dict(c)
