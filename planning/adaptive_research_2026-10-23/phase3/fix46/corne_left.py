"""D66: a copy of the fix45/fix41 corne_left.py (writes into fix46). D64: the stock that op 1 leaves on Corne, by place (fix31/op5_analysis.py's method, op 1 only): a flat 6 mm disc lowered
to each op-1 trace sample tip on a 0.25 mm grid over the stock box; target = the closing of the part surface
(fix9/corne_part.npz) by the 3 mm disc + 0.5 (op 1's axial leave); 'left' = > 0.8 mm above the target. Places: inside the
part (the filled silhouette: case and switch holes), and outside 0-3 / 3-6.2 / 6.2-9 mm (op 1's reach is 9 mm: centre
containment, offset 6). Cells off the part grid are not counted (the grid ends about 2 mm past the part, D49).
Usage: corne_left.py <run dir> <tag>"""
import json, math, sys
from pathlib import Path
import numpy as np
from scipy import ndimage
H = Path(__file__).resolve().parent; P3 = H.parent
d = Path(sys.argv[1]); tag = sys.argv[2]
sim = json.load(open(d / "simulation.json")); lo, hi = sim["stock_bbox_min"], sim["stock_bbox_max"]
P = np.array([s["position"] for s in sim["trace"]["samples"] if s["toolpath_id"] == 1])
g = 0.25; R = 3.0; nx = int(round((hi[0] - lo[0]) / g)); ny = int(round((hi[1] - lo[1]) / g))
top = np.full((ny, nx), float(hi[2]))
seg = np.diff(P, axis=0); L = np.hypot(seg[:, 0], seg[:, 1]); n = np.maximum(1, np.ceil(L / (g / 2)).astype(int))
idx = np.repeat(np.arange(len(seg)), n); f = (np.arange(n.sum()) - np.repeat(np.cumsum(n) - n, n) + 1) / np.repeat(n, n)
D = np.vstack([P[:1], P[idx] + f[:, None] * seg[idx]])
ci = np.floor((D[:, 0] - lo[0]) / g).astype(int); cj = np.floor((D[:, 1] - lo[1]) / g).astype(int); rr = int(math.ceil(R / g)) + 1
for dj in range(-rr, rr + 1):
    for di in range(-rr, rr + 1):
        I, J = ci + di, cj + dj; cx = lo[0] + (I + 0.5) * g; cy = lo[1] + (J + 0.5) * g
        m = ((cx - D[:, 0]) ** 2 + (cy - D[:, 1]) ** 2 <= R * R) & (I >= 0) & (I < nx) & (J >= 0) & (J < ny)
        if m.any(): np.minimum.at(top, (J[m], I[m]), D[m, 2])
part = np.load(P3 / "fix9" / "corne_part.npz"); ptop, pg, px0, py0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
xs = lo[0] + (np.arange(nx) + 0.5) * g; ys = lo[1] + (np.arange(ny) + 0.5) * g; XX, YY = np.meshgrid(xs, ys)
pi_ = np.round((XX - px0) / pg).astype(int); pj = np.round((YY - py0) / pg).astype(int)
ok = (pi_ >= 0) & (pi_ < ptop.shape[1]) & (pj >= 0) & (pj < ptop.shape[0])
surf = np.zeros(XX.shape); t = np.full(XX.shape, -np.inf); t[ok] = ptop[pj[ok], pi_[ok]]; surf[np.isfinite(t)] = t[np.isfinite(t)]
k = int(math.ceil(R / g)); yy, xx = np.mgrid[-k:k + 1, -k:k + 1]; disc = (xx * g) ** 2 + (yy * g) ** 2 <= R * R
target = ndimage.grey_erosion(ndimage.grey_dilation(surf, footprint=disc, mode="nearest"), footprint=disc, mode="nearest") + 0.5
fin = np.isfinite(ptop) & (ptop > 1e-6); fill = ndimage.binary_fill_holes(fin); dout = ndimage.distance_transform_edt(~fill) * pg
inside = np.zeros(XX.shape, bool); inside[ok] = fill[pj[ok], pi_[ok]]
dd = np.full(XX.shape, 99.0); dd[ok] = dout[pj[ok], pi_[ok]]
left = (top - target) > 0.8; cell = g * g; out = {"run": d.name}
for name, m in (("inside the part", inside), ("outside 0-3", ok & ~inside & (dd <= 3)), ("outside 3-6.2", ok & (dd > 3) & (dd <= 6.2)), ("outside 6.2-9", ok & (dd > 6.2) & (dd <= 9))):
    out[name] = {"left_mm2": round(float((left & m).sum() * cell)), "vol_over_target_mm3": round(float(np.clip(top - target, 0, None)[m].sum() * cell)), "max_mm": round(float((top - target)[m].max()), 2)}
print(json.dumps(out)); json.dump(out, open(H / f"left_{tag}.json", "w"), indent=1)
