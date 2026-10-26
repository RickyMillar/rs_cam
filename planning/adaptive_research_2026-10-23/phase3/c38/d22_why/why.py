"""C38 D22 analysis, step 3: the outer strip below Z 17 and the copy op's
links, D20 against C38a, from samples_<run>.npz (extract.py). Asserts each
number the C38 note section 9 states."""
import json
from pathlib import Path
import numpy as np
from scipy import ndimage
HERE = Path(__file__).resolve().parent
FIX9 = HERE.parent.parent / "fix9"
part = np.load(FIX9 / "corne_part.npz")
top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
pad = int(np.ceil(6.0 / g))
sil = np.pad(ndimage.binary_fill_holes(np.isfinite(top) & (top > 1e-6)), pad)
X0, Y0 = x0 - pad * g, y0 - pad * g
dout = ndimage.distance_transform_edt(~sil) * g
din = ndimage.distance_transform_edt(sil) * g
def wall(pos):
    c = np.clip(np.round((pos[:, 0] - X0) / g).astype(int), 0, sil.shape[1] - 1)
    r = np.clip(np.round((pos[:, 1] - Y0) / g).astype(int), 0, sil.shape[0] - 1)
    return np.where(sil[r, c], -din[r, c], dout[r, c])
C_OP1 = 0.2189
STEP = 0.3
res = {}
for run in ("d20", "c38a"):
    S = np.load(HERE / f"samples_{run}.npz")
    it = list(S["intents"]); kn = list(S["kins"])
    pos = S["pos"].astype(float); w = wall(pos); z = pos[:, 2]
    r = {}
    # op 1, the strip below Z 17: centre 3..6 mm outside the wall (R to the
    # containment line), at the three strip levels.
    m1 = (S["tp"] == 1) & S["cut"] & (w > 3.0) & (w <= 6.2)
    lv = np.zeros(len(z), bool)
    for L in (11.0, 5.0, 0.5):
        lv |= np.abs(z - L) < 0.05
    for name in ("clearing_cut", "linking", "entry_plunge"):
        k = m1 & lv & (S["intent"] == it.index(name))
        rad = S["radial"][k]
        r[f"op1_strip_{name}"] = {
            "samples": int(k.sum()), "length_mm": round(float(k.sum()) * STEP, 1),
            "radial_p50": round(float(np.median(rad)), 3) if k.any() else None,
            "radial_over_c": int((rad > C_OP1).sum()),
            "radial_over_0_9": int((rad > 0.9).sum()),
            "feed_p50": round(float(np.median(S["feed"][k])), 0) if k.any() else None,
            "removed_mm3": round(float(S["removed"][k].sum()), 1),
        }
    # copy op links that remove more than 3.30 mm of height (the hard bar's
    # measure: axial engagement of a link sample over 1.1 x Depth/Pass 3.0)
    lk = (S["tp"] == 3) & S["cut"] & (S["intent"] == it.index("linking"))
    bad = lk & (S["axial"] > 3.30)
    wb = w[bad]
    r["copy_link_samples"] = int(lk.sum())
    r["copy_link_over_3_30"] = int(bad.sum())
    r["copy_link_over_moves"] = int(len(np.unique(S["mv"][bad])))
    r["copy_link_over_by_wall"] = {
        "inside": int((wb <= 0).sum()), "0-1.5": int(((wb > 0) & (wb <= 1.5)).sum()),
        "1.5-3": int(((wb > 1.5) & (wb <= 3.0)).sum()), "3-4.5": int(((wb > 3.0) & (wb <= 4.5)).sum()),
        ">4.5": int((wb > 4.5).sum()),
    }
    r["copy_moves_max_index"] = int(S["mv"][S["tp"] == 3].max())
    res[run] = r
print(json.dumps(res, indent=1))
(HERE / "why.json").write_text(json.dumps(res, indent=1))
