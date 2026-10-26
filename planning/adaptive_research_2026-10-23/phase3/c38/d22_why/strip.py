"""C38 D22 analysis, step 2: where and how the C38a run cuts outside the
part silhouette. Reads samples.npz (extract.py) and ../../fix9/corne_part.npz.
The silhouette is padded to cover the stock box, so a sample beyond the mesh
bbox gets its true wall distance."""
import json
from pathlib import Path
import numpy as np
from scipy import ndimage
HERE = Path(__file__).resolve().parent
FIX9 = HERE.parent.parent / "fix9"
S = np.load(HERE / "samples.npz")
part = np.load(FIX9 / "corne_part.npz")
top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
pad = int(np.ceil(6.0 / g))
sil = ndimage.binary_fill_holes(np.isfinite(top) & (top > 1e-6))
sil = np.pad(sil, pad)
X0, Y0 = x0 - pad * g, y0 - pad * g
dout = ndimage.distance_transform_edt(~sil) * g
din = ndimage.distance_transform_edt(sil) * g
def wall(pos):
    c = np.clip(np.round((pos[:, 0] - X0) / g).astype(int), 0, sil.shape[1] - 1)
    r = np.clip(np.round((pos[:, 1] - Y0) / g).astype(int), 0, sil.shape[0] - 1)
    return np.where(sil[r, c], -din[r, c], dout[r, c])
intents = list(S["intents"]); kins = list(S["kins"])
pos = S["pos"].astype(float); w = wall(pos); z = pos[:, 2]
STEP = 0.3
out = {}
C = 0.2189  # op 1 ceiling, s 1.2 R 3 (P2_C2 C24); same for the copy (s/R 0.4)
for tp, name, R in ((1, "op1", 3.0), (3, "copy", 1.5)):
    m = (S["tp"] == tp) & S["cut"]
    rows = {}
    for lo, hi, label in ((-99, 0, "inside"), (0, R, "outside 0-R"), (R, 2 * R, "outside R-2R"), (2 * R, 99, "outside >2R")):
        mm = m & (w > lo) & (w <= hi)
        per = {}
        for i, it in enumerate(intents):
            k = mm & (S["intent"] == i)
            if k.sum() == 0:
                continue
            rad = S["radial"][k]
            per[it] = {
                "samples": int(k.sum()),
                "length_mm": round(float(k.sum()) * STEP, 1),
                "removed_mm3": round(float(S["removed"][k].sum()), 1),
                "radial_over_c": int((rad > C).sum()),
                "radial_p95": round(float(np.percentile(rad, 95)), 3),
                "levels": sorted({round(float(v), 1) for v in z[k]})[:8],
            }
        rows[label] = per
    out[name] = rows
print(json.dumps(out, indent=1))
(HERE / "strip.json").write_text(json.dumps(out, indent=1))
