# C15 (e)/(f): whole-region morph fields on the pocket and the star.
#   (e) harmonic: Laplace u = 0 on the entry disc, u = 1 on every wall and
#       island (tool-centre space); Shortley-Weller arms at both boundaries.
#   (f) distance ratio: u = d_entry / (d_entry + d_wall).
# Levels by the same rule as run_c15 (march.march: the C5 law gap from each
# loop's own curvature). Run: python3 -I whole.py  (writes results_whole.json
# and curves_whole.npz)
import json
import math
import os
import sys
import time

import numpy as np
import scipy.sparse as sp
import scipy.sparse.linalg as spla

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from geom import R, S, D, C_FRAC, TOL, PASS_STEP
from shapes import Shape, H, STAR
from march import Field, march, signed_radius, front_width_v, Replay, SCALLOP
from scipy.spatial import cKDTree


def harmonic(sh):
    """u = 0 on the entry disc, u = 1 on the region boundary, Laplace inside."""
    dom = sh.omega & ~sh.disc
    ny, nx = dom.shape
    idx = -np.ones(dom.shape, int)
    zr, zc = np.nonzero(dom)
    idx[zr, zc] = np.arange(len(zr))
    n = len(zr)
    X0, Y0 = sh.GX[zr, zc], sh.GY[zr, zc]

    def in_dom(x, y):
        return sh.in_omega(x, y) & (np.hypot(x - sh.seed[0], y - sh.seed[1]) > R)

    rows, cols, vals = [], [], []
    diag = np.zeros(n)
    rhs = np.zeros(n)
    arms = {}
    for name, (dr, dc) in (("e", (0, 1)), ("w", (0, -1)), ("n", (1, 0)), ("s", (-1, 0))):
        rr, cc = zr + dr, zc + dc
        inb = (rr >= 0) & (rr < ny) & (cc >= 0) & (cc < nx)
        nb = np.full(n, -1)
        nb[inb] = idx[rr[inb], cc[inb]]
        frac = np.ones(n)
        val = np.zeros(n)
        out = nb < 0
        if out.any():
            x0, y0 = X0[out], Y0[out]
            x1, y1 = x0 + dc * H, y0 + dr * H
            lo = np.zeros(out.sum())
            hi = np.ones(out.sum())
            for _ in range(40):
                m = 0.5 * (lo + hi)
                ins = in_dom(x0 + m * (x1 - x0), y0 + m * (y1 - y0))
                lo = np.where(ins, m, lo)
                hi = np.where(ins, hi, m)
            xb, yb = x0 + hi * (x1 - x0), y0 + hi * (y1 - y0)
            frac[out] = np.maximum(hi, 1e-3)
            val[out] = np.where(np.hypot(xb - sh.seed[0], yb - sh.seed[1]) <= R + 1e-6, 0.0, 1.0)
        arms[name] = (nb, frac, val)
    for a, b in (("e", "w"), ("n", "s")):
        (nbp, hp, vp), (nbm, hm, vm) = arms[a], arms[b]
        cp = 2.0 / (hp * (hp + hm))
        cm = 2.0 / (hm * (hp + hm))
        diag -= cp + cm
        for nb, cf, v in ((nbp, cp, vp), (nbm, cm, vm)):
            m = nb >= 0
            rows.append(np.nonzero(m)[0])
            cols.append(nb[m])
            vals.append(cf[m])
            rhs[~m] -= cf[~m] * v[~m]
    rows.append(np.arange(n))
    cols.append(np.arange(n))
    vals.append(diag)
    A = sp.csr_matrix((np.concatenate(vals), (np.concatenate(rows), np.concatenate(cols))), shape=(n, n))
    u = spla.spsolve(A.tocsc(), rhs)
    U = np.where(sh.disc, 0.0, 1.0)  # outside the region: the wall value
    U[zr, zc] = u
    return U


def ratio(sh):
    de = np.maximum(sh.entry_dist(sh.GX, sh.GY), 0.0)
    dw = np.maximum(sh.wall_dist(sh.GX, sh.GY), 0.0)
    with np.errstate(invalid="ignore", divide="ignore"):
        u = de / (de + dw)
    u = np.where(sh.omega, u, 1.0)
    u = np.where(sh.disc, 0.0, u)
    return np.nan_to_num(u, nan=1.0)


def basin(sh, U):
    """The ratio field behind the island: its smallest value there, and the
    level at which the sublevel set grown from the entry first reaches it
    (bisection on connected components)."""
    from scipy import ndimage
    from geom import hidden
    dom = sh.omega & ~sh.disc
    zone = dom & hidden(sh.GX, sh.GY)
    seed_ij = (int(round((sh.seed[1] - sh.ys[0]) / H)), int(round((sh.seed[0] - sh.xs[0]) / H)))
    lo, hi = 0.0, 1.0
    for _ in range(40):
        t = 0.5 * (lo + hi)
        lab, _ = ndimage.label((U <= t) & (sh.omega))
        reach = lab == lab[seed_ij]
        if (reach & zone).any():
            hi = t
        else:
            lo = t
    return dict(zone_min=float(U[zone].min()), reach_level=hi)


def exact_T(sh):
    """(a): the exact geodesic distance from the entry disc, NaN outside the
    region (its wraps end on the walls as open arcs)."""
    T = np.full(sh.GX.shape, np.nan)
    T[sh.omega] = sh.entry_dist(sh.GX[sh.omega], sh.GY[sh.omega])
    return T


def starter(sh):
    ang = np.linspace(0, 2 * np.pi, int(2 * np.pi * R / TOL))
    return np.vstack([sh.seed + R * np.stack([np.cos(ang), np.sin(ang)], 1), sh.seed[None]])


def loop_metrics(sh, field, curves):
    pts = [starter(sh)]
    n_loops = 0
    closed = 0
    multi = 0
    max_per = 0
    g_all, rho_all, w_all = [], [], []
    ratios = []
    length = 0.0
    wide_tip, narrow_inner, n_loc = 0, 0, 0
    tips, inner = (STAR[0::2], STAR[1::2]) if sh.name == "star" else (None, None)
    for lvl in curves:
        tree = cKDTree(np.concatenate(pts))
        gl = []
        n_loops += len(lvl)
        max_per = max(max_per, len(lvl))
        multi += len(lvl) > 1
        for P in lvl:
            closed += bool(np.hypot(*(P[0] - P[-1])) <= TOL)
            g, _ = tree.query(P)
            rho = signed_radius(P, field)
            g_all.append(g)
            rho_all.append(rho)
            w_all.append(front_width_v(g, rho))
            gl.append(g)
            length += float(np.hypot(*np.diff(P, axis=0).T).sum())
        g = np.concatenate(gl)
        ratios.append(g.min() / g.max())
        if tips is not None and len(ratios) > 1:
            Q = np.concatenate(lvl)
            def nearer_tip(p):
                return np.hypot(*(tips - p).T).min() < np.hypot(*(inner - p).T).min()
            wide_tip += nearer_tip(Q[np.argmax(g)])
            narrow_inner += not nearer_tip(Q[np.argmin(g)])
            n_loc += 1
        pts.extend(lvl)
    g, rho, w = np.concatenate(g_all), np.concatenate(rho_all), np.concatenate(w_all)
    fin = rho[np.isfinite(rho)]
    ratios = np.array(ratios[1:])  # level 1 is measured from the starter lap
    return dict(levels=len(curves), loops=n_loops, closed=closed, open_arcs=n_loops - closed,
                levels_multi=int(multi),
                max_loops_per_level=max_per, length_mm=length, max_gap_mm=float(g.max()),
                max_formula_frac=float(np.nanmax(w) / D),
                min_abs_rho_mm=float(np.abs(fin).min()), min_convex_rho_mm=float(fin[fin > 0].min()),
                worst_gap_ratio=float(ratios.min()), median_gap_ratio=float(np.median(ratios)),
                widest_near_tip=int(wide_tip), narrowest_near_inner=int(narrow_inner), located_loops=int(n_loc))


def replay(sh, curves):
    rp = Replay(stock0=sh.stock0, mx=sh.mx, my=sh.my, seed=sh.seed)
    wi, we, ai = [], [], []
    ov_lv, ov_wall, ov_tip = set(), [], []
    tips = STAR[0::2] if sh.name == "star" else None  # the five outer vertices
    for li, lvl in enumerate(curves):
        for P in lvl:
            w, a = rp.cut(P)
            s = np.concatenate([[0], np.cumsum(np.hypot(*np.diff(P, axis=0).T))])
            inner = (s > R) & (s < s[-1] - R)
            ov = inner & (w / D > C_FRAC + TOL / D)
            if ov.any():
                ov_lv.add(li)
                ov_wall.append(sh.wall_dist(P[ov, 0], P[ov, 1]))
                if tips is not None:
                    dt = np.hypot(P[ov, 0][:, None] - tips[None, :, 0], P[ov, 1][:, None] - tips[None, :, 1])
                    ov_tip.append(dt.argmin(1))
            wi.append(w[inner])
            we.append(w[~inner])
            if s[-1] > 2 * R + PASS_STEP:
                cs = np.cumsum(a)
                k = max(1, int(round(PASS_STEP / (s[-1] / (len(P) - 1)))))
                apm = (cs[k:] - cs[:-k]) / (s[k:] - s[:-k])
                mid = 0.5 * (s[k:] + s[:-k])
                inm = (mid - PASS_STEP / 2 > R) & (mid + PASS_STEP / 2 < s[-1] - R)
                ai.append(apm[inm])
    wi, we, ai = np.concatenate(wi), np.concatenate(we), np.concatenate(ai)
    res = rp.stock & sh.stock0
    bd = sh.boundary_dist(sh.MGX[res], sh.MGY[res]) if res.any() else np.zeros(0)
    interior = bd > SCALLOP + H
    q = TOL / D
    return dict(sw_interior_max=float(wi.max() / D), sw_interior_over=int((wi / D > C_FRAC + q).sum()),
                sw_seam_max=float(we.max() / D), apm_interior_max=float(ai.max() / D),
                residue_interior_mm2=float(interior.sum() * H * H),
                residue_max_wall_distance_mm=max(float(bd.max()), 0.0) + 0.0 if len(bd) else 0.0,
                over_levels=sorted(ov_lv), over_wall_max=float(np.concatenate(ov_wall).max()) if ov_wall else 0.0,
                over_tips=sorted(set(np.concatenate(ov_tip).tolist())) if ov_tip else [])


def main():
    t0 = time.time()
    out = {}
    flat = {}
    for name in ("pocket", "star"):
        sh = Shape(name)
        out[name] = dict(region_mm2=float(sh.omega.sum() * H * H))
        if name == "star":
            out[name]["visible_fraction"] = sh.visible_from_seed()
        for key, fn in (("e", harmonic), ("f", ratio), ("a", exact_T)):
            U = fn(sh)
            field = Field(U, xs=sh.xs, ys=sh.ys, dom=sh.omega & ~sh.disc)
            # (e), (f): the last loop is the wall; (a): the run_c15 stop (its
            # wraps end on the walls, so the last one need not be the wall)
            lv, cv = march(field, starter(sh), max_levels=600, stop_within=TOL if key != "a" else S)
            m = loop_metrics(sh, field, cv)
            r = replay(sh, cv)
            out[name][key] = dict(**m, **r, last_level=float(lv[-1]) if lv else 0.0)
            if key == "f" and name == "pocket":
                out[name][key].update(basin(sh, U))
            print(name, key, out[name][key], round(time.time() - t0, 1), flush=True)
            for i, lvl in enumerate(cv):
                for j, P in enumerate(lvl):
                    flat[f"{name}|{key}|{i}|{j}"] = P.astype(np.float32)
        a = out[name]["a"]
        for key in ("e", "f"):
            r = out[name][key]
            r["extra_levels_vs_a"] = r["levels"] - a["levels"]
            r["length_vs_a"] = r["length_mm"] / a["length_mm"]
    with open(os.path.join(HERE, "results_whole.json"), "w") as f:
        json.dump(out, f, indent=1)
    np.savez_compressed(os.path.join(HERE, "curves_whole.npz"), **flat)
    print("done", round(time.time() - t0, 1))


if __name__ == "__main__":
    main()
