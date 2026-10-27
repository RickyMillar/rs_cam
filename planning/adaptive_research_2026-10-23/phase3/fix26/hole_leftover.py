"""D35: per switch hole of the Corne part, (1) the copy op's F375 ramp-down
moves and time, and (2) the stock left before the copy op (op 1 for
cp_fix6job; op 1 + the CP rest op 5 for body_cp): the area more than
0.8 mm above the target and the peak height above it, with the peak's
stock-top Z. Holes: the connected components of (hole-filled silhouette
and no part) in fix9/corne_part.npz, area >= 20 mm2 (88 components, 83
of 55.4-55.5 mm2; the 5 under 4 mm2 are slivers). A hole's cells are the
component grown by 1.0 mm (the crescents sit on the hole wall). The
target and the stock top are stockmap_d35.py's (target: the closing of
the part by the 6 mm tool, + 0.5 mm leave). Writes hole_leftover.json and
img/hole_leftover.png."""
import json
from pathlib import Path
import numpy as np
from scipy import ndimage
H = Path(__file__).resolve().parent; P3 = H.parent
part = np.load(P3 / "fix9" / "corne_part.npz")
top, g, x0, y0 = part["top"], float(part["g"]), float(part["x0"]), float(part["y0"])
fin = np.isfinite(top) & (top > 1e-6)
holes = ndimage.binary_fill_holes(fin) & ~fin
lab, n = ndimage.label(holes)
area = ndimage.sum(holes, lab, range(1, n + 1)) * g * g
keep = [i + 1 for i in range(n) if area[i] >= 20.0]
cent = ndimage.center_of_mass(holes, lab, keep)
RUNS = {"body_cp": ("fix26/body_cp_corne", "D35_body_cp_precopy"), "cp_fix6job": ("fix25/cp_fix6job/corne", "D35_cp_fix6job_precopy")}
def hole_of(x, y, grow):
    c = np.round((x - x0) / g).astype(int); r = np.round((y - y0) / g).astype(int)
    ok = (c >= 0) & (c < top.shape[1]) & (r >= 0) & (r < top.shape[0])
    out = np.zeros(len(x), int); out[ok] = grow[r[ok], c[ok]]; return out
# grow each hole by 1.0 mm (labels kept; holes stand >= 4 mm apart)
grow = ndimage.grey_dilation(lab * np.isin(lab, keep), footprint=ndimage.generate_binary_structure(2, 1), size=None, mode="constant") if False else None
dist, (ri, ci) = ndimage.distance_transform_edt(~np.isin(lab, keep), return_indices=True)
grow = np.where(dist * g <= 1.0, lab[ri, ci], 0)
res = {"holes": len(keep), "per_hole": {}}
for name, (run, sm) in RUNS.items():
    S = [s for s in json.load(open(P3 / run / "simulation.json"))["trace"]["samples"] if s["toolpath_id"] == 3 and s["cut_kinematics"] == "helix" and round(s["feed_rate_mm_min"]) == 375]
    p = np.array([s["position"] for s in S]); t = np.array([s["segment_time_s"] for s in S]); mi = np.array([s["move_index"] for s in S])
    h = hole_of(p[:, 0], p[:, 1], grow)
    m = np.load(H / f"stock_corne_{sm}.npz"); xs, ys = m["xs"], m["ys"]; L = m["top"] - m["target"]; Tz = m["top"]
    XX, YY = np.meshgrid(xs, ys); hg = hole_of(XX.ravel(), YY.ravel(), grow).reshape(XX.shape); cell = float(m["g"]) ** 2
    for k in keep:
        r = res["per_hole"].setdefault(int(k), {"x": round(float(cent[keep.index(k)][1] * g + x0), 1), "y": round(float(cent[keep.index(k)][0] * g + y0), 1)})
        sel = h == k; mk = (hg == k)
        over = mk & (L > 0.8)
        r[name] = {"ramp_moves": int(len(set(mi[sel].tolist()))), "ramp_s": round(float(t[sel].sum()), 1),
                   "left_over_0.8_mm2": round(float(over.sum() * cell), 1),
                   "left_peak_mm": round(float(L[mk].max()), 2) if mk.any() else None,
                   "left_peak_top_z": round(float(Tz[mk][np.argmax(L[mk])]), 2) if mk.any() else None}
    tot = {"ramp_moves_in_holes": sum(v[name]["ramp_moves"] for v in res["per_hole"].values()),
           "ramp_s_in_holes": round(sum(v[name]["ramp_s"] for v in res["per_hole"].values()), 1),
           "left_over_0.8_mm2_in_holes": round(sum(v[name]["left_over_0.8_mm2"] for v in res["per_hole"].values()), 1),
           "holes_with_left_over_1mm2": sum(1 for v in res["per_hole"].values() if v[name]["left_over_0.8_mm2"] >= 1.0)}
    res[f"total {name}"] = tot; print(name, json.dumps(tot))
ph = res["per_hole"]
peaks = {n_: [v[n_]["left_peak_mm"] for v in ph.values()] for n_ in RUNS}
for n_ in RUNS:
    a = np.array(peaks[n_]); print(n_, "peak above target per hole: median", round(float(np.median(a)), 2), "max", round(float(a.max()), 2),
                                    "| peak top Z hist", np.histogram([v[n_]["left_peak_top_z"] for v in ph.values()], bins=[0, 3.5, 5.5, 11.5, 17.5, 30])[0].tolist())
json.dump(res, open(H / "hole_leftover.json", "w"), indent=1)
import matplotlib; matplotlib.use("Agg"); import matplotlib.pyplot as plt
fig, ax = plt.subplots(1, 3, figsize=(20, 6))
xs_ = [v["x"] for v in ph.values()]; ys_ = [v["y"] for v in ph.values()]
for k, n_ in enumerate(RUNS):
    sc = ax[k].scatter(xs_, ys_, c=[v[n_]["left_over_0.8_mm2"] for v in ph.values()], s=[20 + 12 * v[n_]["ramp_moves"] for v in ph.values()], cmap="magma_r", vmin=0, vmax=30, edgecolors="k")
    ax[k].set_title(f"{n_}\ncolour: left > 0.8 mm (mm2); size: F375 ramps"); ax[k].set_aspect("equal")
    fig.colorbar(sc, ax=ax[k], shrink=0.7)
a = [v["body_cp"]["left_over_0.8_mm2"] for v in ph.values()]; b = [v["cp_fix6job"]["left_over_0.8_mm2"] for v in ph.values()]
ra = [v["body_cp"]["ramp_moves"] for v in ph.values()]; rb = [v["cp_fix6job"]["ramp_moves"] for v in ph.values()]
ax[2].scatter(np.array(a) - np.array(b), np.array(ra) - np.array(rb), s=12)
ax[2].set_xlabel("left > 0.8 mm per hole: body_cp - cp_fix6job (mm2)"); ax[2].set_ylabel("copy-op F375 ramps per hole: body_cp - cp_fix6job"); ax[2].set_title("per hole: more ramps in body_cp, whatever the leftover"); ax[2].axhline(0, c="grey"); ax[2].axvline(0, c="grey")
fig.savefig(H / "img" / "hole_leftover.png", dpi=100, bbox_inches="tight")
