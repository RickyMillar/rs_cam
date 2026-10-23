# C17: run the streamline march on the star and the pocket and write
# results_c17.json and curves_c17.npz. Run: python3 -I run_c17.py
# Runs:
#   h_step:   harmonic direction, step along the streamline = law gap (the
#             lead's rule as briefed)
#   h_normal: harmonic direction, step until the distance to the cut (the
#             normal gap) = law gap
#   g_step:   control, the exact entry distance's gradient, step rule
import json
import math
import os
import sys
import time

import numpy as np
from scipy.spatial import cKDTree

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import stream as st  # noqa: E402
from stream import R, S, D, CD, C_FRAC, TOL, PASS_STEP, H  # noqa: E402
from march import Replay, resample  # noqa: E402  (C15)

C15 = st.C15
RUNS = (("h_step", "harmonic", "step"), ("h_normal", "harmonic", "normal"), ("g_step", "geodesic", "step"))


def pct(x, q):
    return float(np.percentile(x, q)) if len(x) else 0.0


def loop_metrics(sh, loops):
    """Topology, gaps, lengths, smoothness of a march."""
    cut = [st.starter(sh)]
    ratio_all, gap_ratio_loop, normal_all, step_all = [], [], [], []
    L_tot = L_wall = L_air = 0.0
    chord = 0.0
    runs = 0
    dbl = []
    xing = 0
    xing_at = []
    rho_min_free, rho_min_any = np.inf, np.inf
    turn = []
    first_stop = None
    for k, lp in enumerate(loops):
        P = lp["P"]
        tree = cKDTree(np.concatenate(cut))
        moved = ~lp["was_stopped"]
        free = moved & ~lp["stop"]  # moved its full step this loop
        g = tree.query(P)[0]
        if free.any():
            stp = lp["step"][free]
            ok = stp > 1e-9
            normal_all.append(g[free][ok])
            step_all.append(stp[ok])
            ratio_all.append(g[free][ok] / stp[ok])
            if k > 0:
                gl = g[free]
                gap_ratio_loop.append(gl.min() / gl.max())
        if first_stop is None and lp["stop"].any():
            first_stop = k
        sl = st.seglen(P)
        both = lp["was_stopped"] & np.roll(lp["was_stopped"], -1)
        kd = lp["kind"]
        airseg = both & ((kd == 2) | (np.roll(kd, -1) == 2))
        L_tot += sl.sum()
        L_wall += sl[both & ~airseg].sum()
        L_air += sl[airseg].sum()
        # each maximal run of stopped segments: the straight chord between
        # its ends (a lower bound for a link that skips the run)
        if both.any() and not both.all():
            st_ = np.nonzero(both & ~np.roll(both, 1))[0]
            for a in st_:
                b = a
                while both[b % len(P)]:
                    b += 1
                chord += float(np.hypot(*(P[b % len(P)] - P[a])))
                runs += 1
        dbl.append(st.doubled_length(P))
        c_, p_ = st.self_crossings(P, where=True)
        xing += c_
        xing_at += [[k + 1, float(p[0]), float(p[1])] for p in p_]
        rho = st.radius_closed(P)
        # radius on the moving part, away (more than one pass step of arc)
        # from any stop, and anywhere on the moving part
        s = np.concatenate([[0], np.cumsum(sl)])[:-1]
        T = sl.sum()
        stopidx = np.nonzero(lp["stop"])[0]
        if len(stopidx):
            du = np.abs(s[:, None] - s[stopidx][None, :])
            du = np.minimum(du, T - du).min(1)
        else:
            du = np.full(len(P), np.inf)
        mv = moved & np.isfinite(rho)
        if mv.any():
            rho_min_any = min(rho_min_any, float(np.abs(rho[mv]).min()))
        fr = mv & (du > PASS_STEP)
        if fr.any():
            rho_min_free = min(rho_min_free, float(np.abs(rho[fr]).min()))
        # kinks: turning angle over +- one pass step at each change between
        # a stopped and a moving point
        stp_ = lp["stop"]
        ch = np.nonzero(stp_ != np.roll(stp_, 1))[0]
        Pc = np.vstack([P, P[:1]])
        sc = np.concatenate([[0], np.cumsum(sl)])

        def at(q):
            q = np.mod(q, T)
            return np.array([np.interp(q, sc, Pc[:, 0]), np.interp(q, sc, Pc[:, 1])])

        for i in ch:
            a, b, c = at(s[i] - PASS_STEP), P[i], at(s[i] + PASS_STEP)
            u, v = b - a, c - b
            nu, nv = np.hypot(*u), np.hypot(*v)
            if nu > 0 and nv > 0:
                turn.append(math.degrees(math.acos(np.clip((u @ v) / (nu * nv), -1, 1))))
        cut.append(P)
    ratio_all = np.concatenate(ratio_all)
    normal_all = np.concatenate(normal_all)
    step_all = np.concatenate(step_all)
    turn = np.array(turn) if turn else np.zeros(1)
    return dict(
        loops=len(loops), closed=len(loops), self_crossings=int(xing), crossings_at=xing_at,
        all_stopped=bool(loops[-1]["stop"].all()),
        first_stop_loop=int(first_stop + 1) if first_stop is not None else 0,
        loops_with_doubled=int(sum(d > 0 for d in dbl)), doubled_max_mm=float(max(dbl)),
        length_mm=float(L_tot), wall_follow_mm=float(L_wall), air_follow_mm=float(L_air),
        moving_mm=float(L_tot - L_wall - L_air),
        stopped_runs=int(runs), run_chord_mm=float(chord),
        air_stops=int(sum(((lp["kind"] == 2) & ~lp["was_stopped"]).sum() for lp in loops)),
        rule_violation_max_mm=float(max(lp["violation"] for lp in loops)),
        solve_residual_max_mm=float(max(lp["residual"] for lp in loops)),
        normal_over_step_min=float(ratio_all.min()), normal_over_step_p5=pct(ratio_all, 5),
        normal_over_step_median=pct(ratio_all, 50),
        normal_gap_max_mm=float(normal_all.max()), step_max_mm=float(step_all.max()),
        gap_ratio_median=float(np.median(gap_ratio_loop)) if gap_ratio_loop else 1.0,
        gap_ratio_worst=float(np.min(gap_ratio_loop)) if gap_ratio_loop else 1.0,
        rho_min_free_mm=float(rho_min_free), rho_min_moving_mm=float(rho_min_any),
        kink_count=int(len(turn)), kink_max_deg=float(turn.max()), kink_median_deg=float(np.median(turn)),
        kink_p90_deg=pct(turn, 90),
        tail_loops_lt10=int(sum((~lp["was_stopped"]).sum() < 10 for lp in loops)),
    )


def replay_metrics(sh, loops):
    out, res = st.replay_loops(sh, loops)
    w, cls, apm = out["w"], out["cls"], out["apm"]
    q = TOL / D
    inner = w[cls == "interior"]
    nz = inner[inner > 0]
    r = dict(
        sw_interior_max=float(inner.max()), sw_interior_over=int((inner > C_FRAC + q).sum()),
        sw_interior_p50=pct(nz, 50), sw_interior_p90=pct(nz, 90), sw_interior_p99=pct(nz, 99),
        sw_interior_n=int(len(nz)),
        sw_junction_max=float(w[cls == "junction"].max()) if (cls == "junction").any() else 0.0,
        sw_wall_max=float(w[cls == "wall"].max()) if (cls == "wall").any() else 0.0,
        sw_seam_max=float(w[cls == "seam"].max()) if (cls == "seam").any() else 0.0,
        apm_interior_max=float(apm.max()) if len(apm) else 0.0,
        mean_width_frac=float(out["area"] / out["length"] / D),
        removed_mm2=float(out["area"]),
    )
    # where the interior samples over c lie: distance to the wall, loops
    ov = (cls == "interior") & (w > C_FRAC + q)
    # interior samples farther than R from every wall and the island
    allxy = out["xy"][cls == "interior"]
    far = sh.wall_dist(allxy[:, 0], allxy[:, 1]) > R
    r["sw_far_max"] = float(inner[far].max()) if far.any() else 0.0
    r["sw_far_over"] = int((inner[far] > C_FRAC + q).sum())
    r["sw_far_n"] = int(far.sum())
    if ov.any():
        xy = out["xy"][ov]
        wd = sh.wall_dist(xy[:, 0], xy[:, 1])
        r["over_wall_max_mm"] = float(wd.max())
        r["over_within_R_of_wall"] = int((wd <= R).sum())
        r["over_loops"] = sorted(set((out["li"][ov] + 1).tolist()))
        if sh.name == "star":
            tips = st.STAR[0::2]
            dt = np.hypot(xy[:, 0][:, None] - tips[None, :, 0], xy[:, 1][:, None] - tips[None, :, 1]).min(1)
            r["over_tip_dist_max_mm"] = float(dt.max())
    else:
        r["over_wall_max_mm"] = 0.0
        r["over_within_R_of_wall"] = 0
        r["over_loops"] = []
    r.update({"residue_" + k: v for k, v in st.residue(sh, res).items()})
    return r


def open_replay(sh, curves):
    """Removed area and cut length for C15's (a) and (e) curves, cut in order
    (the C15 replay), for the mean width."""
    rp = Replay(stock0=sh.stock0, mx=sh.mx, my=sh.my, seed=sh.seed)
    A = L = 0.0
    for lvl in curves:
        for P in lvl:
            Q = resample(P, TOL)
            w, a = rp.cut(Q)
            A += float(a.sum())
            L += float(np.hypot(*np.diff(Q, axis=0).T).sum())
    return dict(removed_mm2=A, length_mm=L, mean_width_frac=A / L / D)


def load_c15_curves():
    d = np.load(os.path.join(C15, "curves_whole.npz"))
    cur = {}
    for key in d.files:
        nm, k, i, j = key.split("|")
        cur.setdefault((nm, k), {}).setdefault(int(i), []).append((int(j), d[key].astype(float)))
    return {kk: [[P for _, P in sorted(v[i], key=lambda t: t[0])] for i in sorted(v)] for kk, v in cur.items()}


def control_vs_a(sh, loops, a_curves):
    """Control: loops before the first stop against (a)'s wraps at the same
    index (one closed wrap): the largest radial difference (mm)."""
    out = []
    for k, lp in enumerate(loops):
        if lp["stop"].any() or k >= len(a_curves) or len(a_curves[k]) != 1:
            break
        r1 = np.hypot(*(lp["P"] - sh.seed).T)
        r2 = np.hypot(*(a_curves[k][0] - sh.seed).T)
        out.append(max(abs(r1.max() - r2.mean()), abs(r1.min() - r2.mean())))
    return dict(loops_compared=len(out), max_radial_diff_mm=float(max(out)) if out else 0.0)


def main():
    t0 = time.time()
    c15res = json.load(open(os.path.join(C15, "results_whole.json")))
    c15cur = load_c15_curves()
    out = dict(inputs=dict(R=R, s=S, tol=TOL, D=D, c=C_FRAC, cD=CD, pass_step=PASS_STEP, cell=H))
    flat = {}
    for nm in ("star", "pocket"):
        sh = st.Shape(nm)
        fields = {"harmonic": st.harmonic_field(sh), "geodesic": st.geodesic_field(sh)}
        out[nm] = {}
        for key, fld, mode in RUNS:
            dirf = st.Direction(sh, fields[fld])
            loops = st.march(sh, dirf, mode, air_move=True, max_loops=600)
            m = loop_metrics(sh, loops)
            m.update(replay_metrics(sh, loops))
            if key == "g_step":
                m.update(control_vs_a(sh, loops, c15cur[(nm, "a")]))
            out[nm][key] = m
            print(nm, key, json.dumps(m), round(time.time() - t0, 1), flush=True)
            for i, lp in enumerate(loops):
                flat[f"{nm}|{key}|{i}|P"] = lp["P"].astype(np.float32)
                flat[f"{nm}|{key}|{i}|stop"] = lp["stop"]
                flat[f"{nm}|{key}|{i}|was"] = lp["was_stopped"]
                flat[f"{nm}|{key}|{i}|kind"] = lp["kind"].astype(np.int8)
        for k in ("a", "e"):
            r = dict(c15res[nm][k])
            rr = open_replay(sh, c15cur[(nm, k)])
            r.update(removed_mm2=rr["removed_mm2"], mean_width_frac=rr["mean_width_frac"],
                     replay_length_mm=rr["length_mm"])
            out[nm]["c15_" + k] = {kk: r[kk] for kk in ("levels", "loops", "open_arcs", "length_mm", "sw_interior_max",
                                                        "apm_interior_max", "median_gap_ratio", "residue_interior_mm2",
                                                        "residue_max_wall_distance_mm", "removed_mm2", "mean_width_frac", "replay_length_mm",
                                                        "min_abs_rho_mm", "levels_multi")}
            print(nm, k, out[nm]["c15_" + k], round(time.time() - t0, 1), flush=True)
        out[nm]["region_mm2"] = float(sh.omega.sum() * H * H)
    with open(os.path.join(HERE, "results_c17.json"), "w") as f:
        json.dump(out, f, indent=1)
    np.savez_compressed(os.path.join(HERE, "curves_c17.npz"), **flat)
    print("done", round(time.time() - t0, 1))


if __name__ == "__main__":
    main()
