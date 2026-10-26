"""C38 §15: op-1 and copy-op moves of D29 (a_only, Corne) that carry a
feed under the op's clearing feed, by length: the slot steps are at most
s long (1.2 mm op 1, 0.6 mm copy op), so a lowered move longer than one
slot step plus one sample is not a slot step's own move."""
import json
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
exec((H.parent / "d22_why" / "why.py").read_text().split("C_OP1 = ")[0])
S = np.load(H / "samples_d29.npz"); it = list(S["intents"]); pos = S["pos"].astype(float)
out = {}
for tp, s_step, full in ((1, 1.2, None), (3, 0.6, None)):
    m = (S["tp"] == tp) & S["cut"] & (S["intent"] == it.index("clearing_cut"))
    feeds = S["feed"][m]
    top = float(np.percentile(feeds, 95))
    idx = np.nonzero(m)[0]
    rows = []
    for k in np.unique(S["mv"][idx]):
        j = idx[S["mv"][idx] == k]
        f = float(S["feed"][j].min())
        if f >= 0.9 * top:
            continue
        L = float(np.hypot(*(pos[j[-1], :2] - pos[j[0], :2]))) + 0.3
        rows.append((int(k), L, f, float(pos[j[0], 2]), float(wall(pos[j[:1]])[0])))
    rows = np.array(rows) if rows else np.zeros((0, 5))
    longer = rows[rows[:, 1] > s_step + 0.31] if len(rows) else rows
    out[f"tp{tp}"] = {
        "clearing_feed_p95": round(top, 0),
        "reduced_moves": int(len(rows)),
        "reduced_moves_longer_than_a_step": int(len(longer)),
        "longest": [[int(r[0]), round(r[1], 2), round(r[2], 0), round(r[3], 2), round(r[4], 2)] for r in sorted(longer.tolist(), key=lambda r: -r[1])[:6]],
    }
print(json.dumps(out, indent=1))
(H / "longmoves.json").write_text(json.dumps(out, indent=1))
