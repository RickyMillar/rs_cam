"""C22 §17: wanaka's over-load and the stock an earlier draped pass left (all op-1 clearing samples at the
base feed, slot feeds out). Every wanaka over sample lies on the terrain level (Z 8.62, draped, below the
14.08 level), so the level of a sample is its Z band. For each terrain-level sample: the most that an EARLIER
terrain-level clearing sample within 2R stood above it (its tip over this tip). Classes: none above (<= the
job tolerance 0.1 mm), above by (0.1, 1], above by more than 1 mm. Over and under the bound apart, so the
association is read against the base rate. Also the over length by axial engagement.
Writes drape_left.json."""
import collections, json, math
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent; P3 = H.parent.parent
R, TOL, Z14 = 3.0, 0.1, 14.08

def run(path):
    S = [s for s in json.load(open(P3 / path / "simulation.json"))["trace"]["samples"] if s["toolpath_id"] == 1]
    pp = None
    for s in S:
        p = s["position"]; s["_ln"] = math.hypot(p[0] - pp[0], p[1] - pp[1]) if pp else 0.0; pp = p
    base = collections.Counter(round(s["feed_rate_mm_min"]) for s in S if s["is_cutting"] and s["source_intent"] == "clearing_cut").most_common(1)[0][0]
    over_all = collections.Counter(); axial = collections.Counter()
    for s in S:
        if s["is_cutting"] and s["source_intent"] == "clearing_cut" and s["feed_rate_mm_min"] >= base - 0.5 and s["engagement"]["radial_woc_fraction"] > 0.471:
            over_all["terrain level" if s["position"][2] < Z14 - 1e-3 else "upper levels"] += s["_ln"]
            a = s["axial_engagement_mm"]
            axial["<= 2 mm" if a <= 2 else "2-3 mm" if a <= 3 else "> 3 mm"] += s["_ln"]
    blk = [s for s in S if s["is_cutting"] and s["source_intent"] == "clearing_cut" and s["position"][2] < Z14 - 1e-3 and s["feed_rate_mm_min"] >= base - 0.5]
    P = np.array([s["position"] for s in blk])
    grid = collections.defaultdict(list)
    for i, p in enumerate(P): grid[(int(p[0] // (2 * R)), int(p[1] // (2 * R)))].append(i)
    res = collections.Counter()
    for i, s in enumerate(blk):
        p = P[i]; gx, gy = int(p[0] // (2 * R)), int(p[1] // (2 * R)); best = 0.0
        for dx in (-1, 0, 1):
            for dy in (-1, 0, 1):
                for j in grid.get((gx + dx, gy + dy), ()):
                    if j < i and (P[j][0] - p[0]) ** 2 + (P[j][1] - p[1]) ** 2 < (2 * R) ** 2:
                        best = max(best, P[j][2] - p[2])
        k = "none above" if best <= TOL else "above by (0.1, 1]" if best <= 1 else "above by > 1"
        res[("over" if s["engagement"]["radial_woc_fraction"] > 0.471 else "under", k)] += s["_ln"]
    out = {"over by level": {k: round(v, 1) for k, v in over_all.items()}, "over by axial": {k: round(v, 1) for k, v in axial.items()}}
    for side in ("over", "under"):
        out[side] = {k: round(v, 1) for (sd, k), v in sorted(res.items()) if sd == side}
    above = lambda d: d.get("above by (0.1, 1]", 0) + d.get("above by > 1", 0)
    out["over rate with an earlier pass above"] = round(above(out["over"]) / (above(out["over"]) + above(out["under"])), 3)
    out["over rate with none above"] = round(out["over"]["none above"] / (out["over"]["none above"] + out["under"]["none above"]), 3)
    return out

OUT = {"D26": run("fix16/wanaka200_pick"), "D31": run(f"{H.parent.name}/cut_feed/wanaka200")}
print(json.dumps(OUT, indent=1))
json.dump(OUT, open(H / "drape_left.json", "w"), indent=1)
