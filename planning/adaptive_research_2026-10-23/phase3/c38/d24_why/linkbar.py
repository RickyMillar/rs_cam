"""C38 D24 analysis, step 6: the op-1 fed-link bar move 6921 (P15 D24):
place, wall distance and stock-edge distance, from the D24 samples."""
import json
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
exec((H.parent / "d22_why" / "why.py").read_text().split("C_OP1 = ")[0])
S = np.load(H / "samples_d24.npz"); pos = S["pos"].astype(float); it = list(S["intents"])
m = (S["tp"] == 1) & (S["mv"] == 6921)
p = pos[m]; w = wall(p)
out = {"samples": int(m.sum()), "z": sorted({round(float(v), 2) for v in p[:, 2]}),
       "wall_min_max": [round(float(w.min()), 2), round(float(w.max()), 2)],
       "stock_edge_x_mm": round(153.07408142089844 - float(p[:, 0].max()), 2),
       "axial_max": round(float(S["axial"][m].max()), 2),
       "intents": sorted({it[i] for i in S["intent"][m]})}
print(json.dumps(out, indent=1))
assert out == {"samples": 9, "z": [11.0], "wall_min_max": [4.1, 5.69], "stock_edge_x_mm": 3.26, "axial_max": 10.5, "intents": ["linking"]}, out
print("ASSERTS OK")
