"""C22 §16.9 item 3: fix20/turn_place.py's count (fix11/audit.py moves and tangents, Corne op 1,
turns over 30 deg inside the silhouette), once with the audit's 3D tangents and once with the
tangents projected to XY. The difference is the turns that are a change of slope, not of
heading."""
import importlib.util, json, math
from pathlib import Path
import numpy as np
from scipy import ndimage
H = Path(__file__).resolve().parent; P3 = H.parent.parent
spec = importlib.util.spec_from_file_location("audit", P3 / "fix11" / "audit.py"); A = importlib.util.module_from_spec(spec); spec.loader.exec_module(A)
part = np.load(P3 / "fix9" / "corne_part.npz"); top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
sil = ndimage.binary_fill_holes(np.isfinite(top) & (top > 1e-6))
def inside(x, y):
    c, r = int(round((x - x0) / g)), int(round((y - y0) / g))
    return 0 <= r < sil.shape[0] and 0 <= c < sil.shape[1] and bool(sil[r, c])
SX, SY = 0.31372880935668945, -109.31554412841797
def xyn(t):
    if t is None: return None
    n = math.hypot(t[0], t[1]); return None if n < 1e-12 else (t[0]/n, t[1]/n, 0.0)
OUT = {}
for run in ("fix16/corne_pick", "fix20/a_only/corne"):
    d = P3 / run
    n = json.load(open(sorted(d.glob("tp_1_*.json"))[0]))["move_count"]
    mv = A.op_moves(str(d / "program.nc"), "3D Rough 1")[1:n + 1]
    xy = lambda m: math.hypot(m["b"][0] - m["a"][0], m["b"][1] - m["a"][1])
    res = {"3d": 0, "xy": 0}
    for mode in ("3d", "xy"):
        prev = None; prev_xy = False
        for m in mv:
            if m["g"] == 0: prev = None; continue
            t0, t1 = A.tangents(m)
            if t0 is None: continue
            if mode == "xy": t0, t1 = xyn(t0), xyn(t1)
            if prev is not None and t0 is not None and prev_xy and xy(m) >= A.GCODE_EPS:
                t = math.degrees(math.acos(max(-1.0, min(1.0, sum(a * b for a, b in zip(prev, t0))))))
                if t > 30.0 and inside(m["a"][0] + SX, m["a"][1] + SY): res[mode] += 1
            prev = t1; prev_xy = xy(m) >= A.GCODE_EPS
    OUT[run] = res; print(run, res)
json.dump(OUT, open(H / "turns_xy.json", "w"), indent=1)
