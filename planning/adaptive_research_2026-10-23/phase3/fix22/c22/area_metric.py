"""C22 §18: the op-1 clearing length over the bound under the operator's area load (2026-10-27):
L = radial width x axial depth / (D x DPP), read from the simulation as woc x axial_engagement_mm / DPP
(an upper bound: the sim gives the widest width and the deepest depth, not their integral). Against
today's width count (woc over 0.471) on the same samples. Bounds: 0.471 (the width bound kept as an area
fraction) and 0.471 + tol / DPP (the depth read's own tolerance, tol 0.1). Slot feeds out (as load_split).
By axial band too. Writes area_metric.json."""
import collections, json, math
from pathlib import Path
H = Path(__file__).resolve().parent; P3 = H.parent.parent
RUNS = {"wanaka200": (5.46, {"D26": "fix16/wanaka200_pick", "D31": f"{H.parent.name}/cut_feed/wanaka200"}),
        "corne": (6.0, {"D26": "fix16/corne_pick", "D31": f"{H.parent.name}/cut_feed/corne"})}
TOL = 0.1
out = {}
for fx, (dpp, runs) in RUNS.items():
    for tag, run in runs.items():
        S = [s for s in json.load(open(P3 / run / "simulation.json"))["trace"]["samples"] if s["toolpath_id"] == 1]
        base = collections.Counter(round(s["feed_rate_mm_min"]) for s in S if s["is_cutting"] and s["source_intent"] == "clearing_cut").most_common(1)[0][0]
        c = collections.Counter(); pp = None
        for s in S:
            p = s["position"]; ln = math.hypot(p[0] - pp[0], p[1] - pp[1]) if pp else 0.0; pp = p
            if not (s["is_cutting"] and s["source_intent"] == "clearing_cut") or s["feed_rate_mm_min"] < base - 0.5:
                continue
            w = s["engagement"]["radial_woc_fraction"]; a = s["axial_engagement_mm"]; L = w * a / dpp
            band = "ap <= 2" if a <= 2 else "ap 2-3" if a <= 3 else "ap > 3"
            if w > 0.471: c["width over 0.471"] += ln; c[f"width over, {band}"] += ln
            if L > 0.471: c["area over 0.471"] += ln; c[f"area over, {band}"] += ln
            if L > 0.471 + TOL / dpp: c[f"area over {0.471 + TOL / dpp:.3f}"] += ln
        out[f"{fx} {tag}"] = {k: round(v, 1) for k, v in sorted(c.items())}
        print(fx, tag, out[f"{fx} {tag}"])
json.dump(out, open(H / "area_metric.json", "w"), indent=1)
