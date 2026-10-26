"""C38 §14: the two D26 fed-link bars (P15 "D26"), read from the D26
spiral_pick simulation (samples_d26pick.npz, extract.py): per sample the
place (padded silhouette wall distance), the removed volume, the axial and
radial engagement; and the intent of the moves that follow."""
import json
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
exec((H.parent / "d22_why" / "why.py").read_text().split("C_OP1 = ")[0])
S = np.load(H / "samples_d26pick.npz"); it = list(S["intents"])
out = {}
for tp, mv in ((1, 5565), (3, 6340)):
    m = (S["tp"] == tp) & (S["mv"] == mv)
    j = np.nonzero(m)[0]
    p = S["pos"][m].astype(float)
    out[f"tp{tp}_move{mv}"] = {
        "samples": int(m.sum()),
        "intents": sorted({it[i] for i in S["intent"][m]}),
        "wall_mm": [round(float(v), 2) for v in wall(p)],
        "removed_mm3": [round(float(v), 3) for v in S["removed"][m]],
        "axial_mm": [round(float(v), 2) for v in S["axial"][m]],
        "radial": [round(float(v), 3) for v in S["radial"][m]],
        "next_intents": [it[i] for i in S["intent"][j[-1] + 1 : j[-1] + 4]],
    }
print(json.dumps(out, indent=1))
a = out["tp1_move5565"]; b = out["tp3_move6340"]
assert a["samples"] == 9 and a["intents"] == ["linking"] and a["axial_mm"][-1] == 10.5
assert a["removed_mm3"][-1] == 6.037 and a["radial"][-1] == 0.144 and max(a["axial_mm"][1:-1]) == 0.0
assert a["wall_mm"][0] == 4.1 and a["wall_mm"][-1] == 5.69 and a["next_intents"][0] == "clearing_cut"
assert b["samples"] == 3 and b["intents"] == ["linking"] and max(b["axial_mm"]) == 5.52
assert round(sum(b["removed_mm3"]), 3) == 1.204 and max(b["radial"]) == 0.0
assert min(b["wall_mm"]) == 2.95 and max(b["wall_mm"]) == 3.05
print("ASSERTS OK")
