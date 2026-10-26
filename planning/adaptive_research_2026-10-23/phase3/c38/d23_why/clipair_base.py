"""C38 G-CLIPAIR pre-registration baselines (D23, both Corne columns): deep
cuts (axial engagement over 1.5 x Depth/Pass: op 1 9.0 mm, copy 4.5 mm) by
op and centre wall distance; the containment lines are op 1 6 mm and the
copy 3 mm outside the silhouette."""
import json
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
exec((H.parent / "d22_why" / "why.py").read_text().split("C_OP1 = ")[0])
out = {}
for run in ("headon", "pick"):
    S = np.load(H / f"samples_{run}.npz"); it = list(S["intents"])
    pos = S["pos"].astype(float); w = wall(pos)
    r = {}
    for tp, lim, line in ((1, 9.0, 6.0), (3, 4.5, 3.0)):
        m = (S["tp"] == tp) & S["cut"] & (S["axial"] > lim)
        wl = w[m]
        r[f"tp{tp}"] = {
            "deep_samples": int(m.sum()),
            "at_containment_line_pm_0_5": int((np.abs(wl - line) <= 0.5).sum()),
            "inside_silhouette": int((wl <= 0).sum()),
            "axial_max": round(float(S["axial"][m].max()), 2) if m.any() else 0.0,
            "by_intent": {it[i]: int((m & (S["intent"] == i)).sum()) for i in range(len(it)) if (m & (S["intent"] == i)).any()},
        }
    out[run] = r
print(json.dumps(out, indent=1))
(H / "clipair_base.json").write_text(json.dumps(out, indent=1))
