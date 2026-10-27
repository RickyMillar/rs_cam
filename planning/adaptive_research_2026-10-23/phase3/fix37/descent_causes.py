"""D54: a copy of fix34/descent_causes.py (writes into fix37). D49: a copy of fix33/descent_causes.py (writes into fix34). D47: a copy of fix31/descent_causes.py (writes into fix33); one change: the plot shows the wanaka part outline (the sample frame of setup 0, fix9/stockmap.py: x 20-220, y 25-225), not the Corne part. D45: a copy of fix28/descent_causes.py (writes into fix31). D38: the op-1 descents of a Corne body_cp run by their C40 entry cause.
- Descent time: the trace samples of toolpath 1 in fix26/time_split.py's
  'plunge/ramp' class (source_intent entry_plunge, or linking at plunge
  kinematics), grouped by move. A move's end is its last sample (model frame).
- Cause: each "C40 entry kind=.. cause=.. join=.. x= y= z= drop_mm=" line of
  op 1 (index=0) is the END of one entry run (adaptive3d/mod.rs EntryRecord).
  A descent move takes, among the C40 records within 0.6 mm of its end in XY
  (two sample steps), the highest one that ends at or below the move's end
  (a peck ladder's upper moves belong to the run that ends below them);
  else 'unmatched'.
- Air: a sample removes stock when removed_volume_est_mm3 > cell^2 * step
  (fix11/audit.py's VOLUME_QUANTUM, 0.027 mm3 at 0.3 mm); else air.
Usage: descent_causes.py <run dir under phase3> <tag>. Writes descent_causes_<tag>.json and img/descents_<tag>.png."""
import collections, json, re, sys
from pathlib import Path
import numpy as np
H = Path(__file__).resolve().parent; P3 = H.parent
run, tag = sys.argv[1], sys.argv[2]
sim = json.load(open(P3 / run / "simulation.json")); Q = sim["resolution_mm"] ** 2 * sim["sample_step_mm"]
S = [s for s in sim["trace"]["samples"] if s["toolpath_id"] == 1]
mv = collections.defaultdict(list)
for s in S:
    si = s["source_intent"] if isinstance(s["source_intent"], str) else ""
    if s["cut_kinematics"] != "rapid" and (si == "entry_plunge" or (si == "linking" and s["cut_kinematics"] == "plunge")):
        mv[s["move_index"]].append(s)
RX = re.compile(r"C40 entry kind=(\w+) cause=(?:Some\((\w+)\)|None) join=(?:Some\((\w+)\)|None) x=([-\d.]+) y=([-\d.]+) z=([-\d.]+) drop_mm=([-\d.]+)")
rec = []
for ln in open(P3 / run / "stdout_plain.txt"):
    if "index=0" in ln and "C40 entry" in ln:
        m = RX.search(ln)
        if m:
            k, c, j, x, y, z, dr = m.groups(); rec.append({"kind": k, "cause": c or "Other", "join": j, "x": float(x), "y": float(y), "z": float(z), "drop": float(dr)})
RXY = np.array([[r["x"], r["y"]] for r in rec]); RZ = np.array([r["z"] for r in rec])
# the z offset between a record's z (the level / entry end) and the move's last sample z
ends = {k: v[-1]["position"] for k, v in mv.items()}
dz = []
for k, p in ends.items():
    d = np.hypot(RXY[:, 0] - p[0], RXY[:, 1] - p[1]); i = int(np.argmin(d))
    if d[i] < 0.6: dz.append(p[2] - RZ[i])
off = float(np.median(dz)) if dz else 0.0
# A peck ladder's upper moves end above the run's end: among the records within
# 0.6 mm in XY, take the highest whose end (z + offset) lies at or below the
# move's end (+ 0.6 mm); the run ends there.
cause_of = {}
for k, p in ends.items():
    d = np.hypot(RXY[:, 0] - p[0], RXY[:, 1] - p[1])
    cand = [i for i in np.nonzero(d < 0.6)[0] if RZ[i] + off <= p[2] + 0.6]
    cause_of[k] = int(max(cand, key=lambda i: RZ[i])) if cand else None
agg = collections.defaultdict(lambda: {"records": set(), "moves": 0, "time_s": 0.0, "air_s": 0.0, "len": [], "drop": []})
for k, ss in mv.items():
    i = cause_of[k]; key = "unmatched" if i is None else rec[i]["cause"] + ("" if rec[i]["cause"] != "Reentry2d" else f"/{rec[i]['join']}")
    a = agg[key]; a["moves"] += 1
    if i is not None:
        a["records"].add(i)
    t = sum(s["segment_time_s"] for s in ss); a["time_s"] += t; a["air_s"] += sum(s["segment_time_s"] for s in ss if s["removed_volume_est_mm3"] <= Q)
    P = np.array([s["position"] for s in ss]); a["len"].append(float(np.sum(np.linalg.norm(np.diff(P, axis=0), axis=1))) if len(P) > 1 else 0.0)
for i, r in enumerate(rec):
    key = r["cause"] + ("" if r["cause"] != "Reentry2d" else f"/{r['join']}")
    agg[key]["drop"].append(r["drop"]); agg[key].setdefault("all_records", 0); agg[key]["all_records"] += 1
out = {"run": run, "c40_records": len(rec), "descent_moves": len(mv), "z_offset_move_end_minus_record": round(off, 3), "air_quantum_mm3": Q, "causes": {}}
for key, a in sorted(agg.items(), key=lambda kv: -kv[1]["time_s"]):
    out["causes"][key] = {"c40_records": a.get("all_records", 0), "records_matched": len(a["records"]), "descent_moves": a["moves"], "time_s": round(a["time_s"], 1),
                          "air_share": round(a["air_s"] / a["time_s"], 3) if a["time_s"] else None,
                          "median_move_len_mm": round(float(np.median(a["len"])), 2) if a["len"] else None,
                          "median_drop_mm": round(float(np.median(a["drop"])), 2) if a["drop"] else None}
tot = sum(a["time_s"] for a in agg.values()); air = sum(a["air_s"] for a in agg.values())
out["total_time_s"] = round(tot, 1); out["total_air_share"] = round(air / tot, 3)
print(json.dumps(out, indent=1))
json.dump(out, open(H / f"descent_causes_{tag}.json", "w"), indent=1)
import matplotlib; matplotlib.use("Agg"); import matplotlib.pyplot as plt
from scipy import ndimage
(H / "img").mkdir(exist_ok=True)
fig, ax = plt.subplots(figsize=(11, 8))
ax.plot([20, 220, 220, 20, 20], [25, 25, 225, 225, 25], "k-", lw=0.8)
cols = {}
for i, r in enumerate(rec):
    key = r["cause"] + ("" if r["cause"] != "Reentry2d" else f"/{r['join']}")
    cols.setdefault(key, []).append((r["x"], r["y"], r["drop"]))
for key, pts in sorted(cols.items(), key=lambda kv: -len(kv[1])):
    p = np.array(pts); ax.scatter(p[:, 0], p[:, 1], s=4 + 3 * p[:, 2], label=f"{key} ({len(p)})", alpha=0.7)
ax.set_aspect("equal"); ax.legend(fontsize=8, loc="lower right"); ax.set_title(f"{tag}: op-1 C40 entries by cause (size = drop)")
fig.savefig(H / "img" / f"descents_{tag}.png", dpi=100, bbox_inches="tight")
