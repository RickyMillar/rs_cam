# C19: the streamline march clipped to its cut pieces, split per lobe,
# ordered lobe by lobe and linked one-way (C14) or zigzag. The same pipeline
# on C15 (a) for comparison. Run: python3 -I run_c19.py (about a minute);
# writes results_c19.json and paths_c19.npz.
import json
import math
import os
import sys
import time

import numpy as np
from scipy.spatial import cKDTree

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import clip as cl  # noqa: E402
import build as bd  # noqa: E402
from clip import R, S, D, CD, C_FRAC, TOL, PASS_STEP, CELL, TURN_BOUND, QUANTUM, pn, st  # noqa: E402


def on_curve(P, V):
    """Largest distance from the points P to the polyline V."""
    tree = cKDTree(V)
    _, nn = tree.query(P, k=min(6, len(V)))
    nn = np.atleast_2d(nn)
    best = np.full(len(P), np.inf)
    for col in range(nn.shape[1]):
        for off in (-1, 0):
            a = np.clip(nn[:, col] + off, 0, len(V) - 2)
            A, B = V[a], V[a + 1]
            d = B - A
            L2 = np.maximum((d * d).sum(1), 1e-300)
            t = np.clip(((P - A) * d).sum(1) / L2, 0, 1)
            best = np.minimum(best, np.hypot(*(P - (A + t[:, None] * d)).T))
    return float(best.max())


def q(x, p):
    return float(np.percentile(x, p)) if len(x) else 0.0


def summarise(sh, cls, B, lobes):
    pl = B.pl
    out = {}
    lens = {}
    eng = air = 0.0
    turn_time = 0.0
    lens_eng = {}
    w_int, w_all, over_at = [], [], []
    for p in pl.pieces:
        k = p["kind"]
        if k == "retract":
            continue
        ds = np.diff(p["s"])
        e = p["eng"]
        seg_e = 0.5 * (e[1:].astype(float) + e[:-1])
        le = float((ds * seg_e).sum())
        L = float(ds.sum())
        lens[k] = lens.get(k, 0.0) + L
        lens_eng[k] = lens_eng.get(k, 0.0) + le
        if k == "turn":
            turn_time += L / (pn.F_CUT * p["feed_factor"])
            continue
        eng += le
        air += L - le
        if k == "cut":
            m = pn.run_interior(p["s"], e) & p["interior"]
            if m.any():
                w_int.append(p["w"][m] / D)
                o = m & (p["w"] / D > C_FRAC + QUANTUM)
                for t in np.nonzero(o)[0]:
                    over_at.append(dict(x=float(p["P"][t, 0]), y=float(p["P"][t, 1]), w=float(p["w"][t] / D),
                                        level=int(B.pieces[p["src"]]["level"]), lobe=int(p["lobe"]),
                                        s=float(p["s"][t]), L=float(p["s"][-1])))
            if e.any():
                w_all.append(p["w"][e] / D)
    w_int = np.concatenate(w_int) if w_int else np.zeros(1)
    w_all = np.concatenate(w_all) if w_all else np.zeros(1)
    out["by_place"] = by_place(sh, B)
    out.update(engagement(B))
    out["refusals"] = out["by_place"]["refusals"]
    J = B.joins
    cnt = lambda f: int(sum(1 for j in J if f(j)))  # noqa: E731
    plunge = [j for j in J if j["kind"] == "plunge"]
    T = B.turns
    sw = np.array([t["swept"] for t in T]) if T else np.zeros(0)
    ar = np.array([t["area"] for t in T]) if T else np.zeros(0)
    ff = np.array([t["factor"] for t in T]) if T else np.zeros(0)
    tl = np.array([t["length"] for t in T]) if T else np.zeros(0)
    over_b = [j for j in J if j.get("over_bound")]
    res = st.residue(sh, pl.rp.stock & sh.stock0)
    keep = lens.get("link", 0.0) + lens.get("return", 0.0)
    out.update(
        pieces=len(B.pieces), rings=sum(p["ring"] for p in B.pieces), lobes=len(lobes),
        joins=len(J), plunges=len(plunge), entries=1 + len(plunge) + out["refusals"],
        plunge_seam=cnt(lambda j: j["kind"] == "plunge" and j["cls"] == "seam"),
        plunge_arc=cnt(lambda j: j["kind"] == "plunge" and j["cls"] == "arc"),
        plunge_lobe=cnt(lambda j: j["kind"] == "plunge" and j["cls"] == "lobe"),
        plunge_feed=cnt(lambda j: j["kind"] == "plunge" and j.get("route") == "feed"),
        plunge_over_bound=cnt(lambda j: j["kind"] == "plunge" and j.get("route") == "over bound"),
        plunge_link_over=cnt(lambda j: j["kind"] == "plunge" and j.get("route") == "link over c"),
        plunge_freecad=cnt(lambda j: j["kind"] == "plunge" and j.get("route") == "retract"),
        seam_joins=cnt(lambda j: j["cls"] == "seam" and j["kind"] != "none"),
        seam_links=cnt(lambda j: j["cls"] == "seam" and j["kind"] == "link"),
        jump_joins=cnt(lambda j: j["cls"] == "jump"), jump_links=cnt(lambda j: j["cls"] == "jump" and j["kind"] == "link"),
        plunge_jump=cnt(lambda j: j["kind"] == "plunge" and j["cls"] == "jump"),
        chain_links=cnt(lambda j: j["cls"] == "chain"),
        lobe_joins=cnt(lambda j: j["cls"] == "lobe"), lobe_links=cnt(lambda j: j["cls"] == "lobe" and j["kind"] == "link"),
        arc_joins=cnt(lambda j: j["cls"] == "arc"), arc_links=cnt(lambda j: j["cls"] == "arc" and j["kind"] == "link"),
        turns=len(T), over_bound=len(over_b), outside=cnt(lambda j: j.get("outside")),
        over_bound_fit=int(sum(1 for j in over_b if "slanted_bound" in j and j["turn_len"] <= j["slanted_bound"])),
        turn_len_min=float(tl.min()) if len(tl) else 0.0, turn_len_max=float(tl.max()) if len(tl) else 0.0,
        turn_over=int(sum(t["over"] for t in T)),
        turn_swept_q=[q(sw, 0), q(sw, 25), q(sw, 50), q(sw, 75), q(sw, 100)],
        turn_area_q=[q(ar, 0), q(ar, 25), q(ar, 50), q(ar, 75), q(ar, 100)],
        turn_factor_q=[q(ff, 0), q(ff, 25), q(ff, 50), q(ff, 75), q(ff, 100)],
        turn_mm=float(tl.sum()), turn_time_min=turn_time,
        return_mm=lens.get("return", 0.0), return_along=int(sum(r["along"] for r in B.returns)),
        return_routed=int(sum(not r["along"] for r in B.returns)), return_eng_mm=lens_eng.get("return", 0.0),
        link_mm=lens.get("link", 0.0), keepdown_mm=keep,
        cut_mm=lens.get("cut", 0.0), cut_eng_mm=lens_eng.get("cut", 0.0),
        engaged_mm=eng, air_mm=air,
        time_cut_feed_min=(eng + air) / pn.F_CUT + turn_time,
        time_link_feed_min=eng / pn.F_CUT + air / pn.F_LINK_DRESSUP + turn_time,
        cut_width_max=float(w_int.max()), cut_width_over=int((w_int > C_FRAC + QUANTUM).sum()),
        cut_width_n=int(len(w_int)), cut_width_p99=q(w_int, 99),
        cut_width_all_max=float(w_all.max()), cut_width_all_over=int((w_all > C_FRAC + QUANTUM).sum()),
        over_at=over_at,
        residue_mm2=res["mm2"], residue_unreachable_mm2=res["unreachable_mm2"],
        residue_big_mm2=res["big_mm2"], residue_big_at=res["big_at"], residue_pieces=res["pieces"],
        climb_right=B.climb[0], climb_left=B.climb[1],
        over_bound_list=[dict(A=list(map(float, j["A"])), B=list(map(float, j["B"])), length=j["turn_len"],
                              theta=j.get("theta_deg"), slanted=j.get("slanted_bound"), kind=j["kind"],
                              level=int(j["level"])) for j in over_b],
        plunge_list=[dict(A=list(map(float, j["A"])), B=list(map(float, j["B"])), cls=j["cls"], length=j["length"],
                          level=int(j["level"]), route=j.get("route")) for j in plunge],
    )
    # overlay: every cut path lies on its source curve
    dev = 0.0
    curves = {(c["level"], c["curve"]): c for c in cls["curves"]}
    for p in pl.pieces:
        if p["kind"] == "cut":
            flat[k + "|a"] = p["a"].astype(np.float64)
            src = B.pieces[p["src"]]
            dev = max(dev, on_curve(p["P"], curves[(src["level"], src["curve"])]["V"]))
    out["overlay_max_mm"] = dev
    return out


def engagement(B):
    """How thick the cut is. Over the engaged samples of the cut moves: the
    median swept width (fraction of D); per cut move the median of its
    engaged samples, then the median and the smallest over the moves; and
    the mean width: area removed by the cuts over their engaged length."""
    allw, per = [], []
    area = eng = 0.0
    for p in B.pl.pieces:
        if p["kind"] != "cut":
            continue
        e = p["eng"]
        area += float(p["a"].sum())
        ds = np.diff(p["s"])
        eng += float((ds * 0.5 * (e[1:].astype(float) + e[:-1])).sum())
        if e.any():
            w = p["w"][e] / D
            allw.append(w)
            per.append(float(np.median(w)))
    allw = np.concatenate(allw)
    return dict(eng_w_med=float(np.median(allw)), piece_w_med_med=float(np.median(per)),
                piece_w_med_min=float(min(per)), mean_width=area / eng / D, cut_area_mm2=area)


def rho_for_gap(g):
    """The C5 law read backwards: the smallest front radius rho (centre on
    the cleared side) whose allowed gap s'(rho) is at least g (bisection;
    s' grows with rho from 0 to c*D)."""
    from geom import spacing_bound
    if g >= CD:
        return math.inf
    lo, hi = 0.0, 1.0
    while spacing_bound(hi) < g:
        hi *= 2
    for _ in range(80):
        m = 0.5 * (lo + hi)
        lo, hi = (m, hi) if spacing_bound(m) < g else (lo, m)
    return hi


def by_place(sh, B):
    """Engaged samples of the cut pieces by place: 'head' (the leading run on
    the wall, within half a cell of it), 'departure' (within R of arc after
    the point where the piece leaves the wall), 'interior' (more than R of
    arc from both ends of the engaged run), 'end' (the rest). Per place: the
    samples, those over c (plus one cell), the largest reading. Also the
    extension a turn would need to cover the overs right after the
    departure: from the departure to the last over sample within R after it,
    and whether the turn plus that extension is within the C14 bound."""
    lim = C_FRAC + QUANTUM
    acc = {k: [0, 0, 0.0] for k in ("head", "departure", "interior", "end")}
    pieces_over = 0
    refusals = 0
    gaps = []
    pieces_over_place = {k: 0 for k in acc}
    heads = []
    ext = []
    where = []
    for p in B.pl.pieces:
        if p["kind"] != "cut" or B.pieces[p["src"]]["ring"]:
            continue
        P, s, w, e = p["P"], p["s"], p["w"] / D, p["eng"]
        on = sh.wall_dist(P[:, 0], P[:, 1]) <= TOL / 2
        k = 0
        while k < len(on) and on[k]:
            k += 1
        head = np.arange(len(P)) < k
        s_dep = s[k - 1] if k > 0 else 0.0
        dep = ~head & (s - s_dep <= R) & (k > 0)
        inter = pn.run_interior(s, e) & p["interior"] & ~head & ~dep
        end = ~head & ~dep & ~inter
        if k > 1:
            heads.append(float(s[k - 1]))
        pieces_over += bool((e & (w > lim)).any())
        # refusals: runs of engaged samples over c; a run that begins at the
        # first sample of a piece entered by a plunge is the plunge's
        # departure (exempt in the rs_cam replay) and is not counted
        ov = (e & (w > lim)).astype(int)
        st_ = np.nonzero(np.diff(np.concatenate([[0], ov])) == 1)[0]
        refusals += int(len(st_) - (1 if p.get("after_plunge") and len(st_) and st_[0] == 0 else 0))
        for nm_, m in (("head", head), ("departure", dep), ("interior", inter), ("end", end)):
            m = m & e
            pieces_over_place[nm_] += bool((m & (w > lim)).any())
            acc[nm_][0] += int(m.sum())
            acc[nm_][1] += int((m & (w > lim)).sum())
            if m.any():
                acc[nm_][2] = max(acc[nm_][2], float(w[m].max()))
        j = p.get("join", {})
        o = np.nonzero(dep & e & (w > lim))[0]
        # with the head as the turn the piece starts at its departure (k = 1)
        if len(o) and j.get("kind") == "turn":
            x = float(s[o[-1]] - s_dep)
            ext.append(dict(ext=x, turn=float(j["turn_len"]), total=float(j["turn_len"] + x),
                            fits=bool(j["turn_len"] + x <= j["bound"])))
        for t in np.nonzero((dep | inter) & e & (w > lim))[0]:
            where.append([float(P[t, 0]), float(P[t, 1]), float(w[t])])
        lv = B.pieces[p["src"]]["level"]
        od = np.nonzero(dep & e & (w > lim))[0]
        if len(od) and lv > 0:
            prev = np.vstack([c["V"] for (l_, _), c in B.curves.items() if l_ == lv - 1])
            g = cKDTree(prev).query(P[od])[0]
            gaps += list(g)
    return dict(acc={k: dict(n=v[0], over=v[1], max=v[2], pieces=pieces_over_place[k]) for k, v in acc.items()},
                pieces_over=pieces_over, refusals=refusals, heads=len(heads),
                dep_gap_med=float(np.median(gaps)) if gaps else 0.0, dep_gap_max=max(gaps, default=0.0),
                dep_rho_med=rho_for_gap(float(np.median(gaps))) if gaps else 0.0,
                dep_rho_max=rho_for_gap(max(gaps)) if gaps else 0.0, head_mm_max=max(heads, default=0.0),
                head_mm_med=float(np.median(heads)) if heads else 0.0,
                ext_n=len(ext), ext_max=max((x["ext"] for x in ext), default=0.0),
                ext_med=float(np.median([x["ext"] for x in ext])) if ext else 0.0,
                ext_total_max=max((x["total"] for x in ext), default=0.0),
                ext_fit=int(sum(x["fits"] for x in ext)), over_xy=where)


def split_facts(cls, lobes):
    """The level where a curve first has more than one cut piece, and the
    lobes: id, parent, first and last level, pieces."""
    P = cls["pieces"]
    lv = {}
    for p in P:
        lv.setdefault((p["level"], p["curve"]), []).append(p)
    multi = sorted(k[0] for k, v in lv.items() if len(v) > 1)
    # a split of the stock: a lobe with two or more children
    splits = sorted(L["nodes"][-1][0] + 1 for L in lobes if len(L["children"]) >= 2)
    first_closed_multi = None
    for c in cls["curves"]:
        if c["closed"] and len(lv.get((c["level"], c["curve"]), [])) > 1:
            first_closed_multi = c["level"]
            break
    return dict(
        first_multi_level=multi[0] if multi else None,
        first_closed_multi_level=first_closed_multi,
        split_levels=splits,
        lobes=[dict(id=L["id"], parent=L["parent"], first=int(L["nodes"][0][0]), last=int(L["nodes"][-1][0]),
                    children=L["children"], pieces=int(sum(p["lobe"] == L["id"] for p in P)),
                    multi_levels=int(sum(1 for k, v in lv.items() if sum(p["lobe"] == L["id"] for p in v) > 1)))
               for L in lobes],
        pieces_mm=float(sum(p["length"] for p in P)),
        share_min=float(min(p["share"] for p in P)),
        dropped=[dict(level=d["level"], at=list(map(float, d["at"])), samples=d["samples"], cells=d["cells"])
                 for d in cls["dropped"]],
        curve_mm=float(sum(c["sv"][-1] for c in cls["curves"])),
        merged=cls["merged"],
    )


def store(B, tag, flat):
    """Every move of the plan in order: its points, and per point the
    engaged flag and the swept width (fraction of D); a turn also its feed
    factor."""
    for i, p in enumerate(B.pl.pieces):
        k = f"{tag}|{i:05d}|{p['kind']}"
        # float64: the overlay check reads a cut to 1e-6 mm, and the replay
        # in assert_c19.py must meet the same raster cells
        flat[k + "|P"] = p["P"].astype(np.float64)
        if p["kind"] != "retract":
            flat[k + "|e"] = p["eng"]
            flat[k + "|w"] = (p["w"] / D).astype(np.float32)
        if p["kind"] == "cut":
            flat[k + "|a"] = p["a"].astype(np.float64)
            src = B.pieces[p["src"]]
            flat[k + "|src"] = np.array([src["level"], src["curve"], src["lobe"]])
        if p["kind"] == "turn":
            flat[k + "|f"] = np.array([p["feed_factor"]])


# plan variants: (link, head as turn)
MODES = {"oneway": ("oneway", False, False), "oneway_head": ("oneway", True, False),
         "oneway_head_slanted": ("oneway", True, True), "zigzag": ("zigzag", False, False)}


def main():
    out = {"inputs": dict(R=R, S=S, D=D, CD=CD, C_FRAC=C_FRAC, TOL=TOL, PASS_STEP=PASS_STEP, CELL=CELL,
                          TURN_BOUND=TURN_BOUND, QUANTUM=QUANTUM, F_CUT=pn.F_CUT, F_LINK=pn.F_LINK_DRESSUP)}
    flat = {}
    for nm in ("star", "pocket"):
        sh = st.Shape(nm)
        out[nm] = {}
        for src in ("march", "a", "e"):
            t0 = time.time()
            levels = cl.march_levels(nm) if src == "march" else cl.a_levels(nm, sh, src)
            cls = cl.classify(sh, levels)
            lobes, _ = cl.lobe_tree(cls["labs"], cls["pieces"])
            out[nm][src] = dict(split=split_facts(cls, lobes), classify_residue=st.residue(sh, cls["stock"] & sh.stock0))
            for i, p in enumerate(cls["pieces"]):
                flat[f"{nm}|{src}|piece|{i:04d}|{p['lobe']}|{p['level']}"] = p["P"].astype(np.float32)
            for mode in MODES:
                B = bd.Build(sh, cls, lobes, MODES[mode][0], head=MODES[mode][1], slanted=MODES[mode][2]).run()
                m = summarise(sh, cls, B, lobes)
                m["seconds"] = time.time() - t0
                out[nm][src][mode] = m
                store(B, f"{nm}|{src}|{mode}", flat)
                print(nm, src, mode, {k: (round(v, 3) if isinstance(v, float) else v) for k, v in m.items()
                                      if not isinstance(v, list)}, flush=True)
    with open(os.path.join(HERE, "results_c19.json"), "w") as f:
        json.dump(out, f, indent=1)
    np.savez_compressed(os.path.join(HERE, "paths_c19.npz"), **flat)


if __name__ == "__main__":
    main()
