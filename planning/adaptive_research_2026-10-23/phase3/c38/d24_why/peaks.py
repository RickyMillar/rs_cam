"""C38 D24 analysis, step 4: the op-1 18 mm peak (Z 5) and the copy-op
15.56 mm peak (Z 0.5): their centres' wall distance (padded silhouette),
distance to the stock-box edge, the containment line of the op (op 1
6 mm, copy 3 mm), and which earlier levels of the same op cut within R
(the centre column) and within 2R (a disc that touches this disc)."""
import json
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
exec((H.parent / "d22_why" / "why.py").read_text().split("C_OP1 = ")[0])
SMIN, SMAX = (0.31372880935668945, -109.31554412841797), (153.07408142089844, 1.8791859149932861)
S = np.load(H / "samples_d24.npz"); pos = S["pos"].astype(float); w = wall(pos)
edge = np.minimum.reduce([pos[:, 0] - SMIN[0], SMAX[0] - pos[:, 0], pos[:, 1] - SMIN[1], SMAX[1] - pos[:, 1]])
out = {}
for tp, lim, R_, line in ((1, 17.0, 3.0, 6.0), (3, 15.0, 1.5, 3.0)):
    pk = np.nonzero((S["tp"] == tp) & S["cut"] & (S["axial"] > lim))[0]
    rows = []
    for j in pk:
        p = pos[j]
        before = (S["tp"] == tp) & S["cut"] & (np.arange(len(pos)) < j)
        d = np.hypot(pos[:, 0] - p[0], pos[:, 1] - p[1])
        levels = (17.0, 11.0) if tp == 1 else (14.01, 11.01, 8.01, 5.01, 2.01)
        lv_r = [L for L in levels if (before & (d <= R_) & (np.abs(pos[:, 2] - L) < 0.05)).any()]
        lv_2r = sorted({round(float(v), 2) for v in pos[before & (d <= 2 * R_), 2] if v > p[2] + 0.5})
        rows.append({"xyz": [round(float(v), 2) for v in p], "axial": round(float(S["axial"][j]), 2),
                     "wall_mm": round(float(w[j]), 2), "stock_edge_mm": round(float(edge[j]), 2),
                     "beyond_line_by_mm": round(float(w[j] - line), 2),
                     "level_cuts_within_R_at": lv_r, "earlier_levels_within_2R_count": len(lv_2r)})
    out[f"tp{tp}"] = rows[:10]
print(json.dumps(out, indent=1))
(H / "peaks.json").write_text(json.dumps(out, indent=1))
