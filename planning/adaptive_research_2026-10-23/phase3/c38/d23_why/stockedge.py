"""C38 D22 analysis, step 5: the Z 17 outer descents beyond the mesh bbox,
split at R (3 mm) from the stock-box edge. Within R the C31 grow put phantom
material beyond the edge in the 2D grid; at R or more the disc stays inside
the box, so the stock-box clip cannot change the descent. Same selection as
classify_outer_descents.py, on ../../d22/entries_d22.json."""
import json
from pathlib import Path
import numpy as np
from scipy import ndimage
HERE = Path(__file__).resolve().parent
P3 = HERE.parent.parent
part = np.load(P3 / "fix9" / "corne_part.npz")
top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
STOCK_MIN = (0.31372880935668945, -109.31554412841797)
STOCK_MAX = (153.07408142089844, 1.8791859149932861)
R = 3.0
ent = json.loads((P3 / "fix13" / "entries_d23.json").read_text())
COLS = ['zigzag_pick D20', 'spiral_pick C35', 'spiral_headon D23', 'spiral_pick D23']
out = {}
for col in COLS:
    rows = ent["corne " + col]["rows"]
    sel = [r for r in rows if r["style"] == "keep-down link" and r["end_plane"] == 17.0
           and r["descent_mm"] >= 1 and r["removed_mm3"] > 0]
    edges = []
    for r in sel:
        c = int(round((r["x"] - x0) / g)); rr = int(round((r["y"] - y0) / g))
        if 0 <= rr < top.shape[0] and 0 <= c < top.shape[1]:
            continue
        edges.append(min(r["x"] - STOCK_MIN[0], STOCK_MAX[0] - r["x"], r["y"] - STOCK_MIN[1], STOCK_MAX[1] - r["y"]))
    e = np.array(edges)
    out[col] = {"beyond_bbox": int(len(e)), "within_R_of_stock_edge": int((e < R).sum()), "at_R_or_more": int((e >= R).sum())}
print(json.dumps(out, indent=1))
assert out["spiral_headon D23"] == {"beyond_bbox": 18, "within_R_of_stock_edge": 1, "at_R_or_more": 17}, out
assert out["spiral_pick D23"] == {"beyond_bbox": 14, "within_R_of_stock_edge": 1, "at_R_or_more": 13}, out
print("ASSERTS OK")
