"""D63 (for C24): the stock top that op 1 leaves outside the Corne silhouette, by distance band. The stock after op 1 is
rebuilt from op 1's trace samples (a flat 6 mm disc lowered to each sample tip, path densified to 0.125 mm, 0.25 mm grid,
from the stock top 23; the D41 / op5_analysis method). Silhouette: the filled top > 0 cells of fix9/corne_part.npz, padded
by 40 mm (D49 corne_reach.py), distance by EDT. The model frame is the sample frame on Corne. Usage: band_tops.py <run>"""
import json, math, sys
from pathlib import Path
import numpy as np
from scipy import ndimage
P3 = Path(__file__).resolve().parent.parent; run = sys.argv[1]
sim = json.load(open(Path(run) / "simulation.json")); hi = sim["stock_bbox_max"][2]
P = np.array([s["position"] for s in sim["trace"]["samples"] if s["toolpath_id"] == 1])
part = np.load(P3 / "fix9" / "corne_part.npz"); ptop, pg, px0, py0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
pad = int(40 / pg); fill = np.pad(ndimage.binary_fill_holes(np.isfinite(ptop) & (ptop > 1e-6)), pad)
X0, Y0 = px0 - pad * pg, py0 - pad * pg; dout = ndimage.distance_transform_edt(~fill) * pg
g = 0.25; R = 3.0; nx, ny = int(fill.shape[1] * pg / g), int(fill.shape[0] * pg / g)
top = np.full((ny, nx), float(hi))
seg = np.diff(P, axis=0); L = np.hypot(seg[:, 0], seg[:, 1]); n = np.maximum(1, np.ceil(L / (g / 2)).astype(int))
idx = np.repeat(np.arange(len(seg)), n); f = (np.arange(n.sum()) - np.repeat(np.cumsum(n) - n, n) + 1) / np.repeat(n, n)
D = np.vstack([P[:1], P[idx] + f[:, None] * seg[idx]])
ci = np.floor((D[:, 0] - X0) / g).astype(int); cj = np.floor((D[:, 1] - Y0) / g).astype(int); rr = int(math.ceil(R / g)) + 1
for dj in range(-rr, rr + 1):
    for di in range(-rr, rr + 1):
        I, J = ci + di, cj + dj; cx = X0 + (I + 0.5) * g; cy = Y0 + (J + 0.5) * g
        m = ((cx - D[:, 0]) ** 2 + (cy - D[:, 1]) ** 2 <= R * R) & (I >= 0) & (I < nx) & (J >= 0) & (J < ny)
        if m.any(): np.minimum.at(top, (J[m], I[m]), D[m, 2])
xs = X0 + (np.arange(nx) + 0.5) * g; ys = Y0 + (np.arange(ny) + 0.5) * g; XX, YY = np.meshgrid(xs, ys)
pi_ = np.round((XX - X0) / pg).astype(int); pj = np.round((YY - Y0) / pg).astype(int)
ok = (pi_ >= 0) & (pi_ < fill.shape[1]) & (pj >= 0) & (pj < fill.shape[0])
dc = np.full(XX.shape, 99.0); dc[ok] = dout[pj[ok], pi_[ok]]
inside_stock = (XX >= sim["stock_bbox_min"][0]) & (XX <= sim["stock_bbox_max"][0]) & (YY >= sim["stock_bbox_min"][1]) & (YY <= sim["stock_bbox_max"][1])
out = {"stock_top": hi}
for a, b in ((0, 3), (3, 6), (0, 6), (6, 9), (9, 12)):
    m = inside_stock & (dc > a) & (dc <= b)
    t = top[m]
    out[f"outside {a}-{b} mm"] = {"cells": int(m.sum()), "max": round(float(t.max()), 3), "p95": round(float(np.percentile(t, 95)), 3),
                                  "p50": round(float(np.percentile(t, 50)), 3), "area_at_top_mm2 (>= top-0.01)": round(float((t >= hi - 0.01).sum() * g * g), 1),
                                  "area_over_20_mm2": round(float((t > 20.0).sum() * g * g), 1)}
print(json.dumps(out, indent=1)); json.dump(out, open(Path(__file__).resolve().parent / "band_tops.json", "w"), indent=1)
