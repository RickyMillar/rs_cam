"""D69: the plate holes of the Corne model (model_map.py) and the stock each op leaves in them (stockmap.py).
Per hole: area, inscribed width (2 x max distance to the edge), XY extent, plate top Z around it, and the max stock top
inside after each op. Usage: holes.py <model npz> <stock npz> <tps> -> holes_<label>.json"""
import json, sys
import numpy as np
from scipy import ndimage
M = np.load(sys.argv[1]); S = np.load(sys.argv[2]); tps = [int(t) for t in sys.argv[3].split(",")]; label = sys.argv[4]; g = float(S["g"])
z = M["ztop"]; H = M["holes"]; out = []
for i in [i for i in np.unique(H) if i]:
    m = H == i; bb = np.argwhere(m); ring = ndimage.binary_dilation(m, iterations=4) & ~m
    out.append({"id": int(i), "area": round(float(m.sum() * g * g), 2), "width": round(float(2 * ndimage.distance_transform_edt(m).max() * g), 2),
                "ext": [round(float(e), 2) for e in (bb.max(0) - bb.min(0) + 1) * g], "plate_z": round(float(np.nanmedian(z[ring])), 2),
                "cx": round(float(S["lo"][0] + (bb[:, 1].mean() + 0.5) * g), 2), "cy": round(float(S["lo"][1] + (bb[:, 0].mean() + 0.5) * g), 2),
                "stock_max": {t: round(float(S[f"top_after_{t}"][m].max()), 2) for t in tps},
                "stock_mean": {t: round(float(S[f"top_after_{t}"][m].mean()), 2) for t in tps}})
json.dump(out, open(f"holes_{label}.json", "w"), indent=0)
import collections
print(len(out), "holes; (area, width) counts:", collections.Counter((round(h["area"]), round(h["width"], 1)) for h in out).most_common(8))
for h in sorted(out, key=lambda h: h["area"])[:4] + sorted(out, key=lambda h: h["area"])[-4:]: print(h)
