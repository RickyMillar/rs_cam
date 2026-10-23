# C15: far-side alternatives for the pocket, each with its checking numbers.
# Run: python3 -I run_c15.py   (writes results.json and curves.npz here)
import json
import math
import os
import sys
import time

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from geom import (R, S, D, CD, C_FRAC, TOL, CELL, PASS_STEP, SEED, HOLE_R, ISL_C, P_UP, P_LO,
                  A_UP, B_LO, WEDGE, ANG_UP, ANG_LO, SEED_TO_ISL, rho_star, harness_levels,
                  hidden, geo, geo_side, seg_points)
import fields as Fm
from march import (Field, march, fixed_levels, signed_radius, front_width_v, law_spacing_v,
                   Replay, zone_footprint, coverage, SCALLOP, resample)
from harness_svg import wraps as harness_wraps
from scipy.spatial import cKDTree

OUT = HERE


def starter_pts():
    ang = np.linspace(0, 2 * np.pi, int(2 * np.pi * R / TOL))
    return np.vstack([SEED + R * np.stack([np.cos(ang), np.sin(ang)], 1), SEED[None]])


def edge_pts():
    return np.vstack([seg_points(P_UP, A_UP, TOL), seg_points(P_LO, B_LO, TOL)])


def runs(mask):
    """Index runs where mask is true."""
    out = []
    i = 0
    n = len(mask)
    while i < n:
        if mask[i]:
            j = i
            while j < n and mask[j]:
                j += 1
            out.append((i, j))
            i = j
        else:
            i += 1
    return out


def measure(curves, prior_pts, field, zone_only_split):
    """Gap, curvature and formula width of each curve against the cut set
    (prior paths plus earlier levels), restricted to the zone."""
    pts = [prior_pts]
    g_all, rho_all, w_all, rho_int = [], [], [], []
    arcs = 0
    length = 0.0
    for level in curves:
        tree = cKDTree(np.concatenate(pts))
        for P in level:
            g, _ = tree.query(P)
            rho = signed_radius(P, field)
            w = front_width_v(g, rho)
            hz = hidden(P[:, 0], P[:, 1]) if not zone_only_split else np.ones(len(P), bool)
            s = np.concatenate([[0], np.cumsum(np.hypot(*np.diff(P, axis=0).T))])
            interior = (s >= PASS_STEP) & (s <= s[-1] - PASS_STEP)
            for i, j in runs(hz):
                if j - i < 2:
                    continue
                arcs += 1
                length += s[j - 1] - s[i]
            g_all.append(g[hz])
            rho_all.append(rho[hz])
            w_all.append(w[hz])
            rho_int.append(rho[hz & interior])
        pts.extend(level)
    cat = lambda xs: np.concatenate(xs) if xs else np.zeros(0)
    g, rho, w, ri = cat(g_all), cat(rho_all), cat(w_all), cat(rho_int)
    fin = ri[np.isfinite(ri)]
    return dict(
        arcs=arcs,
        length_mm=length,
        max_gap_mm=float(g.max()),
        max_formula_frac=float(np.nanmax(w) / D),
        min_abs_rho_mm=float(np.abs(fin).min()) if len(fin) else math.inf,
        min_convex_rho_mm=float(fin[fin > 0].min()) if (fin > 0).any() else math.inf,
    )


def replay(plan_curves):
    """Raster replay of the curves in level order. Readings in the zone, split
    into the arc interior (more than R of arc length from either end) and the
    arc ends (the wall-end class of C7; the turns of C14 own it)."""
    rp = Replay()
    wi, we, ai, wn, ae, wne, ane = [], [], [], [], [], [], []
    over_d = []
    ani = []
    for level, is_zone in plan_curves:
        for P in level:
            w, a = rp.cut(P)
            s = np.concatenate([[0], np.cumsum(np.hypot(*np.diff(P, axis=0).T))])
            hz = np.ones(len(P), bool) if is_zone else hidden(P[:, 0], P[:, 1])
            inner = (s > R) & (s < s[-1] - R)
            wi.append(w[hz & inner])
            ov = hz & inner & (w / D > C_FRAC + TOL / D)
            over_d.append(np.hypot(P[ov, 0] - ISL_C[0], P[ov, 1] - ISL_C[1]) - HOLE_R)
            wn.append(w[~hz & inner])
            wne.append(w[~hz & ~inner])
            we.append(w[hz & ~inner])
            # area per mm over each pass step (the planner's step length)
            if s[-1] > 2 * R + PASS_STEP:
                cs = np.cumsum(a)
                k = max(1, int(round(PASS_STEP / max(s[-1] / (len(P) - 1), 1e-9))))
                apm = (cs[k:] - cs[:-k]) / (s[k:] - s[:-k])
                mid = 0.5 * (s[k:] + s[:-k])
                hzm = hz[k // 2: k // 2 + len(apm)]
                inm = (mid - PASS_STEP / 2 > R) & (mid + PASS_STEP / 2 < s[-1] - R)
                ai.append(apm[hzm & inm])
                ae.append(apm[hzm & ~inm])
                ane.append(apm[~hzm & ~inm])
                ani.append(apm[~hzm & inm])
    cat = lambda xs: np.concatenate(xs) if xs else np.zeros(0)
    wi, we, ai, wn, ae, wne, ane = cat(wi), cat(we), cat(ai), cat(wn), cat(ae), cat(wne), cat(ane)
    area, maxbd = coverage(rp.stock, zone_footprint())
    q = TOL / D
    return dict(
        sw_interior_max=float(wi.max() / D) if len(wi) else 0.0,
        sw_interior_over=int((wi / D > C_FRAC + q).sum()),
        sw_interior_samples=int(len(wi)),
        sw_near_interior_max=float(wn.max() / D) if len(wn) else 0.0,
        sw_near_end_max=float(wne.max() / D) if len(wne) else 0.0,
        apm_near_interior_max=float(cat(ani).max() / D) if len(cat(ani)) else 0.0,
        apm_near_end_max=float(ane.max() / D) if len(ane) else 0.0,
        sw_end_max=float(we.max() / D) if len(we) else 0.0,
        sw_end_over=int((we / D > C_FRAC + q).sum()),
        apm_interior_max=float(ai.max() / D) if len(ai) else 0.0,
        apm_end_max=float(ae.max() / D) if len(ae) else 0.0,
        over_hole_min_mm=float(cat(over_d).min()) if len(cat(over_d)) else None,
        over_hole_max_mm=float(cat(over_d).max()) if len(cat(over_d)) else None,
        residue_interior_mm2=area,
        residue_max_wall_distance_mm=maxbd,
    )


def v_corners(curves, branch_fn):
    """Branch changes along the zone part of each curve: each is a V where
    two fronts meet (a corner the one-side rule splits or turns at)."""
    n = 0
    for level in curves:
        for P in level:
            hz = hidden(P[:, 0], P[:, 1])
            for i, j in runs(hz):
                if j - i < 2:
                    continue
                b = branch_fn(P[i:j])
                n += int(np.count_nonzero(np.diff(b.astype(int))))
    return n


def pivot_distance(P):
    """True local front radius of the seed field: the distance to the seed
    where visible, else the tangent length to the grown island."""
    out = np.empty(len(P))
    hz = hidden(P[:, 0], P[:, 1])
    out[~hz] = np.hypot(P[~hz, 0] - SEED[0], P[~hz, 1] - SEED[1])
    d = np.hypot(P[hz, 0] - ISL_C[0], P[hz, 1] - ISL_C[1])
    out[hz] = np.sqrt(np.maximum(d * d - HOLE_R ** 2, 0))
    return out


def radius_check(curves, field):
    """Allowed gap from the pass-step radius reading minus the allowed gap
    from the true pivot distance, on the zone interior of convex points."""
    dl = []
    for P in curves:
        P = resample(P, TOL)
        if len(P) < 3:
            continue
        s = np.concatenate([[0], np.cumsum(np.hypot(*np.diff(P, axis=0).T))])
        inner = (s > PASS_STEP) & (s < s[-1] - PASS_STEP) & hidden(P[:, 0], P[:, 1])
        if not inner.any():
            continue
        rh = signed_radius(P, field)[inner]
        rt = pivot_distance(P)[inner]
        ok = rh > 0
        dl.append((law_spacing_v(rh) - law_spacing_v(rt))[ok])
    d = np.concatenate(dl)
    return dict(min_mm=float(d.min()), max_mm=float(d.max()), p95_abs_mm=float(np.percentile(np.abs(d), 95)))


def deviation(wr, value_fn, levels, near_too=False):
    """Harness wraps against an exact field: value minus the nearest level."""
    near, far = [], []
    lv = np.asarray(levels)
    for w in wr:
        hz = hidden(w[:, 0], w[:, 1])
        if hz.sum() == 0:
            if near_too:
                v = value_fn(w)
                k = np.argmin(np.abs(lv[:, None] - v[None, :]), axis=0)
                near.append(v - lv[k])
            continue
        v = value_fn(w[hz])
        k = np.argmin(np.abs(lv[:, None] - v[None, :]), axis=0)
        far.append(v - lv[k])
    f = np.concatenate(far)
    out = dict(n=int(len(f)), rms_mm=float(np.sqrt(np.mean(f ** 2))), max_abs_mm=float(np.abs(f).max()),
               mean_mm=float(f.mean()))
    if near_too:
        out["near_max_abs_mm"] = float(np.abs(np.concatenate(near)).max())
    return out


def wobble(wr, value_fn, levels):
    """Per far-side wrap: the standard deviation of (exact value - level)
    along the wrap (an offset that is the same along a wrap is not a wobble)."""
    sds = []
    lv = np.asarray(levels)
    for w in wr:
        hz = hidden(w[:, 0], w[:, 1])
        if hz.sum() < 5:
            continue
        v = value_fn(w[hz])
        sds.append(float(np.std(v)))
    return dict(wraps=len(sds), max_sd_mm=max(sds), median_sd_mm=float(np.median(sds)))


def main():
    t0 = time.time()
    res = {"inputs": dict(R=R, s=S, tolerance=TOL, cell=CELL, pass_step=PASS_STEP, c=C_FRAC, cD=CD,
                          rho_star=rho_star(), seed=SEED.tolist(), hole_r=HOLE_R,
                          seed_to_island=SEED_TO_ISL, ang_lo_deg=math.degrees(ANG_LO),
                          ang_up_deg=math.degrees(ANG_UP), wedge_deg=math.degrees(WEDGE),
                          P_up=P_UP.tolist(), P_lo=P_LO.tolist(), A=A_UP.tolist(), B=B_LO.tolist(),
                          zone_mm2=float(Fm.ZONE.sum() * TOL * TOL), scallop_mm=SCALLOP)}
    curves_out = {}
    T = Fm.seed_T()

    # First region (shared by every split idea): circles about the seed.
    fv = Field(np.where(Fm.VISIBLE, T, np.nan))
    lv1, cv1 = march(fv, starter_pts())
    res["first_region"] = dict(levels=len(lv1), harness_levels_dev_mm=float(
        np.max(np.abs(np.array(lv1) - np.array(harness_levels(80)[1:len(lv1) + 1])))))
    first_pts = np.concatenate([starter_pts()] + [P for lvl in cv1 for P in lvl])
    curves_out["first"] = cv1
    print("first region", len(lv1), time.time() - t0, flush=True)

    # (a) exact geodesic, whole region, local-pivot law.
    fa = Field(T)
    lva, cva = march(fa, starter_pts())
    ma = measure(cva, starter_pts(), fa, zone_only_split=False)
    ra = replay([(lvl, False) for lvl in cva])
    def seed_branch(P):
        up = geo_side(SEED[0], SEED[1], P[:, 0], P[:, 1], -1)
        lo = geo_side(SEED[0], SEED[1], P[:, 0], P[:, 1], +1)
        return up < lo

    res["a"] = dict(levels=len(lva), v_corners=v_corners(cva, seed_branch), **ma, **ra)
    curves_out["a"] = cva
    print("a", res["a"], time.time() - t0, flush=True)

    # (a0) exact geodesic at the harness levels (the overlay for hypothesis 1).
    hl = harness_levels(float(np.nanmax(T)))[1:]
    cva0 = fixed_levels(fa, hl)
    cva0 = [lvl for lvl in cva0 if lvl]
    ma0 = measure(cva0, starter_pts(), fa, zone_only_split=False)
    ra0 = replay([(lvl, False) for lvl in cva0])
    res["a0"] = dict(levels=len(cva0), v_corners=v_corners(cva0, seed_branch), **ma0, **ra0)
    curves_out["a0"] = cva0
    print("a0", res["a0"], time.time() - t0, flush=True)

    # (a)+(c): the fronts of (a) with the round union of (c) at the meeting
    # line (only in the zone, where both branches bend round the island).
    alpha0 = WEDGE / 2
    g_ax0 = S / math.sin(alpha0)
    rho_f0 = g_ax0 * (R - g_ax0 / 2) / (g_ax0 - CD)
    vf0 = math.sin(alpha0) / (math.sqrt(2) * math.cos(alpha0) ** 2)
    rb = rho_f0 / vf0
    Tz_up = np.full(T.shape, np.nan)
    Tz_lo = np.full(T.shape, np.nan)
    zx, zy = Fm.GX[Fm.ZONE], Fm.GY[Fm.ZONE]
    Tz_up[Fm.ZONE] = geo_side(SEED[0], SEED[1], zx, zy, -1) - R
    Tz_lo[Fm.ZONE] = geo_side(SEED[0], SEED[1], zx, zy, +1) - R
    diff = Tz_up - Tz_lo
    blend = Fm.ZONE & (np.abs(diff) < rb)
    Tac = T.copy()
    with np.errstate(invalid="ignore"):
        bl = (Tz_up + Tz_lo + np.sqrt(np.maximum(2 * rb ** 2 - diff ** 2, 0))) / 2 - rb
    Tac[blend] = bl[blend]
    fac = Field(Tac)
    lvac, cvac = march(fac, starter_pts())
    mac = measure(cvac, starter_pts(), fac, zone_only_split=False)
    rac = replay([(lvl, False) for lvl in cvac])
    res["a_c"] = dict(levels=len(lvac), v_corners=0, blend_mm2=float(blend.sum() * TOL * TOL), **mac, **rac)
    curves_out["a_c"] = cvac
    print("a_c", res["a_c"], time.time() - t0, flush=True)

    # Zone ideas: the first region is cut first; the zone field starts at the
    # exact cut edge.
    cut0 = np.concatenate([first_pts, edge_pts()])
    du, dl = Fm.zone_edges()
    alpha = WEDGE / 2
    # (c) blend radius: the largest concave fillet that keeps plain spacing at
    # the meeting line under the C5 width (derivation in the note), and the
    # blend parameter that gives it (vertex radius = r sin(a) / (sqrt2 cos^2 a)).
    g_axis = S / math.sin(alpha)
    rho_fillet = g_axis * (R - g_axis / 2) / (g_axis - CD)
    vertex_factor = math.sin(alpha) / (math.sqrt(2) * math.cos(alpha) ** 2)
    res["inputs"].update(g_axis_mm=g_axis, rho_fillet_mm=rho_fillet, vertex_factor=vertex_factor)
    zone_fields = {
        "c0": Fm.split_exact(),
        "b": Fm.harmonic(),
        "c": Fm.round_union(rho_fillet / vertex_factor),
        "c_star": Fm.round_union(rho_star() / vertex_factor),
    }
    # (d) one front from the cut edge whose farthest zone point is nearer.
    side = "lo" if np.nanmax(dl) <= np.nanmax(du) else "up"
    res["inputs"].update(d_side=side, d_up_max=float(np.nanmax(du)), d_lo_max=float(np.nanmax(dl)))
    zone_fields["d"] = Fm.one_side(side)
    for key, Fz in zone_fields.items():
        fz = Field(Fz)
        lvz, cvz = march(fz, cut0)
        mz = measure(cvz, cut0, fz, zone_only_split=True)
        rz = replay([(lvl, False) for lvl in cv1] + [(lvl, True) for lvl in cvz])
        vc = v_corners(cvz, lambda P: Fm.edge_distance(P_UP, A_UP, P[:, 0], P[:, 1])
                       < Fm.edge_distance(P_LO, B_LO, P[:, 0], P[:, 1])) if key == "c0" else 0
        res[key] = dict(levels=len(lvz), v_corners=vc, **mz, **rz)
        curves_out[key] = cvz
        print(key, res[key], time.time() - t0, flush=True)

    # Harness wraps against the exact fields.
    hwc = harness_wraps("pocket_geodesic_curv")
    hws = harness_wraps("pocket_geodesic_curv_split")
    seedT = lambda P: geo(SEED[0], SEED[1], P[:, 0], P[:, 1], far_only=False) - R
    res["h1_curv_vs_exact"] = deviation(hwc, seedT, hl, near_too=True)
    zl = harness_levels(60)[1:]

    def c0_exact(P):
        return np.fmin(Fm.edge_distance(P_UP, A_UP, P[:, 0], P[:, 1]),
                       Fm.edge_distance(P_LO, B_LO, P[:, 0], P[:, 1]))

    # the split column's zone wraps: those whose points lie in the zone and
    # whose value is not a seed level (the first region's wraps end at the line)
    zone_w = [w for w in hws if hidden(w[:, 0], w[:, 1]).mean() > 0.5]
    res["h2_split_vs_exact"] = deviation(zone_w, c0_exact, zl)
    res["h2_split_wobble"] = wobble(zone_w, c0_exact, zl)
    res["h1_curv_wobble"] = wobble([w for w in hwc if hidden(w[:, 0], w[:, 1]).sum() >= 5], seedT, hl)
    res["harness_zone_wraps"] = len(zone_w)
    res["radius_exact"] = radius_check([P for lvl in cva0 for P in lvl], fa)
    res["radius_lattice"] = radius_check(hwc, fa)
    print("harness", res["h1_curv_vs_exact"], res["h2_split_vs_exact"], res["h2_split_wobble"],
          res["h1_curv_wobble"], flush=True)

    res["a0_over_min_mm"] = res["a0"]["over_hole_min_mm"]
    res["a0_over_max_mm"] = res["a0"]["over_hole_max_mm"]
    res["a_extra_levels"] = res["a"]["levels"] - res["a0"]["levels"]
    res["a_c_extra_levels"] = res["a_c"]["levels"] - res["a"]["levels"]
    with open(os.path.join(OUT, "results.json"), "w") as f:
        json.dump(res, f, indent=1, default=float)
    flat = {}
    for k, cv in curves_out.items():
        for i, lvl in enumerate(cv):
            for j, P in enumerate(lvl):
                flat[f"{k}|{i}|{j}"] = P.astype(np.float32)
    np.savez_compressed(os.path.join(OUT, "curves.npz"), **flat)
    print("done", time.time() - t0)


if __name__ == "__main__":
    main()
