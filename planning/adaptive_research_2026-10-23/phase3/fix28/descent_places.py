"""D38: where the op-1 descents of each cause stand on Corne: in a switch hole
(fix26/hole_leftover.py's hole components grown by 1 mm), outside the wall
(fix25/p1a_p5a_d34.py's wall distance > 0), or inside the case elsewhere.
Reuses descent_causes.py's matching. Usage: descent_places.py <run> <tag>"""
import collections, json, sys
from pathlib import Path
import numpy as np
from scipy import ndimage
H = Path(__file__).resolve().parent; P3 = H.parent
run, tag = sys.argv[1], sys.argv[2]
src = open(H / "descent_causes.py").read().split("agg = collections")[0].replace("run, tag = sys.argv[1], sys.argv[2]", f"run, tag = {run!r}, {tag!r}")
exec(src)
part = np.load(P3 / "fix9" / "corne_part.npz"); top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
fin = np.isfinite(top) & (top > 1e-6); fill = ndimage.binary_fill_holes(fin); holes = fill & ~fin
lab, n = ndimage.label(holes); area = ndimage.sum(holes, lab, range(1, n + 1)) * g * g
keep = np.isin(lab, [i + 1 for i in range(n) if area[i] >= 20.0])
dist = ndimage.distance_transform_edt(~keep) * g; inhole = dist <= 1.0
def place(x, y):
    c = int(round((x - x0) / g)); r = int(round((y - y0) / g))
    if not (0 <= r < top.shape[0] and 0 <= c < top.shape[1]) or not fill[r, c]:
        return "outside wall"
    return "switch hole" if inhole[r, c] else "inside case"
agg = collections.defaultdict(lambda: [0, 0.0, 0.0])
for k, ss in mv.items():
    i = cause_of[k]; cause = "link plunge (no C40)" if i is None else rec[i]["cause"]
    p = ends[k]; a = agg[(cause, place(p[0], p[1]))]
    a[1] += sum(s["segment_time_s"] for s in ss); a[2] += sum(s["segment_time_s"] for s in ss if s["removed_volume_est_mm3"] <= Q)
cnt = collections.Counter((r["cause"], place(r["x"], r["y"])) for r in rec)
out = {f"{c} | {pl}": {"c40_records": cnt.get((c, pl), 0), "time_s": round(v[1], 1), "air_share": round(v[2] / v[1], 3) if v[1] else None} for (c, pl), v in sorted(agg.items(), key=lambda kv: -kv[1][1])}
for k, v in out.items(): print(f"{k:45} {v}")
json.dump(out, open(H / f"descent_places_{tag}.json", "w"), indent=1)
