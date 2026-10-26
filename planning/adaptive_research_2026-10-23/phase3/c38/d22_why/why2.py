"""C38 D22 analysis, step 4: (a) the op-1 strip cut at Z 11/5/0.5 as moves
(length, direction against the wall-distance gradient); (b) the copy op's
cut outside the silhouette, D20 against C38a; (c) whether the 301 over-link
samples lie where no copy clearing sample of the same level passed within R
before them. Reads samples_<run>.npz."""
import json
from pathlib import Path
import numpy as np
from scipy import ndimage
from scipy.spatial import cKDTree
src = (Path(__file__).resolve().parent / "why.py").read_text().split("C_OP1 = ")[0]
exec(src)
STEP = 0.3
out = {}
S = np.load(HERE / "samples_c38a.npz"); it = list(S["intents"])
pos = S["pos"].astype(float); w = wall(pos); z = pos[:, 2]
# (a) moves of op 1 in the strip at the three levels
lv = np.zeros(len(z), bool)
for L in (11.0, 5.0, 0.5):
    lv |= np.abs(z - L) < 0.05
m = (S["tp"] == 1) & S["cut"] & (w > 3.0) & (w <= 6.2) & lv & (S["intent"] == it.index("clearing_cut"))
idx = np.nonzero(m)[0]
mv = S["mv"][idx]
lens, across = [], []
for k in np.unique(mv):
    j = idx[mv == k]
    if len(j) < 2:
        continue
    a, b = pos[j[0], :2], pos[j[-1], :2]
    L = float(np.hypot(*(b - a)))
    if L < 1e-6:
        continue
    lens.append(L)
    across.append(abs(float(w[j[-1]] - w[j[0]])) / L)
lens, across = np.array(lens), np.array(across)
out["op1_strip_moves"] = {
    "moves": int(len(lens)),
    "length_p50_mm": round(float(np.median(lens)), 2),
    "length_p90_mm": round(float(np.percentile(lens, 90)), 2),
    "across_share_p50": round(float(np.median(across)), 2),
    "moves_mostly_across": int((across > 0.7).sum()),
}
# (b) the copy op outside the silhouette, both runs
for run in ("d20", "c38a"):
    T = np.load(HERE / f"samples_{run}.npz"); ti = list(T["intents"])
    p = T["pos"].astype(float); ww = wall(p)
    row = {}
    for name in ("clearing_cut", "linking", "entry_plunge"):
        k = (T["tp"] == 3) & T["cut"] & (T["intent"] == ti.index(name)) & (ww > 0)
        row[name] = {"length_mm": round(float(k.sum()) * STEP, 1),
                     "removed_mm3": round(float(T["removed"][k].sum()), 1)}
    out[f"copy_outside_{run}"] = row
# (c) the over-link samples against earlier copy clearing samples, same level
cc = (S["tp"] == 3) & S["cut"] & (S["intent"] == it.index("clearing_cut"))
lk = (S["tp"] == 3) & S["cut"] & (S["intent"] == it.index("linking")) & (S["axial"] > 3.30)
bad = np.nonzero(lk)[0]
cut_before = 0
for j in bad:
    same = np.nonzero(cc[:j] & (np.abs(z[:j] - z[j]) < 0.05))[0]
    if len(same) and np.min(np.hypot(*(pos[same, :2] - pos[j, :2]).T)) <= 1.5:
        cut_before += 1
out["over_link_samples"] = int(len(bad))
out["over_link_with_a_same_level_cut_within_R_before"] = cut_before
out["over_link_levels"] = sorted({round(float(v), 2) for v in z[bad]})
print(json.dumps(out, indent=1))
(HERE / "why2.json").write_text(json.dumps(out, indent=1))
