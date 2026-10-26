"""D25 copy of c38/d23_why/op1_deep.py (runs: D24 pick, D25 pick, D25 headon; output in d25/why).

C38 §11.1: op 1 cuts of 12 mm (axial over 9.0 = 1.5 x Depth/Pass 6) in
C35 (fix12, no C38), C38a (nibble, no slot) and D23 (slot), with the
place by the padded silhouette and the stock-box edge distance."""
import json
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
exec((H.parent.parent / "c38" / "d22_why" / "why.py").read_text().split("C_OP1 = ")[0])
SMIN, SMAX = (0.31372880935668945, -109.31554412841797), (153.07408142089844, 1.8791859149932861)
runs = {"D24 pick": H / "samples_d24pick.npz", "D25 pick": H / "samples_d25pick.npz", "D25 headon": H / "samples_d25headon.npz"}
out = {}
for name, f in runs.items():
    S = np.load(f); pos = S["pos"].astype(float); w = wall(pos)
    edge = np.minimum.reduce([pos[:, 0] - SMIN[0], SMAX[0] - pos[:, 0], pos[:, 1] - SMIN[1], SMAX[1] - pos[:, 1]])
    m = (S["tp"] == 1) & S["cut"] & (S["axial"] > 9.0)
    out[name] = {
        "op1_axial_over_9": int(m.sum()),
        "axial_max": round(float(S["axial"][m].max()), 2) if m.any() else 0.0,
        "in_outer_strip_3_6.2": int((m & (w > 3.0) & (w <= 6.2)).sum()),
        "within_R_of_stock_edge": int((m & (edge < 3.0)).sum()),
        "levels": sorted({round(float(v), 2) for v in pos[m, 2]}),
    }
print(json.dumps(out, indent=1))
(H / "op1_deep.json").write_text(json.dumps(out, indent=1))
