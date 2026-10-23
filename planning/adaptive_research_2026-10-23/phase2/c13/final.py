"""C13: every table of P2_C13_lobe_margin.md.

  A  C10's growing bite at C10's reserved spacing (levels 9-15, five arms,
     both orientations): exact widths with walls, the law check without
     walls, and the planner-lattice readings (orientation +1, 4 phases,
     vertex spacing one cell and one pass step).
  B  The same bands under the C13 law (`tilt_min`).
  R  The spacing the exact check accepts (bisection, C13 law, the shortest
     arm), the corner-touch rule's spacing, and the full check at the
     accepted spacing on all arms with the lattice readings.
  K  Cost: the level sequence under the accepted spacing (17.10 to 31.05
     mm), checked band by band, against the current levels 9-15.

Run: python3 -I final.py   (about 15 minutes on 8 workers; writes
final.json next to this file). assert_c13.py reads final.json and
recomputes the cheap numbers itself.
"""
import json
import math
import os
import sys
from multiprocessing import Pool

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geom import (CD, CD_C10, CELL, STEP_LEN, R, S, reserve_delta, tilt,  # noqa: E402
                  w_front, width_no_wall)
from band import build, exact_band, lattice_band  # noqa: E402
from rule import corner_touch  # noqa: E402

# C10 reserve.py: level, law radius (mm), shortest star arm arc (mm)
C10_BANDS = [(9, 19.06, 14.2), (10, 21.05, 12.4), (11, 23.05, 10.6),
             (12, 25.05, 8.9), (13, 27.05, 7.2), (14, 29.05, 5.8), (15, 31.05, 4.2)]
# the C5 law levels 5..15 (C2 table and the C10 model; s 2 from level 11)
LAW = {5: 11.35, 6: 13.23, 7: 15.15, 8: 17.10, 9: 19.06, 10: 21.05, 11: 23.05, 12: 25.05, 13: 27.05,
       14: 29.05, 15: 31.05}
PHASES = [(0.0, 0.0), (0.125, 0.25), (0.25, 0.125), (0.375, 0.375)]
ARMS = range(5)
WORKERS = 8


def c10_law_check(rho, delta, n=4000):
    """Closed-form exact width of C10's tilt (built against 2.1756) at every
    bite, on the stock outside the last circle: the largest, and its bite."""
    best = (-1.0, 0.0)
    for b in np.linspace(delta / n, delta, n):
        be = tilt(b, delta, rho, CD_C10)
        w = width_no_wall(b, be, rho, delta)
        best = max(best, (w, b))
    return best


def check(job):
    """One band: exact (walls), law check (no walls), optional lattice."""
    lvl, rho, delta, arm, sgn, law, do_lattice, priors = job
    b = build(arm, rho, delta, sgn, law, priors)
    row = dict(level=lvl, rho=rho, delta=delta, arm=arm, sgn=sgn, law=law, L_b=b["L_b"],
               ramp_len=b["ramp_len"], spare=b["spare"], fits=b["fits"],
               margin=CD - w_front(rho, delta))
    if not b["fits"]:
        return row
    row["ramp_inside"] = b["ramp_inside"]
    nw = exact_band(b, walls=False)
    ex = exact_band(b, walls=True)
    row.update(nowall=nw["lead"], lead=ex["lead"], piece=ex["lead_piece"],
               wall=ex["lead_at"]["wall"], bite=ex["lead_at"]["bite"],
               ramp=ex["ramp"]["max"], fwd=ex["fwd"]["max"], back=ex["back"]["max"],
               arrival=ex["arrival"], plain_F=ex["plain_F"], plain_E=ex["plain_E"],
               plain_mid=ex["plain_mid"], formula=ex["formula"], touch=corner_touch(b))
    if do_lattice:
        lat = {}
        for name, sp in (("cell", CELL), ("step", STEP_LEN)):
            rs = [lattice_band(b, ph, sp, (ex["zone_F"], ex["zone_E"])) for ph in PHASES]
            lat[name] = dict(lead=[r["lead_max"] for r in rs], at=[r["lead_at"] for r in rs],
                             wall=[r["lead_wall"] for r in rs],
                             ramp=[r["ramp_max"] for r in rs], ramp_med=[r["ramp_min"] for r in rs],
                             ramp_over=[r["ramp_over"] for r in rs], n_ramp=rs[0]["n_ramp"],
                             lead_over=[r["lead_over"] for r in rs],
                             plain=[r["plain_max"] for r in rs], plain_med=[r["plain_mid"] for r in rs])
        row["lattice"] = lat
    return row


def bisect_accept(args):
    """Largest delta in [1, cap] whose band (C13 law, shortest arm, +1)
    passes the exact check; and the corner-touch rule's delta."""
    lvl, rho, cap = args

    def ok(d):
        b = build(0, rho, d, +1, "min")
        if not b["fits"] or not b["ramp_inside"]:
            return False
        return exact_band(b, True)["lead"] <= CD + 1e-6

    def rule(d):
        b = build(0, rho, d, +1, "min")
        return b["fits"] and b["spare"] >= corner_touch(b)

    out = []
    for f in (ok, rule):
        if f(cap):
            out.append(cap)
            continue
        lo, hi = 1.0, cap
        for _ in range(12):
            mid = (lo + hi) / 2
            if f(mid):
                lo = mid
            else:
                hi = mid
        out.append(lo)
    return lvl, out[0], out[1]


def law_cap(k):
    return min(S, LAW[k] - LAW[k - 1])


def sequence(acc):
    """Levels from 17.10 (level 8) to 31.05 mm under the accepted spacing,
    interpolated in rho between the accepted values at the law levels and
    held to the law step; the fixed point rho_k = rho_{k-1} + delta(rho_k),
    the last step cut to end at 31.05."""
    xs = np.array([LAW[k] for k in sorted(acc)])
    ys = np.array([acc[k] for k in sorted(acc)])
    seq = [LAW[8]]
    while seq[-1] < LAW[15] - 1e-9:
        r0 = seq[-1]
        d = float(np.interp(r0 + 2.0, xs, ys))
        for _ in range(20):
            d = min(float(np.interp(r0 + d, xs, ys)), S)
        seq.append(min(r0 + d, LAW[15]))
    return seq


def main():
    deltas = {lvl: reserve_delta(rho, L) for lvl, rho, L in C10_BANDS}
    law = {lvl: c10_law_check(rho, deltas[lvl]) for lvl, rho, _ in C10_BANDS}
    jobsA = [(lvl, rho, deltas[lvl], arm, sgn, "c10", sgn > 0, None)
             for lvl, rho, _ in C10_BANDS for arm in ARMS for sgn in (1, -1)]
    jobsB = [(lvl, rho, deltas[lvl], arm, sgn, "min", False, None)
             for lvl, rho, _ in C10_BANDS for arm in ARMS for sgn in (1, -1)]
    with Pool(WORKERS) as p:
        A = p.map(check, jobsA)
        B = p.map(check, jobsB)
        acc = p.map(bisect_accept, [(k, LAW[k], law_cap(k)) for k in range(9, 16)])
        accepted = {lvl: d for lvl, d, _ in acc}
        rule = {lvl: r for lvl, _, r in acc}
        jobsR = [(k, LAW[k], accepted[k], arm, sgn, "min", sgn > 0, None)
                 for k in range(9, 16) for arm in ARMS for sgn in (1, -1)]
        Rr = p.map(check, jobsR)
        seq = sequence(accepted)
        jobsK = []
        for i in range(1, len(seq)):
            # prior arcs: the earlier levels of the sequence, then the law
            # levels 7, 6, 5 (closed wraps round the core)
            pri = ([seq[i - j] for j in range(1, i + 1)] + [LAW[7], LAW[6], LAW[5]])[:6]
            jobsK.append([(8 + i, seq[i], seq[i] - seq[i - 1], arm, sgn, "min", False, pri)
                          for arm in ARMS for sgn in (1, -1)])
        jobsK = [j for g in jobsK for j in g]
        K = p.map(check, jobsK)
    out = dict(CD=CD, CD_C10=CD_C10, deltas=deltas, law_check=law, A=A, B=B,
               accepted=accepted, rule=rule, R=Rr, seq=seq, K=K)
    here = os.path.dirname(os.path.abspath(__file__))
    with open(os.path.join(here, "final.json"), "w") as f:
        json.dump(out, f, indent=1, default=float)
    return out


if __name__ == "__main__":
    main()
