"""C38 §15: op 1's samples in the outer strip at the strip levels (Z 11, 5,
0.5), D29 a_only Corne: those over the oracle bound c + 0.15 = 0.369 and
over c = 0.219, at the full clearing feed (4000) against a reduced feed;
and the long lowered moves (over 20 mm) by level and place."""
import json
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
exec((H.parent / "d22_why" / "why.py").read_text().split("C_OP1 = ")[0])
S = np.load(H / "samples_d29.npz"); it = list(S["intents"]); pos = S["pos"].astype(float); w = wall(pos); z = pos[:, 2]
lv = np.zeros(len(z), bool)
for L in (11.0, 5.0, 0.5):
    lv |= np.abs(z - L) < 0.05
m = (S["tp"] == 1) & S["cut"] & (w > 3.0) & (w <= 6.2) & lv & (S["intent"] == it.index("clearing_cut"))
r, f = S["radial"][m], S["feed"][m]
full = f >= 3999.0
out = {"strip_samples": int(m.sum()),
       "over_c": int((r > 0.2189).sum()), "over_c_full_feed": int(((r > 0.2189) & full).sum()),
       "over_bound": int((r > 0.369).sum()), "over_bound_full_feed": int(((r > 0.369) & full).sum())}
print(json.dumps(out, indent=1))
(H / "strip_feed.json").write_text(json.dumps(out, indent=1))
