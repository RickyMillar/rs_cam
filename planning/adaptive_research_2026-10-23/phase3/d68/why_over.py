"""D68: where the stock that a > 1 DPP sample of the Corne edge op meets stands. The stock before op 5 is rebuilt as
corne_op5.py does (flat 6 mm disc lowered to each op-1 sample tip, 0.25 mm grid). For each sample of op 5 that removes stock
with axial > DPP + 0.05: the centre's distance outside the silhouette, and the highest cell in its disc and that cell's
distance outside the silhouette. Usage: why_over.py <run under phase3>"""
import json, math, sys, collections
from pathlib import Path
import numpy as np
from scipy import ndimage
H = Path(__file__).resolve().parent; P3 = H.parent
sim = json.load(open(P3 / sys.argv[1] / "simulation.json")); Q = sim["resolution_mm"] ** 2 * sim["sample_step_mm"]; T = sim["trace"]["samples"]
lo, hi = sim["stock_bbox_min"], sim["stock_bbox_max"]; g = 0.25; R = 3.0
nx = int(round((hi[0] - lo[0]) / g)); ny = int(round((hi[1] - lo[1]) / g))
xs = lo[0] + (np.arange(nx) + 0.5) * g; ys = lo[1] + (np.arange(ny) + 0.5) * g
exec(open(H / "corne_op5.py").read().split("def lower")[1].split("P1 = ")[0].join(["def lower", ""]))
P1 = np.array([s["position"] for s in T if s["toolpath_id"] == 1]); before = np.full((ny, nx), float(hi[2])); lower(before, P1)
part = np.load(P3 / "fix9" / "corne_part.npz"); ptop, pg, px0, py0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
fin = np.isfinite(ptop) & (ptop > 1e-6); fill = ndimage.binary_fill_holes(fin)
pad = 80; F = np.pad(fill, pad); dout = ndimage.distance_transform_edt(~F) * pg
def dist(x, y):
    c = int(round((x - px0) / pg)) + pad; r = int(round((y - py0) / pg)) + pad
    return float(dout[r, c]) if 0 <= r < dout.shape[0] and 0 <= c < dout.shape[1] else 99.0
over = [s for s in T if s["toolpath_id"] == 5 and s["removed_volume_est_mm3"] > Q and (s.get("axial_engagement_mm") or 0) > 6.05]
k = int(math.ceil(R / g)); acc = collections.Counter(); acc2 = collections.Counter(); tot = 0.0
for s in over:
    x, y, z = s["position"]; i = int((x - lo[0]) / g); j = int((y - lo[1]) / g)
    best = (-1, None)
    for dj in range(-k, k + 1):
        for di in range(-k, k + 1):
            I, J = i + di, j + dj
            if 0 <= I < nx and 0 <= J < ny and (di * g) ** 2 + (dj * g) ** 2 <= R * R and before[J, I] > best[0]: best = (before[J, I], (xs[I], ys[J]))
    v = s["removed_volume_est_mm3"]; tot += v
    acc[(round(dist(x, y)), round(best[0]), round(z), s["source_intent"] if isinstance(s["source_intent"], str) else "?")] += v; acc2[round(dist(*best[1]))] += v
print("over-DPP volume", round(tot), "by (centre distance outside silhouette mm, highest op-1 stock top in disc, tip Z, intent):")
for kk, vv in sorted(acc.items(), key=lambda kv: -kv[1])[:15]: print("  ", kk, round(vv))
print("by distance of the highest cell outside the silhouette:", sorted((a, round(b)) for a, b in acc2.items()))
