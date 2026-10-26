"""D24 (copy of fix13/bands_d23.py, one more column; that file is a copy of d22/bands_d22.py, more columns): the standing-stock bands of R4 (R4_independent_review.md, the Corne
table): stock more than 0.8 mm above the target, split by distance from the
hole-filled part silhouette. The part mask is fix9/corne_part.npz `top`
resampled (nearest cell) onto the stock grid, then binary_fill_holes. The
distance is the Euclidean distance transform times the grid step. Checks
first that the fix9 D20 map gives R4's D20 row."""
import json, os
import numpy as np
from scipy import ndimage

H = os.path.dirname(os.path.abspath(__file__)); P3 = os.path.join(H, "..")
part = np.load(os.path.join(P3, "fix9", "corne_part.npz"))
ptop, pg, px0, py0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
OUT = [(0, 1.5), (1.5, 3), (3, 4.5), (4.5, 6), (6, 9), (9, 1e9)]
IN = [(0, 3), (3, 6), (6, 1e9)]


def bands(npz):
    m = np.load(npz); top, target, xs, ys, g = m["top"], m["target"], m["xs"], m["ys"], float(m["g"])
    X, Y = np.meshgrid(xs, ys)
    i = np.round((X - px0) / pg).astype(int); j = np.round((Y - py0) / pg).astype(int)
    ok = (i >= 0) & (i < ptop.shape[1]) & (j >= 0) & (j < ptop.shape[0])
    t = np.zeros(X.shape); t[ok] = np.nan_to_num(ptop[j[ok], i[ok]], nan=0.0, neginf=0.0)
    sil = ndimage.binary_fill_holes(t > 1e-6)
    dout = ndimage.distance_transform_edt(~sil) * g
    din = ndimage.distance_transform_edt(sil) * g
    st = (top - target) > 0.8
    c = g * g
    r = {f"outside {lo}-{hi}": round(float((st & ~sil & (dout > lo) & (dout <= hi)).sum() * c)) for lo, hi in OUT}
    r.update({f"inside {lo}-{hi}": round(float((st & sil & (din > lo) & (din <= hi)).sum() * c)) for lo, hi in IN})
    r["outside 3-9"] = r["outside 3-4.5"] + r["outside 4.5-6"] + r["outside 6-9"]
    r["silhouette mm2"] = round(float(sil.sum() * c))
    r["whole box"] = round(float(st.sum() * c))
    return r


d20 = bands(os.path.join(P3, "fix9", "stock_corne_zigzag_pick_D20.npz"))
print("fix9 D20 (R4 check):", d20)
res = {"fix9 D20": d20}
for col, f in (("zigzag_pick_D20", "fix9/stock_corne_zigzag_pick_D20.npz"), ("spiral_pick_C35", "fix13/stock_corne_spiral_pick_C35.npz"), ("spiral_pick_C38a", "d22/stock_corne_spiral_pick_C38a.npz"), ("spiral_headon_D23", "fix13/stock_corne_spiral_headon_D23.npz"), ("spiral_pick_D23", "fix13/stock_corne_spiral_pick_D23.npz"), ("spiral_pick_D24", "fix14/stock_corne_spiral_pick_D24.npz")):
    res[col] = bands(os.path.join(P3, f))
    print(col, res[col])
json.dump(res, open(os.path.join(H, "bands_d24.json"), "w"), indent=1)
