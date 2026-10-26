"""C38 D23 analysis: where the 455 mm2 of the 3-9 mm band stand (D23
spiral_pick stock map, the bands_d23.py method), split by whether an op-1
cutter centre below Z 17 came within R (3 mm) of the cell, and by the
cell's distance to the stock-box edge."""
import json
from pathlib import Path
import numpy as np
from scipy import ndimage
from scipy.spatial import cKDTree
H = Path(__file__).resolve().parent
import sys
RUN = sys.argv[1]
MAP, SAMPLES, OUTF = {"pick": ("fix13/stock_corne_spiral_pick_D23.npz", H / "samples_pick.npz", "band_pick.json"), "c38a": ("d22/stock_corne_spiral_pick_C38a.npz", H.parent / "d22_why" / "samples_c38a.npz", "band_c38a.json")}[RUN]
P3 = H.parent.parent
part = np.load(P3 / "fix9" / "corne_part.npz")
ptop, pg, px0, py0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
m = np.load(P3 / MAP)
top, target, xs, ys, g = m["top"], m["target"], m["xs"], m["ys"], float(m["g"])
X, Y = np.meshgrid(xs, ys)
i = np.round((X - px0) / pg).astype(int); j = np.round((Y - py0) / pg).astype(int)
ok = (i >= 0) & (i < ptop.shape[1]) & (j >= 0) & (j < ptop.shape[0])
t = np.zeros(X.shape); t[ok] = np.nan_to_num(ptop[j[ok], i[ok]], nan=0.0, neginf=0.0)
sil = ndimage.binary_fill_holes(t > 1e-6)
dout = ndimage.distance_transform_edt(~sil) * g
band = ((top - target) > 0.8) & ~sil & (dout > 3) & (dout <= 9)
SMIN, SMAX = (0.31372880935668945, -109.31554412841797), (153.07408142089844, 1.8791859149932861)
edge = np.minimum.reduce([X - SMIN[0], SMAX[0] - X, Y - SMIN[1], SMAX[1] - Y])
S = np.load(SAMPLES)
pos = S["pos"].astype(float)
c = (S["tp"] == 1) & S["cut"] & (pos[:, 2] < 16.5)
tree = cKDTree(pos[c, :2])
d, _ = tree.query(np.c_[X[band], Y[band]])
reached = d <= 3.0
cell = g * g
e = edge[band]
out = {
    "band_3_9_mm2": round(float(band.sum() * cell)),
    "not_reached_mm2": round(float((~reached).sum() * cell)),
    "reached_mm2": round(float(reached.sum() * cell)),
    "not_reached_within_6mm_of_stock_edge_mm2": round(float(((~reached) & (e <= 6.0)).sum() * cell)),
    "reached_within_6mm_of_stock_edge_mm2": round(float((reached & (e <= 6.0)).sum() * cell)),
    "by_distance_from_wall_not_reached": {f"{lo}-{hi}": round(float(((~reached) & (dout[band] > lo) & (dout[band] <= hi)).sum() * cell)) for lo, hi in ((3, 4.5), (4.5, 6), (6, 9))},
    "by_distance_from_wall_reached": {f"{lo}-{hi}": round(float((reached & (dout[band] > lo) & (dout[band] <= hi)).sum() * cell)) for lo, hi in ((3, 4.5), (4.5, 6), (6, 9))},
}
print(json.dumps(out, indent=1))
(H / OUTF).write_text(json.dumps(out, indent=1))
