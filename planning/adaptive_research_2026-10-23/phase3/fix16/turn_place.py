"""D26: the planar turns of fix11/audit.py (junctions whose two fed moves
both travel in XY), split by place: inside the hole-filled Corne silhouette
(fix9/corne_part.npz), outside it on the part grid, or beyond the mesh bbox.
The junction point is the start of the second move, moved from the
G-code work frame to the model frame by the stock box minimum (the G-code
origin is the stock corner: x + 0.3137, y - 109.3155, from simulation.json). Uses audit.py's own
op_moves and tangents. Corne '3D Rough 1' only."""
import importlib.util, json, math, sys
from pathlib import Path
import numpy as np
from scipy import ndimage
H = Path(__file__).resolve().parent; P3 = H.parent
spec = importlib.util.spec_from_file_location("audit", P3 / "fix11" / "audit.py"); A = importlib.util.module_from_spec(spec); spec.loader.exec_module(A)
part = np.load(P3 / "fix9" / "corne_part.npz")
top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
sil = ndimage.binary_fill_holes(np.isfinite(top) & (top > 1e-6))
def place(x, y):
    c, r = int(round((x - x0) / g)), int(round((y - y0) / g))
    if not (0 <= r < sil.shape[0] and 0 <= c < sil.shape[1]):
        return "beyond mesh bbox"
    return "inside" if sil[r, c] else "outside"
out = {}
SX, SY = 0.31372880935668945, -109.31554412841797   # stock_bbox_min, simulation.json
for run in sys.argv[1:]:
    d = P3 / run
    tpf = sorted(d.glob("tp_1_*.json"))[0]; n = json.load(open(tpf))["move_count"]
    nc = d / "program.nc"
    allm = A.op_moves(str(nc), "3D Rough 1"); mv = allm[1:n + 1]
    xy = lambda m: math.hypot(m["b"][0] - m["a"][0], m["b"][1] - m["a"][1])
    res = {"over30": {}, "over90": {}}
    prev = None; prev_xy = False
    for m in mv:
        if m["g"] == 0:
            prev = None; continue
        t0, t1 = A.tangents(m)
        if t0 is None:
            continue
        if prev is not None and prev_xy and xy(m) >= A.GCODE_EPS:
            t = math.degrees(math.acos(max(-1.0, min(1.0, sum(a * b for a, b in zip(prev, t0))))))
            pl = place(m["a"][0] + SX, m["a"][1] + SY)
            if t > 30.0:
                res["over30"][pl] = res["over30"].get(pl, 0) + 1
            if t > 90.0 + 1e-6:
                res["over90"][pl] = res["over90"].get(pl, 0) + 1
        prev = t1; prev_xy = xy(m) >= A.GCODE_EPS
    out[run] = res
    print(run, res)
json.dump(out, open(H / "turn_place.json", "w"), indent=1)
