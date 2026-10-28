"""D69: the model top Z on the stock grid of a run (0.25 mm cells; the max Z of the triangles over each cell centre;
NaN = no model) and the holes of the plate: connected components of no-model cells that do not touch the grid border.
Writes model_<label>.npz (ztop, holes label image). Usage: model_map.py <stl> <stock npz> <label>"""
import struct, sys
import numpy as np
from scipy import ndimage
stl, snpz, label = sys.argv[1:4]
b = open(stl, "rb").read(); n = struct.unpack("<I", b[80:84])[0]
A = np.frombuffer(b[84:84 + n * 50], dtype=np.dtype([("n", "<3f4"), ("v", "<9f4"), ("a", "<u2")]))["v"].reshape(n, 3, 3).astype(float)
S = np.load(snpz); lo, g = S["lo"], float(S["g"]); ny, nx = S["top_after_1"].shape
# The project places the model in the stock frame; the stock is auto_from_model with padding, so read the offset from the bboxes.
mn = A.reshape(-1, 3).min(0); mx = A.reshape(-1, 3).max(0)
print("model bbox", mn.round(3), mx.round(3), file=sys.stderr)
z = np.full((ny, nx), np.nan)
xs = lo[0] + (np.arange(nx) + 0.5) * g; ys = lo[1] + (np.arange(ny) + 0.5) * g
for t in A:
    x0, x1 = t[:, 0].min(), t[:, 0].max(); y0, y1 = t[:, 1].min(), t[:, 1].max()
    i0, i1 = np.searchsorted(xs, x0), np.searchsorted(xs, x1, "right"); j0, j1 = np.searchsorted(ys, y0), np.searchsorted(ys, y1, "right")
    if i0 >= i1 or j0 >= j1: continue
    X, Y = np.meshgrid(xs[i0:i1], ys[j0:j1]); (ax, ay, az), (bx, by, bz), (cx, cy, cz) = t
    d = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy)
    if abs(d) < 1e-12: continue
    l1 = ((by - cy) * (X - cx) + (cx - bx) * (Y - cy)) / d; l2 = ((cy - ay) * (X - cx) + (ax - cx) * (Y - cy)) / d; l3 = 1 - l1 - l2
    m = (l1 >= -1e-9) & (l2 >= -1e-9) & (l3 >= -1e-9)
    if not m.any(): continue
    Z = l1 * az + l2 * bz + l3 * cz; sub = z[j0:j1, i0:i1]; sub[m] = np.fmax(sub[m], Z[m])
empty = np.isnan(z); lab, k = ndimage.label(empty)
border = set(np.unique(np.r_[lab[0], lab[-1], lab[:, 0], lab[:, -1]])) - {0}
holes = np.where(np.isin(lab, list(border)), 0, lab); ids = [i for i in range(1, k + 1) if i not in border]
print("holes", len(ids), file=sys.stderr)
np.savez_compressed(f"model_{label}.npz", ztop=z, holes=holes)
