"""D66: the entries (feed descents) of op 1 by geometric class, and a plot like fix34/img/descents_corne_B_s2p1.png. The
product logs no C40 cause records (the research instruments were deleted at integration step 7.7), so a descent is classed
by its own geometry from the trace: the samples of op 1 in the plunge/ramp class (source_intent entry_plunge, or linking at
plunge kinematics), grouped by move; drop = the Z fall of the move. Classes by drop: <= 1 mm (a ledge / turn re-entry onto
the same level), 1 mm to one DPP (a re-entry from a held link or a short lift), > one DPP (a level or wall drop, the
LargeZ class). Air: removed_volume_est_mm3 <= cell^2 * step. Usage: entries.py <run dir> <tag> <DPP> <corne|wanaka>"""
import collections, json, sys
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent; P3 = H.parent
d = Path(sys.argv[1]); tag = sys.argv[2]; dpp = float(sys.argv[3]); part = sys.argv[4]
sim = json.load(open(d / "simulation.json")); Q = sim["resolution_mm"] ** 2 * sim["sample_step_mm"]
mv = collections.defaultdict(list)
for s in sim["trace"]["samples"]:
    if s["toolpath_id"] != 1: continue
    si = s["source_intent"] if isinstance(s["source_intent"], str) else ""
    if s["cut_kinematics"] != "rapid" and (si == "entry_plunge" or (si == "linking" and s["cut_kinematics"] == "plunge")): mv[s["move_index"]].append(s)
rows = []
for k, ss in mv.items():
    z = [s["position"][2] for s in ss]; drop = max(z) - min(z) + (sim["sample_step_mm"] if len(ss) == 1 else 0.0)
    rows.append({"x": ss[-1]["position"][0], "y": ss[-1]["position"][1], "drop": drop, "t": sum(s["segment_time_s"] for s in ss),
                 "air_t": sum(s["segment_time_s"] for s in ss if s["removed_volume_est_mm3"] <= Q)})
def cls(r): return "drop <= 1 mm" if r["drop"] <= 1.0 else ("drop 1 mm - 1 DPP" if r["drop"] <= dpp + 0.05 else "drop > 1 DPP")
out = {"run": d.name, "descent_moves": len(rows), "classes": {}}
for c in ("drop <= 1 mm", "drop 1 mm - 1 DPP", "drop > 1 DPP"):
    R = [r for r in rows if cls(r) == c]; t = sum(r["t"] for r in R)
    out["classes"][c] = {"moves": len(R), "time_s": round(t, 1), "air_share": round(sum(r["air_t"] for r in R) / t, 3) if t else None,
                         "median_drop_mm": round(float(np.median([r["drop"] for r in R])), 2) if R else None}
print(json.dumps(out)); json.dump(out, open(H / f"entries_{tag}.json", "w"), indent=1)
import matplotlib; matplotlib.use("Agg"); import matplotlib.pyplot as plt
(H / "img").mkdir(exist_ok=True)
fig, ax = plt.subplots(figsize=(11, 8))
if part == "corne":
    pt = np.load(P3 / "fix9" / "corne_part.npz"); top, g, x0, y0 = pt["top"], float(pt["g"]), float(pt["x0"]), float(pt["y0"])
    ax.imshow(np.where(np.isfinite(top) & (top > 1e-6), top, np.nan), origin="lower", extent=[x0, x0 + top.shape[1] * g, y0, y0 + top.shape[0] * g], cmap="Greys", alpha=0.5)
else:
    ax.plot([20, 220, 220, 20, 20], [25, 25, 225, 225, 25], "k-", lw=0.8)
C = {"drop <= 1 mm": "tab:blue", "drop 1 mm - 1 DPP": "tab:orange", "drop > 1 DPP": "tab:red"}
for c in C:
    R = [r for r in rows if cls(r) == c]
    if R: p = np.array([(r["x"], r["y"], r["drop"]) for r in R]); ax.scatter(p[:, 0], p[:, 1], s=4 + 3 * p[:, 2], c=C[c], alpha=0.7, label=f"{c} ({len(R)})")
ax.set_aspect("equal"); ax.legend(fontsize=8, loc="lower right"); ax.set_title(f"{tag}: op-1 feed descents by drop (size = drop)")
fig.savefig(H / "img" / f"entries_{tag}.png", dpi=100, bbox_inches="tight")
