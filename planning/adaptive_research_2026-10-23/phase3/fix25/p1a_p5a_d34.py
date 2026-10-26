"""D34: C38 §16.4 P1a and P5a on the area-oracle npz files (fix25/area_*.npz,
C18's L per read sample).
- P1a: op 1 read samples with L > 0.471 at Corne Z 11 / 5 / 0.5 (|z - level| < 0.05)
  between 3.0 and 6.2 mm outside the wall (the fix21/feed_check.py strip rule:
  the padded hole-filled silhouette of fix9/corne_part.npz). Sum of step_mm.
- P5a: the over-0.471 length (L) of the CP rest op in the edge band against
  CP alone's op 1 in the same band. Corne: 0-9 mm outside the silhouette (any Z).
  wanaka200: within 9 mm of the stock box edge in XY (the sim frame; the box
  from simulation.json stock_bbox_min/max, read from the file head).
Writes p1a_p5a_d34.json."""
import json, re
from pathlib import Path
import numpy as np
from scipy import ndimage
H = Path(__file__).resolve().parent; P3 = H.parent
B = 0.471
part = np.load(P3 / "fix9" / "corne_part.npz")
top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
pad = int(np.ceil(10.0 / g))
sil = np.pad(ndimage.binary_fill_holes(np.isfinite(top) & (top > 1e-6)), pad)
X0, Y0 = x0 - pad * g, y0 - pad * g
dout = ndimage.distance_transform_edt(~sil) * g; din = ndimage.distance_transform_edt(sil) * g
def wall(x, y):
    c = np.clip(np.round((x - X0) / g).astype(int), 0, sil.shape[1] - 1)
    r = np.clip(np.round((y - Y0) / g).astype(int), 0, sil.shape[0] - 1)
    return np.where(sil[r, c], -din[r, c], dout[r, c])
def load(name):
    z = np.load(H / f"area_{name}.npz"); a = z["data"]; c = list(z["cols"])
    return {k: a[:, c.index(k)] for k in c}
def box(run):
    head = open(P3 / run / "simulation.json").read(4000)
    lo = [float(v) for v in re.search(r'"stock_bbox_min": \[([^\]]+)\]', head).group(1).split(",")]
    hi = [float(v) for v in re.search(r'"stock_bbox_max": \[([^\]]+)\]', head).group(1).split(",")]
    return lo, hi
out = {}
for col in ("body_cp", "body_cp_field", "cp_alone"):
    d = load(f"fix25_{col}_corne")
    lv = np.zeros(len(d["z"]), bool)
    for L in (11.0, 5.0, 0.5):
        lv |= np.abs(d["z"] - L) < 0.05
    w = wall(d["x"], d["y"]); m = lv & (w > 3.0) & (w <= 6.2)
    out[f"corne {col} op1 P1a"] = {"strip_read": int(m.sum()), "strip_L_over_mm": round(float(d["step_mm"][m & (d["L"] > B)].sum()), 1),
                                   "strip_width_over_mm": round(float(d["step_mm"][m & (d["sim_radial"] > B)].sum()), 1)}
def edge_corne(d):
    w = wall(d["x"], d["y"]); return (w > 0.0) & (w <= 9.0)
def edge_wanaka(d, run):
    lo, hi = box(run); e = np.minimum.reduce([d["x"] - lo[0], hi[0] - d["x"], d["y"] - lo[1], hi[1] - d["y"]]); return e <= 9.0
for fx, rest, ref, ref_run in (("corne", "tp5", "fix25_cp_alone_corne", None), ("wanaka200", "tp7", "fix25_cp_alone_0.6_wanaka200", "fix25/cp_alone_0.6/wanaka200")):
    r = load(ref); mr = edge_corne(r) if fx == "corne" else edge_wanaka(r, ref_run)
    refv = float(r["step_mm"][mr & (r["L"] > B)].sum())
    for col in ("body_cp", "body_cp_field"):
        d = load(f"fix25_{col}_{fx}_{rest}"); m = edge_corne(d) if fx == "corne" else edge_wanaka(d, f"fix25/{col}/{fx}")
        v = float(d["step_mm"][m & (d["L"] > B)].sum()); tol = max(0.2 * refv, 15.0)
        out[f"{fx} {col} {rest} P5a"] = {"edge_read": int(m.sum()), "edge_L_over_mm": round(v, 1), "cp_alone_op1_edge_L_over_mm": round(refv, 1),
                                         "cp_alone_edge_read": int(mr.sum()), "tolerance_mm": round(tol, 1), "holds": bool(abs(v - refv) <= tol)}
for k, v in out.items():
    print(k, json.dumps(v))
json.dump(out, open(H / "p1a_p5a_d34.json", "w"), indent=1)
