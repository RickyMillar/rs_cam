# C17 additions: (1) engaged / air split, times and retracts for C15 (a),
# C15 (e) and the streamline marches; (2) the family between (a) and (e):
#   u_lam = (1 - lam) * T / T_max + lam * u_harm
# with T the exact entry distance (C15 (a)) and u_harm the harmonic field
# (C15 (e)), both 0 on the entry disc. Outside the region T keeps its closed
# form and u_harm is 1 (the wall value), so a level line that reaches a
# wall is cut there into an open arc; at lam = 1 no level reaches a wall.
# Levels: the C15 level rule (march.march), run until every node lies within
# one cell of a cut, the same stop for every lam (the last level of (e) is
# the wall). Plans: plan.Planner (FreeCAD link rule, R1 §4).
# Run: python3 -I sweep.py   (writes sweep.json and sweep_paths.npz)
import json
import math
import os
import sys
import time
from multiprocessing import Pool

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import stream as st  # noqa: E402
import plan as pn  # noqa: E402
import whole  # noqa: E402  (C15)
from march import Field, march, resample  # noqa: E402  (C15)
from stream import TOL, H  # noqa: E402

LAMS = [round(0.05 * i, 2) for i in range(21)]  # the sweep grid: 0.05 in lam
SHAPES = {}


class ClipField(Field):
    """C15 Field whose level lines are cut to the tool-centre region."""

    def __init__(self, F, sh):
        super().__init__(F, xs=sh.xs, ys=sh.ys, dom=sh.omega & ~sh.disc)
        self.sh = sh

    def lines(self, t):
        out = []
        for ln in self.gen.lines(t):
            if len(ln) < 2:
                continue
            P = resample(ln, TOL)
            # a point within one cell outside the region is the contour's
            # own error (bilinear on the 0.1 mm grid), not a wall crossing
            ins = self.sh.in_omega(P[:, 0], P[:, 1]) | (self.sh.wall_dist(P[:, 0], P[:, 1]) > -TOL)
            if self.sh.name == "star":
                ins &= st.STAR_INSIDE(P)
            if ins.all():
                out.append(P)
                continue
            k = np.nonzero(np.diff(np.concatenate([[0], ins.astype(int), [0]])))[0]
            for a, b in zip(k[0::2], k[1::2]):
                if b - a >= 2:
                    out.append(P[a:b])
        return out


def setup(nm):
    sh = st.Shape(nm)
    U = whole.harmonic(sh)
    T = np.maximum(sh.entry_dist(sh.GX, sh.GY), 0.0)
    Tmax = float(T[sh.omega].max())
    SHAPES[nm] = (sh, U, T / Tmax, Tmax)


def run_plan(sh, levels, trimmed=False):
    pl = pn.Planner(sh)
    pl.trimmed = trimmed
    for i, lv in enumerate(levels):
        pl.level = i
        pl.add_level(lv)
    m = pn.summarise(pl, sh)
    m["levels"] = len(levels)
    m["curves"] = sum(len(lv) for lv in levels)
    m["open_arcs"] = sum(not pn.is_closed(P) for lv in levels for P in lv)
    m["levels_multi"] = sum(len(lv) > 1 for lv in levels)
    m["levels_with_retract"] = len({p["level"] for p in pl.pieces if p["kind"] == "retract"})
    return m, pl


def store(pl):
    """Pieces for drawing: every 3rd point, the engaged flag per point."""
    out = []
    for p in pl.pieces:
        if p["kind"] == "retract":
            out.append(("retract", p["P"], np.zeros(len(p["P"]), bool)))
        else:
            idx = np.unique(np.r_[np.arange(0, len(p["P"]), 3), len(p["P"]) - 1])
            out.append((p["kind"], p["P"][idx], p["eng"][idx]))
    return out


def task(args):
    nm, key = args
    sh, U, Tn, Tmax = SHAPES[nm]
    t0 = time.time()
    if isinstance(key, float):
        F = (1 - key) * Tn + key * U
        F = np.where(sh.disc, 0.0, F)
        field = ClipField(F, sh)
        lv, cv = march(field, whole.starter(sh), max_levels=600, stop_within=TOL)
    else:
        cv = key_levels(nm, key.replace("_trim", ""))
    m, pl = run_plan(sh, cv, trimmed=isinstance(key, str) and key.endswith("_trim"))
    m["seconds"] = time.time() - t0
    print(nm, key, {k: (round(v, 3) if isinstance(v, float) else v) for k, v in m.items()}, flush=True)
    return nm, key, m, store(pl)


def key_levels(nm, key):
    from page_c17 import c15_curves, load
    if key in ("c15_a", "c15_e"):
        return c15_curves()[(nm, key[-1])]
    _, cur = load()
    return [[np.vstack([lp["P"].astype(float), lp["P"][:1].astype(float)])] for lp in cur[(nm, key)]]


def init():
    for nm in ("star", "pocket"):
        setup(nm)


def feasible(m):
    """lam* conditions: (1) no retract; (2) the cut parts' interior width at
    most c plus one cell (the replay quantum)."""
    return m["retracts"] == 0 and m["cut_width_max"] <= st.C_FRAC + TOL / st.D


def main():
    for nm in ("star", "pocket"):
        setup(nm)
    tasks = [(nm, k) for nm in ("star", "pocket") for k in ("c15_a", "c15_e", "h_step", "g_step", "c15_e_trim", "h_step_trim")]
    tasks += [(nm, float(l)) for nm in ("star", "pocket") for l in LAMS]
    with Pool(8, initializer=init) as pool:
        out = pool.map(task, tasks, chunksize=1)
        # refine: between the largest infeasible lam below the smallest
        # feasible one, a grid ten times finer, down to 0.001
        for nm in ("star", "pocket"):
            step = 0.05
            while step > 0.0015:
                ms = {k: m for n, k, m, _ in out if n == nm and isinstance(k, float)}
                feas = [l for l in sorted(ms) if feasible(ms[l])]
                if not feas or feas[0] == 0:
                    break
                lo = max(l for l in ms if l < feas[0])
                fine = step / 10 if step > 0.011 else 0.001
                extra = [(nm, round(lo + fine * i, 3)) for i in range(1, int(round((feas[0] - lo) / fine)))]
                extra = [e for e in extra if e[1] not in ms]
                if not extra:
                    break
                out += pool.map(task, extra, chunksize=1)
                step = fine
    res = {"lams": LAMS, "inputs": dict(F_cut=pn.F_CUT, F_link_dressup=pn.F_LINK_DRESSUP, clear=pn.CLEAR,
                                        route_ratio=pn.ROUTE_RATIO, cut_through=pn.CUT_THROUGH,
                                        Tmax={nm: SHAPES[nm][3] for nm in SHAPES})}
    flat = {}
    for nm, key, m, pieces in out:
        k = key if isinstance(key, str) else f"lam{key:.3f}"
        if not isinstance(key, str):
            m["lam"] = key
            m["feasible"] = feasible(m)
        res.setdefault(nm, {})[k] = m
        for i, (kind, P, eng) in enumerate(pieces):
            flat[f"{nm}|{k}|{i}|{kind}|P"] = P.astype(np.float32)
            flat[f"{nm}|{k}|{i}|{kind}|e"] = eng
    for nm in ("star", "pocket"):
        lam = sorted(v["lam"] for k, v in res[nm].items() if k.startswith("lam"))
        res[nm]["lam_list"] = lam
        feas = [l for l in lam if res[nm][f"lam{l:.3f}"]["feasible"]]
        res[nm]["feasible_lams"] = feas
        if feas:
            best = min(feas, key=lambda l: res[nm][f"lam{l:.3f}"]["time_cut_feed_min"])
            best_l = min(feas, key=lambda l: res[nm][f"lam{l:.3f}"]["time_link_feed_min"])
            res[nm]["lam_star"], res[nm]["lam_star_link_feed"] = best, best_l
        else:
            res[nm]["lam_star"] = None
    with open(os.path.join(HERE, "sweep.json"), "w") as f:
        json.dump(res, f, indent=1)
    np.savez_compressed(os.path.join(HERE, "sweep_paths.npz"), **flat)
    print("done", {nm: (res[nm]["lam_star"], res[nm]["feasible_lams"]) for nm in ("star", "pocket")})


if __name__ == "__main__":
    main()
