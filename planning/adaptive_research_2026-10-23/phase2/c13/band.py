"""C13: one lobe band of the growing-bite lead-in with a reserved margin,
on one star arm: the paths, the exact widths and the lattice readings."""
import math

import numpy as np

from geom import (CD, CD_C10, CELL, STEP_LEN, C, R, SEED, Grid, Stock, arc_range, ramp_samples,
                  arm_axis, boundary_dist, circle_pts, in_region, path_widths,
                  ramp_polar, ramp_polar_exact, ramp_polar_min, resample_polyline,
                  tilt, tilt_exact, tilt_min, subdivide_at, w_front)

N_PRIOR = 6  # prior arcs: the disc at rho reaches rho - R, and the wall
#              corners of arc j reach rho_j + R; six arcs of >= 1.5 mm put
#              the oldest corner below rho - R - 6 (out of reach).
PITCH = 0.05  # exact samples along the path (mm); the max is refined at PITCH / 25


def build(arm, rho, delta, sgn, law="c10", priors=None):
    """The band: prior arcs at rho - j delta (j = 1..6) clipped to the
    region piece that holds the arm axis; the ramp from E of arc a (bite 0)
    to full bite; arc b from there to F; arc b back from F to E. `law`:
    "c10" (EntryLaw tilt plus the circular excess, against C10's 2.1756),
    "exact" (`tilt_exact`) or "min" (`tilt_min`), both against the derived
    ceiling. `priors`: the prior arc radii, newest first (default: rho - j
    delta, j = 1..6); the first must be rho - delta."""
    th0 = arm_axis(arm)
    arcs = []
    radii = priors if priors is not None else [rho - j * delta for j in range(1, N_PRIOR + 1)]
    for r in radii:
        rg = arc_range(r, th0)
        if rg is not None:
            arcs.append((r, rg[0], rg[1]))
    ra, ta0, ta1 = arcs[0]
    tb0, tb1 = arc_range(rho, th0)
    E_a = ta0 if sgn > 0 else ta1
    E_b, F_b = (tb0, tb1) if sgn > 0 else (tb1, tb0)
    if law == "c10":
        th, rr = ramp_polar(rho, delta, E_a, sgn, CD_C10)
    else:
        got = (ramp_polar_exact if law == "exact" else ramp_polar_min)(rho, delta, E_a, sgn, CD)
        if got is None:
            return dict(arm=arm, rho=rho, delta=delta, sgn=sgn, fits=False, spare=float("-inf"),
                        L_b=abs(tb1 - tb0) * rho, L_a=abs(ta1 - ta0) * ra, ramp_len=float("inf"))
        th, rr = got
    th_P = th[-1]
    ramp = SEED + rr[:, None] * np.c_[np.cos(th), np.sin(th)]
    # the law heading at each node: tilt beta(b) outward from the tangent
    law_tilt = {"c10": lambda bb: tilt(bb, delta, rho, CD_C10),
                "exact": lambda bb: tilt_exact(bb, rho, delta, CD),
                "min": lambda bb: tilt_min(bb, rho, delta, CD)}[law]
    bites = rr - (rho - delta)
    betas = np.array([law_tilt(max(bb, bites[1])) for bb in bites])
    e_t = sgn * np.c_[-np.sin(th), np.cos(th)]
    e_r = np.c_[np.cos(th), np.sin(th)]
    heads = np.cos(betas)[:, None] * e_t + np.sin(betas)[:, None] * e_r
    out = dict(arm=arm, rho=rho, delta=delta, sgn=sgn, arcs=arcs, th0=th0,
               L_b=abs(tb1 - tb0) * rho, L_a=abs(ta1 - ta0) * ra,
               ramp_len=float(np.sum(np.hypot(*np.diff(ramp, axis=0).T))),
               spare=float(sgn * (F_b - th_P) * rho), E_b=E_b, F_b=F_b, th_P=th_P)
    out["fits"] = out["spare"] > 0
    if not out["fits"]:
        return out
    bd = boundary_dist(ramp[:, 0], ramp[:, 1])
    out["ramp_inside"] = bool((in_region(ramp[:, 0], ramp[:, 1]) | (bd >= R - 1e-9)).all())
    out.update(ramp=ramp, ramp_heads=heads, fwd=circle_pts(rho, th_P, F_b, 0.01),
               back=circle_pts(rho, F_b, E_b, 0.01))
    return out


def _widths(st, plain, P, piece, H=None):
    """Band widths at P, and the mask of the points that count as lead-in:
    all of the ramp; on fwd and back the points where the plain pass at the
    same point and heading (prior stock only) holds the ceiling. A point
    where the plain pass is over the ceiling is the plain wall arrival."""
    w = path_widths(st, P, H)
    if piece == "ramp" or plain is None:
        return w, np.ones(len(P), bool)
    return w, path_widths(plain, P, H) <= CD


def _max_lead(st, plain, P, piece, native=None, heads=None):
    """The largest lead-in width on a piece, refined round the best sample:
    on the ramp over every integrated node between its neighbours (with the
    law heading), elsewhere at PITCH / 25."""
    if native is not None:
        idx, P, _ = ramp_samples(native, PITCH)
        H = heads[idx]
    else:
        H = None
    w, keep = _widths(st, plain, P, piece, H)
    wk = np.where(keep, w, -1.0)
    arrival = float(w[~keep].max()) if (~keep).any() else -1.0
    i = int(np.argmax(wk))
    lo, hi = max(i - 1, 0), min(i + 1, len(P) - 1)
    best, at = float(wk[i]), P[i]
    if hi > lo:
        if native is not None:
            seg, hs = native[idx[lo]:idx[hi] + 1], heads[idx[lo]:idx[hi] + 1]
        else:
            seg, hs = resample_polyline(P[lo:hi + 1], PITCH / 25), None
        ws, ks = _widths(st, plain, seg, piece, hs)
        ws = np.where(ks, ws, -1.0)
        j = int(np.argmax(ws))
        if ws[j] > best:
            best, at = float(ws[j]), seg[j]
    return best, at, arrival


def arrival_zone(st, rho, th_from, th_to):
    """The plain pass at rho from th_from to th_to: its largest exact width
    (at the wall it reaches), the width at mid-arc, and the wall distance
    (mm, cutter centre to the inset wall) under which it is over the
    ceiling as it reaches the wall (0 when it never is)."""
    P = circle_pts(rho, th_from, th_to, PITCH)
    w = path_widths(st, P)
    d = boundary_dist(P[:, 0], P[:, 1]) - R
    over = np.flatnonzero(w > CD)
    over = over[over > len(P) // 2]
    zone = float(d[over[0]]) if over.size else 0.0
    return zone, float(w.max()), float(w[len(w) // 2])


def exact_band(b, walls=True):
    """Exact widths along ramp, forward and back. `lead` is the largest
    width the lead-in is answerable for (`_widths`); `arrival` the largest
    on points where the plain pass is over the ceiling too."""
    st = Stock(b["arcs"], b["th0"], walls)
    plain = Stock(b["arcs"], b["th0"], walls)
    zF, plainF, plain_mid = arrival_zone(plain, b["rho"], b["E_b"], b["F_b"])
    zE, plainE, _ = arrival_zone(plain, b["rho"], b["F_b"], b["E_b"])
    res = dict(zone_F=zF, zone_E=zE, plain_F=plainF, plain_E=plainE, plain_mid=plain_mid,
               formula=w_front(b["rho"], b["delta"]))
    lead = (-1.0, None, None)
    arrival = -1.0
    for name in ("ramp", "fwd", "back"):
        P = resample_polyline(b[name], PITCH)
        if name == "back":
            st.add_path(np.vstack([b["ramp"], b["fwd"]]))
        ramp_args = (b["ramp"], b["ramp_heads"]) if name == "ramp" else (None, None)
        wmax, at, arr = _max_lead(st, plain, P, name, *ramp_args)
        arrival = max(arrival, arr)
        res[name] = dict(max=wmax, at=at, wall=float(boundary_dist(at[0], at[1]) - R),
                         bite=float(np.hypot(*(at - SEED)) - (b["rho"] - b["delta"])))
        if wmax > lead[0]:
            lead = (wmax, name, res[name])
    res["lead"] = lead[0]
    res["lead_piece"] = lead[1]
    res["lead_at"] = lead[2]
    res["arrival"] = arrival
    return res


def lattice_band(b, phase=(0.0, 0.0), spacing=CELL, zones=(0.0, 0.0)):
    """Planner readings (x D, mm) of the band path with vertices `spacing`
    apart (CELL: the plan pieces as `lead_band` builds them; STEP_LEN: the
    planner's own pass step), after the prior arcs are stamped, and of the
    plain pass E -> F on a twin grid. `zones` = (zone_F, zone_E): a fwd or
    back step that ends nearer its wall than the exact arrival zone counts
    as arrival, the rest as lead."""
    g = Grid(phase=phase)
    for r, t0, t1 in reversed(b["arcs"]):
        g.stamp_path(subdivide_at(circle_pts(r, t0, t1, 1.0), CELL))
    g2 = Grid.__new__(Grid)
    g2.__dict__ = {k: (v.copy() if isinstance(v, np.ndarray) else v) for k, v in g.__dict__.items()}
    ramp = resample_polyline(b["ramp"], spacing)
    fwd = resample_polyline(b["fwd"], spacing)
    back = resample_polyline(b["back"], spacing)
    P = np.vstack([ramp, fwd[1:], back[1:]])
    rd = g.read_path(P) * 2 * R
    nr, nf = len(ramp) - 1, len(fwd) - 1
    ends = P[1:]
    wall = boundary_dist(ends[:, 0], ends[:, 1]) - R
    piece = np.array([0] * nr + [1] * nf + [2] * (len(back) - 1))
    half_f = np.arange(len(rd)) - nr > nf // 2
    half_b = np.arange(len(rd)) - nr - nf > (len(back) - 1) // 2
    arr = ((piece == 1) & half_f & (wall < zones[0])) | ((piece == 2) & half_b & (wall < zones[1]))
    plain = g2.read_path(resample_polyline(circle_pts(b["rho"], b["E_b"], b["F_b"], 0.01), spacing)) * 2 * R
    lead = rd[~arr]
    j = int(np.argmax(np.where(~arr, rd, -1.0)))
    names = ("ramp", "fwd", "back")
    return dict(max=float(rd.max()), lead_max=float(lead.max()), ramp_max=float(rd[:nr].max()),
                ramp_min=float(np.median(rd[:nr])), lead_at=names[piece[j]], lead_wall=float(wall[j]),
                ramp_over=int(np.sum(rd[:nr] > CD + 1e-12)), n_ramp=nr,
                arrival_max=float(rd[arr].max()) if arr.any() else 0.0,
                lead_over=int(np.sum(lead > CD + 1e-12)), over=int(np.sum(rd > CD + 1e-12)),
                plain_max=float(plain.max()), plain_over=int(np.sum(plain > CD + 1e-12)),
                plain_mid=float(np.median(plain)))
