# C19: assert every number that farside_ideas_v5.html and
# P2_C19_march_arms.md quote (quotes_c19.json lists them).
# Run: python3 -I assert_c19.py            (checks the stored results)
#      python3 -I assert_c19.py --rerun    (runs run_c19.py and page_c19.py first)
#
# Each quoted value is recomputed here, not read back from results_c19.json,
# where the stored paths allow it:
#   - plan numbers from the moves in paths_c19.npz (lengths, engaged flags,
#     swept widths, turn feed factors, the order of moves);
#   - the overlay from C17's loops and C15's curves directly;
#   - coverage left by a fresh replay of the two card plans per shape;
#   - the split facts by a fresh run of the classification;
#   - C17 numbers from sweep.json, D12 numbers from d12_harness.txt;
#   - c, the turn bound and the law radius from the code formulas.
# The plunge kinds and the departure gap, extension and climb counts are
# pipeline records: they are checked against the plan where a plan
# relation exists (the kinds sum to the plunges).
import html as htmlmod
import json
import math
import os
import re
import subprocess
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
PH2 = os.path.dirname(HERE)
ROOT = os.path.dirname(PH2)
sys.path.insert(0, HERE)
FAILS = []
N_IND = [0]


def check(ok, what):
    if not ok:
        FAILS.append(what)
        print("FAIL", what)


if "--rerun" in sys.argv:
    subprocess.run([sys.executable, "-I", os.path.join(HERE, "run_c19.py")], check=True)
    subprocess.run([sys.executable, "-I", os.path.join(HERE, "page_c19.py")], check=True)

import clip as cl  # noqa: E402
import stream as st  # noqa: E402

# ── 1. constants from the code formulas ───────────────────────────────
R, s, tol, cell = 3.0, 2.0, 0.1, 0.5
D = 2 * R
f_arc = min(max(math.acos(1 - s / R) / (2 * math.pi) * 1.05, 0.0), 0.5)
c = (1 - math.cos(2 * math.pi * f_arc)) / 2
lim = c + tol / D
bound = s + (1 + math.sqrt(2)) * cell
check(abs(c * D - 2.175868) < 5e-7 and abs(cl.CD - c * D) < 1e-12, "c*D")
check(abs(bound - 3.207) < 5e-4 and abs(cl.TURN_BOUND - bound) < 1e-12, "turn bound")
F_CUT, F_LINK = 1500.0, 500.0

res = json.load(open(os.path.join(HERE, "results_c19.json")))
sw = json.load(open(os.path.join(PH2, "c17", "sweep.json")))
QU = json.load(open(os.path.join(HERE, "quotes_c19.json")))
page = htmlmod.unescape(open(os.path.join(ROOT, "farside_ideas_v5.html")).read())
note = open(os.path.join(ROOT, "P2_C19_march_arms.md")).read()

# ── 2. the plans from the stored moves ────────────────────────────────
npz = np.load(os.path.join(HERE, "paths_c19.npz"))
PLANS = {}
for k in npz.files:
    if k.split("|")[2] == "piece":  # the clip's pieces (drawing input), not moves
        continue
    nm, src, mode, i, kind, part = k.split("|")
    PLANS.setdefault((nm, src, mode), {}).setdefault(int(i), dict(kind=kind))[part] = npz[k]
PLANS = {k: [v[i] for i in sorted(v)] for k, v in PLANS.items()}
SH = {nm: st.Shape(nm) for nm in ("star", "pocket")}


def arc(P):
    return np.concatenate([[0], np.cumsum(np.hypot(*np.diff(P.astype(float), axis=0).T))])


def run_interior(sv, e):
    out = np.zeros(len(sv), bool)
    k = 0
    n = len(e)
    while k < n:
        if e[k]:
            j = k
            while j + 1 < n and e[j + 1]:
                j += 1
            idx = np.arange(k, j + 1)
            out[idx] = (sv[idx] - sv[k] > R) & (sv[j] - sv[idx] > R)
            k = j + 1
        else:
            k += 1
    return out


def plan_numbers(nm, src, mode):
    mv = PLANS[(nm, src, mode)]
    sh = SH[nm]
    o = dict(plunges=0, turns=0, turn_over=0, keepdown_mm=0.0, engaged_mm=0.0, air_mm=0.0, turn_time=0.0)
    fac = []
    refusals = 0
    allw, per = [], []
    area = engc = 0.0
    acc = {k: [0, 0, 0.0] for k in ("head", "departure", "interior", "end")}
    for n_, m in enumerate(mv):
        kind = m["kind"]
        if kind == "retract":
            o["plunges"] += 1
            continue
        sv = arc(m["P"])
        L = float(sv[-1])
        if kind == "turn":
            o["turns"] += 1
            f = float(m["f"][0])
            fac.append(f)
            o["turn_over"] += f < 1
            o["turn_time"] += L / (F_CUT * f)
            continue
        e = m["e"]
        seg = 0.5 * (e[1:].astype(float) + e[:-1])
        le = float((np.diff(sv) * seg).sum())
        o["engaged_mm"] += le
        o["air_mm"] += L - le
        if kind in ("link", "return"):
            o["keepdown_mm"] += L
        if kind == "cut":
            w = m["w"].astype(float)
            P = m["P"].astype(float)
            a_ = float(m["a"].sum())
            area += a_
            engc += le
            if e.any():
                allw.append(w[e])
            if le > 0:
                per.append(a_ / le / D)
            # rings (the core loops) are closed cut moves: the counts leave them out
            if np.hypot(*(P[0] - P[-1])) <= 1e-9:
                continue
            ov = (e & (w > lim)).astype(int)
            starts = np.nonzero(np.diff(np.concatenate([[0], ov])) == 1)[0]
            after = n_ > 0 and mv[n_ - 1]["kind"] == "retract"
            refusals += len(starts) - (1 if after and len(starts) and starts[0] == 0 else 0)
            on = sh.wall_dist(P[:, 0], P[:, 1]) <= tol / 2
            k = 0
            while k < len(on) and on[k]:
                k += 1
            head = np.arange(len(P)) < k
            s_dep = sv[k - 1] if k > 0 else 0.0
            dep = ~head & (sv - s_dep <= R) & (k > 0)
            interior = (sv > R) & (sv < L - R)
            inter = run_interior(sv, e) & interior & ~head & ~dep
            end = ~head & ~dep & ~inter
            for nm_, mk in (("head", head), ("departure", dep), ("interior", inter), ("end", end)):
                mk = mk & e
                acc[nm_][0] += int(mk.sum())
                acc[nm_][1] += int((mk & (w > lim)).sum())
                if mk.any():
                    acc[nm_][2] = max(acc[nm_][2], float(w[mk].max()))
    o["refusals"] = refusals
    o["entries"] = 1 + o["plunges"] + refusals
    o["time_cut_feed_min"] = (o["engaged_mm"] + o["air_mm"]) / F_CUT + o["turn_time"]
    o["time_link_feed_min"] = o["engaged_mm"] / F_CUT + o["air_mm"] / F_LINK + o["turn_time"]
    o["turn_factor_med"] = float(np.percentile(fac, 50)) if fac else 0.0
    o["place"] = acc
    o["eng_w_med"] = float(np.median(np.concatenate(allw)))
    o["piece_w_med"] = float(np.median(per))
    o["piece_w_min"] = float(min(per))
    o["mean_width"] = area / engc / D
    return o


def seg_dist(P, V):
    """Largest distance of the points P from the polyline V (brute force)."""
    A, B = V[:-1], V[1:]
    d = B - A
    L2 = np.maximum((d * d).sum(1), 1e-300)
    best = 0.0
    for k in range(0, len(P), 200):
        Q = P[k:k + 200]
        t = np.clip(((Q[:, None, :] - A[None]) * d[None]).sum(2) / L2[None], 0, 1)
        X = A[None] + t[..., None] * d[None]
        best = max(best, float(np.hypot(*(Q[:, None, :] - X).transpose(2, 0, 1)).min(1).max()))
    return best


def overlay(nm, src, mode):
    if src == "march":
        d = np.load(os.path.join(PH2, "c17", "curves_c17.npz"))
        V = {}
        for k in d.files:
            n_, run, i, part = k.split("|")
            if n_ == nm and run == "h_step" and part == "P":
                P = d[k].astype(float)
                V[(int(i), 0)] = np.vstack([P, P[:1]])
    else:
        d = np.load(os.path.join(PH2, "c15", "curves_whole.npz"))
        V = {}
        for k in d.files:
            n_, key, i, j = k.split("|")
            if n_ == nm and key == src:
                V.setdefault(int(i), []).append(d[k].astype(float))
        V = {(i, j): P for i, ps in V.items() for j, P in enumerate(ps)}
    dev = 0.0
    for m in PLANS[(nm, src, mode)]:
        if m["kind"] == "cut":
            lv = int(m["src"][0])
            cands = [P for (i, _), P in V.items() if i == lv]
            dev = max(dev, min(seg_dist(m["P"].astype(float), P) for P in cands))
    return dev


def replay_residue(nm, src, mode):
    sh = SH[nm]
    rp = st.Replay(stock0=sh.stock0, mx=sh.mx, my=sh.my, seed=sh.seed)
    for m in PLANS[(nm, src, mode)]:
        if m["kind"] != "retract":
            P = m["P"].astype(float)
            if len(P) >= 2 and arc(P)[-1] > 0:
                rp.cut(P)  # the stored move is the replay's own sampling
    return st.residue(sh, rp.stock & sh.stock0)


CALC = {}
for key in PLANS:
    CALC[key] = plan_numbers(*key)
    x = res[key[0]][key[1]][key[2]]
    tot = x["plunge_seam"] + x["plunge_arc"] + x["plunge_lobe"] + x["plunge_jump"]
    check(tot == CALC[key]["plunges"] == x["plunges"], f"{key} plunge kinds sum {tot} vs {CALC[key]['plunges']}")
CARD = "oneway_head_slanted"
for nm in ("star", "pocket"):
    for src in ("march", "a", "e"):
        CALC[(nm, src, CARD)]["overlay_max_mm"] = overlay(nm, src, CARD)
        check(CALC[(nm, src, CARD)]["overlay_max_mm"] <= 1e-6, f"{nm} {src} overlay")
        r = replay_residue(nm, src, CARD)
        CALC[(nm, src, CARD)]["residue_mm2"] = r["mm2"]
        CALC[(nm, src, CARD)]["residue_unreachable_mm2"] = r["unreachable_mm2"]

# ── 3. the split, by a fresh classification ───────────────────────────
SPLIT = {}
for nm, src in [(n, s_) for n in ("star", "pocket") for s_ in ("march", "e")]:
    cls = cl.classify(SH[nm], cl.march_levels(nm) if src == "march" else cl.a_levels(nm, SH[nm], src))
    lobes, _ = cl.lobe_tree(cls["labs"], cls["pieces"])
    ps = cls["pieces"]
    by = {}
    for p in ps:
        by.setdefault((p["level"], p["curve"]), []).append(p)
    pm = float(sum(p["length"] for p in ps))
    cm = float(sum(c_["sv"][-1] for c_ in cls["curves"]))
    SPLIT[(nm, src)] = dict(first_multi_level=min(k[0] for k, v in by.items() if len(v) > 1),
                     split_levels=", ".join(str(v) for v in sorted(L["nodes"][-1][0] + 1 for L in lobes if len(L["children"]) >= 2)),
                     lobes_n=len(lobes), pieces_n=len(ps), share_min=min(p["share"] for p in ps),
                     merged_n=len(cls["merged"]), dropped_n=len(cls["dropped"]), pieces_mm=pm, curve_mm=cm,
                     clip_share=pm / cm, lobe0_multi=sum(1 for k, v in by.items()
                                                         if sum(p["lobe"] == 0 for p in v) > 1))

# ── 4. D12 (Rust), independent parse ──────────────────────────────────
txt = open(os.path.join(PH2, "d12_harness.txt")).read()
D12 = {}
for nm in ("star", "pocket"):
    blk = txt.split(f"\n{nm} / geodesic_local_oneway\n", 1)[1].split("\nstar / ", 1)[0].split("\npocket / ", 1)[0]
    g = lambda pat: re.search(pat, blk).group(1)  # noqa: E731
    m = dict(cut_mm=float(g(r"cut length: ([\d.]+)")), keepdown_mm=float(g(r"keep-down links: \d+ \(([\d.]+)")),
             entries=int(g(r"entries \(plunges\): (\d+)")), join_entries=int(g(r"\(join\) (\d+)")),
             mop_entries=int(g(r"residue mop (\d+)")), turns=int(g(r"clearing turns (\d+),")),
             over_bound=int(g(r"over the bound 3.207 mm (\d+)")),
             factor_med=float(g(r"turn feed factor: min [\d.]+, quartiles [\d.]+ / ([\d.]+)")),
             no_turn_max=float(g(r"steps on no turn \(emitted\): max ([\d.]+)")))
    m["time_cut_feed_min"] = (m["cut_mm"] + m["keepdown_mm"]) / F_CUT
    m["time_link_feed_min"] = m["cut_mm"] / F_CUT + m["keepdown_mm"] / F_LINK
    check(m["entries"] == 1 + m["join_entries"] + m["mop_entries"], f"D12 {nm} entries add up")
    D12[nm] = m


# ── 5. the law radius ─────────────────────────────────────────────────
def law_gap(rho):
    return rho + R - math.sqrt(R * R + rho * rho + 2 * rho * (R - c * D))


def rho_for(g):
    lo, hi = 0.0, 64.0
    for _ in range(100):
        m = 0.5 * (lo + hi)
        lo, hi = (m, hi) if law_gap(m) < g else (lo, m)
    return hi


# ── 6. every quote ────────────────────────────────────────────────────
def fmt(v, f):
    if f == "s":
        return str(v)
    if f == "%":
        return f"{100 * v:.0f}%"
    return format(v, f)


def value(key):
    """Recompute a quoted value; returns (value, independent?)."""
    parts = key.split("|")
    if parts[0] == "const":
        return {"cD": c * D, "c": c, "turn_bound": bound, "c_plus_cell": lim, "F_cut": F_CUT, "F_link": F_LINK}[parts[1]], True
    nm = parts[0]
    if parts[1] == "c17":
        h = sw[nm]["h_step"]
        if parts[2] == "air_share":
            return h["air_mm"] / (h["air_mm"] + h["engaged_mm"]), True
        return h[parts[2]], True
    if parts[1] == "d12":
        return D12[nm][parts[2]], True
    if parts[1] == "c17e":
        return sw[nm]["c15_e"][parts[2]], True
    if parts[1] == "ratio":
        a = CALC[(nm, "a", CARD)]
        if parts[2] in ("eng_vs_a", "e_eng_vs_a"):
            x = CALC[(nm, "e" if parts[2].startswith("e_") else "march", CARD)]
            return x["engaged_mm"] / a["engaged_mm"], True
        x = CALC[(nm, parts[2][3:], CARD)]  # be_<src>: break-even seconds per entry
        return 60 * (x["time_cut_feed_min"] - a["time_cut_feed_min"]) / (a["entries"] - x["entries"]), True
    src = parts[1]
    if parts[2] == "split":
        return SPLIT[(nm, src)][parts[3]], True
    mode, metric = parts[2], parts[3]
    x = res[nm][src][mode]
    cc = CALC.get((nm, src, mode), {})
    if metric.startswith("place."):
        _, place, k = metric.split(".")
        if k == "pieces":
            return x["by_place"]["acc"][place]["pieces"], False
        i = {"n": 0, "over": 1, "max": 2}[k]
        return cc["place"][place][i], True
    if metric.startswith("bp."):
        k = metric[3:]
        b = x["by_place"]
        if k in ("dep_rho_med", "dep_rho_max"):
            return rho_for(b["dep_gap_med" if k.endswith("med") else "dep_gap_max"]), True
        return b[k], False
    if metric.startswith("theta_"):
        th = [t["theta"] for t in x["over_bound_list"]]
        return {"theta_med": float(np.median(th)), "theta_min": min(th), "theta_max": max(th)}[metric], False
    if metric in cc:
        return cc[metric], True
    return x[metric], False


for key, qv in QU.items():
    v, ind = value(key)
    N_IND[0] += ind
    want = fmt(v, qv["fmt"])
    if qv["fmt"] == ".1e":  # the overlay: a bound, not a match
        check(float(qv["text"]) <= 1e-6 and v <= 1e-6, f"{key} overlay {qv['text']} / {v}")
    else:
        check(want == qv["text"], f"{key}: quoted {qv['text']} recomputed {want}")
    # a stored value must agree with its recomputation too
    if not isinstance(v, str) and not isinstance(qv["value"], str) and qv["fmt"] != ".1e":
        check(fmt(qv["value"], qv["fmt"]) == want, f"{key}: stored {qv['value']} recomputed {v}")
    check(qv["text"] in page, f"{key}: '{qv['text']}' not on the page")
    check(qv["text"] in note, f"{key}: '{qv['text']}' not in the note")

# the split facts in results_c19.json match the fresh run
for nm in ("star", "pocket"):
    for src in ("march", "e"):
        sp = res[nm][src]["split"]
        check(sp["first_multi_level"] == SPLIT[(nm, src)]["first_multi_level"], f"{nm} {src} first split level")
        check(len(sp["lobes"]) == SPLIT[(nm, src)]["lobes_n"], f"{nm} {src} lobes")

print(f"{len(QU)} quotes, {N_IND[0]} recomputed independently, {len(QU) - N_IND[0]} pipeline records checked by value")
if FAILS:
    print(f"{len(FAILS)} FAILED")
    sys.exit(1)
print("ALL OK")
