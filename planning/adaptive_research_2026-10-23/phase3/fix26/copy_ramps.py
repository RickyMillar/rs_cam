"""D35: where the copy op (tp 3, the 3 mm tool) spends its F375 ramp-down
time, body_cp against cp_fix6job. Reads the trace samples of tp 3 with
cut_kinematics 'helix' (a fed move that descends while it travels in XY) at
375 mm/min (the copy op's plunge_rate; the emitted, modulated feed). Each
sample is placed by:
  wall : distance to the hole-filled Corne silhouette (fix9/corne_part.npz),
         negative inside (fix25/p1a_p5a_d34.py's rule);
  z    : the sample's model Z;
  left : the stock above the target before the copy op at that XY
         (fix26/stock_corne_D35_<run>_precopy.npz, stockmap_d35.py).
Prints the time per class and writes copy_ramps.json and img/copy_ramps.png."""
import json, collections
from pathlib import Path
import numpy as np
from scipy import ndimage
H = Path(__file__).resolve().parent; P3 = H.parent
part = np.load(P3 / "fix9" / "corne_part.npz")
top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
pad = int(np.ceil(10.0 / g))
sil = np.pad(ndimage.binary_fill_holes(np.isfinite(top) & (top > 1e-6)), pad)
X0, Y0 = x0 - pad * g, y0 - pad * g
dout = ndimage.distance_transform_edt(~sil) * g; din = ndimage.distance_transform_edt(sil) * g
def wall(x, y):
    c = np.clip(np.round((x - X0) / g).astype(int), 0, sil.shape[1] - 1); r = np.clip(np.round((y - Y0) / g).astype(int), 0, sil.shape[0] - 1)
    return np.where(sil[r, c], -din[r, c], dout[r, c])
WB = [(-1e9, -6), (-6, -3), (-3, 0), (0, 3), (3, 6.2), (6.2, 9), (9, 1e9)]
ZB = [(-1, 3), (3, 6.5), (6.5, 9.5), (9.5, 12.5), (12.5, 15.5), (15.5, 30)]
RUNS = {"body_cp": ("fix26/body_cp_corne", "D35_body_cp_precopy"), "cp_fix6job": ("fix25/cp_fix6job/corne", "D35_cp_fix6job_precopy")}
out = {}; pts = {}
for name, (run, sm) in RUNS.items():
    S = [s for s in json.load(open(P3 / run / "simulation.json"))["trace"]["samples"] if s["toolpath_id"] == 3]
    R = [s for s in S if s["cut_kinematics"] == "helix" and round(s["feed_rate_mm_min"]) == 375]
    p = np.array([s["position"] for s in R]); t = np.array([s["segment_time_s"] for s in R]); rem = np.array([s["removed_volume_est_mm3"] for s in R])
    w = wall(p[:, 0], p[:, 1])
    m = np.load(H / f"stock_corne_{sm}.npz"); st, tg, xs, ys = m["top"], m["target"], m["xs"], m["ys"]; gg = float(m["g"])
    i = np.clip(np.floor((p[:, 0] - xs[0] + gg / 2) / gg).astype(int), 0, len(xs) - 1); j = np.clip(np.floor((p[:, 1] - ys[0] + gg / 2) / gg).astype(int), 0, len(ys) - 1)
    left = (st - tg)[j, i]
    r = {"samples": len(R), "time_s": round(float(t.sum()), 1), "removed_mm3": round(float(rem.sum()), 1),
         "time_removing_s": round(float(t[rem > 0.027].sum()), 1),
         "ramp_moves": len({s["move_index"] for s in R}),
         "ramp_moves_inside_wall": len({s["move_index"] for s, ww in zip(R, wall(p[:, 0], p[:, 1])) if ww < 0}),
         "mean_ramp_s": round(float(t.sum()) / max(1, len({s["move_index"] for s in R})), 2),
         "by_wall_mm": {f"{lo}..{hi}": round(float(t[(w > lo) & (w <= hi)].sum()), 1) for lo, hi in WB},
         "by_z": {f"{lo}..{hi}": round(float(t[(p[:, 2] > lo) & (p[:, 2] <= hi)].sum()), 1) for lo, hi in ZB},
         "by_left_before_copy_mm": {k: round(float(t[mk].sum()), 1) for k, mk in (("<=0.8", left <= 0.8), ("0.8-3", (left > 0.8) & (left <= 3)), ("3-6", (left > 3) & (left <= 6)), (">6", left > 6))}}
    out[name] = r; pts[name] = (p, t, w)
    print(name, json.dumps(r))
# the precopy leftover that body_cp has and CP has not (both > 0.8 mm above target)
a = np.load(H / "stock_corne_D35_body_cp_precopy.npz"); b = np.load(H / "stock_corne_D35_cp_fix6job_precopy.npz")
la, lb = (a["top"] - a["target"]) > 0.8, (b["top"] - b["target"]) > 0.8
XX, YY = np.meshgrid(a["xs"], a["ys"]); W = wall(XX, YY); c = float(a["g"]) ** 2
only = la & ~lb
out["leftover_only_body_cp_mm2"] = {f"{lo}..{hi}": round(float((only & (W > lo) & (W <= hi)).sum() * c)) for lo, hi in WB}
out["leftover_only_cp_mm2"] = {f"{lo}..{hi}": round(float((lb & ~la & (W > lo) & (W <= hi)).sum() * c)) for lo, hi in WB}
print("leftover only body_cp (mm2 by wall band)", out["leftover_only_body_cp_mm2"]); print("leftover only CP", out["leftover_only_cp_mm2"])
json.dump(out, open(H / "copy_ramps.json", "w"), indent=1)
import matplotlib; matplotlib.use("Agg"); import matplotlib.pyplot as plt
(H / "img").mkdir(exist_ok=True)
fig, ax = plt.subplots(1, 2, figsize=(16, 7), sharex=True, sharey=True)
for k, (name, sm) in enumerate((("body_cp", "D35_body_cp_precopy"), ("cp_fix6job", "D35_cp_fix6job_precopy"))):
    m = np.load(H / f"stock_corne_{sm}.npz"); L = m["top"] - m["target"]
    ax[k].imshow(np.where(L > 0.8, L, np.nan), origin="lower", extent=[m["xs"][0], m["xs"][-1], m["ys"][0], m["ys"][-1]], cmap="Greys", vmin=0, vmax=12, alpha=0.7)
    ax[k].contour(XX, YY, W, levels=[0.0, 3.0, 6.2], colors=["k", "tab:blue", "tab:blue"], linewidths=[1.0, 0.6, 0.6])
    p, t, w = pts[name]
    sc = ax[k].scatter(p[:, 0], p[:, 1], c=p[:, 2], s=2, cmap="viridis", vmin=0, vmax=20)
    ax[k].set_title(f"{name}: copy-op F375 ramps, {out[name]['time_s']} s, {out[name]['ramp_moves']} moves")
    ax[k].set_aspect("equal")
fig.suptitle("grey: stock > 0.8 mm above target before the copy op; black: the part wall; blue: 3 and 6.2 mm outside")
fig.colorbar(sc, ax=ax, label="sample Z (mm)", shrink=0.8)
fig.savefig(H / "img" / "copy_ramps.png", dpi=110, bbox_inches="tight")
