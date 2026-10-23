"""C13: the reserved spacing the exact check accepts (bisection, min law,
shortest arm), against the derived rule: the full-bite point P must be at
least the corner-touch length l_c from the far wall end F."""
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import math
import numpy as np
from multiprocessing import Pool
from geom import CD, R, S, Stock, circle_pts, path_widths, w_front
from band import build, exact_band

def corner_touch(b, tol=1e-4):
    """Arc length (mm) from F_b at which the plain pass at rho, after arc a,
    first reads more than w(rho, delta) + tol (the far wall's corner stock
    reaches its rim): the plain pass E -> F sampled at 0.01 mm."""
    st = Stock(b["arcs"], b["th0"], True)
    P = circle_pts(b["rho"], b["E_b"], b["F_b"], 0.01)
    w = path_widths(st, P)
    base = w_front(b["rho"], b["delta"])
    half = len(P) // 2
    up = np.flatnonzero(w[half:] > base + tol)
    if not up.size:
        return 0.0
    i = half + up[0]
    return float(np.sum(np.hypot(*np.diff(P[i:], axis=0).T)))

def passes(arm, rho, d, sgn=+1, law="min"):
    b = build(arm, rho, d, sgn, law)
    if not b["fits"] or not b.get("ramp_inside", False):
        return False, b, None
    e = exact_band(b, True)
    return e["lead"] <= CD + 1e-6, b, e

def bisect_check(args):
    lvl, rho, cap, arm = args
    ok, b, e = passes(arm, rho, cap)
    if ok:
        return lvl, cap, True
    lo, hi = 1.0, cap
    for _ in range(12):
        mid = (lo + hi) / 2
        if passes(arm, rho, mid)[0]:
            lo = mid
        else:
            hi = mid
    return lvl, lo, False

def bisect_rule(args):
    lvl, rho, cap, arm = args
    def ok(d):
        b = build(arm, rho, d, +1, "min")
        return b["fits"] and b["spare"] >= corner_touch(b)
    if ok(cap):
        return lvl, cap
    lo, hi = 1.0, cap
    for _ in range(12):
        mid = (lo + hi) / 2
        if ok(mid):
            lo = mid
        else:
            hi = mid
    return lvl, lo

if __name__ == "__main__":
    lv = [3.0, 4.44, 6.03, 7.73, 9.51, 11.35, 13.23, 15.15, 17.10, 19.06, 21.05, 23.05, 25.05, 27.05, 29.05, 31.05]
    jobs = [(k, lv[k], min(S, lv[k] - lv[k - 1]), 0) for k in range(9, 16)]
    with Pool(8) as p:
        chk = p.map(bisect_check, jobs)
        rul = p.map(bisect_rule, jobs)
    for (lvl, dc, capok), (_, dr) in zip(chk, rul):
        print(f"level {lvl}: exact-check delta {dc:.4f} (cap passes {capok}), rule delta {dr:.4f}")
