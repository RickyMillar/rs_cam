"""C22 §16.14: what wanaka's D31 over-load rise against D26 is made of (all op-1 clearing samples
over 0.471, slot feeds out, as fix21/load_split.py), from the two simulation artefacts:
1. by move kind (fix21/c22/nearA_kind.py's step rule; descents and climbs apart);
2. by place against the D26 plunge feet (an entry_plunge move's end, within 2R and DPP/2 in Z),
   and against the run's own plunge feet;
3. the split / cycle / over trade over the builds, from P15_D_verify.md (D29 v4, D30, D31, D26).
Writes d31_rise.json."""
import collections, json, math
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent; P3 = H.parent.parent
R, DPP = 3.0, 5.46; THR = max(1.1 * DPP, 1.0)

def load(run):
    return [s for s in json.load(open(P3 / run / "simulation.json"))["trace"]["samples"] if s["toolpath_id"] == 1]

def analyse(S):
    last = {}
    for s in S: last[s["move_index"]] = s
    ms = sorted(last); prev = {b: a for a, b in zip(ms, ms[1:])}
    def step(m):
        a = prev.get(m)
        if a is None or last[m]["source_intent"] != "clearing_cut": return None
        pa, pb = last[a]["position"], last[m]["position"]
        dz = pb[2] - pa[2]; xy = math.hypot(pb[0] - pa[0], pb[1] - pa[1])
        return dz if (abs(dz) > THR or (xy > 1e-6 and abs(dz) / xy > 0.3) or (xy <= 1e-6 and abs(dz) > 1e-6)) else None
    feet = []
    for m in ms:
        a = prev.get(m)
        if a is None: continue
        pa, pb = last[a]["position"], last[m]["position"]
        if last[m]["source_intent"] == "entry_plunge" and math.hypot(pb[0] - pa[0], pb[1] - pa[1]) < 1e-6 and pb[2] < pa[2] - 1e-6:
            feet.append(pb)
    base = collections.Counter(round(s["feed_rate_mm_min"]) for s in S if s["is_cutting"] and s["source_intent"] == "clearing_cut").most_common(1)[0][0]
    over, pp = [], None
    for s in S:
        p = s["position"]; ln = math.hypot(p[0] - pp[0], p[1] - pp[1]) if pp else 0.0; pp = p
        if not (s["is_cutting"] and s["source_intent"] == "clearing_cut" and s["engagement"]["radial_woc_fraction"] > 0.471): continue
        if s["feed_rate_mm_min"] < base - 0.5: continue
        m = s["move_index"]; st = step(m)
        if st is not None: k = "on a climb" if st > 0 else "on a descent"
        elif prev.get(m) is not None and step(prev[m]) is not None: k = "after a climb" if step(prev[m]) > 0 else "after a descent"
        else: k = "level"
        over.append((p, ln, k))
    return np.array(feet), over

def near(A, p):
    return bool(len(A)) and bool(np.any((np.hypot(A[:, 0] - p[0], A[:, 1] - p[1]) <= 2 * R) & (np.abs(A[:, 2] - p[2]) <= DPP / 2)))

F26, O26 = analyse(load("fix16/wanaka200_pick"))
F31, O31 = analyse(load(f"{H.parent.name}/cut_feed/wanaka200"))
out = {"plunge feet": {"D26": len(F26), "D31": len(F31)}}
for tag, O, own in (("D26", O26, F26), ("D31", O31, F31)):
    kind = collections.Counter(); place = collections.Counter()
    for p, ln, k in O:
        kind[k] += ln
        place[("at a D26 plunge foot" if near(F26, p) else "not at a D26 foot") + (", own foot" if near(own, p) else ", no own foot")] += ln
    out[tag] = {"over_mm": round(sum(kind.values()), 1), "by kind": {a: round(b, 1) for a, b in sorted(kind.items())},
                "by place": {a: round(b, 1) for a, b in sorted(place.items())}}
# P15_D_verify.md: step split made / avoided, cycle (s), over 0.471 total (mm), wanaka and Corne.
# D26 ran the path rule: every flagged step split (made = made + avoided of the D31 rule).
TRADE = {"wanaka200": {"D29v4": (496, 4201, 1991, 2121.0 + 1223.9 + 20.1), "D30": (2493, 2274, 2693, 2858.2),
                       "D31": (2505, 2262, 2694, 2856.0), "D26": (2505 + 2262, 0, 3513, 2225.3)},
         "corne": {"D29v4": (0, 2527, 2458, 185.4 + 794.7 + 297.5), "D30": (1439, 1088, 2996, 1069.6),
                   "D31": (1630, 897, 3011, 1065.9), "D26": (1630 + 897, 0, 3247, 780.8)}}
out["trade"] = {}
for fx, t in TRADE.items():
    rows = {}
    for a, b in (("D29v4", "D31"), ("D30", "D31"), ("D31", "D26")):
        ds = t[b][0] - t[a][0]; dc = t[b][2] - t[a][2]; do = t[b][3] - t[a][3]
        rows[f"{a}->{b}"] = {"splits": ds, "cycle_s": dc, "over_mm": round(do, 1),
                             "s_per_split": round(dc / ds, 3) if ds else None, "over_mm_per_split": round(do / ds, 3) if ds else None}
    out["trade"][fx] = rows
print(json.dumps(out, indent=1))
json.dump(out, open(H / "d31_rise.json", "w"), indent=1)
