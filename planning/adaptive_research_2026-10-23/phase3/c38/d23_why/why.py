"""C38 D23 analysis: op 1 in the outer strip (slot feeds), the copy op's deep
cuts, from samples_<run>.npz (extract.py). Read only."""
import json
from pathlib import Path
import numpy as np
from scipy import ndimage
H = Path(__file__).resolve().parent
src = (H.parent / "d22_why" / "why.py").read_text().split("C_OP1 = ")[0].replace(
    'HERE = Path(__file__).resolve().parent\nFIX9 = HERE.parent.parent / "fix9"',
    'HERE = Path(__file__).resolve().parent\nFIX9 = HERE.parent.parent / "fix9"')
exec(src)
STEP = 0.3
C = 0.2189
out = {}
for run in ("pick", "headon"):
    S = np.load(H / f"samples_{run}.npz"); it = list(S["intents"])
    pos = S["pos"].astype(float); w = wall(pos); z = pos[:, 2]
    lv = np.zeros(len(z), bool)
    for L in (11.0, 5.0, 0.5):
        lv |= np.abs(z - L) < 0.05
    m = (S["tp"] == 1) & S["cut"] & (w > 3.0) & (w <= 6.2) & lv & (S["intent"] == it.index("clearing_cut"))
    f = S["feed"][m]; rad = S["radial"][m]
    full = float(np.max(S["feed"][(S["tp"] == 1) & S["cut"] & (S["intent"] == it.index("clearing_cut"))]))
    red = f < 0.9 * np.median(S["feed"][(S["tp"] == 1) & S["cut"] & (S["intent"] == it.index("clearing_cut"))])
    out[run] = {
        "op1_strip_cut_mm": round(float(m.sum()) * STEP, 1),
        "op1_strip_radial_p50": round(float(np.median(rad)), 3),
        "op1_strip_radial_over_c": int((rad > C).sum()),
        "op1_strip_samples": int(m.sum()),
        "op1_strip_reduced_feed_samples": int(red.sum()),
        "op1_strip_reduced_feed_p50": round(float(np.median(f[red])), 0) if red.any() else None,
        "op1_strip_over_c_at_reduced_feed": int((red & (rad > C)).sum()),
        "op1_clearing_feed_max": full,
    }
    # copy op: samples cutting deeper than 1.5 x its Depth/Pass (4.5 mm)
    cm = (S["tp"] == 3) & S["cut"] & (S["axial"] > 4.5)
    wc = w[cm]
    out[run]["copy_deep_samples"] = int(cm.sum())
    out[run]["copy_deep_by_intent"] = {it[i]: int((cm & (S["intent"] == i)).sum()) for i in range(len(it)) if (cm & (S["intent"] == i)).any()}
    out[run]["copy_deep_by_wall"] = {"inside": int((wc <= 0).sum()), "0-1.5": int(((wc > 0) & (wc <= 1.5)).sum()),
                                     "1.5-3": int(((wc > 1.5) & (wc <= 3)).sum()), ">3": int((wc > 3).sum())}
    out[run]["copy_deep_axial_max"] = round(float(S["axial"][cm].max()), 2) if cm.any() else 0.0
    out[run]["copy_deep_levels"] = sorted({round(float(v), 2) for v in z[cm]})
print(json.dumps(out, indent=1))
(H / "why.json").write_text(json.dumps(out, indent=1))
