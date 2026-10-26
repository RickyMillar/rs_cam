"""C38 D23 analysis: are the corridor slot steps fed at c/w? Op 1 in the
outer strip at Z 11/5/0.5 (centres 3-6.2 mm outside the silhouette), D23
spiral_pick: the feed of the over-ceiling samples, and the length of the
emitted moves against the slot step (at most s = 1.2 mm)."""
import json
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent
exec((H.parent / "d22_why" / "why.py").read_text().split("C_OP1 = ")[0])
S = np.load(H / "samples_pick.npz"); it = list(S["intents"])
pos = S["pos"].astype(float); w = wall(pos); z = pos[:, 2]
lv = np.zeros(len(z), bool)
for L in (11.0, 5.0, 0.5):
    lv |= np.abs(z - L) < 0.05
m = (S["tp"] == 1) & S["cut"] & (w > 3.0) & (w <= 6.2) & lv & (S["intent"] == it.index("clearing_cut"))
rad, f = S["radial"][m], S["feed"][m]
idx = np.nonzero(m)[0]; mv = S["mv"][idx]
lens = []
for k in np.unique(mv):
    j = idx[mv == k]
    if len(j) >= 2:
        lens.append(float(np.hypot(*(pos[j[-1], :2] - pos[j[0], :2]))) + 0.3)
lens = np.array(lens)
out = {
    "samples": int(m.sum()),
    "over_c": int((rad > 0.2189).sum()),
    "over_c_at_full_feed_4000": int(((rad > 0.2189) & (f >= 3999)).sum()),
    "reduced_feed_samples": int((f < 3999).sum()),
    "moves": int(len(lens)),
    "move_length_p50_mm": round(float(np.median(lens)), 2),
    "moves_longer_than_s": int((lens > 1.2 + 0.31).sum()),
}
print(json.dumps(out, indent=1))
assert out["samples"] == 6049 and out["over_c"] == 2940
assert out["over_c_at_full_feed_4000"] == 2940
assert out["reduced_feed_samples"] == 6
print("ASSERTS OK")
(H / "slotfeed.json").write_text(json.dumps(out, indent=1))

# The time the strip would take with the ruling's feed (F x c/w, w the
# sample's radial; the area measure is not in the samples, so this is the
# swept-width form only): the added seconds over the emitted full feed.
over = rad > 0.2189
extra_s = float(np.sum(STEP_MM * (rad[over] / 0.2189 - 1.0) / f[over]) * 60.0) if (STEP_MM := 0.3) else 0.0
print("added seconds at F x c/w on the over-ceiling strip samples:", round(extra_s, 1))
(H / "slotfeed_time.json").write_text(json.dumps({"added_s": round(extra_s, 1)}))
