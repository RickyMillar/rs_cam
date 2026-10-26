"""D22: a copy of c38/classify_outer_descents.py run on d22/entries_d22.json for
three columns; the selection and the classes are unchanged, and the stock box
is the D20 header as in the original (the stock box is the same project).
The original asserts are kept for the D20 column only.

C38: classify the Corne D20 Z 17 wall-end descents that lie outside the part
silhouette (R4 section 7.2: 124 of them) by what stands beyond them.

Inputs (read only): ../fix9/entries.json (D20 rows), ../fix9/corne_part.npz
(the part heightmap on the mesh bbox, cell 0.2 mm). The stock box comes from
the D20 simulation.json header (stock_bbox_min / stock_bbox_max), copied here
with its source.

A descent is a keep-down-link descent of 1 mm or more that removes stock, at
end plane Z 17 (R4's selection). "Outside" = not in the hole-filled
silhouette. Classes:
- beyond the mesh bbox: the part grid does not cover it; it lies in the 5 mm
  margin between the mesh bbox and the stock box, so its distance to the
  stock-box edge is printed (the true free edge: no stock beyond the box);
- on the grid: its distance to the silhouette, in R4's bands.

Run: python3 -I classify_outer_descents.py  (asserts every number the C38
note states).
"""
import json
from pathlib import Path

import numpy as np
from scipy import ndimage

HERE = Path(__file__).resolve().parent
FIX9 = HERE.parent / "fix9"
P3 = HERE.parent
# phase3/fix7/corne/geodesic_local_zigzag_pick/simulation.json header.
STOCK_MIN = (0.31372880935668945, -109.31554412841797)
STOCK_MAX = (153.07408142089844, 1.8791859149932861)

part = np.load(FIX9 / "corne_part.npz")
top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
sil = ndimage.binary_fill_holes(np.isfinite(top) & (top > 1e-6))
dist_out = ndimage.distance_transform_edt(~sil) * g

import sys
res = {}
for col, run in (("zigzag_pick D20", ""), ("spiral_pick C35", ""), ("spiral_headon D23", ""), ("spiral_pick D23", "")):
    rows = json.loads((HERE / "entries_d23.json").read_text())["corne " + col]["rows"]
    sel = [r for r in rows if r["style"] == "keep-down link" and r["end_plane"] == 17.0
           and r["descent_mm"] >= 1 and r["removed_mm3"] > 0]
    beyond_bbox, on_grid = [], []
    for r in sel:
        c = int(round((r["x"] - x0) / g))
        rr = int(round((r["y"] - y0) / g))
        edge = min(r["x"] - STOCK_MIN[0], STOCK_MAX[0] - r["x"], r["y"] - STOCK_MIN[1], STOCK_MAX[1] - r["y"])
        if 0 <= rr < sil.shape[0] and 0 <= c < sil.shape[1]:
            if sil[rr, c]:
                continue
            on_grid.append((float(dist_out[rr, c]), edge))
        else:
            beyond_bbox.append(edge)

    on = np.array(on_grid)
    bands = {}
    for lo, hi in [(0, 1.5), (1.5, 3), (3, 4.5), (4.5, 6), (6, 99)]:
        bands[f"{lo}-{hi}"] = int(((on[:, 0] >= lo) & (on[:, 0] < hi)).sum())
    out = {
        "z17_descents": len(sel),
        "outside_silhouette": len(beyond_bbox) + len(on_grid),
        "beyond_mesh_bbox": len(beyond_bbox),
        "beyond_bbox_stock_edge_mm_min_max": [round(min(beyond_bbox), 2), round(max(beyond_bbox), 2)],
        "on_grid_by_silhouette_distance_mm": bands,
        "on_grid_stock_edge_mm_min": round(float(on[:, 1].min()), 2),
    }
    res[col] = out
    print(col, json.dumps(out))
json.dump(res, open(HERE / "classify_d23.json", "w"), indent=1)
d = res["zigzag_pick D20"]
assert (d["z17_descents"], d["outside_silhouette"], d["beyond_mesh_bbox"]) == (238, 124, 45), d
assert d["on_grid_by_silhouette_distance_mm"] == {"0-1.5": 0, "1.5-3": 6, "3-4.5": 58, "4.5-6": 15, "6-99": 0}
print("D20 reproduces the C38 note (238 / 124 / 45 and the bands)")
