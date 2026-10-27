"""D47: op-1 area load table (fix27/sweep_table.py's rule: area_oracle.py read with AREA_SCREEN=0, every clearing_cut cutting
sample; percentiles weighted by XY step). Own bound = ceiling(s) + (0.5 + 0.1 + 0.05) / 6 (D37; wanaka op 1 has the same
radial leave 0.5 and tolerance 0.1 as Corne; tool R 3)."""
import json, math
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
def wq(v, w, q):
    o = np.argsort(v); c = np.cumsum(w[o]); return float(v[o][np.searchsorted(c, q * c[-1])])
def ceiling(s, R=3.0):
    a = math.acos(max(-1, min(1, 1 - min(s, 2 * R) / R))) / (2 * math.pi) * 1.05
    return (1 - math.cos(2 * math.pi * min(a, 0.5))) / 2
rows = {}
for tag, f, s in (("CP alone 0.6", "area_fix25_cp_alone_0.6_wanaka200.npz", 2.0), ("geo s2.0", "area_fix33_s2p0.npz", 2.0), ("geo s3.5", "area_fix33_s3p5.npz", 3.5)):
    if not (H / f).exists(): continue
    z = np.load(H / f); a = z["data"]; c = list(z["cols"]); L = a[:, c.index("L")]; w = a[:, c.index("step_mm")]
    b = round(ceiling(s) + (0.5 + 0.1 + 0.05) / 6, 3)
    r = {"stepover": s, "read": int(len(a)), "L_max": round(float(L.max()), 3), "L_p99": round(wq(L, w, 0.99), 3), "L_p95": round(wq(L, w, 0.95), 3),
         "own_bound": b, "mm_over_own_bound": round(float(w[L > b].sum()), 1), "mm_over_0.471": round(float(w[L > 0.471].sum()), 1),
         "mm_over_0.734": round(float(w[L > 0.734].sum()), 1), "xy_mm_read": round(float(w.sum()), 1)}
    rows[tag] = r; print(tag, json.dumps(r))
json.dump(rows, open(H / "load_table.json", "w"), indent=1)
