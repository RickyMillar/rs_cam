"""C38 D23 analysis: op 1 strip samples over the oracle bound c + (planner
cell + sim cell + tolerance) / D = 0.2189 + (0.5 + 0.3 + 0.1) / 6 = 0.369
(the common::adaptive_islands bound, Corne cells), C38a (nibble) against
D23 spiral_pick (slot)."""
import json
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
exec((H.parent / "d22_why" / "why.py").read_text().split("C_OP1 = ")[0])
BOUND = 0.2189 + (0.5 + 0.3 + 0.1) / 6.0
out = {}
for run, f in (("nibble C38a", H.parent / "d22_why" / "samples_c38a.npz"), ("slot D23 pick", H / "samples_pick.npz")):
    S = np.load(f); it = list(S["intents"])
    pos = S["pos"].astype(float); w = wall(pos); z = pos[:, 2]
    lv = np.zeros(len(z), bool)
    for L in (11.0, 5.0, 0.5):
        lv |= np.abs(z - L) < 0.05
    m = (S["tp"] == 1) & S["cut"] & (w > 3.0) & (w <= 6.2) & lv & (S["intent"] == it.index("clearing_cut"))
    r = S["radial"][m]
    out[run] = {"samples": int(m.sum()), "over_bound": int((r > BOUND).sum()), "share": round(float((r > BOUND).mean()), 3)}
print(json.dumps(out, indent=1))
assert out["nibble C38a"]["samples"] == 15159 and out["slot D23 pick"]["samples"] == 6049
(H / "overbound.json").write_text(json.dumps(out, indent=1))
print("ASSERTS OK")
